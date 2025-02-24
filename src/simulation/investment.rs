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
pub fn perform_agent_investment<'a, I>(model: &Model, flows: I, assets: &mut AssetPool)
where
    I: Iterator<Item = (AssetID, &'a Rc<str>, &'a TimeSliceID, f64)>,
{
    info!("Performing agent investment...");

    let mut utilisation = HashMap::new();
    for (asset_id, commodity_id, time_slice, flow) in flows {
        let asset = assets.get(asset_id);
        if asset.process.get_commodity_flow(commodity_id).is_pac {
            let key = (asset_id, time_slice.clone());
            let ts_fraction = model.time_slice_info.fractions.get(time_slice).unwrap();
            let value = flow.abs() / (asset.maximum_activity() * ts_fraction);
            let exists = utilisation.insert(key, value).is_some();
            assert!(!exists, "Multiple PACs found for asset");

            info!(
                "Agent {}, process {}, {}: utilisation {}",
                asset.agent_id, asset.process.id, time_slice, value
            );
        }
    }
}
