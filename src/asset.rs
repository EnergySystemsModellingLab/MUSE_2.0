//! Assets are instances of a process which are owned and invested in by agents.
use crate::agent::AgentID;
use crate::commodity::CommodityID;
use crate::process::{Process, ProcessFlow, ProcessParameter};
use crate::region::RegionID;
use crate::time_slice::TimeSliceID;
use anyhow::{ensure, Context, Result};
use std::collections::HashSet;
use std::ops::RangeInclusive;
use std::rc::Rc;

/// A unique identifier for an asset
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AssetID(u32);

impl AssetID {
    /// Sentinel value assigned to [`Asset`]s when they are initially created
    pub const INVALID: AssetID = AssetID(u32::MAX);
}

/// An asset controlled by an agent.
#[derive(Clone, Debug, PartialEq)]
pub struct Asset {
    /// A unique identifier for the asset
    pub id: AssetID,
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
    /// The `id` field is initially set to [`AssetID::INVALID`], but is changed to a unique value
    /// when the asset is stored in an [`AssetPool`].
    pub fn new(
        agent_id: AgentID,
        process: Rc<Process>,
        region_id: RegionID,
        capacity: f64,
        commission_year: u32,
    ) -> Result<Self> {
        ensure!(commission_year > 0, "Commission year must be > 0");
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

        Ok(Self {
            id: AssetID::INVALID,
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

        // Multiply the fractional capacity in self.process by this asset's actual capacity
        (max_act * limits.start())..=(max_act * limits.end())
    }

    /// Maximum activity for this asset (PAC energy produced/consumed per year)
    pub fn maximum_activity(&self) -> f64 {
        self.capacity * self.process_parameter.capacity_to_activity
    }

    /// Iterate over the asset's flows
    pub fn iter_flows(&self) -> impl Iterator<Item = &ProcessFlow> {
        self.process
            .flows
            .get(&(self.region_id.clone(), self.commission_year))
            .unwrap()
            .values()
    }

    /// Iterate over the asset's Primary Activity Commodity flows
    pub fn iter_pacs(&self) -> impl Iterator<Item = &ProcessFlow> {
        self.process
            .iter_pacs(&self.region_id, self.commission_year)
    }
}

/// A pool of [`Asset`]s
pub struct AssetPool {
    /// The pool of assets, both active and yet to be commissioned.
    ///
    /// Sorted in order of commission year.
    assets: Vec<Asset>,
    /// Current milestone year.
    current_year: u32,
}

impl AssetPool {
    /// Create a new [`AssetPool`]
    pub fn new(mut assets: Vec<Asset>) -> Self {
        // Sort in order of commission year
        assets.sort_by(|a, b| a.commission_year.cmp(&b.commission_year));

        // Assign each asset a unique ID
        for (id, asset) in assets.iter_mut().enumerate() {
            asset.id = AssetID(id as u32);
        }

        Self {
            assets,
            current_year: 0,
        }
    }

    /// Commission new assets for the specified milestone year
    pub fn commission_new(&mut self, year: u32) {
        assert!(
            year >= self.current_year,
            "Assets have already been commissioned for year {year}"
        );
        self.current_year = year;
    }

    /// Decommission old assets for the specified milestone year
    pub fn decomission_old(&mut self, year: u32) {
        assert!(
            year >= self.current_year,
            "Cannot decommission assets in the past (current year: {})",
            self.current_year
        );
        self.assets.retain(|asset| asset.decommission_year() > year);
    }

    /// Get an asset with the specified ID.
    ///
    /// # Returns
    ///
    /// Reference to an [`Asset`] if found, else `None`. The asset may not be found if it has
    /// already been decommissioned.
    pub fn get(&self, id: AssetID) -> Option<&Asset> {
        // The assets in `active` are in order of ID
        let idx = self
            .assets
            .binary_search_by(|asset| asset.id.cmp(&id))
            .ok()?;

        Some(&self.assets[idx])
    }

    /// Iterate over active assets
    pub fn iter(&self) -> impl Iterator<Item = &Asset> {
        self.assets
            .iter()
            .take_while(|asset| asset.commission_year <= self.current_year)
    }

    /// Iterate over active assets for a particular region
    pub fn iter_for_region<'a>(
        &'a self,
        region_id: &'a RegionID,
    ) -> impl Iterator<Item = &'a Asset> {
        self.iter().filter(|asset| asset.region_id == *region_id)
    }

