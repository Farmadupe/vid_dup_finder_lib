use std::{
    hash::Hash,
    num::NonZeroU32,
    path::{Path, PathBuf},
};

use bitvec::prelude::*;

use serde::{Deserialize, Serialize};

use vid_dup_finder_common::crop_resize_buf;
use vid_dup_finder_common::Crop;

use crate::{
    definitions::{DCT_SIZE, HASH_BITS, HASH_WORDS},
    video_hashing::dct_3d::Dct3d,
    Error::NotEnoughFrames,
};

use image::GrayImage;

/// A hash of a video file, used for video duplicate detection. The hash contains information about
/// the first 30 seconds of a video, and also the duration. Searches will use these data to determine
/// similarity.

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct VideoHash {
    //#[serde(with = "BigArray")]
    hash: [usize; HASH_WORDS as usize],
    src_path: PathBuf,
    duration: u32,
}

impl Default for VideoHash {
    fn default() -> Self {
        Self {
            hash: [0; HASH_WORDS as usize],
            src_path: PathBuf::new(),
            duration: Default::default(),
        }
    }
}

impl VideoHash {
    pub(crate) fn from_frames(
        frames: impl Clone + IntoIterator<Item = GrayImage>,
        src_path: PathBuf,
        duration: u32,
    ) -> Result<Self, crate::Error> {
        let dct_size = NonZeroU32::try_from(DCT_SIZE).expect("will not be nonzero");

        let mut it = frames.into_iter().peekable();
        let first_frame = it.peek().ok_or(NotEnoughFrames)?;
        let width = first_frame.width();
        let height = first_frame.height();

        let no_crop = Crop::from_edge_offsets((width, height), 0, 0, 0, 0);

        let frames_64x64 = it.map(|frame| crop_resize_buf(frame, dct_size, dct_size, no_crop));

        let dct = Dct3d::from_images(frames_64x64).ok_or(NotEnoughFrames)?;

        // Pack the raw bits of the hash into a bit vector.
        let mut bitarr: BitArray<[usize; HASH_WORDS as usize], Lsb0> = BitArray::ZERO;
        assert!(bitarr.len() >= HASH_BITS as usize);
        for (mut bitarr_val, hash_bit) in bitarr.iter_mut().zip(dct.hash_bits()) {
            *bitarr_val = hash_bit;
        }

        let hash = Self::from_components(src_path, bitarr, duration);

        Ok(hash)
    }

    /////Create a `VideoHash` from the video file at src_path, using default options.
    ///// * Returns the hash itself if successful, otherwise
    ///// * an error with the reason that the hash could not be created.
    ///// # Errors
    ///// Returns `Err` if the video file does not exist, is not a video, is too short, or has no video frames.
    ///// # Panics
    ///// Should only panic due to internal implementation error
    // pub fn from_path(src_path: impl AsRef<Path>) -> Result<Self, HashCreationErrorKind> {
    //     Self::from_path_with_options(src_path, DEFAULT_HASH_CREATION_OPTIONS)
    // }

    ///// Create a VideoHash with the given options. Only use this function if the default options
    ///// do not work for you.
    /////
    ///// # Errors
    ///// Returns `Err` if the video file does not exist, is not a video, is too short, or has no video frames.
    /////
    ///// # Panics
    ///// Should only panic due to internal implementation error
    // pub fn from_path_with_options(
    //     src_path: impl AsRef<Path>,
    //     opts: HashCreationOptions,
    // ) -> Result<Self, HashCreationErrorKind> {

    //     let src_path = src_path.as_ref().to_path_buf();
    //     let dct_size = NonZeroU32::try_from(DCT_SIZE).expect("will not be nonzero");
    //     //println!("{src_path:?}");

    //     //iterate the frames of a video file
    //     //also checks that there is at least one frame and bails early if there isn't
    //     let iterate_frames = || {
    //         let it = frame_extract_util::iterate_video_frames(&src_path, opts.skip_forward_amount)?;

    //         Ok(it.map(Rc::new))
    //     };

    //     //Before generating the hash, if requested we detect black bands around the edges of the video
    //     //frames, which will be discarded before generating the hash.

    //     //cache frames (faster) or stream them (significantly less memory)
    //     let frames = iterate_frames()?.collect::<Vec<_>>();
    //     let crop = detect_crop(frames.clone(), opts.cropdetect)
    //         .ok_or_else(|| HashCreationErrorKind::NotEnoughFrames(src_path.clone()))?;

