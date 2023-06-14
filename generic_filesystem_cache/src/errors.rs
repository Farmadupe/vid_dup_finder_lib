use std::{fmt::Debug, path::PathBuf};

use thiserror::Error;

pub type FsCacheResult<T> = Result<T, FsCacheErrorKind>;

#[derive(Error, Debug)]
pub enum FsCacheErrorKind {
    #[error("Error accessing cache storage file {path}: {src}")]
    CacheFileIo { src: std::io::Error, path: PathBuf },

    #[error("IO error accessing {src}: {path}")]
    CacheItemIo { src: String, path: PathBuf },

    #[error("Key missing from cache: {0}")]
    KeyMissing(PathBuf),

    #[error("Failed to serialize items from cache file {path}: {src}")]
    Serialization { src: String, path: PathBuf },

    #[error("Failed to deserialize items from cache file {path}: {src}")]
    Deserialization { src: String, path: PathBuf },
}
