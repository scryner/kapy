pub mod config;
mod codec;

use config::Config;

pub fn clone(conf: Config) -> Result<CloneStatistics, String> {
    let mut statistics: CloneStatistics;

    // traverse 'FROM' directory
    todo!();
}

pub struct CloneStatistics {
    resized: usize,
    converted_to_jpeg: usize,
    converted_to_heic: usize,
    gps_added: usize,
    total_processed_files: usize,
    total_not_processed_files: usize,
}
