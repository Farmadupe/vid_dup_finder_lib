use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use FsCacheErrorKind::*;

use super::cache_interface::CacheInterface;
use super::{
    base_fs_cache::BaseFsCache,
    errors::{FsCacheErrorKind, FsCacheResult},
};

/// How a file on disk may have changed since the last time the cache was updated
enum UpdateAction {
    NoChange,
    Update(SystemTime),
    Remove,
}

#[derive(Serialize, Deserialize, Clone)]
struct MtimeCacheEntry<T> {
    cache_mtime: SystemTime,
    value: T,
}

pub struct ProcessingFsCache<I>
where
    I: CacheInterface,
{
    base_cache: BaseFsCache<MtimeCacheEntry<I::T>>,
    interface: I,
}

impl<I> ProcessingFsCache<I>
where
    I: CacheInterface + Send + Sync,
{
    pub fn new(
        cache_save_threshold: u32,
        cache_path: PathBuf,
        interface: I,
    ) -> FsCacheResult<Self> {
        match BaseFsCache::new(cache_save_threshold, cache_path) {
            Ok(base_cache) => Ok(Self {
                base_cache,
                interface,
            }),
            Err(e) => Err(e),
        }
    }

    pub fn save(&self) -> FsCacheResult<()> {
        self.base_cache.save()
    }

    #[inline]
    pub fn remove(&self, key: impl AsRef<Path>) -> FsCacheResult<()> {
        self.base_cache.remove(key)
    }

    #[inline]
    pub fn fetch(&self, key: impl AsRef<Path>) -> FsCacheResult<I::T> {
        match self.base_cache.fetch(key.as_ref()) {
            Ok(MtimeCacheEntry { value, .. }) => Ok(value),
            Err(e) => Err(e),
        }
    }

    #[inline]
    pub fn contains_key(&self, key: impl AsRef<Path>) -> bool {
        self.base_cache.contains_key(key)
    }

    #[inline]
    pub fn fetch_update(&self, key: impl AsRef<Path>) -> FsCacheResult<Option<I::T>> {
        //insertion required if:
        // * Item is not in cache.
        // * Cached item is out of date.

        let key = key.as_ref();

        match self.get_update_action(key)? {
            UpdateAction::NoChange => self.fetch(key).map(Option::from),
            UpdateAction::Update(fs_mtime) => {
                self.force_update_inner(key, fs_mtime).map(Option::from)
            }
            UpdateAction::Remove => self.remove(key).map(|_| None),
        }
    }

    // #[inline]
    // pub fn force_update(&self, key: impl AsRef<Path>) -> FsCacheResult<I::T> {
    //     let key = key.as_ref();

    //     self.force_update_inner(
    //         key,
    //         Self::fs_mtime(key).map_err(|e| FsCacheErrorKind::CacheFileIo {
    //             path: key.to_path_buf(),
    //             src: e,
    //         })?,
    //     )
    // }

    fn force_update_inner(&self, key: impl AsRef<Path>, mtime: SystemTime) -> FsCacheResult<I::T> {
        let key = key.as_ref();

        let value = self.interface.load(key);
        let cache_entry = MtimeCacheEntry {
            cache_mtime: mtime,
            value,
        };
        self.base_cache.insert(key.to_path_buf(), cache_entry)?;

        self.fetch(key)
    }

    // #[inline]
    // pub fn contains_key(&self, key: &Path) -> bool {
    //     self.base_cache.contains_key(key)
    // }

    #[inline]
    pub fn keys(&self) -> Vec<PathBuf> {
        self.base_cache.keys()
    }

    // #[inline]
    // pub fn len(&self) -> usize {
    //     self.base_cache.len()
    // }

    // #[inline]
    // pub fn is_empty(&self) -> bool {
    //     self.base_cache.is_empty()
    // }

    fn fs_mtime(key: &Path) -> Result<SystemTime, std::io::Error> {
        fs::metadata(key)?.modified()
    }

    // helper function to get whether a particular path has been updated in the filesystem.
    // Contains a hacky workaround for a problem where SSHFS (and presumably FUSE underneath)
    // reports different mtimes for files compared to a backing BTRFS filesystem (FUSE/sshfs probably
    // reports less granular mtimes?), where a file will only be considered stale if the mtime
    // is different by more than DURATION_TOLERANCE.
    fn get_update_action(&self, key: &Path) -> FsCacheResult<UpdateAction> {
        // debug: switch between ignoring nanos and not (current  workaround for nanos-difference might be causing issues?)
        let include_nanos = false;

        //If the path is not present on the filesystem, then remove it from the cache
        //(it may have never existed in the cache but this is OK)
        let fs_mtime = match Self::fs_mtime(key) {
            Ok(fs_mtime) => fs_mtime,
            Err(e) => match e.kind() {
                std::io::ErrorKind::NotFound => return Ok(UpdateAction::Remove),
                _ => {
                    return Err(CacheFileIo {
                        path: key.to_path_buf(),
                        src: e,
                    })
                }
            },
        };

        //if the file exists on the filesystem but not in the cache, we will insert it.
        let cache_mtime = match self.base_cache.fetch(key) {
            Ok(entry) => entry.cache_mtime,
            Err(_e) => return Ok(UpdateAction::Update(fs_mtime)),
        };

        //otherwise, see if the file is changed...
        let is_stale = if include_nanos {
            //original implementation used the following code, which produced errors as SystemTime::duration_since
            //appears to return an error if only the nanos portion of the fields differ
            fs_mtime != cache_mtime
        } else {
            // To fix the problem the durations are converted seconds since unix epoch.
            const DURATION_TOLERANCE_SECS: i64 = 2;
            let cache_mtime_secs = cache_mtime
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            let fs_mtime_secs = fs_mtime
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            (cache_mtime_secs - fs_mtime_secs).abs() > DURATION_TOLERANCE_SECS
        };

        if is_stale {
            Ok(UpdateAction::Update(fs_mtime))
        } else {
            Ok(UpdateAction::NoChange)
        }
    }
}
