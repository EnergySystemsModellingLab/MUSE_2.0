//! Assets are instances of a process which are owned and invested in by agents.
use crate::agent::AgentID;
use crate::commodity::CommodityID;
use crate::process::{Process, ProcessFlow, ProcessParameter};
use crate::region::RegionID;
use crate::time_slice::TimeSliceID;
use crate::units::{Activity, ActivityPerCapacity, Capacity, MoneyPerActivity};
use anyhow::{ensure, Context, Result};
use indexmap::IndexMap;
use itertools::{chain, Itertools};
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use std::ops::{Deref, RangeInclusive};
use std::rc::Rc;
use std::slice;

/// A unique identifier for an asset
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
pub struct AssetID(u32);

/// An asset controlled by an agent.
#[derive(Clone, PartialEq)]
pub struct Asset {
    /// A unique identifier for the asset
    pub id: Option<AssetID>,
    /// A unique identifier for the agent.
    ///
    /// Candidate assets may not have an agent ID.
    pub agent_id: Option<AgentID>,
    /// The [`Process`] that this asset corresponds to
    pub process: Rc<Process>,
    /// The [`ProcessParameter`] corresponding to the asset's region and commission year
    pub process_parameter: Rc<ProcessParameter>,
    /// The region in which the asset is located
    pub region_id: RegionID,
    /// Capacity of asset
    pub capacity: Capacity,
    /// The year the asset was commissioned
    pub commission_year: u32,
    /// The year the asset was decommissioned (if relevant)
    pub decommission_year: Option<u32>,
}

impl Asset {
    /// Create a new [`Asset`].
    ///
    /// The `id` field is initially set to `None`, but is changed to a unique value when the asset
    /// is stored in an [`AssetPool`].
    pub fn new(
        agent_id: Option<AgentID>,
        process: Rc<Process>,
        region_id: RegionID,
        capacity: Capacity,
        commission_year: u32,
    ) -> Result<Self> {
        let asset = Self::new_without_capacity(agent_id, process, region_id, commission_year)?;

        check_capacity_valid_for_asset(capacity)?;
        Ok(Self { capacity, ..asset })
    }

    /// Create a new [`Asset`] without any capacity.
    ///
    /// Only candidate assets should have no capacity.
    pub fn new_without_capacity(
        agent_id: Option<AgentID>,
        process: Rc<Process>,
        region_id: RegionID,
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

        Ok(Self {
            id: None,
            agent_id,
            process,
            process_parameter,
            region_id,
            capacity: Capacity(0.0),
            commission_year,
            decommission_year: None,
        })
    }

    /// The last year in which this asset should be decommissioned
    pub fn max_decommission_year(&self) -> u32 {
        self.commission_year + self.process_parameter.lifetime
    }

    /// Get the activity limits for this asset in a particular time slice
    pub fn get_activity_limits(&self, time_slice: &TimeSliceID) -> RangeInclusive<Activity> {
        let limits = self
            .process
            .activity_limits
            .get(&(
                self.region_id.clone(),
                self.commission_year,
                time_slice.clone(),
            ))
            .unwrap();
        let max_act = self.max_activity();

        // limits in real units (which are user defined)
        (max_act * *limits.start())..=(max_act * *limits.end())
    }

    /// Get the activity limits per unit of capacity for this asset in a particular time slice
    pub fn get_activity_per_capacity_limits(
        &self,
        time_slice: &TimeSliceID,
    ) -> RangeInclusive<ActivityPerCapacity> {
        let limits = self
            .process
            .activity_limits
            .get(&(
                self.region_id.clone(),
                self.commission_year,
                time_slice.clone(),
            ))
            .unwrap();
        let cap2act = self.process_parameter.capacity_to_activity;
        (cap2act * *limits.start())..=(cap2act * *limits.end())
    }

    /// Get the operating cost for this asset in a given year and time slice
    pub fn get_operating_cost(&self, year: u32, time_slice: &TimeSliceID) -> MoneyPerActivity {
        // The cost for all commodity flows (including levies/incentives)
        let flows_cost: MoneyPerActivity = self
            .iter_flows()
            .map(|flow| flow.get_total_cost(&self.region_id, year, time_slice))
            .sum();

        self.process_parameter.variable_operating_cost + flows_cost
    }

