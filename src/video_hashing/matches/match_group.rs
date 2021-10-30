use std::{convert::From, path::Path, vec::Vec};

use crate::*;

/// A group of duplicate videos detected by [crate::search] or [crate::search_with_references].
///
/// If the search was performed against a set of references, the reference is included.
///
/// A MatchGroup can be queried for the paths of the videos that it contains.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
pub struct MatchGroup {
    reference: Option<VideoHash>,
    duplicates: Vec<VideoHash>,
}

impl MatchGroup {
    ///Create a new matchgroup by supplying the hashes of the matching items.
    pub fn new(entries: impl IntoIterator<Item = VideoHash>) -> Self {
        Self {
            reference: None,
            duplicates: entries.into_iter().collect(),
        }
    }

    ///Create a new MatchGroup by supplying the hashes of the maching items, and the hash of
    ///the reference video.
    pub fn new_with_reference(reference: VideoHash, entries: impl IntoIterator<Item = VideoHash>) -> Self {
        Self {
            reference: Some(reference),
            duplicates: entries.into_iter().collect(),
        }
    }

    /// The number of duplicate videos in this group.
    pub fn len(&self) -> usize {
        self.duplicates.len()
    }

    /// The path to the reference video, if it exists.
    pub fn reference(&self) -> Option<&Path> {
        self.reference.as_ref().map(&VideoHash::src_path)
    }

    /// An iterator for the paths of the duplicates in this MatchGroup
    pub fn duplicates(&self) -> impl Iterator<Item = &Path> {
        self.duplicates.iter().map(|x| x.src_path())
    }
}

impl<I, T> From<I> for MatchGroup
where
    I: IntoIterator<Item = T>,
    T: std::borrow::Borrow<VideoHash>,
{
    fn from(it: I) -> MatchGroup {
        MatchGroup::new(it.into_iter().map(|x| x.borrow().clone()))
    }
}
