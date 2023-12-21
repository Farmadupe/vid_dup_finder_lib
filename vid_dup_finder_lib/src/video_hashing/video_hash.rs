use std::{
    hash::Hash,
    num::NonZeroU32,
    path::{Path, PathBuf},
};
use vid_dup_finder_common::video_frames_gray::VdfFrameExt;

use bitvec::prelude::*;

use serde::{Deserialize, Serialize};

use vid_dup_finder_common::crop_resize_flat;
use vid_dup_finder_common::{video_frames_gray::LetterboxColour, Crop};

use crate::{
    definitions::{
        CropdetectType, DCT_SIZE, DEFAULT_VID_HASH_SKIP_FORWARD, HASH_BITS, HASH_QWORDS,
        VID_HASH_DURATION,
    },
    video_hashing::dct_3d::Dct3d,
    *,
};

use ffmpeg_gst_wrapper::ffmpeg_impl as ffmpeg_gst;

use ffmpeg_gst::{duration, FrameReaderCfgUnified, VideoFrameGrayUnified};
use image::GenericImageView;

/// If hashes created using the default settings do
/// not work for you, you can customize how input videos are processed
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct HashCreationOptions {
    /// The number of seconds from the beginning of the video that will be discarded
    /// If the video is shorter than this value plus the video-hash-duration, then
    /// no skipping will occur.
    pub skip_forward_amount: f64,

    /// The type of crop detection to be used.
    pub cropdetect: CropdetectType,
}

pub const DEFAULT_HASH_CREATION_OPTIONS: HashCreationOptions = HashCreationOptions {
    skip_forward_amount: DEFAULT_VID_HASH_SKIP_FORWARD,
    cropdetect: CropdetectType::Letterbox,
};

/// A hash of a video file, used for video duplicate detection. The hash contains information about
/// the first 30 seconds of a video, and also the duration. Searches will use these data to determine
/// similarity.

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct VideoHash {
    //#[serde(with = "BigArray")]
    hash: [u64; HASH_QWORDS],
    src_path: PathBuf,
    duration: u32,
}

impl Default for VideoHash {
    fn default() -> Self {
        Self {
            hash: [0; HASH_QWORDS],
            src_path: PathBuf::new(),
            duration: Default::default(),
        }
    }
}

impl VideoHash {
    ///Create a `VideoHash` from the video file at src_path, using default options.
    /// * Returns the hash itself if successful, otherwise
    /// * an error with the reason that the hash could not be created.
    pub fn from_path(src_path: impl AsRef<Path>) -> Result<Self, HashCreationErrorKind> {
        Self::from_path_with_options(src_path, DEFAULT_HASH_CREATION_OPTIONS)
    }

    /// Create a VideoHash with the given options. Only use this function if the default options
    /// do not work for you.
    pub fn from_path_with_options(
        src_path: impl AsRef<Path>,
        opts: HashCreationOptions,
    ) -> Result<Self, HashCreationErrorKind> {
        let src_path = src_path.as_ref().to_path_buf();
        let dct_size = NonZeroU32::try_from(DCT_SIZE as u32).unwrap();
        //println!("{src_path:?}");

        let iterate_frames =
            || Self::iterate_video_frames_inner(&src_path, opts.skip_forward_amount);

        //Before generating the hash, if requested we detect black bands around the edges of the video
        //frames, which will be discarded before generating the hash.

        let crop = match opts.cropdetect {
            CropdetectType::NoCrop => Self::detect_noletterbox_crop(iterate_frames()?)?,
            CropdetectType::Letterbox => {
                Self::detect_letterbox_crop(iterate_frames()?, opts.cropdetect)?
            }
        }.ok_or(HashCreationErrorKind::Other)?;

        let dct = {
            let frames_64x64 = iterate_frames()?
                .filter_map(|frame| crop_resize_flat(frame.as_flat(), dct_size, dct_size, crop));

            Dct3d::from_images(frames_64x64)
        };

        // Pack the raw bits of the hash into a bit vector.
        let mut bitarr: BitArray<[u64; HASH_QWORDS], Lsb0> = BitArray::ZERO;
        assert!(bitarr.len() >= HASH_BITS);
        for (mut bitarr_val, hash_bit) in bitarr.iter_mut().zip(dct.hash_bits()) {
            *bitarr_val = hash_bit;
        }

        //build the rest of the hash.
        let duration = duration(&src_path)
            .ok_or_else(|| HashCreationErrorKind::NotVideo(src_path.to_path_buf()))?
            .as_secs();

        let hash = Self::from_components(src_path, bitarr, duration as u32);

        Ok(hash)
    }

