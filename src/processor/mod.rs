pub mod image;
pub mod gps;

use std::sync::Once;
use std::ffi::{CStr, CString,c_void};
use std::path::PathBuf;

use anyhow::{Result, anyhow};
use magick_rust::{magick_wand_genesis, MagickWand, bindings};
use regex::Regex;
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
    let rating = image_rating(&wand);
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

fn get_image_profile(wand: &MagickWand, name: &str) -> Result<String> {
    let c_name = CString::new(name).unwrap();
    let mut n = 0;

    let result = unsafe { bindings::MagickGetImageProfile(wand.wand, c_name.as_ptr(), &mut n) };

    let value = if result.is_null() {
        Err(anyhow!("missing profile"))
    } else {
        // convert (and copy) the C string to a Rust string
        let cstr = unsafe { CStr::from_ptr(result as *const i8) };
        Ok(cstr.to_string_lossy().into_owned().trim().to_string())
    };

    unsafe {
        bindings::MagickRelinquishMemory(result as *mut c_void);
    }
    value
}

fn image_rating(wand: &MagickWand) -> i8 {
    let xmp = match get_image_profile(wand, "xmp") {
        Ok(xmp) => xmp,
        _ => return 0,
    };

    let re = Regex::new(r#"xmp:Rating="(?P<rating>[0-9]+)""#).unwrap();
    if let Some(captures) = re.captures(&xmp) {
        let vals = captures.name("rating").unwrap().as_str();
        let val = vals.parse::<i8>().unwrap_or(0);

        return val;
    }

    return 0;
}

#[allow(dead_code)]
fn get_image_properties(wand: &MagickWand, name: &str) -> Result<Vec<String>> {
    let c_name = CString::new(name).unwrap();
    let mut c_n_properties: usize = 0;

    let result = unsafe {
        bindings::MagickGetImageProperties(wand.wand, c_name.as_ptr(), &mut c_n_properties)
    };

    let mut properties = Vec::new();

    let value = if result.is_null() {
        Err(anyhow!("missing properties"))
    } else {
        for i in 0..c_n_properties {
            let ptr = unsafe { *(result.add(i)) };

            let cstr = unsafe { CStr::from_ptr(ptr) };
            properties.push(cstr.to_string_lossy().into_owned());
        }
        Ok(properties)
    };

    unsafe {
        bindings::MagickRelinquishMemory(result as *mut c_void);
    }
    value
}