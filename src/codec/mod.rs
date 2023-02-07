use std::path::Path;

use anyhow::Result;

mod jpeg;
mod heic;
mod gpx;
mod exif;

pub struct Image {}

pub trait ImageCodec {
    fn decode(path: &Path) -> Result<Image>;
    fn encode(image: &Image) -> Result<()>;
}
