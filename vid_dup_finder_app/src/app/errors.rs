use thiserror::Error;
use vid_dup_finder_lib::*;

use crate::video_hash_filesystem_cache::*;

use self::file_hash_filesystem_cache::FileContentCacheErrorKind;

#[derive(Error, Debug)]
pub enum AppError {
    /////////////////////////////////
    //hash cache problems
    #[error(transparent)]
    CacheErrror(#[from] VdfCacheError),

    #[error("Hash Creation Error: {0}")]
    CreateHashError(#[from] Error),

    /////////////////////////////////
    //file contents cache error
    #[error("File content cache error: {0}")]
    ContentCacheError(#[from] FileContentCacheErrorKind),

    /////////////////////////////////
    //gui
    #[error("Failed to start the GUI")]
    #[allow(dead_code)] // variant is unused when gui is not compiled
    GuiStartError,
}

pub fn print_error_and_quit(e: eyre::Report) -> ! {
    #[allow(clippy::print_stderr)]
    let () = eprintln!("{:?}", e);
    std::process::exit(1);
}
