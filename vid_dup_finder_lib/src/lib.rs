#![allow(clippy::let_and_return)]
#![allow(clippy::len_without_is_empty)]
#![warn(clippy::cast_lossless)]
#![warn(clippy::print_stdout)]
#![warn(clippy::print_stderr)]
#![warn(clippy::todo)]
#![warn(clippy::dbg_macro)]
#![warn(clippy::unimplemented)]
#![warn(clippy::unwrap_used)]
#![warn(clippy::panic)]
//#![warn(clippy::expect_used)]
#![allow(clippy::doc_markdown)]

//! # Overview
//! `vid_dup_finder_lib` is a library for finding near-duplicate video files.
//! A near-duplicate video is a file that closely resembles another but may have differences
//! such as format, resolution, quality, or framerate.
//!
//! However The library will not match video files if they have been rotated/flipped, sped up or slowed down, or embedded in
//! the corner of another video.
//!
//! # High Level API
//! First provide the paths to a set of video files and turn them into hashes
//! Then, use one of the duplicate detection functions to discover which videos are duplicates
//! of each other.
//! ```rust,
//! use vid_dup_finder_lib::VideoHash;
//! use vid_dup_finder_lib::MatchGroup;
//! use vid_dup_finder_lib::CreationOptions;
//! use vid_dup_finder_lib::VideoHashBuilder;
//!
//!
//! # use std::ffi::OsStr;
//! # use std::path::Path;
//! # let dup_vid_path_1 = std::env::current_dir().unwrap().join(OsStr::new("examples/vids/cat.1.mp4"));
//! # let dup_vid_path_2 = std::env::current_dir().unwrap().join(OsStr::new("examples/vids/cat.3.webm"));
//! # let other_vid_path = std::env::current_dir().unwrap().join(OsStr::new("examples/vids/dog.3.webm"));
//! // Paths to some vids to search for duplicates.
//! // Let's assume the first two videos are duplicates and the third is unrelated.
//! let vids = [&dup_vid_path_1, &dup_vid_path_2, &other_vid_path];
//! let builder = VideoHashBuilder::default();
//! let hashes = vids.iter().map(|vid| builder.hash(vid.to_path_buf()).unwrap());
//!
//! // You have to choose a tolerance between 0.0 and 1.0 for searching. Higher numbers
//! // mean searches will match more different videos. The default search tolerance of
//! // 0.3 is a good starting point for searching, but you can use a lower number if
//! // there are too many false positives in your results.
//! let tolerance = vid_dup_finder_lib::DEFAULT_SEARCH_TOLERANCE;
//!
//! // Perform the search...
//! let dup_groups: Vec<MatchGroup> = vid_dup_finder_lib::search(hashes, tolerance);
//!
//! //dup_groups will contain a single match_group..
//! assert_eq!(dup_groups.len(), 1);
//! assert_eq!(dup_groups[0].len(), 2);
//!
//! //with only the duplicated videos inside.
//! let dup_files: Vec<&Path> = dup_groups[0].duplicates().collect();
//! assert!(dup_files.contains(&dup_vid_path_1.as_path()));
//! assert!(dup_files.contains(&dup_vid_path_2.as_path()));
//! assert!(!dup_files.contains(&other_vid_path.as_path()));
//! ```
//!
//! # Prerequisites
//! This crate calls Ffmpeg from the command line. You must make Ffmpeg and Ffprobe available
//! on the command line, for example:
//!
//! * Debian-based systems: ```# apt-get install ffmpeg```
//! * Yum-based systems: ```# yum install ffmpeg```
//! * Windows:
//!     1) Download the correct installer from <https://ffmpeg.org/download.html>
//!     2) Run the installer and install ffmpeg to any directory
//!     3) Add the directory into the PATH environment variable
//!
//! Unfortunately this requirement exists due to technical reasons (no documented, and memory-leak-free bindings
//! exist to ffmpeg) and licensing reasons (statically linking to Ffmpeg may introduce additional
//! transitive licensing requirements on end users of this library),
//!
//! # How it works
//! To generate a hash from a video file, this library reads N (e.g 64) frames from the first few seconds of the video file
//! (or if the file is shorter it reads N frames evenly spaced across the entire video). It then resizes each frame
//! down to NxN pixels in size, forming a NxNxN matrix. The three-dimensional [discrete cosine transform](http://hackerfactor.com/blog/index.php%3F/archives/432-Looks-Like-It.html)
//! of this matrix is calculated resulting in another NxNxN matrix. Due to the 'energy compaction' quality of the DCT,
//! the MxMxM "sub matrix" (e.g 5x5x5) at one corner of the NxNxN matrix contains the majority of information about the the content of the video. A hash is build
//! where each bit is the positive/negative magnitude of each bin in the MxMxM cube. The length of the video is also included
//! in the hash, as this can be used to speed up searching.
//!
//! You can then use the library to search with these hashes. Searches will return any group of videos
//! with a similar length, and whose hashes differ by less than a set threshold.
//!
//! ## Search functions
//! The following search functions are available:
//! * To find all duplicate videos within a set: [`crate::search`]
//! * To find all duplicate videos using a set of reference videos: [`crate::search_with_references`]
//!
//! # Caching
//! To generate the hashes this library must decode the first 20 seconds of each video it processes
//! if there are a lot of viedos this takes a very long time. There is a companion crate called
//! `video_hash_filesystem_cache` which will store caches on disk in between searches, reducing the amount of time
//! spent loading videos.
//!
//! # Limitations
//! The library is specifically designed to find near-duplicate videos (i.e ones that have not been significantly edited).
//! Many transformations are capable of defeating it, such as rotation/flipping, watermarking, or time-offsetting.  
//! However if the transformations are minor (a faint watermark, a small crop etc) then this library should still
//! detect duplicate videos.
//!
//! Because the aim of this library is to find near-duplicates, the hashes are generated from the first
//! 20 seconds of video content to save time.
//!
//! This library is will not defeat "classic" methods of hiding duplicates, such as horizontal mirroring, changing
//! playback speed, or embedding video content in the corner of a static frame.
//!
//!
//! ## False Positives
//! Because this library only checks the first few seconds of each video, if two videos are the same
//! length and share the first few seconds of video content, they will be reported as a false match. This
//! may occur for TV shows which contain opening credits.
//!
// //! # A note on data structures
// //! The hashes produced by this library fully satisfy the triangle equality, and it is possible to use a
// //! [BK tree](https://en.wikipedia.org/wiki/BK-tree) to search for duplicates. I did implement a naive BK tree
// //! however in every experiment I ran (at least up to 1million hashes) it was significantly slower at finding duplicates than
// //! neatly placing every hash in a vector and doing a O(n^2) comparison of every hash against every other hash.
// //! The current implementation of this library still uses a O(n^2) algorithm, but it takes advantage of the fact that
// //! videos with differing durations cannot be duplicates of each other to practically reduce the number of comparisons
// //! required. However if all your videos are the same length searches will unfortunately still perform n^2 comparisons.

mod definitions;
mod video_hashing;

pub use video_hashing::{
    matches::match_group::MatchGroup, video_dup_finder::search,
    video_dup_finder::search_with_references, video_hash::VideoHash,
    video_hash_builder::CreationOptions, video_hash_builder::VideoHashBuilder, Error,
};

pub use definitions::{
    Cropdetect, DEFAULT_SEARCH_TOLERANCE, DEFAULT_VID_HASH_DURATION, DEFAULT_VID_HASH_SKIP_FORWARD,
};

#[cfg(any(feature = "test-util", test))]
pub use definitions::TOLERANCE_SCALING_FACTOR;
#[cfg(any(feature = "test-util", test))]
pub use video_hashing::video_hash::test_util;

#[doc(hidden)]
/// Utilities for visualizing the frames that vid_dup_finder_lib extracts
/// These functions are not part of the stable API.
pub mod debug_util {
    pub use crate::video_hashing::video_hash_builder::build_frame_reader;
}

type VideoHashResult<T> = Result<T, crate::Error>;
