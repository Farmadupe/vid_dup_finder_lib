use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

use clap::{value_parser, ArgAction::*};
use vid_dup_finder_lib::*;

use crate::app::*;

// file specification
const FILE_PATHS: &str = "Directories/files to search";
const REF_PATHS: &str = "Reference file paths";
const EXCL_FILE_PATHS: &str = "Exclude file paths";
const EXCL_EXTS: &str = "Exclude file extensions";

// Type of search
const DISPLAY_MATCH_DB_MATCHES: &str =
    "Do not search, and instead treat the contents of the match database as search results";
const DISPLAY_MATCH_DB_FALSEPOS : &str = "Do not search, and instead treat the false positive list in the match database as search results";
const DISPLAY_MATCH_DB_VALIDATION_FAILURES: &str = "Ignore all search configuration, and instead return all videos that are both confirmed and false positives in the DB.";

//cache update settings
const CACHE_FILE: &str = "Cache file path";
const UPDATE_CACHE_ONLY: &str = "Update cache only. Do not perform any search";
const NO_UPDATE_CACHE: &str = "Do not update the cache. Search using alreaady-cached data";
const RELOAD_ERR_VIDS: &str = "Reload error videos";
const RELOAD_ALL_VIDS: &str = "Reload all videos";

//hashing configuration
const CROPDETECT: &str = "Cropdetect algorithm";

const SKIP_FORWARD: &str = "Amount";
const DURATION: &str = "Hash Duration";

//match confirmation/filtering
const MATCH_DB_PATH: &str = "Match database path";
const MATCH_DB_FIX_MOVED_FILES: &str = "Check all matchdb entries still exist";
const MATCH_DB_REMOVE_KNOWN_MATCHES: &str = "Return matches already in the database";
const MATCH_DB_REMOVE_FALSEPOS: &str =
    "Filter out matches that the database knows are false positive";
const MATCH_DB_SHOW_MISSED_MATCHES: &str =
    "show the items from the matchdb that should have been returned but were not";

//output settings
const CARTESIAN_PRODUCT: &str = "Cartesian Product";
const SORTED: &str = "Sort";
const OUTPUT_FORMAT: &str = "Format";
const OUTPUT_THUMBS_DIR: &str = "Directory";

//gui settings
const GUI_SLINT: &str = "Run other gui";
const GUI_TRASH_PATH: &str = "Gui trash path";
const GUI_MAX_THUMBS: &str = "maximum number of thumbnails in gui";

//search configuration
const TOLERANCE: &str = "Comparison tolerance";
const OUTPUT_KIND: &str = "What to output (default is to print duplicate items)";

// Arg specification
const ARGS_FILE: &str = "Args file";

//Verbosity
const VERBOSITY_QUIET: &str = "Quiet";
const VERBOSITY_VERBOSE: &str = "Verbose";

const DISPLAY_ORDERING: [&str; 32] = [
    //
    // file specification
    FILE_PATHS,
    REF_PATHS,
    EXCL_FILE_PATHS,
    EXCL_EXTS,
    //type of search
    DISPLAY_MATCH_DB_MATCHES,
    DISPLAY_MATCH_DB_FALSEPOS,
    DISPLAY_MATCH_DB_VALIDATION_FAILURES,
    //
    //search modifiers
    TOLERANCE,
    //
    //HASHING
    CROPDETECT,
    SKIP_FORWARD,
    DURATION,
    //
    //caching
    CACHE_FILE,
    UPDATE_CACHE_ONLY,
    NO_UPDATE_CACHE,
    RELOAD_ERR_VIDS,
    RELOAD_ALL_VIDS,
    //
    //outputs
    CARTESIAN_PRODUCT,
    SORTED,
    OUTPUT_KIND,
    OUTPUT_FORMAT,
    OUTPUT_THUMBS_DIR,
    //
    //match database
    MATCH_DB_PATH,
    MATCH_DB_FIX_MOVED_FILES,
    MATCH_DB_REMOVE_KNOWN_MATCHES,
    MATCH_DB_REMOVE_FALSEPOS,
    MATCH_DB_SHOW_MISSED_MATCHES,
    //
    //verbosity
    VERBOSITY_QUIET,
    VERBOSITY_VERBOSE,
    //
    //gui
    GUI_SLINT,
    GUI_TRASH_PATH,
    GUI_MAX_THUMBS,
    //argument replacement
    ARGS_FILE,
];

