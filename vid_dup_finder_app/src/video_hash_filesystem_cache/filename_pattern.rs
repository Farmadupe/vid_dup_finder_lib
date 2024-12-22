use eyre::eyre;
use ignore::WalkState;
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

pub trait FilterFilenames {
    fn includes(&self, src_path: impl AsRef<Path>) -> bool;
}

#[derive(Debug, Clone)]
pub struct FilenamePattern {
    incl_paths: Vec<PathBuf>,
    excl_paths: Vec<PathBuf>,
    excl_exts: Vec<OsString>,
}

impl FilenamePattern {
    // Generate a FilenamePattern. src_paths are the
    pub fn new(
        incl_paths: Vec<PathBuf>,
        excl_paths: Vec<PathBuf>,
        excl_exts: Vec<OsString>,
    ) -> eyre::Result<Self> {
        let ret = Self {
            incl_paths,
            excl_paths,
            excl_exts,
        };

        //check that the same path does not appear in srcs and excls
        if let Some(excluded_start_path) = ret
            .incl_paths
            .iter()
            .find(|incl_path| ret.raw_excludes(incl_path))
        {
            return Err(eyre::Report::msg(format!(
                "incl_path \"{}\" is excluded",
                excluded_start_path.to_string_lossy(),
            )));
        }

        Ok(ret)
    }

    fn raw_includes(&self, p: impl AsRef<Path>) -> bool {
        self.incl_paths
            .iter()
            .any(|src_path| p.as_ref().starts_with(src_path))
    }

    fn raw_excludes(&self, p: impl AsRef<Path>) -> bool {
        self.excl_paths
            .iter()
            .any(|excl_path| p.as_ref().starts_with(excl_path))
    }

    fn has_ignore_ext(&self, src_path: impl AsRef<Path>) -> bool {
        self.excl_exts.iter().any(|ext| {
            src_path
                .as_ref()
                .extension()
                .unwrap_or_default()
                .eq_ignore_ascii_case(ext)
        })
    }
}

impl FilterFilenames for &FilenamePattern {
    /// Returns true if the given path is a child of any src_path,
    /// and is not a child of any excl_path.
    fn includes(&self, src_path: impl AsRef<Path>) -> bool {
        self.raw_includes(&src_path)
            && !self.raw_excludes(&src_path)
            && !self.has_ignore_ext(&src_path)
    }
}

impl FilterFilenames for FilenamePattern {
    fn includes(&self, src_path: impl AsRef<Path>) -> bool {
        (&self).includes(src_path)
    }
}

//walkdir integration
impl FilenamePattern {
    //visit all files on the filesystem that are included.
    pub fn iterate_from_fs(&self) -> eyre::Result<crossbeam_channel::Receiver<PathBuf>> {
        //test that all start paths and excl paths actually exist.
        for incl_path in &self.incl_paths {
            if !incl_path.exists() {
                return Err(eyre::Report::msg(format!(
                    "incl_path \"{}\" does not exist",
                    incl_path.to_string_lossy(),
                )));
            }
        }

        for excl_path in &self.excl_paths {
            if !excl_path.exists() {
                return Err(eyre::Report::msg(format!(
                    "excl_path \"{}\" is does not exist",
                    excl_path.to_string_lossy(),
                )));
            }
        }

        let mut start_paths = self.incl_paths.iter();
        let mut walker = ignore::WalkBuilder::new(start_paths.next().unwrap());
        for p in start_paths {
            walker.add(p);
        }

        let (snd, rcv) = crossbeam_channel::bounded(100);
        std::thread::spawn({
            let filt = self.clone();
            move || {
                walker.build_parallel().run(|| {
                    Box::new(|res| match res {
                        Err(e) => {
                            let _report = eyre!(e).wrap_err("File enumeration failed");
                            WalkState::Skip
                        }
                        Ok(entry) => {
                            let src_path = entry.path().to_path_buf();
                            if !filt.includes(entry.path()) {
                                WalkState::Skip
                            } else if src_path.is_file() {
                                snd.send(src_path).unwrap();
                                WalkState::Continue
                            } else {
                                WalkState::Continue
                            }
                        }
                    })
                });
                drop(snd);
            }
        });

        Ok(rcv)
    }
}
