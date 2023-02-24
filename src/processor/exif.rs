use std::ffi::{c_char, c_int, CStr, CString};
use std::ops::Deref;
use std::path::Path;

use anyhow::{anyhow, Result};

#[repr(C)]
struct ExifMetadataT {
    // opaque structure
    _data: [u8; 0],
    _marker:
    core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
}

#[link(name = "libexif")]
extern "C" {
    fn exif_metadata_new() -> *mut ExifMetadataT;
    fn exif_metadata_open(metadata: *mut ExifMetadataT, path: *const c_char) -> c_int;
    fn exif_metadata_open_blob(metadata: *mut ExifMetadataT, blob: *const u8, blob_len: usize) -> c_int;
    fn exif_metadata_save_blob(metadata: *mut ExifMetadataT, blob: *const u8, blob_len: usize, out_blob: *mut *mut u8) -> usize;
    fn exif_metadata_add_gps_info(metadata: *mut ExifMetadataT, lat: f64, lon: f64, alt: f64) -> c_int;
    fn exif_get_mime(metadata: *mut ExifMetadataT) -> *mut c_char;
    fn exif_get_tag_string(metadata: *mut ExifMetadataT, tag: *const c_char) -> *mut c_char;
    fn exif_metadata_destroy(metadata: *const *mut ExifMetadataT);
}

// safe implementation
pub struct Metadata {
    raw: *mut ExifMetadataT,
}

impl Drop for Metadata {
    fn drop(&mut self) {
        unsafe {
            exif_metadata_destroy(&self.raw);
        }
    }
}

impl Metadata {
    pub fn new_from_path(path: Box<dyn AsRef<Path>>) -> Result<Self> {
        unsafe {
            let raw = exif_metadata_new();
            let path = path.deref().as_ref();

            let path = match path.to_str() {
                Some(path) => CString::new(path)?,
                None => return Err(anyhow!("Invalid path"))
            };

            let rc = exif_metadata_open(raw, path.as_ptr());
            if rc == 0 {
                Ok(Metadata {
                    raw
                })
            } else {
                Err(anyhow!("Failed to read metadata"))
            }
        }
    }

    pub fn new_from_blob(blob: &Vec<u8>) -> Result<Self> {
        unsafe {
            let raw = exif_metadata_new();

            let rc = exif_metadata_open_blob(raw, blob.as_ptr(), blob.len());
            if rc == 0 {
                Ok(Metadata {
                    raw
                })
            } else {
                Err(anyhow!("Failed to read metadata"))
            }
        }
    }

    pub fn get_mime(&self) -> Result<String> {
        unsafe {
            let val = exif_get_mime(self.raw);
            if val.is_null() {
                return Err(anyhow!("Failed to get mime"));
            }

            let val = CStr::from_ptr(val as *const c_char);
            let val = val.to_str()?.to_string();

            Ok(val)
        }

    }

    pub fn get_tag<T>(&self, tag: T) -> Result<String>
        where T: AsRef<str> {
        let tag = CString::new(tag.as_ref()).unwrap();
        let tag = tag.as_ptr();

        unsafe {
            let val = exif_get_tag_string(self.raw, tag);
            if val.is_null() {
                return Err(anyhow!("Failed to get tag string"));
            }

            let val = CStr::from_ptr(val as *const c_char);
            let val = val.to_str()?.to_string();

            Ok(val)
        }
    }

    pub fn add_gps_info(&self, gps_info: GpsInfo) -> Result<()> {
        unsafe {
            let rc = exif_metadata_add_gps_info(self.raw, gps_info.lat, gps_info.lon, gps_info.lon);

            if rc != 0 {
                Err(anyhow!("Failed to add gps info"))
            }  else {
                Ok(())
            }
        }
    }

    pub fn paste_to_blob(&self, blob: &Vec<u8>) -> Result<Vec<u8>> {
        unsafe {
            let mut out_blob: *mut u8 = std::ptr::null_mut();

            let new_len = exif_metadata_save_blob(self.raw,
                                                  blob.as_ptr(), blob.len(),
                                                  &mut out_blob);
            if new_len < 1 {
                Err(anyhow!("Failed to paste metadata to blob"))
            } else {
                Ok(Vec::from_raw_parts(out_blob, new_len, new_len))
            }
        }
    }
}

pub struct GpsInfo {
    pub lat: f64,
    pub lon: f64,
    pub alt: f64,
}
