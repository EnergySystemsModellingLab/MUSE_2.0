//! The module responsible for writing output data to disk.
use crate::agent::AgentID;
use crate::asset::{Asset, AssetID, AssetRef};
use crate::commodity::CommodityID;
use crate::process::ProcessID;
use crate::region::RegionID;
use crate::simulation::optimisation::{FlowMap, Solution};
use crate::simulation::CommodityPrices;
use crate::time_slice::TimeSliceID;
use anyhow::{Context, Result};
use csv;
use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf};

pub mod metadata;
use metadata::write_metadata;

/// The root folder in which model-specific output folders will be created
const OUTPUT_DIRECTORY_ROOT: &str = "muse2_results";

/// The output file name for commodity flows
const COMMODITY_FLOWS_FILE_NAME: &str = "commodity_flows.csv";

/// The output file name for commodity prices
const COMMODITY_PRICES_FILE_NAME: &str = "commodity_prices.csv";

/// The output file name for assets
const ASSETS_FILE_NAME: &str = "assets.csv";

/// The output file name for commodity balance duals
const COMMODITY_BALANCE_DUALS_FILE_NAME: &str = "debug_commodity_balance_duals.csv";

/// The output file name for activity duals
const ACTIVITY_DUALS_FILE_NAME: &str = "debug_activity_duals.csv";

/// Get the model name from the specified directory path
pub fn get_output_dir(model_dir: &Path) -> Result<PathBuf> {
    // Get the model name from the dir path. This ends up being convoluted because we need to check
    // for all possible errors. Ugh.
    let model_dir = model_dir
        .canonicalize() // canonicalise in case the user has specified "."
        .context("Could not resolve path to model")?;

    let model_name = model_dir
        .file_name()
        .context("Model cannot be in root folder")?
        .to_str()
        .context("Invalid chars in model dir name")?;

    // Construct path
    Ok([OUTPUT_DIRECTORY_ROOT, model_name].iter().collect())
}

/// Create a new output directory for the model specified at `model_dir`.
pub fn create_output_directory(output_dir: &Path) -> Result<()> {
    if output_dir.is_dir() {
        // already exists
        return Ok(());
    }

    // Try to create the directory, with parents
    fs::create_dir_all(output_dir)?;

    Ok(())
}

/// Represents a row in the assets output CSV file.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct AssetRow {
    milestone_year: u32,
    asset_id: AssetID,
    process_id: ProcessID,
    region_id: RegionID,
    agent_id: AgentID,
    commission_year: u32,
}

impl AssetRow {
    /// Create a new [`AssetRow`]
    fn new(milestone_year: u32, asset: &Asset) -> Self {
        Self {
            milestone_year,
            asset_id: asset.id.unwrap(),
            process_id: asset.process.id.clone(),
            region_id: asset.region_id.clone(),
            agent_id: asset.agent_id.clone(),
            commission_year: asset.commission_year,
        }
    }
}

/// Represents the flow-related data in a row of the commodity flows CSV file.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct CommodityFlowRow {
    milestone_year: u32,
    asset_id: AssetID,
    commodity_id: CommodityID,
    time_slice: TimeSliceID,
    flow: f64,
}

/// Represents a row in the commodity prices CSV file
#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct CommodityPriceRow {
    milestone_year: u32,
    commodity_id: CommodityID,
    region_id: RegionID,
    time_slice: TimeSliceID,
    price: f64,
}

/// Represents the activity duals data in a row of the activity duals CSV file
#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct ActivityDualsRow {
    milestone_year: u32,
    asset_id: AssetID,
    time_slice: TimeSliceID,
    value: f64,
}

/// Represents the commodity balance duals data in a row of the commodity balance duals CSV file
#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct CommodityBalanceDualsRow {
    milestone_year: u32,
    commodity_id: CommodityID,
    region_id: RegionID,
    time_slice: TimeSliceID,
    value: f64,
}

/// For writing extra debug information about the model
struct DebugDataWriter {
    commodity_balance_duals_writer: csv::Writer<File>,
    activity_duals_writer: csv::Writer<File>,
}

impl DebugDataWriter {
    /// Open CSV files to write debug info to
    ///
    /// # Arguments
    ///
    /// * `output_path` - Folder where files will be saved
    fn create(output_path: &Path) -> Result<Self> {
        let new_writer = |file_name| {
            let file_path = output_path.join(file_name);
            csv::Writer::from_path(file_path)
        };

        Ok(Self {
            commodity_balance_duals_writer: new_writer(COMMODITY_BALANCE_DUALS_FILE_NAME)?,
            activity_duals_writer: new_writer(ACTIVITY_DUALS_FILE_NAME)?,
        })
    }

    /// Write all debug info to output files
    fn write_debug_info(&mut self, milestone_year: u32, solution: &Solution) -> Result<()> {
        self.write_activity_duals(milestone_year, solution.iter_activity_duals())?;
        self.write_commodity_balance_duals(
            milestone_year,
            solution.iter_commodity_balance_duals(),
        )?;
        Ok(())
    }