    /// Returns an iterator over frames of a video. The same frames that are used when building a hash.

    fn iterate_video_frames_inner(
        src_path: impl AsRef<Path>,
        skip_forward_amount: f64,
    ) -> Result<impl Iterator<Item = VideoFrameGrayUnified>, HashCreationErrorKind> {
        // let print_error_then_discard_it = |maybe_frame: Result<_. _>| match maybe_frame {
        //     Ok(frame) => Some(frame),
        //     Err(e) => {
        //         println!("error: {e:?}");
        //         None
        //     }
        // };

        let silently_discard_error = Result::ok;

        let ret = Self::build_frame_reader_inner(src_path.as_ref(), skip_forward_amount)?
            .spawn_gray()
            .map_err(|e| HashCreationErrorKind::VideoProcessing {
                src_path: src_path.as_ref().to_path_buf(),
                error: e,
            })?
            .filter_map(silently_discard_error);

        Ok(ret)
    }

    fn detect_noletterbox_crop(
        mut frames: impl Iterator<Item = VideoFrameGrayUnified>,
    ) -> Result<Option<Crop>, HashCreationErrorKind> {
        let dimensions = frames.next().unwrap().dimensions();

        Ok(Some(Crop::new(dimensions, 0, 0, 0, 0)))
    }

    fn detect_letterbox_crop(
        frames: impl Iterator<Item = VideoFrameGrayUnified>,
        crop_type: CropdetectType,
    ) -> Result<Option<Crop>, HashCreationErrorKind> {
        // we don't need all of the frames to detect the crop. (this isn't a huge speedup because gstreamer still
        // decodes every frame)
        let frames = frames.step_by(8).take(8);

        // Given an existing crop and another video frame, return the union of that crop
        // and the frame's detected letterboxing.
        let add_letterbox_crop_frame = |crop: Option<Crop>, frame: VideoFrameGrayUnified| {
            let this_frame_crop = frame.letterbox_crop(LetterboxColour::AnyColour(16));
            match crop {
                Some(crop) => Some(crop.union(&this_frame_crop)),
                None => Some(this_frame_crop),
            }
        };

        match crop_type {
            CropdetectType::NoCrop => Ok(None), // should be unreachable.
            CropdetectType::Letterbox => {
                let ret = frames.fold(None, add_letterbox_crop_frame);
                Ok(ret)
            }
        }
    }

    /// utility helper -- Build a gstreamer frame reader with the correct
    /// configuration for building temporal hashes.
    ///
    /// If the video is longer than 30 seconds, then frames from only the first 30 seconds
    /// of the video will be used. If it is shorter then frames evenly spaced across the length
    /// of the video will be used instead.
    ///
    fn build_frame_reader_inner(
        src_path: impl AsRef<Path>,
        skip_forward_amount: f64,
    ) -> Result<FrameReaderCfgUnified, HashCreationErrorKind> {
        let src_path = src_path.as_ref();

        // The video duration influcences the exact frames chosen to build the hash
        let vid_duration = duration(src_path)
            .ok_or_else(|| HashCreationErrorKind::NotVideo(src_path.to_path_buf()))?
            .as_secs_f64();

        let max_seek_amount = skip_forward_amount;
        let max_hash_duration = VID_HASH_DURATION;

        let fps;
        let seek_amount;

        // If the video is really short then set the FPS really high and
        // try get whatever frames are available. This might not succeed
        // because a really short video might not have 64 total frames,
        //
        // But don't sweat over this corner because for degernerately short videos
        // a duplicate-image utility might work just as well instead.
        if vid_duration < 2.0 {
            fps = 64.0;
            seek_amount = 0f64;

        //Otherwise if the video is shorter than the desired runtime for building
        //a hash, set the FPS to evenly sample frames across the length of the video
        //
        //(But to avoid the effect of cumulative rounding errors, try and make
        //the last frame be 2 seconds before the end. Otherwise sometimes we only
        //get 63 frames.)
        } else if vid_duration < max_hash_duration {
            fps = 64.0 / (vid_duration - 2.0);
            seek_amount = 0f64;

        //If the video is long enough to sample max_hash_duration's worth of content
        //to build the hash, but not long enough that we can apply the full skip,
        //then skip forwards as far as possible.
        } else if vid_duration < max_seek_amount + max_hash_duration {
            fps = 64.0 / max_hash_duration;
            seek_amount = vid_duration - max_hash_duration - 2.0;

        //Otherwise the video is long enough to do what we want.
        } else {
            fps = 64.0 / max_hash_duration;
            seek_amount = max_seek_amount;
        }

        //gstreamer expects framerates to be expressed as integer fractions, so
        //scale the float framerate by a large number and convert to integer.
        let fps = ((fps * 16384.0) as u64, 16384);

        //Spawn gstreamer pipeline to begin getting video frames.
        let mut builder = FrameReaderCfgUnified::from_path(src_path);
        builder.fps(fps);
        if seek_amount > 0f64 {
            builder.start_offset(seek_amount);
        }

        Ok(builder)
    }

