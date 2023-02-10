use std::path::Path;
use anyhow::{anyhow, Result};
use crate::config::Config;

pub fn do_clone(conf: Config, cred_path: &Path) {
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
