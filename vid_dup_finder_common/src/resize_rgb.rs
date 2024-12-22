use image::RgbImage;

use fast_image_resize as fr;
use fr::DynamicImageView::U8x3 as DynView;
use std::num::NonZeroU32;

#[must_use]
pub fn resize_img_rgb(frame: &RgbImage, new_width: NonZeroU32, new_height: NonZeroU32) -> RgbImage {
    let src_frame_fr = fr::ImageView::from_buffer(
        frame.width().try_into().unwrap(),
        frame.height().try_into().unwrap(),
        frame.as_raw(),
    )
    .unwrap();

    let mut dst_frame_buf =
        vec![0u8; 3usize * u32::from(new_height) as usize * u32::from(new_height) as usize];
    let dst_frame_fr =
        fr::ImageViewMut::from_buffer(new_width, new_height, &mut dst_frame_buf).unwrap();

    let mut resizer = fr::Resizer::new(fr::ResizeAlg::Convolution(fr::FilterType::Lanczos3));

    resizer
        .resize(
            &DynView(src_frame_fr),
            &mut fr::DynamicImageViewMut::U8x3(dst_frame_fr),
        )
        .unwrap();

    let dst_frame_img = RgbImage::from_vec(new_width.into(), new_height.into(), dst_frame_buf);

    dst_frame_img.unwrap()
}