    //     let frames_64x64 = frames
    //         .iter()
    //         .filter_map(|frame| crop_resize_flat(frame.as_flat(), dct_size, dct_size, crop));

    //     let dct = Dct3d::from_images(frames_64x64)
    //         .ok_or_else(|| HashCreationErrorKind::NotEnoughFrames(src_path.to_path_buf()))?;

    //     #[allow(clippy::print_stdout)]
    //     #[allow(clippy::print_stderr)]
    //     if CropdetectType::MotionDebug == opts.cropdetect {
    //         println!(
    //             "croppadd{{croptop={},cropbottom={},cropleft={},cropright={}}}",
    //             crop.top, crop.bottom, crop.left, crop.right
    //         );
    //         eprintln!(
    //             "crop:{}:{}:{}:{}:",
    //             crop.top, crop.right, crop.bottom, crop.left
    //         );
    //     }

    //     #[allow(clippy::print_stdout)]
    //     if CropdetectType::MotionDebug2 == opts.cropdetect {
    //         println!(
    //             "crop:{}:{}:{}:{}:",
    //             crop.top, crop.right, crop.bottom, crop.left
    //         );
    //     }

    //     // Pack the raw bits of the hash into a bit vector.
    //     let mut bitarr: BitArray<[u64; HASH_QWORDS as usize], Lsb0> = BitArray::ZERO;
    //     assert!(bitarr.len() >= HASH_BITS as usize);
    //     for (mut bitarr_val, hash_bit) in bitarr.iter_mut().zip(dct.hash_bits()) {
    //         *bitarr_val = hash_bit;
    //     }

    //     //build the rest of the hash.
    //     let duration_secs = duration(&src_path)
    //         .ok_or_else(|| HashCreationErrorKind::NotVideo(src_path.clone()))?
    //         .as_secs() as u32;

    //     let hash = Self::from_components(src_path, bitarr, duration_secs);

    //     Ok(hash)
    // }

    fn from_components(
        src_path: impl AsRef<Path>,
        hash_bits: BitArray<[usize; HASH_WORDS as usize], Lsb0>,
        duration: u32,
    ) -> Self {
        Self {
            hash: hash_bits.into_inner(),
            src_path: src_path.as_ref().to_owned(),
            duration,
        }
    }

    /// The path to the video file from which this hash was created.
    #[must_use]
    pub fn src_path(&self) -> &Path {
        &self.src_path
    }

    /// The duration in seconds of the video.
    #[must_use]
    pub const fn duration(&self) -> u32 {
        self.duration
    }

    /// The raw haming distance from this hash to another hash.
    #[must_use]
    pub fn hamming_distance(&self, other: &Self) -> u32 {
        hamming_distance(&self.hash, &other.hash)
    }
}

#[doc(hidden)]
#[cfg(feature = "app_only_fns")]
impl VideoHash {
    /// The distance from this hash to another hash, but normalized into the range 0..=1
    #[must_use]
    pub fn normalized_hamming_distance(&self, other: &Self) -> f64 {
        let raw_distance = f64::from(self.hamming_distance(other));

        raw_distance / crate::definitions::TOLERANCE_SCALING_FACTOR
    }

    /// An iterator over the raw bits of the hash, The bits are returned in an arbitrary order
    /// which is not guaranteed (this fn exists for debug and visualization purposes only.)
    pub fn raw_hash(&self) -> impl Iterator<Item = bool> + '_ {
        // the self hash field may have some unused bits at the end, which we do not want
        // to return, however it doesn't seem to be possible to construct a BitVec from a
        // slice with a non-multiple-of-the-raw-storage-size length

        let full_raw_slice = BitSlice::<usize, Lsb0>::from_slice(&self.hash);
        let correct_size_slice = &full_raw_slice[..HASH_BITS as usize];

        correct_size_slice.iter().by_vals()
    }

    #[must_use]
    pub const fn hash_frame_dimensions() -> (usize, usize) {
        use crate::definitions::HASH_SIZE;
        (HASH_SIZE as usize, HASH_SIZE as usize)
    }

    #[must_use]
    pub fn hash_bits(&self) -> &BitSlice<usize, Lsb0> {
        &BitSlice::from_slice(&self.hash)[..HASH_BITS as usize]
    }
}

impl AsRef<Self> for VideoHash {
    fn as_ref(&self) -> &Self {
        self
    }
}