    /// Maximum activity for this asset
    pub fn max_activity(&self) -> Activity {
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

    /// Whether this asset has been commissioned
    pub fn is_commissioned(&self) -> bool {
        self.id.is_some()
    }

    /// Iterate over the asset's flows
    pub fn iter_flows(&self) -> impl Iterator<Item = &ProcessFlow> {
        self.get_flows_map().values()
    }

    /// Get the primary output (if any) for this asset
    pub fn primary_output(&self) -> Option<&ProcessFlow> {
        self.get_flows_map()
            .values()
            .find(|flow| flow.is_primary_output)
    }
}

impl std::fmt::Debug for Asset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Asset")
            .field("id", &self.id)
            .field("agent_id", &self.agent_id)
            .field("process", &self.process.id)
            .field("region_id", &self.region_id)
            .field("capacity", &self.capacity)
            .field("commission_year", &self.commission_year)
            .finish()
    }
}

/// Whether the specified value is a valid capacity for an asset
pub fn check_capacity_valid_for_asset(capacity: Capacity) -> Result<()> {
    ensure!(
        capacity.is_finite() && capacity > Capacity(0.0),
        "Capacity must be a finite, positive number"
    );

    Ok(())
}

/// A wrapper around [`Asset`] for storing references in maps.
///
/// If the asset has been commissioned, then comparison and hashing is done based on the asset ID,
/// otherwise a combination of other parameters is used.
///
/// [`Ord`] is implemented for [`AssetRef`], but it will panic for non-commissioned assets.
#[derive(Clone, Debug)]
pub struct AssetRef(Rc<Asset>);

impl AssetRef {
    /// Make a mutable reference to the underlying [`Asset`]
    pub fn make_mut(&mut self) -> &mut Asset {
        Rc::make_mut(&mut self.0)
    }
}

impl From<Rc<Asset>> for AssetRef {
    fn from(value: Rc<Asset>) -> Self {
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
        if self.0.id.is_some() {
            self.0.id == other.0.id
        } else {
            other.0.id.is_none()
                && Rc::ptr_eq(&self.0.process, &other.0.process)
                && self.0.region_id == other.0.region_id
                && self.0.commission_year == other.0.commission_year
        }
    }
}

impl Eq for AssetRef {}

impl Hash for AssetRef {
    /// Hash asset based purely on its ID
    fn hash<H: Hasher>(&self, state: &mut H) {
        if let Some(id) = self.0.id {
            id.hash(state);
        } else {
            self.0.process.id.hash(state);
            self.0.region_id.hash(state);
            self.0.commission_year.hash(state);
        }
    }
}

impl PartialOrd for AssetRef {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AssetRef {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.unwrap().cmp(&other.id.unwrap())
    }
}

/// Convert the specified assets to being decommissioned and return
fn decommission_assets<'a, I>(assets: I, year: u32) -> impl Iterator<Item = AssetRef> + 'a
where
    I: IntoIterator<Item = AssetRef> + 'a,
{
    assets.into_iter().map(move |mut asset| {
        assert!(
            asset.is_commissioned(),
            "Cannot decommission an asset that hasn't been commissioned"
        );
        assert!(
            asset.decommission_year.is_none(),
            "Asset decommissioned twice"
        );
        asset.make_mut().decommission_year = Some(year);

        asset
    })
}

/// A pool of [`Asset`]s
pub struct AssetPool {
    /// The pool of active assets, sorted by ID
    pub active: Vec<AssetRef>,
    /// Assets that have not yet been commissioned, sorted by commission year
    future: Vec<Asset>,
    /// Assets that have been decommissioned
    decommissioned: Vec<AssetRef>,
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
            decommissioned: Vec::new(),
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
        // Remove assets which are due for decommissioning
        let to_decommission = self
            .active
            .extract_if(.., |asset| asset.max_decommission_year() <= year);

