use std::convert::TryInto;

use crate::definitions::*;
use crate::*;
use ffmpeg_cmdline_utils::*;
use image::GrayImage;

pub struct TimeDomainSeq {
    frames: Vec<TimeDomainFrame>,
}

impl TimeDomainSeq {
    pub fn from_framified_video(frames: &VideoFrames) -> Self {
        //first resize the video
        let frames = frames.resize(RESIZE_IMAGE_X as u32, RESIZE_IMAGE_Y as u32);
        let grey_frames = GrayFramifiedVideo::from(frames).into_inner();

        let frames = grey_frames
            .iter()
            .map(TimeDomainFrame::from_grey_frame)
            .collect::<Vec<_>>();

        Self { frames }
    }

    pub fn temporalize(&self) -> Self {
        let frames = self
            .frames
            .windows(2)
            .map(|old_and_new| {
                //elemently check if new is greater than old.
                let old_matrix = &old_and_new[0];
                let new_matrix = &old_and_new[1];
                old_matrix.diff(new_matrix)
            })
            .collect::<Vec<_>>();

        Self { frames }
    }

    pub fn eliminate_high_frequencies(&self) -> Self {
        let frames = self
            .frames
            .iter()
            .map(TimeDomainFrame::eliminate_high_frequencies)
            .collect::<Vec<_>>();
        Self { frames }
    }

    pub fn hash(&self) -> Vec<Vec<u64>> {
        let whole_seq_average =
            self.frames.iter().map(TimeDomainFrame::average).sum::<f64>() / self.frames.len() as f64;
        self.frames.iter().map(|frame| frame.hash(whole_seq_average)).collect()
    }
}

struct TimeDomainFrame {
    frame: [f64; HASH_IMAGE_X * HASH_IMAGE_Y],
}

impl TimeDomainFrame {
    fn from_grey_frame(frame: &GrayImage) -> Self {
        let dct = utils::dct_ops::perform_dct(&image::DynamicImage::ImageLuma8(frame.clone()));

        let rowstride = (frame.dimensions().0 as f64).sqrt() as usize;

        let topleft_square_size = 8;
        //now get the topleftmost square

        let topleft_bins = dct
            .chunks(rowstride)
            .take(topleft_square_size)
            .chain(std::iter::repeat([0f64; 8].as_ref()))
            .take(8)
            .map(|r| {
                r.iter()
                    .cloned()
                    .take(topleft_square_size)
                    .chain(std::iter::repeat(0f64))
                    .take(8)
                    .collect::<Vec<_>>()
            });

        // let frame_1 = &topleft_bins[0].iter().map(|x| *x as i64).collect::<Vec<_>>();

        // println!(
        //     "rowstride: {:#?}, len: {:#?}, {:03?}",
        //     rowstride,
        //     frame_1.len(),
        //     frame_1
        // );

        Self {
            frame: topleft_bins
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
        }
    }

    fn eliminate_high_frequencies(&self) -> Self {
        // #[rustfmt::skip]
        // let mask: Vec<f64> = [
        // 0, 1, 1, 1, 1, 1 ,1, 0,
        // 1, 1, 1, 1, 1, 1 ,0, 0,
        // 1, 1, 1, 1, 1, 0 ,0, 0,
        // 1, 1, 1, 1, 0, 0 ,0, 0,
        // 1, 1, 1, 0, 0, 0 ,0, 0,
        // 1, 1, 0, 0, 0, 0 ,0, 0,
        // 1, 0, 0, 0, 0, 0 ,0, 0,
        // 0, 0, 0, 0, 0, 0 ,0, 0,

        // 0, 1, 1, 1, 1, 1 ,1, 1,
        // 1, 1, 1, 1, 1, 1 ,1, 1,
        // 1, 1, 1, 1, 1, 1 ,1, 1,
        // 1, 1, 1, 1, 1, 1 ,1, 1,
        // 1, 1, 1, 1, 1, 1 ,1, 1,
        // 1, 1, 1, 1, 1, 1 ,1, 1,
        // 1, 1, 1, 1, 1, 1 ,1, 1,
        // 1, 1, 1, 1, 1, 1 ,1, 1,

        // ].iter().map(|x| *x as f64).collect::<Vec<_>>();

        let mut frame = self.frame;
        frame[0] = 0.0;
        Self { frame }
    }

    fn diff(&self, other: &Self) -> Self {
        let frame = self
            .frame
            .iter()
            .zip(other.frame.iter())
            .map(|(old_val, new_val)| {
                let new_greater = new_val > old_val;
                let significant = (new_val.abs() - old_val.abs()).abs() > 15f64;
                //let significant = true;
                if new_greater && significant {
                    1000f64
                } else {
                    0f64
                }
            })
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        Self { frame }
    }

    fn average(&self) -> f64 {
        self.frame.iter().sum::<f64>() / self.frame.len() as f64
    }

    fn hash(&self, average: f64) -> Vec<u64> {
        //Now work out the average value, so that we can reduce each element to a single bit.
        //let all_equal = self.frame.iter().all(|elem| *elem == self.frame[0]);

        //let bittify_pred = |x| if all_equal { false } else { x > average };
        let bittify_pred = |x| x > average;
        Self::bittify(&self.frame, &bittify_pred)
    }

    fn bittify<T, F>(stuff: &[T], predicate: &F) -> Vec<u64>
    where
        F: Fn(T) -> bool,
        T: Copy,
    {
        let mut bit_mask: u64 = 1;
        let mut ret = vec![];
        let mut bitstring: u64 = 0;

        for (i, element) in stuff.iter().enumerate() {
            if predicate(*element) {
                bitstring |= bitstring ^ bit_mask;
            }

            if i % 64 == 63 {
                ret.push(bitstring);
                bit_mask = 1;
                bitstring = 0;
            } else {
                bit_mask <<= 1;
            }
        }
        ret
    }
}
