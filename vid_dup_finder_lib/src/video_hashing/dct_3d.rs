use crate::definitions::DCT_SIZE;
use crate::definitions::HASH_SIZE;

use image::GenericImageView;
use ndarray::{prelude::*, s};

#[cfg(feature = "debug_hash_generation")]
use std::path::Path;
#[cfg(feature = "debug_hash_generation")]
use vid_dup_finder_common::grid_images_rgb;

use super::raw_dct_ops::dct_3d;

pub struct Dct3d(Array3<f64>);

const DCT_PATT: [usize; 3] = [DCT_SIZE as usize, DCT_SIZE as usize, DCT_SIZE as usize];
const HASH_PATT: [usize; 3] = [HASH_SIZE as usize, HASH_SIZE as usize, HASH_SIZE as usize];

impl Dct3d {
    pub fn from_images<I, V>(src_frames: I) -> Option<Self>
    where
        I: IntoIterator<Item = V>,
        V: GenericImageView<Pixel = image::Luma<u8>>,
    {
        //
        // Now extract the raw data, convert and scale into f64, in preparation for DCT.
        let mut frames_matrix = Array3::zeros(DCT_PATT);

        let mut frame_counter = 0;
        for (frame_idx, frame) in src_frames.into_iter().enumerate().take(DCT_SIZE as usize) {
            frame_counter += 1;
            //the caller must make sure that the supplied frames have DCT_SIZE width and
            //height
            let frame_width = frame.width() as usize;
            let frame_height = frame.height() as usize;
            assert_eq!(
                frame_width, DCT_SIZE as usize,
                "Frame width must be #{DCT_SIZE}, but is actually #{frame_width}"
            );
            assert_eq!(
                frame_height, DCT_SIZE as usize,
                "Frame width must be #{DCT_SIZE}, but is actually #{frame_height}"
            );

            for (col, row, pix) in frame.pixels() {
                *frames_matrix
                    .get_mut([frame_idx, col as usize, row as usize])
                    .expect("protected by above assertions") = pix.to_centered_f64();
            }
        }

        if frame_counter == DCT_SIZE {
            let dct = dct_3d(&frames_matrix);
            Some(Self(dct))
        } else {
            None
        }
    }

    pub fn hash_bits(&self) -> impl Iterator<Item = bool> + '_ {
        //keep the lowest frequency bins.

        Self::hash_bins(&self.0)
            .into_iter()
            .copied()
            .map(|x| x > 0.0)
    }

    fn hash_bins(m: &Array3<f64>) -> ArrayView3<f64> {
        m.slice(s![..HASH_PATT[0], ..HASH_PATT[1], ..HASH_PATT[2]])
    }
}

//debug-only functions
#[cfg(feature = "debug_hash_generation")]
impl Dct3d {
    pub fn save_debug_imgs(&self, dest: impl AsRef<Path>, suffix: &str) {
        let dest = dest.as_ref();

        let tgt_size = 64;
        let dct_window_sizes = [64usize, 32, 16, 8];
        for dct_window_size in dct_window_sizes {
            let debug_imgs = self.debug_images(
                (dct_window_size, dct_window_size, dct_window_size),
                (tgt_size, tgt_size, tgt_size),
            );

            let debug_imgs = debug_imgs.chunks(8).collect::<Vec<_>>();
            let debug_img = grid_images_rgb(debug_imgs.as_slice());

            // for (i, img) in debug_imgs.iter().enumerate() {
            let filename = format!(
                "{}_resized_{}x{}_{}.png",
                suffix, dct_window_size, dct_window_size, 0
            );

            let path = dest.join(filename);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            if path.exists() {
                std::fs::remove_file(&path).unwrap();
            }

            debug_img.unwrap().save(&path).unwrap();
            // }
        }
    }

    pub fn debug_images(
        &self,
        hash_size: (usize, usize, usize),
        tgt_size: (usize, usize, usize),
    ) -> Vec<RgbImage> {
        let mut ret = Array3::<f64>::zeros(tgt_size);

        let most_sig_bins =
            self.0
                .clone()
                .slice_move(s![..hash_size.0, ..hash_size.1, ..hash_size.2]);

        for (idx, val) in most_sig_bins.indexed_iter() {
            //*ret.get_mut(idx).unwrap() = val.signum() * 32767.0
            *ret.get_mut(idx).unwrap() = *val;
        }

        let idct = idct_3d(&ret);
        let idct_u8 = idct.map(|x| (x + 128.0).clamp(0.0, 255.0).to_u8().unwrap());

        let (num_frames, x_len, y_len) = idct_u8.dim();
        //println!("{:?}", (num_frames, x_len, y_len));

        let mut ret = (0..num_frames)
            .map(|_| RgbImage::new(x_len as u32, y_len as u32))
            .collect::<Vec<_>>();

        for ((frame_idx, x, y), val) in idct_u8.indexed_iter() {
            *ret.get_mut(frame_idx)
                .unwrap()
                .get_pixel_mut(x as u32, y as u32) = image::Rgb::<u8>([*val, *val, *val]);
        }

        ret
    }
}

trait LumaPixExt {
    fn to_centered_f64(&self) -> f64;
}

impl LumaPixExt for image::Luma<u8> {
    fn to_centered_f64(&self) -> f64 {
        let Self([luma]) = self;
        f64::from(*luma) - 128.0
    }
}
