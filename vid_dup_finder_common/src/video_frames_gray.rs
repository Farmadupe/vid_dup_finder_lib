use std::borrow::Borrow;

use image::{GenericImageView, GrayImage, ImageBuffer, Luma, SubImage};

use crate::crop::Crop;
use crate::crop_resize_buf;

pub type GrayImageRef<'a> = image::ImageBuffer<Luma<u8>, &'a [u8]>;

#[derive(Copy, Clone)]
pub enum LetterboxColour {
    _BlackWhite(u8),
    AnyColour(u8),
}

pub struct VideoFramesGray<T> {
    frames: Vec<T>,
}

impl<T> VideoFramesGray<T>
where
    T: GenericImageView<Pixel = Luma<u8>>,
{
    pub fn from_images(images: impl IntoIterator<Item = T>) -> Option<Self> {
        let img_vec = images.into_iter().collect::<Vec<_>>();
        if img_vec.is_empty() {
            return None;
        }

        Some(Self { frames: img_vec })
    }

    #[must_use]
    pub fn into_inner(self) -> Vec<T> {
        self.frames
    }
}

impl<'a> VideoFramesGray<SubImageGrayFlatNT<'a>> {}

impl<I> VideoFramesGray<I>
where
    I: Borrow<ImageBuffer<Luma<u8>, Vec<u8>>>,
{
    #[must_use]
    pub fn crop_resize_owned(
        &self,
        new_width: u32,
        new_height: u32,
        crop: (u32, u32, u32, u32),
    ) -> VideoFramesGray<GrayImage> {
        let new_frames = self
            .frames
            .iter()
            .map(|frame| crop_resize_buf(frame.borrow(), new_width, new_height, crop))
            .collect();

        VideoFramesGray { frames: new_frames }
    }
}

impl<'a, I> VideoFramesGray<I>
where
    I: Borrow<ImageBuffer<Luma<u8>, &'a [u8]>>,
{
    #[must_use]
    pub fn crop_resize_borrowed(
        &self,
        new_width: u32,
        new_height: u32,
        crop: (u32, u32, u32, u32),
    ) -> VideoFramesGray<GrayImage> {
        let new_frames = self
            .frames
            .iter()
            .map(|frame| crop_resize_buf(frame.borrow(), new_width, new_height, crop))
            .collect();

        VideoFramesGray { frames: new_frames }
    }
}

pub trait VdfFrameExt {
    type Item: image::GenericImageView<Pixel = image::Luma<u8>>;

    fn frame(&self) -> &Self::Item;

    #[must_use]
    //detect the letterbox of a single video frame
    fn letterbox_crop(&self, colour: LetterboxColour) -> Crop {
        let frame = self.frame();
        enum Side {
            Left,
            Right,
            Top,
            Bottom,
        }
        use Side::*;

        let (width, height) = frame.dimensions();
        let measure_side = |side: Side| -> u32 {
            //get the window of pixels representing the next row/column to be checked
            let pixel_window = |idx: u32| {
                #[rustfmt::skip]
                let ret = match side {
                    //                   x                y                 width  height
                    Left   => frame.view(idx,             0,                1,     height),
                    Right  => frame.view(width - idx - 1, 0,                1,     height),
                    Top    => frame.view(0,               idx,              width, 1),
                    Bottom => frame.view(0,               height - idx - 1, width, 1),
                };
                ret
            };

            let is_letterbox = |strip: &SubImage<&Self::Item>| {
                use LetterboxColour::*;
                match colour {
                    _BlackWhite(tol) => strip.pixels().all(|(_x, _y, image::Luma::<u8>([l]))| {
                        let black_enough = l <= tol;
                        let white_enough = l >= (u8::MAX - tol);
                        black_enough || white_enough
                    }),
                    AnyColour(tol) => {
                        //calculate range
                        let mut min_l = u8::MAX;
                        let mut max_l = u8::MIN;

                        for (_x, _y, image::Luma::<u8>([l])) in strip.pixels() {
                            min_l = min_l.min(l);
                            max_l = max_l.max(l);
                        }
                        let range_l = max_l.saturating_sub(min_l);

                        range_l <= tol
                    }
                }
            };

            let pix_range = match side {
                Left | Right => 0..width,
                Top | Bottom => 0..height,
            };

            pix_range
                .map(pixel_window)
                .take_while(|x| is_letterbox(x))
                .count() as u32
        };

        Crop::new(
            (width, height),
            measure_side(Left),
            measure_side(Right),
            measure_side(Top),
            measure_side(Bottom),
        )
    }
}

impl<T> VdfFrameExt for T
where
    T: GenericImageView<Pixel = Luma<u8>>,
{
    type Item = T;

    fn frame(&self) -> &Self::Item {
        self
    }
}

pub trait VdfFrameSeqExt {
    type Item: image::GenericImageView<Pixel = image::Luma<u8>> + Clone;

    fn frames(&self) -> &[Self::Item];

    fn letterbox_crop(&self) -> Crop {
        use LetterboxColour::*;
        let cfg: LetterboxColour = AnyColour(16);

        let crop = self
            .frames()
            .iter()
            .map(|frame| frame.letterbox_crop(cfg))
            .reduce(|x, y| x.union(&y))
            .unwrap();

        crop
    }
}

impl<T> VdfFrameSeqExt for VideoFramesGray<T>
where
    T: GenericImageView<Pixel = Luma<u8>> + Clone,
{
    type Item = T;

    fn frames(&self) -> &[Self::Item] {
        self.frames.as_slice()
    }
}

#[derive(Clone)]
pub struct RgbImageAsGray(pub image::RgbImage);
impl image::GenericImageView for RgbImageAsGray {
    type Pixel = image::Luma<u8>;

    fn dimensions(&self) -> (u32, u32) {
        self.0.dimensions()
    }

    fn bounds(&self) -> (u32, u32, u32, u32) {
        self.0.bounds()
    }

    fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
        use image::Pixel;
        self.0.get_pixel(x, y).to_luma()
    }
}

pub struct SubImageGrayNT<'a>(pub image::SubImage<&'a GrayImageRef<'a>>);
impl<'a> image::GenericImageView for SubImageGrayNT<'a> {
    type Pixel = image::Luma<u8>;

    fn dimensions(&self) -> (u32, u32) {
        self.0.dimensions()
    }

    fn bounds(&self) -> (u32, u32, u32, u32) {
        self.0.bounds()
    }

    fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
        self.0.get_pixel(x, y)
    }
}

pub struct SubImageGrayFlatNT<'a>(pub SubImage<&'a image::flat::View<&'a [u8], Luma<u8>>>);
impl<'a> image::GenericImageView for SubImageGrayFlatNT<'a> {
    type Pixel = image::Luma<u8>;

    fn dimensions(&self) -> (u32, u32) {
        self.0.dimensions()
    }

    fn bounds(&self) -> (u32, u32, u32, u32) {
        self.0.bounds()
    }

    fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
        self.0.get_pixel(x, y)
    }
}
