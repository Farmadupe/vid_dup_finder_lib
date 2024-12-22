//! A utility crate with a cache for [vid_dup_finder_lib::VideoHash].
//! This crate defines struct [VideoHashFilesystemCache], which caches hashes of videos to disk.
//!
//! The cache stores the last modified time of the underlying files and will automatically
//! update when this changes.
//!
//! # Example (with individual files)
//! ```rust,
//! # use std::path::PathBuf;
//! # use std::ffi::OsStr;
//! use video_hash_filesystem_cache::*;
//! use vid_dup_finder_lib::*;
//!
//! //Gstreamer must be initialized first
//! vid_dup_finder_lib::init_gstreamer();
//!
//! // Create a cache on disk which will save itself to disk after every 100 changes
//! # let cache_file_path = PathBuf::from("vid_dup_finder_example_cache.bin");
//! let cache = VideoHashFilesystemCache::new(100, cache_file_path, None, false, false).expect("failed to create cache");
//!
//! // Now create a video hash by calling get_update on the cache.
//! # let vid_file_path = std::env::current_dir().unwrap().join(OsStr::new("../vid_dup_finder_lib/examples/vids/cat.1.mp4"));
//! let video_hash : VideoHash = match cache.fetch_update(&vid_file_path) {
//!    Ok(Some(Ok(hash)))  => hash,     // A hash was successfully created/fetched
//!    Ok(None)            => panic!(), // None is returned when vid_file_path is removed from the filesystem
//!    Ok(Some(Err(_e)))   => panic!(), // Ok(Some(Err())) is returned when an error occurs while creating a VideoHash
//!    Err(cache_error)    => panic!(), //"All other Io errors")
//! };
//!
//! // Subsequent calls will fetch the hash from the cache instead of creating it from the filesystem.
//!
//! // The cache must be saved to disk at the end of execution,
//! // otherwise changes since the last save will be lost.
//! cache.save().unwrap()
//!```
//!
//! # Caching many videos at once
//! the file projection is used for updating many files at once.
//! when created with a set of starting paths, it can be passed to the cache
//! to update all child files of those starting paths.
//!
//! ## Example (caching an entire directory)
//! ```rust,
//! # use std::path::PathBuf;
//! # use std::ffi::OsStr;
//! use video_hash_filesystem_cache::*;
//! use vid_dup_finder_lib::*;
//! //Gstreamer must be initialized first
//! vid_dup_finder_lib::init_gstreamer();
//!
//! // Create a cache on disk which will save itself to disk after every 100 changes
//! # let cache_file_path = PathBuf::from("vid_dup_finder_example_cache.bin");
//! let cache = VideoHashFilesystemCache::new(100, cache_file_path, None, false, false).expect("failed to create cache");
//!
//! // Create the projection representing two directories of video files.
//! // the second argument is a list of directories/paths to be ignored
//! # let video_dir_1 = std::env::current_dir().unwrap().join(OsStr::new("../vid_dup_finder_lib/examples/vids"));
//! # let video_dirs = vec![video_dir_1];
//! # let excl_dirs : Vec<PathBuf> = vec![];
//! # let excl_exts : Vec<&OsStr> = vec![];
//! let mut projection = FileProjection::new(&video_dirs, &excl_dirs, &excl_exts).unwrap();
//! let project_errs = projection.project_using_fs().unwrap();
//!
//! // Update the cache using the projection. a list of individual loading errors will be returned.
//! let cache_update_errs = cache.update_using_fs(projection.projected_files());
//!
//! // Now all videos under videos_dir_1 and videos_dir_2 will be cached.
//! // They can be retrieved from the cache without touching the filesystem using
//! // VideoHashFilesystemCache::fetch
//! # let vid_file_path = std::env::current_dir().unwrap().join(OsStr::new("../vid_dup_finder_lib/examples/vids/cat.1.mp4"));
//! let video_hash : VideoHash = cache.fetch(&vid_file_path).unwrap();
//!
//! // The cache must be saved to disk at the end of execution,
//! // otherwise changes since the last save will be lost.
//! cache.save().unwrap()
//! ```

#![deny(clippy::print_stdout)]
#![deny(clippy::print_stderr)]

pub(crate) mod cache_entry;
pub(crate) mod cache_metadata;
pub(crate) mod errors;
pub(crate) mod file_hash_filesystem_cache;
pub(crate) mod filename_pattern;
pub(crate) mod generic_cache_if;
pub(crate) mod generic_filesystem_cache;
#[allow(clippy::module_inception)]
pub(crate) mod video_hash_filesystem_cache;

//exports
pub use self::video_hash_filesystem_cache::VideoHashFilesystemCache;
pub use errors::VdfCacheError;
