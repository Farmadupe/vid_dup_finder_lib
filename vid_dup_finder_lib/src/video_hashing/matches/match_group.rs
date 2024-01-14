use itertools::Itertools;
use std::{
    convert::From,
    path::{Path, PathBuf},
    vec::Vec,
};

/// A group of duplicate videos detected by `[crate::search]` or [`crate::search_with_references`].
///
/// If the search was performed against a set of references, the reference is included.
///
/// A `MatchGroup` can be queried for the paths of the videos that it contains.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct MatchGroup {
    reference: Option<PathBuf>,
    duplicates: Vec<PathBuf>,
}

impl MatchGroup {
    #[doc(hidden)]
    ///Create a new matchgroup by supplying the paths of the matching items.
    pub fn new(entries: impl IntoIterator<Item = PathBuf>) -> Self {
        let duplicates = entries.into_iter().collect::<Vec<_>>();

        debug_assert!(duplicates.len() >= 2);

        Self {
            reference: None,
            duplicates,
        }
    }

    #[doc(hidden)]
    ///Create a new MatchGroup by supplying the paths of the maching items, and the path of
    ///the reference video.
    pub fn new_with_reference(
        reference: PathBuf,
        entries: impl IntoIterator<Item = PathBuf>,
    ) -> Self {
        let duplicates = entries.into_iter().collect::<Vec<_>>();

        debug_assert!(!duplicates.is_empty());

        Self {
            reference: Some(reference),
            duplicates,
        }
    }

    /// The number of duplicate videos in this group.
    #[must_use]
    pub fn len(&self) -> usize {
        self.duplicates.len()
    }

    /// The path to the reference video, if it exists.
    #[must_use]
    pub fn reference(&self) -> Option<&Path> {
        self.reference.as_deref()
    }

    /// An iterator for the paths of the duplicates in this `MatchGroup`
    pub fn duplicates(&self) -> impl Iterator<Item = &Path> {
        self.duplicates.iter().map(&PathBuf::as_path)
    }

    /// All the paths in this `MatchGroup`, regardless
    /// of whether the path is a reference or not

    pub fn contained_paths(&self) -> impl Iterator<Item = &Path> {
        // a scratch iterator to return the reference video, if there is one.
        let mut done = false;
        let ref_as_iter = std::iter::from_fn(move || {
            if done {
                None
            } else {
                done = true;
                self.reference.as_deref()
            }
        });

        self.duplicates().chain(ref_as_iter)
    }

    /// Returns all combinations of duplicate videos in this group.
    /// If there is no reference video, then this is every video paired with
    /// every other video. If there is a video, then returns every video
    /// paired with the reference.
    #[must_use]
    pub fn dup_combinations(&self) -> Vec<Self> {
        match &self.reference {
            Some(ref r) => self
                .duplicates
                .iter()
                .cloned()
                .map(|dup| Self::new_with_reference(r.clone(), std::iter::once(dup)))
                .collect(),

            None => self
                .duplicates
                .iter()
                .cloned()
                .tuple_combinations::<(_, _)>()
                .map(|(h1, h2)| Self::new([h1, h2]))
                .collect(),
        }
    }
}

impl<I, T> From<I> for MatchGroup
where
    I: IntoIterator<Item = T>,
    T: AsRef<Path>,
{
    fn from(it: I) -> Self {
        Self::new(it.into_iter().map(|x| x.as_ref().to_path_buf()))
    }
}
