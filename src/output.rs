//! The module responsible for writing output data to disk.
use crate::simulation::CommodityPrices;
use anyhow::{Context, Result};
use csv;
use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// The root folder in which model-specific output folders will be created
const OUTPUT_DIRECTORY_ROOT: &str = "muse2_results";

/// The output file name for commodity prices
const COMMODITY_PRICES_FILE_NAME: &str = "commodity_prices.csv";

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

    /// Flush the underlying stream
    pub fn flush(&mut self) -> Result<()> {
        Ok(self.0.flush()?)
    }
}

#[cfg(test)]
mod tests {
    use std::iter;

    use super::*;
    use crate::time_slice::TimeSliceID;
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
            prices_wtr.flush().unwrap();
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
}
