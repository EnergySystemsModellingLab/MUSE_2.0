# List of Validation Checks for Input Files

## IDs

A number of entities in the MUSE 2.0 simulation (specifically: agents, commodities, processes and
regions) are represented with unique IDs. These are defined in different CSV files (`agents.csv`
etc.) in a column called `id`, along with additional information.

Elsewhere in CSV input files, these IDs are referred to, with the column name prefixed by the entity
type. For example, a column containing (single) process IDs will be called `process_id`. When these
CSV files are loaded it will be checked whether the ID is defined in the main CSV file for that
entity type (i.e., `processes.csv` for this example).

## Time slices

Time slices, which represent different parts of the year, are defined in `time_slices.csv`. These
are composed of a season and a time of day: for example, `winter` and `day`. Single time slices are
represented in other CSV as season and time of day separated by a dot, e.g. `winter.day`. All CSV
files which refer to time slices must use this form and both the season and time of day must be
defined in `time_slices.csv`.

In some CSV files (e.g, `demand_slicing.csv`), it is possible to specify a range of time slices
instead of a single time slice. In this case, the possible values are:

- A single time slice in the normal form (e.g. `winter.day`)
- A single season (e.g. `winter`)
- `annual`, representing the whole year
