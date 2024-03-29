use std::ffi::{c_void, CString};
use std::slice;
use anyhow::{anyhow, Result};
use libheif_rs::{Channel, ColorSpace, CompressionFormat, EncoderQuality, HeifContext, Image, LibHeif, RgbChroma};
use magick_rust::{bindings, MagickWand};

pub fn encode(wand: &mut MagickWand, quality: u8) -> Result<Vec<u8>> {
    let width = wand.get_image_width();
    let height = wand.get_image_height();

    // get image profiles
    let exif_profile = get_image_profile(wand, "exif");
    let xmp_profile = get_image_profile(wand, "xmp");

    // make blob
    let blob = match wand.export_image_pixels(0, 0, width, height, "RGB") {
        Some(rgb) => rgb,
        None => return Err(anyhow!("Failed to export image pixels"))
    };

    // make image to encode
    let width = width as u32;
    let height = height as u32;

    let mut image = Image::new(width, height, ColorSpace::Rgb(RgbChroma::Rgb))?;
    image.create_plane(Channel::Interleaved, width, height, 24)?;

    let planes = image.planes_mut();
    let interleaved_plane = planes.interleaved.unwrap();
    let data = interleaved_plane.data;
    let stride = interleaved_plane.stride;

    // fill image pixels
    let blob_slice = blob.as_slice();

    let width = width as usize;
    for y in 0..height as usize{
        let x0_for_blob = y * width * 3;
        let x0_for_data = y * stride;

        data[x0_for_data..x0_for_data+width*3].clone_from_slice(&blob_slice[x0_for_blob..x0_for_blob +width*3])
    }

    // encode image
    let lib_heif = LibHeif::new();
    let mut context = HeifContext::new()?;
    let mut encoder = lib_heif.encoder_for_format(CompressionFormat::Hevc)?;
    encoder.set_quality(EncoderQuality::Lossy(interpolate_quality(quality)))?;
    let handle = context.encode_image(&image, &mut encoder, None)?;

    // add metadata
    if let Some(exif) = exif_profile {
        context.add_exif_metadata(&handle, &exif)?;
    }

    if let Some(xmp) = xmp_profile {
        context.add_xmp_metadata(&handle, &xmp)?;
    }

    context.write_to_bytes()
        .map_err(|e| anyhow!("Failed to write to bytes: {}", e))
}

fn interpolate_quality(quality: u8) -> u8 {
    let biases = vec![
        (70, 50.),
        (85, 60.),
        (92, 70.),
        (95, 80.),
        (100, 100.),
    ];

    let tolerance = 0.1;

    let mut lower: Option<(i32, f32)> = None;
    let mut upper: Option<(i32, f32)> = None;

    for (i, &(q, biased)) in biases.iter().enumerate() {
        if q == quality {
            return biased as u8;
        } else if q < quality {
            lower = Some((i as i32, biased));
        } else if q > quality {
            upper = Some((i as i32, biased));
            break;
        }
    }

    match (lower, upper) {
        (Some((lower_idx, lower_size)), Some((upper_idx, upper_size))) => {
            let lower_quality = biases[lower_idx as usize].0 as f32;
            let upper_quality = biases[upper_idx as usize].0 as f32;
            let quality_ratio = (quality as f32 - lower_quality) / (upper_quality - lower_quality);
            let interpolated_size = lower_size + quality_ratio * (upper_size - lower_size);

            if (interpolated_size - lower_size).abs() < tolerance {
                return lower_size as u8;
            } else if (interpolated_size - upper_size).abs() < tolerance {
                return upper_size as u8;
            } else {
                return interpolated_size as u8;
            }
        }
        (Some((_lower_idx, lower_size)), None) => {
            return lower_size as u8;
        }
        (None, Some((_upper_idx, upper_size))) => {
            return upper_size as u8;
        }
        _ => {
            // never reached
            panic!("Unable to interpolate file size")
        },
    }
}

fn get_image_profile<T: AsRef<str>>(wand: &mut MagickWand, name: T) -> Option<Vec<u8>> {
    let c_name = CString::new(name.as_ref()).unwrap();
    let mut n = 0;

    let out_blob = unsafe { bindings::MagickGetImageProfile(wand.wand, c_name.as_ptr(), &mut n) };

    let value = unsafe {
        if out_blob.is_null() {
            None
        } else {
            let slice = slice::from_raw_parts(out_blob as *const u8, n);
            Some(slice.to_vec())
        }
    };

    unsafe {
        bindings::MagickRelinquishMemory(out_blob as *mut c_void);
    }

    value
}
