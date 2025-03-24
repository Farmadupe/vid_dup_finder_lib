use std::{borrow::Borrow, num::NonZeroU32, ops::Deref};

use fast_image_resize::images::ImageRef;
use image::{DynamicImage, GrayImage, ImageBuffer, Luma};

use crate::Crop;

use fast_image_resize::{self as fr, Resizer};

#[must_use]
pub fn crop_resize_buf<I, C>(
    src_frame: I,
    new_width: NonZeroU32,
    new_height: NonZeroU32,
    crop: Crop,
) -> GrayImage
where
    I: Borrow<ImageBuffer<Luma<u8>, C>>,
    C: Deref<Target = [u8]>,
{
    let src_frame = src_frame.borrow();

    let src_ref = ImageRef::new(
        src_frame.width(),
        src_frame.height(),
        src_frame.deref(),
        fast_image_resize::PixelType::U8,
    )
    .unwrap();

    let mut dst_image =
        DynamicImage::ImageLuma8(GrayImage::new(new_width.into(), new_height.into()));

    let mut resizer = Resizer::new();
    let (left, top, width, height) = crop.as_view_args();
    resizer
        .resize(
            &src_ref,
            &mut dst_image,
            Some(&fr::ResizeOptions::new().crop(
                left as f64,
                top as f64,
                width as f64,
                height as f64,
            )),
        )
        .unwrap();

    let DynamicImage::ImageLuma8(dst_image) = dst_image else {
        unreachable!()
    };

    dst_image
}

#[must_use]
pub fn resize_frame<I, C>(frame: I, new_width: NonZeroU32, new_height: NonZeroU32) -> GrayImage
where
    I: Borrow<ImageBuffer<Luma<u8>, C>>,
    C: Deref<Target = [u8]>,
    C: AsRef<[u8]>,
{
    let frame = frame.borrow();

    crop_resize_buf(
        frame,
        new_width,
        new_height,
        Crop::from_edge_offsets(frame.dimensions(), 0, 0, 0, 0),
    )
}
