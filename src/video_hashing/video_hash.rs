use std::{
    cmp::{max, min},
    hash::Hash,
    path::{Path, PathBuf},
};

use ffmpeg_cmdline_utils::*;
#[cfg(feature = "app_only_fns")]
use image::RgbImage;
use serde::{Deserialize, Serialize};

use crate::{
    dct_hasher::TimeDomainSeq,
    definitions::{HASH_IMAGE_X, HASH_IMAGE_Y, HASH_NUM_IMAGES},
    *,
};

const HASH_FRAME_QWORDS: usize = (HASH_IMAGE_X * HASH_IMAGE_Y) / 64;
const SPATIAL_HASH_QWORDS: usize = HASH_FRAME_QWORDS * HASH_NUM_IMAGES;
const TEMPORAL_HASH_QWORDS: usize = HASH_FRAME_QWORDS * (HASH_NUM_IMAGES - 1);
const HASH_QWORDS: usize = SPATIAL_HASH_QWORDS + TEMPORAL_HASH_QWORDS;

/// A hash of a video file, used for video duplicate detection. The hash contains information about
/// the first 30 seconds of a video, and also the duration. Searches will use these data to determine
/// similarity.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default, Serialize, Deserialize)]
pub struct VideoHash {
    hash: [u64; HASH_QWORDS],
    num_frames: u32,
    src_path: PathBuf,
    duration: u32,
}

impl VideoHash {
    ///Create a VideoHash from the video file at src_path.
    /// * Returns the hash itself if successful, otherwise
    /// * an error with the reason that the hash could not be created.
    pub fn from_path(src_path: impl AsRef<Path>) -> Result<Self, HashCreationErrorKind> {
        Self::from_path_inner(src_path).map(|(hash, _stats)| hash)
    }


    fn from_path_inner(
        src_path: impl AsRef<Path>
    ) -> Result<(Self, VideoStats), HashCreationErrorKind> {
        //Check that ffmpeg thinks there is a video at this path.
        if let Err(e) = ffmpeg_cmdline_utils::is_video_file(src_path.as_ref()) {
            return Err(HashCreationErrorKind::DetermineVideo {
                src_path: src_path.as_ref().to_path_buf(),
                error: e,
            });
        }

        //since this is a video, get the video and its associated statistics
        let (frame_iter, stats) = Self::temporal_hash_frame_builder(&src_path)?;
        let frames = frame_iter.collect::<Vec<_>>();

        // If we didn't decode num_frames frames, a hash cannot be constructed from the decoded video.
        if frames.len() != HASH_NUM_IMAGES {
            return Err(HashCreationErrorKind::VideoLength(src_path.as_ref().to_path_buf()));
        }

        //remove letterboxing
        let frames = VideoFrames::from_images(&frames);
        let mut frames = frames.without_letterbox();

        //resize to remove high frequency details
        frames = frames.resize(crate::definitions::RESIZE_IMAGE_X, crate::definitions::RESIZE_IMAGE_Y);

        let spatial_hash_seq = TimeDomainSeq::from_framified_video(&frames);
        let spatial_hash_bits = spatial_hash_seq.eliminate_high_frequencies().hash();

        let temporal_hash_seq = spatial_hash_seq.temporalize();
        let temporal_hash_bits = temporal_hash_seq
            .eliminate_high_frequencies()
            .hash()
            .into_iter()
            .take(HASH_NUM_IMAGES - 1)
            .collect();

        let hash =
            VideoHash::from_components(src_path, spatial_hash_bits, temporal_hash_bits, stats.duration as u32).unwrap();
        let stats = VideoStats::new(stats, frames.png_size());
        Ok((hash, stats))
    }

    //utility helper -- Build a ffmpeg frame reader with the correct configuration for building temporal hashes.
    fn temporal_hash_frame_builder(
        src_path: impl AsRef<Path>
    ) -> Result<(FfmpegFrames, VideoInfo), HashCreationErrorKind> {
        FfmpegFrameReaderBuilder::new(src_path.as_ref().to_owned())
            .num_frames(HASH_NUM_IMAGES as u32)
            .fps(crate::definitions::HASH_FRAMERATE)
            .timeout_secs(60)
            .spawn()
            .map_err(|e| HashCreationErrorKind::VideoProcessing {
                src_path: src_path.as_ref().to_path_buf(),
                error: e,
            })
    }

