use crate::input::read_csv_as_vec;
use crate::time_slice::TimeSliceLevel;
use serde::Deserialize;
use serde_string_enum::DeserializeLabeledStringEnum;
use std::path::Path;

const COMMODITY_FILE_NAME: &str = "commodities.csv";

#[derive(PartialEq, Debug, Deserialize)]
pub struct Commodity {
    pub id: String,
    pub description: String,
    #[serde(rename = "type")] // NB: we can't name a field type as it's a reserved keyword
    pub commodity_type: CommodityType,
    pub time_slice_level: TimeSliceLevel,
}

/// Commodity balance type
#[derive(PartialEq, Debug, DeserializeLabeledStringEnum)]
pub enum CommodityType {
    #[string = "sed"]
    SupplyEqualsDemand,
    #[string = "svd"]
    ServiceDemand,
    #[string = "inc"]
    InputCommodity,
    #[string = "ouc"]
    OutputCommodity,
}

/// Read commodity data from the specified model directory.
pub fn read_commodities(model_dir: &Path) -> Vec<Commodity> {
    read_csv_as_vec(&model_dir.join(COMMODITY_FILE_NAME))
}
