//tweakable. Shorter numbers require less processing time to generate a hash.
// Ideally big numbers are more resistant to small offsets in time between duplicate videos.
pub const VID_HASH_DURATION: f64 = 15.0;

// The default time to skip forwards when before extracting video frames. Used to skip past
// title credits and/or overlays at the beginning of videos.
// Higher numbers extend hasing time (because seeking to this point in videos must be
// done accurately). Lower numbers risk not skipping far enough to avoid title credits etc.
pub const DEFAULT_VID_HASH_SKIP_FORWARD: f64 = 30.0;

//tweakable. Number of frames that the 3d DCT is performed on. Higher numbers extend hashing time
// but (hopefully) makes hashes more robust to small time offsets.
pub const DCT_SIZE: u32 = 64;

// Hash definitions
pub const HASH_SIZE: u32 = 8; //number of images

/// A good starting tolerance to use when searching for videos.
/// You can use a lower tolerance if you are getting too many false positives,
pub const DEFAULT_SEARCH_TOLERANCE: f64 = 0.30;

//At user-level the tolerance parameter is specified as real between 0 and 1.
//The is the scaling factor to map into the integer-domain being used for calculations.
pub const TOLERANCE_SCALING_FACTOR: f64 = (HASH_SIZE.pow(3)) as f64;

//Same as nightly's usize::unstable_div_ceil
const fn calc_qwords(num_bits: u32) -> u32 {
    let divisor = num_bits / 64;
    let remainder = num_bits % 64;
    if remainder == 0 {
        divisor
    } else {
        divisor + 1
    }
}

pub const HASH_BITS: u32 = HASH_SIZE.pow(3);
pub const HASH_QWORDS: u32 = calc_qwords(HASH_BITS);

//Whether to exclude black bars from the edges of videos
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, enum_utils::FromStr)]
pub enum CropdetectType {
    NoCrop,    //Include black bars on the edges of video frames in hashes.
    Letterbox, //Detect and remove black bars from the edges of video frames.
}
