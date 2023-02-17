pub mod image;
pub mod gps;

use std::mem;
use std::ops::Add;
use std::sync::Once;
use std::path::Path;
use std::rc::Rc;

use console::style;
use anyhow::{Result, Error, anyhow};
use chrono::{DateTime, FixedOffset, Local};
use magick_rust::magick_wand_genesis;
use walkdir::DirEntry;
use crate::config::Config;
use crate::processor::gps::GpsSearch;
use crate::processor::image::{ProcessState, Statistics as ImageStatistics};

static START: Once = Once::new();

pub struct CloneStatistics {
    pub total_cloned: usize,
    pub image: Option<ImageStatistics>,
    pub gps_added: usize,
}

impl CloneStatistics {
    pub fn new() -> Self {
        Self {
            total_cloned: 0,
            image: None,
            gps_added: 0,
        }
    }

    /*
    ---
    123 total images
    120 succeed / 3 failed
    ---
    110 added gps info
     10 just copied
    100 processed
      - 50 resized
      - 30 adjusted quality
      - 90 converted to HEIC
      -  0 converted to JPEG
     */
    pub fn print_with_error(&self, errors: &Vec<(&DirEntry, Error)>) {
        println!("{}", style("---").dim());

        let error_len = errors.len();
        let width = max_width(vec![self.total_cloned, error_len]);
        println!("{:>width$} total images", style(self.total_cloned).blue());
        println!("{:>width$} succeed / {} failed ",
                 style(self.total_cloned - error_len).green(),
                 if error_len > 0 { style(error_len).red() } else { style(error_len).dim() });

        println!("{}", style("---").dim());

        println!("{:>width$} added gps info", self.gps_added);
        if let Some(image_stat) = &self.image {
            println!("{:>width$} just copied", image_stat.copying);
            println!("{:>width$} processed", image_stat.converted);

            let converted_stat = &image_stat.converted_statistics;

            let inner_width = max_width(vec![converted_stat.resized,
                                             converted_stat.adjust_quality,
                                             converted_stat.converted_to_heic,
                                             converted_stat.converted_to_jpeg]);

            println!("{:>width$} {:>inner_width$} resized", style("-").yellow(), converted_stat.resized);
            println!("{:>width$} {:>inner_width$} adjusted quality", style("-").yellow(), converted_stat.adjust_quality);
            println!("{:>width$} {:>inner_width$} converted to HEIC", style("-").yellow(), converted_stat.converted_to_heic);
            println!("{:>width$} {:>inner_width$} converted to JPEG", style("-").yellow(), converted_stat.converted_to_jpeg);
        }
    }
}

fn max_width(nums: Vec<usize>) -> usize {
    let widths: Vec<usize> = nums.iter().map(|n| {
        (*n as f64).log10().floor() as usize + 1
    }).collect();

    *widths.iter().max().unwrap()
}

impl Add for CloneStatistics {
    type Output = CloneStatistics;

    fn add(self, rhs: Self) -> Self::Output {
        let image_stat = match (self.image, rhs.image) {
            (Some(self_image), Some(rhs_image)) => {
                Some(self_image + rhs_image)
            }
            (Some(self_image), None) => {
                Some(self_image)
            }
            (None, Some(rhs_image)) => {
                Some(rhs_image)
            }
            (None, None) => None,
        };


        Self {
            total_cloned: self.total_cloned + rhs.total_cloned,
            image: image_stat,
            gps_added: self.gps_added + rhs.gps_added,
        }
    }
}

fn prelude() {
    START.call_once(|| {
        magick_wand_genesis();
    });
}

pub enum CloneState {
    Inspect(String),
    AddGps(String),
    Reading(String),
    Copying(String, String),
    Converting(String, String, String),
}

pub fn clone_image<'a, F>(conf: &Config,
                          in_file: &Path, out_dir: &Path,
                          gpx: Rc<Box<dyn GpsSearch + 'a>>,
                          dry_run: bool,
                          when_update: F) -> Result<CloneStatistics>
    where
        F: Fn(CloneState)
{
    // Initialize MagickWand if it needed
    prelude();

    let mut statistics = CloneStatistics::new();

    // check arguments
    if !in_file.is_file() {
        return Err(anyhow!("Input path '{}' is not file", in_file.to_str().unwrap()));
    }

    if !out_dir.is_dir() {
        return Err(anyhow!("Output path '{}' is not directory", in_file.to_str().unwrap()));
    }

    // try to read image
    let in_path_str = in_file.file_name().unwrap().to_str().unwrap();    // never failed
    when_update(CloneState::Inspect(String::from(in_path_str)));

    let image_blob = image::read_image_to_blob(in_file)?;

    // move value from image_blob
    let gps_recorded = image_blob.gps_recorded;
    let format = image_blob.format;
    let taken_at = image_blob.taken_at;
    let mut blob = image_blob.blob;
    let mut gps_added = false;

    if !gps_recorded && format.to_lowercase() != "heic" {
        // try to match gps
        // currently, EXIV2 the library to manipulate EXIF under hood is not support HEIF/HEIC
        when_update(CloneState::AddGps(String::from(in_path_str)));

        let taken_at = taken_at.to_fixed_offset();
        let gpx = gpx.clone();

        if let Some(waypoint) = gpx.search(&taken_at) {
            let lat = waypoint.point().y();
            let lon = waypoint.point().x();
            let alt = waypoint.elevation.unwrap_or(0.0);

            let mut other_blob = gps::add_gps_info(&blob, lat, lon, alt)?;
            mem::swap::<Vec<u8>>(&mut blob, &mut other_blob);

            // early drop other_blob
            drop(other_blob);

            gps_added = true;
        }
    }

    // try to process command to manipulate image
    match image::process(conf, in_file, out_dir, &blob, dry_run, |state| {
        match state {
            ProcessState::Reading(in_path) => {
                when_update(CloneState::Reading(in_path));
            }
            ProcessState::JustCopying(in_path, out_path) => {
                when_update(CloneState::Copying(in_path, out_path));
            }
            ProcessState::Rewriting(in_path, out_path, cmd) => {
                when_update(CloneState::Converting(in_path, out_path, cmd));
            }
        }
    }) {
        Ok(image_stat) => {
            statistics.total_cloned += 1;
            statistics.image = Some(image_stat);
            statistics.gps_added += if gps_added { 1 } else { 0 };
        }
        Err(e) => {
            return Err(anyhow!("Failed to process image: {}", e.to_string()));
        }
    }

    Ok(statistics)
}

trait ToFixedOffset {
    fn to_fixed_offset(&self) -> DateTime<FixedOffset>;
}

impl ToFixedOffset for DateTime<Local> {
    fn to_fixed_offset(&self) -> DateTime<FixedOffset> {
        self.with_timezone(self.offset())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_max_width() {
        let v = vec![123, 435322, 2];
        let width = max_width(v);

        assert_eq!(width, 6);
    }
}
