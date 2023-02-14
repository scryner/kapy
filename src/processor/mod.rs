pub mod image;
pub mod gps;

use std::fs;
use std::sync::Once;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use magick_rust::{magick_wand_genesis, MagickWand};
use crate::config::Config;

static START: Once = Once::new();

fn prelude() {
    START.call_once(|| {
        magick_wand_genesis();
    });
}

pub fn do_clone(conf: &Config, in_file: &Path, out_dir: &Path) -> Result<()> {
    // Initialize MagickWand if it needed
    prelude();

    // check arguments
    if !in_file.is_file() {
        return Err(anyhow!("Input path '{}' is not file", in_file.to_str().unwrap()))
    }

    if !out_dir.is_dir() {
        return Err(anyhow!("Output path '{}' is not directory", in_file.to_str().unwrap()))
    }

    // try to read image
    let (blob, format) = image::read_image_to_blob(in_file)?;

    if format.to_lowercase() != "heic" {
        // try to match gps
        // currently, EXIV2 the library to manipulate EXIF under hood is not support HEIF/HEIC

        todo!();
    }

    // try to process command to manipulate image
    if let Err(e) = image::process(conf, in_file, out_dir, &blob) {
        return Err(anyhow!("Failed to process image: {}", e.to_string()));
    }

    Ok(())
}

