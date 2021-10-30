use ffmpeg_cmdline_utils::*;
use serde::{Deserialize, Serialize};

/// Additional Information about a video file, primarily for use with the example app.
/// Intended to be used to guide
/// choices about deleting/preserving duplicate videos. This struct
/// is intended to be used with the GUI portion of the example app.
#[doc(hidden)]
#[derive(PartialEq, Clone, Debug, Default, Serialize, Deserialize)]
pub struct VideoStats {
    cmdline_stats: VideoInfo,
    pub png_size: u32,
}

impl VideoStats {
    pub fn new(cmdline_stats: VideoInfo, png_size: u32) -> Self {
        Self {
            cmdline_stats,
            png_size,
        }
    }

    pub fn duration(&self) -> f64 {
        self.cmdline_stats.duration()
    }
    pub fn size(&self) -> u64 {
        self.cmdline_stats.size()
    }
    pub fn bit_rate(&self) -> u32 {
        self.cmdline_stats.bit_rate()
    }
    pub fn resolution(&self) -> (u32, u32) {
        self.cmdline_stats.resolution()
    }
    pub fn has_audio(&self) -> bool {
        self.cmdline_stats.has_audio()
    }
    pub fn png_size(&self) -> u32 {
        self.png_size
    }
}
