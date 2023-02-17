use std::collections::HashMap;
use std::ffi::{CStr, CString, c_void, c_uchar, c_char};
use std::fs;
use std::ops::Add;
use std::path::Path;

use regex::Regex;
use anyhow::{Result, anyhow};
use chrono::{Datelike, DateTime, Local, NaiveDateTime, TimeZone};
use magick_rust::{MagickWand, bindings};

use crate::config::{Command, Config, Quality, Resize};

pub struct Statistics {
    pub skipped: usize,
    pub copying: usize,
    pub converted: usize,
    pub converted_statistics: ConvertedStatistics,
}

impl Statistics {
    fn new() -> Self {
        Self {
            skipped: 0,
            copying: 0,
            converted: 0,
            converted_statistics: ConvertedStatistics {
                resized: 0,
                adjust_quality: 0,
                converted_to_jpeg: 0,
                converted_to_heic: 0,
            },
        }
    }
}

impl Add for Statistics {
    type Output = Statistics;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            skipped: self.skipped + rhs.skipped,
            copying: self.copying + rhs.copying,
            converted: self.converted + rhs.converted,
            converted_statistics: self.converted_statistics + rhs.converted_statistics,
        }
    }
}

pub struct ConvertedStatistics {
    pub resized: usize,
    pub adjust_quality: usize,
    pub converted_to_jpeg: usize,
    pub converted_to_heic: usize,
}

impl Add for ConvertedStatistics {
    type Output = ConvertedStatistics;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            resized: self.resized + rhs.resized,
            adjust_quality: self.adjust_quality + rhs.adjust_quality,
            converted_to_jpeg: self.converted_to_jpeg + rhs.converted_to_jpeg,
            converted_to_heic: self.converted_to_heic + rhs.converted_to_heic,
        }
    }
}

pub enum ProcessState {
    Reading(String),
    JustCopying(String, String),
    Rewriting(String, String, String),
}