        // Set `decommission_year` and copy to `self.decommissioned`
        let decommissioned = decommission_assets(to_decommission, year);
        self.decommissioned.extend(decommissioned);
    }

    /// Decommission the specified assets if they are no longer in the active pool.
    ///
    /// # Arguments
    ///
    /// * `assets` - Assets to possibly decommission
    /// * `year` - Decommissioning year
    ///
    /// # Panics
    ///
    /// Panics if any of the provided assets was never commissioned or has already been
    /// decommissioned.
    pub fn decommission_if_not_active<I>(&mut self, assets: I, year: u32)
    where
        I: IntoIterator<Item = AssetRef>,
    {
        // Decommission assets not found in active pool
        let to_decommission = assets.into_iter().filter(|asset| {
            let id = asset
                .id
                .expect("Cannot decommission asset that has not been commissioned");

            // Return true if asset **not** in active pool
            self.active
                .binary_search_by(|asset| asset.id.unwrap().cmp(&id))
                .is_err()
        });

        // Set `decommission_year` and copy to `self.decommissioned`
        let decommissioned = decommission_assets(to_decommission, year);
        self.decommissioned.extend(decommissioned);
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
    pub fn iter_active(&self) -> slice::Iter<AssetRef> {
        self.active.iter()
    }

    /// Iterate over decommissioned assets
    pub fn iter_decommissioned(&self) -> slice::Iter<AssetRef> {
        self.decommissioned.iter()
    }

    /// Iterate over all commissioned and decommissioned assets.
    ///
    /// NB: Not-yet-commissioned assets are not included.
    pub fn iter_all(&self) -> impl Iterator<Item = &AssetRef> {
        chain(self.iter_active(), self.iter_decommissioned())
    }

    /// Return current active pool and clear
    pub fn take(&mut self) -> Vec<AssetRef> {
        std::mem::take(&mut self.active)
    }

    /// Extend the active pool with existing or candidate assets
    pub fn extend<I>(&mut self, assets: I)
    where
        I: IntoIterator<Item = AssetRef>,
    {
        let assets = assets.into_iter().map(|mut asset| {
            if !asset.is_commissioned() {
                // Asset is newly created from process so we need to assign an ID
                let asset = asset.make_mut();
                asset.id = Some(AssetID(self.next_id));
                self.next_id += 1;
            }

            asset
        });

        // Add assets to pool
        self.active.extend(assets);

        // New assets may not have been sorted, but active needs to be sorted by ID
        self.active.sort();

        // Sanity check: all assets should be unique
        debug_assert_eq!(self.active.iter().unique().count(), self.active.len());
    }
}

/// Additional methods for iterating over assets
pub trait AssetIterator<'a>: Iterator<Item = &'a AssetRef> + Sized
where
    Self: 'a,
{
    /// Filter assets by the agent that owns them
    fn filter_agent(self, agent_id: &'a AgentID) -> impl Iterator<Item = &'a AssetRef> + 'a {
        self.filter(move |asset| asset.agent_id.as_ref() == Some(agent_id))
    }

    /// Iterate over assets that have the given commodity as a primary output
    fn filter_primary_producers_of(
        self,
        commodity_id: &'a CommodityID,
    ) -> impl Iterator<Item = &'a AssetRef> + 'a {
        self.filter(move |asset| {
            asset
                .primary_output()
                .is_some_and(|flow| &flow.commodity.id == commodity_id)
        })
    }

    /// Filter the assets by region
    fn filter_region(self, region_id: &'a RegionID) -> impl Iterator<Item = &'a AssetRef> + 'a {
        self.filter(move |asset| asset.region_id == *region_id)
    }

    /// Iterate over process flows affecting the given commodity
    fn flows_for_commodity(
        self,
        commodity_id: &'a CommodityID,
    ) -> impl Iterator<Item = (&'a AssetRef, &'a ProcessFlow)> + 'a {
        self.filter_map(|asset| Some((asset, asset.get_flow(commodity_id)?)))
    }
}

impl<'a, I> AssetIterator<'a> for I where I: Iterator<Item = &'a AssetRef> + Sized + 'a {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::{assert_error, process, time_slice};
    use crate::process::{
        Process, ProcessActivityLimitsMap, ProcessFlowsMap, ProcessParameter, ProcessParameterMap,
    };
    use crate::units::{
        ActivityPerCapacity, Dimensionless, MoneyPerActivity, MoneyPerCapacity,
        MoneyPerCapacityPerYear,
    };
    use indexmap::IndexSet;
    use itertools::{assert_equal, Itertools};
    use rstest::{fixture, rstest};
    use std::iter;
    use std::ops::RangeInclusive;

