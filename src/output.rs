//! The module responsible for writing output data to disk.
use crate::agent::AgentID;
use crate::asset::{Asset, AssetID, AssetPool, AssetRef};
use crate::commodity::CommodityID;
use crate::process::ProcessID;
use crate::region::RegionID;
use crate::simulation::optimisation::{FlowMap, Solution};
use crate::simulation::CommodityPrices;
use crate::time_slice::TimeSliceID;
use crate::units::{Activity, Capacity, Flow, Money, MoneyPerActivity, MoneyPerFlow};
use anyhow::{Context, Result};
use csv;
use itertools::Itertools;
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

/// The output file name for raw activity
const ACTIVITY_FILE_NAME: &str = "debug_activity.csv";

/// The output file name for commodity balance duals
const COMMODITY_BALANCE_DUALS_FILE_NAME: &str = "debug_commodity_balance_duals.csv";

/// The output file name for activity duals
const ACTIVITY_DUALS_FILE_NAME: &str = "debug_activity_duals.csv";

/// The output file name for extra solver output values
const SOLVER_VALUES_FILE_NAME: &str = "debug_solver.csv";

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
    asset_id: AssetID,
    process_id: ProcessID,
    region_id: RegionID,
    agent_id: AgentID,
    commission_year: u32,
    decommission_year: Option<u32>,
    capacity: Capacity,
}

impl AssetRow {
    /// Create a new [`AssetRow`]
    fn new(asset: &Asset) -> Self {
        Self {
            asset_id: asset.id.unwrap(),
            process_id: asset.process.id.clone(),
            region_id: asset.region_id.clone(),
            agent_id: asset.agent_id.clone().unwrap(),
            commission_year: asset.commission_year,
            decommission_year: asset.decommission_year,
            capacity: asset.capacity,
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
    flow: Flow,
}

/// Represents a row in the commodity prices CSV file
#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct CommodityPriceRow {
    milestone_year: u32,
    commodity_id: CommodityID,
    region_id: RegionID,
    time_slice: TimeSliceID,
    price: MoneyPerFlow,
}

/// Represents the activity in a row of the activity CSV file
#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct ActivityRow {
    milestone_year: u32,
    run_number: u32,
    asset_id: Option<AssetID>,
    time_slice: TimeSliceID,
    activity: Activity,
}

/// Represents the activity duals data in a row of the activity duals CSV file
#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct ActivityDualsRow {
    milestone_year: u32,
    run_number: u32,
    asset_id: Option<AssetID>,
    time_slice: TimeSliceID,
    value: MoneyPerActivity,
}

/// Represents the commodity balance duals data in a row of the commodity balance duals CSV file
#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct CommodityBalanceDualsRow {
    milestone_year: u32,
    run_number: u32,
    commodity_id: CommodityID,
    region_id: RegionID,
    time_slice: TimeSliceID,
    value: MoneyPerFlow,
}

/// Represents solver output values
#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct SolverValuesRow {
    milestone_year: u32,
    run_number: u32,
    objective_value: Money,
}

