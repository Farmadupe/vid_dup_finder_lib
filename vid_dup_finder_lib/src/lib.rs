#![allow(clippy::let_and_return)]
#![allow(clippy::len_without_is_empty)]
#![warn(clippy::cast_lossless)]
#![warn(clippy::print_stdout)]
#![warn(clippy::print_stderr)]
//#![warn(clippy::pedantic)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]

//! # Overview
//! `vid_dup_finder_lib` is a library for finding near-duplicate video files, even if the videos are in different
//! formats, size/resolution, quality, or framerate.
//!
//! The library will not find duplicate video files if they have been rotated/flipped, sped up or slowed down, or embedded in
//! the corner of another video. It also cannot find if a clip of one video is embedded within another video.
//!
//! # High Level API
//! First provide the paths to a set of video files and turn them into hashes
//! Then, use one of the duplicate detection functions to discover which videos are duplicates
//! of each other.
//! ```rust,
//! use vid_dup_finder_lib::VideoHash;
//! use vid_dup_finder_lib::MatchGroup;
//!
//! //Gstreamer must be initialized first
//! vid_dup_finder_lib::init_gstreamer();
//!
//! # use std::ffi::OsStr;
//! # use std::path::Path;
//! # let dup_vid_path_1 = std::env::current_dir().unwrap().join(OsStr::new("examples/vids/cat.1.mp4"));
//! # let dup_vid_path_2 = std::env::current_dir().unwrap().join(OsStr::new("examples/vids/cat.3.webm"));
//! # let other_vid_path = std::env::current_dir().unwrap().join(OsStr::new("examples/vids/dog.3.webm"));
//! // Paths to some vids to search for duplicates.
//! // Let's assume the first two videos are duplicates and the third is unrelated.
//! let vids = [&dup_vid_path_1, &dup_vid_path_2, &other_vid_path];
//! let hashes = vids.iter().map(VideoHash::from_path).map(Result::unwrap);
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
//! To generate a hash from a video file, this library reads 64 frames from the first 20 seconds of the video file
//! (or if the file is shorter it reads 64 frames evenly spaced across the entire video). It then resizes each frame
//! down to 64x64 pixels in size, forming a 64x64x64 matrix. The three-dimensional [discrete cosine transform](http://hackerfactor.com/blog/index.php%3F/archives/432-Looks-Like-It.html)
//! of this matrix is calculated resulting in another 64x64x64 matrix. Due to the 'energy compaction' quality of the DCT,
//! the 5x5x5 nearest to the origin contains a low frequency description about the the content of the video. A hash of 125 bits is built
//! where each bit is the positive/negative magnitude of each bin in the 5x5x5 cube. The length of the video is also included
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
//! Because this library only checks the first 30 seconds of each video, if two videos are the same
//! length and share the first 30 seconds of video content, they will be reported as a false match. This
//! may occur for TV shows which contain opening credits.
//!
//! # A note on data structures
//! The hashes produced by this library fully satisfy the triangle equality, and it is possible to use a
//! [BK tree](https://en.wikipedia.org/wiki/BK-tree) to search for duplicates. I did implement a naive BK tree
//! however in every experiment I ran (at least up to 1million hashes) it was significantly slower at finding duplicates than
//! neatly placing every hash in a vector and doing a O(n^2) comparison of every hash against every other hash.
//! The current implementation of this library still uses a O(n^2) algorithm, but it takes advantage of the fact that
//! videos with differing durations cannot be duplicates of each other to practically reduce the number of comparisons
//! required. However if all your videos are the same length searches will unfortunately still perform n^2 comparisons.

mod definitions;
mod video_hashing;

use ffmpeg_gst_wrapper::ffmpeg_gst;

pub fn init_gstreamer() {
    ffmpeg_gst::init_gstreamer()
}

pub use video_hashing::{
    hash_creation_error_kind::HashCreationErrorKind, matches::match_group::MatchGroup,
    video_dup_finder::*, video_hash::HashCreationOptions, video_hash::VideoHash,
    video_hash::DEFAULT_HASH_CREATION_OPTIONS,
};

pub use definitions::{CropdetectType, DEFAULT_SEARCH_TOLERANCE};

#[cfg(any(feature = "test-util", test))]
pub use definitions::TOLERANCE_SCALING_FACTOR;
#[cfg(any(feature = "test-util", test))]
pub use video_hashing::video_hash::test_util::{self, *};
