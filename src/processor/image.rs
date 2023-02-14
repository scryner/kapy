use std::ffi::{CStr, CString, c_void};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use regex::Regex;
use anyhow::{Result, anyhow};
use chrono::{Datelike, DateTime, Local, NaiveDateTime, TimeZone};
use magick_rust::{MagickWand, bindings};

use crate::config::{Command, Config, Format, Quality, Resize};

pub fn process(conf: &Config, in_file: &Path, out_dir: &Path, blob: &Vec<u8>) -> Result<()> {
    let mut wand = MagickWand::new();

    // read image from blob
    wand.read_image_blob(blob)?;

    // get image rating
    let rating = rating(&wand);
    let cmd = conf.command(rating);

    // make out directory
    let taken_at = taken_at(&wand, in_file)?;

    let out_dir = out_dir
        .join(taken_at.year().to_string())
        .join(format!("{}-{}-{}", taken_at.year(), taken_at.month(), taken_at.day()));

    fs::create_dir_all(&out_dir)?;

    // process command
    match manipulate_by_command(&mut wand, cmd)? {
        SaveType::JustCopying => {
            // just copying
            let out_path = out_path(in_file, &out_dir, None)?;
            let out_path = Path::new(&out_path);

            fs::copy(in_file, out_path)?;
        }
        SaveType::NeedConverting(format) => {
            let out_path = out_path(in_file, &out_dir, Some(&format))?;
            wand.write_image(&out_path)?;
        }
    }

    Ok(())
}

pub fn taken_at(wand: &MagickWand, in_file: &Path) -> Result<DateTime<Local>> {
    // try to get date time from EXIF (e.g., 2023:02:03 18:14:18)
    match wand.get_image_property("exif:DateTime") {
        Ok(at) => {
            let naive_date = NaiveDateTime::parse_from_str(&at, "%Y:%m:%d %H:%M:%S")?;
            let local_datetime = Local.from_local_datetime(&naive_date).unwrap();   // never failed
            Ok(local_datetime)
        }
        Err(_) => {
            // try to get data time from created_at in file meta
            let created_at = in_file.metadata()?.created()?;
            Ok(DateTime::from(created_at))
        }
    }
}

fn out_path(in_file: &Path, out_dir: &Path, format: Option<&str>) -> Result<String> {
    let filename = match in_file.file_stem() {
        Some(stem) => stem.to_str().unwrap(),   // never failed
        None => {
            // never reached
            return Err(anyhow!("Failed to find stem of file"));
        }
    };

    let ext = match in_file.extension() {
        Some(ext) => ext.to_str().unwrap(), // never failed
        None => {
            // never reached
            return Err(anyhow!("Failed to find extension of file"));
        }
    };

    let mut dest_filename = String::new();

    match format {
        Some(format) => {
            let mut dest_ext = String::from(format).to_lowercase();
            if dest_ext == "jpeg" {
                dest_ext = String::from("jpg");
            }

            dest_filename = format!("{}.{}", filename, dest_ext);
        }
        None => {
            dest_filename = format!("{}.{}", filename, ext);
        }
    }

    let out_path = out_dir.to_path_buf()
        .join(&dest_filename);

    Ok(String::from(out_path.to_str().unwrap()))    // never failed
}

pub fn read_image_to_blob(path: &Path) -> Result<(Vec<u8>, String)> {
    let wand = MagickWand::new();
    let path = match path.to_str() {
        Some(p) => p,
        None => {
            // never reached
            return Err(anyhow!("Invalid path to have incompatible UTF-8"));
        }
    };

    // read image from file
    wand.read_image(path)?;

    // get file format
    let format = wand.get_image_format()?;

    // write image to blob
    match wand.write_image_blob(&format) {
        Ok(ret) => Ok((ret, format)),
        Err(e) => {
            Err(anyhow!("Failed to write image to blob: {}", e))
        }
    }
}

enum SaveType {
    JustCopying,
    NeedConverting(String),
}

