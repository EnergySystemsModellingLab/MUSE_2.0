//! Assets are instances of a process which are owned and invested in by agents.
use crate::agent::AgentID;
use crate::commodity::CommodityID;
use crate::process::{Process, ProcessFlow, ProcessParameter};
use crate::region::RegionID;
use crate::time_slice::TimeSliceID;
use anyhow::{ensure, Context, Result};
use indexmap::IndexMap;
use std::hash::{Hash, Hasher};
use std::ops::{Deref, RangeInclusive};
use std::rc::Rc;

/// A unique identifier for an asset
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AssetID(u32);

/// An asset controlled by an agent.
#[derive(Clone, Debug, PartialEq)]
pub struct Asset {
    /// A unique identifier for the asset
    id: Option<AssetID>,
    /// A unique identifier for the agent
    pub agent_id: AgentID,
    /// The [`Process`] that this asset corresponds to
    pub process: Rc<Process>,
    /// The [`ProcessParameter`] corresponding to the asset's region and commission year
    pub process_parameter: Rc<ProcessParameter>,
    /// The region in which the asset is located
    pub region_id: RegionID,
    /// Capacity of asset
    pub capacity: f64,
    /// The year the asset comes online
    pub commission_year: u32,
}

impl Asset {
    /// Create a new [`Asset`].
    ///
    /// The `id` field is initially set to `None`, but is changed to a unique value when the asset
    /// is stored in an [`AssetPool`].
    pub fn new(
        agent_id: AgentID,
        process: Rc<Process>,
        region_id: RegionID,
        capacity: f64,
        commission_year: u32,
    ) -> Result<Self> {
        ensure!(
            process.regions.contains(&region_id),
            "Region {} is not one of the regions in which process {} operates",
            region_id,
            process.id
        );

        let process_parameter = process
            .parameters
            .get(&(region_id.clone(), commission_year))
            .with_context(|| {
                format!(
                    "Process {} does not operate in the year {}",
                    process.id, commission_year
                )
            })?
            .clone();

        ensure!(
            capacity.is_finite() && capacity > 0.0,
            "Capacity must be a finite, positive number"
        );

        Ok(Self {
            id: None,
            agent_id,
            process,
            process_parameter,
            region_id,
            capacity,
            commission_year,
        })
    }

    /// The last year in which this asset should be decommissioned
    pub fn decommission_year(&self) -> u32 {
        self.commission_year + self.process_parameter.lifetime
    }

    /// Get the energy limits for this asset in a particular time slice
    ///
    /// This is an absolute max and min on the PAC energy produced/consumed in that time slice.
    pub fn get_energy_limits(&self, time_slice: &TimeSliceID) -> RangeInclusive<f64> {
        let limits = self
            .process
            .energy_limits
            .get(&(
                self.region_id.clone(),
                self.commission_year,
                time_slice.clone(),
            ))
            .unwrap();
        let max_act = self.maximum_activity();

        // Multiply the fractional energy limits by this asset's maximum activity to get energy
        // limits in real units (which are user defined)
        (max_act * limits.start())..=(max_act * limits.end())
    }

    /// Maximum activity for this asset (PAC energy produced/consumed per year)
    pub fn maximum_activity(&self) -> f64 {
        self.capacity * self.process_parameter.capacity_to_activity
    }

    /// Get a specific process flow
    pub fn get_flow(&self, commodity_id: &CommodityID) -> Option<&ProcessFlow> {
        self.get_flows_map().get(commodity_id)
    }

    /// Get the process flows map for this asset
    fn get_flows_map(&self) -> &IndexMap<CommodityID, ProcessFlow> {
        self.process
            .flows
            .get(&(self.region_id.clone(), self.commission_year))
            .unwrap()
    }

    /// Iterate over the asset's flows
    pub fn iter_flows(&self) -> impl Iterator<Item = &ProcessFlow> {
        self.get_flows_map().values()
    }

    /// Iterate over the asset's Primary Activity Commodity flows
    pub fn iter_pacs(&self) -> impl Iterator<Item = &ProcessFlow> {
        self.process
            .iter_pacs(&self.region_id, self.commission_year)
    }
}

/// A wrapper around [`Asset`] for storing references in maps.
///
/// An [`AssetRef`] is guaranteed to have been commissioned at some point, though it may
/// subsequently have been decommissioned.
#[derive(Clone, Debug)]
pub struct AssetRef(Rc<Asset>);

