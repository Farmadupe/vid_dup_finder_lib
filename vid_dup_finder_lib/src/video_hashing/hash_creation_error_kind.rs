use std::path::PathBuf;

use thiserror::Error;

use ffmpeg_gst::FfmpegGstError;
use ffmpeg_gst_wrapper::ffmpeg_impl as ffmpeg_gst;

use serde::{Deserialize, Serialize};

/// An error that prevented a video hash from being created.
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum HashCreationErrorKind {
    /// File is not a video.
    #[error("File is not a video: {0}")]
    NotVideo(PathBuf),

    /// Error occurred while processing video.
    #[error("Video processing error at {src_path}: {error}")]
    VideoProcessing {
        src_path: PathBuf,
        error: FfmpegGstError,
    },
}