    /// Write activity duals to file
    fn write_activity_duals<'a, I>(&mut self, milestone_year: u32, iter: I) -> Result<()>
    where
        I: Iterator<Item = (&'a AssetRef, &'a TimeSliceID, f64)>,
    {
        for (asset, time_slice, value) in iter {
            let row = ActivityDualsRow {
                milestone_year,
                asset_id: asset.id.unwrap(),
                time_slice: time_slice.clone(),
                value,
            };
            self.activity_duals_writer.serialize(row)?;
        }

        Ok(())
    }

    /// Write commodity balance duals to file
    fn write_commodity_balance_duals<'a, I>(&mut self, milestone_year: u32, iter: I) -> Result<()>
    where
        I: Iterator<Item = (&'a CommodityID, &'a RegionID, &'a TimeSliceID, f64)>,
    {
        for (commodity_id, region_id, time_slice, value) in iter {
            let row = CommodityBalanceDualsRow {
                milestone_year,
                commodity_id: commodity_id.clone(),
                region_id: region_id.clone(),
                time_slice: time_slice.clone(),
                value,
            };
            self.commodity_balance_duals_writer.serialize(row)?;
        }

        Ok(())
    }

    /// Flush the underlying streams
    fn flush(&mut self) -> Result<()> {
        self.commodity_balance_duals_writer.flush()?;
        self.activity_duals_writer.flush()?;

        Ok(())
    }
}

/// An object for writing commodity prices to file
pub struct DataWriter {
    assets_writer: csv::Writer<File>,
    flows_writer: csv::Writer<File>,
    prices_writer: csv::Writer<File>,
    debug_writer: Option<DebugDataWriter>,
}

impl DataWriter {
    /// Open CSV files to write output data to
    ///
    /// # Arguments
    ///
    /// * `output_path` - Folder where files will be saved
    /// * `model_path` - Path to input model
    /// * `save_debug_info` - Whether to include extra CSV files for debugging model
    pub fn create(output_path: &Path, model_path: &Path, save_debug_info: bool) -> Result<Self> {
        write_metadata(output_path, model_path).context("Failed to save metadata")?;

        let new_writer = |file_name| {
            let file_path = output_path.join(file_name);
            csv::Writer::from_path(file_path)
        };

        let debug_writer = if save_debug_info {
            // Create debug CSV files
            Some(DebugDataWriter::create(output_path)?)
        } else {
            None
        };

        Ok(Self {
            assets_writer: new_writer(ASSETS_FILE_NAME)?,
            flows_writer: new_writer(COMMODITY_FLOWS_FILE_NAME)?,
            prices_writer: new_writer(COMMODITY_PRICES_FILE_NAME)?,
            debug_writer,
        })
    }

    /// Write assets to a CSV file
    pub fn write_assets<'a, I>(&mut self, milestone_year: u32, assets: I) -> Result<()>
    where
        I: Iterator<Item = &'a AssetRef>,
    {
        for asset in assets {
            let row = AssetRow::new(milestone_year, asset);
            self.assets_writer.serialize(row)?;
        }

        Ok(())
    }

    /// Write commodity flows to a CSV file
    pub fn write_flows(&mut self, milestone_year: u32, flow_map: &FlowMap) -> Result<()> {
        for ((asset, commodity_id, time_slice), flow) in flow_map {
            let row = CommodityFlowRow {
                milestone_year,
                asset_id: asset.id.unwrap(),
                commodity_id: commodity_id.clone(),
                time_slice: time_slice.clone(),
                flow: *flow,
            };
            self.flows_writer.serialize(row)?;
        }

        Ok(())
    }

    /// Write commodity prices to a CSV file
    pub fn write_prices(&mut self, milestone_year: u32, prices: &CommodityPrices) -> Result<()> {
        for (commodity_id, region_id, time_slice, price) in prices.iter() {
            let row = CommodityPriceRow {
                milestone_year,
                commodity_id: commodity_id.clone(),
                region_id: region_id.clone(),
                time_slice: time_slice.clone(),
                price,
            };
            self.prices_writer.serialize(row)?;
        }

        Ok(())
    }

    /// Write debug information to CSV files
    pub fn write_debug_info(&mut self, milestone_year: u32, solution: &Solution) -> Result<()> {
        if let Some(ref mut wtr) = &mut self.debug_writer {
            wtr.write_debug_info(milestone_year, solution)?;
        }

        Ok(())
    }

