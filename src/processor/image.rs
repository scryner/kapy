use anyhow::{Result, anyhow};
use magick_rust::{MagickWand, bindings};

use crate::config::{Command, Quality, Resize};

pub fn process_command(wand: &mut MagickWand, cmd: &Command) -> Result<()> {
    match cmd {
        Command::ByPass => return Ok(()),
        Command::Convert {
            resize, format: _, quality
        } => {
            let mut width = 0;
            let mut height = 0;
            let mut need_to_resize = false;

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
                }
                Quality::Preserve => ()
            }
        }
    }

    Ok(())
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
