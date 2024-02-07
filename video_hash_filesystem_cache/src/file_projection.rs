#![warn(clippy::panic)]
#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]

use std::{
    collections::{hash_map::RandomState, HashSet},
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    result::Result,
};

use itertools::Itertools;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use walkdir::WalkDir;

/// Errors encountered during the file enumeration process.
#[derive(Error, Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum FileProjectionError {
    /// A src_path or excl_path could not be read from the filesystem.
    #[error("Path not found: {0:?}")]
    SrcPathNotFound(Vec<PathBuf>),

    #[error("Excl path not found: {0:?}")]
    ExclPathNotFound(Vec<PathBuf>),

    /// a src_path is excluded by an excl_path.
    #[error("A start path is excluded by an excl path")]
    SrcPathExcluded {
        src_path: PathBuf,
        excl_path: PathBuf,
    },
}

#[derive(Error, Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[error("File enumeration failed: {0}")]
pub struct FilesystemError(String);

impl FilesystemError {
    pub fn into_message(self) -> String {
        self.0
    }
}

impl From<walkdir::Error> for FilesystemError {
    fn from(e: walkdir::Error) -> Self {
        Self(format!("{e}"))
    }
}

impl From<&walkdir::Error> for FilesystemError {
    fn from(e: &walkdir::Error) -> Self {
        Self(format!("{e}"))
    }
}

pub enum FileProjectionOutcome {
    File(PathBuf),
    RecoverableErr(FileProjectionError),
    FatalErr(FileProjectionError),
}

/// A utility struct for holding a set of paths, and all children from those paths.
/// Contains an associated set of "exclude" paths whose children should not be returned.
#[derive(Debug, Clone)]
pub struct FileProjection {
    src_paths: Vec<PathBuf>,
    excl_paths: Vec<PathBuf>,
    projected_files: HashSet<PathBuf, RandomState>,
    excl_exts: Vec<OsString>,
}

impl FileProjection {
    /// Create a new FileProjection with the given src_paths, excl_paths and ignore-extensions
    /// Child files can either be got by projecting the src_paths, either from
    /// the filesystem (project_using_fs), or from some list (project_using_list).
    /// Once projection has occurred the projected files will be cached by this struct
    /// (This feature is mostly to avoid having to visit the filesystem more than once
    /// when performing large projections)
    ///
    /// Projected files can be retrieved by calling [projected_files][Self::projected_files]
    pub fn new(
        src_paths: impl IntoIterator<Item = impl AsRef<Path>>,
        excl_paths: impl IntoIterator<Item = impl AsRef<Path>>,
        excl_exts: impl IntoIterator<Item = impl AsRef<OsStr>>,
    ) -> Result<Self, FileProjectionError> {
        use FileProjectionError::*;

        let src_paths = src_paths
            .into_iter()
            .map(|p| p.as_ref().to_path_buf())
            .collect::<Vec<_>>();

        let excl_path_is_used = |excl_path: &PathBuf| {
            src_paths
                .iter()
                .any(|src_path| excl_path.starts_with(src_path))
        };

        let excl_paths = excl_paths
            .into_iter()
            .map(|p| p.as_ref().to_path_buf())
            .filter(excl_path_is_used)
            .collect::<Vec<_>>();

        //check that the same path does not appear in srcs and excls
        for src_path in &src_paths {
            for excl_path in &excl_paths {
                if src_path == excl_path {
                    let src_path = src_path.to_path_buf();
                    let excl_path = excl_path.to_path_buf();
                    return Err(SrcPathExcluded {
                        src_path,
                        excl_path,
                    });
                }
            }
        }

        Ok(Self {
            src_paths,
            excl_paths,
            projected_files: Default::default(),
            excl_exts: excl_exts
                .into_iter()
                .map(|x| x.as_ref().to_os_string())
                .collect(),
        })
    }

    pub fn is_empty(&self) -> bool {
        self.src_paths.is_empty()
    }

    /// Returns true if the given path is a child of any src_path,
    /// and is not a child of any excl_path.
    pub fn contains(&self, src_path: impl AsRef<Path>) -> bool {
        self.raw_includes(&src_path) && !self.raw_excludes(&src_path)
    }

    fn raw_includes(&self, p: impl AsRef<Path>) -> bool {
        self.src_paths
            .iter()
            .any(|src_path| p.as_ref().starts_with(src_path))
    }

    fn raw_excludes(&self, p: impl AsRef<Path>) -> bool {
        self.excl_paths
            .iter()
            .any(|excl_path| p.as_ref().starts_with(excl_path))
    }

    /// Visit the filesystem to get all child files which are a child of any of Self::src_paths,
    /// and which are not a child of Self::excl_paths.
    pub fn project_using_fs(&mut self) -> Result<Vec<FilesystemError>, FileProjectionError> {
        use FileProjectionError::*;

        //we will return a fatal error if any directory/file that the user
        //has specified does not exist.
        let src_fatal = self
            .src_paths
            .iter()
            .filter(|&p| (!p.exists()))
            .map(|x| x.to_path_buf())
            .collect::<Vec<_>>();
        if !src_fatal.is_empty() {
            return Err(SrcPathNotFound(src_fatal));
        }

        let excl_fatal = self
            .excl_paths
            .iter()
            .filter(|&p| (!p.exists()))
            .map(|x| x.to_path_buf())
            .collect::<Vec<_>>();
        if !excl_fatal.is_empty() {
            return Err(ExclPathNotFound(excl_fatal));
        }

        let (enumerated_paths, recoverable_errs): (_, Vec<_>) = self
            .src_paths
            .iter()
            .flat_map(|src_path| {
                WalkDir::new(src_path).into_iter().filter_entry(|entry| {
                    let src_path = entry.path();
                    self.contains(src_path) && !self.has_ignore_ext(src_path)
                })
            })
            .filter_map(|dir_entry_res| match dir_entry_res {
                Err(e) => Some(Err(FilesystemError::from(e))),
                Ok(dir_entry) => {
                    let src_path = dir_entry.path();
                    if src_path.is_file() {
                        Some(Ok(src_path.to_path_buf()))
                    } else {
                        None
                    }
                }
            })
            .partition_result();

        self.projected_files = enumerated_paths;

        Ok(recoverable_errs)
    }

    /// Enumerate files by filtering a list of paths.
    pub fn project_using_list(&mut self, list: impl IntoIterator<Item = impl AsRef<Path>>) {
        self.projected_files = list
            .into_iter()
            .filter(|p| self.contains(p))
            .map(|x| x.as_ref().to_path_buf())
            .collect();
    }

    /// Obtain the set of all enumerated files. File enumeration must have already
    /// taken place.
    pub fn projected_files(&self) -> impl Iterator<Item = &Path> {
        self.projected_files.iter().map(|p| p.as_path())
    }

    pub fn projected_files2(&self) -> &HashSet<PathBuf> {
        &self.projected_files
    }

    fn has_ignore_ext(&self, src_path: &Path) -> bool {
        self.excl_exts.iter().any(|ext| {
            src_path
                .extension()
                .unwrap_or_default()
                .eq_ignore_ascii_case(ext)
        })
    }
}
