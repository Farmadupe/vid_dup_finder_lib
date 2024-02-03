use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use generic_filesystem_cache::*;

use crate::{cache_metadata::VdfCacheMetadata, *};
#[cfg(feature = "parallel_loading")]
use rayon::prelude::*;
use vid_dup_finder_lib::{CropdetectType, HashCreationErrorKind, VideoHash};

use super::generic_cache_if::GenericCacheIf;

/// A disk-backed cache for hashes of videos on the filesystem.
/// This is a utility struct for long term storage of [VideoHashes][vid_dup_finder_lib::VideoHash].
/// The cache tracks modification times of the underlying video files, and will automatically
/// recalculate hashes based on this.
///
/// Cache entries are created and retrieved by calling [fetch_update][`VideoHashFilesystemCache::fetch_update`] with the path to a video
/// on disk. If there is no entry in the cache, or the modification time of the video is newer then
/// the cache will create a video hash for the underlying file. If the video is already cached then
/// the cache will supply its cached data
///
/// Hashes can be obtained from the cache without visiting the underlying video on the filesystem with
/// [fetch][`VideoHashFilesystemCache::fetch`].
///
/// To update all hashes within a given directory (or set of directories) use [update_using_fs][`VideoHashFilesystemCache::update_using_fs`]
///
/// # A note on interior mutability
/// All methods on this struct and its [underlying implementation][generic_filesystem_cache::ProcessingFsCache] are use
/// interior mutability allow for operations to occur in parallel.
pub struct VideoHashFilesystemCache(ProcessingFsCache<GenericCacheIf>);

impl VideoHashFilesystemCache {
    /// Load a VideoHash cache from disk the specified path. If no cache exists at cache_path
    /// then a new cache will be created.
    ///
    /// The cache will automatically save its contents to disk when cache_save_threshold write/delete
    /// operations have occurred to the cache.
    ///
    /// Note: The cache does not automatically save its contents when it goes out of scope. You must manually
    /// call [save][`VideoHashFilesystemCache::save`] after you have made the last modification to the chache contents.
    ///
    /// Returns an error if it was not possible to load the cache or create a new one.
    pub fn new(
        cache_save_thresold: u32,
        cache_path: PathBuf,
        cropdetect: CropdetectType,
        skip_forward_amount: f64,
    ) -> Result<Self, VdfCacheError> {
        Self::validate_or_create_metadata_file(&cache_path, cropdetect, skip_forward_amount)?;

        let interface = GenericCacheIf::new(skip_forward_amount, cropdetect);

        let ret = ProcessingFsCache::new(cache_save_thresold, cache_path, interface)?;
        Ok(Self(ret))
    }

    fn create_metadata_file(
        metadata_path: impl AsRef<Path>,
        cropdetect: CropdetectType,
        skip_forward_amount: f64,
    ) -> Result<(), VdfCacheError> {
        let content = VdfCacheMetadata::new(cropdetect, skip_forward_amount).to_disk_fmt();

        std::fs::write(metadata_path.as_ref(), content).map_err(|e| {
            VdfCacheError::CacheErrror(FsCacheErrorKind::CacheFileIo {
                src: e,
                path: metadata_path.as_ref().to_path_buf(),
            })
        })?;

        Ok(())
    }

    fn validate_or_create_metadata_file(
        cache_path: impl AsRef<Path>,
        cropdetect: CropdetectType,
        skip_forward_amount: f64,
    ) -> Result<(), VdfCacheError> {
        let cache_path = cache_path.as_ref();
        let cache_exists = cache_path.exists();

        let cache_stem = &cache_path
            .file_stem()
            .map(|x| x.to_string_lossy().to_string())
            .ok_or_else(|| {
                VdfCacheError::CacheErrror(FsCacheErrorKind::CacheFileIo {
                    src: std::io::Error::from_raw_os_error(22),
                    path: cache_path.to_path_buf(),
                })
            })?;

        let metadata_path = &cache_path.with_file_name(format!("{cache_stem}.metadata.txt"));
        let metadata_exists = metadata_path.exists();

        if !cache_exists {
            Self::create_metadata_file(metadata_path, cropdetect, skip_forward_amount)?;
            return Ok(());
        }

        if cache_exists && !metadata_exists {
            dbg!(cache_path);
            dbg!(metadata_path);
            panic!("Cache exists but metadata is absent");
        };

        if !metadata_exists {
            Self::create_metadata_file(metadata_path, cropdetect, skip_forward_amount)?;
            return Ok(());
        }

        let content = std::fs::read_to_string(metadata_path).map_err(|e| {
            VdfCacheError::CacheErrror(FsCacheErrorKind::CacheFileIo {
                src: e,
                path: metadata_path.clone(),
            })
        })?;

        let act_metadata = VdfCacheMetadata::try_parse(&content)
            .map_err(VdfCacheError::MetadataValidationError)?;

        act_metadata
            .validate(cropdetect, skip_forward_amount)
            .map_err(VdfCacheError::MetadataValidationError)?;

        Ok(())
    }

