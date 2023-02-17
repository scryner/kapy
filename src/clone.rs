use std::path::{Path, PathBuf};
use std::{fs, process};
use std::time::SystemTime;
use anyhow::{anyhow, Result};
use core::time::Duration;
use std::rc::Rc;
use chrono::{Local, LocalResult, NaiveDateTime, TimeZone};
use console::style;
use regex::Regex;
use walkdir::{WalkDir, DirEntry};

use crate::processor::gps::{GpsSearch, GpxStorage, NoopGpsSearch};
use crate::drive::GoogleDrive;
use crate::drive::auth::{CredPath, GoogleAuthenticator, ListenPort};
use crate::config::Config;
use crate::processor;
use crate::processor::{CloneStatistics, CloneState};
use crate::progress::{PanelType, Progress, Update};

const MAX_DEPTH: usize = 10;
const DEFAULT_MAX_SEARCH_FILES_ON_GOOGLE_DRIVE: usize = 100;
const DEFAULT_GPS_MATCH_WITHIN: Duration = Duration::from_secs(5 * 60); // match within 5 min

pub fn do_clone(conf: Config, cred_path: &Path, ignore_geotag: bool, dry_run: bool) {
    // print info
    let import_from = conf.import_from().to_str().unwrap();
    let import_to = conf.import_to().to_str().unwrap();
    println!("Cloning from {} to {}...", style(import_from).bold().cyan(),
             style(import_to).bold().green());

    // check path existence
    if !conf.import_from().exists() {
        eprintln!("Invalid 'from' directory: not existed");
        process::exit(1)
    } else if !conf.import_from().is_dir() {
        eprintln!("Invalid 'from' directory: it is a file, not directory");
        process::exit(1)
    }

    if !conf.import_to().exists() {
        eprintln!("Invalid 'to' directory: not existed");
        process::exit(1)
    } else if !conf.import_to().is_dir() {
        eprintln!("Invalid 'to' directory: it is a file, not directory");
        process::exit(1)
    }

    // calculate when to copy started (since the last save to 'conf.to_path')
    let to_be_import_after = match to_be_imported_after(conf.import_to()) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to determine date and time to be imported after: {}", e);
            process::exit(1);
        }
    };

    // get to import files
    let import_entries = import_entries(conf.import_from());

    // filter import files to retrieve
    let import_entries = match to_be_import_after {
        Some(t) => {
            import_entries.into_iter().filter(|entry| {
                let entry_created_at = entry.metadata().unwrap().created().unwrap();
                entry_created_at > t
            }).collect()
        }
        None => import_entries
    };

    // calculate first date and end date among import files
    let (oldest_created_at, most_recent_created_at) = match oldest_and_most_recent_created(&import_entries) {
        Ok((oldest, most_recent)) => (oldest, most_recent),
        Err(e) => {
            eprintln!("Failed to find oldest and most recent files: {}", e);
            process::exit(1);
        }
    };

    // make gps search trait object
    let gps_search: Rc<Box<dyn GpsSearch>> = if ignore_geotag {
        Rc::new(Box::new(NoopGpsSearch))
    } else {
        // make a progress
        println!("Preparing GPX storage from google drive...");
        let progress = Progress::new(vec![
            PanelType::Message("gpx_filename"),
        ]);

        // initialize google drive
        let mut count = 0;

        let auth = GoogleAuthenticator::new(ListenPort::DefaultPort, CredPath::Path(cred_path));
        let drive = GoogleDrive::new(auth);

        match GpxStorage::from_google_drive(&drive, oldest_created_at, most_recent_created_at,
                                            DEFAULT_MAX_SEARCH_FILES_ON_GOOGLE_DRIVE, DEFAULT_GPS_MATCH_WITHIN,
                                            |filename| {
                                                progress.update("gpx_filename",
                                                                Update::Incr(Some(format!("{} is downloading and pouring...", style(filename).bold()))));
                                                count += 1;
                                            }) {
            Ok(search) => {
                progress.finish_all();
                progress.println(format!("{} gpx files are retrieved", style(count).bold()));
                progress.clear();

                Rc::new(Box::new(search))
            }
            Err(e) => {
                eprintln!("Failed to initialize geotag search on your google drive: {}", e);
                process::exit(1);
            }
        }
    };

    // process clone
    let mut clone_statistics = CloneStatistics::new();
    let mut errors = Vec::new();

    // make progress
    {
        let progress = Progress::new(vec![
            PanelType::Bar("files_bar", import_entries.len() as u64),
            PanelType::Message("state"),
        ]);

        for entry in import_entries.iter() {
            let gps_search = Rc::clone(&gps_search);

            match processor::clone_image(&conf, entry.path(), conf.import_to(),
                                         gps_search, dry_run,
                                         |state| {
                                             match state {
                                                 CloneState::Inspect(in_path) => {
                                                     progress.update("state", Update::Incr(Some(format!("{}: inspecting...", style(in_path).bold()))));
                                                 }
                                                 CloneState::AddGps(in_path) => {
                                                     progress.update("state", Update::Incr(Some(format!("{}: adding gps info...", style(in_path).bold()))));
                                                 }
                                                 CloneState::Reading(in_path) => {
                                                     progress.update("state", Update::Incr(Some(format!("{}: reading...", style(in_path).bold()))));
                                                 }
                                                 CloneState::Copying(in_path, out_path) => {
                                                     progress.update("state", Update::Incr(Some(format!("{} {} {}: copying...", style(in_path).cyan(), style("→").bold(), style(out_path).green()))));
                                                 }
                                                 CloneState::Converting(in_path, out_path, cmd) => {
                                                     progress.update("state", Update::Incr(Some(format!("{} {} {}: converting {}...", style(in_path).cyan(), style("→").bold(), style(out_path).green(), style(cmd).dim()))));
                                                 }
                                             }
                                         }) {
                Ok(stat) => {
                    clone_statistics = clone_statistics + stat;
                    progress.update("files_bar", Update::Incr(None));
                }
                Err(e) => {
                    errors.push((entry, e));
                }
            }
        }

        progress.finish_all();
        progress.clear();
    }

    // print-out clone statistics
    clone_statistics.print_with_error(&errors);
}

