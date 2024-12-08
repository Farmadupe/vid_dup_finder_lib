#![allow(clippy::len_without_is_empty)]

//! # Overview
//! vid_dup_finder_lib is a library for creating perceptual hashes of video files
//! and using those hashes to find duplicates.
//!
//! # How it works
//! The library generates hashes from the following information:
//! * Video duration
//! * The [discrete cosine transform](http://hackerfactor.com/blog/index.php%3F/archives/432-Looks-Like-It.html) of ten frames from the first 30 seconds (the "spatial" component);
//! * The change in magnitude of each frequency component of ten frames from the first 30 seconds (the "temporal" component)
//!
//! Therefore, this library will find videos which for a given tolerance "look the same", "have the same movement" and "have the same length"
//!
//! # High Level API
//! First provide the paths to a set of video files and turn them into hashes
//! Then, use one of the duplicate detection functions to discover which videos are duplicates
//! of each other.
//! ```rust,
//! use vid_dup_finder_lib::VideoHash;
//! use vid_dup_finder_lib::NormalizedTolerance;
//!
//! # use std::ffi::OsStr;
//! # let dup_vid_path_1 = std::env::current_dir().unwrap().join(OsStr::new("examples/vids/cat.1.mp4"));
//! # let dup_vid_path_2 = std::env::current_dir().unwrap().join(OsStr::new("examples/vids/cat.3.webm"));
//! # let other_vid_path = std::env::current_dir().unwrap().join(OsStr::new("examples/vids/dog.3.webm"));
//! // Paths to some vids to search for duplicates.
//! // Let's assume the first two videos are duplicates and the third is unrelated.
//! let vids = [dup_vid_path_1, dup_vid_path_2, other_vid_path];
//! let hashes = vids.iter().map(VideoHash::from_path).map(Result::unwrap);
//!
//! //Perform the search. dup_groups will detect the two duplicate videos
//! let tolerance = NormalizedTolerance::default();
//! let dup_groups = vid_dup_finder_lib::search(hashes, tolerance);
//! assert_eq!(dup_groups.len(), 1);
//! assert_eq!(dup_groups[0].len(), 2);
//! ```
//!
//! ## Search functions
//! The following search functions are available:
//! * To find all duplicate videos within a set: [`crate::search`]
//! * To find all duplicate videos using a set of reference videos: [`crate::search_with_references`]
//!
//! # Caching
//! To generate the hashes this library must decode the first 30 seconds of each video it processes
//! if there are a lot of viedos this takes a very long time. There is a companion crate called
//! video_hash_filesystem_cache which will store caches on disk in between searches, reducing the amount of time
//! spent loading videos.
//!
//! # Limitations
//! The library is specifically designed to find near-duplicate videos (i.e ones that have not been significantly edited).
//! Many transformations are capable of defeating it, such as rotation/flipping, watermarking, or time-offsetting.  
//! However if the transformations are minor (a faint watermark, a small crop etc) then this library should still
//! detect duplicate videos.
//!
//! Because the aim of this library is to find near-duplicates, the temporal and spatial hashes are generated from the first
//! 30 seconds of video content. This saves time
//!
//! Because this library is designed to detect near duplicates it only looks at the first 30 seconds of any video.
//! Therefore it is completely incapable of detecting if one video is a portion of another. For example you cannot
//! use it to detect a duplicate scene from an entire movie.
//!
//! This library is will not defeat "classic" methods of hiding duplicates, such as horizontal mirroring, changing
//! playback speed, or embedding video content in the corner of a static frame.
//!
//! todo: The concepts in this library can be extended to be able to detect the subset-videos described above
//!
//! ## False Positives
//! Because this library only checks the first 30 seconds of each video, if two videos are the same
//! length and share the first 30 seconds of video content, they will be reported as a false match. This
//! may occur for TV shows which contain opening credits.
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
//! exist to ffmpeg and/or gstreamer) and licensing reasons (statically linking to Ffmpeg may introduce additional
//! transitive licensing requirements on end users of this library),
//!
// //! # Gui and Command Line application
// //! There is a command line application

pub(crate) mod dct_hasher;
pub(crate) mod definitions;
pub(crate) mod utils;
pub(crate) mod video_hashing;

pub(crate) use video_hashing::distance::RawTolerance;

pub use video_hashing::{
    hash_creation_error_kind::HashCreationErrorKind,
    distance::{NormalizedDistance, NormalizedTolerance, RawDistance},
    matches::match_group::MatchGroup,
    video_dup_finder::*,
    video_hash::VideoHash,
    video_stats::*,
};

#[cfg(test)]
pub use video_hashing::video_hash::test_util;
