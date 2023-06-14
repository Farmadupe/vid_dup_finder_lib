#![allow(clippy::let_and_return)]
#![allow(clippy::zero_prefixed_literal)]
#![warn(clippy::redundant_pub_crate)]
#![warn(clippy::print_stdout)]
#![warn(clippy::print_stderr)]
//#![warn(clippy::pedantic)]
#![warn(clippy::unwrap_used)]

//! A crate for reading video frames and processing them as images, using gstreamer as a backend.
//!
//! To start reading frames, you create a [`VideoFrameIterBuilder`] with a URI to the location of the video. Then call `spawn_gray` of `spawn_rgb` to receive
//! an iterator over video frames.
//!
//! Has integration with the [`image`] crate for easy image processing (but also allows direct access to raw pixels if that's what you want)
//!
//! The interface is small and minimal.
//!
//! # Examples
//! Iterate over all video frames, and print the total number of frames.
//!
//! ```
//! use vid_frame_iter::VideoFrameIterBuilder;
//! use glib::Error;
//!
//! fn main() -> Result<(), glib::Error> {
//! #     use std::ffi::OsStr;
//!
//!       //Must call this first.
//!       vid_frame_iter::init_gstreamer();
//!
//!       //(If you have a file on disk you have to convert it to a URI string first)
//! #     #[allow(non_snake_case)]
//! #     let VIDEO_URI_HERE : String = url::Url::from_file_path(std::env::current_dir().unwrap().join(OsStr::new("examples/vids/dog.1.mp4")).to_string_lossy().to_string()).unwrap().to_string(); println!("{VIDEO_URI_HERE}");
//!
//!       // Create a VideoFrameIterBuilder. There are a few extra methods for
//!       // providing options, but for now we can just provide the URI.
//!       let builder = VideoFrameIterBuilder::from_uri(VIDEO_URI_HERE);
//!       let mut frames = builder.spawn_rgb()?;
//!
//!       // Count the frames and print them!
//!       let total_frames = frames.count();
//!       println!("total frames: {total_frames}");
//!
//! #     //sanity check
//! #     assert_eq!(total_frames, 1080);
//!       Ok(())
//! }
//! ```
//!
//!
//! Save one from per second to disk
//! ```
//! use vid_frame_iter::VideoFrameIterBuilder;
//! use vid_frame_iter::ImageFns;
//! use glib::Error;
//!
//! fn main() -> Result<(), glib::Error> {
//! #     use std::ffi::OsStr;
//!
//!       //Must call this first.
//!       vid_frame_iter::init_gstreamer();
//!
//!       //(If you have a file on disk you have to convert it to a URI string first)
//! #     #[allow(non_snake_case)]
//! #     //let VIDEO_URI_HERE : String = std::env::current_dir().unwrap().join(OsStr::new("examples/vids/dog.1.mp4")).to_string_lossy().to_string();
//! #     let VIDEO_URI_HERE : String = url::Url::from_file_path(std::env::current_dir().unwrap().join(OsStr::new("examples/vids/dog.1.mp4")).to_string_lossy().to_string()).unwrap().to_string(); println!("{VIDEO_URI_HERE}");
//!
//!       let mut builder = VideoFrameIterBuilder::from_uri(VIDEO_URI_HERE);
//!       builder.frame_rate((1, 1));
//!       let mut frames = builder.spawn_rgb()?;
//!
//!       //Gstreamer is internally discarding unwanted frames, so we just process every frame we receive.
//!       for (idx, frame) in frames.enumerate() {
//!           match frame {
//!               Err(e) => return Err(e),
//!               Ok(frame) => {
//!                   // We have to convert the frame to an [`image::ImageBuffer`] to be able to save it.
//!                   let frame_buf: image::RgbImage = frame.to_imagebuffer();
//!                   match frame_buf.save_with_format(format!("{idx}.bmp"), image::ImageFormat::Bmp) {
//!                       Ok(()) => (),
//!                       Err(_e) => () //handle image save error here
//!                   }
//!               }
//!           }
//!       }
//!
//!       Ok(())
//! }
//! ```
//! # Error handling
//! Instead of defining its own error type this crate uses the libglib [`glib::Error`] type. You can handle errors by switching on the `matches` method
//! from [`glib::Error`].
//! ```
//! # use std::ffi::OsStr;
//! use vid_frame_iter::VideoFrameIterBuilder;
//! use glib::Error;
//!
//! fn main() -> Result<(), glib::Error> {
//!       vid_frame_iter::init_gstreamer();
//!
//! #     #[allow(non_snake_case)]
//! #     let BAD_VIDEO_URI_HERE : String = "$%$%$%$%$%$%$".to_string();
//!
//!       // A video file that doesn't exist...
//!       match VideoFrameIterBuilder::from_uri(BAD_VIDEO_URI_HERE).spawn_rgb() {
//!           Ok(frame_iterator) => (),//no error
//!           Err(e) => {
//!
//!               // You need to write an if-elsif chain for all the errors you
//!               // want to handle.
//!               if e.matches(gstreamer::ResourceError::NotFound) {
//!                   println!("oh no! That file doesn't exist!");
//!               } else {
//!                   println!("whoops! This is a different error!")
//!               }
//!           }
//!       }
//!       Ok(())
//! }
//! ```
//!
//! # Supported operating systems
//! Currently only tested on Ubuntu Linux 22.04. This crate should work in MacOS and windows but this has not been tested.
//!
//! # Installing
//! You should follow the detailed instructions written for gstreamer-rs [here.](https://github.com/sdroege/gstreamer-rs#installation)
//!

/// Utilities for getting duration and dimensions of a video without decoding frames.
pub mod mediainfo_utils;

// Provides [`VideoFrameIter`] and [`VideoFrameIterBuilder`]
/// Functions for decoding on your Nvidia GPU.
pub mod extras;
pub mod frame_iter;

pub use frame_iter::GrayFrame;
pub use frame_iter::ImageFns;
pub use frame_iter::RgbFrame;
pub use frame_iter::VideoFrameIter;
pub use frame_iter::VideoFrameIterBuilder;

pub use extras::*;
pub use mediainfo_utils::*;

/// Initialize gstreamer. You must call this function before calling any other function in this crate.
pub fn init_gstreamer() {
    gstreamer::init().expect("Failed to initialize gstreamer")
}
