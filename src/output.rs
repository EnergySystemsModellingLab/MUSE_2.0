//! The module responsible for writing output data to disk.
use crate::agent::{Asset, AssetID, AssetPool};
use crate::simulation::CommodityPrices;
use crate::time_slice::TimeSliceID;
use anyhow::{Context, Result};
use csv;
use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// The root folder in which model-specific output folders will be created
const OUTPUT_DIRECTORY_ROOT: &str = "muse2_results";

/// The output file name for commodity flows
const COMMODITY_FLOWS_FILE_NAME: &str = "commodity_flows.csv";

/// The output file name for commodity prices
const COMMODITY_PRICES_FILE_NAME: &str = "commodity_prices.csv";

/// The output file name for assets
const ASSETS_FILE_NAME: &str = "assets.csv";

/// Create a new output directory for the model specified at `model_dir`.
pub fn create_output_directory(model_dir: &Path) -> Result<PathBuf> {
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
    let path: PathBuf = [OUTPUT_DIRECTORY_ROOT, model_name].iter().collect();
    if path.is_dir() {
        // already exists
        return Ok(path);
    }

    // Try to create the directory, with parents
    fs::create_dir_all(&path)?;

    Ok(path)
}

/// Represents the flow-related data in a row of the commodity flows CSV file.
///
/// This will be written along with an [`AssetRow`] containing asset-related info.
#[derive(Serialize)]
struct CommodityFlowRow {
    commodity_id: Rc<str>,
    time_slice: String,
    flow: f64,
    dual: f64,
}

/// An object for writing commodity flows to file
pub struct CommodityFlowWriter(csv::Writer<File>);

impl CommodityFlowWriter {
    /// Create a new CSV file to write commodity flows to
    pub fn create(output_path: &Path) -> Result<Self> {
        let file_path = output_path.join(COMMODITY_FLOWS_FILE_NAME);
        Ok(Self(csv::Writer::from_path(file_path)?))
    }

    /// Write commodity flows to a CSV file
    pub fn write<'a, I>(&mut self, milestone_year: u32, assets: &AssetPool, flows: I) -> Result<()>
    where
        I: Iterator<Item = (AssetID, &'a Rc<str>, &'a TimeSliceID, f64, f64)>,
    {
        for (asset_id, commodity_id, time_slice, flow, dual) in flows {
            let asset = assets.get(asset_id);
            let asset_row = AssetRow::new(milestone_year, asset);
            let flow_row = CommodityFlowRow {
                commodity_id: Rc::clone(commodity_id),
                time_slice: time_slice.to_string(),
                flow,
                dual,
            };
            self.0.serialize((asset_row, flow_row))?;
        }

        Ok(())
    }
}

/// Represents a row in the commodity prices CSV file
#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct CommodityPriceRow {
    milestone_year: u32,
    commodity_id: Rc<str>,
    time_slice: String,
    price: f64,
}

/// An object for writing commodity prices to file
pub struct CommodityPricesWriter(csv::Writer<File>);

impl CommodityPricesWriter {
    /// Create a new CSV file to write commodity prices to
    pub fn create(output_path: &Path) -> Result<Self> {
        let file_path = output_path.join(COMMODITY_PRICES_FILE_NAME);
        Ok(Self(csv::Writer::from_path(file_path)?))
    }

    /// Write commodity prices to a CSV file
    pub fn write(&mut self, milestone_year: u32, prices: &CommodityPrices) -> Result<()> {
        for (commodity_id, time_slice, price) in prices.iter() {
            let row = CommodityPriceRow {
                milestone_year,
                commodity_id: Rc::clone(commodity_id),
                time_slice: time_slice.to_string(),
                price,
            };
            self.0.serialize(row)?;
        }

        Ok(())
    }
}

/// Represents a row in the assets output CSV file
#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct AssetRow {
    milestone_year: u32,
    process_id: Rc<str>,
    region_id: Rc<str>,
    agent_id: Rc<str>,
    commission_year: u32,
}

impl AssetRow {
    fn new(milestone_year: u32, asset: &Asset) -> Self {
        Self {
            milestone_year,
            process_id: Rc::clone(&asset.process.id),
            region_id: Rc::clone(&asset.region_id),
            agent_id: Rc::clone(&asset.agent_id),
            commission_year: asset.commission_year,
        }
    }
}

/// An object for writing assets to file
pub struct AssetsWriter(csv::Writer<File>);

impl AssetsWriter {
    /// Create a new CSV file to write assets to
    pub fn create(output_path: &Path) -> Result<Self> {
        let file_path = output_path.join(ASSETS_FILE_NAME);
        Ok(Self(csv::Writer::from_path(file_path)?))
    }

    /// Write assets to a CSV file
    pub fn write<'a, I>(&mut self, milestone_year: u32, assets: I) -> Result<()>
    where
        I: Iterator<Item = &'a Asset>,
    {
        for asset in assets {
            let row = AssetRow::new(milestone_year, asset);
            self.0.serialize(row)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, iter};

    use super::*;
    use crate::{
        process::{Process, ProcessParameter},
        region::RegionSelection,
        time_slice::TimeSliceID,
    };
    use itertools::{assert_equal, Itertools};
    use tempfile::tempdir;

    #[test]
    fn test_commodity_prices_writer() {
        let commodity_id = "commodity1".into();
        let time_slice = TimeSliceID {
            season: "winter".into(),
            time_of_day: "day".into(),
        };
        let milestone_year = 2020;
        let price = 42.0;
        let mut prices = CommodityPrices::default();
        prices.insert(&commodity_id, &time_slice, price);

        let dir = tempdir().unwrap();

        // Write a price
        {
            let mut prices_wtr = CommodityPricesWriter::create(dir.path()).unwrap();
            prices_wtr.write(milestone_year, &prices).unwrap();
        }

        // Read back and compare
        let expected = CommodityPriceRow {
            commodity_id,
            milestone_year,
            time_slice: time_slice.to_string(),
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

    #[test]
    fn test_assets_writer() {
        let milestone_year = 2020;
        let process_id = "process1".into();
        let region_id = "GBR".into();
        let agent_id = "agent1".into();
        let commission_year = 2015;
        let process_param = ProcessParameter {
            process_id: "process1".to_string(),
            years: 2010..=2020,
            capital_cost: 5.0,
            fixed_operating_cost: 2.0,
            variable_operating_cost: 1.0,
            lifetime: 5,
            discount_rate: 0.9,
            cap2act: 3.0,
        };
        let process = Rc::new(Process {
            id: Rc::clone(&process_id),
            description: "Description".into(),
            capacity_fractions: HashMap::new(),
            flows: vec![],
            parameter: process_param.clone(),
            regions: RegionSelection::All,
        });
        let asset = Asset::new(agent_id, process, region_id, 2.0, commission_year);

        let dir = tempdir().unwrap();

        // Write an asset
        {
            let mut assets_wtr = AssetsWriter::create(dir.path()).unwrap();
            assets_wtr
                .write(milestone_year, iter::once(&asset))
                .unwrap();
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
}
