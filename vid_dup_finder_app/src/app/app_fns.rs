use crate::{app::app_fns::filename_pattern::FilenamePattern, video_hash_filesystem_cache::*};
use filename_pattern::FilterFilenames;
use itertools::Itertools;
use match_group_ext::MatchGroupExt;
#[cfg(feature = "parallel_loading")]
use rayon::prelude::*;
use serde::Serialize;
use serde_json::json;
#[cfg(feature = "print_timings")]
use std::time::Instant;
use std::{
    collections::{hash_map::RandomState, HashSet},
    error::Error,
    io::BufWriter,
    path::{Path, PathBuf},
};
use vid_dup_finder_lib::*;

use crate::app::*;

// * read cfg
// * load paths
// * update video hash cache
// * update matchdb cache
// * perform search
// * filter results
// * output results

// struct App {
//     cfg: AppCfg,
//     cands: HashSet<PathBuf>,
//     refs: HashSet<PathBuf>,
//     vid_cache: Option<VideoHashFilesystemCache>,
//     matchdb: Option<MatchDb>,
// }

pub fn run_app() -> i32 {
    let cfg = arg_parse::parse_args();
    // dbg!(&cfg);
    configure_logs(cfg.output_cfg.verbosity);

    let ret = match run_app_inner(&cfg) {
        Ok(()) => 0,
        Err(fatal_error) => {
            print_fatal_err(fatal_error, cfg.output_cfg.verbosity);
            1
        }
    };

    ret
}

// vid_dup_finder can open a lot of file handles, so make sure that
// more than the default 1024 is available

#[cfg(target_family = "unix")]
fn make_sure_lots_of_file_handles_are_available() {
    const NOFILE: rlimit::Resource = rlimit::Resource::NOFILE;
    const MIN_NOFILE: u64 = 16384;

    // for now, print no message if the any error occurs (will happen if user isn't privileged
    // to raise their own max file descriptors)
    let Ok((curr_soft, curr_hard)) = rlimit::getrlimit(NOFILE) else {
        return;
    };

    if curr_soft >= MIN_NOFILE && curr_hard >= MIN_NOFILE {
        return;
    }

    let new_soft = curr_soft.max(MIN_NOFILE);
    let new_hard = curr_hard.max(MIN_NOFILE);

    let Ok(()) = rlimit::setrlimit(NOFILE, new_soft, new_hard) else {
        return;
    };
}

