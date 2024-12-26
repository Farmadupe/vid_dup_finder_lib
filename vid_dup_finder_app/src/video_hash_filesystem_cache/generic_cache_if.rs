use std::path::Path;

use super::generic_filesystem_cache::*;
use vid_dup_finder_lib::*;

pub struct GenericCacheIf {
    skip_forward_amount: f64,
    duration: f64,
    cropdetect: Cropdetect,
}

impl GenericCacheIf {
    pub const fn new(skip_forward_amount: f64, duration: f64, cropdetect: Cropdetect) -> Self {
        Self {
            skip_forward_amount,
            duration,
            cropdetect,
        }
    }
}

impl CacheInterface for GenericCacheIf {
    type T = Result<VideoHash, Error>;

    fn load(&self, src_path: impl AsRef<Path>) -> Self::T {
        let src_path = src_path.as_ref().to_path_buf();
        let opts = CreationOptions {
            skip_forward_amount: self.skip_forward_amount,
            duration: self.duration,
            cropdetect: self.cropdetect,
        };

        #[cfg(feature = "gstreamer_backend")]
        let new_entry =
            gstreamer_builder::VideoHashBuilder::from_options(opts).hash(src_path.clone());

        #[cfg(feature = "ffmpeg_backend")]
        let new_entry = ffmpeg_builder::VideoHashBuilder::from_options(opts).hash(src_path.clone());

        match &new_entry {
            Ok(hash) => info!(target: "hash_creation",
                "inserting : {}",
                hash.src_path().display()
            ),
            Err(e) => {
                let err_msg = e.to_string();
                let src_path_string = src_path.to_string_lossy();
                warn!(target: "hash_creation", "Hashing failed: {err_msg}: {src_path_string}")
            }
        }

        new_entry
    }
}