/// For writing extra debug information about the model
struct DebugDataWriter {
    activity_writer: csv::Writer<File>,
    commodity_balance_duals_writer: csv::Writer<File>,
    activity_duals_writer: csv::Writer<File>,
    solver_values_writer: csv::Writer<File>,
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
            activity_writer: new_writer(ACTIVITY_FILE_NAME)?,
            commodity_balance_duals_writer: new_writer(COMMODITY_BALANCE_DUALS_FILE_NAME)?,
            activity_duals_writer: new_writer(ACTIVITY_DUALS_FILE_NAME)?,
            solver_values_writer: new_writer(SOLVER_VALUES_FILE_NAME)?,
        })
    }

    /// Write debug info about the dispatch optimisation
    fn write_debug_info(
        &mut self,
        milestone_year: u32,
        run_number: u32,
        solution: &Solution,
    ) -> Result<()> {
        self.write_activity(milestone_year, run_number, solution.iter_activity())?;
        self.write_activity_duals(milestone_year, run_number, solution.iter_activity_duals())?;
        self.write_commodity_balance_duals(
            milestone_year,
            run_number,
            solution.iter_commodity_balance_duals(),
        )?;
        self.write_solver_values(milestone_year, run_number, solution.objective_value)?;
        Ok(())
    }

    // Write activity to file
    fn write_activity<'a, I>(&mut self, milestone_year: u32, run_number: u32, iter: I) -> Result<()>
    where
        I: Iterator<Item = (&'a AssetRef, &'a TimeSliceID, Activity)>,
    {
        for (asset, time_slice, activity) in iter {
            let row = ActivityRow {
                milestone_year,
                run_number,
                asset_id: asset.id,
                time_slice: time_slice.clone(),
                activity,
            };
            self.activity_writer.serialize(row)?;
        }

        Ok(())
    }

    /// Write activity duals to file
    fn write_activity_duals<'a, I>(
        &mut self,
        milestone_year: u32,
        run_number: u32,
        iter: I,
    ) -> Result<()>
    where
        I: Iterator<Item = (&'a AssetRef, &'a TimeSliceID, MoneyPerActivity)>,
    {
        for (asset, time_slice, value) in iter {
            let row = ActivityDualsRow {
                milestone_year,
                run_number,
                asset_id: asset.id,
                time_slice: time_slice.clone(),
                value,
            };
            self.activity_duals_writer.serialize(row)?;
        }

        Ok(())
    }

    /// Write commodity balance duals to file
    fn write_commodity_balance_duals<'a, I>(
        &mut self,
        milestone_year: u32,
        run_number: u32,
        iter: I,
    ) -> Result<()>
    where
        I: Iterator<Item = (&'a CommodityID, &'a RegionID, &'a TimeSliceID, MoneyPerFlow)>,
    {
        for (commodity_id, region_id, time_slice, value) in iter {
            let row = CommodityBalanceDualsRow {
                milestone_year,
                run_number,
                commodity_id: commodity_id.clone(),
                region_id: region_id.clone(),
                time_slice: time_slice.clone(),
                value,
            };
            self.commodity_balance_duals_writer.serialize(row)?;
        }

        Ok(())
    }

    /// Write additional solver output values to file
    fn write_solver_values(
        &mut self,
        milestone_year: u32,
        run_number: u32,
        objective_value: Money,
    ) -> Result<()> {
        let row = SolverValuesRow {
            milestone_year,
            run_number,
            objective_value,
        };
        self.solver_values_writer.serialize(row)?;
        self.solver_values_writer.flush()?;

        Ok(())
    }

    /// Flush the underlying streams
    fn flush(&mut self) -> Result<()> {
        self.activity_writer.flush()?;
        self.commodity_balance_duals_writer.flush()?;

        Ok(())
    }
}