fn build_app() -> clap::Command {
    let get_ordering = |arg_name: &str| -> usize {
        match DISPLAY_ORDERING.iter().position(|x| *x == arg_name) {
            Some(idx) => idx,
            None => {
                panic!("argument not assigned a display order: {arg_name:?}");
            }
        }
    };

    //clap requires all default values to be &'_ str. I want to provide compile-time &'static str for the below values,
    //but I couldn't find a way to turn f64 into &'static str at compile time. So the next best thing to do is to build
    //the strings at runtime.
    //Note: This is not a memory leak -- these strings need to last for the lifetime of the program.
    //let tol = vid_dup_finder_lib::DEFAULT_SEARCH_TOLERANCE;
    //let default_tol_string: &'static str = Box::leak(tol.to_string().into_boxed_str());
    let default_tol_string = "0.3";

    //args are not added through method chaining because rustfmt struggles with very long expressions.
    let mut clap_app = clap::Command::new("Video duplicate finder")
        .version(clap::crate_version!())
        .about("Detect duplicate video files")
        .help_template(include_str!("arg_parse_template.txt"));

    clap_app = clap_app.arg(
        clap::Arg::new(FILE_PATHS)
            .long("files")
            .required_unless_present(ARGS_FILE)
            .num_args(0..)
            .value_parser(value_parser!(PathBuf))
            .action(Append)
            .help("Paths containing new video files. These files will be checked for uniqueness against each other, or if --refs is specified, then against the files given in that argument.")
            .display_order(get_ordering(FILE_PATHS)),
    );

    clap_app = clap_app.arg(
        clap::Arg::new(REF_PATHS)
            .long("with-refs")
            .num_args(0..)
            .value_parser(value_parser!(PathBuf))
            .action(Append)
            .help("Paths containing reference video files. When present the files given by --files will be searched for duplicates against these files")
            .display_order(get_ordering(REF_PATHS)),
    );

    clap_app = clap_app.arg(
        clap::Arg::new(EXCL_FILE_PATHS)
            .long("exclude")
            .num_args(0..)
            .value_parser(value_parser!(PathBuf))
            .action(Append)
            .help("Paths to be excluded from searches")
            .display_order(get_ordering(EXCL_FILE_PATHS)),
    );

    clap_app = clap_app.arg(
        clap::Arg::new(EXCL_EXTS)
            .long("exclude-exts")
            .num_args(0..)
            .value_parser(value_parser!(OsString))
            .help("File extensions to be excluded from searches. When specified the default file exclusion extensions will be replaced with the given values. Extensions must be comma separated with no spaces, e.g '--exclude-exts ext1,ext2,ext3'")
            .value_delimiter(',')
            .action(Append)
            .default_value("png,jpg,bmp,jpeg,txt,text,db,gif,rb,py,mp3,wma,wav,ogg,db,flac,zip,rar,7z,pdf,htm,html,xls,doc,ppt,odt,ods,docx,xlsx,rtf,log,trashinfo,js,css,py,rs,aac,txt~,sh,DS_Store,kdenlive,part,webp,srt")
            .display_order(get_ordering(EXCL_EXTS)),
    );

    clap_app = clap_app.arg(
        clap::Arg::new(DISPLAY_MATCH_DB_MATCHES)
            .long("display-match-db-matches")
            .requires(MATCH_DB_PATH)
            .num_args(0)
            .conflicts_with_all([
                DISPLAY_MATCH_DB_FALSEPOS,
                DISPLAY_MATCH_DB_VALIDATION_FAILURES,
                MATCH_DB_REMOVE_FALSEPOS,
                MATCH_DB_REMOVE_KNOWN_MATCHES,
            ])
            .action(SetTrue)
            .display_order(get_ordering(DISPLAY_MATCH_DB_MATCHES)),
    );

    clap_app = clap_app.arg(
        clap::Arg::new(DISPLAY_MATCH_DB_FALSEPOS)
            .long("display-match-db-falsepos")
            .requires(MATCH_DB_PATH)
            .num_args(0)
            .conflicts_with_all([
                DISPLAY_MATCH_DB_MATCHES,
                DISPLAY_MATCH_DB_VALIDATION_FAILURES,
                MATCH_DB_REMOVE_FALSEPOS,
                MATCH_DB_REMOVE_KNOWN_MATCHES,
            ])
            .action(SetTrue)
            .display_order(get_ordering(DISPLAY_MATCH_DB_FALSEPOS)),
    );

    clap_app = clap_app.arg(
        clap::Arg::new(DISPLAY_MATCH_DB_VALIDATION_FAILURES)
            .long("display-match-db-validation-failures")
            .requires(MATCH_DB_PATH)
            .conflicts_with_all([
                DISPLAY_MATCH_DB_MATCHES,
                DISPLAY_MATCH_DB_FALSEPOS,
                MATCH_DB_REMOVE_FALSEPOS,
                MATCH_DB_REMOVE_KNOWN_MATCHES,
            ])
            .action(SetTrue)
            .num_args(0)
            .display_order(get_ordering(DISPLAY_MATCH_DB_VALIDATION_FAILURES)),
    );

    //obtain the path to the default cache file at runtime.
    //(Perhaps this shouldn't be in the arg parser??)
    let default_cache_file = || {
        directories_next::ProjectDirs::from("", "vid_dup_finder", "vid_dup_finder")
            .unwrap()
            .cache_dir()
            .join("vid_dup_finder_cache.bin")
            .to_string_lossy()
            .to_string()
    };

    clap_app = clap_app.arg(
        clap::Arg::new(CACHE_FILE)
            .long("cache-file")
            .value_parser(value_parser!(PathBuf))
            .num_args(1)
            .default_value(default_cache_file())
            .help("An optional custom location for the cache file (used to speed up repeated runs)")
            .display_order(get_ordering(CACHE_FILE)),
    );

    clap_app = clap_app.arg(
        clap::Arg::new(UPDATE_CACHE_ONLY)
            .long("update-cache-only")
            .help("Do not run a search. Update the cache and then exit.")
            .conflicts_with(NO_UPDATE_CACHE)
            .num_args(0)
            .action(SetTrue)
            .display_order(get_ordering(UPDATE_CACHE_ONLY)),
    );

    clap_app = clap_app.arg(
        clap::Arg::new(RELOAD_ERR_VIDS)
            .long("reload-errs")
            .help("Attempt to re-process videos which previously failed to loead.")
            .conflicts_with(NO_UPDATE_CACHE)
            .action(SetTrue)
            .display_order(get_ordering(RELOAD_ERR_VIDS)),
    );

    clap_app = clap_app.arg(
        clap::Arg::new(RELOAD_ALL_VIDS)
            .long("reload-all")
            .help("re-process all videos.")
            .conflicts_with(NO_UPDATE_CACHE)
            .action(SetTrue)
            .display_order(get_ordering(RELOAD_ALL_VIDS)),
    );

    #[cfg(all(target_family = "unix", feature = "gui_slint"))]
    #[allow(unused_mut)]
    let mut clap_app = clap_app.arg(
        clap::Arg::new(GUI_SLINT)
            .long("gui-slint")
            .help("Start a GUI that aids in deleting duplicate videos.")
            .num_args(0)
            .action(SetTrue)
            .display_order(get_ordering(GUI_SLINT)),
    );

    #[cfg(all(target_family = "unix", feature="gui_slint"))]
    #[allow(unused_mut)]
    let mut clap_app = clap_app.arg(
        clap::Arg::new(GUI_TRASH_PATH)
            .long("gui-trash-path")
            .hide(true)
            .value_parser(value_parser!(PathBuf))
            .num_args(1)
            .help(
                "For use in the gui: Directory that duplicate files will be moved to when using the \"keep\" operation",
            )
            .display_order(get_ordering(GUI_TRASH_PATH)),
    );

    #[cfg(all(target_family = "unix", feature = "gui_slint"))]
    let mut clap_app = clap_app.arg(
        clap::Arg::new(GUI_MAX_THUMBS)
            .long("gui-max-thumbs")
            .hide(true)
            .value_parser(value_parser!(u64))
            .num_args(1)
            .display_order(get_ordering(GUI_MAX_THUMBS)),
    );

    clap_app = clap_app.arg(
        clap::Arg::new(OUTPUT_KIND)
            .long("output")
            .help("Whether to output groups of duplicates, a list of unique videos, or nothing")
            .value_parser(value_parser!(OutputKindRaw))
            .num_args(1)
            .display_order(get_ordering(OUTPUT_KIND)),
    );

    clap_app = clap_app.arg(
        clap::Arg::new(CARTESIAN_PRODUCT)
            .long("cartesian")
            .help("whether to produce cartesian product")
            .action(SetTrue)
            .display_order(get_ordering(CARTESIAN_PRODUCT)),
    );

    clap_app = clap_app.arg(
        clap::Arg::new(SORTED)
            .long("sort")
            .help("Whether to sort results by the number of matching videos, or by how similar the videos are")
            .value_parser(value_parser!(Sorting))
            .default_value("num-matches")
            .num_args(1)
            .display_order(get_ordering(SORTED)),
    );

    clap_app = clap_app.arg(
        clap::Arg::new(OUTPUT_FORMAT)
            .long("output-format")
            .help("Whether to output as normal text , or JSON.")
            .value_parser(value_parser!(OutputFormat))
            .default_value("normal")
            .num_args(1)
            .display_order(get_ordering(OUTPUT_FORMAT)),
    );

    clap_app = clap_app.arg(
        clap::Arg::new(OUTPUT_THUMBS_DIR)
            .long("match-thumbnails-dir")
            .value_parser(value_parser!(PathBuf))
            .num_args(1)
            .help("Write thumbnails of matched images to the given directory")
            .display_order(get_ordering(OUTPUT_THUMBS_DIR)),
    );

    clap_app = clap_app.arg(
        clap::Arg::new(MATCH_DB_PATH)
            .long("matchdb")
            .value_parser(value_parser!(PathBuf))
            .num_args(1)
            .help("print statistics relative to this database of known matches")
            .display_order(get_ordering(MATCH_DB_PATH)),
    );

    clap_app = clap_app.arg(
        clap::Arg::new(MATCH_DB_FIX_MOVED_FILES)
            .long("matchdb-fix-moved-files")
            .requires(MATCH_DB_PATH)
            .action(SetTrue)
            .num_args(0)
            .display_order(get_ordering(MATCH_DB_FIX_MOVED_FILES)),
    );

    clap_app = clap_app.arg(
        clap::Arg::new(MATCH_DB_REMOVE_KNOWN_MATCHES)
            .long("matchdb-remove-known-matches")
            .requires(MATCH_DB_PATH)
            .conflicts_with_all([
                DISPLAY_MATCH_DB_MATCHES,
                DISPLAY_MATCH_DB_FALSEPOS,
                DISPLAY_MATCH_DB_VALIDATION_FAILURES,
            ])
            .action(SetTrue)
            .num_args(0)
            .display_order(get_ordering(MATCH_DB_REMOVE_KNOWN_MATCHES)),
    );

    clap_app = clap_app.arg(
        clap::Arg::new(MATCH_DB_REMOVE_FALSEPOS)
            .long("matchdb-remove-falsepos")
            .requires(MATCH_DB_PATH)
            .conflicts_with_all([
                DISPLAY_MATCH_DB_MATCHES,
                DISPLAY_MATCH_DB_FALSEPOS,
                DISPLAY_MATCH_DB_VALIDATION_FAILURES,
            ])
            .action(SetTrue)
            .num_args(0)
            .display_order(get_ordering(MATCH_DB_REMOVE_FALSEPOS)),
    );

    clap_app = clap_app.arg(
        clap::Arg::new(MATCH_DB_SHOW_MISSED_MATCHES)
            .long("matchdb-show-missed-matches")
            .requires(MATCH_DB_PATH)
            .conflicts_with_all([
                DISPLAY_MATCH_DB_MATCHES,
                DISPLAY_MATCH_DB_FALSEPOS,
                DISPLAY_MATCH_DB_VALIDATION_FAILURES,
                MATCH_DB_REMOVE_FALSEPOS,
            ])
            .action(SetTrue)
            .num_args(0)
            .display_order(get_ordering(MATCH_DB_SHOW_MISSED_MATCHES)),
    );

    clap_app = clap_app.arg( clap::Arg::new(TOLERANCE)
            .long("tolerance")

            .help("Search tolerance. A number between 0.0 and 1.0. Low values mean videos must be very similar before they will match, high numbers will permit more differences. Suggested values are in the range 0.0 to 0.2")
            .default_value(default_tol_string)
            .display_order(get_ordering(TOLERANCE))
            .num_args(1)
            .value_parser(value_parser!(f64)));

    clap_app = clap_app.arg(
        clap::Arg::new(CROPDETECT)
            .long("cropdetect")
            .help("fixme")
            .value_parser(value_parser!(CropdetectTypeArg))
            .num_args(1)
            .display_order(get_ordering(CROPDETECT)),
    );

    clap_app = clap_app.arg(
        clap::Arg::new(SKIP_FORWARD)
            .long("skip-forward")
            .num_args(1)
            .value_parser(value_parser!(f64))
            .help("Skip forward by a given number of seconds before extracting frames to build the hash. Can be used to skip into sequences")
            .display_order(get_ordering(SKIP_FORWARD)),
    );

    clap_app = clap_app.arg(
        clap::Arg::new(DURATION)
            .long("hash-duration")
            .num_args(1)
            .value_parser(value_parser!(f64))
            .help("The length in seconds of the portion of the video that will be used for creating the hash")
            .display_order(get_ordering(DURATION)),
    );

    clap_app = clap_app.arg(
        clap::Arg::new(NO_UPDATE_CACHE)
            .long("no-update-cache")
            .help("Do not update caches from filesystem. Search using only hashes already cached from previous runs.")
            .action(SetTrue)
            .num_args(0)
            .display_order(get_ordering(NO_UPDATE_CACHE)),
    );

    clap_app = clap_app.arg(
        clap::Arg::new(ARGS_FILE)
            .long("args-file")
            .value_parser(value_parser!(PathBuf))
            .num_args(1)
            .help("Read command line arguments from a file. If this argument is used it must be the only argument")
            .display_order(get_ordering(ARGS_FILE)),
    );

    clap_app = clap_app.arg(
        clap::Arg::new(VERBOSITY_QUIET)
            .long("quiet")
            .help("Reduced verbosity")
            .conflicts_with(VERBOSITY_VERBOSE)
            .action(SetTrue)
            .display_order(get_ordering(VERBOSITY_QUIET)),
    );

    clap_app = clap_app.arg(
        clap::Arg::new(VERBOSITY_VERBOSE)
            .long("verbose")
            .help("Increased verbosity")
            .conflicts_with(VERBOSITY_QUIET)
            .action(SetTrue)
            .display_order(get_ordering(VERBOSITY_VERBOSE)),
    );

    clap_app
}

