use std::{
    fmt::Debug,
    path::{Path, PathBuf},
    time::Duration,
};

use image::{GrayImage, RgbImage};

//#[cfg(all(feature = "ffmpeg_backend", feature = "gstreamer_backend"))]
//compile_error!("feature \"ffmpeg_backend\" and feature \"gstreamer_backend\" cannot be enabled at the same time");

#[cfg(feature = "ffmpeg_backend")]
#[allow(dead_code)]
fn get_resolution_ffmpeg(src_path: &Path) -> Result<(u32, u32), FrameReadCfgErr> {
    let info = ffmpeg_cmdline_utils::VideoInfo::new(src_path)
        .map_err(|e| FrameReadCfgErr(format!("{e}")))?;
    Ok(info.resolution())
}

#[cfg(feature = "ffmpeg_backend")]
#[allow(dead_code)]
fn get_duration_ffmpeg(src_path: &Path) -> Result<Duration, FrameReadCfgErr> {
    let info = ffmpeg_cmdline_utils::VideoInfo::new(src_path)
        .map_err(|e| FrameReadCfgErr(format!("{e}")))?;
    Ok(info.duration())
}

#[cfg(feature = "gstreamer_backend")]
fn get_duration_gst(src_path: &Path) -> Result<Duration, FrameReadCfgErr> {
    vid_frame_iter::init_gstreamer();
    let uri_string = url::Url::from_file_path(src_path).map_err(|_e| {
        FrameReadCfgErr(format!(
            "unable to convert path to URI: {}",
            src_path.to_string_lossy()
        ))
    })?;
    match vid_frame_iter::mediainfo_utils::duration(uri_string) {
        Ok(Some(duration)) => Ok(duration),
        Ok(None) => Err(FrameReadCfgErr("unable to obtain duration".to_string())),
        Err(e) => Err(FrameReadCfgErr(format!("{}", e.message()))),
    }
}

#[cfg(feature = "gstreamer_backend")]
fn get_resolution_gst(src_path: &Path) -> Result<(u32, u32), FrameReadCfgErr> {
    vid_frame_iter::init_gstreamer();
    let uri_string = url::Url::from_file_path(src_path).map_err(|e| {
        FrameReadCfgErr(format!(
            "unable to convert path to URI: {} -- {:?}",
            src_path.to_string_lossy(),
            e
        ))
    })?;
    match vid_frame_iter::mediainfo_utils::dimensions(uri_string) {
        Ok(Some(dims)) => Ok(dims),
        Ok(None) => Err(FrameReadCfgErr("unable to obtain resolution".to_string())),
        Err(e) => Err(FrameReadCfgErr(format!("{}", e.message()))),
    }
}

pub fn get_resolution(src_path: &Path) -> Result<(u32, u32), FrameReadCfgErr> {
    cfg_if::cfg_if! {
        if #[cfg(feature = "gstreamer_backend")] {
            return get_resolution_gst(&src_path);
        } else if #[cfg(feature = "ffmpeg_backend")] {
            return get_resolution_ffmpeg(&src_path);
        }
    }
}

pub fn get_duration(src_path: &Path) -> Result<Duration, FrameReadCfgErr> {
    cfg_if::cfg_if! {
        if #[cfg(feature = "gstreamer_backend")] {
            return get_duration_gst(&src_path);
        } else if #[cfg(feature = "ffmpeg_backend")] {
            return get_duration_ffmpeg(&src_path);
        }
    }
}

#[derive(Debug, Clone)]
pub struct FrameReadCfg {
    src_path: PathBuf,
    fps: Option<(u64, u64)>,
    start_offset: Option<f64>,
}

#[derive(Debug)]
pub struct FrameReadCfgErr(String);

impl FrameReadCfg {
    pub fn from_path(src_path: &Path) -> Self {
        Self {
            src_path: src_path.to_path_buf(),
            fps: None,
            start_offset: None,
        }
    }

    pub fn fps(&mut self, fps: (u64, u64)) {
        self.fps = Some(fps)
    }

    pub fn start_offset(&mut self, offset: f64) {
        self.start_offset = Some(offset)
    }

    #[cfg(feature = "gstreamer_backend")]
    fn spawn_gray_gst(self) -> impl Iterator<Item = Result<GrayImage, FrameReadCfgErr>> {
        use vid_frame_iter::{ImageFns, VideoFrameIterBuilder};

        vid_frame_iter::init_gstreamer();
        let uri_string = url::Url::from_file_path(self.src_path).unwrap().to_string();

        let mut builder = VideoFrameIterBuilder::from_uri(uri_string);

        if let Some(fps) = self.fps {
            builder.frame_rate(fps);
        }

        if let Some(start_offset) = self.start_offset {
            builder.start_offset(start_offset);
        }

        let mut maybe_it = builder.spawn_gray();
        let mut done = false;
        std::iter::from_fn(move || {
            if done {
                None
            } else {
                match &mut maybe_it {
                    Err(e) => {
                        done = true;
                        let text = format!("{}", e);
                        Some(Err(FrameReadCfgErr(text)))
                    }
                    &mut Ok(ref mut it) => match it.next() {
                        Some(next_frame) => match next_frame {
                            Ok(next_frame) => Some(Ok(next_frame.to_imagebuffer())),
                            Err(e) => {
                                let text = format!("{}", e);
                                Some(Err(FrameReadCfgErr(text)))
                            }
                        },
                        None => None,
                    },
                }
            }
        })
    }

