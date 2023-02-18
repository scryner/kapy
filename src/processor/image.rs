use std::collections::HashMap;
use std::ffi::{CStr, CString, c_void, c_char};
use std::fs;
use std::mem::swap;
use std::ops::Add;
use std::path::{Path, PathBuf};
use std::sync::Once;

use regex::Regex;
use anyhow::{Result, anyhow};
use chrono::{Datelike, DateTime, Local, NaiveDateTime, TimeZone};
use magick_rust::{MagickWand, bindings, magick_wand_genesis};

use crate::config::{Command, Config, Format, Quality, Resize};

static START: Once = Once::new();

fn prelude() {
    START.call_once(|| {
        magick_wand_genesis();
    });
}

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
                gps_added: 0,
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
    pub gps_added: usize,
}

impl Add for ConvertedStatistics {
    type Output = ConvertedStatistics;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            resized: self.resized + rhs.resized,
            adjust_quality: self.adjust_quality + rhs.adjust_quality,
            converted_to_jpeg: self.converted_to_jpeg + rhs.converted_to_jpeg,
            converted_to_heic: self.converted_to_heic + rhs.converted_to_heic,
            gps_added: self.gps_added + rhs.gps_added,
        }
    }
}

pub enum ProcessState {
    Reading(String),
    AddingGps(String),
    JustCopying(String, String),
    Rewriting(String, String, String),
}

