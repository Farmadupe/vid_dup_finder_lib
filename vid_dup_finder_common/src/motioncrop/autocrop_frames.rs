use std::ops::Deref;

use imageproc::contrast::stretch_contrast_mut;
use itertools::Itertools;

use image::{buffer::ConvertBuffer, GenericImageView, GrayImage, Luma, RgbImage};

use super::{
    darkest_frame::DarkestFrame,
    frame_change::FrameChange,
    utils::{colourize_regions, maskize_regions, regionize_image, tint_cropped_area, RgbChan},
};
use crate::{
    crop::Crop,
    motioncrop::utils::clear_out_cropped_area,
    video_frames_gray::{LetterboxColour, VdfFrameExt},
};

#[derive(Debug, Clone)]
pub struct MotiondetectCrop {
    _crop: (),
    // darkest_frame: DarkestFrame,
    // movement_intensity: FrameChange<I>,
    // guessed_regions: Vec<Crop>,
    // frames_added: usize,
}

// struct CropAndMaskedoutFrames {
//     crop: Option<Crop>,
//     masked_out_frames: Vec<GrayImage>,
// }

impl MotiondetectCrop {
    #[allow(clippy::new_without_default)]
    #[must_use]
    pub fn from_frames(
        frames: impl IntoIterator<Item = impl Deref<Target = GrayImage>>,
    ) -> Option<Crop> {
        //for now, we need a mutable copy of all of the frames to do this crop :(
        let mut frames = frames
            .into_iter()
            .map(|f| f.deref().clone())
            .collect::<Vec<_>>();

        if frames.len() < 2 {
            return None;
        }

        let mut min_pix: u8 = 255;
        let mut max_pix: u8 = 0;
        #[allow(unused_variables)]
        frames.iter().enumerate().for_each(|(i, frame)| {
            let mut frame_min_pix: u8 = 255;
            let mut frame_max_pix: u8 = 0;

            for Luma([pix]) in frame.pixels() {
                if *pix < frame_min_pix {
                    frame_min_pix = *pix;
                }

                if *pix > frame_max_pix {
                    frame_max_pix = *pix;
                }
            }

            // eprintln!("frame {i}, min: {frame_min_pix}, max: {frame_max_pix}");

            // if let Some(debug_dir) = debug_img_dir() {
            //     if frame_max_pix == 255 {
            //         let mut annotated: RgbImage = frame.clone().convert();
            //         for Rgb([ref mut r, ref mut g, ref mut b]) in annotated.pixels_mut() {
            //             if *r == 255 && *g == 255 && *b == 255 {
            //                 *g = 0;
            //                 *b = 0;
            //             }
            //         }

            //         annotated
            //             .save(Path::new(&debug_dir).join(format!("{i}.png")))
            //             .unwrap();
            //     }
            // }

            min_pix = min_pix.min(frame_min_pix);
            max_pix = max_pix.max(frame_max_pix);
        });

        if debug_img_dir().is_some() {
            let (modal_pix, modal_count) = {
                let mut acc = [0usize; 256];
                let each_pix = frames.iter().flat_map(|frame| frame.pixels());
                for Luma([inty]) in each_pix {
                    acc[*inty as usize] += 1;
                }

                acc.into_iter().enumerate().max_by_key(|&(_, item)| item)
            }
            .unwrap();

            let num_pix = (frames[0].width() * frames[0].height()) * frames.len() as u32;

            let modal_proportion = modal_count as f64 / num_pix as f64;

            #[allow(clippy::print_stderr)]
            let () = eprintln!("minmax_inty: ({min_pix:?}, {max_pix:?}) modal pix: {modal_pix:?} modal pix proportion: {:.0}%", modal_proportion * 100.0);
        }

        #[allow(clippy::collapsible_if)]
        if max_pix != 255 && min_pix != 0 {
            if min_pix < max_pix {
                for frame in &mut frames {
                    stretch_contrast_mut(frame, min_pix, max_pix, 0, 255);
                }
            }
        }

        //check that all frames are the same size
        for (f1, f2) in frames.iter().tuple_windows::<(_, _)>() {
            if f1.dimensions() != f2.dimensions() {
                return None;
            }
        }

        //get the letterbox crop
        let letterbox_crop = frames
            .iter()
            .fold(None, |acc, frame| {
                let this_frame_letterbox = frame.letterbox_crop(LetterboxColour::AnyColour(16));

                match acc {
                    None => Some(this_frame_letterbox),
                    Some(c) => Some(this_frame_letterbox.union(&c)),
                }
            })
            .expect("should always be at least 1 frame by this point");

        //whiten out the letterbox
        for frame in frames.iter_mut() {
            for (x, y) in letterbox_crop.enumerate_coords_excluded() {
                frame.put_pixel(x, y, Luma([255]));
            }
        }

        let crop_1 = Self::from_frames_one(&frames);

        let first_frame = frames[0].clone();

        let crop_2 = match crop_1 {
            None => None,
            Some(crop_1) => {
                for (i, frame) in frames.iter_mut().enumerate() {
                    if i == 1 {
                        if let Some(debug_dir) = debug_img_dir() {
                            frame.save(format!("{debug_dir}/{i}_a.png")).unwrap();
                        }
                    }
                    clear_out_cropped_area(frame, crop_1);
                    if i == 1 {
                        if let Some(debug_dir) = debug_img_dir() {
                            frame.save(format!("{debug_dir}/{i}_b.png")).unwrap();
                        }
                    }
                }
                Self::from_frames_one(&frames)
            }
        };

        let crops = [crop_1, crop_2].into_iter().flatten().collect::<Vec<_>>();

        if crops.is_empty() {
            return Some(letterbox_crop);
        }

        let filtered_crops = crops.iter().copied();

        //reject implausible aspect ratio
        let worst_aspect_ratio: f64 = 3.0;
        let filtered_crops = filtered_crops.filter(|crop| {
            let width = f64::from(crop.width());
            let height = f64::from(crop.height());
            let aspect_ratio = if width > height {
                width / height
            } else {
                height / width
            };

            aspect_ratio <= worst_aspect_ratio
        });

        //reject crops that are too small
        let largest_area = f64::from(crops.iter().map(Crop::area).max()?);
        let filtered_crops =
            filtered_crops.filter(|crop| f64::from(crop.area()) > largest_area * 0.8);

        //select topmost if there are several candidates
        let ret = filtered_crops.min_by_key(|crop| crop.top);

        //if we didn't actually detect anything, just return the letterbox
        let ret = ret.unwrap_or(letterbox_crop);

        if let Some(debug_dir) = debug_img_dir() {
            let mut first_frame: RgbImage = first_frame.convert();

            for crop in crops.iter().copied() {
                let chan = if crop == ret {
                    RgbChan::Red
                } else {
                    RgbChan::Blue
                };

                first_frame = tint_cropped_area(&first_frame, crop, chan);
            }

            first_frame
                .save(format!("{debug_dir}/combined.png"))
                .unwrap();
        }

        Some(ret)
    }