    fn from_components(
        src_path: impl AsRef<Path>,
        spatial_hash: Vec<Vec<u64>>,
        temporal_hash: Vec<Vec<u64>>,
        duration: u32,
    ) -> Result<Self, HashCreationErrorKind> {
        let mut hash_arr: [u64; HASH_QWORDS] = [0; HASH_QWORDS];

        let flattened_spatial_hash = spatial_hash.iter().flatten().copied().collect::<Vec<_>>();
        hash_arr[..flattened_spatial_hash.len()].copy_from_slice(&flattened_spatial_hash);

        let flattened_temporal_hash = temporal_hash.into_iter().flatten().collect::<Vec<_>>();
        hash_arr[SPATIAL_HASH_QWORDS..SPATIAL_HASH_QWORDS + flattened_temporal_hash.len()]
            .copy_from_slice(&flattened_temporal_hash);

        let ret = Self {
            hash: hash_arr,
            num_frames: spatial_hash.len() as u32,
            src_path: src_path.as_ref().to_owned(),
            duration: duration as u32,
        };

        Ok(ret)
    }

    /// The path to the video file from which this hash was created.
    pub fn src_path(&self) -> &Path {
        &self.src_path
    }

    /// The duration of the video from which this hash was created.
    pub fn duration(&self) -> u32 {
        self.duration
    }

    /// The raw haming distance from this hash to another hash.
    pub fn levenshtein_distance(&self, other: &VideoHash) -> RawDistance {
        //first get the raw distance, ignoring the frame count. Since unused frames are
        //guaranteed to be zeroes, we do not care about the difference between them here
        let raw_dist = Self::hamming_distance(&self.hash, &other.hash);

        //Where there is a difference between frame counts, this is accounted for by
        //declaring maximum edit distance between the frames.
        let shared_frames = min(self.num_frames, other.num_frames);
        let max_frames = max(self.num_frames, other.num_frames);

        let different_spatial_frames = max_frames - shared_frames;
        let different_temporal_frames = different_spatial_frames;
        let length_mismatch_qwords = (different_spatial_frames + different_temporal_frames) * HASH_FRAME_QWORDS as u32;

        let length_mismatch_dist = length_mismatch_qwords * 64;

        RawDistance {
            distance: raw_dist + length_mismatch_dist,
        }
    }

    /// The distance from this hash to another hash, but normalized into the range 0..=1
    pub fn normalized_levenshtein_distance(&self, other: &VideoHash) -> NormalizedDistance {
        let raw_distance = self.levenshtein_distance(other).u32_value() as f64;
        let num_frames = max(self.num_frames, other.num_frames);
        let max_spatial_distance = (64 * num_frames) as f64;
        let max_temporal_distance = (64 * (num_frames - 1)) as f64;
        let max_distance = max_spatial_distance + max_temporal_distance;

        NormalizedDistance::new(raw_distance / max_distance)
    }

    //Utility helper: Get the hamming distance between two bitstrings.
    fn hamming_distance<const N: usize>(x: &[u64; N], y: &[u64; N]) -> u32 {
        x.iter().zip(y.iter()).fold(0, |acc, (x, y)| {
            let difference = x ^ y;
            let set_bits = difference.count_ones();
            acc + set_bits
        })
    }
}

impl AsRef<VideoHash> for VideoHash {
    fn as_ref(&self) -> &VideoHash {
        self
    }
}

//associated functions for TemporalHash that are only required for the example application.
//(todo: Move these into the app crate)
#[cfg(feature = "app_only_fns")]
#[doc(hidden)]
impl VideoHash {
    pub fn from_path_with_stats(
        src_path: impl AsRef<Path>,
    ) -> Result<(Self, VideoStats), HashCreationErrorKind> {
        Self::from_path_inner(src_path)
    }

    const WHITE_PIXEL: [u8; 3] = [u8::MIN, u8::MIN, u8::MIN];
    const BLACK_PIXEL: [u8; 3] = [u8::MAX, u8::MAX, u8::MAX];
    pub fn spatial_thumbs(&self) -> Vec<RgbImage> {
        (0..self.num_frames)
            .map(|frame_no| {
                let mut frame_bits = self.hash[frame_no as usize];

                let mut frame = RgbImage::new(8, 8);
                for y in 0..8 {
                    for x in 0..8 {
                        frame.get_pixel_mut(x, y).0 = match frame_bits % 2 {
                            0 => Self::WHITE_PIXEL,
                            _ => Self::BLACK_PIXEL,
                        };
                        frame_bits = frame_bits.rotate_right(1);
                    }
                }

                frame
            })
            .collect()
    }