fn run_app_inner(cfg: &AppCfg) -> eyre::Result<()> {
    #[cfg(target_family = "unix")]
    make_sure_lots_of_file_handles_are_available();

    //shorten some long variable names
    let cand_dirs = &cfg.dir_cfg.cand_dirs;
    let ref_dirs = &cfg.dir_cfg.ref_dirs;

    // Check that there are no shared paths in refs and cands.
    for cand_path in cand_dirs {
        for ref_path in ref_dirs {
            if cand_path == ref_path {
                return Err(eyre::Report::msg(format!(
                    "path in candidates and references: {}",
                    cand_path.to_string_lossy()
                )));
            }
        }
    }

    // Check that all cand, ref, and excl dirs exist
    let non_exist_cands = cfg.dir_cfg.cand_dirs.iter().filter(|d| !d.exists());
    match non_exist_cands.collect::<Vec<_>>().as_slice() {
        [] => (),
        missing_dirs => {
            return Err(eyre::Report::msg(format!(
                "cand_dirs not found: {}",
                missing_dirs.iter().map(|p| p.to_string_lossy()).join(", ")
            )));
        }
    }

    let non_exist_refs = cfg.dir_cfg.ref_dirs.iter().filter(|d| !d.exists());
    match non_exist_refs.collect::<Vec<_>>().as_slice() {
        [] => (),
        missing_dirs => {
            return Err(eyre::Report::msg(format!(
                "ref_dirs not found: {}",
                missing_dirs.iter().map(|p| p.to_string_lossy()).join(", ")
            )));
        }
    }

    let non_exist_excls = cfg.dir_cfg.excl_dirs.iter().filter(|d| !d.exists());
    match non_exist_excls.collect::<Vec<_>>().as_slice() {
        [] => (),
        missing_dirs => {
            return Err(eyre::Report::msg(format!(
                "excl_dirs not found: {}",
                missing_dirs.iter().map(|p| p.to_string_lossy()).join(", ")
            )));
        }
    }

    #[cfg(feature = "print_timings")]
    let cache_load_start = Instant::now();

    //load up existing hashes from disk.
    let cache_save_threshold = 2000;
    let cache = VideoHashFilesystemCache::new(
        cache_save_threshold,
        cfg.cache_cfg.cache_path.as_ref().unwrap().clone(),
        cfg.hash_cfg.cropdetect,
        cfg.hash_cfg.skip_forward,
        cfg.hash_cfg.duration,
    )?;

    // let content_cache = if let Some(matchdb_path) = &cfg.matchdb_cfg.db_path {
    //     let content_cache_path = MatchDb::content_cache_path(matchdb_path);
    //     Some(FileContentCache::new(
    //         cache_save_threshold,
    //         content_cache_path,
    //     )?)
    // } else {
    //     None
    // };

    #[cfg(feature = "print_timings")]
    #[allow(clippy::print_stdout)]
    let () = println!(
        "cache_load time: {}",
        cache_load_start.elapsed().as_secs_f64()
    );

    //first build the file projections
    // If any ref_path is a child of any cand_path, add it as an excl of cand_paths. This allows ref_paths to be located
    // in subdirs of cand_paths.
    // let cand_excls = excl_dirs.iter().chain(ref_dirs);
    // let ref_excls = excl_dirs.iter().chain(cand_dirs);

    // Update the cache file with all videos specified by --files and --with-refs
    if !cfg.cache_cfg.no_update_cache {
        update_hash_cache(cfg, &cache)?;
    }

    //if the match db is requested then create it.
    let match_db_requested = cfg.matchdb_cfg.db_path.is_some();
    let match_db = match_db_requested.then(|| {
        #[cfg(feature = "print_timings")]
        let match_db_load_start = Instant::now();

        let db_path = cfg.matchdb_cfg.db_path.as_ref().unwrap();

        //check if there is an existing DB and load it.
        //Otherwise create a new DB.
        let mut db = if MatchDb::exists_on_disk(db_path) {
            let db = match MatchDb::from_disk(db_path) {
                Ok(db) => db,
                Err(e) => {
                    error!("{e}");
                    std::process::exit(1);
                }
            };

            db
        } else {
            MatchDb::new(db_path)
        };

        #[cfg(feature = "print_timings")]
        #[allow(clippy::print_stdout)]
        let () = println!(
            "match_db_load time: {}",
            match_db_load_start.elapsed().as_secs_f64()
        );

        let filename_filter = create_filename_filter(cfg);
        let paths_to_update_matchdb = cache
            .all_cached_paths()
            .into_iter()
            .filter(|p| filename_filter.includes(p))
            .collect::<Vec<_>>();

        db.update_file_content_cache(paths_to_update_matchdb.iter().cloned())
            .unwrap();

        //if requested, load the raw db entries into the match database

        if let Err(e) = db.load_new_inputs() {
            error!("{e}");
            std::process::exit(1);
        }

        if cfg.matchdb_cfg.fix_moved_files {
            if let Err(e) = db.fix_moved_files() {
                error!("{e}");
                std::process::exit(1);
            }
        }

        //save the updated matchdb
        db.to_disk();

        db
    });

    //if the app was only invoked to update the cache, then we're done at this point.
    if cfg.update_cache_only {
        return Ok(());
    }

    // Perform the search
    let non_search_output_requested = cfg.display_match_db_matches
        || cfg.display_match_db_falsepos
        || cfg.display_match_db_validation_failures;

    let search_output = if non_search_output_requested {
        display_match_db_output(cfg, match_db.as_ref().unwrap())
    } else {
        search_disk(cfg, &cache, match_db.as_ref())
    };

    do_app_outputs(cfg, search_output, cache)?;

    Ok(())
}

