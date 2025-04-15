//! Assets are instances of a process which are owned and invested in by agents.
use crate::agent::AgentID;
use crate::commodity::Commodity;
use crate::process::Process;
use crate::time_slice::TimeSliceID;
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
    /// The region in which the asset is located
    pub region_id: Rc<str>,
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
        region_id: Rc<str>,
        capacity: f64,
        commission_year: u32,
    ) -> Self {
        Self {
            id: AssetID::INVALID,
            agent_id,
            process,
            region_id,
            capacity,
            commission_year,
        }
    }

    /// The last year in which this asset should be decommissioned
    pub fn decommission_year(&self) -> u32 {
        self.commission_year + self.process.parameter.lifetime
    }

    /// Get the activity limits for this asset in a particular time slice
    pub fn get_activity_limits(&self, time_slice: &TimeSliceID) -> RangeInclusive<f64> {
        let limits = self.process.activity_limits.get(time_slice).unwrap();
        let max_act = self.maximum_activity();

        // Multiply the fractional capacity in self.process by this asset's actual capacity
        (max_act * limits.start())..=(max_act * limits.end())
    }

    /// Maximum activity for this asset in a year
    pub fn maximum_activity(&self) -> f64 {
        self.capacity * self.process.parameter.capacity_to_activity
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
        region_id: &'a Rc<str>,
    ) -> impl Iterator<Item = &'a Asset> {
        self.iter().filter(|asset| asset.region_id == *region_id)
    }

    /// Iterate over only the active assets in a given region that produce or consume a given
    /// commodity
    pub fn iter_for_region_and_commodity<'a>(
        &'a self,
        region_id: &'a Rc<str>,
        commodity: &'a Rc<Commodity>,
    ) -> impl Iterator<Item = &'a Asset> {
        self.iter_for_region(region_id)
            .filter(|asset| asset.process.contains_commodity_flow(commodity))
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
    use crate::commodity::{CommodityCostMap, CommodityType, DemandMap};
    use crate::process::{ActivityLimitsMap, FlowType, Process, ProcessFlow, ProcessParameter};
    use crate::region::RegionSelection;
    use crate::time_slice::TimeSliceLevel;
    use itertools::{assert_equal, Itertools};
    use std::iter;

    #[test]
    fn test_asset_get_activity_limits() {
        let time_slice = TimeSliceID {
            season: "winter".into(),
            time_of_day: "day".into(),
        };
        let process_param = ProcessParameter {
            years: 2010..=2020,
            capital_cost: 5.0,
            fixed_operating_cost: 2.0,
            variable_operating_cost: 1.0,
            lifetime: 5,
            discount_rate: 0.9,
            capacity_to_activity: 3.0,
        };
        let commodity = Rc::new(Commodity {
            id: "commodity1".into(),
            description: "Some description".into(),
            kind: CommodityType::InputCommodity,
            time_slice_level: TimeSliceLevel::Annual,
            costs: CommodityCostMap::new(),
            demand: DemandMap::new(),
        });
        let flow = ProcessFlow {
            process_id: "id1".into(),
            commodity: Rc::clone(&commodity),
            flow: 1.0,
            flow_type: FlowType::Fixed,
            flow_cost: 1.0,
            is_pac: true,
        };
        let fraction_limits = 1.0..=f64::INFINITY;
        let activity_limits = iter::once((time_slice.clone(), fraction_limits)).collect();
        let process = Rc::new(Process {
            id: "process1".into(),
            description: "Description".into(),
            activity_limits,
            flows: vec![flow.clone()],
            parameter: process_param.clone(),
            regions: RegionSelection::All,
        });
        let asset = Asset {
            id: AssetID(0),
            agent_id: "agent1".into(),
            process: Rc::clone(&process),
            region_id: "GBR".into(),
            capacity: 2.0,
            commission_year: 2010,
        };

        assert_eq!(asset.get_activity_limits(&time_slice), 6.0..=f64::INFINITY);
    }

    fn create_asset_pool() -> AssetPool {
        let process_param = ProcessParameter {
            years: 2010..=2020,
            capital_cost: 5.0,
            fixed_operating_cost: 2.0,
            variable_operating_cost: 1.0,
            lifetime: 5,
            discount_rate: 0.9,
            capacity_to_activity: 1.0,
        };
        let process = Rc::new(Process {
            id: "process1".into(),
            description: "Description".into(),
            activity_limits: ActivityLimitsMap::new(),
            flows: vec![],
            parameter: process_param.clone(),
            regions: RegionSelection::All,
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
            })
            .into_iter()
            .collect_vec();

        AssetPool::new(future)
    }

    #[test]
    fn test_asset_pool_new() {
        let assets = create_asset_pool();
        assert!(assets.current_year == 0);

        // Should be in order of commission year
        assert!(assets.assets.len() == 2);
        assert!(assets.assets[0].commission_year == 2010);
        assert!(assets.assets[1].commission_year == 2020);
    }

    #[test]
    fn test_asset_pool_commission_new() {
        // Asset to be commissioned in this year
        let mut assets = create_asset_pool();
        assets.commission_new(2010);
        assert!(assets.current_year == 2010);
        assert_equal(assets.iter(), iter::once(&assets.assets[0]));

        // Commission year has passed
        let mut assets = create_asset_pool();
        assets.commission_new(2011);
        assert!(assets.current_year == 2011);
        assert_equal(assets.iter(), iter::once(&assets.assets[0]));

        // Nothing to commission for this year
        let mut assets = create_asset_pool();
        assets.commission_new(2000);
        assert!(assets.current_year == 2000);
        assert!(assets.iter().next().is_none()); // no active assets
    }

    #[test]
    fn test_asset_pool_decommission_old() {
        let mut assets = create_asset_pool();
        let assets2 = assets.assets.clone();

        assets.commission_new(2020);
        assert!(assets.assets.len() == 2);
        assets.decomission_old(2020); // should decommission first asset (lifetime == 5)
        assert_equal(&assets.assets, iter::once(&assets2[1]));
        assets.decomission_old(2022); // nothing to decommission
        assert_equal(&assets.assets, iter::once(&assets2[1]));
        assets.decomission_old(2025); // should decommission second asset
        assert!(assets.assets.is_empty());
    }

    #[test]
    fn test_asset_pool_get() {
        let mut assets = create_asset_pool();
        assets.commission_new(2020);
        assert!(assets.get(AssetID(0)) == Some(&assets.assets[0]));
        assert!(assets.get(AssetID(1)) == Some(&assets.assets[1]));
    }

    #[test]
    fn test_asset_pool_retain() {
        let mut assets = create_asset_pool();

        // Even though we are retaining no assets, none have been commissioned so the asset pool
        // should not be changed
        assets.retain(&HashSet::new());
        assert_eq!(assets.assets.len(), 2);

        // Decommission all active assets
        assets.commission_new(2010); // Commission first asset
        assets.retain(&HashSet::new());
        assert_eq!(assets.assets.len(), 1);
        assert_eq!(assets.assets[0].id, AssetID(1));

        // Decommission single asset
        let mut assets = create_asset_pool();
        assets.commission_new(2020); // Commission all assets
        assets.retain(&iter::once(AssetID(1)).collect());
        assert_eq!(assets.assets.len(), 1);
        assert_eq!(assets.assets[0].id, AssetID(1));
    }
}
