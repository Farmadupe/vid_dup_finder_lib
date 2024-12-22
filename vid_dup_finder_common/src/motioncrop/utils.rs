use image::{GenericImageView, GrayImage, Luma, Rgb, RgbImage};
use imageproc::definitions::Image;
use itertools::Itertools;

use crate::Crop;

pub(super) fn regionize_image(img: &GrayImage) -> (Image<Luma<u32>>, usize) {
    use imageproc::region_labelling::Connectivity::Eight;
    let fg = imageproc::region_labelling::connected_components(img, Eight, image::Luma([0]));

    let num_regions = fg.pixels().unique().count() - 1;

    (fg, num_regions)
}

pub(super) fn maskize_regions(regions: &Image<Luma<u32>>) -> GrayImage {
    let mut ret = GrayImage::new(regions.width(), regions.height());

    for (&mut Luma([ref mut ret_pix]), &Luma([ref region_pix])) in
        ret.pixels_mut().zip(regions.pixels())
    {
        if *region_pix != 0 {
            *ret_pix = 255;
        } else {
            *ret_pix = 0;
        }
    }

    ret
}

pub(super) fn regions_in_mask(img: &Image<Luma<u32>>, mask: &GrayImage) -> Vec<u32> {
    assert!(img.dimensions() == mask.dimensions());
    let mut ret = vec![];
    for (Luma([img_pix]), Luma([mask_pix])) in img.pixels().zip(mask.pixels()) {
        if *mask_pix == 255 && !ret.contains(img_pix) {
            ret.push(*img_pix);
        }
    }

    ret
}

pub(super) fn retain_regions(img: &Image<Luma<u32>>, regions: &[u32]) -> Image<Luma<u32>> {
    let mut ret = img.clone();
    for &mut Luma([ref mut pix]) in ret.pixels_mut() {
        if !regions.contains(pix) {
            *pix = 0;
        }
    }

    ret
}

pub(super) fn largest_region(img: &Image<Luma<u32>>) -> Option<u32> {
    let mut acc: Vec<usize> = vec![];
    for Luma([pix]) in img.pixels() {
        if *pix == 0 {
            continue;
        }
        acc.resize_with((*pix as usize + 1).max(acc.len()), || 0);
        acc[*pix as usize] += 1;
    }

    acc.iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.cmp(b))
        .map(|(index, _)| index as u32)
}

pub fn view_mask(img: &GrayImage) -> Option<image::SubImage<&GrayImage>> {
    let masked_pixels = img
        .enumerate_pixels()
        .filter(|(_x, _y, Luma([pix]))| *pix == 255);

    let (Some(min_x), Some(min_y), Some(max_x), Some(max_y)) = masked_pixels.fold(
        (None, None, None, None),
        |(acc_min_x, acc_min_y, acc_max_x, acc_max_y), (x, y, Luma([_pix]))| {
            //println!("{y}");
            (
                acc_min_x.map_or(Some(x), |a| Some(x.min(a))),
                acc_min_y.map_or(Some(y), |a| Some(y.min(a))),
                acc_max_x.map_or(Some(x), |a| Some(x.max(a))),
                acc_max_y.map_or(Some(y), |a| Some(y.max(a))),
            )
        },
    ) else {
        return None;
    };

    //dbg!((min_x, min_y, max_x, max_y));

    let width = (max_x - min_x) + 1;
    let height = (max_y - min_y) + 1;

    let ret = img.view(min_x, min_y, width, height);
    Some(ret)
}

// pub(super) fn view_letterbox(img: &mut GrayImage, crop: Crop) -> image::SubImage<&mut GrayImage> {
//     let (x, y, width, height) = crop.as_view_args();
//     img.sub_image(x, y, width, height)
// }

pub(super) fn boolean_and(img_1: &GrayImage, img_2: &GrayImage) -> GrayImage {
    assert!(img_1.dimensions() == img_2.dimensions());
    let mut ret = GrayImage::new(img_1.width(), img_1.height());

    for ((&mut Luma([ref mut ret_pix]), &Luma([ref img_1_pix])), &Luma([ref img_2_pix])) in
        ret.pixels_mut().zip(img_1.pixels()).zip(img_2.pixels())
    {
        if *img_1_pix == 255 && *img_2_pix == 255 {
            *ret_pix = 255;
        } else {
            *ret_pix = 0;
        }
    }

    ret
}

////////////////////////////////////////////////////////////////////////////////////
pub(super) fn clear_out_cropped_area(img: &mut GrayImage, crop: Crop) {
    for (x, y) in crop.enumerate_coords() {
        img.put_pixel(x, y, Luma([255]));
    }
}

#[allow(dead_code)]
pub(super) enum RgbChan {
    Red,
    Green,
    Blue,
}
pub(super) fn tint_cropped_area(img: &RgbImage, crop: Crop, chan: RgbChan) -> RgbImage {
    let mut ret = img.clone();
    let (x, y, width, height) = crop.as_view_args();
    let view = img.view(x, y, width, height);

    for (x, y, _pix) in view.pixels() {
        let &mut Rgb([ref mut r, ref mut g, ref mut b]) =
            ret.get_pixel_mut(view.offsets().0 + x, view.offsets().1 + y);
        match chan {
            RgbChan::Red => *r = 255,
            RgbChan::Green => *g = 255,
            RgbChan::Blue => *b = 255,
        }
    }
    ret
}

pub(super) fn colourize_regions(img: &Image<Luma<u32>>) -> RgbImage {
    let colours = [
        Rgb::<u8>([0, 0, 255]),
        Rgb::<u8>([255, 0, 255]),
        Rgb::<u8>([128, 128, 128]),
        Rgb::<u8>([0, 128, 0]),
        Rgb::<u8>([0, 255, 0]),
        Rgb::<u8>([128, 0, 0]),
        Rgb::<u8>([0, 0, 128]),
        Rgb::<u8>([128, 128, 0]),
        Rgb::<u8>([128, 0, 128]),
        Rgb::<u8>([255, 0, 0]),
        Rgb::<u8>([192, 192, 192]),
        Rgb::<u8>([0, 128, 128]),
        Rgb::<u8>([255, 255, 0]),
    ];

    let mut ret = RgbImage::new(img.width(), img.height());

    for (Luma([region_pix]), ret_pix) in img.pixels().zip(ret.pixels_mut()) {
        if *region_pix != 0 {
            *ret_pix = *colours.get(*region_pix as usize % colours.len()).unwrap();
        }
    }

    ret
}

// pub(super) fn into_gray_image<V>(img: impl Deref<Target = V>) -> GrayImage
// where
//     V: GenericImageView<Pixel = Luma<u8>>,
// {
//     let mut ret = GrayImage::new(img.width(), img.height());
//     for (Luma([ref mut dst_pix]), (_x, _y, Luma([src_pix]))) in ret.pixels_mut().zip(img.pixels()) {
//         *dst_pix = src_pix;
//     }

//     ret
// }
