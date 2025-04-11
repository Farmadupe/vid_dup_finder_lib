use std::path::{Path, PathBuf};

use super::generic_filesystem_cache::*;

use itertools::Itertools;
#[cfg(feature = "parallel_loading")]
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Error, Serialize, Deserialize)]
pub enum FileContentCacheErrorKind {
    #[error("IO error: {0}")]
    Io(String),
    #[error("Cache error: {0}")]
    Cache(String),
}

impl From<std::io::Error> for FileContentCacheErrorKind {
    fn from(_value: std::io::Error) -> Self {
        Self::Io("IO error".to_owned())
    }
}

impl From<FsCacheErrorKind> for FileContentCacheErrorKind {
    fn from(_value: FsCacheErrorKind) -> Self {
        Self::Cache("Cache error".to_owned())
    }
}

pub struct FileContentCacheIf {}

impl FileContentCacheIf {
    pub const fn new() -> Self {
        Self {}
    }
}

impl CacheInterface for FileContentCacheIf {
    type T = Result<blake3::Hash, FileContentCacheErrorKind>;

    fn load(&self, src_path: impl AsRef<Path>) -> Self::T {
        let mut hasher = blake3::Hasher::new();
        let new_entry = hasher
            .update_mmap(src_path.as_ref())
            .map_err(FileContentCacheErrorKind::from);
        match &new_entry {
            Ok(_) => info!(target: "hash_creation",
                "contents caching : {}",
                src_path.as_ref().to_string_lossy()
            ),
            Err(e) => warn!(target: "hash_creation", "Hashing failed: {}", e.to_string()),
        }
        Ok(hasher.finalize())
    }
}

pub struct FileContentCache(ProcessingFsCache<FileContentCacheIf>);

impl FileContentCache {
    /// Load a VideoHash cache from disk the specified path. If no cache exists at cache_path
    /// then a new cache will be created.
    ///
    /// The cache will automatically save its contents to disk when cache_save_threshold write/delete
    /// operations have occurred to the cache.
    ///
    /// Note: The cache does not automatically save its contents when it goes out of scope. You must manually
    /// call [`Self::save`] after you have made the last modification to the chache contents.
    ///
    /// Returns an error if it was not possible to load the cache or create a new one.
    pub fn new(
        cache_save_thresold: u32,
        cache_path: PathBuf,
    ) -> Result<Self, FileContentCacheErrorKind> {
        let interface = FileContentCacheIf::new();

        let ret = ProcessingFsCache::new(cache_save_thresold, cache_path, interface)?;
        Ok(Self(ret))
    }

    /// Fetch the hash for the video file at the given source path. If the cache does not already contain a hash
    /// will not create one. This method does not read ``src_path`` on the filesystem.
    ///
    /// Returns an error if the cache has no entry for `src_path` .
    #[inline]
    pub fn fetch(
        &self,
        src_path: impl AsRef<Path>,
    ) -> Result<blake3::Hash, FileContentCacheErrorKind> {
        self.fetch_entry(src_path)
    }

    /// Get the paths of all VideoHashes stored in the cache.
    #[inline]
    pub fn all_cached_paths(&self) -> Vec<PathBuf> {
        self.0
            .keys()
            .into_iter()
            .filter(|src_path| self.fetch(src_path).is_ok())
            .collect()
    }

    // pub fn error_paths(&self) -> Vec<PathBuf> {
    //     self.0
    //         .keys()
    //         .into_iter()
    //         .filter(|src_path| self.fetch(src_path).is_err())
    //         .collect()
    // }

    /// If ``src_path`` has not been modified since it was cached, then return the cached hash.
    /// If ``src_path`` has been deleted, then remove it from the cache and return None.
    /// Otherwise create a new hash, insert it into the cache, and return it.
    ///
    /// Returns an error if it was not possible to generate a hash from `src_path`.
    #[inline]
    pub fn fetch_update(
        &self,
        src_path: impl AsRef<Path>,
    ) -> Result<Option<Result<blake3::Hash, FileContentCacheErrorKind>>, FileContentCacheErrorKind>
    {
        let ret = self
            .0
            .fetch_update(src_path)
            .map_err(FileContentCacheErrorKind::from);
        ret
    }

