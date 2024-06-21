mod regions;

use std::path::Path;
use regions::read_regions_from_csv;

fn main() {
    let file_path = Path::new("regions.csv");
    
    match read_regions_from_csv(file_path) {
        Ok(regions) => {
            for region in regions {
                println!("Short Name: {}, Description: {}", region.short_name, region.description);
            }
        }
        Err(err) => {
            eprintln!("Error reading regions from CSV: {}", err);
        }
    }
}