    #[rstest]
    #[case(Capacity(0.01))]
    #[case(Capacity(0.5))]
    #[case(Capacity(1.0))]
    #[case(Capacity(100.0))]
    fn test_asset_new_valid(process: Process, #[case] capacity: Capacity) {
        let agent_id = Some(AgentID("agent1".into()));
        let region_id = RegionID("GBR".into());
        let asset = Asset::new(agent_id, process.into(), region_id, capacity, 2015).unwrap();
        assert!(asset.id.is_none());
    }

    #[rstest]
    #[case(Capacity(0.0))]
    #[case(Capacity(-0.01))]
    #[case(Capacity(-1.0))]
    #[case(Capacity(f64::NAN))]
    #[case(Capacity(f64::INFINITY))]
    #[case(Capacity(f64::NEG_INFINITY))]
    fn test_asset_new_invalid_capacity(process: Process, #[case] capacity: Capacity) {
        let agent_id = Some(AgentID("agent1".into()));
        let region_id = RegionID("GBR".into());
        assert_error!(
            Asset::new(agent_id, process.into(), region_id, capacity, 2015),
            "Capacity must be a finite, positive number"
        );
    }

    #[rstest]
    fn test_asset_new_invalid_commission_year(process: Process) {
        let agent_id = Some(AgentID("agent1".into()));
        let region_id = RegionID("GBR".into());
        assert_error!(
            Asset::new(agent_id, process.into(), region_id, Capacity(1.0), 2009),
            "Process process1 does not operate in the year 2009"
        );
    }

    #[rstest]
    fn test_asset_new_invalid_region(process: Process) {
        let agent_id = Some(AgentID("agent1".into()));
        let region_id = RegionID("FRA".into());
        assert_error!(
            Asset::new(agent_id, process.into(), region_id, Capacity(1.0), 2015),
            "Region FRA is not one of the regions in which process process1 operates"
        );
    }

    #[fixture]
    fn asset_pool() -> AssetPool {
        let process_param = Rc::new(ProcessParameter {
            capital_cost: MoneyPerCapacity(5.0),
            fixed_operating_cost: MoneyPerCapacityPerYear(2.0),
            variable_operating_cost: MoneyPerActivity(1.0),
            lifetime: 5,
            discount_rate: Dimensionless(0.9),
            capacity_to_activity: ActivityPerCapacity(1.0),
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
            activity_limits: ProcessActivityLimitsMap::new(),
            flows: ProcessFlowsMap::new(),
            parameters: process_parameter_map,
            regions: IndexSet::from(["GBR".into()]),
        });
        let future = [2020, 2010]
            .map(|year| {
                Asset::new(
                    Some("agent1".into()),
                    Rc::clone(&process),
                    "GBR".into(),
                    Capacity(1.0),
                    year,
                )
                .unwrap()
            })
            .into_iter()
            .collect_vec();

        AssetPool::new(future)
    }

    #[fixture]
    fn process_with_activity_limits() -> Process {
        let process_param = Rc::new(ProcessParameter {
            capital_cost: MoneyPerCapacity(5.0),
            fixed_operating_cost: MoneyPerCapacityPerYear(2.0),
            variable_operating_cost: MoneyPerActivity(1.0),
            lifetime: 5,
            discount_rate: Dimensionless(0.9),
            capacity_to_activity: ActivityPerCapacity(3.0),
        });
        let years = RangeInclusive::new(2010, 2020).collect_vec();
        let process_parameter_map: ProcessParameterMap = years
            .iter()
            .map(|&year| (("GBR".into(), year), process_param.clone()))
            .collect();
        let time_slice = TimeSliceID {
            season: "winter".into(),
            time_of_day: "day".into(),
        };
        let fraction_limits = Dimensionless(1.0)..=Dimensionless(2.0);
        let mut activity_limits = ProcessActivityLimitsMap::new();
        for year in [2010, 2020] {
            activity_limits.insert(
                ("GBR".into(), year, time_slice.clone()),
                fraction_limits.clone(),
            );
        }
        Process {
            id: "process1".into(),
            description: "Description".into(),
            years: vec![2010, 2020],
            activity_limits,
            flows: ProcessFlowsMap::new(),
            parameters: process_parameter_map,
            regions: IndexSet::from(["GBR".into()]),
        }
    }

