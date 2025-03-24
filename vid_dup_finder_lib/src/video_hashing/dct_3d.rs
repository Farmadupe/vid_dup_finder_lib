use crate::definitions::DCT_SIZE;
use crate::definitions::HASH_SIZE;

use image::GenericImageView;
use ndarray::{prelude::*, s};

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

trait LumaPixExt {
    fn to_centered_f64(&self) -> f64;
}

impl LumaPixExt for image::Luma<u8> {
    fn to_centered_f64(&self) -> f64 {
        let Self([luma]) = self;
        f64::from(*luma) - 128.0
    }
}
