use anyhow::{anyhow, Result};
use imgref::ImgVec;
use ravif::{AlphaColorMode, ColorSpace, EncodedImage, Encoder, RGBA8};
use load_image::export::rgb::ComponentMap;

#[allow(unused_imports)]
use load_image::load_data;

#[allow(unused_imports)]
use rayon::prelude::*;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

pub fn encode(blob :Vec<u8>, quality: f32) -> Result<EncodedImage> {
    // transform blob
    let img = load_rgba(&blob, false)
        .map_err(|e| anyhow!("Failed to load rgb: {}", e))?;

    // refine encoder options
    let alpha_quality = ((quality + 100.)/2.).min(quality + quality/4. + 2.);
    let speed = 4_u8;

    // make encoder
    let enc = Encoder::new()
        .with_quality(quality)
        .with_speed(speed)
        .with_alpha_quality(alpha_quality)
        .with_internal_color_space(ColorSpace::YCbCr)
        .with_num_threads(None)
        .with_alpha_color_mode(AlphaColorMode::UnassociatedClean);

    // encode
    enc.encode_rgba(img.as_ref())
        .map_err(|e| anyhow!("Failed to encode to AVIF: {}", e))
}

#[cfg(not(feature = "cocoa_image"))]
fn load_rgba(data: &[u8], premultiplied_alpha: bool) -> Result<ImgVec<RGBA8>, BoxError> {

    let img = load_image::load_data(data)?.into_imgvec();
    let mut img = match img {
        load_image::export::imgref::ImgVecKind::RGB8(img) => img.map_buf(|buf| buf.into_iter().map(|px| px.alpha(255)).collect()),
        load_image::export::imgref::ImgVecKind::RGBA8(img) => img,
        load_image::export::imgref::ImgVecKind::RGB16(img) => img.map_buf(|buf| buf.into_iter().map(|px| px.map(|c| (c >> 8) as u8).alpha(255)).collect()),
        load_image::export::imgref::ImgVecKind::RGBA16(img) => img.map_buf(|buf| buf.into_iter().map(|px| px.map(|c| (c >> 8) as u8)).collect()),
        load_image::export::imgref::ImgVecKind::GRAY8(img) => img.map_buf(|buf| buf.into_iter().map(|g| { let c = g.0; RGBA8::new(c,c,c,255) }).collect()),
        load_image::export::imgref::ImgVecKind::GRAY16(img) => img.map_buf(|buf| buf.into_iter().map(|g| { let c = (g.0>>8) as u8; RGBA8::new(c,c,c,255) }).collect()),
        load_image::export::imgref::ImgVecKind::GRAYA8(img) => img.map_buf(|buf| buf.into_iter().map(|g| { let c = g.0; RGBA8::new(c,c,c,g.1) }).collect()),
        load_image::export::imgref::ImgVecKind::GRAYA16(img) => img.map_buf(|buf| buf.into_iter().map(|g| { let c = (g.0>>8) as u8; RGBA8::new(c,c,c,(g.1>>8) as u8) }).collect()),
    };

    if premultiplied_alpha {
        img.pixels_mut().for_each(|px| {
            px.r = (u16::from(px.r) * u16::from(px.a) / 255) as u8;
            px.g = (u16::from(px.g) * u16::from(px.a) / 255) as u8;
            px.b = (u16::from(px.b) * u16::from(px.a) / 255) as u8;
        });
    }
    Ok(img)
}

#[cfg(feature = "cocoa_image")]
fn load_rgba(data: &[u8], premultiplied_alpha: bool) -> Result<ImgVec<RGBA8>, BoxError> {
    if premultiplied_alpha {
        Ok(cocoa_image::decode_image_as_rgba_premultiplied(data)?)
    } else {
        Ok(cocoa_image::decode_image_as_rgba(data)?)
    }
}