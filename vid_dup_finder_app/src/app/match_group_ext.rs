use std::path::{Path, PathBuf};

use ffmpeg_gst_wrapper::FrameReadCfgTrait;
use image::{GenericImage, ImageBuffer, RgbImage};
use itertools::{Either, Itertools};
use vid_dup_finder_common::FrameSeqRgb;
use vid_dup_finder_lib::MatchGroup;

use crate::video_hash_filesystem_cache::filename_pattern::{FilenamePattern, FilterFilenames};

pub trait MatchGroupExt {
    fn to_image(&self) -> Result<RgbImage, String>;

    fn filter<F>(&self, filter: F) -> Option<MatchGroup>
    where
        F: FilterFilenames;

    //given a filter that will match references, return a set of groups with the same videos,
    //but with references occupying the reference field.
    fn extract_reference(
        &self,
        reference_filter: &FilenamePattern,
    ) -> impl Iterator<Item = MatchGroup>;
}

impl MatchGroupExt for MatchGroup {
    #[cfg(target_family = "unix")]
    fn to_image(&self) -> Result<RgbImage, String> {
        #[cfg(feature = "ffmpeg_backend")]
        return to_image_temp::<ffmpeg_gst_wrapper::ffmpeg_impl::FrameReaderCfgFfmpeg>(
            self.contained_paths(),
        );

        #[cfg(feature = "gstreamer_backend")]
        return to_image_temp::<ffmpeg_gst_wrapper::ffmpeg_impl::FrameReaderCfgFfmpeg>(
            self.contained_paths(),
        );
    }

    #[cfg(target_family = "windows")]
    fn to_image(&self) -> Result<RgbImage, String> {
        Err("".to_string())
    }

    fn filter<F>(&self, filter: F) -> Option<MatchGroup>
    where
        F: FilterFilenames,
    {
        if let Some(reference) = self.reference() {
            if !filter.includes(reference) {
                None
            } else {
                let new_dups = self
                    .duplicates()
                    .filter(|p| filter.includes(p))
                    .map(|p| p.to_path_buf())
                    .collect::<Vec<_>>();
                if new_dups.is_empty() {
                    None
                } else {
                    MatchGroup::new_with_reference(reference.to_path_buf(), new_dups).ok()
                }
            }
        } else {
            let new_dups = self
                .duplicates()
                .filter(|p| filter.includes(p))
                .map(|p| p.to_path_buf())
                .collect::<Vec<_>>();
            if new_dups.len() < 2 {
                None
            } else {
                MatchGroup::new(new_dups).ok()
            }
        }
    }

    fn extract_reference(
        &self,
        reference_filter: &FilenamePattern,
    ) -> impl Iterator<Item = MatchGroup> {
        assert!(self.reference().is_none());

        let (cand_paths, ref_paths): (Vec<_>, Vec<_>) = self
            .duplicates()
            .map(|x| x.to_path_buf())
            .partition_map(|dup| {
                if reference_filter.includes(&dup) {
                    Either::Left(dup)
                } else {
                    Either::Right(dup)
                }
            });

        std::iter::from_fn({
            let mut it = ref_paths.into_iter();
            move || {
                // There must be at least one cand to be able to form a group
                if cand_paths.is_empty() {
                    None
                } else {
                    it.next().and_then(|ref_path| {
                        MatchGroup::new_with_reference(
                            ref_path.to_path_buf(),
                            cand_paths.iter().map(PathBuf::from),
                        )
                        .ok()
                    })
                }
            }
        })
    }
}

//for use when a real image cannot be generated
fn fallback_image() -> RgbImage {
    let font =
        ab_glyph::FontRef::try_from_slice(include_bytes!("font/NotoSans-Regular.ttf")).unwrap();
    let mut ret = RgbImage::new(150, 150);

    let red_pix = image::Rgb::<u8>([127, 0, 0]);
    let white_pix = image::Rgb::<u8>([160, 160, 160]);

    ret.fill(128);

    let rectangle = imageproc::rect::Rect::at(10, 10).of_size(130, 130);

    imageproc::drawing::draw_filled_rect_mut(&mut ret, rectangle, red_pix);

    imageproc::drawing::draw_text_mut(
        &mut ret,
        white_pix,
        20,
        50,
        ab_glyph::PxScale { x: 60.0, y: 60.0 },
        &font,
        "Error",
    );

    //imageproc::drawing::draw_cross_mut(&mut ret, white_pix, 100, 100);

    ret
}