/// An object for writing commodity prices to file
pub struct DataWriter {
    assets_path: PathBuf,
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
            assets_path: output_path.join(ASSETS_FILE_NAME),
            flows_writer: new_writer(COMMODITY_FLOWS_FILE_NAME)?,
            prices_writer: new_writer(COMMODITY_PRICES_FILE_NAME)?,
            debug_writer,
        })
    }

    /// Write information to various output CSV files
    pub fn write(
        &mut self,
        milestone_year: u32,
        assets: &AssetPool,
        flow_map: &FlowMap,
        prices: &CommodityPrices,
    ) -> Result<()> {
        self.write_assets(assets.iter_all())?;
        self.write_flows(milestone_year, flow_map)?;
        self.write_prices(milestone_year, prices)?;

        Ok(())
    }

    /// Write debug info about the dispatch optimisation
    pub fn write_debug_info(
        &mut self,
        milestone_year: u32,
        run_number: u32,
        solution: &Solution,
    ) -> Result<()> {
        if let Some(ref mut wtr) = &mut self.debug_writer {
            wtr.write_debug_info(milestone_year, run_number, solution)?;
        }

        Ok(())
    }

    /// Write assets to a CSV file.
    ///
    /// The whole file is written at once and is overwritten with subsequent invocations. This is
    /// done so that partial results will be written in the case of errors and so that the user can
    /// see the results while the simulation is still running.
    ///
    /// The file is sorted by asset ID.
    ///
    /// # Panics
    ///
    /// Panics if any of the assets has not yet been commissioned (decommissioned assets are fine).
    fn write_assets<'a, I>(&mut self, assets: I) -> Result<()>
    where
        I: Iterator<Item = &'a AssetRef>,
    {
        let mut writer = csv::Writer::from_path(&self.assets_path)?;
        for asset in assets.sorted() {
            let row = AssetRow::new(asset);
            writer.serialize(row)?;
        }
        writer.flush()?;

        Ok(())
    }

    /// Write commodity flows to a CSV file
    fn write_flows(&mut self, milestone_year: u32, flow_map: &FlowMap) -> Result<()> {
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
    fn write_prices(&mut self, milestone_year: u32, prices: &CommodityPrices) -> Result<()> {
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
        let dir = tempdir().unwrap();

        // Write an asset
        {
            let mut writer = DataWriter::create(dir.path(), dir.path(), false).unwrap();
            writer.write_assets(assets.iter()).unwrap();
            writer.flush().unwrap();
        }

        // Read back and compare
        let asset = assets.iter().next().unwrap();
        let expected = AssetRow::new(asset);
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
            (asset.clone(), commodity_id.clone(), time_slice.clone()) => Flow(42.0)
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
            flow: Flow(42.0),
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
        let price = MoneyPerFlow(42.0);
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
        let run_number = 42;
        let value = MoneyPerFlow(0.5);
        let dir = tempdir().unwrap();

        // Write commodity balance dual
        {
            let mut writer = DebugDataWriter::create(dir.path()).unwrap();
            writer
                .write_commodity_balance_duals(
                    milestone_year,
                    run_number,
                    iter::once((&commodity_id, &region_id, &time_slice, value)),
                )
                .unwrap();
            writer.flush().unwrap();
        }

        // Read back and compare
        let expected = CommodityBalanceDualsRow {
            milestone_year,
            run_number,
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
        let run_number = 42;
        let value = MoneyPerActivity(0.5);
        let dir = tempdir().unwrap();
        let asset = assets.iter().next().unwrap();

        // Write activity dual
        {
            let mut writer = DebugDataWriter::create(dir.path()).unwrap();
            writer
                .write_activity_duals(
                    milestone_year,
                    run_number,
                    iter::once((asset, &time_slice, value)),
                )
                .unwrap();
            writer.flush().unwrap();
        }

        // Read back and compare
        let expected = ActivityDualsRow {
            milestone_year,
            run_number,
            asset_id: asset.id,
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

    #[rstest]
    fn test_write_activity(assets: AssetPool, time_slice: TimeSliceID) {
        let milestone_year = 2020;
        let run_number = 42;
        let activity = Activity(100.5);
        let dir = tempdir().unwrap();
        let asset = assets.iter().next().unwrap();

        // Write activity
        {
            let mut writer = DebugDataWriter::create(dir.path()).unwrap();
            writer
                .write_activity(
                    milestone_year,
                    run_number,
                    iter::once((asset, &time_slice, activity)),
                )
                .unwrap();
            writer.flush().unwrap();
        }

        // Read back and compare
        let expected = ActivityRow {
            milestone_year,
            run_number,
            asset_id: asset.id,
            time_slice,
            activity,
        };
        let records: Vec<ActivityRow> = csv::Reader::from_path(dir.path().join(ACTIVITY_FILE_NAME))
            .unwrap()
            .into_deserialize()
            .try_collect()
            .unwrap();
        assert_equal(records, iter::once(expected));
    }

    #[rstest]
    fn test_write_solver_values() {
        let milestone_year = 2020;
        let run_number = 42;
        let objective_value = Money(1234.56);
        let dir = tempdir().unwrap();

        // Write solver values
        {
            let mut writer = DebugDataWriter::create(dir.path()).unwrap();
            writer
                .write_solver_values(milestone_year, run_number, objective_value)
                .unwrap();
            writer.flush().unwrap();
        }

        // Read back and compare
        let expected = SolverValuesRow {
            milestone_year,
            run_number,
            objective_value,
        };
        let records: Vec<SolverValuesRow> =
            csv::Reader::from_path(dir.path().join(SOLVER_VALUES_FILE_NAME))
                .unwrap()
                .into_deserialize()
                .try_collect()
                .unwrap();
        assert_equal(records, iter::once(expected));
    }
}
