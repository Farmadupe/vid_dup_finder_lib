use std::path::Path;

use serde::{de::DeserializeOwned, Serialize};

// Users of the generic filesystem cache should implement this interface.
pub trait CacheInterface {
    type T: Serialize + DeserializeOwned + Clone + Send + Sync;

    fn load(&self, src_path: impl AsRef<Path>) -> Self::T;
}