    fn from_components(
        src_path: impl AsRef<Path>,
        hash_bits: BitArray<[u64; HASH_QWORDS], Lsb0>,
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

    /// The duration of the video from which this hash was created.
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

#[cfg(feature = "app_only_fns")]
impl VideoHash {
    pub fn build_frame_reader(
        src_path: impl AsRef<Path>,
        skip_forward_amount: f64,
    ) -> Result<FrameReaderCfgUnified, HashCreationErrorKind> {
        Self::build_frame_reader_inner(src_path, skip_forward_amount)
    }

    pub fn iterate_video_frames(
        src_path: impl AsRef<Path>,
        skip_forward_amount: f64,
    ) -> Result<impl Iterator<Item = VideoFrameGrayUnified>, HashCreationErrorKind> {
        Self::iterate_video_frames_inner(src_path, skip_forward_amount)
    }

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

        let full_raw_slice = BitSlice::<u64, Lsb0>::from_slice(&self.hash);
        let correct_size_slice = &full_raw_slice[..HASH_BITS];

        correct_size_slice.iter().by_vals()
    }

    /// Construct an image from the raw bits of the hash by performing the reversing the steps
    /// that produced the hash. The resulting image may be recognizable.
    #[must_use]
    pub fn reconstructed_frames(&self) -> Vec<image::RgbImage> {
        video_hashing::dct_3d::Dct3d::image_from_video_hash(self)
    }

    #[must_use]
    pub const fn hash_frame_dimensions() -> (usize, usize) {
        use crate::definitions::HASH_SIZE;
        (HASH_SIZE, HASH_SIZE)
    }

    #[must_use]
    pub fn hash_bits(&self) -> &BitSlice<u64, Lsb0> {
        &BitSlice::from_slice(&self.hash)[..HASH_BITS]
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
    use crate::video_hashing::video_hash::{HASH_BITS, HASH_QWORDS};
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
            Self::from_components(name, BitArray::new([u64::MAX; HASH_QWORDS]), 0)
        }

        pub fn empty_hash(name: impl AsRef<Path>) -> Self {
            Self::from_components(name, BitArray::ZERO, 0)
        }

        //generate a set of temporal hashes, each with a given distance from the empty hash.
        #[must_use]
        pub fn hash_with_spatial_distance(&self, target_distance: u32, rng: &mut StdRng) -> Self {
            fn flip_a_bit(bits: &mut [u64], rng: &mut StdRng) {
                let chosen_qword = rng.gen_range(0..bits.len());
                let chosen_bit = rng.gen_range(0..64);
                bits[chosen_qword] ^= 2u64.pow(chosen_bit);
            }

            //flip bits until the required distance is reached
            let mut ret_hash = self.clone();
            let mut curr_distance = self.hamming_distance(&ret_hash);
            while curr_distance < target_distance {
                flip_a_bit(&mut ret_hash.hash, rng);
                curr_distance = self.hamming_distance(&ret_hash);
            }
            assert!(self.hamming_distance(&ret_hash) == target_distance);
            ret_hash
        }

        pub fn random_hash(rng: &mut StdRng) -> Self {
            Self::random_hash_inner(rng)
        }

        fn random_hash_inner(rng: &mut StdRng) -> Self {
            use std::path::PathBuf;

            let mut hash: BitArray<[u64; HASH_QWORDS], Lsb0> = BitArray::ZERO;
            for mut bit in hash.iter_mut().take(HASH_BITS) {
                *bit = rng.gen_bool(0.5);
            }

            Self {
                hash: hash.into_inner(),
                src_path: PathBuf::from(""),
                duration: 0,
            }
        }
    }
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

//Utility helper: Get the hamming distance between two bitstrings.
fn hamming_distance<const N: usize>(x: &[u64; N], y: &[u64; N]) -> u32 {
    x.iter().zip(y.iter()).fold(0, |acc, (x, y)| {
        let difference = x ^ y;
        let set_bits = difference.count_ones();
        acc + set_bits
    })
}