impl From<Rc<Asset>> for AssetRef {
    fn from(value: Rc<Asset>) -> Self {
        assert!(value.id.is_some());
        Self(value)
    }
}

impl From<Asset> for AssetRef {
    fn from(value: Asset) -> Self {
        Self::from(Rc::new(value))
    }
}

impl From<AssetRef> for Rc<Asset> {
    fn from(value: AssetRef) -> Self {
        value.0
    }
}

impl Deref for AssetRef {
    type Target = Asset;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PartialEq for AssetRef {
    fn eq(&self, other: &Self) -> bool {
        self.0.id == other.0.id
    }
}

impl Eq for AssetRef {}

impl Hash for AssetRef {
    /// Hash asset based purely on its ID.
    ///
    /// Panics if the asset has not been commissioned (and therefore has no ID).
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.id.unwrap().hash(state);
    }
}

/// A pool of [`Asset`]s
pub struct AssetPool {
    /// The pool of active assets
    active: Vec<AssetRef>,
    /// Assets that have not yet been commissioned, sorted by commission year
    future: Vec<Asset>,
    /// Next available asset ID number
    next_id: u32,
}

impl AssetPool {
    /// Create a new [`AssetPool`]
    pub fn new(mut assets: Vec<Asset>) -> Self {
        // Sort in order of commission year
        assets.sort_by(|a, b| a.commission_year.cmp(&b.commission_year));

        Self {
            active: Vec::new(),
            future: assets,
            next_id: 0,
        }
    }

    /// Commission new assets for the specified milestone year from the input data
    pub fn commission_new(&mut self, year: u32) {
        // Count the number of assets to move
        let count = self
            .future
            .iter()
            .take_while(|asset| asset.commission_year <= year)
            .count();

        // Move assets from future to active
        for mut asset in self.future.drain(0..count) {
            asset.id = Some(AssetID(self.next_id));
            self.next_id += 1;
            self.active.push(asset.into());
        }
    }

    /// Decommission old assets for the specified milestone year
    pub fn decommission_old(&mut self, year: u32) {
        self.active.retain(|asset| asset.decommission_year() > year);
    }

    /// Get an asset with the specified ID.
    ///
    /// # Returns
    ///
    /// An [`AssetRef`] if found, else `None`. The asset may not be found if it has already been
    /// decommissioned.
    pub fn get(&self, id: AssetID) -> Option<&AssetRef> {
        // The assets in `active` are in order of ID
        let idx = self
            .active
            .binary_search_by(|asset| asset.id.unwrap().cmp(&id))
            .ok()?;

        Some(&self.active[idx])
    }

    /// Iterate over active assets
    pub fn iter(&self) -> impl Iterator<Item = &AssetRef> {
        self.active.iter()
    }

