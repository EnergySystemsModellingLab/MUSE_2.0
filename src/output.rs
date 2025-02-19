//! The module responsible for writing output data to disk.
use crate::simulation::CommodityPrices;
use anyhow::{Context, Result};
use csv::Writer;
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};

/// The root folder in which model-specific output folders will be created
const OUTPUT_DIRECTORY_ROOT: &str = "muse2_results";

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

#[derive(Serialize)]
struct CommodityPriceRow {
    milestone_year: u32,
    commodity_id: String,
    time_slice: String,
    price: f64,
}

/// Write commodity prices to a CSV file.
pub fn write_commodity_prices_to_csv(
    output_path: &Path,
    milestone_year: u32,
    prices: &CommodityPrices,
) -> Result<()> {
    let file_path = output_path.join("commodity_prices.csv");
    let file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(file_path)?;
    let mut wtr = Writer::from_writer(file);

    for (commodity_id, time_slice, price) in prices.iter() {
        let row = CommodityPriceRow {
            milestone_year,
            commodity_id: commodity_id.to_string(),
            time_slice: time_slice.to_string(),
            price,
        };
        wtr.serialize(row)?;
    }

    wtr.flush()?;
    Ok(())
}
