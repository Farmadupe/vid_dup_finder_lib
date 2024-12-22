use std::borrow::Borrow;

use image::{GenericImage, GenericImageView, RgbImage};

/// Given a two-dimensional array of images, arrange them all in a grid.
/// All images must share the same dimensions.
/// Returns None if there are no images.
///
/// Panics if the images are not all the same dimensions.
#[must_use]
pub fn grid_images_rgb(images: &[&[RgbImage]]) -> Option<RgbImage> {
    //Check that all image dimensions are equal to the dimensions of the first
    //image.
    let mut all_img_dimensions = images
        .iter()
        .flat_map(|row| row.iter().map(RgbImage::dimensions));
    let (img_x, img_y) = all_img_dimensions.next()?;

    assert!(all_img_dimensions
        .all(|(curr_img_x, curr_img_y)| curr_img_x == img_x && curr_img_y == img_y));

    //work out how wide and how deep the output buffer needs to be
    let grid_num_x = images
        .iter()
        .map(|i| u32::try_from(i.len()).expect("unreachable"))
        .max()?;
    let grid_num_y = u32::try_from(images.len()).expect("unreachable");

    //create the output buffer and fill it.
    let mut grid_buf = RgbImage::new(grid_num_x * img_x, grid_num_y * img_y);
    for (col_no, row_imgs) in images.iter().enumerate() {
        for (row_no, img) in row_imgs.iter().enumerate() {
            let x_coord = u32::try_from(row_no).expect("unreachable") * img_x;
            let y_coord = u32::try_from(col_no).expect("unreachable") * img_y;
            grid_buf
                .copy_from(img as &RgbImage, x_coord, y_coord)
                .expect("unreachable due to above assertion about image dimensions");
        }
    }

    Some(grid_buf)
}

///Arrange a sequence of images side by side in a row.
///The images must all be the same size.
///
/// Returns None if there are no images
/// Panics if the images are not all the same size
pub fn row_images<'a, ExactIter, View, Pixel, Subpix>(
    images: ExactIter,
) -> Option<image::ImageBuffer<Pixel, Vec<Subpix>>>
where
    ExactIter: ExactSizeIterator<Item = &'a View>,
    View: GenericImageView<Pixel = Pixel> + 'a,
    Pixel: image::Pixel<Subpixel = Subpix>,
{
    type RetBuf<Pixel, Subpix> = image::ImageBuffer<Pixel, Vec<Subpix>>;

    //get the number of images and their size. If dimensions is None
    //then there are no images, so return None as there is no work to do.
    let mut images = images.map(|x| x.borrow()).peekable();
    let (img_x, img_y) = images.peek().map(|x| x.dimensions())?;
    let len = u32::try_from(images.len()).expect("unreachable");

    let mut ret = RetBuf::new(len * img_x, img_y);

    for (col_no, img) in images.enumerate() {
        let x_coord = u32::try_from(col_no).expect("unreachable") * img_x;
        ret.copy_from(img, x_coord, 0).unwrap();
    }

    Some(ret)
}

///Arrange a sequence of images top to bottom.
///The images must all be the same size.
///
/// Returns None if there are no images
/// Panics if the images are not all the same size
pub fn stack_images<'a, ExactIter, View, Pixel, Subpix>(
    images: ExactIter,
) -> Option<image::ImageBuffer<Pixel, Vec<Subpix>>>
where
    ExactIter: ExactSizeIterator<Item = &'a View>,
    View: GenericImageView<Pixel = Pixel> + 'a,
    Pixel: image::Pixel<Subpixel = Subpix>,
{
    type RetBuf<Pixel, Subpix> = image::ImageBuffer<Pixel, Vec<Subpix>>;

    //get the number of images and their size. If dimensions is None
    //then there are no images, so return None as there is no work to do.
    let mut images = images.map(|img| img.borrow()).peekable();
    let (img_x, img_y) = images.peek().map(|img| img.dimensions())?;
    let len = u32::try_from(images.len()).expect("unreachable");

    let mut ret = RetBuf::new(img_x, len * img_y);

    for (row_no, img) in images.enumerate() {
        let y_coord = u32::try_from(row_no).expect("unreachable") * img_y;
        ret.copy_from(img, 0, y_coord).unwrap();
    }

    Some(ret)
}
