use image::{GenericImageView, GrayImage, Luma, SubImage};

use crate::{crop::Crop, motioncrop::autocrop_frames::MotiondetectCrop};

#[derive(Copy, Clone)]
pub enum LetterboxColour {
    BlackWhite(u8),
    AnyColour(u8),
}

pub struct VideoFramesGray {
    frames: Vec<GrayImage>,
}

impl VideoFramesGray {
    pub fn from_images(images: impl IntoIterator<Item = GrayImage>) -> Option<Self> {
        let img_vec = images.into_iter().collect::<Vec<_>>();
        if img_vec.is_empty() {
            return None;
        }

        Some(Self { frames: img_vec })
    }

    #[must_use]
    pub fn into_inner(self) -> Vec<GrayImage> {
        self.frames
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
                let min_proportion: f64 = 0.9;
                let matching_pixels = match colour {
                    BlackWhite(tol) => strip
                        .pixels()
                        .filter(|(_x, _y, Luma([l]))| {
                            let black_enough = *l <= tol;
                            let white_enough = *l >= (u8::MAX - tol);
                            black_enough || white_enough
                        })
                        .count(),
                    AnyColour(tol) => {
                        let mut mode_acc = [0usize; u8::MAX as usize + 1];

                        for (_x, _y, image::Luma::<u8>([l])) in strip.pixels() {
                            mode_acc[l as usize] += 1;
                        }

                        let mode = mode_acc
                            .iter()
                            .enumerate()
                            .max_by_key(|(_i, sum)| *sum)
                            .map(|(i, _sum)| i as u8)
                            .unwrap();

                        let count = strip
                            .pixels()
                            .filter(|(_x, _y, Luma([pix]))| mode.abs_diff(*pix) <= tol)
                            .count();

                        count
                    }
                };
                let proportion =
                    matching_pixels as f64 / (strip.dimensions().0 * strip.dimensions().1) as f64;

                proportion > min_proportion
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

        let l = measure_side(Left);
        let r = measure_side(Right);
        let t = measure_side(Top);
        let b = measure_side(Bottom);

        //sanity check -- make sure there is at least 1 pix in the horz and vert dimension
        let remaining_horz = (width as i32) - (l as i32) - (r as i32);
        let remaining_vert = (height as i32) - (t as i32) - (b as i32);

        if (remaining_horz >= 1) && (remaining_vert >= 1) {
            Crop::from_edge_offsets((width, height), l, r, t, b)
        } else {
            Crop::from_edge_offsets((width, height), 0, 0, 0, 0)
        }
    }

    fn cropped(&self, crop: Crop) -> image::SubImage<&Self::Item> {
        let (x, y, w, h) = crop.as_view_args();
        assert!(self.frame().dimensions() == crop.orig_res);
        self.frame().view(x, y, w, h)
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
    fn frames(&self) -> &[GrayImage];

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

    fn motiondetect_crop(&self) -> Crop {
        if let Some(ret) = MotiondetectCrop::from_frames(self.frames()) {
            ret
        } else {
            self.letterbox_crop()
        }
    }
}

impl VdfFrameSeqExt for VideoFramesGray {
    fn frames(&self) -> &[GrayImage] {
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

    fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
        use image::Pixel;
        self.0.get_pixel(x, y).to_luma()
    }
}

pub fn cropdetect_none(frames: &[GrayImage]) -> Option<Crop> {
    let dimensions = frames.iter().next().map(|f| f.dimensions())?;

    Some(Crop::from_edge_offsets(dimensions, 0, 0, 0, 0))
}

pub fn cropdetect_letterbox(frames: &[GrayImage]) -> Option<Crop> {
    // we don't need all of the frames to detect the crop. (this isn't a huge speedup because gstreamer still
    // decodes every frame)
    let frames = frames.iter().step_by(8).take(8);

    let ret = frames
        .map(|f| f.letterbox_crop(LetterboxColour::AnyColour(16)))
        .reduce(|a, b| a.union(&b))?;
    Some(ret)
}

pub fn cropdetect_motion(frames: &[GrayImage]) -> Option<Crop> {
    MotiondetectCrop::from_frames(frames)
}

#[cfg(test)]
mod test {
    use image::GrayImage;

    use crate::Crop;

    use super::VdfFrameExt;

    //Should find no crop, as the letterboxes will converge
    #[test]
    fn test_letterbox_crop_white_img_finds_no_crop() {
        #[rustfmt::skip]
        let pixs = vec![
            255, 255, 255,
            255, 255, 255,
            255, 255, 255,
        ];
        let img = GrayImage::from_vec(3, 3, pixs).unwrap();

        //test black/white
        {
            let exp = Crop::from_edge_offsets((3, 3), 0, 0, 0, 0);
            let act = img.letterbox_crop(super::LetterboxColour::BlackWhite(1));

            assert_eq!(exp, act);
        }

        //test anycolour
        {
            let exp = Crop::from_edge_offsets((3, 3), 0, 0, 0, 0);
            let act = img.letterbox_crop(super::LetterboxColour::AnyColour(1));

            assert_eq!(exp, act);
        }
    }

    //Should find no crop, as the letterboxes will converge
    #[test]
    fn test_letterbox_crop_black_img_finds_no_crop() {
        #[rustfmt::skip]
            let pixs = vec![
                  0,   0,   0,
                  0,   0,   0,
                  0,   0,   0,
            ];
        let img = GrayImage::from_vec(3, 3, pixs).unwrap();

        //test black/white
        {
            let exp = Crop::from_edge_offsets((3, 3), 0, 0, 0, 0);
            let act = img.letterbox_crop(super::LetterboxColour::BlackWhite(1));

            assert_eq!(exp, act);
        }

        //test anycolour
        {
            let exp = Crop::from_edge_offsets((3, 3), 0, 0, 0, 0);
            let act = img.letterbox_crop(super::LetterboxColour::AnyColour(1));

            assert_eq!(exp, act);
        }
    }

