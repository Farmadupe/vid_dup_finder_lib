#[cfg(feature = "gstreamer_backend")]
pub mod gst_impl {
    use image::{GenericImageView, GrayImage, Luma, Rgb, RgbImage};
    use serde::{Deserialize, Serialize};
    use thiserror::Error;

    use vid_frame_iter::{RgbFrame, VideoFrameIter};

    use vid_frame_iter::{GrayFrame, ImageFns, VideoFrameIterBuilder};

    use std::{fmt::Display, path::Path};

    #[derive(Error, Debug, Clone, Serialize, Deserialize)]
    pub struct FfmpegGstError(());

    impl From<glib::Error> for FfmpegGstError {
        fn from(_e: glib::Error) -> Self {
            Self(())
        }
    }

    impl Display for FfmpegGstError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            "error".fmt(f)
        }
    }

    pub fn init_gstreamer() {
        vid_frame_iter::init_gstreamer()
    }

    pub fn duration(src_path: impl AsRef<Path>) -> Option<std::time::Duration> {
        let uri_string = url::Url::from_file_path(src_path).ok()?.to_string();
        //println!("{:?}", vid_frame_iter::mediainfo_utils::codec(&uri_string));

        vid_frame_iter::mediainfo_utils::duration(uri_string)
            .ok()
            .flatten()
    }

    pub fn resolution(src_path: impl AsRef<Path>) -> Option<(u32, u32)> {
        let uri_string = url::Url::from_file_path(src_path).ok()?.to_string();
        vid_frame_iter::mediainfo_utils::dimensions(uri_string)
            .ok()
            .flatten()
    }

    #[derive(Debug, Clone)]
    pub struct VideoFrameRgbUnified(RgbFrame);

    impl GenericImageView for VideoFrameRgbUnified {
        type Pixel = Rgb<u8>;

        fn dimensions(&self) -> (u32, u32) {
            self.0.dimensions()
        }

        fn bounds(&self) -> (u32, u32, u32, u32) {
            self.0.bounds()
        }

        fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
            self.0.get_pixel(x, y)
        }
    }

    impl VideoFrameRgbUnified {
        pub fn as_flat(&self) -> image::FlatSamples<&[u8]> {
            self.0.as_flat()
        }

        pub fn inner(&self) -> RgbFrame {
            self.0.clone()
        }

        pub fn frame_owned(&self) -> RgbImage {
            self.0.clone().to_imagebuffer()
        }
    }

    #[derive(Debug, Clone)]
    pub struct VideoFrameGrayUnified(GrayFrame);

    impl GenericImageView for VideoFrameGrayUnified {
        type Pixel = Luma<u8>;

        fn dimensions(&self) -> (u32, u32) {
            self.0.dimensions()
        }

        fn bounds(&self) -> (u32, u32, u32, u32) {
            self.0.bounds()
        }

        fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
            self.0.get_pixel(x, y)
        }
    }

    impl VideoFrameGrayUnified {
        pub fn as_flat(&self) -> image::FlatSamples<&[u8]> {
            self.0.as_flat()
        }

        pub fn inner(&self) -> GrayFrame {
            self.0.clone()
        }

        pub fn frame_owned(&self) -> GrayImage {
            self.0.clone().to_imagebuffer()
        }
    }

    pub struct FrameReaderCfgUnified(VideoFrameIterBuilder);

    impl FrameReaderCfgUnified {
        pub fn from_path(src_path: &Path) -> Self {
            let uri_string = url::Url::from_file_path(src_path).unwrap().to_string();
            Self(VideoFrameIterBuilder::from_uri(uri_string))
        }

        pub fn fps(&mut self, fps: (u64, u64)) {
            self.0.frame_rate(fps)
        }

        pub fn start_offset(&mut self, duration: f64) {
            self.0.start_offset(duration)
        }

        pub fn inner(self) -> VideoFrameIterBuilder {
            self.0
        }

        pub fn spawn_gray(self) -> Result<VideoFramesIterGrayUnified, FfmpegGstError> {
            match self.0.spawn_gray() {
                Ok(frames) => Ok(VideoFramesIterGrayUnified(frames)),
                Err(e) => Err(FfmpegGstError::from(e)),
            }
        }

        pub fn spawn_rgb(self) -> Result<VideoFramesIterRgbUnified, FfmpegGstError> {
            match self.0.spawn_rgb() {
                Ok(frames) => Ok(VideoFramesIterRgbUnified(frames)),
                Err(e) => Err(FfmpegGstError::from(e)),
            }
        }
    }

    pub struct VideoFramesIterGrayUnified(VideoFrameIter<GrayFrame>);

    impl Iterator for VideoFramesIterGrayUnified {
        type Item = Result<VideoFrameGrayUnified, FfmpegGstError>;

        fn next(&mut self) -> Option<Self::Item> {
            match self.0.next() {
                Some(Ok(frame)) => Some(Ok(VideoFrameGrayUnified(frame))),
                Some(Err(_e)) => Some(Err(FfmpegGstError(()))),
                None => None,
            }
        }
    }

    pub struct VideoFramesIterRgbUnified(VideoFrameIter<RgbFrame>);

    impl Iterator for VideoFramesIterRgbUnified {
        type Item = Result<VideoFrameRgbUnified, FfmpegGstError>;

        fn next(&mut self) -> Option<Self::Item> {
            match self.0.next() {
                Some(Ok(frame)) => Some(Ok(VideoFrameRgbUnified(frame))),
                Some(Err(_e)) => Some(Err(FfmpegGstError(()))),
                None => None,
            }
        }
    }

    pub fn deprioritize_nvidia_gpu_decoding() {
        vid_frame_iter::deprioritize_nvidia_gpu_decoding()
    }
}

