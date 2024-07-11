use polars::prelude::*;
use std::fmt;
use std::path::Path;

pub struct ProcessInfo {
    pub processes: DataFrame,
    pub availabilities: DataFrame,
    pub pacs: DataFrame,
}

impl fmt::Display for ProcessInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Processes: {}\nProcess availabilities: {}\nProcess PACs: {}",
            self.processes, self.availabilities, self.pacs
        )
    }
}

fn read_csv(file_path: &Path, schema: Schema) -> PolarsResult<DataFrame> {
    let df = CsvReadOptions::default()
        .with_schema(Some(Arc::new(schema)))
        .try_into_reader_with_file_path(Some(file_path.to_path_buf()))?
        .finish()?;

    Ok(df)
}

pub fn read_processes(
    processes_file_path: &Path,
    process_availabilities_file_path: &Path,
    process_pacs_file_path: &Path,
) -> PolarsResult<ProcessInfo> {
    let processes_schema = Schema::from_iter([
        Field::new("id", DataType::String),
        Field::new("description", DataType::String),
    ]);
    let availabilities_schema = Schema::from_iter([
        Field::new("process_id", DataType::String),
        Field::new("limit_type", DataType::String),
        Field::new("timeslice", DataType::String),
        Field::new("value", DataType::Float64),
    ]);
    let pacs_schema = Schema::from_iter([
        Field::new("process_id", DataType::String),
        Field::new("pac", DataType::String),
    ]);

    Ok(ProcessInfo {
        processes: read_csv(processes_file_path, processes_schema)?,
        availabilities: read_csv(process_availabilities_file_path, availabilities_schema)?,
        pacs: read_csv(process_pacs_file_path, pacs_schema)?,
    })
}
