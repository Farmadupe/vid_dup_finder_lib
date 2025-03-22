/// The default tolerance when performing searches. A value of 0.0 means videos will get paired
/// only if their hashes are identical. A value of 1.0 means a video hash will match any other.
/// Reccomend to start with a high value e.g 0.35 and to lower it if there are too many false
/// positives
pub const DEFAULT_SEARCH_TOLERANCE: f64 = 0.35;

/// The default time to skip forwards when before extracting video frames. Used to skip past
/// title credits and/or overlays at the beginning of videos.
/// Higher numbers extend hasing time (because seeking to this point in videos must be
/// done accurately). Lower numbers risk not skipping far enough to avoid title credits etc.
///
/// If any video is shorter than this duration, then the amount skipped will be reduced to ensure
/// a hash can be generated.
///
/// Unit: Seconds
///
/// Reccomended range: 0-300.
pub const DEFAULT_VID_HASH_SKIP_FORWARD: f64 = 15.0;

/// The default time at the start of the video to generate hashes from.
/// Lower values speed up the hashing process because less video data needs to be extracted.
/// Higher values produce slightly more reliable hashes.
///
/// If any video is shorter than this duration, then hashes will be generated from the entire video.
///
/// Unit: Seconds
///
/// reccomended range: 2-60.
pub const DEFAULT_VID_HASH_DURATION: f64 = 10.0;

//tweakable. Number of frames that the 3d DCT is performed on. Higher numbers extend hashing time
// but (hopefully) makes hashes more robust to small time offsets.
//This generates a cube of DCT_SIZExDCT_SIZExDCT_SIZE bits, of which the HASH_SIZE cube MSBs will be taken
pub const DCT_SIZE: u32 = 16;

#[cfg(any(
    all(feature = "hash_size_6", not(feature = "hash_size_10")),
    all(not(feature = "hash_size_6"), not(feature = "hash_size_10"))
))]
pub const HASH_SIZE: u32 = 6;

#[cfg(all(not(feature = "hash_size_6"), feature = "hash_size_10"))]
pub const HASH_SIZE: u32 = 10;

#[cfg(all(feature = "hash_size_6", feature = "hash_size_10"))]
compile_error!("features 'hash_size_6' and 'hash_size_10' cannot be selected at the same time");

//At user-level the tolerance parameter is specified as real between 0 and 1.
//The is the scaling factor to map into the integer-domain being used for calculations.
pub const TOLERANCE_SCALING_FACTOR: f64 = (HASH_SIZE.pow(3)) as f64;

pub const HASH_BITS: u32 = HASH_SIZE.pow(3);
pub const HASH_WORDS: u32 = HASH_BITS.div_ceil(usize::BITS);

/// Algorithms to detect [black bars](https://en.wikipedia.org/wiki/Letterboxing_(filming))  around the edges of video frames
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, enum_utils::FromStr)]
pub enum Cropdetect {
    /// Do not detect letterboxing
    None,
    /// Detect letterboxes around the edges of videos (top, bottom, left, right)
    Letterbox,
    /// Detect regions of videos that contain motion
    Motion,
}