pub trait ExtractFrameCfg {}

#[cfg(feature = "ffmpeg_backend")]
pub mod ffmpeg_impl {
    use std::{fmt::Display, path::Path};

    use ffmpeg_cmdline_utils::{
        FfmpegError, FfmpegFrameIterGray, FfmpegFrameIterRgb, FfmpegFrameReaderBuilder, VideoInfo,
    };
    use image::{GenericImageView, GrayImage, Luma, RgbImage};
    use serde::{Deserialize, Serialize};
    use thiserror::Error;

    #[derive(Error, Debug, Clone, Serialize, Deserialize)]
    pub struct FfmpegGstError(FfmpegError);

    impl From<FfmpegError> for FfmpegGstError {
        fn from(e: FfmpegError) -> Self {
            Self(e)
        }
    }

    impl Display for FfmpegGstError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            self.0.fmt(f)
        }
    }

    pub fn init_gstreamer() {}

    pub fn duration(src_path: impl AsRef<Path>) -> Option<std::time::Duration> {
        VideoInfo::new(src_path.as_ref()).ok().map(|i| i.duration())
    }

    pub fn resolution(src_path: impl AsRef<Path>) -> Option<(u32, u32)> {
        VideoInfo::new(src_path.as_ref())
            .ok()
            .map(|i| i.resolution())
    }

    #[derive(Debug, Clone)]
    pub struct VideoFrameGrayUnified(image::GrayImage);

    impl GenericImageView for VideoFrameGrayUnified {
        type Pixel = Luma<u8>;

        fn dimensions(&self) -> (u32, u32) {
            self.0.dimensions()
        }

        fn bounds(&self) -> (u32, u32, u32, u32) {
            self.0.bounds()
        }

        fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
            *self.0.get_pixel(x, y)
        }
    }

    #[derive(Debug, Clone)]
    pub struct VideoFrameRgbUnified(image::RgbImage);

    impl GenericImageView for VideoFrameRgbUnified {
        type Pixel = image::Rgb<u8>;

        fn dimensions(&self) -> (u32, u32) {
            self.0.dimensions()
        }

        fn bounds(&self) -> (u32, u32, u32, u32) {
            self.0.bounds()
        }

        fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
            *self.0.get_pixel(x, y)
        }
    }

    impl VideoFrameRgbUnified {
        pub fn as_flat(&self) -> image::FlatSamples<&[u8]> {
            self.0.as_flat_samples()
        }

        pub fn inner(&self) -> image::RgbImage {
            self.0.clone()
        }

        pub fn frame_owned(&self) -> RgbImage {
            self.0.clone()
        }
    }

    impl VideoFrameGrayUnified {
        pub fn as_flat(&self) -> image::FlatSamples<&[u8]> {
            self.0.as_flat_samples()
        }

        pub fn inner(&self) -> GrayImage {
            self.0.clone()
        }

        pub fn frame_owned(&self) -> GrayImage {
            self.0.clone()
        }
    }

    #[derive(Debug, Clone)]
    pub struct FrameReaderCfgWindows {
        src_path: String,
        fps: Option<(u64, u64)>,
        seek_at_start: Option<f64>,
    }

    pub struct FrameReaderCfgUnified(FrameReaderCfgWindows);

    impl FrameReaderCfgUnified {
        #[must_use]
        pub fn from_path(src_path: &Path) -> Self {
            Self(FrameReaderCfgWindows {
                src_path: src_path.to_string_lossy().to_string(),
                fps: None,
                seek_at_start: None,
            })
        }

        pub fn fps(&mut self, fps: (u64, u64)) {
            self.0.fps = Some(fps);
        }

        pub fn start_offset(&mut self, duration: f64) {
            self.0.seek_at_start = Some(duration);
        }

        fn spawn_inner(self) -> FfmpegFrameReaderBuilder {
            let mut ret = FfmpegFrameReaderBuilder::new(self.0.src_path);

            if let Some((num, den)) = self.0.fps {
                ret.fps(format!("{num}/{den}"));
            }

            ret.num_frames(100);

            if let Some(amount) = self.0.seek_at_start {
                ret.skip_forward(amount as u32);
            }

            ret.timeout_secs(120);

            ret
        }

        pub fn spawn_gray(self) -> Result<VideoFramesIterGrayUnified, FfmpegGstError> {
            match self.spawn_inner().spawn_gray() {
                Ok((frames, _)) => Ok(VideoFramesIterGrayUnified(frames)),
                Err(e) => Err(FfmpegGstError::from(e)),
            }
        }

        pub fn spawn_rgb(self) -> Result<VideoFramesIterRgbUnified, FfmpegGstError> {
            match self.spawn_inner().spawn_rgb() {
                Ok((frames, _)) => Ok(VideoFramesIterRgbUnified(frames)),
                Err(e) => Err(FfmpegGstError::from(e)),
            }
        }
    }

    pub struct VideoFramesIterGrayUnified(FfmpegFrameIterGray);

    impl Iterator for VideoFramesIterGrayUnified {
        type Item = Result<VideoFrameGrayUnified, FfmpegGstError>;

        fn next(&mut self) -> Option<Self::Item> {
            self.0.next().map(|f| Ok(VideoFrameGrayUnified(f)))
        }
    }

    pub struct VideoFramesIterRgbUnified(FfmpegFrameIterRgb);

    impl Iterator for VideoFramesIterRgbUnified {
        type Item = Result<VideoFrameRgbUnified, FfmpegGstError>;

        fn next(&mut self) -> Option<Self::Item> {
            self.0.next().map(|f| Ok(VideoFrameRgbUnified(f)))
        }
    }

    //noop -- gst specific function.
    pub fn deprioritize_nvidia_gpu_decoding() {}
}

//#[cfg(feature = "gstreamer_backend")]
//pub use gst_impl::{
//    deprioritize_nvidia_gpu_decoding, duration, init_gstreamer, resolution, FfmpegGstError,
//    FrameReaderCfgUnified, VideoFrameGrayUnified,
//};
//
//#[cfg(feature = "ffmpeg_backend")]
//pub use ffmpeg_impl::{
//    deprioritize_nvidia_gpu_decoding, duration, init_gstreamer, resolution, FfmpegGstError,
//    FrameReaderCfgUnified, VideoFrameGrayUnified,
//};