fn manipulate_by_command(wand: &mut MagickWand, cmd: &Command) -> Result<SaveType> {
    let mut need_to_resize = false;
    let mut need_to_adjust_quality = false;
    let mut need_to_convert_ext = false;

    match cmd {
        Command::ByPass => Ok(SaveType::JustCopying),
        Command::Convert {
            resize, format, quality
        } => {
            let mut width = 0;
            let mut height = 0;

            let img_width = wand.get_image_width();
            let img_height = wand.get_image_height();

            // resizing
            loop {
                match resize {
                    Resize::Percentage(percentage) => {
                        if *percentage >= 100 {
                            break;
                        }
                        let ratio: f64 = *percentage as f64 / 100.0;

                        width = (img_width as f64 * ratio) as usize;
                        height = (img_height as f64 * ratio) as usize;

                        need_to_resize = true;
                    }
                    Resize::MPixels(m_pixels) => {
                        let img_pixels = img_width * img_height;
                        let target_pixels = *m_pixels as usize * 1000000;

                        let proportion_to_target = target_pixels as f64 / img_pixels as f64;

                        if proportion_to_target > 0.9 {
                            // not needed to resize (differ under 10%)
                            break;
                        }

                        // calculate target width and height
                        width = (img_width as f64 * proportion_to_target) as usize;
                        height = (img_height as f64 * proportion_to_target) as usize;

                        need_to_resize = true;
                    }
                    Resize::Preserve => ()
                }

                if need_to_resize {
                    if width >= img_width || height >= img_height {
                        return Err(anyhow!("Invalid target image size ({}, {}) from ({}, {})",
                            width, height, img_width, img_height));
                    }

                    wand.resize_image(width, height, bindings::FilterType_LanczosFilter)
                }

                break;
            }

            // quality
            match quality {
                Quality::Percentage(percentage) => {
                    if let Err(e) = wand.set_image_compression_quality(*percentage as usize) {
                        return Err(anyhow!("Failed to set image quality to {}%: {}", percentage, e.to_string()));
                    }
                    need_to_adjust_quality = true;
                }
                Quality::Preserve => ()
            }

            // get original file format
            let orig_format = wand.get_image_format()?;
            let dest_format = format.as_str();

            if orig_format != dest_format {
                need_to_convert_ext = true;
            }

            // return
            if !need_to_resize && !need_to_adjust_quality && !need_to_convert_ext {
                Ok(SaveType::JustCopying)
            } else {
                Ok(SaveType::NeedConverting(String::from(dest_format)))
            }
        }
    }
}

fn image_profile(wand: &MagickWand, name: &str) -> Result<String> {
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

pub fn rating(wand: &MagickWand) -> i8 {
    let xmp = match image_profile(wand, "xmp") {
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
pub fn image_properties(wand: &MagickWand, name: &str) -> Result<Vec<String>> {
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

#[cfg(test)]
mod tests {
    use crate::{config, processor};
    use config::Command;
    use crate::config::Format;
    use crate::processor::do_clone;
    use super::*;

    #[test]
    fn get_format() {
        processor::prelude();

        let wand = MagickWand::new();
        wand.read_image("sample.jpg").unwrap();

        let format = wand.get_image_format().unwrap();
        println!("format = {}", format);
    }

    #[test]
    fn process_to_convert() {
        processor::prelude();

        // read image
        let mut wand = MagickWand::new();
        wand.read_image("sample.jpg").unwrap();

        // read image size
        let origin_width = wand.get_image_width();
        let origin_height = wand.get_image_height();

        // process it
        let command = Command::Convert {
            resize: Resize::Percentage(50),
            format: Format::JPEG,
            quality: Quality::Preserve,
        };

        process_command(&mut wand, &command).unwrap();

        // write image to blob
        let processed = wand.write_image_blob("sample2_2.jpg").unwrap();

        // re-read image from blob
        let wand = MagickWand::new();
        wand.read_image_blob(processed).unwrap();

        // check image size
        let target_width = wand.get_image_width();
        let target_height = wand.get_image_height();

        assert_eq!(origin_width / 2, target_width);
        assert_eq!(origin_height / 2, target_height);
    }
}
