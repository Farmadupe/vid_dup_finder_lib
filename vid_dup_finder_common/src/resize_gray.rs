use std::{borrow::Borrow, ops::Deref};

use image::{FlatSamples, GrayImage, ImageBuffer, Luma};

#[must_use]
pub fn crop_resize_buf<I, C>(
    src_frame: I,
    new_width: u32,
    new_height: u32,
    crop: (u32, u32, u32, u32),
) -> GrayImage
where
    I: Borrow<ImageBuffer<Luma<u8>, C>>,
    C: Deref<Target = [u8]>,
{
    let src_frame = src_frame.borrow();

    let (left, top, width, height) = crop;

    use fast_image_resize as fr;

    use fr::CropBox;
    use fr::DynamicImageView::U8 as DynView;
    use std::num::NonZeroU32;

    let new_width = NonZeroU32::try_from(new_width).unwrap();
    let new_height = NonZeroU32::try_from(new_height).unwrap();

    let mut src_frame_fr = fr::ImageView::from_buffer(
        src_frame.width().try_into().unwrap(),
        src_frame.height().try_into().unwrap(),
        src_frame,
    )
    .unwrap();

    src_frame_fr
        .set_crop_box(CropBox {
            left,
            top,
            width: NonZeroU32::try_from(width).unwrap(),
            height: NonZeroU32::try_from(height).unwrap(),
        })
        .unwrap();

    let mut dst_frame_buf =
        vec![0u8; u32::from(new_height) as usize * u32::from(new_height) as usize];
    let mut dst_frame_fr = fr::Image::from_slice_u8(
        new_width,
        new_height,
        &mut dst_frame_buf,
        src_frame_fr.pixel_type(),
    )
    .unwrap();

    let mut resizer = fr::Resizer::new(fr::ResizeAlg::Convolution(fr::FilterType::Lanczos3));

    resizer
        .resize(&DynView(src_frame_fr), &mut dst_frame_fr.view_mut())
        .unwrap();

    std::mem::drop(dst_frame_fr);
    let dst_frame_img = GrayImage::from_vec(new_width.into(), new_height.into(), dst_frame_buf);

    dst_frame_img.unwrap()
}

#[must_use]
pub fn crop_resize_flat<C>(
    src_frame: FlatSamples<C>,
    new_width: u32,
    new_height: u32,
    crop: (u32, u32, u32, u32),
) -> GrayImage
where
    C: AsRef<[u8]>,
{
    let (left, top, width, height) = crop;

    //make sure crop frame fits the image
    let max_width = src_frame.layout.width - left - 1;
    let width = max_width.min(width);

    let max_height = src_frame.layout.height - top - 1;
    let height = max_height.min(height);

    // let src_width = src_frame.width();
    // let src_height = src_frame.height();

    //println!("src_dimensions: {src_width:?}x{src_height:?}, src_crop: (left: {left}, top: {top}, width: {width}, height: {height}), new_width: {new_width}, new_height: {new_height}");

    use fast_image_resize as fr;

    use fr::CropBox;
    use std::num::NonZeroU32;

    let new_width = NonZeroU32::try_from(new_width).unwrap();
    let new_height = NonZeroU32::try_from(new_height).unwrap();

    let src_frame_raw: &[u8] = src_frame.as_slice();
    let old_width = src_frame.layout.width as usize;
    let old_height_stride = src_frame.layout.height_stride;
    let rows = src_frame_raw
        .chunks_exact(old_height_stride)
        .map(|chunk| {
            let row_slice = &chunk[0..old_width];
            unsafe { std::mem::transmute(row_slice) }
        })
        .collect::<Vec<_>>();

    let mut src_frame_fr = fr::ImageView::new(
        src_frame.layout.width.try_into().unwrap(),
        src_frame.layout.height.try_into().unwrap(),
        rows,
    )
    .unwrap();

    {
        let crop_box = CropBox {
            left,
            top,
            width: NonZeroU32::try_from(width).unwrap(),
            height: NonZeroU32::try_from(height).unwrap(),
        };
        if let Err(_e) = src_frame_fr.set_crop_box(crop_box) {
            // println!("{crop_box:#?}");
            // println!("{e:#?}");
        }
    }

    let mut dst_frame_buf =
        vec![0u8; u32::from(new_height) as usize * u32::from(new_height) as usize];
    let mut dst_frame_fr = fr::Image::from_slice_u8(
        new_width,
        new_height,
        &mut dst_frame_buf,
        src_frame_fr.pixel_type(),
    )
    .unwrap();

    let mut resizer = fr::Resizer::new(fr::ResizeAlg::Convolution(fr::FilterType::Lanczos3));

    resizer
        .resize(
            &fr::DynamicImageView::U8(src_frame_fr),
            &mut dst_frame_fr.view_mut(),
        )
        .unwrap();

    std::mem::drop(dst_frame_fr);
    let dst_frame_img = GrayImage::from_vec(new_width.into(), new_height.into(), dst_frame_buf);

    dst_frame_img.unwrap()
}

#[must_use]
pub fn resize_frame<I, C>(frame: I, new_width: u32, new_height: u32) -> GrayImage
where
    I: Borrow<ImageBuffer<Luma<u8>, C>>,
    C: Deref<Target = [u8]>,
{
    #[cfg(feature = "resize_fast")]
    fn resize_frame_fast<I, C>(src_frame_img: I, new_width: u32, new_height: u32) -> GrayImage
    where
        I: Borrow<ImageBuffer<Luma<u8>, C>>,
        C: Deref<Target = [u8]>,
    {
        use fast_image_resize as fr;
        use fr::DynamicImageView::U8 as DynView;
        use std::num::NonZeroU32;

        let src_frame_img = src_frame_img.borrow();

        let new_width = NonZeroU32::try_from(new_width).unwrap();
        let new_height = NonZeroU32::try_from(new_height).unwrap();

        let src_frame_fr = fr::ImageView::from_buffer(
            src_frame_img.width().try_into().unwrap(),
            src_frame_img.height().try_into().unwrap(),
            src_frame_img.as_raw(),
        )
        .unwrap();

        let mut dst_frame_buf =
            vec![0u8; u32::from(new_height) as usize * u32::from(new_height) as usize];
        let mut dst_frame_fr = fr::Image::from_slice_u8(
            new_width,
            new_height,
            &mut dst_frame_buf,
            src_frame_fr.pixel_type(),
        )
        .unwrap();

        let mut resizer = fr::Resizer::new(fr::ResizeAlg::Convolution(fr::FilterType::Lanczos3));

        resizer
            .resize(&DynView(src_frame_fr), &mut dst_frame_fr.view_mut())
            .unwrap();

        std::mem::drop(dst_frame_fr);
        let dst_frame_img = GrayImage::from_vec(new_width.into(), new_height.into(), dst_frame_buf);

        dst_frame_img.unwrap()
    }

    #[must_use]
    #[cfg(not(feature = "resize_fast"))]
    fn resize_frame_image<I, C>(frame: I, new_width: u32, new_height: u32) -> GrayImage
    where
        I: Borrow<ImageBuffer<Luma<u8>, C>>,
        C: Deref<Target = [u8]>,
    {
        image::imageops::resize(
            frame.borrow(),
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
