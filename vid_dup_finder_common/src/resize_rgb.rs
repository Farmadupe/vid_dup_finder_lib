use fast_image_resize::{images::ImageRef, Resizer};
use image::{DynamicImage, RgbImage};

use std::{num::NonZeroU32, ops::Deref};

#[must_use]
pub fn resize_img_rgb(frame: &RgbImage, new_width: NonZeroU32, new_height: NonZeroU32) -> RgbImage {
    let frame = ImageRef::new(
        frame.width(),
        frame.height(),
        frame.deref(),
        fast_image_resize::PixelType::U8x3,
    )
    .unwrap();

    let mut dst_image = DynamicImage::ImageRgb8(RgbImage::new(new_width.into(), new_height.into()));

    let mut resizer = Resizer::new();

    resizer.resize(&frame, &mut dst_image, None).unwrap();

    let DynamicImage::ImageRgb8(dst_image) = dst_image else {
        unreachable!()
    };

    dst_image
}
