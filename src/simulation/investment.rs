//! Code for performing agent investment.
use crate::agent::AssetPool;
use crate::model::Model;
use log::info;

/// Perform agent investment to determine capacity investment of new assets for next milestone year.
///
/// # Arguments
///
/// * `model` - The model
/// * `assets` - The asset pool
pub fn perform_agent_investment(_model: &Model, _assets: &mut AssetPool) {
    info!("Performing agent investment...");
}
