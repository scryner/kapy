pub mod image;
pub mod gps;

use std::ops::Add;
use std::path::Path;
use std::rc::Rc;

use console::style;
use anyhow::{Result, Error, anyhow};
use chrono::{DateTime, FixedOffset, Local};

use crate::config::Config;
use crate::processor::gps::GpsSearch;
use crate::processor::image::{HEIC_FORMAT, Inspection, ProcessState, Statistics as ImageStatistics};

pub struct CloneStatistics {
    pub total_cloned: usize,
    pub image: Option<ImageStatistics>,
}

impl CloneStatistics {
    pub fn new() -> Self {
        Self {
            total_cloned: 0,
            image: None,
        }
    }

    /*
    123 total images (120 succeed / 3 failed)
    ---
     10 just copied
    100 processed
      - 100 gps added
      -  50 resized
      -  30 adjusted quality
      -  90 converted to HEIC
      -   0 converted to JPEG
     */
    pub fn print_with_error(&self, total_images: usize, errors: &Vec<(&Inspection, Error)>) {
        let error_len = errors.len();
        let width = max_width(vec![self.total_cloned, error_len]);
        print!("{:>5} total images", style(self.total_cloned).cyan().bold());
        println!(" ({:>width$} succeed / {} failed)",
                 style(total_images - error_len).green(),
                 if error_len > 0 { style(error_len).red() } else { style(error_len).dim() });

        println!("{}", style("---").dim());

        if let Some(image_stat) = &self.image {
            println!("{:>width$} just copied", image_stat.copying);
            println!("{:>width$} skipped", image_stat.skipped);
            println!("{:>width$} converted", image_stat.converted);

            let converted_stat = &image_stat.converted_statistics;

            let inner_width = max_width(vec![converted_stat.resized,
                                             converted_stat.adjust_quality,
                                             converted_stat.converted_to_heic,
                                             converted_stat.converted_to_jpeg]);

            println!("{:>width$} {:>inner_width$} gps added", style("-").yellow(), converted_stat.gps_added);
            println!("{:>width$} {:>inner_width$} resized", style("-").yellow(), converted_stat.resized);
            println!("{:>width$} {:>inner_width$} adjusted quality", style("-").yellow(), converted_stat.adjust_quality);
            println!("{:>width$} {:>inner_width$} converted to HEIC", style("-").yellow(), converted_stat.converted_to_heic);
            println!("{:>width$} {:>inner_width$} converted to JPEG", style("-").yellow(), converted_stat.converted_to_jpeg);
        }

        // print errors
        if errors.len() > 0 {
            println!("{}", style("---").dim());
            println!("Errors:");
            for (inspection, e) in errors.iter() {
                println!("{} {}: {}", style("-").red(),
                         style(inspection.path.to_str().unwrap()).red().bold(), e);
            }
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
        }
    }
}

pub enum CloneState {
    AddGps(String),
    Reading(String),
    Copying(String, String),
    Converting(String, String, String),
}

pub fn clone_image<'a, F>(conf: &Config,
                          in_file: &Path, out_dir: &Path,
                          inspection: &Inspection,
                          gpx: Rc<Box<dyn GpsSearch + 'a>>,
                          dry_run: bool,
                          when_update: F) -> Result<CloneStatistics>
    where
        F: Fn(CloneState)
{
    let mut statistics = CloneStatistics::new();

    // check arguments
    if !in_file.is_file() {
        return Err(anyhow!("Input path '{}' is not file", in_file.to_str().unwrap()));
    }

    if !out_dir.is_dir() {
        return Err(anyhow!("Output path '{}' is not directory", in_file.to_str().unwrap()));
    }

    // retrieve gps data
    let mut gps_info = None;
    if !inspection.gps_recorded && inspection.format != HEIC_FORMAT {
        // try to match gps
        // currently, EXIV2 the library to manipulate EXIF under hood is not support HEIF/HEIC
        let gpx = gpx.clone();
        let taken_at = inspection.taken_at.to_fixed_offset();

        if let Some(waypoint) = gpx.search(&taken_at) {
            gps_info = Some(image::GpsInfo {
                lat: waypoint.point().y(),
                lon: waypoint.point().x(),
                alt: waypoint.elevation.unwrap_or(0.0),
            });
        } else {
            gps_info = None
        }
    }

    // try to process command to manipulate image
    match image::process(conf, in_file, out_dir, &inspection, gps_info, dry_run, |state| {
        match state {
            ProcessState::Reading(in_path) => {
                when_update(CloneState::Reading(in_path));
            }
            ProcessState::AddingGps(in_path) => {
                when_update(CloneState::AddGps(in_path));
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