    /// Flush the underlying streams
    pub fn flush(&mut self) -> Result<()> {
        self.assets_writer.flush()?;
        self.flows_writer.flush()?;
        self.prices_writer.flush()?;
        if let Some(ref mut wtr) = &mut self.debug_writer {
            wtr.flush()?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset::AssetPool;
    use crate::fixture::{assets, commodity_id, region_id, time_slice};
    use crate::time_slice::TimeSliceID;
    use indexmap::indexmap;
    use itertools::{assert_equal, Itertools};
    use rstest::rstest;
    use std::iter;
    use tempfile::tempdir;

    #[rstest]
    fn test_write_assets(assets: AssetPool) {
        let milestone_year = 2020;
        let dir = tempdir().unwrap();

        // Write an asset
        {
            let mut writer = DataWriter::create(dir.path(), dir.path(), false).unwrap();
            writer.write_assets(milestone_year, assets.iter()).unwrap();
            writer.flush().unwrap();
        }

        // Read back and compare
        let asset = assets.iter().next().unwrap();
        let expected = AssetRow::new(milestone_year, asset);
        let records: Vec<AssetRow> = csv::Reader::from_path(dir.path().join(ASSETS_FILE_NAME))
            .unwrap()
            .into_deserialize()
            .try_collect()
            .unwrap();
        assert_equal(records, iter::once(expected));
    }

    #[rstest]
    fn test_write_flows(assets: AssetPool, commodity_id: CommodityID, time_slice: TimeSliceID) {
        let milestone_year = 2020;
        let asset = assets.iter().next().unwrap();
        let flow_map = indexmap! {
            (asset.clone(), commodity_id.clone(), time_slice.clone()) => 42.0
        };

        // Write a flow
        let dir = tempdir().unwrap();
        {
            let mut writer = DataWriter::create(dir.path(), dir.path(), false).unwrap();
            writer.write_flows(milestone_year, &flow_map).unwrap();
            writer.flush().unwrap();
        }

        // Read back and compare
        let expected = CommodityFlowRow {
            milestone_year,
            asset_id: asset.id.unwrap(),
            commodity_id,
            time_slice,
            flow: 42.0,
        };
        let records: Vec<CommodityFlowRow> =
            csv::Reader::from_path(dir.path().join(COMMODITY_FLOWS_FILE_NAME))
                .unwrap()
                .into_deserialize()
                .try_collect()
                .unwrap();
        assert_equal(records, iter::once(expected));
    }

    #[rstest]
    fn test_write_prices(commodity_id: CommodityID, region_id: RegionID, time_slice: TimeSliceID) {
        let milestone_year = 2020;
        let price = 42.0;
        let mut prices = CommodityPrices::default();
        prices.insert(&commodity_id, &region_id, &time_slice, price);

        let dir = tempdir().unwrap();

        // Write a price
        {
            let mut writer = DataWriter::create(dir.path(), dir.path(), false).unwrap();
            writer.write_prices(milestone_year, &prices).unwrap();
            writer.flush().unwrap();
        }

        // Read back and compare
        let expected = CommodityPriceRow {
            milestone_year,
            commodity_id,
            region_id,
            time_slice,
            price,
        };
        let records: Vec<CommodityPriceRow> =
            csv::Reader::from_path(dir.path().join(COMMODITY_PRICES_FILE_NAME))
                .unwrap()
                .into_deserialize()
                .try_collect()
                .unwrap();
        assert_equal(records, iter::once(expected));
    }

    #[rstest]
    fn test_write_commodity_balance_duals(
        commodity_id: CommodityID,
        region_id: RegionID,
        time_slice: TimeSliceID,
    ) {
        let milestone_year = 2020;
        let value = 0.5;
        let dir = tempdir().unwrap();

        // Write commodity balance dual
        {
            let mut writer = DebugDataWriter::create(dir.path()).unwrap();
            writer
                .write_commodity_balance_duals(
                    milestone_year,
                    iter::once((&commodity_id, &region_id, &time_slice, value)),
                )
                .unwrap();
            writer.flush().unwrap();
        }

        // Read back and compare
        let expected = CommodityBalanceDualsRow {
            milestone_year,
            commodity_id,
            region_id,
            time_slice,
            value,
        };
        let records: Vec<CommodityBalanceDualsRow> =
            csv::Reader::from_path(dir.path().join(COMMODITY_BALANCE_DUALS_FILE_NAME))
                .unwrap()
                .into_deserialize()
                .try_collect()
                .unwrap();
        assert_equal(records, iter::once(expected));
    }

    #[rstest]
    fn test_write_activity_duals(assets: AssetPool, time_slice: TimeSliceID) {
        let milestone_year = 2020;
        let value = 0.5;
        let dir = tempdir().unwrap();
        let asset = assets.iter().next().unwrap();

        // Write activity dual
        {
            let mut writer = DebugDataWriter::create(dir.path()).unwrap();
            writer
                .write_activity_duals(milestone_year, iter::once((asset, &time_slice, value)))
                .unwrap();
            writer.flush().unwrap();
        }

        // Read back and compare
        let expected = ActivityDualsRow {
            milestone_year,
            asset_id: asset.id.unwrap(),
            time_slice,
            value,
        };
        let records: Vec<ActivityDualsRow> =
            csv::Reader::from_path(dir.path().join(ACTIVITY_DUALS_FILE_NAME))
                .unwrap()
                .into_deserialize()
                .try_collect()
                .unwrap();
        assert_equal(records, iter::once(expected));
    }
}
