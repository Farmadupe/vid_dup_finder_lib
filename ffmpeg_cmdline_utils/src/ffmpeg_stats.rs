use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::*;

#[derive(Debug, Deserialize, Serialize, Clone, Error)]
pub enum VideoInfoError {
    #[error("Error parsing stats: {0}")]
    JsonError(String),
    #[error("Error parsing stats: {0}")]
    ParseIntError(String),
    #[error("Error parsing stats: {0}")]
    ParseFloatError(String),
}

impl From<serde_json::Error> for VideoInfoError {
    fn from(e: serde_json::Error) -> Self {
        //limit maximum number of characters
        let error_string = format!("{e}").chars().take(500).collect::<String>();
        VideoInfoError::JsonError(error_string)
    }
}

impl From<std::num::ParseIntError> for VideoInfoError {
    fn from(e: std::num::ParseIntError) -> Self {
        VideoInfoError::ParseIntError(format!("{e}"))
    }
}

impl From<std::num::ParseFloatError> for VideoInfoError {
    fn from(e: std::num::ParseFloatError) -> Self {
        VideoInfoError::ParseFloatError(format!("{e}"))
    }
}

// There is a slighty gotcha in ffmpeg where if the video metadata declares a rotation,
// raw (x, y) resolution in that metadata refers to the "unrotated" resolution. we must
// therefore swap the x and y values if the rotation is 90 or 270
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug, Copy, Serialize, Deserialize, Hash)]
enum FfmpegVideoRotation {
    Rot0,
    Rot90,
    Rot180,
    Rot270,
}
use FfmpegVideoRotation::*;

impl Default for FfmpegVideoRotation {
    fn default() -> Self {
        Self::Rot0
    }
}

/// Some of the video metadata that can be obtained by using ffprobe.
#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize, Default)]
pub struct VideoInfo {
    duration: std::time::Duration,
    file_size: u64,
    resolution: (u32, u32),
}

impl VideoInfo {
    /// Use ffprobe to get the duration and resolution of a video. If the video contains multiple streams then only information
    /// about the first stream will be returned.
    ///
    /// # errors
    /// * The file cannot be read or is not recognized as a video by ffprobe
    /// * The output from ffprobe could not be parsed as JSON
    /// * The output from ffprobe did not contain all expected fields.
    pub fn new<P>(src_path: P) -> Result<Self, FfmpegError>
    where
        P: AsRef<Path>,
    {
        let stats_string = get_video_stats(&src_path)?;

        let stats_parsed: Value =
            serde_json::from_str(&stats_string).map_err(VideoInfoError::from)?;

        let duration = if let Value::String(d) = &stats_parsed["format"]["duration"] {
            std::time::Duration::from_secs_f64(d.parse().map_err(VideoInfoError::from)?)
        } else {
            std::time::Duration::from_secs_f64(0.0)
        };

        let file_size = if let Value::String(s) = &stats_parsed["format"]["size"] {
            s.parse().map_err(VideoInfoError::from)?
        } else {
            0
        };

        // If the video metadata declares that a video is rotated, then FFMPEG will conveniently autorotate
        // each frame for us, however we will have to remember to swap around x and y axis if the rotation is
        // 90 or 270
        let rotation = {
            //extract the rotation from the JSON
            let rotation = Self::first_video(&stats_parsed).and_then(|video_stream| {
                video_stream
                    .get("side_data_list")
                    .and_then(|y| y.get(0).and_then(|x| x.get("rotation").cloned()))
            });

            //if the rotation is found, it may either be a JSON String or JSON number, so unify
            //them here.
            let rotation = rotation.map(|rotation| match rotation {
                Value::Number(val) => val.as_i64().unwrap(),
                Value::String(val) => val.parse::<i64>().unwrap(),
                _ => panic!(
                    "got invalid json value type for video rotation. Expected: String or Number"
                ),
            });

            //now make sure that the value is one of the four cardinal directions and return it
            //(or if no rotation is specified, return 0/360)
            match rotation {
                None => FfmpegVideoRotation::Rot0,
                Some(0) => FfmpegVideoRotation::Rot0,
                Some(90) | Some(-270) => FfmpegVideoRotation::Rot90,
                Some(180) | Some(-180) => FfmpegVideoRotation::Rot180,
                Some(-90) | Some(270) => FfmpegVideoRotation::Rot270,
                Some(_) => panic!(
                    "ffprobe failure. Got unexpected rotation. src_path: {}, rotation: {:?}",
                    src_path.as_ref().display(),
                    rotation
                ),
            }
        };

        let resolution = {
            let first_width = Self::first_vid_u32(&stats_parsed, "width").unwrap_or(0);
            let first_height = Self::first_vid_u32(&stats_parsed, "height").unwrap_or(0);

            if matches!(rotation, Rot0 | Rot180) {
                (first_width, first_height)
            } else {
                (first_height, first_width)
            }
        };

        Ok(VideoInfo {
            duration,
            file_size,
            resolution,
        })
    }

    /// The duration of the video in seconds
    pub fn duration(&self) -> std::time::Duration {
        self.duration
    }

    /// The size of the video in bytes
    pub fn file_size(&self) -> u64 {
        self.file_size
    }

    /// The resolution of the video in pixels.
    /// Note the returned value is correct for the orientation that the video is intended
    /// to be viewed. (Ffprobe returns a surprising value by default if the video is stored rotated)
    pub fn resolution(&self) -> (u32, u32) {
        self.resolution
    }

    fn first_video(stats_parsed: &Value) -> Option<&Value> {
        Self::streams_of_type(stats_parsed, "video").and_then(|mut videos| videos.drain(..).next())
    }

    fn streams_of_type<'a>(stats_parsed: &'a Value, stream_type: &str) -> Option<Vec<&'a Value>> {
        if let Value::Array(streams) = &stats_parsed["streams"] {
            let ret = streams
                .iter()
                .filter(|s| match &s["codec_type"] {
                    Value::String(codec_type) => codec_type == stream_type,
                    _ => false,
                })
                .collect();

            Some(ret)
        } else {
            None
        }
    }

    fn first_vid_u32(stats_parsed: &Value, field_name: &str) -> Option<u32> {
        let video_streams = Self::streams_of_type(stats_parsed, "video")?;

        let all_matched_values = video_streams
            .iter()
            .filter_map(|stream| {
                if let Value::Number(v) = &stream[field_name] {
                    Some(v.as_u64()? as u32)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        all_matched_values.iter().cloned().next()
    }
}
