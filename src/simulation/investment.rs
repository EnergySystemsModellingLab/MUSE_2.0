//! Code for performing agent investment.
use crate::agent::{AssetID, AssetPool};
use crate::model::Model;
use crate::time_slice::TimeSliceID;
use log::info;
use std::collections::HashMap;
use std::rc::Rc;

/// Perform agent investment to determine capacity investment of new assets for next milestone year.
///
/// # Arguments
///
/// * `model` - The model
/// * `assets` - The asset pool
pub fn perform_agent_investment<'a, I>(_model: &Model, flows: I, assets: &mut AssetPool)
where
    I: Iterator<Item = (AssetID, &'a Rc<str>, &'a TimeSliceID, f64)>,
{
    info!("Performing agent investment...");

    let mut utilisation = HashMap::new();
    for (asset_id, commodity_id, _time_slice, flow) in flows {
        let asset = assets.get(asset_id);
        let pac1 = asset.process.iter_pacs().next().unwrap();
        if *commodity_id != pac1.commodity.id {
            continue;
        }

        let value = utilisation.entry(asset_id).or_insert(0.0);
        *value += flow.abs();
    }

    for (asset_id, value) in utilisation.iter_mut() {
        let asset = assets.get(*asset_id);
        *value /= asset.maximum_activity();

        info!(
            "Agent {}, process {}: utilisation {}",
            asset.agent_id, asset.process.id, value
        );
    }
}