pub fn process<F>(conf: &Config, in_file: &Path, out_dir: &Path,
                  blob: &Vec<u8>, dry_run: bool, when_update: F) -> Result<Statistics>
    where
        F: Fn(ProcessState)
{
    let mut wand = MagickWand::new();
    let mut statistics = Statistics::new();

    // read image from blob
    let in_path_str = in_file.file_name().unwrap().to_str().unwrap();

    when_update(ProcessState::Reading(String::from(in_path_str)));
    wand.read_image_blob(blob)?;

    // get image rating
    let rating = rating_from_wand(&wand);
    let cmd = conf.command(rating);

    // make out directory
    let taken_at = taken_at(&wand, in_file)?;

    let out_dir = out_dir
        .join(taken_at.year().to_string())
        .join(format!("{:04}-{:02}-{:02}", taken_at.year(), taken_at.month(), taken_at.day()));

    fs::create_dir_all(&out_dir)?;

    // process command
    match manipulate_by_command(&mut wand, cmd)? {
        SaveType::JustCopying => {
            if !dry_run {
                // just copying
                let out_path = out_path(in_file, &out_dir, None)?;
                let out_path = Path::new(&out_path);
                let out_path_str = out_path.file_name().unwrap().to_str().unwrap();

                if !out_path.exists() {
                    when_update(ProcessState::JustCopying(
                        String::from(in_path_str),
                        String::from(out_path_str)));

                    fs::copy(in_file, out_path)?;
                    statistics.copying += 1;
                } else {
                    statistics.skipped += 1;
                }
            } else {
                statistics.skipped += 1;
            }
        }
        SaveType::NeedRewrite {
            resize,
            adjust_quality,
            convert,
            format
        } => {
            if !dry_run {
                let out_path_string = out_path(in_file, &out_dir, Some(&format))?;
                let out_path = Path::new(&out_path_string);
                let out_filename_str = out_path.file_name().unwrap().to_str().unwrap();

                if !out_path.exists() {
                    when_update(ProcessState::Rewriting(
                        String::from(in_path_str),
                        String::from(out_filename_str),
                        cmd.to_string(),
                    ));

                    wand.write_image(&out_path_string)?;
                    statistics.converted += 1;

                    if resize { statistics.converted_statistics.resized += 1 };
                    if adjust_quality { statistics.converted_statistics.adjust_quality += 1 };
                    if convert {
                        match format.to_lowercase().as_str() {
                            "jpeg" | "jpg" => statistics.converted_statistics.converted_to_jpeg += 1,
                            "heic" => statistics.converted_statistics.converted_to_heic += 1,
                            _ => {
                                panic!("never reached! wrong format '{}'", format);
                            }
                        }
                    }
                } else {
                    statistics.skipped += 1;
                }
            } else {
                statistics.skipped += 1;
            }
        }
    }

    Ok(statistics)
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

    let dest_filename;

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

pub struct ImageBlob {
    pub blob: Vec<u8>,
    pub format: String,
    pub gps_recorded: bool,
    pub taken_at: DateTime<Local>,
}

pub fn read_image_to_blob(path: &Path) -> Result<ImageBlob> {
    let wand = MagickWand::new();
    let path_str = match path.to_str() {
        Some(p) => p,
        None => {
            // never reached
            return Err(anyhow!("Invalid path to have incompatible UTF-8"));
        }
    };

    // read image from file
    wand.read_image(path_str)?;

    // get file format
    let format = wand.get_image_format()?;

    // get gps recorded
    let gps_recorded = gps_recorded(&wand);

    // get taken at
    let taken_at = taken_at(&wand, path)?;

    // write image to blob
    match wand.write_image_blob(&format) {
        Ok(blob) => Ok(ImageBlob {
            blob,
            format,
            gps_recorded,
            taken_at,
        }),
        Err(e) => {
            Err(anyhow!("Failed to write image to blob: {}", e))
        }
    }
}

fn gps_recorded(wand: &MagickWand) -> bool {
    match (wand.get_image_property("exif:GPSLatitude"), wand.get_image_property("exif:GPSLongitude")) {
        (Ok(_), Ok(_)) => true,
        (_, _) => false
    }
}

enum SaveType {
    JustCopying,
    NeedRewrite {
        resize: bool,
        adjust_quality: bool,
        convert: bool,
        format: String,
    },
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
                Ok(SaveType::NeedRewrite {
                    resize: need_to_resize,
                    adjust_quality: need_to_adjust_quality,
                    convert: need_to_convert_ext,
                    format: String::from(dest_format),
                })
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

pub fn rating_from_wand(wand: &MagickWand) -> i8 {
    let xmp = match image_profile(wand, "xmp") {
        Ok(xmp) => xmp,
        _ => return -1,
    };

    let re = Regex::new(r#"xmp:Rating="(?P<rating>[0-9]+)""#).unwrap();
    if let Some(captures) = re.captures(&xmp) {
        let vals = captures.name("rating").unwrap().as_str();
        let val = vals.parse::<i8>().unwrap_or(0);

        return val;
    }

    return -1;
}

// native implementation to add gps info
extern "C" {
    fn native_add_gps_info_to_blob(blob: *const u8, blob_len: usize, out_blob: *mut *mut u8, lat: f64, lon: f64, alt: f64) -> usize;
    fn native_get_rating_from_path(path: *const u8) -> i32;
    fn native_get_tags_from_path(path: *const c_uchar, tags: *mut *mut c_uchar, tag_len: usize) -> *mut *mut c_uchar;
}

// safe implementation to add gps info
pub fn add_gps_info_to_blob(blob: &Vec<u8>, lat: f64, lon: f64, alt: f64) -> Result<Vec<u8>> {
    let new_len;

    unsafe {
        let blob_len = blob.len();
        let mut out_blob: *mut u8 = std::ptr::null_mut();

        new_len = native_add_gps_info_to_blob(blob.as_ptr(), blob_len, &mut out_blob, lat, lon, alt);
        if new_len > 0 {
            Ok(Vec::from_raw_parts(out_blob, new_len, new_len))
        } else {
            Err(anyhow!("Failed to add gps info"))
        }
    }
}

// safe implementation to get tags
pub fn tags_from_path(path: &Path, tags: Vec<String>) -> Result<HashMap<String, String>> {
    // prepare to pass tags
    let tag_len = tags.len();
    let mut ctags: Vec<Vec<u8>> = tags.iter().map(|s| s.as_bytes().to_vec()).collect();
    let mut ctags: Vec<*mut c_uchar> = ctags
        .iter_mut()
        .map(|vec| vec.as_mut_ptr())
        .collect();

    let mut vals = Vec::new();

    unsafe {
        // transform ctags to unsigned char**
        let mut ctags_ptr: *mut *mut c_uchar = ctags.as_mut_ptr();

        // call native code
        let path_str = CString::new(path.to_str().unwrap()).unwrap();
        let vals_ptr = native_get_tags_from_path(path_str.as_ptr() as *const c_uchar, ctags_ptr, tag_len);

        // transform
        for i in 0..tag_len {
            let val_ptr = *vals_ptr.offset(i as isize) as *const c_char;
            if !val_ptr.is_null() {
                let val_str = CStr::from_ptr(*vals_ptr.offset(i as isize) as *const c_char);
                let val = val_str.to_str()?.to_string();
                vals.push(val);
            } else {
                vals.push(String::new());
            }
        }
    }

    // make hashmap according to tags
    let mut m = HashMap::new();
    for (i, tag) in tags.iter().enumerate() {
        m.insert(tag.clone(), vals.get(i).unwrap().clone());
    }

    Ok(m)
}

// safe implementation to get rating info
#[allow(dead_code)]
pub fn rating_from_path(path: &Path) -> i8 {
    let rating;
    let path_str = path.to_str().unwrap();

    unsafe {
        rating = native_get_rating_from_path(path_str.as_ptr());
    }

    rating as i8
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

        manipulate_by_command(&mut wand, &command).unwrap();

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

    #[test]
    fn get_core_metadata() {
        let tags = vec![
            "Exif.Image.DateTime".to_string(),
            "Xmp.xmp.Rating".to_string(),
            "Exif.GPSInfo.GPSLatitude".to_string(),
            "Exif.GPSInfo.GPSLongitude".to_string(),
            "Exif.GPSInfo.GPSAltitude".to_string(),
        ];

        let path = Path::new("sample.jpg");
        let vals = tags_from_path(path, tags).unwrap();

        for (k, v) in vals.iter() {
            println!("{}: {}", k, v);
        }
    }
}