//Utilities for testing
#[doc(hidden)]
//#[cfg(any(feature = "test-util", test))]
pub mod test_util {

    use std::path::Path;

    use super::VideoHash;
    use crate::video_hashing::video_hash::{HASH_BITS, HASH_WORDS};
    use bitvec::prelude::*;
    use rand::prelude::*;

    #[doc(hidden)]
    impl VideoHash {
        #[must_use]
        pub fn with_duration(&self, duration: u32) -> Self {
            let mut ret = self.clone();
            ret.duration = duration;
            ret
        }

        #[must_use]
        pub fn with_src_path(&self, src_path: impl AsRef<Path>) -> Self {
            let mut ret = self.clone();
            ret.src_path = src_path.as_ref().to_path_buf();
            ret
        }

        pub fn full_hash(name: impl AsRef<Path>) -> Self {
            Self::from_components(name, BitArray::new([usize::MAX; HASH_WORDS as usize]), 0)
        }

        pub fn empty_hash(name: impl AsRef<Path>) -> Self {
            Self::from_components(name, BitArray::ZERO, 0)
        }

        //generate a set of temporal hashes, each with a given distance from the empty hash.
        #[must_use]
        pub fn hash_with_spatial_distance(&self, target_distance: u32, rng: &mut StdRng) -> Self {
            let mut flip_a_bit = |bits: &mut [usize]| {
                let chosen_qword = rng.random_range(0..bits.len());
                let chosen_bit = rng.random_range(0..usize::BITS);
                bits[chosen_qword] ^= 2usize.pow(chosen_bit);
            };

            //flip bits until the required distance is reached
            let mut ret_hash = self.clone();
            let mut curr_distance = self.hamming_distance(&ret_hash);
            while curr_distance < target_distance {
                flip_a_bit(&mut ret_hash.hash);
                curr_distance = self.hamming_distance(&ret_hash);
            }
            assert!(self.hamming_distance(&ret_hash) == target_distance);
            ret_hash
        }

        pub fn random_hash(rng: &mut StdRng) -> Self {
            use std::path::PathBuf;

            let mut hash: BitArray<[usize; HASH_WORDS as usize], Lsb0> = BitArray::ZERO;
            for mut bit in hash.iter_mut().take(HASH_BITS as usize) {
                *bit = rng.random_bool(0.5);
            }

            Self {
                hash: hash.into_inner(),
                src_path: PathBuf::from(""),
                duration: 0,
            }
        }
    }
}

//Utility helper: Get the hamming distance between two bitstrings.
fn hamming_distance<const N: usize>(x: &[usize; N], y: &[usize; N]) -> u32 {
    x.iter().zip(y.iter()).fold(0, |acc, (x, y)| {
        let difference = x ^ y;
        let set_bits = difference.count_ones();
        acc + set_bits
    })
}

#[cfg(test)]
mod test {
    use rand::prelude::*;

    use super::VideoHash;

    #[test]
    fn test_triangle_inequality() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(1);
        for _i in 0..1_000 {
            let thash1 = VideoHash::random_hash(&mut rng);
            let thash2 = VideoHash::random_hash(&mut rng);
            let thash3 = VideoHash::random_hash(&mut rng);

            let d12 = thash1.hamming_distance(&thash2);
            let d13 = thash1.hamming_distance(&thash3);
            let d23 = thash2.hamming_distance(&thash3);

            assert!(d12 <= d13 + d23);
        }
    }
    #[test]
    fn test_distance_between_two_empty_hashes_is_0() {
        let empty_hash_1 = VideoHash::empty_hash("");
        let empty_hash_2 = VideoHash::empty_hash("");

        let dist = empty_hash_1.hamming_distance(&empty_hash_2);
        //println!("{:#?}", dist);
        assert_eq!(0, dist);
    }

    #[test]
    fn test_distance_between_two_full_hashes_is_0() {
        let empty_hash_1 = VideoHash::full_hash("");
        let empty_hash_2 = VideoHash::full_hash("");

        let dist = empty_hash_1.hamming_distance(&empty_hash_2);
        assert_eq!(0, dist);
    }

    #[test]
    fn test_symmetry() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(2);
        for _i in 0..1_000 {
            let thash1 = VideoHash::random_hash(&mut rng);
            let thash2 = VideoHash::random_hash(&mut rng);

            assert_eq!(
                thash1.hamming_distance(&thash2),
                thash2.hamming_distance(&thash1)
            );
        }
    }
}