    /// Fetch the hash for the video file at the given source path. If the cache does not already contain a hash
    /// will not create one. This method does not read ``src_path`` on the filesystem.
    ///
    /// Returns an error if the cache has no entry for `src_path` .
    #[inline]
    pub fn fetch(&self, src_path: impl AsRef<Path>) -> Result<VideoHash, VdfCacheError> {
        self.fetch_entry(src_path).map_err(VdfCacheError::from)
    }

    /// Get the paths of all [VideoHashes][VideoHash] stored in the cache.
    #[inline]
    pub fn all_cached_paths(&self) -> Vec<PathBuf> {
        self.0
            .keys()
            .into_iter()
            .filter(|src_path| self.fetch(src_path).is_ok())
            .collect()
    }

    pub fn error_paths(&self) -> Vec<PathBuf> {
        self.0
            .keys()
            .into_iter()
            .filter(|src_path| self.fetch(src_path).is_err())
            .collect()
    }

    /// If ``src_path`` has not been modified since it was cached, then return the cached hash.
    /// If ``src_path`` has been deleted, then remove it from the cache and return None.
    /// Otherwise create a new hash, insert it into the cache, and return it.
    ///
    /// Returns an error if it was not possible to generate a hash from `src_path`.
    #[inline]
    pub fn fetch_update(
        &self,
        src_path: impl AsRef<Path>,
    ) -> Result<Option<Result<VideoHash, HashCreationErrorKind>>, VdfCacheError> {
        self.0.fetch_update(src_path).map_err(VdfCacheError::from)
    }

    #[inline]
    pub fn force_update(
        &self,
        src_path: impl AsRef<Path>,
    ) -> Result<Option<Result<VideoHash, HashCreationErrorKind>>, VdfCacheError> {
        let _ = self.0.remove(&src_path);
        self.0.fetch_update(&src_path).map_err(VdfCacheError::from)
    }

    /// Save the cache to disk.
    ///
    ///Returns an error if it was not possible to write the cache to disk.
    #[inline]
    pub fn save(&self) -> Result<(), VdfCacheError> {
        self.0.save().map_err(VdfCacheError::from)
    }

    /// For all files on the filesystem matching ``file_projection``, update the cache for all new or modified files.
    /// Also, remove items from the cache if they no longer exist in the underlying filesystem.
    ///
    /// # Return values
    /// This function will return ``Err`` if any fatal error occurs. Otherwise, it returns a group
    /// of nonfatal errors, typically a list of paths for which a [`VideoHash`] could not be generated.
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
    pub fn update_using_fs(
        &self,
        fs_paths: impl IntoIterator<Item = impl AsRef<Path>>,
        force_load: bool,
    ) -> Result<Vec<VdfCacheError>, VdfCacheError> {
        let mut errs_ret = vec![];

        //deduplicate all the items for loading with a HashSet.
        let mut all_paths = HashSet::new();
        all_paths.extend(fs_paths.into_iter().map(|p| p.as_ref().to_path_buf()));

        if !force_load {
            all_paths.extend(self.all_cached_paths());
        }

        let mut all_paths = all_paths.into_iter().collect::<Vec<_>>();
        all_paths.sort_by_cached_key(|x| {
            let k1 = x.extension().unwrap_or_default().to_owned();
            let k2 = x.to_owned();

            (k1, k2)
        });

        //Delete those items which have disappeared from the filesystem,
        // and add what's new.
        #[cfg(feature = "parallel_loading")]
        {
            let errs = all_paths.into_iter().par_bridge().filter_map(|path| {
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
    fn fetch_entry(&self, src_path: impl AsRef<Path>) -> Result<VideoHash, VdfCacheError> {
        match self.0.fetch(src_path) {
            Ok(x) => x.map_err(VdfCacheError::from),
            Err(e) => Err(VdfCacheError::from(e)),
        }
    }
}
