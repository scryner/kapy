use std::path::Path;

use anyhow::Result;

pub mod resize;
pub mod format;
pub mod gps;

pub trait Process {
    fn process(&self, image: Image) -> Result<Image>;
}

pub enum Image {
    FilePath(Box<Path>),
}