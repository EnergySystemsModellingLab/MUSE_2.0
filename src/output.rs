//! The module responsible for writing output data to disk.
use crate::agent::AgentID;
use crate::asset::{Asset, AssetID, AssetPool};
use crate::commodity::CommodityID;
use crate::process::ProcessID;
use crate::region::RegionID;
use crate::simulation::CommodityPrices;
use crate::time_slice::TimeSliceID;
use anyhow::{Context, Result};
use csv;
use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf};

/// The root folder in which model-specific output folders will be created
const OUTPUT_DIRECTORY_ROOT: &str = "muse2_results";

/// The output file name for commodity flows
const COMMODITY_FLOWS_FILE_NAME: &str = "commodity_flows.csv";

/// The output file name for commodity prices
const COMMODITY_PRICES_FILE_NAME: &str = "commodity_prices.csv";

/// The output file name for assets
const ASSETS_FILE_NAME: &str = "assets.csv";

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

/// Represents a row in the assets output CSV file
#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct AssetRow {
    milestone_year: u32,
    process_id: ProcessID,
    region_id: RegionID,
    agent_id: AgentID,
    commission_year: u32,
}

impl AssetRow {
    fn new(milestone_year: u32, asset: &Asset) -> Self {
        Self {
            milestone_year,
            process_id: asset.process.id.clone(),
            region_id: asset.region_id.clone(),
            agent_id: asset.agent_id.clone(),
            commission_year: asset.commission_year,
        }
    }
}

/// Represents the flow-related data in a row of the commodity flows CSV file.
///
/// This will be written along with an [`AssetRow`] containing asset-related info.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct CommodityFlowRow {
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

/// An object for writing commodity prices to file
pub struct DataWriter {
    assets_writer: csv::Writer<File>,
    flows_writer: csv::Writer<File>,
    prices_writer: csv::Writer<File>,
}

impl DataWriter {
    /// Create a new CSV files to write output data to
    pub fn create(output_path: &Path) -> Result<Self> {
        let new_writer = |file_name| {
            let file_path = output_path.join(file_name);
            csv::Writer::from_path(file_path)
        };

        Ok(Self {
            assets_writer: new_writer(ASSETS_FILE_NAME)?,
            flows_writer: new_writer(COMMODITY_FLOWS_FILE_NAME)?,
            prices_writer: new_writer(COMMODITY_PRICES_FILE_NAME)?,
        })
    }

    /// Write assets to a CSV file
    pub fn write_assets<'a, I>(&mut self, milestone_year: u32, assets: I) -> Result<()>
    where
        I: Iterator<Item = &'a Asset>,
    {
        for asset in assets {
            let row = AssetRow::new(milestone_year, asset);
            self.assets_writer.serialize(row)?;
        }

        Ok(())
    }

    /// Write commodity flows to a CSV file
    pub fn write_flows<'a, I>(
        &mut self,
        milestone_year: u32,
        assets: &AssetPool,
        flows: I,
    ) -> Result<()>
    where
        I: Iterator<Item = (AssetID, &'a CommodityID, &'a TimeSliceID, f64)>,
    {
        for (asset_id, commodity_id, time_slice, flow) in flows {
            let asset = assets.get(asset_id).unwrap();
            let asset_row = AssetRow::new(milestone_year, asset);
            let flow_row = CommodityFlowRow {
                commodity_id: commodity_id.clone(),
                time_slice: time_slice.clone(),
                flow,
            };
            self.flows_writer.serialize((asset_row, flow_row))?;
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

    /// Flush the underlying streams
    pub fn flush(&mut self) -> Result<()> {
        self.assets_writer.flush()?;
        self.flows_writer.flush()?;
        self.prices_writer.flush()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::process;
    use crate::process::Process;
    use crate::time_slice::TimeSliceID;
    use itertools::{assert_equal, Itertools};
    use rstest::{fixture, rstest};
    use std::iter;
    use tempfile::tempdir;

    #[fixture]
    pub fn asset(process: Process) -> Asset {
        let region_id: RegionID = "GBR".into();
        let agent_id = "agent1".into();
        let commission_year = 2015;
        Asset::new(agent_id, process.into(), region_id, 2.0, commission_year).unwrap()
    }

    #[rstest]
    fn test_write_assets(asset: Asset) {
        let milestone_year = 2020;
        let dir = tempdir().unwrap();

        // Write an asset
        {
            let mut writer = DataWriter::create(dir.path()).unwrap();
            writer
                .write_assets(milestone_year, iter::once(&asset))
                .unwrap();
            writer.flush().unwrap();
        }

        // Read back and compare
        let expected = AssetRow::new(milestone_year, &asset);
        let records: Vec<AssetRow> = csv::Reader::from_path(dir.path().join(ASSETS_FILE_NAME))
            .unwrap()
            .into_deserialize()
            .try_collect()
            .unwrap();
        assert_equal(records, iter::once(expected));
    }

    #[rstest]
    fn test_write_flows(asset: Asset) {
        let milestone_year = 2020;
        let commodity_id = "commodity1".into();
        let time_slice = TimeSliceID {
            season: "winter".into(),
            time_of_day: "day".into(),
        };
        let mut assets = AssetPool::new(vec![asset]);
        assets.commission_new(2020);
        let flow_item = (
            assets.iter().next().unwrap().id,
            &commodity_id,
            &time_slice,
            42.0,
        );

        // Write a flow
        let dir = tempdir().unwrap();
        {
            let mut writer = DataWriter::create(dir.path()).unwrap();
            writer
                .write_flows(milestone_year, &assets, iter::once(flow_item))
                .unwrap();
            writer.flush().unwrap();
        }

        // Read back and compare
        let expected = CommodityFlowRow {
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

    #[test]
    fn test_write_prices() {
        let commodity_id = "commodity1".into();
        let region_id = "GBR".into();
        let time_slice = TimeSliceID {
            season: "winter".into(),
            time_of_day: "day".into(),
        };
        let milestone_year = 2020;
        let price = 42.0;
        let mut prices = CommodityPrices::default();
        prices.insert(&commodity_id, &region_id, &time_slice, price);

        let dir = tempdir().unwrap();

        // Write a price
        {
            let mut writer = DataWriter::create(dir.path()).unwrap();
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
}
