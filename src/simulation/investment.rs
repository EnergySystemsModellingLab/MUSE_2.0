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

    let utilisation = create_utilisation_map(flows, assets);
    for (agent_id, vec) in utilisation {
        for (asset_id, value) in vec {
            let asset = assets.get(asset_id);
            info!(
                "Agent {}, process {}: utilisation {}",
                agent_id, asset.process.id, value
            );
        }
    }
}

fn create_utilisation_map<'a, I>(
    flows: I,
    assets: &mut AssetPool,
) -> HashMap<Rc<str>, Vec<(AssetID, f64)>>
where
    I: Iterator<Item = (AssetID, &'a Rc<str>, &'a TimeSliceID, f64)>,
{
    let mut utilisation = HashMap::new();
    for (asset_id, commodity_id, _time_slice, flow) in flows {
        let asset = assets.get(asset_id);
        let pac1 = asset.process.iter_pacs().next().unwrap();
        if *commodity_id != pac1.commodity.id {
            continue;
        }

        let vec = utilisation
            .entry(Rc::clone(&asset.agent_id))
            .or_insert_with(Vec::new);
        let value = match vec
            .iter_mut()
            .find(|(asset_id2, _value)| asset_id == *asset_id2)
        {
            Some(value) => &mut value.1,
            None => {
                vec.push((asset_id, 0.0));
                &mut vec.last_mut().unwrap().1
            }
        };

        *value += flow.abs();
    }

    for vec in utilisation.values_mut() {
        for (asset_id, value) in vec.iter_mut() {
            let asset = assets.get(*asset_id);
            *value /= asset.maximum_activity();
        }
    }

    utilisation
}