pub fn process<F>(conf: &Config, in_file: &Path, out_dir: &Path,
                  inspection: &Inspection, gps_info: Option<GpsInfo>,
                  dry_run: bool, when_update: F) -> Result<Statistics>
    where
        F: Fn(ProcessState)
{
    prelude();

    let mut statistics = Statistics::new();

    let taken_at = inspection.taken_at;

    let out_dir = out_dir
        .join(taken_at.year().to_string())
        .join(format!("{:04}-{:02}-{:02}", taken_at.year(), taken_at.month(), taken_at.day()));

    fs::create_dir_all(&out_dir)?;

    let cmd = conf.command(inspection.rating);
    let in_path_str = in_file.file_name().unwrap().to_str().unwrap();

    // process command
    let save_opt = save_option_by_command(cmd, inspection, gps_info)?;
    if let Some(rewrite_info) = save_opt {
        loop {
            // determine file path according to rewrite info
            let out_path_string = out_path(in_file, &out_dir, rewrite_info.target_format.clone())?;
            let out_path = Path::new(&out_path_string);
            let out_filename_str = out_path.file_name().unwrap().to_str().unwrap();

            if out_path.exists() {
                statistics.skipped += 1;
                break;
            }

            let mut wand = MagickWand::new();

            if let Some(gps_info) = rewrite_info.gps_info {
                // read image fom file to blob
                when_update(ProcessState::Reading(String::from(in_path_str)));
                let mut blob = read_image_to_blob(in_file)?;

                // adding gps
                when_update(ProcessState::AddingGps(String::from(in_path_str)));
                let mut other_blob = add_gps_info_to_blob(&blob, gps_info)?;
                swap(&mut blob, &mut other_blob);
                drop(other_blob);

                statistics.converted_statistics.gps_added += 1;

                // re-read from blob
                wand.read_image_blob(&blob)?;
            } else {
                when_update(ProcessState::Reading(String::from(in_path_str)));
                wand.read_image(in_file.to_str().unwrap())?;
            }

            // determine resize
            let img_width = wand.get_image_width();
            let img_height = wand.get_image_height();

            if let Some((width, height)) = determine_resize(img_width, img_height, &rewrite_info.resize) {
                if width >= img_width || height >= img_height {
                    return Err(anyhow!("Invalid target image size ({}, {}) from ({}, {})",
                            width, height, img_width, img_height));
                }

                wand.resize_image(width, height, bindings::FilterType_LanczosFilter);
                statistics.converted_statistics.resized += 1;
            }

            // quality
            if let Some(percentage) = rewrite_info.quality {
                wand.set_image_compression_quality(percentage as usize)?;
                statistics.converted_statistics.adjust_quality += 1;
            }

            // rewrite
            if dry_run {
                statistics.skipped += 1;
            } else {
                // actually rewrite
                when_update(ProcessState::Rewriting(
                    String::from(in_path_str),
                    String::from(out_filename_str),
                    cmd.to_string(),
                ));

                wand.write_image(&out_path_string)?;
                statistics.converted += 1;

                match rewrite_info.target_format {
                    Some(ref format) => {
                        match format.as_str() {
                            JPEG_FORMAT => statistics.converted_statistics.converted_to_jpeg += 1,
                            HEIC_FORMAT => statistics.converted_statistics.converted_to_heic += 1,
                            _ => ()
                        }
                    }
                    None => ()
                }
            }

            break;
        }
    } else {
        // just copying
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

    Ok(statistics)
}

fn out_path(in_file: &Path, out_dir: &Path, format: Option<String>) -> Result<String> {
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

pub const JPEG_FORMAT: &str = "jpeg";
pub const HEIC_FORMAT: &str = "heic";

const META_DATETIME: &str = "Exif.Image.DateTime";
const META_RATING: &str = "Xmp.xmp.Rating";
const META_GPS_LAT: &str = "Exif.GPSInfo.GPSLatitude";
const META_GPS_LON: &str = "Exif.GPSInfo.GPSLongitude";

pub struct Inspection {
    pub path: PathBuf,
    pub format: String,
    pub gps_recorded: bool,
    pub taken_at: DateTime<Local>,
    pub rating: i8,
}

pub fn inspect_image_from_path(path: &Path) -> Result<Inspection> {
    let tags = vec![
        String::from(META_DATETIME),
        String::from(META_RATING),
        String::from(META_GPS_LAT),
        String::from(META_GPS_LON),
    ];

    // get from gexiv2 library
    let (mime, vals) = tags_from_path(path, tags)?;

    // get format
    let format = match mime.as_str() {
        "image/jpeg" => JPEG_FORMAT,
        "video/quicktime" => HEIC_FORMAT,
        _ => return Err(anyhow!("Unsupported mime: {}", mime))
    };

    // get gps recorded
    let lat_recorded = match vals.get(META_GPS_LAT) {
        Some(s) => s.len() > 0,
        None => false,
    };

    let lon_recorded = match vals.get(META_GPS_LON) {
        Some(s) => s.len() > 0,
        None => false,
    };

    let gps_recorded = lat_recorded && lon_recorded;

    // get taken at
    let taken_at;

    match vals.get(META_DATETIME) {
        Some(dt) if dt.len() > 0 => {
            let naive_date = NaiveDateTime::parse_from_str(&dt, "%Y:%m:%d %H:%M:%S")?;
            taken_at = Local.from_local_datetime(&naive_date).unwrap();   // never failed
        }
        _ => {
            let created_at = path.metadata()?.created()?;
            taken_at = DateTime::from(created_at);
        }
    }

    // get rating
    let mut rating = -1;

    if let Some(rating_str) = vals.get(META_RATING) {
        match rating_str.parse::<i8>() {
            Ok(n) => rating = n,
            _ => ()
        }
    }

    Ok(Inspection {
        path: path.to_path_buf(),
        format: format.to_string(),
        gps_recorded,
        taken_at,
        rating,
    })
}

fn read_image_to_blob(path: &Path) -> Result<Vec<u8>> {
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

    // write image to blob
    match wand.write_image_blob(&format) {
        Ok(blob) => Ok(blob),
        Err(e) => {
            Err(anyhow!("Failed to write image to blob: {}", e))
        }
    }
}

pub struct ConvertInfo {
    pub resize: Resize,
    pub quality: Option<u8>,
    pub target_format: Option<String>,
    pub gps_info: Option<GpsInfo>,
}

fn save_option_by_command(cmd: &Command, inspection: &Inspection, gps_info: Option<GpsInfo>) -> Result<Option<ConvertInfo>> {
    let (resize, format, quality) = match cmd {
        Command::Convert { resize, format, quality } => {
            (resize, format, quality)
        }
        Command::ByPass => {
            return if inspection.gps_recorded || gps_info.is_none() {
                Ok(None)
            } else {
                Ok(Some(ConvertInfo {
                    resize: Resize::Preserve,
                    quality: None,
                    target_format: None,
                    gps_info,
                }))
            };
        }
    };

    // resize
    let resize = resize.clone();

    // quality
    let quality = match quality {
        Quality::Percentage(p) => {
            Some(*p)
        }
        Quality::Preserve => None
    };

    // convert
    let convert = match format {
        Format::JPEG if inspection.format.as_str() != JPEG_FORMAT => Some(JPEG_FORMAT.to_string()),
        Format::HEIC if inspection.format.as_str() != HEIC_FORMAT => Some(HEIC_FORMAT.to_string()),
        _ => None
    };

    Ok(Some(ConvertInfo {
        resize,
        quality,
        target_format: convert,
        gps_info,
    }))
}

fn determine_resize(img_width: usize, img_height: usize, resize: &Resize) -> Option<(usize, usize)> {
    match resize {
        Resize::Percentage(percentage) => {
            if *percentage >= 100 {
                return None;
            }
            let ratio: f64 = *percentage as f64 / 100.0;

            let width = (img_width as f64 * ratio) as usize;
            let height = (img_height as f64 * ratio) as usize;

            Some((width, height))
        }
        Resize::MPixels(m_pixels) => {
            let img_pixels = img_width * img_height;
            let target_pixels = *m_pixels as usize * 1000000;

            let proportion_to_target = target_pixels as f64 / img_pixels as f64;

            if proportion_to_target > 0.9 {
                // not needed to resize (differ under 10%)
                return None;
            }

            // calculate target width and height
            let width = (img_width as f64 * proportion_to_target) as usize;
            let height = (img_height as f64 * proportion_to_target) as usize;

            Some((width, height))
        }

        Resize::Preserve => None,
    }
}

#[allow(dead_code)]
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

#[allow(dead_code)]
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
    fn native_get_tags_from_path(path: *const c_char, tags: *mut *mut c_char, tag_len: usize) -> *mut *mut c_char;
}

// safe implementation to add gps info

pub struct GpsInfo {
    pub lat: f64,
    pub lon: f64,
    pub alt: f64,
}

pub fn add_gps_info_to_blob(blob: &Vec<u8>, gps_info: GpsInfo) -> Result<Vec<u8>> {
    let new_len;

    unsafe {
        let blob_len = blob.len();
        let mut out_blob: *mut u8 = std::ptr::null_mut();

        new_len = native_add_gps_info_to_blob(blob.as_ptr(), blob_len, &mut out_blob,
                                              gps_info.lat, gps_info.lon, gps_info.alt);
        if new_len > 0 {
            Ok(Vec::from_raw_parts(out_blob, new_len, new_len))
        } else {
            Err(anyhow!("Failed to add gps info"))
        }
    }
}

// safe implementation to get tags
fn tags_from_path(path: &Path, tags: Vec<String>) -> Result<(String, HashMap<String, String>)> {
    // prepare to pass tags
    let tag_len = tags.len();
    let mut ctags: Vec<CString> = tags.iter().map(|s| CString::new(s.as_str()).unwrap()).collect();
    let mut ctags: Vec<*mut c_char> = ctags
        .iter_mut()
        .map(|vec| vec.as_ptr() as *mut c_char)
        .collect();

    let mut vals = Vec::new();
    let mut mime = String::new();

    unsafe {
        // transform ctags to unsigned char**
        let ctags_ptr: *mut *mut c_char = ctags.as_mut_ptr();

        // call native code
        let path_str = CString::new(path.to_str().unwrap()).unwrap();
        let vals_ptr = native_get_tags_from_path(path_str.as_ptr(), ctags_ptr, tag_len);

        // transform
        for i in 0..tag_len + 1 {
            let val_ptr = *vals_ptr.offset(i as isize) as *const c_char;
            if !val_ptr.is_null() {
                let val_str = CStr::from_ptr(*vals_ptr.offset(i as isize) as *const c_char);
                let val = val_str.to_str()?.to_string();

                if i == tag_len {
                    // at last, we added mime type from native code
                    mime = val;
                } else {
                    vals.push(val);
                }
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

    Ok((mime, m))
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
        prelude();

        let wand = MagickWand::new();
        wand.read_image("sample.jpg").unwrap();

        let format = wand.get_image_format().unwrap();
        println!("format = {}", format);
    }

    #[test]
    fn get_core_metadata() {
        let tags = vec![
            String::from(META_DATETIME),
            String::from(META_RATING),
            String::from(META_GPS_LAT),
            String::from(META_GPS_LON),
        ];

        let path = Path::new("/Users/scryner/geota/IMGP2798.heic");
        let (mime, vals) = tags_from_path(path, tags).unwrap();

        println!("mime: {}", mime);

        for (k, v) in vals.iter() {
            println!("{}: {}", k, v);
        }
    }
}
