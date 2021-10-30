// Frame definitions (pre hashing)
pub const RESIZE_IMAGE_X: u32 = 32;
pub const RESIZE_IMAGE_Y: u32 = 32;

// Hash definitions
pub const HASH_NUM_IMAGES: usize = 10;
pub const HASH_IMAGE_X: usize = 8;
pub const HASH_IMAGE_Y: usize = 8;
pub const HASH_FRAMERATE: &str = "1/3";

//At user-level the tolerance parameter is specified as real between 0 and 1.
//The is the scaling factor to map into the integer-domain being used for calculations.
pub const TOLERANCE_SCALING_FACTOR: f64 =
    (HASH_IMAGE_X * HASH_IMAGE_Y * (HASH_NUM_IMAGES + HASH_NUM_IMAGES - 1)) as f64;
