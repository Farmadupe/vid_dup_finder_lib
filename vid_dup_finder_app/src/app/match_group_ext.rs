use std::path::Path;

use image::{GenericImage, ImageBuffer, RgbImage};
use vid_dup_finder_lib::MatchGroup;

use vid_dup_finder_common::FrameSeqRgb;

use ffmpeg_gst_wrapper::ffmpeg_impl as ffmpeg_gst;

use ffmpeg_gst::FrameReaderCfgUnified;

pub trait MatchGroupExt {
    fn to_image(&self) -> Result<RgbImage, String>;
}

impl MatchGroupExt for MatchGroup {
    #[cfg(target_family = "unix")]
    fn to_image(&self) -> Result<RgbImage, String> {
        to_image_temp(self.contained_paths())
    }

    #[cfg(target_family = "windows")]
    fn to_image(&self) -> Result<RgbImage, String> {
        Err("".to_string())
    }
}

//for use when a real image cannot be generated
fn fallback_image() -> RgbImage {
    let font = rusttype::Font::try_from_bytes(include_bytes!("font/NotoSans-Regular.ttf")).unwrap();
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
        rusttype::Scale { x: 60.0, y: 60.0 },
        &font,
        "Error",
    );

    //imageproc::drawing::draw_cross_mut(&mut ret, white_pix, 100, 100);

    ret
}

fn grid_images_with_text(images: &[(String, Vec<RgbImage>)]) -> Result<RgbImage, String> {
    let font = rusttype::Font::try_from_bytes(include_bytes!("font/NotoSans-Regular.ttf")).unwrap();

    let (first_src_path, first_row) = images
        .get(0)
        .ok_or_else(|| "grid_images failed: No images were supplied".to_string())?;
    let first_img = first_row.get(0).ok_or_else(|| {
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
            rusttype::Scale { x: 15.0, y: 15.0 },
            &font,
            src_path.as_str(),
        );
    }

    Ok(grid_buf)
}

#[cfg(target_family = "unix")]
fn to_image_temp(
    img_paths: impl IntoIterator<Item = impl AsRef<Path>>,
) -> Result<RgbImage, String> {
    let all_thumbs: Vec<(String, Vec<RgbImage>)> = img_paths
        .into_iter()
        .map(|src_path| {
            let src_path = src_path.as_ref();

            let get_frames = |fps| {
                let mut builder = FrameReaderCfgUnified::from_path(src_path);
                builder.fps(fps);
                builder.spawn_rgb()
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
                    warn!("failed to generate output image for {src_path}. Got error {e}",);
                    None
                }
            };

            //process each frame.
            let frames = frames.and_then(|frames| {
                let frame_vec = frames
                    .into_iter()
                    .filter_map(Result::ok)
                    .map(|f| f.frame_owned())
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
                let new_frames = frames.resize(150, 150);
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
