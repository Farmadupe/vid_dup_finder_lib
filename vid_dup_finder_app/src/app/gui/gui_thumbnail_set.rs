use std::{
    collections::HashMap,
    num::NonZeroU32,
    path::{Path, PathBuf},
};

use gdk_pixbuf::Pixbuf;
use image::{buffer::ConvertBuffer, RgbImage};

#[cfg(feature = "parallel_loading")]
use rayon::prelude::*;

use vid_dup_finder_common::{row_images, video_frames_gray::VdfFrameSeqExt, FrameSeqRgb};
use vid_dup_finder_lib::*;

use super::gui_zoom::ZoomState;
use crate::app::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbChoice {
    Video,
    CropdetectVideo,
    HashBits,
    Reddened,
    Rebuilt,
}

#[derive(Debug)]
struct ThumbRow {
    thumbs: Vec<RgbImage>,
    src_path: PathBuf,
}

impl ThumbRow {
    pub fn video_from_filename(src_path: &Path) -> Self {
        Self {
            thumbs: VideoHash::build_frame_reader(
                src_path,
                vid_dup_finder_lib::DEFAULT_HASH_CREATION_OPTIONS.skip_forward_amount,
            )
            .ok()
            .map(|x| x.spawn_rgb())
            .and_then(Result::ok)
            .map(|it| {
                it.filter_map(|res| match res {
                    Ok(x) => Some(x),
                    Err(_e) => {
                        //println!("{e:?}");
                        None
                    }
                })
                .step_by(8)
                .take(8)
                .map(|frame| frame.frame_owned())
                .collect::<Vec<_>>()
            })
            .unwrap_or_else(Self::fallback_images),
            src_path: src_path.to_path_buf(),
        }
    }

    pub fn rebuilt_from_hash(hash: &VideoHash) -> Self {
        Self {
            thumbs: hash.reconstructed_frames(),
            src_path: hash.src_path().to_path_buf(),
        }
    }

    pub fn spatial_from_hash(hash: &VideoHash) -> Self {
        Self {
            thumbs: hash_bits(hash)
                .into_iter()
                .map(|img| img.convert())
                .collect(),

            src_path: hash.src_path().to_path_buf(),
        }
    }

    pub fn reddened_from_hash_bits(hash: &VideoHash) -> Self {
        Self {
            thumbs: reddened_thumbs(hash)
                .into_iter()
                .map(|img| img.convert())
                .collect(),

            src_path: hash.src_path().to_path_buf(),
        }
    }

    pub fn zoom(&self, zoom: ZoomState) -> RgbImage {
        use gui::gui_zoom::ZoomValue::*;
        match zoom.get() {
            User(size) => {
                #[cfg(feature = "parallel_loading")]
                let it = self.thumbs.par_iter();

                #[cfg(not(feature = "parallel_loading"))]
                let it = self.thumbs.iter();

                let images = it
                    .map(|thumb| {
                        let size = NonZeroU32::try_from(size).unwrap();
                        vid_dup_finder_common::resize_rgb::resize_img_rgb(thumb, size, size)
                    })
                    .collect::<Vec<_>>();

                row_images(images.iter()).unwrap()
            }
            Native => row_images(self.thumbs.iter()).unwrap(),
        }
    }

    //if an error occurs while generating thumbs, supply a default image as a placeholder
    fn fallback_images() -> Vec<RgbImage> {
        vec![RgbImage::new(100, 100), RgbImage::new(100, 100)]
    }

    fn without_letterbox(&self) -> Self {
        let uncropped_frames = FrameSeqRgb::from_images(self.thumbs.clone()).unwrap();
        let crop = uncropped_frames.letterbox_crop();
        dbg!(crop);
        let frames = uncropped_frames.crop(crop.as_view_args());

        //println!("{crop:#?}");

        Self {
            thumbs: frames.into_inner(),
            src_path: self.src_path.clone(),
        }
    }
}

#[derive(Debug)]
struct GuiThumbnail {
    filename: PathBuf,
    hash: VideoHash,

    base_video: Option<ThumbRow>,
    base_cropdetect: Option<ThumbRow>,
    hash_bits: Option<ThumbRow>,
    reddened: Option<ThumbRow>,
    rebuilt: Option<ThumbRow>,

    resized_thumb: Option<RgbImage>,

    zoom: ZoomState,
    choice: ThumbChoice,

    rendered_zoom: Option<ZoomState>,
    rendered_choice: Option<ThumbChoice>,
}

impl GuiThumbnail {
    pub fn new(filename: &Path, hash: VideoHash, zoom: ZoomState, choice: ThumbChoice) -> Self {
        Self {
            filename: filename.to_path_buf(),

            hash,

            base_video: None,
            base_cropdetect: None,
            hash_bits: None,
            reddened: None,
            resized_thumb: None,
            rebuilt: None,

            zoom,
            choice,

            rendered_zoom: None,
            rendered_choice: None,
        }
    }

