pub mod image;
pub mod gps;

use std::ops::Add;
use std::sync::Once;
use std::path::Path;

use anyhow::{Result, anyhow};
use magick_rust::magick_wand_genesis;
use crate::config::Config;
use crate::processor::image::{Statistics as ImageStatistics};

static START: Once = Once::new();

pub struct CloneStatistics {
    pub image: Option<ImageStatistics>,
}

impl CloneStatistics {
    pub fn new() -> Self {
        Self {
            image: None,
        }
    }
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
            image: image_stat,
        }
    }
}


fn prelude() {
    START.call_once(|| {
        magick_wand_genesis();
    });
}

pub fn clone_image(conf: &Config, in_file: &Path, out_dir: &Path) -> Result<CloneStatistics> {
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
    let (blob, format) = image::read_image_to_blob(in_file)?;

    if format.to_lowercase() != "heic" {
        // try to match gps
        // currently, EXIV2 the library to manipulate EXIF under hood is not support HEIF/HEIC

        todo!();
    }

    // try to process command to manipulate image
    match image::process(conf, in_file, out_dir, &blob) {
        Ok(image_stat) => {
            statistics.image = Some(image_stat);
        }
        Err(e) => {
            return Err(anyhow!("Failed to process image: {}", e.to_string()));
        }
    }

    Ok(statistics)
}