fn grid_images_with_text(images: &[(String, Vec<RgbImage>)]) -> Result<RgbImage, String> {
    let font =
        ab_glyph::FontRef::try_from_slice(include_bytes!("font/NotoSans-Regular.ttf")).unwrap();

    let (first_src_path, first_row) = images
        .first()
        .ok_or_else(|| "grid_images failed: No images were supplied".to_string())?;
    let first_img = first_row.first().ok_or_else(|| {
        format!("grid_images failed: No images were supplied for {first_src_path}",)
    })?;
    let (img_x, img_y) = first_img.dimensions();
    let grid_num_x = images
        .iter()
        .map(|(_src_path, imgs)| imgs.len())
        .max()
        .unwrap_or(0) as u32;
    let grid_num_y = images.len() as u32;

    let txt_y = 20;

    let grid_buf_row_y = img_y as i32 + txt_y;

    let grid_buf_x = img_x as i32 * grid_num_x as i32;
    let grid_buf_y = grid_buf_row_y as u32 * grid_num_y;

    let mut grid_buf: RgbImage = ImageBuffer::new(grid_buf_x as u32, grid_buf_y);

    for (col_no, (src_path, row_imgs)) in images.iter().enumerate() {
        let y_coord = col_no as i32 * grid_buf_row_y;
        for (row_no, img) in row_imgs.iter().enumerate() {
            let x_coord = row_no as i32 * img_x as i32;

            grid_buf
                .copy_from(
                    img as &RgbImage,
                    x_coord as u32,
                    y_coord as u32 + txt_y as u32,
                )
                .unwrap();
        }
        imageproc::drawing::draw_text_mut(
            &mut grid_buf,
            image::Rgb::<u8>([255, 255, 255]),
            0,
            y_coord + 3,
            ab_glyph::PxScale { x: 15.0, y: 15.0 },
            &font,
            src_path.as_str(),
        );
    }

    Ok(grid_buf)
}

#[cfg(target_family = "unix")]
fn to_image_temp<T: FrameReadCfgTrait>(
    img_paths: impl IntoIterator<Item = impl AsRef<Path>>,
) -> Result<RgbImage, String> {
    use std::num::NonZeroU32;

    let all_thumbs: Vec<(String, Vec<RgbImage>)> = img_paths
        .into_iter()
        .map(|src_path| {
            let src_path = src_path.as_ref();

            let get_frames = |fps| {
                let mut builder = T::from_path(src_path);
                builder.fps(fps);
                let mut frame_iterator = builder.spawn_rgb().peekable();

                match frame_iterator.peek() {
                    None => Err(()),
                    Some(Err(_)) => Err(()),
                    Some(Ok(_frame)) => Ok(frame_iterator),
                }
            };

            //first try and get 7 frames at 1/5 fps. If that doesn't work then try a faster framerate
            //Finally try and get 7 frames at native framerate.
            let b1 = || get_frames((1, 5));
            let b2 = || get_frames((2, 1));
            let b3 = || get_frames((5, 1));
            let b4 = || get_frames((30, 1));

            let frame_builder = b1().or_else(|_| b2()).or_else(|_| b3()).or_else(|_| b4());

            let frames = match frame_builder {
                Ok(frame_iter) => Some(frame_iter),
                Err(e) => {
                    let src_path = src_path.display();
                    warn!("failed to generate output image for {src_path}. Got error {e:?}",);
                    None
                }
            };

            //process each frame.
            let frames = frames.and_then(|frames| {
                let frame_vec = frames
                    .into_iter()
                    .filter_map(Result::ok)
                    .take(4)
                    .collect::<Vec<_>>();
                let seq = FrameSeqRgb::from_images(frame_vec);

                if seq.is_none() {
                    warn!(
                        "Failed to extract any frames from video: {}",
                        src_path.display()
                    );
                }

                seq
            });

            //if any step failed, then use the fallback images instead
            let frames = frames.unwrap_or_else(|| {
                FrameSeqRgb::from_images((0..5).map(|_i| fallback_image())).unwrap()
            });

            (src_path.to_string_lossy().to_string(), frames)
        })
        .flat_map(|(path, frames)| {
            if false {
                vec![(path, frames.into_inner())].into_iter()
            } else {
                let size = NonZeroU32::new(150).expect("literal value");
                let new_frames = frames.resize(size, size);
                let new_frames = new_frames.into_inner();
                //println!("{:?}, {}", new_frames[0].dimensions(), new_frames.len());
                vec![(path, new_frames)].into_iter()
            }
        })
        .collect::<Vec<_>>();

    if all_thumbs.is_empty() {
        Err("no thumbs".to_string())
    } else {
        grid_images_with_text(&all_thumbs)
    }
}