    /// Iterate over active assets for a particular region
    pub fn iter_for_region<'a>(
        &'a self,
        region_id: &'a RegionID,
    ) -> impl Iterator<Item = &'a AssetRef> {
        self.iter().filter(|asset| asset.region_id == *region_id)
    }

    /// Iterate over only the active assets in a given region that produce or consume a given
    /// commodity
    pub fn iter_for_region_and_commodity<'a>(
        &'a self,
        region_id: &'a RegionID,
        commodity_id: &'a CommodityID,
    ) -> impl Iterator<Item = &'a AssetRef> {
        self.iter_for_region(region_id).filter(|asset| {
            asset.process.contains_commodity_flow(
                commodity_id,
                &asset.region_id,
                asset.commission_year,
            )
        })
    }

    /// Replace the active pool with new and/or already commissioned assets
    pub fn replace_active_pool<I>(&mut self, assets: I)
    where
        I: IntoIterator<Item = Rc<Asset>>,
    {
        let new_pool = assets.into_iter().map(|mut asset| {
            if asset.id.is_some() {
                // Already commissioned
                asset.into()
            } else {
                let mut update_id = |asset: &mut Asset| {
                    // We need to assign an ID
                    asset.id = Some(AssetID(self.next_id));
                    self.next_id += 1;
                };

                // Asset is newly created from process. We use `get_mut` to avoid a clone in the
                // (likely) case that there is only one reference to `asset`
                let rc_asset = match Rc::get_mut(&mut asset) {
                    Some(asset_inner) => {
                        update_id(asset_inner);
                        asset
                    }
                    None => {
                        let mut asset = asset.as_ref().clone();
                        update_id(&mut asset);
                        asset.into()
                    }
                };

                rc_asset.into()
            }
        });

        self.active.clear();
        self.active.extend(new_pool);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::{assert_error, process};
    use crate::process::{
        Process, ProcessEnergyLimitsMap, ProcessFlowsMap, ProcessParameter, ProcessParameterMap,
    };
    use itertools::{assert_equal, Itertools};
    use rstest::{fixture, rstest};
    use std::collections::HashSet;
    use std::iter;
    use std::ops::RangeInclusive;

    #[rstest]
    #[case(0.01)]
    #[case(0.5)]
    #[case(1.0)]
    #[case(100.0)]
    fn test_asset_new_valid(process: Process, #[case] capacity: f64) {
        let agent_id = AgentID("agent1".into());
        let region_id = RegionID("GBR".into());
        let asset = Asset::new(agent_id, process.into(), region_id, capacity, 2015).unwrap();
        assert!(asset.id.is_none());
    }

    #[rstest]
    #[case(0.0)]
    #[case(-0.01)]
    #[case(-1.0)]
    #[case(f64::NAN)]
    #[case(f64::INFINITY)]
    #[case(f64::NEG_INFINITY)]
    fn test_asset_new_invalid_capacity(process: Process, #[case] capacity: f64) {
        let agent_id = AgentID("agent1".into());
        let region_id = RegionID("GBR".into());
        assert_error!(
            Asset::new(agent_id, process.into(), region_id, capacity, 2015),
            "Capacity must be a finite, positive number"
        );
    }

    #[rstest]
    fn test_asset_new_invalid_commission_year(process: Process) {
        let agent_id = AgentID("agent1".into());
        let region_id = RegionID("GBR".into());
        assert_error!(
            Asset::new(agent_id, process.into(), region_id, 1.0, 2009),
            "Process process1 does not operate in the year 2009"
        );
    }

    #[rstest]
    fn test_asset_new_invalid_region(process: Process) {
        let agent_id = AgentID("agent1".into());
        let region_id = RegionID("FRA".into());
        assert_error!(
            Asset::new(agent_id, process.into(), region_id, 1.0, 2015),
            "Region FRA is not one of the regions in which process process1 operates"
        );
    }

    #[fixture]
    fn asset_pool() -> AssetPool {
        let process_param = Rc::new(ProcessParameter {
            capital_cost: 5.0,
            fixed_operating_cost: 2.0,
            variable_operating_cost: 1.0,
            lifetime: 5,
            discount_rate: 0.9,
            capacity_to_activity: 1.0,
        });
        let years = RangeInclusive::new(2010, 2020).collect_vec();
        let process_parameter_map: ProcessParameterMap = years
            .iter()
            .map(|&year| (("GBR".into(), year), process_param.clone()))
            .collect();
        let process = Rc::new(Process {
            id: "process1".into(),
            description: "Description".into(),
            years: vec![2010, 2020],
            energy_limits: ProcessEnergyLimitsMap::new(),
            flows: ProcessFlowsMap::new(),
            parameters: process_parameter_map,
            regions: HashSet::from(["GBR".into()]),
        });
        let future = [2020, 2010]
            .map(|year| {
                Asset::new(
                    "agent1".into(),
                    Rc::clone(&process),
                    "GBR".into(),
                    1.0,
                    year,
                )
                .unwrap()
            })
            .into_iter()
            .collect_vec();

        AssetPool::new(future)
    }

    #[test]
    fn test_asset_get_energy_limits() {
        let time_slice = TimeSliceID {
            season: "winter".into(),
            time_of_day: "day".into(),
        };
        let process_param = Rc::new(ProcessParameter {
            capital_cost: 5.0,
            fixed_operating_cost: 2.0,
            variable_operating_cost: 1.0,
            lifetime: 5,
            discount_rate: 0.9,
            capacity_to_activity: 3.0,
        });
        let years = RangeInclusive::new(2010, 2020).collect_vec();
        let process_parameter_map: ProcessParameterMap = years
            .iter()
            .map(|&year| (("GBR".into(), year), process_param.clone()))
            .collect();
        let fraction_limits = 1.0..=f64::INFINITY;
        let mut energy_limits = ProcessEnergyLimitsMap::new();
        for year in [2010, 2020] {
            energy_limits.insert(
                ("GBR".into(), year, time_slice.clone()),
                fraction_limits.clone(),
            );
        }
        let process = Rc::new(Process {
            id: "process1".into(),
            description: "Description".into(),
            years: vec![2010, 2020],
            energy_limits,
            flows: ProcessFlowsMap::new(),
            parameters: process_parameter_map,
            regions: HashSet::from(["GBR".into()]),
        });
        let asset = Asset::new(
            "agent1".into(),
            Rc::clone(&process),
            "GBR".into(),
            2.0,
            2010,
        )
        .unwrap();

        assert_eq!(asset.get_energy_limits(&time_slice), 6.0..=f64::INFINITY);
    }

    #[rstest]
    fn test_asset_pool_new(asset_pool: AssetPool) {
        // Should be in order of commission year
        assert!(asset_pool.active.is_empty());
        assert!(asset_pool.future.len() == 2);
        assert!(asset_pool.future[0].commission_year == 2010);
        assert!(asset_pool.future[1].commission_year == 2020);
    }

    #[rstest]
    fn test_asset_pool_commission_new1(mut asset_pool: AssetPool) {
        // Asset to be commissioned in this year
        asset_pool.commission_new(2010);
        assert_equal(asset_pool.iter(), iter::once(&asset_pool.active[0]));
    }

    #[rstest]
    fn test_asset_pool_commission_new2(mut asset_pool: AssetPool) {
        // Commission year has passed
        asset_pool.commission_new(2011);
        assert_equal(asset_pool.iter(), iter::once(&asset_pool.active[0]));
    }

    #[rstest]
    fn test_asset_pool_commission_new3(mut asset_pool: AssetPool) {
        // Nothing to commission for this year
        asset_pool.commission_new(2000);
        assert!(asset_pool.iter().next().is_none()); // no active assets
    }

    #[rstest]
    fn test_asset_pool_decommission_old(mut asset_pool: AssetPool) {
        asset_pool.commission_new(2020);
        assert_eq!(asset_pool.active.len(), 2);
        asset_pool.decommission_old(2020); // should decommission first asset (lifetime == 5)
        assert_eq!(asset_pool.active.len(), 1);
        assert_eq!(asset_pool.active[0].commission_year, 2020);
        asset_pool.decommission_old(2022); // nothing to decommission
        assert_eq!(asset_pool.active.len(), 1);
        assert_eq!(asset_pool.active[0].commission_year, 2020);
        asset_pool.decommission_old(2025); // should decommission second asset
        assert!(asset_pool.active.is_empty());
    }

    #[rstest]
    fn test_asset_pool_get(mut asset_pool: AssetPool) {
        asset_pool.commission_new(2020);
        assert_eq!(asset_pool.get(AssetID(0)), Some(&asset_pool.active[0]));
        assert_eq!(asset_pool.get(AssetID(1)), Some(&asset_pool.active[1]));
    }

    #[rstest]
    fn test_asset_pool_replace_active_pool_existing(mut asset_pool: AssetPool) {
        asset_pool.commission_new(2020);
        assert_eq!(asset_pool.active.len(), 2);
        asset_pool.replace_active_pool(iter::once(asset_pool.active[1].clone().into()));
        assert_eq!(asset_pool.active.len(), 1);
        assert_eq!(asset_pool.active[0].id, Some(AssetID(1)));
    }

    #[rstest]
    fn test_asset_pool_replace_active_pool_new_asset(mut asset_pool: AssetPool, process: Process) {
        let asset = Asset::new(
            "some_other_agent".into(),
            process.into(),
            "GBR".into(),
            2.0,
            2010,
        )
        .unwrap();

        asset_pool.commission_new(2020);
        assert_eq!(asset_pool.active.len(), 2);
        asset_pool.replace_active_pool(iter::once(asset.into()));
        assert_eq!(asset_pool.active.len(), 1);
        assert_eq!(asset_pool.active[0].id, Some(AssetID(2)));
        assert_eq!(asset_pool.active[0].agent_id, "some_other_agent".into());
    }
}
