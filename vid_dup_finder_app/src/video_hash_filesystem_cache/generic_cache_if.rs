use std::path::Path;

use super::generic_filesystem_cache::*;
use vid_dup_finder_lib::*;

pub struct GenericCacheIf {
    skip_forward_amount: f64,
    cropdetect: CropdetectType,
}

impl GenericCacheIf {
    pub const fn new(skip_forward_amount: f64, cropdetect: CropdetectType) -> Self {
        Self {
            skip_forward_amount,
            cropdetect,
        }
    }
}

impl CacheInterface for GenericCacheIf {
    type T = Result<VideoHash, HashCreationErrorKind>;

    fn load(&self, src_path: impl AsRef<Path>) -> Self::T {
        let new_entry = VideoHash::from_path_with_options(
            src_path,
            HashCreationOptions {
                skip_forward_amount: self.skip_forward_amount,
                cropdetect: self.cropdetect,
            },
        );

        match &new_entry {
            Ok(hash) => info!(target: "hash_creation",
                "inserting : {}",
                hash.src_path().display()
            ),
            Err(e) => warn!(target: "hash_creation", "Hashing failed: {}", e.to_string()),
        }

        new_entry
    }
}
