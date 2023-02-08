pub mod image;
pub mod gps;

use std::sync::Once;
use std::path::PathBuf;

use anyhow::{Result, anyhow};
use magick_rust::{magick_wand_genesis, MagickWand};
use crate::config::Config;

static START: Once = Once::new();

fn prelude() {
    START.call_once(|| {
        magick_wand_genesis();
    });
}

pub fn do_clone(conf: &Config, in_path: PathBuf, out_path: PathBuf) -> Result<()> {
    // Initialize MagickWand if it needed
    prelude();

    let in_path = in_path.to_str().unwrap(); // never failed
    let mut wand = MagickWand::new();

    // try to read image
    if let Err(e) = wand.read_image(in_path) {
        return Err(anyhow!("Failed to read image from '{}': {}", in_path, e.to_string()));
    }

    // try to match gps

    // get image rating
    let rating = image::rating(&wand);
    let command = conf.command(rating);

    // try to process command to manipulate image
    if let Err(e) = image::process_command(&mut wand, command) {
        return Err(anyhow!("Failed to process image: {}", e.to_string()));
    }

    // write(clone) image
    let out_path = out_path.to_str().unwrap();
    if let Err(e) = wand.write_image(out_path) {
        return Err(anyhow!("Failed to write image from '{}': {}", out_path, e.to_string()));
    }

    Ok(())
}