    #[fixture]
    fn asset_with_activity_limits(process_with_activity_limits: Process) -> Asset {
        Asset::new(
            Some("agent1".into()),
            Rc::new(process_with_activity_limits),
            "GBR".into(),
            Capacity(2.0),
            2010,
        )
        .unwrap()
    }

    #[rstest]
    fn test_asset_get_activity_limits(asset_with_activity_limits: Asset, time_slice: TimeSliceID) {
        assert_eq!(
            asset_with_activity_limits.get_activity_limits(&time_slice),
            Activity(6.0)..=Activity(12.0)
        );
    }

    #[rstest]
    fn test_asset_get_activity_per_capacity_limits(
        asset_with_activity_limits: Asset,
        time_slice: TimeSliceID,
    ) {
        assert_eq!(
            asset_with_activity_limits.get_activity_per_capacity_limits(&time_slice),
            ActivityPerCapacity(3.0)..=ActivityPerCapacity(6.0)
        );
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
        assert_equal(asset_pool.iter_active(), iter::once(&asset_pool.active[0]));
    }

    #[rstest]
    fn test_asset_pool_commission_new2(mut asset_pool: AssetPool) {
        // Commission year has passed
        asset_pool.commission_new(2011);
        assert_equal(asset_pool.iter_active(), iter::once(&asset_pool.active[0]));
    }

    #[rstest]
    fn test_asset_pool_commission_new3(mut asset_pool: AssetPool) {
        // Nothing to commission for this year
        asset_pool.commission_new(2000);
        assert!(asset_pool.iter_active().next().is_none()); // no active assets
    }

    #[rstest]
    fn test_asset_pool_decommission_old(mut asset_pool: AssetPool) {
        asset_pool.commission_new(2020);
        assert_eq!(asset_pool.active.len(), 2);
        asset_pool.decommission_old(2020); // should decommission first asset (lifetime == 5)
        assert_eq!(asset_pool.active.len(), 1);
        assert_eq!(asset_pool.active[0].commission_year, 2020);
        assert_eq!(asset_pool.decommissioned.len(), 1);
        assert_eq!(asset_pool.decommissioned[0].commission_year, 2010);
        assert_eq!(asset_pool.decommissioned[0].decommission_year, Some(2020));
        asset_pool.decommission_old(2022); // nothing to decommission
        assert_eq!(asset_pool.active.len(), 1);
        assert_eq!(asset_pool.active[0].commission_year, 2020);
        assert_eq!(asset_pool.decommissioned.len(), 1);
        assert_eq!(asset_pool.decommissioned[0].commission_year, 2010);
        assert_eq!(asset_pool.decommissioned[0].decommission_year, Some(2020));
        asset_pool.decommission_old(2025); // should decommission second asset
        assert!(asset_pool.active.is_empty());
        assert_eq!(asset_pool.decommissioned.len(), 2);
        assert_eq!(asset_pool.decommissioned[0].commission_year, 2010);
        assert_eq!(asset_pool.decommissioned[0].decommission_year, Some(2020));
        assert_eq!(asset_pool.decommissioned[1].commission_year, 2020);
        assert_eq!(asset_pool.decommissioned[1].decommission_year, Some(2025));
    }

    #[rstest]
    fn test_asset_pool_get(mut asset_pool: AssetPool) {
        asset_pool.commission_new(2020);
        assert_eq!(asset_pool.get(AssetID(0)), Some(&asset_pool.active[0]));
        assert_eq!(asset_pool.get(AssetID(1)), Some(&asset_pool.active[1]));
    }

    #[rstest]
    fn test_asset_pool_extend_empty(mut asset_pool: AssetPool) {
        // Start with commissioned assets
        asset_pool.commission_new(2020);
        let original_count = asset_pool.active.len();

        // Extend with empty iterator
        asset_pool.extend(std::iter::empty());

        assert_eq!(asset_pool.active.len(), original_count);
    }

    #[rstest]
    fn test_asset_pool_extend_existing_assets(mut asset_pool: AssetPool) {
        // Start with some commissioned assets
        asset_pool.commission_new(2020);
        assert_eq!(asset_pool.active.len(), 2);
        let existing_assets = asset_pool.take();

        // Extend with the same assets (should maintain their IDs)
        asset_pool.extend(existing_assets.clone());

        assert_eq!(asset_pool.active.len(), 2);
        assert_eq!(asset_pool.active[0].id, Some(AssetID(0)));
        assert_eq!(asset_pool.active[1].id, Some(AssetID(1)));
    }

