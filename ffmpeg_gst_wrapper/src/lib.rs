use std::{fmt::Debug, path::Path, time::Duration};

use image::{GrayImage, RgbImage};

//#[cfg(all(feature = "ffmpeg_backend", feature = "gstreamer_backend"))]
//compile_error!("feature \"ffmpeg_backend\" and feature \"gstreamer_backend\" cannot be enabled at the same time");

pub trait FrameReadCfgTrait {
    type E: Debug + std::error::Error;

    fn from_path(src_path: &Path) -> Self;
    fn get_duration(&self) -> Result<Duration, Self::E>;
    fn get_resolution(&self) -> Result<(u32, u32), Self::E>;
    fn fps(&mut self, fps: (u64, u64));
    fn start_offset(&mut self, offset: f64);
    fn spawn_gray(self) -> impl Iterator<Item = Result<GrayImage, Self::E>>;
    fn spawn_rgb(self) -> impl Iterator<Item = Result<RgbImage, Self::E>>;
}

#[cfg(feature = "gstreamer_backend")]
pub mod gst_impl {
    use std::{path::Path, time::Duration};

    use image::{GrayImage, RgbImage};
    use serde::{Deserialize, Serialize};
    use thiserror::Error;
    use vid_frame_iter::{ImageFns, VideoFrameIterBuilder};

    use crate::FrameReadCfgTrait;

    #[derive(Debug, Clone)]
    pub struct FrameReaderCfgGst(VideoFrameIterBuilder);

    #[derive(Error, Debug, Clone, Serialize, Deserialize)]
    pub enum GstError {
        #[error("Not a video")]
        NotVideo,

        #[error("gstreamer error: {0}")]
        GlibError(String),
    }

    impl From<glib::Error> for GstError {
        fn from(e: glib::Error) -> Self {
            Self::GlibError(format!(
                "quark: {}, code: {}, message: {}",
                e.domain().as_str(),
                unsafe { (*e.as_ptr()).code },
                e.message()
            ))
        }
    }

    impl FrameReadCfgTrait for FrameReaderCfgGst {
        type E = GstError;

        fn from_path(src_path: &Path) -> Self {
            vid_frame_iter::init_gstreamer();
            let uri_string = url::Url::from_file_path(src_path).unwrap().to_string();
            Self(VideoFrameIterBuilder::from_uri(uri_string))
        }

        fn fps(&mut self, fps: (u64, u64)) {
            self.0.frame_rate(fps)
        }

        fn start_offset(&mut self, duration: f64) {
            self.0.start_offset(duration)
        }

        fn spawn_gray(self) -> impl Iterator<Item = Result<GrayImage, Self::E>> {
            let mut maybe_it = self.0.spawn_gray();
            let mut done = false;
            std::iter::from_fn(move || {
                if done {
                    None
                } else {
                    match &mut maybe_it {
                        Err(e) => {
                            done = true;
                            Some(Err(e.clone().into()))
                        }
                        &mut Ok(ref mut it) => match it.next() {
                            Some(next_frame) => match next_frame {
                                Ok(next_frame) => Some(Ok(next_frame.to_imagebuffer())),
                                Err(e) => Some(Err(e.clone().into())),
                            },
                            None => None,
                        },
                    }
                }
            })
        }

        fn spawn_rgb(self) -> impl Iterator<Item = Result<RgbImage, Self::E>> {
            let mut maybe_it = self.0.spawn_rgb();
            let mut done = false;
            std::iter::from_fn(move || {
                if done {
                    None
                } else {
                    match &mut maybe_it {
                        Err(e) => {
                            done = true;
                            Some(Err(e.clone().into()))
                        }
                        &mut Ok(ref mut it) => match it.next() {
                            Some(next_frame) => match next_frame {
                                Ok(next_frame) => Some(Ok(next_frame.to_imagebuffer())),
                                Err(e) => Some(Err(e.clone().into())),
                            },
                            None => None,
                        },
                    }
                }
            })
        }

        fn get_duration(&self) -> Result<Duration, Self::E> {
            match vid_frame_iter::mediainfo_utils::duration(self.0.uri()) {
                Ok(Some(duration)) => Ok(duration),
                Ok(None) => Err(GstError::NotVideo),
                Err(e) => Err(e.into()),
            }
        }

        fn get_resolution(&self) -> Result<(u32, u32), Self::E> {
            match vid_frame_iter::mediainfo_utils::dimensions(self.0.uri()) {
                Ok(Some(dims)) => Ok(dims),
                Ok(None) => Err(GstError::NotVideo),
                Err(e) => Err(e.into()),
            }
        }
    }
}

#[cfg(feature = "ffmpeg_backend")]
pub mod ffmpeg_impl {

    use ffmpeg_cmdline_utils::{FfmpegError, FfmpegFrameReaderBuilder, VideoInfo};
    use image::{GrayImage, RgbImage};

    use crate::FrameReadCfgTrait;

    #[derive(Debug, Clone)]
    pub struct FrameReaderCfgFfmpeg(FfmpegFrameReaderBuilder);

    impl FrameReadCfgTrait for FrameReaderCfgFfmpeg {
        type E = FfmpegError;

        fn from_path(src_path: &std::path::Path) -> Self {
            Self(FfmpegFrameReaderBuilder::new(src_path))
        }

        fn get_duration(&self) -> Result<std::time::Duration, Self::E> {
            let info = VideoInfo::new(self.0.src_path())?;
            Ok(info.duration())
        }

        fn get_resolution(&self) -> Result<(u32, u32), Self::E> {
            let info = VideoInfo::new(self.0.src_path())?;
            Ok(info.resolution())
        }

        fn fps(&mut self, (fps_num, fps_den): (u64, u64)) {
            self.0.fps(format!("{fps_num}/{fps_den}"));
        }

        fn start_offset(&mut self, offset: f64) {
            self.0.skip_forward(offset as u32);
        }

        fn spawn_gray(self) -> impl Iterator<Item = Result<GrayImage, Self::E>> {
            let mut maybe_it = self.0.spawn_gray();
            let mut done = false;
            std::iter::from_fn(move || {
                if done {
                    None
                } else {
                    match &mut maybe_it {
                        Err(e) => {
                            done = true;
                            Some(Err(e.clone()))
                        }
                        &mut Ok((ref mut it, ref mut _info)) => it.next().map(Ok),
                    }
                }
            })
        }

        fn spawn_rgb(self) -> impl Iterator<Item = Result<RgbImage, Self::E>> {
            let mut maybe_it = self.0.spawn_rgb();
            let mut done = false;
            std::iter::from_fn(move || {
                if done {
                    None
                } else {
                    match &mut maybe_it {
                        Err(e) => {
                            done = true;
                            Some(Err(e.clone()))
                        }
                        &mut Ok((ref mut it, ref mut _info)) => it.next().map(Ok),
                    }
                }
            })
        }
    }
}
