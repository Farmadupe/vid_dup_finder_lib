use std::ffi::OsString;
use std::path::PathBuf;

use vid_dup_finder_lib::Cropdetect;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReportVerbosity {
    Quiet,
    Default,
    Verbose,
}

// How are the outputs sorted?
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Sorting {
    NumMatches,
    Distance,
    Duration,
    RevNumMatches,
    RevDistance,
    RevDuration,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum OutputFormat {
    Normal,
    Json,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) enum CropdetectTypeArg {
    None,
    Letterbox,
    Motion,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) enum OutputKindRaw {
    NoOutput,
    Unique,
    Dups,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TextOutputCfg {
    NoOutput,
    Unique(OutputFormat),
    Dups {
        format: OutputFormat,
        sorting: Sorting,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GuiOutputCfg {
    NoGui,

    GuiSlint {
        sorting: Sorting,
        trash_path: Option<PathBuf>,
        max_thumbs: Option<u64>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ThumbOutputCfg {
    NoThumbs,
    Thumbs {
        thumbs_dir: PathBuf,
        sorting: Sorting,
    },
}

#[derive(Debug, Clone)]
pub struct OutputCfg {
    pub text: TextOutputCfg,
    pub thumbs: ThumbOutputCfg,
    #[allow(dead_code)]
    pub gui: GuiOutputCfg,

    pub verbosity: ReportVerbosity,
    pub cartesian_product: bool,
}

#[derive(Debug, Clone)]
pub struct DirCfg {
    pub cand_dirs: Vec<PathBuf>,
    pub ref_dirs: Vec<PathBuf>,
    pub excl_dirs: Vec<PathBuf>,
    pub excl_exts: Vec<OsString>,
}

#[derive(Debug, Clone)]
pub struct MatchDbCfg {
    pub db_path: Option<PathBuf>,

    pub fix_moved_files: bool,

    pub remove_known_matches: bool,
    pub remove_falsepos: bool,
}

#[derive(Debug, Clone)]
pub struct CacheCfg {
    pub cache_path: Option<PathBuf>,
    pub no_update_cache: bool,
}

#[derive(Debug, Clone)]
pub struct HashCfg {
    pub cropdetect: Cropdetect,
    pub skip_forward: f64,
    pub duration: f64,
}

#[derive(Debug, Clone)]
pub struct AppCfg {
    pub cache_cfg: CacheCfg,
    pub dir_cfg: DirCfg,

    pub hash_cfg: HashCfg,
    pub output_cfg: OutputCfg,

    pub display_match_db_matches: bool,
    pub display_match_db_falsepos: bool,
    pub display_match_db_validation_failures: bool,
    pub show_missed_matches: bool,

    pub update_cache_only: bool,
    pub reload_err_vids: bool,
    pub reload_all_vids: bool,

    pub matchdb_cfg: MatchDbCfg,

    pub tolerance: f64,
}
