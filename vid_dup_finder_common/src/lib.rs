#![allow(clippy::let_and_return)]
#![deny(clippy::print_stdout)]
#![deny(clippy::print_stderr)]
#![deny(clippy::dbg_macro)]

// #![warn(clippy::unnecessary_cast)]
// #![warn(clippy::cast_lossless)]
// #![warn(clippy::cast_possible_truncation)]
// #![warn(clippy::cast_possible_wrap)]
// #![warn(clippy::cast_precision_loss)]
// #![warn(clippy::cast_sign_loss)]

pub mod compositing;
mod crop;
pub mod motioncrop;
pub mod resize_gray;
pub mod resize_rgb;
pub mod video_frames_gray;
pub mod video_frames_rgb;

pub use compositing::grid_images_rgb;
pub use compositing::row_images;
pub use crop::Crop;
pub use resize_gray::crop_resize_buf;
pub use video_frames_gray::VideoFramesGray;
pub use video_frames_rgb::FrameSeqRgb;