    #[rstest]
    fn test_asset_pool_extend_new_assets(mut asset_pool: AssetPool, process: Process) {
        // Start with some commissioned assets
        asset_pool.commission_new(2020);
        let original_count = asset_pool.active.len();

        // Create new non-commissioned assets
        let process_rc = Rc::new(process);
        let new_assets = [
            Asset::new(
                Some("agent2".into()),
                Rc::clone(&process_rc),
                "GBR".into(),
                Capacity(1.5),
                2015,
            )
            .unwrap()
            .into(),
            Asset::new(
                Some("agent3".into()),
                Rc::clone(&process_rc),
                "GBR".into(),
                Capacity(2.5),
                2018,
            )
            .unwrap()
            .into(),
        ];

        asset_pool.extend(new_assets);

        assert_eq!(asset_pool.active.len(), original_count + 2);
        // New assets should get IDs 2 and 3
        assert_eq!(asset_pool.active[original_count].id, Some(AssetID(2)));
        assert_eq!(asset_pool.active[original_count + 1].id, Some(AssetID(3)));
        assert_eq!(
            asset_pool.active[original_count].agent_id,
            Some("agent2".into())
        );
        assert_eq!(
            asset_pool.active[original_count + 1].agent_id,
            Some("agent3".into())
        );
    }

    #[rstest]
    fn test_asset_pool_extend_mixed_assets(mut asset_pool: AssetPool, process: Process) {
        // Start with some commissioned assets
        asset_pool.commission_new(2020);

        // Create a new non-commissioned asset
        let new_asset = Asset::new(
            Some("agent_new".into()),
            process.into(),
            "GBR".into(),
            Capacity(3.0),
            2019,
        )
        .unwrap()
        .into();

        // Extend with just the new asset (not mixing with existing to avoid duplicates)
        asset_pool.extend([new_asset]);

        assert_eq!(asset_pool.active.len(), 3);
        // Check that we have the original assets plus the new one
        assert!(asset_pool.active.iter().any(|a| a.id == Some(AssetID(0))));
        assert!(asset_pool.active.iter().any(|a| a.id == Some(AssetID(1))));
        assert!(asset_pool.active.iter().any(|a| a.id == Some(AssetID(2))));
        // Check that the new asset has the correct agent
        assert!(asset_pool
            .active
            .iter()
            .any(|a| a.agent_id == Some("agent_new".into())));
    }

    #[rstest]
    fn test_asset_pool_extend_maintains_sort_order(mut asset_pool: AssetPool, process: Process) {
        // Start with some commissioned assets
        asset_pool.commission_new(2020);

        // Create new assets that would be out of order if added at the end
        let process_rc = Rc::new(process);
        let new_assets = [
            Asset::new(
                Some("agent_high_id".into()),
                Rc::clone(&process_rc),
                "GBR".into(),
                Capacity(1.0),
                2016,
            )
            .unwrap()
            .into(),
            Asset::new(
                Some("agent_low_id".into()),
                Rc::clone(&process_rc),
                "GBR".into(),
                Capacity(1.0),
                2017,
            )
            .unwrap()
            .into(),
        ];

        asset_pool.extend(new_assets);

        // Check that assets are sorted by ID
        let ids: Vec<u32> = asset_pool.iter_active().map(|a| a.id.unwrap().0).collect();
        assert_equal(ids, 0..4);
    }

    #[rstest]
    fn test_asset_pool_extend_no_duplicates_expected(mut asset_pool: AssetPool) {
        // Start with some commissioned assets
        asset_pool.commission_new(2020);
        let original_count = asset_pool.active.len();

        // The extend method expects unique assets - adding duplicates would violate
        // the debug assertion, so this test verifies the normal case
        asset_pool.extend(std::iter::empty::<AssetRef>());

        assert_eq!(asset_pool.active.len(), original_count);
        // Verify all assets are still unique (this is what the debug_assert checks)
        assert_eq!(
            asset_pool.active.iter().unique().count(),
            asset_pool.active.len()
        );
    }

