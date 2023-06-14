#![allow(clippy::let_and_return)]
#![deny(clippy::print_stdout)]
#![deny(clippy::print_stderr)]

pub mod compositing;
mod crop;
pub mod resize_gray;
pub mod resize_rgb;
pub mod video_frames_gray;
pub mod video_frames_rgb;

pub use compositing::grid_images_rgb;
pub use compositing::row_images;
pub use crop::Crop;
pub use resize_gray::crop_resize_buf;
pub use resize_gray::crop_resize_flat;
pub use video_frames_gray::VideoFramesGray;
pub use video_frames_rgb::FrameSeqRgb;