    pub fn temporal_thumbs(&self) -> Vec<RgbImage> {
        (0..self.num_frames - 1)
            .map(|frame_no| {
                let mut frame_bits = self.hash[SPATIAL_HASH_QWORDS..][frame_no as usize];

                let mut frame = RgbImage::new(8, 8);
                for y in 0..8 {
                    for x in 0..8 {
                        frame.get_pixel_mut(x, y).0 = match frame_bits % 2 {
                            0 => Self::WHITE_PIXEL,
                            _ => Self::BLACK_PIXEL,
                        };
                        frame_bits = frame_bits.rotate_right(1);
                    }
                }

                frame
            })
            .collect::<Vec<_>>()
    }
    pub fn reconstructed_thumbs(&self) -> Vec<RgbImage> {
        (0..self.num_frames)
            .map(|frame_no| {
                let mut frame_bits = self.hash[frame_no as usize];
                let mut frame = vec![0f64; 64];

                for y in 0..8 {
                    for x in 0..8 {
                        *frame.get_mut(x * y).unwrap() = (frame_bits % 2) as f64;

                        frame_bits = frame_bits.rotate_left(1);
                    }
                }

                frame
            })
            .map(|x| crate::utils::dct_ops::inverse_dct(&x))
            .map(|dynamic_image| dynamic_image.to_rgb8())
            .collect()
    }
}

//Utilities for testing
#[doc(hidden)]
pub mod test_util {

    use std::path::Path;

    use rand::prelude::*;

    use super::VideoHash;
    use crate::{definitions::HASH_NUM_IMAGES, video_hashing::video_hash::SPATIAL_HASH_QWORDS};

    #[doc(hidden)]
    impl VideoHash {
        pub fn with_duration(&self, duration: u32) -> Self {
            let mut ret = self.clone();
            ret.duration = duration;
            ret
        }

        pub fn with_src_path(&self, src_path: impl AsRef<Path>) -> Self {
            let mut ret = self.clone();
            ret.src_path = src_path.as_ref().to_path_buf();
            ret
        }

        pub fn full_hash(name: impl AsRef<Path>, num_frames: usize) -> Self {
            VideoHash::from_components(
                name,
                vec![vec![u64::MAX; 1]; num_frames],
                vec![vec![u64::MAX; 1]; num_frames - 1],
                0,
            )
            .unwrap()
        }

        pub fn empty_hash(name: impl AsRef<Path>, num_frames: usize) -> Self {
            VideoHash::from_components(
                name,
                vec![vec![u64::MIN; 1]; num_frames],
                vec![vec![u64::MIN; 1]; num_frames - 1],
                0,
            )
            .unwrap()
        }

        //generate a set of temporal hashes, each with a given distance from the empty hash.
        pub fn hash_with_spatial_distance(&self, target_distance: u32, rng: &mut StdRng) -> VideoHash {
            //for now, we will only support hashes where every frame is present.
            assert!(self.num_frames as usize == SPATIAL_HASH_QWORDS);

            fn flip_a_bit(bits: &mut [u64], rng: &mut StdRng) {
                let chosen_qword = rng.gen_range(0..bits.len());
                let chosen_bit = rng.gen_range(0..64);
                bits[chosen_qword] ^= 2u64.pow(chosen_bit);
            }

            //flip bits until the required distance is reached
            let mut ret_hash = self.clone();
            let mut curr_distance: u32 = self.levenshtein_distance(&ret_hash).u32_value();
            while curr_distance < target_distance {
                flip_a_bit(&mut ret_hash.hash, rng);
                curr_distance = self.levenshtein_distance(&ret_hash).u32_value();
            }
            assert!(self.levenshtein_distance(&ret_hash).u32_value() == target_distance);
            ret_hash
        }

        pub fn random_hash(rng: &mut StdRng) -> VideoHash {
            Self::random_hash_inner(rng, None)
        }

        pub fn random_hash_with_len(rng: &mut StdRng, len: usize) -> VideoHash {
            Self::random_hash_inner(rng, Some(len))
        }