    /// Iterate over only the active assets in a given region that produce or consume a given
    /// commodity
    pub fn iter_for_region_and_commodity<'a>(
        &'a self,
        region_id: &'a RegionID,
        commodity_id: &'a CommodityID,
    ) -> impl Iterator<Item = &'a Asset> {
        self.iter_for_region(region_id).filter(|asset| {
            asset.process.contains_commodity_flow(
                commodity_id,
                &asset.region_id,
                asset.commission_year,
            )
        })
    }

    /// Retain all assets whose IDs are in `assets_to_keep`.
    ///
    /// Other assets will be decommissioned. Assets which have not yet been commissioned will not be
    /// affected.
    pub fn retain(&mut self, assets_to_keep: &HashSet<AssetID>) {
        // Sanity check: all IDs should be valid. As this check is slow, only do it for debug
        // builds.
        debug_assert!(
            assets_to_keep.iter().all(|id| self.get(*id).is_some()),
            "One or more asset IDs were invalid"
        );

        self.assets.retain(|asset| {
            assets_to_keep.contains(&asset.id) || asset.commission_year > self.current_year
        });
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
    use std::iter;
    use std::ops::RangeInclusive;

    #[rstest]
    fn test_asset_new_valid(process: Process) {
        let agent_id = AgentID("agent1".into());
        let region_id = RegionID("GBR".into());
        let asset = Asset::new(agent_id, process.into(), region_id, 1.0, 2015).unwrap();
        assert!(asset.id == AssetID::INVALID);
    }

    #[rstest]
    fn test_asset_new_invalid_commission_year_zero(process: Process) {
        let agent_id = AgentID("agent1".into());
        let region_id = RegionID("GBR".into());
        assert_error!(
            Asset::new(agent_id, process.into(), region_id, 1.0, 0),
            "Commission year must be > 0"
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
        assert!(asset_pool.current_year == 0);

        // Should be in order of commission year
        assert!(asset_pool.assets.len() == 2);
        assert!(asset_pool.assets[0].commission_year == 2010);
        assert!(asset_pool.assets[1].commission_year == 2020);
    }

    #[rstest]
    fn test_asset_pool_commission_new1(mut asset_pool: AssetPool) {
        // Asset to be commissioned in this year
        asset_pool.commission_new(2010);
        assert!(asset_pool.current_year == 2010);
        assert_equal(asset_pool.iter(), iter::once(&asset_pool.assets[0]));
    }

    #[rstest]
    fn test_asset_pool_commission_new2(mut asset_pool: AssetPool) {
        // Commission year has passed
        asset_pool.commission_new(2011);
        assert!(asset_pool.current_year == 2011);
        assert_equal(asset_pool.iter(), iter::once(&asset_pool.assets[0]));
    }

    #[rstest]
    fn test_asset_pool_commission_new3(mut asset_pool: AssetPool) {
        // Nothing to commission for this year
        asset_pool.commission_new(2000);
        assert!(asset_pool.current_year == 2000);
        assert!(asset_pool.iter().next().is_none()); // no active assets
    }

    #[rstest]
    fn test_asset_pool_decommission_old(mut asset_pool: AssetPool) {
        let asset_pool2 = asset_pool.assets.clone();

        asset_pool.commission_new(2020);
        assert!(asset_pool.assets.len() == 2);
        asset_pool.decomission_old(2020); // should decommission first asset (lifetime == 5)
        assert_equal(&asset_pool.assets, iter::once(&asset_pool2[1]));
        asset_pool.decomission_old(2022); // nothing to decommission
        assert_equal(&asset_pool.assets, iter::once(&asset_pool2[1]));
        asset_pool.decomission_old(2025); // should decommission second asset
        assert!(asset_pool.assets.is_empty());
    }

    #[rstest]
    fn test_asset_pool_get(mut asset_pool: AssetPool) {
        asset_pool.commission_new(2020);
        assert!(asset_pool.get(AssetID(0)) == Some(&asset_pool.assets[0]));
        assert!(asset_pool.get(AssetID(1)) == Some(&asset_pool.assets[1]));
    }

    #[rstest]
    fn test_asset_pool_retain1(mut asset_pool: AssetPool) {
        // Even though we are retaining no assets, none have been commissioned so the asset pool
        // should not be changed
        asset_pool.retain(&HashSet::new());
        assert_eq!(asset_pool.assets.len(), 2);

        // Decommission all active assets
        asset_pool.commission_new(2010); // Commission first asset
        asset_pool.retain(&HashSet::new());
        assert_eq!(asset_pool.assets.len(), 1);
        assert_eq!(asset_pool.assets[0].id, AssetID(1));
    }

    #[rstest]
    fn test_asset_pool_retain2(mut asset_pool: AssetPool) {
        // Decommission single asset
        asset_pool.commission_new(2020); // Commission all assets
        asset_pool.retain(&iter::once(AssetID(1)).collect());
        assert_eq!(asset_pool.assets.len(), 1);
        assert_eq!(asset_pool.assets[0].id, AssetID(1));
    }
}
