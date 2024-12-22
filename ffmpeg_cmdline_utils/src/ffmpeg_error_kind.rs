use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;

use crate::*;

/// Various causes of failure for ffmpeg/ffprobe functions.
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum FfmpegError {
    /// Ffmpeg/Ffprobe command was not found. Make sure Ffmpeg is installed and can be found on the command line.
    #[error("ffmpeg/ffprobe file not found. Make sure ffmpeg/ffprobe are installed and visible on the command line")]
    FfmpegNotFound,

    /// Io error occurred while executing Ffmpeg/Ffprobe command
    #[error("Ffmpeg IO error")]
    Io(String),

    /// Ffmpeg/Ffprobe returned a nonzero exit code. Because ffmpeg sometimes prints long error strings
    /// to stderr, The resulting string contains the first few hundred characters of the error message.
    #[error("Internal Ffmpeg Failure: {0}")]
    FfmpegInternal(String),

    /// Failed to interpret Ffmpeg/Ffprobe output as a utf8-string.
    #[error("utf8 parsing/conversion failure")]
    Utf8Conversion,

    /// When using Ffprobe to obtain the resolution of the video file before beginning the
    /// decoding process, either the X or Y dimensions was zero.
    /// Note: This sometimes occur when attempting to decode frames from an audio file.
    #[error("Ffmmpeg decoded no frames from the video")]
    InvalidResolution,

    /// Failed to obtain video information.
    #[error("Failed to get video properties")]
    Info(#[from] VideoInfoError),
}