    #[rstest]
    fn test_asset_pool_extend_increments_next_id(mut asset_pool: AssetPool, process: Process) {
        // Start with some commissioned assets
        asset_pool.commission_new(2020);
        assert_eq!(asset_pool.next_id, 2); // Should be 2 after commissioning 2 assets

        // Create new non-commissioned assets
        let process_rc = Rc::new(process);
        let new_assets = [
            Asset::new(
                Some("agent1".into()),
                Rc::clone(&process_rc),
                "GBR".into(),
                Capacity(1.0),
                2015,
            )
            .unwrap()
            .into(),
            Asset::new(
                Some("agent2".into()),
                Rc::clone(&process_rc),
                "GBR".into(),
                Capacity(1.0),
                2016,
            )
            .unwrap()
            .into(),
        ];

        asset_pool.extend(new_assets);

        // next_id should have incremented for each new asset
        assert_eq!(asset_pool.next_id, 4);
        assert_eq!(asset_pool.active[2].id, Some(AssetID(2)));
        assert_eq!(asset_pool.active[3].id, Some(AssetID(3)));
    }

    #[rstest]
    fn test_asset_pool_decommission_if_not_active(mut asset_pool: AssetPool) {
        // Commission some assets
        asset_pool.commission_new(2020);
        assert_eq!(asset_pool.active.len(), 2);
        assert_eq!(asset_pool.decommissioned.len(), 0);

        // Remove one asset from the active pool (simulating it being removed elsewhere)
        let removed_asset = asset_pool.active.remove(0);
        assert_eq!(asset_pool.active.len(), 1);

        // Try to decommission both the removed asset (not in active) and an active asset
        let assets_to_check = vec![removed_asset.clone(), asset_pool.active[0].clone()];
        asset_pool.decommission_if_not_active(assets_to_check, 2025);

        // Only the removed asset should be decommissioned (since it's not in active pool)
        assert_eq!(asset_pool.active.len(), 1); // Active pool unchanged
        assert_eq!(asset_pool.decommissioned.len(), 1);
        assert_eq!(asset_pool.decommissioned[0].id, removed_asset.id);
        assert_eq!(asset_pool.decommissioned[0].decommission_year, Some(2025));
    }

    #[rstest]
    fn test_asset_pool_decommission_if_not_active_all_active(mut asset_pool: AssetPool) {
        // Commission some assets
        asset_pool.commission_new(2020);
        assert_eq!(asset_pool.active.len(), 2);
        assert_eq!(asset_pool.decommissioned.len(), 0);

        // Try to decommission assets that are all still in the active pool
        let assets_to_check = asset_pool.active.clone();
        asset_pool.decommission_if_not_active(assets_to_check, 2025);

        // Nothing should be decommissioned since all assets are still active
        assert_eq!(asset_pool.active.len(), 2);
        assert_eq!(asset_pool.decommissioned.len(), 0);
    }

    #[rstest]
    fn test_asset_pool_decommission_if_not_active_none_active(mut asset_pool: AssetPool) {
        // Commission some assets
        asset_pool.commission_new(2020);
        let all_assets = asset_pool.active.clone();

        // Clear the active pool (simulating all assets being removed)
        asset_pool.active.clear();

        // Try to decommission the assets that are no longer active
        asset_pool.decommission_if_not_active(all_assets.clone(), 2025);

        // All assets should be decommissioned since none are in active pool
        assert_eq!(asset_pool.active.len(), 0);
        assert_eq!(asset_pool.decommissioned.len(), 2);
        assert_eq!(asset_pool.decommissioned[0].id, all_assets[0].id);
        assert_eq!(asset_pool.decommissioned[0].decommission_year, Some(2025));
        assert_eq!(asset_pool.decommissioned[1].id, all_assets[1].id);
        assert_eq!(asset_pool.decommissioned[1].decommission_year, Some(2025));
    }

    #[rstest]
    #[should_panic(expected = "Cannot decommission asset that has not been commissioned")]
    fn test_asset_pool_decommission_if_not_active_non_commissioned_asset(
        mut asset_pool: AssetPool,
        process: Process,
    ) {
        // Create a non-commissioned asset
        let non_commissioned_asset = Asset::new(
            Some("agent_new".into()),
            process.into(),
            "GBR".into(),
            Capacity(1.0),
            2015,
        )
        .unwrap()
        .into();

        // This should panic because the asset was never commissioned
        asset_pool.decommission_if_not_active(vec![non_commissioned_asset], 2025);
    }
}