    #[allow(clippy::new_without_default)]
    #[must_use]
    fn from_frames_one(frames: &[GrayImage]) -> Option<Crop> {
        // dbg!(letterbox_crop, letterbox_crop.as_view_args());

        //quick heuristic to see if there are lots of white/black pixels left
        // if false {
        //     let bg_pix_proportion = {
        //         let total_pix = frames.len() * width as usize * height as usize;
        //         let bg_pix = frames
        //             .iter()
        //             .flat_map(|f| f.pixels())
        //             .filter(|Luma([pix])| (*pix > 230 || *pix < 5) && *pix != 254)
        //             .count();
        //         let val = bg_pix as f64 / total_pix as f64;

        //         // let s = format!("total_pix: {total_pix}, bg_pix: {bg_pix}, proportion: {val}");
        //         // dbg!(s);
        //         val
        //     };

        //     if bg_pix_proportion < MIN_SATURATED_PIX {
        //         return None;
        //     }
        // }
        let mut darkest_frame = DarkestFrame::try_from_seq(frames)?;
        let mut movement_intensity =
            FrameChange::try_from_iter(frames.iter().tuple_windows::<(_, _)>())?;

        let largest_motion_region = movement_intensity.largest_region_with_motion();

        let retained_region_mask =
            darkest_frame.largest_dark_region_with_motion(largest_motion_region)?;

        let subimage = super::utils::view_mask(&retained_region_mask)?;

        let ret = {
            let (x, y) = subimage.offsets();
            let (orig_width, orig_height) = frames.first().unwrap().dimensions();

            Crop::from_topleft_and_dims(
                (orig_width, orig_height),
                x,
                y,
                subimage.width(),
                subimage.height(),
            )
        };

        if let Some(debug_dir) = debug_img_dir() {
            let rand = rand::random::<u64>();

            std::fs::create_dir_all(&debug_dir).ok();

            colourize_regions(&regionize_image(darkest_frame.inner()).0)
                .save(format!("{debug_dir}/{rand}darkest_frame.png"))
                .unwrap();
            colourize_regions(&regionize_image(largest_motion_region).0)
                .save(format!("{debug_dir}/{rand}largest_motion_region.png"))
                .unwrap();

            maskize_regions(&movement_intensity.largest_area().unwrap())
                .save(format!("{debug_dir}/{rand}movement_intensity_largest.png"))
                .unwrap();

            retained_region_mask
                .save(format!("{debug_dir}/{rand}retained_region.png"))
                .unwrap();

            let (a, b, c, d) = ret.as_view_args();
            let check_it_worked = frames[0].view(a, b, c, d);
            check_it_worked
                .to_image()
                .save(format!("{debug_dir}/{rand}_check_final.png"))
                .unwrap();

            subimage
                .to_image()
                .save(format!("{debug_dir}/{rand}check_pre_subimage.png"))
                .unwrap();
        }

        if ret.is_uncropped() {
            Some(ret)
        } else if let Some(eroded) = Some(ret).and_then(Crop::eroded).and_then(Crop::eroded) {
            Some(eroded)
        } else {
            Some(ret)
        }
    }
}

fn debug_img_dir() -> Option<String> {
    std::env::var("AUTOCROP_DEBUG_IMG_DIR").ok()
}