#[allow(clippy::print_stdout)]
fn do_app_outputs(
    cfg: &AppCfg,
    mut search_output: SearchOutput,
    cache: VideoHashFilesystemCache,
) -> Result<(), AppError> {
    use super::app_cfg::{OutputFormat::*, TextOutputCfg::*, ThumbOutputCfg::*};

    ////////////////////////////////////////////////////////////////////////////
    // Text Output
    ////////////////////////////////////////////////////////////////////////////
    match cfg.output_cfg.text {
        NoOutput => (),
        Unique(format) => {
            let dup_paths = search_output
                .dup_paths()
                .map(PathBuf::from)
                .collect::<HashSet<PathBuf, RandomState>>();

            let cands_filter = create_cands_filename_filter(cfg);
            let all_hash_paths = cache.all_cached_paths();
            let cands = all_hash_paths
                .into_iter()
                .filter(|p| cands_filter.includes(p))
                .collect::<HashSet<_>>();

            let unique_paths = cands.difference(&dup_paths);

            match format {
                Normal => {
                    for unique_file in unique_paths {
                        println!("{}", unique_file.display());
                    }
                }
                Json => {
                    let stdout = BufWriter::new(std::io::stdout());

                    serde_json::to_writer_pretty(stdout, &json!(unique_paths.collect::<Vec<_>>()))
                        .unwrap_or_default();
                    println!();
                }
            }
        }

        //////////////////////////////
        // Unstructure text output.
        Dups {
            format: Normal,
            sorting,
        } => {
            search_output.sort(sorting, &cache);
            for group in search_output.dup_groups() {
                if let Some(video) = group.reference() {
                    println!("{}", video.display());
                }
                for video in group.duplicates() {
                    println!("{}", video.display());
                }
                println!();
            }
        }

        ///////////////
        // Json output
        Dups {
            format: Json,
            sorting,
        } => {
            search_output.sort(sorting, &cache);

            //Sturct only exists to be serialized.
            #[derive(Serialize)]
            struct JsonStruct<'a> {
                reference: Option<&'a Path>,
                duplicates: Vec<&'a Path>,
            }

            let output_vec: Vec<JsonStruct> = search_output
                .dup_groups()
                .map(|group| JsonStruct {
                    reference: group.reference(),
                    duplicates: group.duplicates().collect(),
                })
                .collect();

            let stdout = BufWriter::new(std::io::stdout());
            serde_json::to_writer_pretty(stdout, &json!(output_vec)).unwrap_or_default();
            println!();
        }
    }

    ////////////////////////////////////////////////////////////////////////////
    // Thumbnail file output
    ////////////////////////////////////////////////////////////////////////////
    match &cfg.output_cfg.thumbs {
        NoThumbs => (),
        Thumbs {
            thumbs_dir,
            sorting,
        } => {
            if matches!(cfg.output_cfg.text, Unique(_)) {
                let dup_paths = search_output
                    .dup_paths()
                    .map(PathBuf::from)
                    .collect::<HashSet<PathBuf, RandomState>>();

                let cands_filter = create_cands_filename_filter(cfg);
                let all_hash_paths = cache.all_cached_paths();
                let cands = all_hash_paths
                    .into_iter()
                    .filter(|p| cands_filter.includes(p))
                    .collect::<HashSet<_>>();

                let unique_paths = cands.difference(&dup_paths);

                let new_groups = unique_paths
                    .filter_map(|p| MatchGroup::new([p.to_path_buf(), p.to_path_buf()]).ok());

                let mut new_search_output = SearchOutput::new(new_groups.collect());
                new_search_output.sort(*sorting, &cache);
                new_search_output.save_debug_imgs(thumbs_dir);
            } else {
                search_output.sort(*sorting, &cache);
                search_output.save_debug_imgs(thumbs_dir);
            }
        }
    }

    ////////////////////////////////////////////////////////////////////////////
    // Gui output
    ////////////////////////////////////////////////////////////////////////////
    #[cfg(all(target_family = "unix", feature = "gui_slint"))]
    match &cfg.output_cfg.gui {
        super::app_cfg::GuiOutputCfg::NoGui => (),
        super::app_cfg::GuiOutputCfg::GuiSlint {
            sorting,
            trash_path,
            max_thumbs: _max_thumbs,
        } => {
            if matches!(cfg.output_cfg.text, Unique(_)) {
                let dup_paths = search_output
                    .dup_paths()
                    .map(PathBuf::from)
                    .collect::<HashSet<PathBuf, RandomState>>();

                let cands_filter = create_cands_filename_filter(cfg);
                let all_hash_paths = cache.all_cached_paths();
                let cands = all_hash_paths
                    .into_iter()
                    .filter(|p| cands_filter.includes(p))
                    .collect::<HashSet<_>>();

                let unique_paths = cands.difference(&dup_paths);

                let new_groups = unique_paths
                    .filter_map(|p| MatchGroup::new([p.to_path_buf(), p.to_path_buf()]).ok());

                search_output = SearchOutput::new(new_groups.collect());
            }

            search_output.sort(*sorting, &cache);
            let thunks = search_output.resolution_thunks(&cache, trash_path.as_deref());

            #[cfg(feature = "gui_slint")]
            run_gui_slint(thunks).unwrap();
        }
    }
    Ok(())
}

