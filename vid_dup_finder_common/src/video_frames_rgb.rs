use std::num::NonZeroU32;

use image::{GenericImageView, RgbImage};

use crate::{resize_rgb::resize_img_rgb, video_frames_gray::RgbImageAsGray, Crop};

pub struct FrameSeqRgb {
    frames: Vec<RgbImageAsGray>,
}

impl FrameSeqRgb {
    pub fn from_images(images: impl IntoIterator<Item = RgbImage>) -> Option<Self> {
        let img_vec = images.into_iter().map(RgbImageAsGray).collect::<Vec<_>>();

        if img_vec.is_empty() {
            return None;
        }

        Some(Self { frames: img_vec })
    }

    #[must_use]
    pub fn into_inner(self) -> Vec<RgbImage> {
        self.frames.into_iter().map(|x| x.0).collect::<Vec<_>>()
    }

    #[must_use]
    pub fn crop(&self, crop: Crop) -> Self {
        let new_frames = self
            .frames
            .iter()
            .map(|img| {
                let (x, y, w, h) = crop.as_view_args();
                img.0.view(x, y, w, h).to_image()
            })
            .map(RgbImageAsGray)
            .collect();

        Self { frames: new_frames }
    }

    #[must_use]
    pub fn resize(&self, new_width: NonZeroU32, new_height: NonZeroU32) -> Self {
        let resized_frames = self
            .frames
            .iter()
            .map(|frame| resize_img_rgb(&frame.0, new_width, new_height))
            .map(RgbImageAsGray)
            .collect();

        Self {
            frames: resized_frames,
        }
    }
}