    pub fn get(&mut self) -> RgbImage {
        let should_rerender = self.rendered_zoom.is_none()
            || self.rendered_choice.is_none()
            || self.rendered_zoom.unwrap() != self.zoom
            || self.rendered_choice.unwrap() != self.choice;

        self.rendered_zoom = Some(self.zoom);
        self.rendered_choice = Some(self.choice);

        if !should_rerender {
            return self.resized_thumb.as_ref().unwrap().clone();
        }

        match self.choice {
            ThumbChoice::Video => {
                if self.base_video.is_none() {
                    self.base_video = Some(ThumbRow::video_from_filename(&self.filename));
                }
            }
            ThumbChoice::CropdetectVideo => {
                if self.base_video.is_none() {
                    self.base_video = Some(ThumbRow::video_from_filename(&self.filename));
                }

                if self.base_cropdetect.is_none() {
                    self.base_cropdetect =
                        Some(self.base_video.as_ref().unwrap().without_letterbox());
                }
            }

            ThumbChoice::HashBits => {
                if self.hash_bits.is_none() {
                    self.hash_bits = Some(ThumbRow::spatial_from_hash(&self.hash));
                }
            }
            ThumbChoice::Reddened => {
                if self.reddened.is_none() {
                    //first get the hash bits
                    if self.hash_bits.is_none() {
                        self.hash_bits = Some(ThumbRow::spatial_from_hash(&self.hash));
                    }
                    self.reddened = Some(ThumbRow::reddened_from_hash_bits(&self.hash));
                }
            }
            ThumbChoice::Rebuilt => {
                if self.rebuilt.is_none() {
                    self.rebuilt = Some(ThumbRow::rebuilt_from_hash(&self.hash));
                }
            }
        }

        match self.choice {
            ThumbChoice::Video => {
                self.resized_thumb = Some(self.base_video.as_ref().unwrap().zoom(self.zoom));
            }
            ThumbChoice::CropdetectVideo => {
                self.resized_thumb = Some(self.base_cropdetect.as_ref().unwrap().zoom(self.zoom));
            }
            ThumbChoice::HashBits => {
                self.resized_thumb = Some(self.hash_bits.as_ref().unwrap().zoom(self.zoom));
            }
            ThumbChoice::Reddened => {
                self.resized_thumb = Some(self.reddened.as_ref().unwrap().zoom(self.zoom));
            }
            ThumbChoice::Rebuilt => {
                self.resized_thumb = Some(self.rebuilt.as_ref().unwrap().zoom(self.zoom));
            }
        }

        self.resized_thumb.as_ref().unwrap().clone()
    }

    pub fn set_zoom(&mut self, zoom: ZoomState) {
        self.zoom = zoom;
    }

    pub fn set_choice(&mut self, choice: ThumbChoice) {
        self.choice = choice;
    }
}

#[derive(Debug)]
pub struct GuiThumbnailSet {
    thumbs: HashMap<PathBuf, GuiThumbnail>,
}

impl GuiThumbnailSet {
    pub fn new(info: Vec<(&Path, VideoHash)>, zoom: ZoomState, choice: ThumbChoice) -> Self {
        let mut thumbs = HashMap::new();

        #[cfg(feature = "parallel_loading")]
        let it = info.into_par_iter();

        #[cfg(not(feature = "parallel_loading"))]
        let it = info.into_iter();

        let imgs = it
            .map(|(src_path, hash)| {
                (
                    src_path.to_path_buf(),
                    GuiThumbnail::new(src_path, hash, zoom, choice),
                )
            })
            .collect::<Vec<_>>();

        for (src_path, thumb) in imgs {
            thumbs.insert(src_path.clone(), thumb);
        }

        Self { thumbs }
    }

    pub fn set_zoom(&mut self, val: ZoomState) {
        #[cfg(feature = "parallel_loading")]
        let it = self.thumbs.par_iter_mut();

        #[cfg(not(feature = "parallel_loading"))]
        let it = self.thumbs.iter_mut();

        it.for_each(|(_src_path, thumb)| {
            thumb.set_zoom(val);
        });
    }

    pub fn set_choice(&mut self, val: ThumbChoice) {
        #[cfg(feature = "parallel_loading")]
        let it = self.thumbs.par_iter_mut();

        #[cfg(not(feature = "parallel_loading"))]
        let it = self.thumbs.iter_mut();

        it.for_each(|(_src_path, thumb)| {
            thumb.set_choice(val);
        });
    }

    pub fn get_pixbufs(&mut self) -> HashMap<PathBuf, Pixbuf> {
        let mut ret = HashMap::new();
        for (src_path, thumb) in &mut self.thumbs {
            let x = thumb.get();
            ret.insert(src_path.clone(), Self::image_to_gdk_pixbuf(x));
        }

        ret
    }

    fn image_to_gdk_pixbuf(img: RgbImage) -> Pixbuf {
        let (width, height) = img.dimensions();
        let bytes = glib::Bytes::from_owned(img.into_raw());

        Pixbuf::from_bytes(
            &bytes,
            gdk_pixbuf::Colorspace::Rgb,
            false,
            8,
            width as i32,
            height as i32,
            width as i32 * 3,
        )
    }
}

const BLACK_PIXEL: image::Rgb<u8> = image::Rgb([0, 0, 0]);
const WHITE_PIXEL: image::Rgb<u8> = image::Rgb([255, 255, 255]);
const _RED_PIXEL: image::Rgb<u8> = image::Rgb([255, 0, 0]);
pub fn hash_bits(hash: &VideoHash) -> Vec<RgbImage> {
    let raw_bits = hash.hash_bits();
    let (x, y) = VideoHash::hash_frame_dimensions();
    let raw_frames = raw_bits.chunks_exact(x * y);
    let ret = raw_frames
        .into_iter()
        .map(|raw_frame| {
            let mut image = RgbImage::new(x as u32, y as u32);
            let raw_rows = raw_frame.chunks_exact(x);
            raw_rows.into_iter().enumerate().for_each(|(x, raw_row)| {
                raw_row.iter().enumerate().for_each(|(y, elem)| {
                    if *elem {
                        *image.get_pixel_mut(x as u32, y as u32) = WHITE_PIXEL;
                    } else {
                        *image.get_pixel_mut(x as u32, y as u32) = BLACK_PIXEL;
                    }
                });
            });
            image
        })
        .collect::<Vec<_>>();

    ret
}

pub fn reddened_thumbs(_hash: &VideoHash) -> Vec<RgbImage> {
    unimplemented!();
}