fn import_entries(dir: &Path) -> Vec<DirEntry> {
    walk_and_filter_only_supported_images(dir)
}

fn oldest_and_most_recent_created(entries: &Vec<DirEntry>) -> Result<(SystemTime, SystemTime)> {
    let created_at_list = entries.iter()
        .map(|entry| entry.metadata().unwrap().created().unwrap())
        .collect::<Vec<SystemTime>>();

    let oldest = created_at_list.iter().min();
    let most_recent = created_at_list.iter().max();

    if oldest == None || most_recent == None {
        Err(anyhow!("Failed to find oldest and most recent file"))
    } else {
        Ok((oldest.unwrap().clone(), most_recent.unwrap().clone()))
    }
}

const RE_ONLY_YEAR: &str = "^[0-9]{4}$";
const RE_YEAR_MONTH_DAY: &str = r"(?P<year>[0-9]{4})-(?P<month>[0-9]{2})-(?P<day>[0-9]{2})$";

fn to_be_imported_after(out_dir: &Path) -> Result<Option<SystemTime>> {
    // find first-level: e.g., 2023
    let first_depth_dir = get_last_modified_dir(out_dir, Some(RE_ONLY_YEAR))?;
    if let Some(first_depth_dir) = first_depth_dir {
        // find second-level: e.g., 2023-02-16
        return if let Some(second_depth_dir) = get_last_modified_dir(&first_depth_dir, Some(RE_YEAR_MONTH_DAY))? {
            let t = system_time_from_str(second_depth_dir.file_name().unwrap().to_str().unwrap())?;
            Ok(Some(t))
        } else {
            // get first day of given year
            let first_day_of_year = system_time_from_str(first_depth_dir.file_name().unwrap().to_str().unwrap())?;
            Ok(Some(first_day_of_year))
        };
    }

    Ok(None)
}

fn walk_and_filter_only_supported_images(dir: &Path) -> Vec<DirEntry> {
    let mut entries = Vec::new();

    for entry in WalkDir::new(dir)
        .max_depth(MAX_DEPTH)
        .into_iter()
        .filter_map(|entry| {
            let entry = match entry {
                Ok(v) => v,
                Err(_) => return None,
            };

            let path = entry.path();
            if !path.is_file() {
                return None;
            }

            if let Some(ext) = path.extension()?.to_str() {
                return match ext.to_lowercase().as_str() {
                    "jpeg" | "jpg" | "heic" => Some(entry),
                    _ => None,
                };
            }

            None
        }) {
        entries.push(entry);
    }

    entries
}

fn get_last_modified_dir(dir: &Path, re_pattern: Option<&str>) -> Result<Option<PathBuf>> {
    let mut last_modified: Option<PathBuf> = None;

    for entry in fs::read_dir(dir)? {
        let entry = entry?;

        if entry.file_type()?.is_dir() {
            if let Some(pattern) = re_pattern {
                if let Some(filename) = entry.file_name().to_str() {
                    let re = Regex::new(pattern)?;
                    if !re.is_match(filename) {
                        continue;
                    }
                } else {
                    continue;
                }
            }

            if let Some(ref prev_entry) = last_modified {
                let prev_modified_time = prev_entry.metadata()?.modified()?;
                let modified_time = entry.metadata()?.modified()?;
                if modified_time > prev_modified_time {
                    last_modified = Some(entry.path());
                }
            } else {
                last_modified = Some(entry.path());
            }
        }
    }

    Ok(last_modified)
}

fn system_time_from_str(s: &str) -> Result<SystemTime> {
    let re_only_year = Regex::new(RE_ONLY_YEAR)?;
    let re_year_month_day = Regex::new(RE_YEAR_MONTH_DAY)?;

    let naive_str;

    if re_only_year.is_match(s) {
        naive_str = format!("{}-01-01 00:00:00", s);
    } else if re_year_month_day.is_match(s) {
        let captures = re_year_month_day.captures(s).unwrap();

        let year = captures.name("year").unwrap().as_str();
        let month = captures.name("month").unwrap().as_str();
        let day = captures.name("day").unwrap().as_str();

        naive_str = format!("{}-{}-{} 00:00:00", year, month, day);
    } else {
        return Err(anyhow!("Invalid str to convert to system time '{}'", s));
    }

    let naive_dt = NaiveDateTime::parse_from_str(&naive_str, "%Y-%m-%d %H:%M:%S")?;
    let local_dt = match Local.from_local_datetime(&naive_dt) {
        LocalResult::Single(dt) => {
            dt
        }
        _ => {
            // never reached
            return Err(anyhow!("Failed to local datetime"));
        }
    };


    Ok(SystemTime::UNIX_EPOCH + Duration::from_secs(local_dt.timestamp() as u64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_time() {
        let s = "2023";
        let st = system_time_from_str(s).unwrap();

        println!("{} => {:?}", s, st);

        let s = "2023-02-16";
        let st = system_time_from_str(s).unwrap();

        println!("{} => {:?}", s, st);
    }
}
