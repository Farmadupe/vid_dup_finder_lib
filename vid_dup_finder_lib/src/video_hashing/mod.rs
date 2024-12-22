pub mod hash_creation_error_kind;
pub mod matches;
mod search_algorithm;
pub mod video_dup_finder;
pub mod video_hash;

mod dct_3d;

pub mod frame_extract_util;
mod raw_dct_ops;
pub mod video_hash_builder;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// An error that prevented a video hash from being created.
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum Error {
    /// File is not a video.
    #[error("File is not a video")]
    NotVideo,

    #[error("Video processing error: {0}")]
    VidProc(String),

    #[error("Could not extract enough frames")]
    NotEnoughFrames,
}