        fn random_hash_inner(rng: &mut StdRng, num_frames_arg: Option<usize>) -> VideoHash {
            use std::path::PathBuf;

            use crate::video_hashing::video_hash::HASH_FRAME_QWORDS;

            let num_frames = num_frames_arg.unwrap_or_else(|| rng.gen_range(2..=HASH_NUM_IMAGES));

            let mut ret = VideoHash {
                hash: Default::default(),
                num_frames: num_frames as u32,
                src_path: PathBuf::from(""),
                duration: 0,
            };

            let shash_qwords = HASH_FRAME_QWORDS * num_frames;
            for val in ret.hash.iter_mut().take(shash_qwords) {
                *val = rng.gen();
            }

            let thash_qwords = HASH_FRAME_QWORDS * (num_frames - 1);
            for val in ret.hash.iter_mut().skip(shash_qwords).take(thash_qwords as usize - 1) {
                *val = rng.gen();
            }

            ret
        }
    }
}

#[cfg(test)]
mod test {
    use rand::prelude::*;

    use super::VideoHash;
    use crate::definitions::HASH_NUM_IMAGES;

    #[test]
    fn test_triangle_inequality() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(1);
        for _i in 0..1_000 {
            let thash1 = VideoHash::random_hash(&mut rng);
            let thash2 = VideoHash::random_hash(&mut rng);
            let thash3 = VideoHash::random_hash(&mut rng);

            let d12 = thash1.levenshtein_distance(&thash2);
            let d13 = thash1.levenshtein_distance(&thash3);
            let d23 = thash2.levenshtein_distance(&thash3);

            assert!(d12.distance <= d13.distance + d23.distance);
        }
    }
    #[test]
    fn test_distance_between_two_empty_hashes_is_0() {
        for num_frames in 2..=HASH_NUM_IMAGES {
            let empty_hash_1 = VideoHash::empty_hash("", num_frames);
            let empty_hash_2 = VideoHash::empty_hash("", num_frames);

            let dist = empty_hash_1.levenshtein_distance(&empty_hash_2);
            println!("{:#?}", dist);
            assert_eq!(0, dist.distance);
        }
    }

    #[test]
    fn test_distance_between_two_full_hashes_is_0() {
        for num_frames in 2..=HASH_NUM_IMAGES {
            let empty_hash_1 = VideoHash::full_hash("", num_frames);
            let empty_hash_2 = VideoHash::full_hash("", num_frames);

            let dist = empty_hash_1.levenshtein_distance(&empty_hash_2);
            assert_eq!(0, dist.distance + dist.distance);
        }
    }

    #[test]
    fn test_distance_between_empty_and_full_is_max() {
        let num_frames_range = 2..=HASH_NUM_IMAGES;
        let all_frame_len_combinations = num_frames_range.clone().zip(num_frames_range);

        for (len_1, len_2) in all_frame_len_combinations {
            let num_frames = len_1.max(len_2) as u32;
            let empty_hash = VideoHash::empty_hash("", len_1);
            let full_hash = VideoHash::full_hash("", len_2);

            let exp_spatial = 64 * num_frames;
            let exp_temporal = 64 * (num_frames - 1);

            let act = empty_hash.levenshtein_distance(&full_hash);

            assert_eq!(act.distance, exp_spatial + exp_temporal);
        }
    }

    #[test]
    fn test_distance_between_unequal_length_includes_length_difference() {
        //when two hashes are different, the raw distance should take into
        //account the difference in frame length by reporting maximum possible
        //distance for those frames
        let num_frames_range = 2..=HASH_NUM_IMAGES;
        let all_frame_len_combinations = num_frames_range.clone().zip(num_frames_range);

        for (len_1, len_2) in all_frame_len_combinations {
            let empty_hash_1 = VideoHash::empty_hash("", len_1);
            let empty_hash_2 = VideoHash::empty_hash("", len_2);

            let differing_frames = (len_1 as i32 - len_2 as i32).abs() as u32;
            let expected_spatial_diff = differing_frames * 64;
            let expected_temporal_diff = differing_frames * 64;

            let actual_distance = empty_hash_1.levenshtein_distance(&empty_hash_2);
            assert_eq!(actual_distance.distance, expected_spatial_diff + expected_temporal_diff);
        }
    }

    #[test]
    fn test_symmetry() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(2);
        for _i in 0..1_000 {
            let thash1 = VideoHash::random_hash(&mut rng);
            let thash2 = VideoHash::random_hash(&mut rng);

            assert_eq!(
                thash1.levenshtein_distance(&thash2),
                thash2.levenshtein_distance(&thash1)
            )
        }
    }
}
