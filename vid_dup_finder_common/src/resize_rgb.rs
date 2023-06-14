use image::RgbImage;

#[must_use]

pub fn resize_img_rgb(frame: &RgbImage, new_width: u32, new_height: u32) -> RgbImage {
    #[must_use]
    #[cfg(feature = "resize_fast")]
    fn resize_frame_fast(src_frame_img: &RgbImage, new_width: u32, new_height: u32) -> RgbImage {
        use fast_image_resize as fr;
        use fr::DynamicImageView::U8x3 as DynView;
        use std::num::NonZeroU32;

        let new_width = NonZeroU32::try_from(new_width).unwrap();
        let new_height = NonZeroU32::try_from(new_height).unwrap();

        let src_frame_fr = fr::ImageView::from_buffer(
            src_frame_img.width().try_into().unwrap(),
            src_frame_img.height().try_into().unwrap(),
            src_frame_img.as_raw(),
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

    #[must_use]
    #[cfg(not(feature = "resize_fast"))]
    fn resize_frame_image(frame: &RgbImage, new_width: u32, new_height: u32) -> RgbImage {
        image::imageops::resize(
            frame,
            new_width,
            new_height,
            image::imageops::FilterType::Triangle,
        )
    }

    #[cfg(feature = "resize_fast")]
    return resize_frame_fast(frame, new_width, new_height);

    #[cfg(not(feature = "resize_fast"))]
    return resize_frame_image(frame, new_width, new_height);
}