fn search_disk(
    cfg: &AppCfg,
    cache: &VideoHashFilesystemCache,

    match_db: Option<&MatchDb>,
) -> SearchOutput {
    #[cfg(feature = "print_timings")]
    let hash_fetch_start = Instant::now();

    // Now that we have updated the caches, we can fetch hashes from the cache in preparation for a search.
    // the unwraps here are infallible, as the keys we are fetching are sourced from the cache itself.
    let all_hash_paths = cache.all_cached_paths();

    let cands_filter = create_cands_filename_filter(cfg);
    let cand_hashes = all_hash_paths
        .iter()
        .filter(|&p| cands_filter.includes(p))
        .map(|p| cache.fetch(p).unwrap())
        .collect::<Vec<_>>();

    let refs_filter = create_refs_filename_filter(cfg);
    let ref_hashes = all_hash_paths
        .iter()
        .filter(|&p| refs_filter.includes(p))
        .map(|p| cache.fetch(p).unwrap())
        .collect::<Vec<_>>();

    #[cfg(feature = "print_timings")]
    #[allow(clippy::print_stdout)]
    let () = println!(
        "hash_fetch time: {}",
        hash_fetch_start.elapsed().as_secs_f64()
    );

    #[cfg(feature = "print_timings")]
    let search_start = Instant::now();

    //sanity check: Warn the user if no files were selected for the search
    if cand_hashes.is_empty() {
        warn!("No files were found at the paths given by --files. No results will be returned.");
    }

    //sanity check: Warn the user if no refs were selected (but only if the user asked for refs)
    if !cfg.dir_cfg.ref_dirs.is_empty() && ref_hashes.is_empty() {
        warn!(
            "No reference files were found at the paths given by --with-refs. No results will be returned."
        );
    }

    //If there are just cands, then perform a find-all search. Otherwise perform a with-refs search.
    let mut matchset = if ref_hashes.is_empty() {
        search(cand_hashes, cfg.tolerance)
    } else {
        search_with_references(ref_hashes, cand_hashes, cfg.tolerance)
    };

    //unfortunately currently need to convert each matchgroup into
    //its cartesian product to apply filters.
    if cfg.output_cfg.cartesian_product {
        matchset = matchset
            .iter()
            .flat_map(|group| group.dup_combinations())
            .collect();
    }

    #[cfg(feature = "print_timings")]
    #[allow(clippy::print_stdout)]
    let () = println!("search time: {}", search_start.elapsed().as_secs_f64());

    #[cfg(feature = "print_timings")]
    let match_db_filter_start = Instant::now();

    //now apply matchdb filtering operations as requested.

    let filtering_required = match_db.is_some()
        && (cfg.matchdb_cfg.remove_falsepos || cfg.matchdb_cfg.remove_known_matches);
    let mut search_output = if !filtering_required {
        SearchOutput::new(matchset)
    } else {
        let match_db = match_db.as_ref().unwrap();

        let num_groups_pre_filter = matchset.len();

        let mut num_falsepos_removed = 0;

        let all_files_filter = create_filename_filter(cfg);
        let num_db_matches = match_db
            .confirmed_groups()
            .filter_map(|group| group.filter(&all_files_filter))
            .flat_map(|group| group.dup_combinations())
            .count();

        let group_would_be_falsepos = |group: &[PathBuf], cand_path: &Path| {
            group
                .iter()
                .any(|group_path| match_db.is_falsepos(group_path, cand_path))
        };

        // let first_group_that_is_confirmed = |groups: &[MatchGroup], cand_path: &Path| {
        //     groups
        //         .iter()
        //         .position(|group| match_db.all_confirmed(group.contained_paths(), cand_path))
        // };

        let first_group_that_is_unconfirmed = |groups: &Vec<Vec<PathBuf>>, cand_path: &Path| {
            groups
                .iter()
                .position(|group| !match_db.all_confirmed(group, cand_path))
        };

        #[cfg(feature = "print_timings")]
        let remove_known_start = Instant::now();

        if cfg.matchdb_cfg.remove_known_matches {
            #[cfg(feature = "parallel_loading")]
            let it = matchset.into_par_iter();

            #[cfg(not(feature = "parallel_loading"))]
            let it = matchset.into_iter();

            // #[cfg(not(feature = "parallel_loading"))]
            // let matchset = matchset.into_iter();
            matchset = it
                .flat_map(|group| {
                    let mut ret: Vec<Vec<PathBuf>> = vec![];
                    for src_path in group.contained_paths() {
                        if let Some(idx) = first_group_that_is_unconfirmed(&ret, src_path) {
                            ret[idx].push(src_path.to_path_buf())
                        } else {
                            ret.push(vec![src_path.to_path_buf()])
                        }
                    }
                    ret.into_iter()
                        .filter_map(|paths| MatchGroup::new(paths).ok())
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        }
        #[cfg(feature = "print_timings")]
        #[allow(clippy::print_stdout)]
        let () = println!(
            "matchdb_remove_known time: {}",
            remove_known_start.elapsed().as_secs_f64()
        );

        if cfg.matchdb_cfg.remove_falsepos {
            matchset = matchset
                .into_iter()
                .flat_map(|group| {
                    let mut ret: Vec<PathBuf> = vec![];

                    for src_path in group.contained_paths() {
                        if ret.is_empty() {
                            ret.push(src_path.to_path_buf());
                        } else if !group_would_be_falsepos(&ret, src_path) {
                            ret.push(src_path.to_path_buf())
                        } else {
                            num_falsepos_removed += 1;
                        }
                    }
                    match MatchGroup::new(ret) {
                        Ok(group) => vec![group],
                        Err(_) => vec![],
                    }
                })
                .collect::<Vec<_>>()
        }

        let search_output = SearchOutput::new(matchset);

        #[cfg(feature = "print_timings")]
        let match_db_coalesce_start = Instant::now();

        // for err in coalesce_errs {
        //     warn!("coalesce had a problem: {err}");
        // }

        #[cfg(feature = "print_timings")]
        #[allow(clippy::print_stdout)]
        let () = println!(
            "match_db_coalesce time: {}",
            match_db_coalesce_start.elapsed().as_secs_f64()
        );

        #[allow(clippy::print_stdout)]
        {
            println!(
                "There were {num_groups_pre_filter} groups pre filtering and {} groups after.",
                search_output.len(),
            );

            println!(
                "Search failed to find {} groups in the match_db",
                num_db_matches as isize - num_groups_pre_filter as isize
            );

            // if cfg.matchdb_cfg.remove_known_matches {
            //     println!("Removed {num_confirmed_removed} already-known matches.");
            // }

            if cfg.matchdb_cfg.remove_falsepos {
                println!("Removed {num_falsepos_removed} false positive matches.");
            }

            // if cfg.matchdb_cfg.remove_unknown_matches {
            //     println!("Removed {num_unconfirmed_removed} unconfirmed matches.");
            // }
        }

        search_output
    };

    #[cfg(feature = "print_timings")]
    #[allow(clippy::print_stdout)]
    let () = println!(
        "match_db_filter time: {}",
        match_db_filter_start.elapsed().as_secs_f64()
    );

    if cfg.show_missed_matches {
        search_output = show_missed_matches(match_db.as_ref().unwrap(), search_output);
    }

    search_output
}

/// Find the items in the given search output that should have been returned, but were not.
fn show_missed_matches(match_db: &MatchDb, curr_output: SearchOutput) -> SearchOutput {
    let all_found_in_search = curr_output
        .dup_groups()
        .flat_map(|group| group.dup_combinations())
        .map(|combo_group| {
            combo_group
                .contained_paths()
                .map(|x| x.to_path_buf())
                .sorted()
                .collect::<Vec<_>>()
        })
        .collect::<HashSet<_>>();

    let all_confirmed = match_db
        .confirmed_groups()
        .flat_map(|group| group.dup_combinations())
        .map(|combo_group| {
            combo_group
                .contained_paths()
                .map(|x| x.to_path_buf())
                .sorted()
                .collect::<Vec<_>>()
        })
        .collect::<HashSet<_>>();

    //.map(|group| group.map(|entry| entry.path.clone()).collect::<Vec<_>>());

    let all_confirmed_len = match_db.confirmed_groups().count();

    let filtered = all_confirmed
        .difference(&all_found_in_search)
        .map(|x| x.iter().map(|x| x.to_path_buf()))
        .filter_map(|x| MatchGroup::new(x).ok())
        .collect::<Vec<_>>();

    let remaining_len = filtered.len();

    #[allow(clippy::print_stdout)]
    let () = println!(
        "all confirmed groups: {all_confirmed_len}, failed to match groups: {remaining_len}"
    );

    SearchOutput::new(filtered)
}

fn display_match_db_output(cfg: &AppCfg, match_db: &MatchDb) -> SearchOutput {
    fn display_match_db_matches(cfg: &AppCfg, match_db: &MatchDb) -> Vec<MatchGroup> {
        //If the search is being done without references, then filter all items in the match db
        //that are in teh cand projection, and return all where there are at least two entries

        if cfg.dir_cfg.ref_dirs.is_empty() {
            let cands_filter = create_cands_filename_filter(cfg);
            let groups = match_db.confirmed_groups();
            groups
                .filter_map(|group| group.filter(&cands_filter))
                .collect::<Vec<_>>()

        //References are more complex because if a group contains multiple references then more
        //than one group must be returned (one group for each reference)
        } else {
            let all_filter = create_filename_filter(cfg);
            let refs_filter = create_refs_filename_filter(cfg);
            match_db
                .confirmed_groups()
                .filter_map(|g| g.filter(&all_filter))
                .flat_map(|g| g.extract_reference(&refs_filter).collect::<Vec<_>>())
                .collect::<Vec<MatchGroup>>()
        }
    }

    fn display_match_db_falsepos(cfg: &AppCfg, match_db: &MatchDb) -> Vec<MatchGroup> {
        let all_filter = create_filename_filter(cfg);

        let falsepos_groups = match_db
            .falsepos_groups()
            .filter_map(|group| group.filter(&all_filter));

        if cfg.dir_cfg.ref_dirs.is_empty() {
            falsepos_groups.collect::<Vec<_>>()
        } else {
            let refs_filter = create_refs_filename_filter(cfg);
            falsepos_groups
                .flat_map(|group| group.extract_reference(&refs_filter).collect::<Vec<_>>())
                .flat_map(|group| group.dup_combinations())
                .collect::<Vec<_>>()
        }
    }

    fn display_match_db_validation_failures(match_db: &MatchDb) -> Vec<MatchGroup> {
        match_db
            .confirmed_and_falsepos_entries()
            .filter_map(|(e1, e2)| MatchGroup::new([e1.to_path_buf(), e2.to_path_buf()]).ok())
            .collect::<Vec<_>>()
    }

    let matchset = if cfg.display_match_db_matches {
        display_match_db_matches(cfg, match_db)
    } else if cfg.display_match_db_falsepos {
        display_match_db_falsepos(cfg, match_db)
    } else if cfg.display_match_db_validation_failures {
        display_match_db_validation_failures(match_db)
    } else {
        unreachable!()
    };

    SearchOutput::new(matchset)
}

fn create_filename_filter(cfg: &AppCfg) -> FilenamePattern {
    let incl_paths = cfg
        .dir_cfg
        .cand_dirs
        .iter()
        .chain(cfg.dir_cfg.ref_dirs.iter())
        .cloned()
        .collect::<Vec<_>>();
    let excl_paths = cfg.dir_cfg.excl_dirs.clone();
    let excl_exts = cfg.dir_cfg.excl_exts.clone();

    FilenamePattern::new(incl_paths, excl_paths, excl_exts)
        .unwrap_or_else(|e| print_error_and_quit(e))
}

fn create_cands_filename_filter(cfg: &AppCfg) -> FilenamePattern {
    let incl_paths = cfg.dir_cfg.cand_dirs.clone();
    let excl_paths = cfg
        .dir_cfg
        .excl_dirs
        .iter()
        .chain(cfg.dir_cfg.ref_dirs.iter())
        .cloned()
        .collect::<Vec<_>>();
    let excl_exts = cfg.dir_cfg.excl_exts.clone();
    FilenamePattern::new(incl_paths, excl_paths, excl_exts)
        .unwrap_or_else(|e| print_error_and_quit(e))
}

fn create_refs_filename_filter(cfg: &AppCfg) -> FilenamePattern {
    let incl_paths = cfg.dir_cfg.ref_dirs.clone();
    let excl_paths = cfg
        .dir_cfg
        .excl_dirs
        .iter()
        .chain(cfg.dir_cfg.cand_dirs.iter())
        .cloned()
        .collect::<Vec<_>>();

    let excl_exts = cfg.dir_cfg.excl_exts.clone();

    FilenamePattern::new(incl_paths, excl_paths, excl_exts)
        .unwrap_or_else(|e| print_error_and_quit(e))
}

fn update_hash_cache(cfg: &AppCfg, cache: &VideoHashFilesystemCache) -> eyre::Result<()> {
    #[cfg(feature = "print_timings")]
    let cache_update_start = Instant::now();

    let file_filter = create_filename_filter(cfg);

    if cfg.reload_all_vids {
        cache.clear();
    } else if cfg.reload_err_vids {
        for path in cache.error_paths() {
            if file_filter.includes(&path) {
                if let Err(_e) = cache.remove(path) {
                    //loading_errs.push(e);
                }
            }
        }
    }

    // let all_files = file_filter
    //     .iterate_from_fs()?
    //     .into_iter()
    //     .collect::<HashSet<_>>();
    // cache.remove_deleted_items(all_files.iter().cloned());
    // cache.update_using_fs(all_files.iter().cloned());

    let it = file_filter.iterate_from_fs()?.into_iter();
    let t = iter_tee::Tee::new(it);

    cache.update_using_fs(t.clone());
    for src_path in cache.all_cached_paths() {
        if file_filter.includes(&src_path) {
            if let Ok(false) = src_path.try_exists() {
                cache.remove(src_path).unwrap();
            }
        }
    }
    cache.save().unwrap();

    #[cfg(feature = "print_timings")]
    #[allow(clippy::print_stdout)]
    let () = println!(
        "cache_update time: {}",
        cache_update_start.elapsed().as_secs_f64()
    );

    Ok(())
}

fn print_fatal_err(fatal_err: eyre::Report, verbosity: ReportVerbosity) {
    error!(target: "app-errorlog", "{}", fatal_err);

    if verbosity == ReportVerbosity::Verbose {
        let mut source: Option<&(dyn Error + 'static)> = fatal_err.source();
        while let Some(e) = source {
            error!(target: "app-errorlog", "    caused by: {}", e);
            source = e.source();
        }
    }
}

pub fn configure_logs(verbosity: ReportVerbosity) {
    use simplelog::*;

    //let cfg = Default::default();
    let mut cfg = simplelog::ConfigBuilder::new();
    cfg.add_filter_ignore("generic_cache_insert".to_string());

    let min_loglevel = match verbosity {
        ReportVerbosity::Quiet => LevelFilter::Warn,
        ReportVerbosity::Default => LevelFilter::Info,
        ReportVerbosity::Verbose => LevelFilter::Trace,
    };

    TermLogger::init(
        min_loglevel,
        cfg.build(),
        TerminalMode::Stderr,
        ColorChoice::Auto,
    )
    .expect("TermLogger failed to initialize");
}
