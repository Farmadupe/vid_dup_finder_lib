use image::{GenericImageView, RgbImage};

use crate::{
    resize_rgb::resize_img_rgb,
    video_frames_gray::{RgbImageAsGray, VdfFrameSeqExt},
};

pub struct FrameSeqRgb {
    frames: Vec<RgbImageAsGray>,
}

impl VdfFrameSeqExt for FrameSeqRgb {
    type Item = RgbImageAsGray;

    fn frames(&self) -> &[Self::Item] {
        self.frames.as_slice()
    }
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
    pub fn crop(&self, (x, y, width, height): (u32, u32, u32, u32)) -> Self {
        let new_frames = self
            .frames
            .iter()
            .map(|img| img.0.view(x, y, width, height).to_image())
            .map(RgbImageAsGray)
            .collect();

        Self { frames: new_frames }
    }

    #[must_use]
    pub fn resize(&self, new_width: u32, new_height: u32) -> Self {
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
