use super::generic_filesystem_cache::*;
use thiserror::Error;
use vid_dup_finder_lib::*;

/// Errors occurring while inserting or removing an item from a cache.
#[derive(Error, Debug)]
pub enum VdfCacheError {
    /// An error occurred when creating a [VideoHash]
    #[error(transparent)]
    CreateHashError(#[from] Error),

    #[error("Metadata validation error: {0}")]
    MetadataValidationError(String),

    /// An caching error occurred.
    #[error(transparent)]
    CacheErrror(#[from] FsCacheErrorKind),
}