    #[inline]
    pub fn force_update(
        &self,
        src_path: impl AsRef<Path>,
    ) -> Result<Option<Result<blake3::Hash, FileContentCacheErrorKind>>, FileContentCacheErrorKind>
    {
        let _ = self.0.remove(&src_path);
        self.0
            .fetch_update(&src_path)
            .map_err(FileContentCacheErrorKind::from)
    }

    /// Save the cache to disk.
    ///
    ///Returns an error if it was not possible to write the cache to disk.
    #[inline]
    pub fn save(&self) -> Result<(), FileContentCacheErrorKind> {
        self.0.save().map_err(FileContentCacheErrorKind::from)
    }

    /// For all files on the filesystem matching ``file_projection``, update the cache for all new or modified files.
    /// Also, remove items from the cache if they no longer exist in the underlying filesystem.
    ///
    /// # Return values
    /// This function will return ``Err`` if any fatal error occurs. Otherwise, it returns a group
    /// of nonfatal errors, typically a list of paths for which a VideoHash could not be generated.
    ///
    /// ## Fatal errors
    ///    * Unable to read any of the starting directories in ``file_projection``
    ///    * Any Io error when reading/writing to the cache file itself.
    ///
    /// ## Nonfatal errors
    ///    * Failure to create a hash from any individual file.
    ///    * Failure to remove an item from the cache (This is unlikely and should only occur if
    ///      calling this function more than once at the same time with overlapping paths)
    ///
    /// # Parallelism
    /// To speed up loading there is a cargo feature to allow hashes to be created from videos in parallel.
    /// Parallel loading is much faster than sequential loading but be aware that since Ffmpeg is already multithreaded
    /// this can use up a lot of CPU time.
    #[inline]
    pub fn update_using_fs<T>(
        &self,
        fs_paths: T,
        force_load: bool,
    ) -> Result<Vec<FileContentCacheErrorKind>, FileContentCacheErrorKind>
    where
        T: IntoIterator<Item = PathBuf>,
        <T as IntoIterator>::IntoIter: Send,
    {
        let mut errs_ret = vec![];

        if force_load {
            for p in self.all_cached_paths() {
                self.0.remove(p).unwrap();
            }
        }

        //deduplicate all the items for loading with a HashSet.
        let all_paths = fs_paths.into_iter().unique();
        // .collect::<HashSet<_>>();

        // all_paths.sort_by_cached_key(|x| {
        //     let k1 = x.extension().unwrap_or_default().to_owned();
        //     let k2 = x.to_owned();

        //     (k1, k2)
        // });

        //Delete those items which have disappeared from the filesystem,
        // and add what's new.
        #[cfg(feature = "parallel_loading")]
        {
            let errs = all_paths.par_bridge().filter_map(|path| {
                if force_load {
                    self.force_update(&path).err()
                } else {
                    self.fetch_update(&path).err()
                }
            });
            errs_ret.par_extend(errs);
        }

        #[cfg(not(feature = "parallel_loading"))]
        {
            for path in all_paths {
                if let Err(e) = if force_load {
                    self.force_update(&path)
                } else {
                    self.fetch_update(&path)
                } {
                    errs_ret.push(e);
                }
            }
        }
        Ok(errs_ret)
    }

    #[inline]
    fn fetch_entry(
        &self,
        src_path: impl AsRef<Path>,
    ) -> Result<blake3::Hash, FileContentCacheErrorKind> {
        match self.0.fetch(src_path) {
            Ok(x) => x,
            Err(e) => Err(FileContentCacheErrorKind::from(e)),
        }
    }
}
