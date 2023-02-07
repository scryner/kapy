use std::path::Path;
use crate::codec::{Image, ImageCodec};

pub struct JPEG;

impl ImageCodec for JPEG {
    fn decode(path: &Path) -> anyhow::Result<Image> {
        todo!()
    }

    fn encode(image: &Image) -> anyhow::Result<()> {
        todo!()
    }
}