pub fn parse_args() -> AppCfg {
    //capture the cwd once, to minimize the risk of working with two values if it is changed by the OS at runtime.
    let cwd = std::env::current_dir().expect("failed to extract cwd");

    //Start by parsing the provided arguments from the commandline. If the --args-file
    //argument is provided, then we will ignore the true command line arguments and
    //take the arguments from the file instead.
    let args = get_args_from_cmdline_or_file();

    let file_paths = match args.get_many::<PathBuf>(FILE_PATHS) {
        Some(paths) => paths
            .into_iter()
            .map(|p| absolutify_path(&cwd, p))
            .collect(),
        None => vec![],
    };

    let ref_file_paths = match args.get_many::<PathBuf>(REF_PATHS) {
        Some(ref_file_dirs) => ref_file_dirs.map(|p| absolutify_path(&cwd, p)).collect(),
        None => vec![],
    };

    let exclude_file_paths = match args.get_many::<PathBuf>(EXCL_FILE_PATHS) {
        Some(exclude_file_paths) => exclude_file_paths
            .map(|p| absolutify_path(&cwd, p))
            .collect(),
        None => vec![],
    };

    let excl_exts = args
        .get_many::<OsString>(EXCL_EXTS)
        .unwrap()
        .map(OsString::from)
        .collect();

    let tolerance = *args
        .get_one::<f64>(TOLERANCE)
        .unwrap_or(&DEFAULT_SEARCH_TOLERANCE);

    let cache_cfg = CacheCfg {
        cache_path: args.get_one::<PathBuf>(CACHE_FILE).map(PathBuf::from),
        no_update_cache: args.get_flag(NO_UPDATE_CACHE),
    };

    let hash_cfg = HashCfg {
        cropdetect: match args.get_one::<CropdetectTypeArg>(CROPDETECT) {
            None | Some(CropdetectTypeArg::None) => Cropdetect::None,
            Some(CropdetectTypeArg::Letterbox) => Cropdetect::Letterbox,
            Some(CropdetectTypeArg::Motion) => Cropdetect::Motion,
        },
        skip_forward: *args
            .get_one::<f64>(SKIP_FORWARD)
            .unwrap_or(&CreationOptions::default().skip_forward_amount),

        duration: *args
            .get_one::<f64>(DURATION)
            .unwrap_or(&CreationOptions::default().duration),
    };

    let dir_cfg = DirCfg {
        cand_dirs: file_paths,
        ref_dirs: ref_file_paths,
        excl_dirs: exclude_file_paths,
        excl_exts,
    };

    let verbosity = if args.get_flag(VERBOSITY_QUIET) {
        ReportVerbosity::Quiet
    } else if args.get_flag(VERBOSITY_VERBOSE) {
        ReportVerbosity::Verbose
    } else {
        ReportVerbosity::Default
    };

    let output_cfg = {
        let sorting = *args
            .get_one::<Sorting>(SORTED)
            .expect("This argument has a default value");

        let thumbs_cfg = match args.get_one::<PathBuf>(OUTPUT_THUMBS_DIR) {
            Some(dir) => ThumbOutputCfg::Thumbs {
                thumbs_dir: absolutify_path(&cwd, dir),
                sorting,
            },
            None => ThumbOutputCfg::NoThumbs,
        };

        //Gui is an optional component.
        let gui_available = cfg!(all(target_family = "unix", feature = "gui_slint"));
        let gui_cfg = if gui_available && cfg!(feature = "gui_slint") && args.get_flag(GUI_SLINT) {
            GuiOutputCfg::GuiSlint {
                sorting,
                trash_path: args.get_one::<PathBuf>(GUI_TRASH_PATH).map(PathBuf::from),
                max_thumbs: args.get_one::<u64>(GUI_MAX_THUMBS).cloned(),
            }
        } else {
            GuiOutputCfg::NoGui
        };

        let text_cfg = {
            let format = *args
                .get_one::<OutputFormat>(OUTPUT_FORMAT)
                .expect("This argument has a default value");

            match args.get_one::<OutputKindRaw>(OUTPUT_KIND) {
                Some(OutputKindRaw::NoOutput) => TextOutputCfg::NoOutput,
                Some(OutputKindRaw::Unique) => TextOutputCfg::Unique(format),
                Some(OutputKindRaw::Dups) => TextOutputCfg::Dups { format, sorting },

                //handle the default: If the user wrote no explicit argument for any type of output, then they probably
                //wanted a list of duplicate video files.
                None => {
                    let gui_not_requested = matches!(gui_cfg, GuiOutputCfg::NoGui);
                    let thumbs_not_requested = matches!(thumbs_cfg, ThumbOutputCfg::NoThumbs);

                    if gui_not_requested && thumbs_not_requested {
                        TextOutputCfg::Dups { format, sorting }
                    } else {
                        TextOutputCfg::NoOutput
                    }
                }
            }
        };

        OutputCfg {
            cartesian_product: args.get_flag(CARTESIAN_PRODUCT),
            text: text_cfg,
            thumbs: thumbs_cfg,
            gui: gui_cfg,

            verbosity,
        }
    };

    let matchdb_cfg = MatchDbCfg {
        db_path: args.get_one::<PathBuf>(MATCH_DB_PATH).map(PathBuf::from),
        fix_moved_files: args.get_flag(MATCH_DB_FIX_MOVED_FILES),
        remove_known_matches: args.get_flag(MATCH_DB_REMOVE_KNOWN_MATCHES),
        remove_falsepos: args.get_flag(MATCH_DB_REMOVE_FALSEPOS),
    };

    let ret = AppCfg {
        cache_cfg,
        dir_cfg,
        hash_cfg,
        output_cfg,

        display_match_db_matches: args.get_flag(DISPLAY_MATCH_DB_MATCHES),
        display_match_db_falsepos: args.get_flag(DISPLAY_MATCH_DB_FALSEPOS),
        display_match_db_validation_failures: args.get_flag(DISPLAY_MATCH_DB_VALIDATION_FAILURES),
        show_missed_matches: args.get_flag(MATCH_DB_SHOW_MISSED_MATCHES),

        update_cache_only: args.get_flag(UPDATE_CACHE_ONLY),
        reload_err_vids: args.get_flag(RELOAD_ERR_VIDS),
        reload_all_vids: args.get_flag(RELOAD_ALL_VIDS),

        matchdb_cfg,
        tolerance,
    };

    ret
}

