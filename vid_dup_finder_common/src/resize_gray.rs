use std::{borrow::Borrow, num::NonZeroU32, ops::Deref};

use image::{FlatSamples, GrayImage, ImageBuffer, Luma};

use crate::Crop;

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

///Crop an image, then resize it.
#[must_use]
pub fn crop_resize_flat<C>(
    src_frame: FlatSamples<C>,
    new_width: NonZeroU32,
    new_height: NonZeroU32,
    crop: Crop,
) -> Option<GrayImage>
where
    C: AsRef<[u8]>,
{
    //easier to not consider zero size images
    let src_frame_width = NonZeroU32::try_from(src_frame.layout.width).ok()?;
    let src_frame_height = NonZeroU32::try_from(src_frame.layout.height).ok()?;

    let (left, top, width, height) = crop.as_view_args();
    let width = NonZeroU32::try_from(width).ok()?;
    let height = NonZeroU32::try_from(height).ok()?;

    //make sure crop frame fits the image
    let max_width = u32::from(src_frame_width).saturating_sub(left);
    let width = NonZeroU32::try_from(max_width.min(u32::from(width))).ok()?;

    let max_height = u32::from(src_frame_height).saturating_sub(top);
    let height = NonZeroU32::try_from(max_height.min(u32::from(height))).ok()?;

    // let src_width = src_frame.width();
    // let src_height = src_frame.height();

    //println!("src_dimensions: {src_width:?}x{src_height:?}, src_crop: (left: {left}, top: {top}, width: {width}, height: {height}), new_width: {new_width}, new_height: {new_height}");

    use fast_image_resize as fr;

    use fr::CropBox;

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
        src_frame_width,
        src_frame_height,
        rows,
    )
    .unwrap();

    {
        // println!(
        //     "src_width: {}, src_height: {}, crop: {:?}",
        //     src_frame.layout.width, src_frame.layout.height, crop
        // );
        // println!("width: {width:?}, height: {height:?}");

        let crop_box = CropBox {
            left,
            top,
            width,
            height,
        };
        if let Err(_e) = src_frame_fr.set_crop_box(crop_box) {
            return None;
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

    dst_frame_img
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