use std::path::PathBuf;

use thiserror::Error;
use vid_dup_finder_lib::*;

use video_hash_filesystem_cache::*;

#[derive(Error, Debug)]
pub enum AppError {
    /////////////////////////////////
    // Argument parsing
    #[error("Args file not found at {0}")]
    ArgsFileNotFound(PathBuf, #[source] std::io::Error),

    #[error("Failed to parse args file at given location: {0}: {1}")]
    ArgsFileParse(PathBuf, String),

    /////////////////////////////////
    //Impossible combination of --files, --with-refs --exclude given.
    //It's important to get the wording of these right because these errors
    //are very easy to trigger.
    #[error("Path occurs in both --files and --with-refs: {0}")]
    PathInFilesAndRefs(PathBuf),

    #[error("Path in --files is excluded by --exclude. Path: {src_path}, Exclusion: {excl_path}")]
    SrcPathExcludedError {
        src_path: PathBuf,
        excl_path: PathBuf,
    },

    #[error(
        "Path in --with-refs is excluded by --exclude. Path: {src_path}, Exclusion: {excl_path}"
    )]
    RefPathExcludedError {
        src_path: PathBuf,
        excl_path: PathBuf,
    },

    #[error("Path in --files not found: {0:?}")]
    CandPathNotFoundError(Vec<PathBuf>),

    #[error("Path in --with-refs not found: {0:?}")]
    RefPathNotFoundError(Vec<PathBuf>),

    #[error("Path in --exclude not found: {0:?}")]
    ExclPathNotFoundError(Vec<PathBuf>),

    /////////////////////////////////
    //Other file projection problems
    #[error("Error while searching filesystem for videos: {0}")]
    FileSearchError(String),

    /////////////////////////////////
    //hash cache problems
    #[error(transparent)]
    CacheErrror(#[from] VdfCacheError),

    #[error("Hash Creation Error: {0}")]
    CreateHashError(#[from] HashCreationErrorKind),

    /////////////////////////////////
    //gui
    #[error("Failed to start the GUI")]
    #[allow(dead_code)] // variant is unused when gui is not compiled
    GuiStartError,
}