// Arguments are always first read from the command line, but if --args-file
// is present, then arguments are actually located in a file on disk.
// This fn obtains the args from the correct location.
fn get_args_from_cmdline_or_file() -> clap::ArgMatches {
    let cmdline_args = build_app().get_matches();

    match cmdline_args.get_one::<PathBuf>(ARGS_FILE) {
        None => cmdline_args,
        Some(args_path) => get_argsfile_args(args_path),
    }
}

fn get_argsfile_args(argsfile_path: &Path) -> clap::ArgMatches {
    let argsfile_text = std::fs::read_to_string(argsfile_path).map_err(eyre::Report::msg);

    //now strip comments from the args file
    let args_file_contents = argsfile_text
        .and_then(|text| crate::comment_fix_issue_1::shell::strip(text).map_err(eyre::Report::msg));

    //the arguments file needs to be split into args in the same way as the shell would do it.
    //call out to an external crate for this.
    let args = args_file_contents
        .and_then(|contents| shell_words::split(&contents).map_err(eyre::Report::msg));

    let args = args
        .map_err(|e| {
            e.wrap_err(format!(
                "Failed to parse args file at location {}",
                argsfile_path.to_string_lossy()
            ))
        })
        .unwrap_or_else(|e| print_error_and_quit(e));

    //When parsing args from file, the binary name will not be present,
    // so update the parser that we use to not expect it.
    let matches = build_app().no_binary_name(true).get_matches_from(args);
    matches
}

fn absolutify_path(cwd: &Path, path: &Path) -> PathBuf {
    //get the absolute path if it is not absolute, by prepending the cwd.
    let path = if path.is_relative() {
        cwd.join(path)
    } else {
        path.to_path_buf()
    };

    //now try canonicalizing the path. If that fails then silently ignore the failure and carry on (bad idea?)
    let p = path.canonicalize().unwrap_or(path);

    p
}
