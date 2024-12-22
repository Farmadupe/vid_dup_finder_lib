use std::{
    fmt::Debug,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU32, Ordering::Relaxed},
};

use log::info;
use log::trace;
use parking_lot::RwLock;
use serde::{de::DeserializeOwned, Serialize};

#[allow(dead_code)]
enum SerializationBackend {
    Bincode,
    Json,
}

const BACKEND: SerializationBackend = SerializationBackend::Bincode;

use super::errors::{
    FsCacheErrorKind::{self, *},
    FsCacheResult,
};

//Types defining the on-disk format of the filesystem cacher.
type CacheDiskFormat<T> = std::collections::HashMap<PathBuf, T>;

#[derive(Default, Debug)]
pub struct BaseFsCache<T> {
    loaded_from_disk: bool,
    cache_save_threshold: u32,
    cache_modified_count: AtomicU32,
    cache_path: PathBuf,
    cache: RwLock<CacheDiskFormat<T>>,
}

impl<T> BaseFsCache<T>
where
    T: DeserializeOwned + Serialize + Send + Sync + Clone,
{
    pub fn new(cache_save_threshold: u32, cache_path: PathBuf) -> FsCacheResult<Self> {
        let mut ret = Self {
            loaded_from_disk: false,
            cache_save_threshold,
            cache_modified_count: AtomicU32::default(),
            cache_path,
            cache: RwLock::default(),
        };

        match ret.load_cache_from_disk() {
            Ok(()) => Ok(ret),
            Err(e) => Err(e),
        }
    }

    pub fn save(&self) -> FsCacheResult<()> {
        let modified_count = self.cache_modified_count.load(Relaxed);
        if modified_count > 0 {
            self.save_inner()
        } else {
            Ok(())
        }
    }

    fn save_inner(&self) -> FsCacheResult<()> {
        use std::io::BufWriter;

        //The cache file and its directory may not exist yet. So first create the directory
        //first if necessary.
        if !&self.cache_path.exists() {
            if let Some(ref parent_dir) = self.cache_path.parent() {
                if let Err(e) = std::fs::create_dir_all(parent_dir) {
                    return Err(CacheFileIo {
                        src: e,
                        path: self.cache_path.clone(),
                    });
                }
            }
        }

        //If the application dies or gets killed while saving, we risk losing the cache.
        //So we will first save the cache to a temporary file and rename it into the real
        //cache file.
        let temp_store_path = self.cache_path.with_extension("tmp");

        info!(
            target: "generic_cache_transactions",
            "saving updated cache at {} of size {}",

            self.cache_path.display(),
            self.cache.read().len()
        );

        let temp_cache_file = match std::fs::File::create(&temp_store_path) {
            Ok(temp_cache_file) => Ok(temp_cache_file),
            Err(e) => Err(CacheFileIo {
                src: e,
                path: self.cache_path.clone(),
            }),
        }?;

        let mut cache_buf = BufWriter::new(temp_cache_file);

        let readable_cache = self.cache.read();

        match BACKEND {
            SerializationBackend::Bincode => {
                if let Err(e) = bincode::serialize_into(&mut cache_buf, &*readable_cache) {
                    return Err(Serialization {
                        src: format!("{e}"),
                        path: self.cache_path.clone(),
                    });
                }
            }
            SerializationBackend::Json => {
                let json_string = match serde_json::to_string(&*readable_cache) {
                    Ok(s) => s,
                    Err(e) => {
                        return Err(Serialization {
                            src: format!("{e}"),
                            path: self.cache_path.clone(),
                        });
                    }
                };

                if let Err(e) = cache_buf.write_all(json_string.as_bytes()) {
                    return Err(Serialization {
                        src: format!("{e}"),
                        path: self.cache_path.clone(),
                    });
                }
            }
        }

        let temp_cache_file = match cache_buf.into_inner() {
            Err(e) => {
                return Err(CacheFileIo {
                    src: e.into_error(),
                    path: self.cache_path.clone(),
                })
            }
            Ok(x) => x,
        };

        if let Err(e) = temp_cache_file.sync_all() {
            return Err(CacheFileIo {
                src: e,
                path: self.cache_path.clone(),
            });
        }

        //now move the store to replace the old one.
        if let Err(e) = std::fs::rename(temp_store_path, &self.cache_path) {
            return Err(CacheFileIo {
                src: e,
                path: self.cache_path.clone(),
            });
        }

        Ok(())
    }

    fn load_cache_from_disk(&mut self) -> FsCacheResult<()> {
        //Try and read from disk. If there is nothing  available, this is not an error.
        //It just means that no cached values can be used. If so then go ahead and return early
        //as there is no deserialization to do.
        if !&self.cache_path.exists() {
            info!(target: "generic_cache_startup",
                "Creating new cache file: {}.", self.cache_path.display()
            );
            self.cache = RwLock::default();
            self.loaded_from_disk = true;
            return Ok(());
        }

        let cache_file = match std::fs::File::open(&self.cache_path) {
            Ok(f) => f,
            Err(e) => {
                return Err(CacheFileIo {
                    src: e,
                    path: self.cache_path.clone(),
                })
            }
        };

        //we may fail to read the hash file. This most likely to occur in development if <T> is changed.
        let reader = std::io::BufReader::new(cache_file);
        let cache_file_data: CacheDiskFormat<_> = match BACKEND {
            SerializationBackend::Bincode => match bincode::deserialize_from(reader) {
                Ok(data) => data,
                Err(e) => {
                    return Err(Deserialization {
                        src: format!("{e}"),
                        path: self.cache_path.clone(),
                    })
                }
            },
            SerializationBackend::Json => match serde_json::from_reader(reader) {
                Ok(data) => data,
                Err(e) => {
                    return Err(Deserialization {
                        src: format!("{e}"),
                        path: self.cache_path.clone(),
                    })
                }
            },
        };

        self.cache = RwLock::new(cache_file_data);
        self.loaded_from_disk = true;

        trace!(target: "generic_cache_startup",
            "Loaded cache. Path: {}, Entries: {}", self.cache_path.display(), self.len()
        );
        Ok(())
    }

    /////////////////////////////
    // Wrappers for HashMap.
    /////////////////////////////

    pub fn insert(&self, key: PathBuf, item: T) -> FsCacheResult<()> {
        let cache_modified_count = self.cache_modified_count.fetch_add(1, Relaxed);

        info!(target: "generic_cache_insert",
            "inserting : {}",
            key.display()
        );
        let cache_entry = item;
        {
            let mut writeable_cache = self.cache.write();
            writeable_cache.insert(key, cache_entry);
        }
        self.update_transaction_count_and_save_if_necessary(cache_modified_count)
    }

    pub fn remove(&self, key: impl AsRef<Path>) -> FsCacheResult<()> {
        {
            info!(target: "generic_cache_remove", "Removing: {}", key.as_ref().display());
            let mut writeable_cache = self.cache.write();
            writeable_cache.remove(key.as_ref());
        }
        let cache_modified_count = self.cache_modified_count.fetch_add(1, Relaxed);
        self.update_transaction_count_and_save_if_necessary(cache_modified_count)
    }

    fn update_transaction_count_and_save_if_necessary(&self, prev_count: u32) -> FsCacheResult<()> {
        // We need to defend against
        // 1) multiple saves of data when only one should be performed
        // 2) Failing to reset the cache_modified_count to 0. I think we
        // can guarantee both of these things with Relaxed accesses.
        //
        // todo: I think the above two points are true, but we should probably
        // guarantee better behaviour than that. I think at worst here, every
        // operation could trigger a save of the cache as cache_modified_count
        // isn't guaranteed to be sensibly propagated between threads.
        if prev_count == self.cache_save_threshold - 1 {
            self.cache_modified_count.store(0, Relaxed);
            self.save_inner()
        } else {
            Ok(())
        }
    }

    pub fn fetch(&self, key: &Path) -> Result<T, FsCacheErrorKind> {
        match self.cache.read().get(key) {
            Some(value) => Ok(value.clone()),
            None => Err(FsCacheErrorKind::KeyMissing(key.to_path_buf())),
        }
    }

    // pub fn contains_key(&self, key: &Path) -> bool {
    //     self.cache.read().contains_key(key)
    // }

    pub fn keys(&self) -> Vec<PathBuf> {
        self.cache.read().keys().cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.cache.read().len()
    }

    pub fn contains_key(&self, key: impl AsRef<Path>) -> bool {
        self.cache.read().contains_key(key.as_ref())
    }

    // pub fn is_empty(&self) -> bool {
    //     self.cache.read().is_empty()
    // }
}
