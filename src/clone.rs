use std::path::Path;
use std::process;
use std::time::SystemTime;
use anyhow::{anyhow, Result};
use core::time::Duration;
use walkdir::{DirEntry, WalkDir};

use crate::processor::gps::{GpsSearch, GpxStorage, NoopGpsSearch};
use crate::drive::GoogleDrive;
use crate::drive::auth::{CredPath, GoogleAuthenticator, ListenPort};
use crate::config::Config;
use crate::processor;
use crate::processor::CloneStatistics;

const MAX_DEPTH: usize = 10;
const DEFAULT_MAX_SEARCH_FILES_ON_GOOGLE_DRIVE: usize = 100;
const DEFAULT_GPS_MATCH_WITHIN: Duration = Duration::from_secs(5 * 60); // match within 5 min

pub fn do_clone(conf: Config, cred_path: &Path, ignore_geotag: bool) {
    // calculate first date and end date among import files
    let import_entries = match import_entries(conf.import_from()) {
        Ok(ret) => ret,
        Err(e) => {
            eprintln!("Failed to get import entries: {}", e);
            process::exit(1);
        }
    };

    let (oldest_created_at, most_recent_created_at) = match oldest_and_most_recent_created(&import_entries) {
        Ok((oldest, most_recent)) => (oldest, most_recent),
        Err(e) => {
            eprintln!("Failed to find oldest and most recent files: {}", e);
            process::exit(1);
        }
    };

    let gps_search: Box<dyn GpsSearch> = if ignore_geotag {
        Box::new(NoopGpsSearch)
    } else {
        // initialize google drive
        let auth = GoogleAuthenticator::new(ListenPort::DefaultPort, CredPath::Path(cred_path));
        let drive = GoogleDrive::new(auth);

        match GpxStorage::from_google_drive(&drive, oldest_created_at, most_recent_created_at,
                                DEFAULT_MAX_SEARCH_FILES_ON_GOOGLE_DRIVE, DEFAULT_GPS_MATCH_WITHIN) {
            Ok(search) => Box::new(search),
            Err(e) => {
                eprintln!("Failed to initialize geotag search on your google drive: {}", e);
                process::exit(1);
            }
        }
    };

    // process clone
    let mut clone_statistics = CloneStatistics::new();
    let mut error_entries = Vec::new();

    for entry in import_entries.iter() {
        match processor::clone_image(&conf, entry.path(), conf.import_to()) {
            Ok(stat) => {
                clone_statistics = clone_statistics + stat;
            }
            Err(e) => {
                error_entries.push((entry, e));
            }
        }
    }

    // print-out clone statistics
    todo!();
}



fn import_entries(dir: &Path) -> Result<Vec<DirEntry>> {
    let mut import_entries: Vec<DirEntry> = Vec::new();

    // get all metadata from in directory
    for entry in WalkDir::new(dir)
        .max_depth(MAX_DEPTH)
        .into_iter()
        .filter_map(|entry| {
            if let Ok(entry) = entry {
                let path = entry.path();

                return if path.is_file() && path.extension().map_or(false, |ext| {
                    if let Some(ext) = ext.to_str() {
                        match ext.to_lowercase().as_str() {
                            "jpeg" | "jpg" | "heic" => true,
                            _ => false,
                        }
                    } else {
                        false
                    }
                }) {
                    Some(entry)
                } else {
                    None
                }
            } else {
                None
            }
        }) {
        import_entries.push(entry);
    };

    Ok(import_entries)
}

fn oldest_and_most_recent_created(entries: &Vec<DirEntry>) -> Result<(SystemTime, SystemTime)> {
    let created_at_list = entries.iter()
        .map(|entry| entry.metadata().unwrap().created().unwrap() )
        .collect::<Vec<SystemTime>>();

    let oldest = created_at_list.iter().min();
    let most_recent = created_at_list.iter().max();

    if oldest == None || most_recent == None {
        Err(anyhow!("Failed to find oldest and most recent file"))
    } else {
        Ok((oldest.unwrap().clone(), most_recent.unwrap().clone()))
    }
}
