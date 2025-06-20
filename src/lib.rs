//! Common functionality for MUSE 2.0.
#![warn(missing_docs)]
pub mod agent;
pub mod asset;
pub mod commands;
pub mod commodity;
pub mod id;
pub mod input;
pub mod log;
pub mod metrics;
pub mod model;
pub mod output;
pub mod process;
pub mod region;
pub mod settings;
pub mod simulation;
pub mod time_slice;
pub mod units;
pub mod year;

#[cfg(test)]
mod fixture;
