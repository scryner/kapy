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
    encoder.set_quality(EncoderQuality::Lossy(quality))?;
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
