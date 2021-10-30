use std::path::PathBuf;

use ffmpeg_cmdline_utils::*;
use thiserror::Error;

use serde::{Deserialize, Serialize};

/// Error type for the various reasons why a VideoHash could not be created from a video file.
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum HashCreationErrorKind {
    /// It was not possible to determine whether the file at src_path was a video or not.
    /// Implementaiton note: The command line tool Ffprobe is responsible for determining whether a file
    /// is a video. This error will be returned whenever ffprobe returns a nonzero exit status.
    #[error("Failed to determine whether file is a video: {src_path}")]
    DetermineVideo { src_path: PathBuf, error: FfmpegErrorKind },

    /// The supplied video was too short. Hashes can only be generated from files that are longer
    /// than 30 seconds.
    #[error("Too short: {0}")]
    VideoLength(PathBuf),

    /// The Ffmpeg command line tool encountered an error while extracting frames from the video.
    #[error("Processing error at {src_path}: {error}")]
    VideoProcessing { src_path: PathBuf, error: FfmpegErrorKind },
}