    #[cfg(feature = "gstreamer_backend")]
    fn spawn_rgb_gst(self) -> impl Iterator<Item = Result<RgbImage, FrameReadCfgErr>> {
        use vid_frame_iter::{ImageFns, VideoFrameIterBuilder};

        vid_frame_iter::init_gstreamer();
        let uri_string = url::Url::from_file_path(self.src_path).unwrap().to_string();

        let mut builder = VideoFrameIterBuilder::from_uri(uri_string);

        if let Some(fps) = self.fps {
            builder.frame_rate(fps);
        }

        if let Some(start_offset) = self.start_offset {
            builder.start_offset(start_offset);
        }

        let mut maybe_it = builder.spawn_rgb();
        let mut done = false;
        std::iter::from_fn(move || {
            if done {
                None
            } else {
                match &mut maybe_it {
                    Err(e) => {
                        done = true;
                        let text = format!("{}", e);
                        Some(Err(FrameReadCfgErr(text)))
                    }
                    &mut Ok(ref mut it) => match it.next() {
                        Some(next_frame) => match next_frame {
                            Ok(next_frame) => Some(Ok(next_frame.to_imagebuffer())),
                            Err(e) => {
                                let text = format!("{}", e);
                                Some(Err(FrameReadCfgErr(text)))
                            }
                        },
                        None => None,
                    },
                }
            }
        })
    }

    #[cfg(feature = "ffmpeg_backend")]
    #[allow(dead_code)]
    fn spawn_gray_ffmpeg(self) -> impl Iterator<Item = Result<GrayImage, FrameReadCfgErr>> {
        use ffmpeg_cmdline_utils::FfmpegFrameReaderBuilder;
        let mut builder = FfmpegFrameReaderBuilder::new(self.src_path);

        if let Some((fps_num, fps_den)) = self.fps {
            builder.fps(format!("{fps_num}/{fps_den}"));
        }

        if let Some(offset) = self.start_offset {
            builder.skip_forward(offset as u32);
        }

        let mut maybe_it = builder.spawn_gray();
        let mut done = false;
        std::iter::from_fn(move || {
            if done {
                None
            } else {
                match &mut maybe_it {
                    Err(e) => {
                        done = true;
                        Some(Err(FrameReadCfgErr(format!("{:?}", e.clone()))))
                    }
                    &mut Ok((ref mut it, ref mut _info)) => it.next().map(Ok),
                }
            }
        })
    }

    #[cfg(feature = "ffmpeg_backend")]
    #[allow(dead_code)]
    fn spawn_rgb_ffmpeg(self) -> impl Iterator<Item = Result<RgbImage, FrameReadCfgErr>> {
        use ffmpeg_cmdline_utils::FfmpegFrameReaderBuilder;
        let mut builder = FfmpegFrameReaderBuilder::new(self.src_path);

        if let Some((fps_num, fps_den)) = self.fps {
            builder.fps(format!("{fps_num}/{fps_den}"));
        }

        if let Some(offset) = self.start_offset {
            builder.skip_forward(offset as u32);
        }

        let mut maybe_it = builder.spawn_rgb();
        let mut done = false;
        std::iter::from_fn(move || {
            if done {
                None
            } else {
                match &mut maybe_it {
                    Err(e) => {
                        done = true;
                        Some(Err(FrameReadCfgErr(format!("{:?}", e.clone()))))
                    }
                    &mut Ok((ref mut it, ref mut _info)) => it.next().map(Ok),
                }
            }
        })
    }

    pub fn spawn_gray(self) -> impl Iterator<Item = Result<GrayImage, FrameReadCfgErr>> {
        cfg_if::cfg_if! {
            if #[cfg(feature = "gstreamer_backend")] {
                return self.spawn_gray_gst();
            } else if #[cfg(feature = "ffmpeg_backend")] {
                return self.spawn_gray_ffmpeg();
            }
        }
    }

    pub fn spawn_rgb(self) -> impl Iterator<Item = Result<RgbImage, FrameReadCfgErr>> {
        cfg_if::cfg_if! {
            if #[cfg(feature = "gstreamer_backend")] {
                return self.spawn_rgb_gst();
            } else if #[cfg(feature = "ffmpeg_backend")] {
                return self.spawn_rgb_ffmpeg();
            }
        }
    }
}