    #[test]
    fn test_letterbox_crop_any_colour_gray() {
        #[rustfmt::skip]
        let pixs = vec![
            127, 127, 127,
            127, 0,   127,
            127, 127, 127,
        ];
        let img = GrayImage::from_vec(3, 3, pixs).unwrap();

        //test black/white
        {
            let exp = Crop::from_edge_offsets((3, 3), 0, 0, 0, 0);
            let act = img.letterbox_crop(super::LetterboxColour::BlackWhite(1));

            assert_eq!(exp, act);
        }

        //test anycolour
        {
            let exp = Crop::from_edge_offsets((3, 3), 1, 1, 1, 1);
            let act = img.letterbox_crop(super::LetterboxColour::AnyColour(1));

            assert_eq!(exp, act);
        }
    }

    #[test]
    fn test_letterbox_crop_any_threshold() {
        #[rustfmt::skip]
        let pixs = vec![
            120, 130, 120,
            130, 0,   130,
            120, 130, 120,
        ];
        let img = GrayImage::from_vec(3, 3, pixs).unwrap();

        //just under, should find no crop
        {
            let exp = Crop::from_edge_offsets((3, 3), 0, 0, 0, 0);
            let act = img.letterbox_crop(super::LetterboxColour::AnyColour(9));

            assert_eq!(exp, act);
        }

        {
            let exp = Crop::from_edge_offsets((3, 3), 1, 1, 1, 1);
            let act = img.letterbox_crop(super::LetterboxColour::AnyColour(10));

            assert_eq!(exp, act);
        }
    }

    #[test]
    fn test_letterbox_crop_onepix() {
        #[rustfmt::skip]
        let pixs = vec![
            0,   0,   0,
            0, 127,   0,
            0,   0,   0,
        ];
        let img = GrayImage::from_vec(3, 3, pixs).unwrap();

        //test black/white
        {
            let exp = Crop::from_edge_offsets((3, 3), 1, 1, 1, 1);
            let act = img.letterbox_crop(super::LetterboxColour::BlackWhite(10));

            assert_eq!(exp, act);
        }

        //test anycolour
        {
            let exp = Crop::from_edge_offsets((3, 3), 1, 1, 1, 1);
            let act = img.letterbox_crop(super::LetterboxColour::AnyColour(1));

            assert_eq!(exp, act);
        }
    }

    #[test]
    fn test_letterbox_crop_topcorner() {
        #[rustfmt::skip]
        let pixs = vec![
            127, 0,   0,
            0,   0,   0,
            0,   0,   0,
        ];
        let img = GrayImage::from_vec(3, 3, pixs).unwrap();

        //test black/white
        {
            let exp = Crop::from_edge_offsets((3, 3), 0, 2, 0, 2);
            let act = img.letterbox_crop(super::LetterboxColour::BlackWhite(10));

            assert_eq!(exp, act);
        }

        //test anycolour
        {
            let exp = Crop::from_edge_offsets((3, 3), 0, 2, 0, 2);
            let act = img.letterbox_crop(super::LetterboxColour::AnyColour(1));

            assert_eq!(exp, act);
        }
    }

    #[test]
    fn test_letterbox_crop_rightedge() {
        #[rustfmt::skip]
        let pixs = vec![
              0,   0, 200,
              0,   0, 120,
              0,   0, 100,
        ];
        let img = GrayImage::from_vec(3, 3, pixs).unwrap();

        //test black/white
        {
            let exp = Crop::from_edge_offsets((3, 3), 2, 0, 0, 0);
            let act = img.letterbox_crop(super::LetterboxColour::BlackWhite(10));

            assert_eq!(exp, act);
        }

        //test anycolour
        {
            let exp = Crop::from_edge_offsets((3, 3), 2, 0, 0, 0);
            let act = img.letterbox_crop(super::LetterboxColour::AnyColour(1));

            assert_eq!(exp, act);
        }
    }

    #[test]
    fn test_letterbox_crop_bottom_right_2pix() {
        #[rustfmt::skip]
        let pixs = vec![
              0,   0,   0,
              0,   127, 0,
              0,   0, 127,
        ];
        let img = GrayImage::from_vec(3, 3, pixs).unwrap();

        //test black/white
        {
            let exp = Crop::from_edge_offsets((3, 3), 1, 0, 1, 0);
            let act = img.letterbox_crop(super::LetterboxColour::BlackWhite(10));

            assert_eq!(exp, act);
        }

        //test anycolour
        {
            let exp = Crop::from_edge_offsets((3, 3), 1, 0, 1, 0);
            let act = img.letterbox_crop(super::LetterboxColour::AnyColour(1));

            assert_eq!(exp, act);
        }
    }

    #[test]
    fn test_letterbox_crop_2pix_bottom() {
        #[rustfmt::skip]
        let pixs = vec![
            0,   0,   0,   0, 0,
            0, 255, 255, 255, 0,
            0, 255, 255, 255, 0,
            0, 255, 255, 255, 0,
            0,   0,   0,   0, 0,
            0,   0,   0,   0, 0,
        ];
        let img = GrayImage::from_vec(5, 6, pixs).unwrap();

        let exp = Crop::from_edge_offsets((5, 6), 1, 1, 1, 2);
        let act = img.letterbox_crop(super::LetterboxColour::AnyColour(1));

        assert_eq!(exp, act)
    }
}
