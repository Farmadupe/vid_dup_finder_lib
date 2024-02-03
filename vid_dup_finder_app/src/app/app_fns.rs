use super::app_cfg::AppCfg;

#[cfg(feature = "print_timings")]
use std::time::Instant;

use itertools::{Either, Itertools};
use serde::Serialize;
use serde_json::json;
use std::{
    collections::{hash_map::RandomState, HashSet},
    error::Error,
    io::BufWriter,
    path::{Path, PathBuf},
};
use vid_dup_finder_lib::*;
use video_hash_filesystem_cache::*;

use crate::app::*;

pub fn run_app() -> i32 {
    //Parse arguments and bail early if there is an error.
    let cfg = match arg_parse::parse_args() {
        Ok(cfg) => {
            configure_logs(cfg.output_cfg.verbosity);
            cfg
        }
        Err(fatal) => {
            //Errors are reported using TermLogger, which is configured from the argument parser.
            //But if a fatal error occurred during parsing the logger would not be configured when
            //we attempt to print the fatal error. So if a fatal error occurs, start the logger
            //before returning the error.
            configure_logs(ReportVerbosity::Verbose);
            print_fatal_err(&fatal, ReportVerbosity::Verbose);
            return 1;
        }
    };

    match run_app_inner(&cfg) {
        Ok(nonfatal_errs) => {
            print_nonfatal_errs(&nonfatal_errs);
            0
        }
        Err(fatal_error) => {
            print_fatal_err(&fatal_error, cfg.output_cfg.verbosity);
            1
        }
    }
}

fn run_app_inner(cfg: &AppCfg) -> Result<Vec<AppError>, AppError> {
    let mut nonfatal_errs: Vec<AppError> = vec![];

    //if the match db is requested then create it.
    let match_db_requested = cfg.matchdb_cfg.db_path.is_some();
    let match_db = match_db_requested.then(|| read_match_db(&cfg.matchdb_cfg));

    //shorten some long variable names
    let cand_dirs = &cfg.dir_cfg.cand_dirs;
    let ref_dirs = &cfg.dir_cfg.ref_dirs;
    let excl_dirs = &cfg.dir_cfg.excl_dirs;
    let excl_exts = &cfg.dir_cfg.excl_exts;

    // Check that there are no shared paths in refs and cands.
    for cand_path in cand_dirs {
        for ref_path in ref_dirs {
            if cand_path == ref_path {
                return Err(AppError::PathInFilesAndRefs(cand_path.clone()));
            }
        }
    }

    #[cfg(feature = "print_timings")]
    let cache_load_start = Instant::now();

    //load up existing hashes from disk.
    let cache_save_threshold = 100;
    let cache = VideoHashFilesystemCache::new(
        cache_save_threshold,
        cfg.cache_cfg.cache_path.as_ref().unwrap().clone(),
        cfg.hash_cfg.cropdetect,
        cfg.hash_cfg.skip_forward,
    )?;

    #[cfg(feature = "print_timings")]
    println!(
        "cache_load time: {}",
        cache_load_start.elapsed().as_secs_f64()
    );

    //first build the file projections
    // If any ref_path is a child of any cand_path, add it as an excl of cand_paths. This allows ref_paths to be located
    // in subdirs of cand_paths.
    let cand_excls = excl_dirs.iter().chain(ref_dirs);
    let mut cands = match FileProjection::new(cand_dirs, cand_excls, excl_exts) {
        Ok(x) => x,
        Err(FileProjectionError::SrcPathExcluded {
            src_path,
            excl_path,
        }) => {
            return Err(AppError::SrcPathExcludedError {
                src_path,
                excl_path,
            })
        }
        Err(_) => unreachable!(),
    };

    let ref_excls = excl_dirs.iter().chain(cand_dirs);
    let mut refs = match FileProjection::new(ref_dirs, ref_excls, excl_exts) {
        Ok(x) => x,
        Err(FileProjectionError::SrcPathExcluded {
            src_path,
            excl_path,
        }) => {
            return Err(AppError::RefPathExcludedError {
                src_path,
                excl_path,
            })
        }
        Err(_) => unreachable!(),
    };

    // Update the cache file with all videos specified by --files and --with-refs
    if !cfg.cache_cfg.no_update_cache {
        update_hash_cache(
            &mut cands,
            &mut refs,
            &mut nonfatal_errs,
            cfg.reload_err_vids,
            &cache,
        )?;
    }

    //if the app was only invoked to update the cache, then we're done at this point.
    if cfg.update_cache_only {
        return Ok(nonfatal_errs);
    }

    // Perform the search
    let non_search_output_requested = cfg.display_match_db_matches
        || cfg.display_match_db_falsepos
        || cfg.display_match_db_validation_failures;

    let search_output = if non_search_output_requested {
        display_match_db_output(cfg, match_db.as_ref().unwrap(), &cands, &refs)
    } else {
        search_disk(cfg, &cache, &mut cands, &mut refs, &match_db)
    };

    do_app_outputs(cfg, search_output, cands, cache)?;

    Ok(nonfatal_errs)
}

#[allow(clippy::print_stdout)]
fn do_app_outputs(
    cfg: &AppCfg,
    mut search_output: SearchOutput,
    cands: FileProjection,
    _cache: VideoHashFilesystemCache,
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

            let unique_paths = cands.projected_files2().difference(&dup_paths);

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
            search_output.sort(sorting);
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
            search_output.sort(sorting);

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
            search_output.sort(*sorting);
            search_output.save_debug_imgs(thumbs_dir);
        }
    }

    ////////////////////////////////////////////////////////////////////////////
    // Gui output
    ////////////////////////////////////////////////////////////////////////////
    match &cfg.output_cfg.gui {
        super::app_cfg::GuiOutputCfg::NoGui => (),
        super::app_cfg::GuiOutputCfg::Gui {
            sorting,
            trash_path,
        } => {
            #[cfg(all(target_family = "unix", feature = "gui"))]
            {
                search_output.sort(*sorting);
                let thunks = search_output.resolution_thunks(&_cache, trash_path.as_deref());
                run_gui(thunks)?;
            }
            #[cfg(not(all(target_family = "unix", feature = "gui")))]
            {
                let _ = sorting;
                let _ = trash_path;
                panic!("GUI not available");
            }
        }
    }
    Ok(())
}

fn read_match_db(cfg: &MatchDbCfg) -> MatchDb {
    #[cfg(feature = "print_timings")]
    let match_db_load_start = Instant::now();

    let db_path = cfg.db_path.as_ref().unwrap();

    //check if there is an existing DB and load it.
    //Otherwise create a new DB.
    let mut match_db = if MatchDb::exists_on_disk(db_path) {
        let mut db = MatchDb::from_disk(db_path);
        db.remove_deleted_items();

        db
    } else {
        MatchDb::new(db_path)
    };

    //if requested, load the raw db entries into the match database
    match_db.load_raw(&cfg.raw_data_paths).unwrap();

    if cfg.validate_entries_exist {
        match_db.remove_deleted_items();
    }

    //save the updated matchdb
    match_db.to_disk();

    #[cfg(feature = "print_timings")]
    println!(
        "match_db_load time: {}",
        match_db_load_start.elapsed().as_secs_f64()
    );

    match_db
}

fn search_disk(
    cfg: &AppCfg,
    cache: &VideoHashFilesystemCache,
    cand_projection: &mut FileProjection,
    ref_projection: &mut FileProjection,
    match_db: &Option<MatchDb>,
) -> SearchOutput {
    #[cfg(feature = "print_timings")]
    let hash_fetch_start = Instant::now();

    // Now that we have updated the caches, we can fetch hashes from the cache in preparation for a search.
    // the unwraps here are infallible, as the keys we are fetching are sourced from the cache itself.
    let all_hash_paths = cache
        .all_cached_paths()
        .iter()
        .cloned()
        .collect::<HashSet<PathBuf, RandomState>>();
    cand_projection.project_using_list(&all_hash_paths);
    ref_projection.project_using_list(&all_hash_paths);

    let cand_hashes = cand_projection
        .projected_files()
        .map(|cand_path| cache.fetch(cand_path).unwrap())
        .collect::<Vec<_>>();
    let ref_hashes = ref_projection
        .projected_files()
        .map(|ref_path| cache.fetch(ref_path).unwrap())
        .collect::<Vec<_>>();

    #[cfg(feature = "print_timings")]
    println!(
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
        warn!("No reference files were found at the paths given by --with-refs. No results will be returned.");
    }

    //If there are just cands, then perform a find-all search. Otherwise perform a with-refs search.
    let mut matchset = if ref_hashes.is_empty() {
        search(cand_hashes, cfg.tolerance)
    } else {
        search_with_references(ref_hashes, cand_hashes, cfg.tolerance)
    };

    #[cfg(feature = "print_timings")]
    println!("search time: {}", search_start.elapsed().as_secs_f64());

    #[cfg(feature = "print_timings")]
    let match_db_filter_start = Instant::now();

    //now apply matchdb filtering operations as requested.

    let filtering_required = match_db.is_some()
        && (cfg.matchdb_cfg.remove_falsepos
            || cfg.matchdb_cfg.remove_known_matches
            || cfg.matchdb_cfg.remove_unknown_matches);
    let search_output = if !filtering_required {
        SearchOutput::new(matchset)
    } else {
        let match_db = match_db.as_ref().unwrap();

        let num_groups_pre_filter = matchset.len();

        //unfortunately currently need to convert each matchgroup into
        //its cartesian product to apply filters.
        matchset = matchset
            .iter()
            .flat_map(|group| group.dup_combinations())
            .collect();

        let mut num_confirmed_removed = 0;
        let mut num_unconfirmed_removed = 0;
        let mut num_falsepos_removed = 0;

        let num_db_matches = match_db
            .all_confirmed()
            .filter_map(|db_paths| {
                let db_match_items_found = db_paths
                    .filter(|&path| {
                        cand_projection.projected_files2().contains(path)
                            || ref_projection.projected_files2().contains(path)
                    })
                    .count();

                if db_match_items_found > 1 {
                    Some(())
                } else {
                    None
                }
            })
            .count();

        //for each group cycle through all the matches within
        //and perform filtering.
        let filtered_out_matchset = matchset.into_iter().filter(|group| {
            //Since the cartesian product has been taken we know that
            //there will be exactly two items inside each group
            let (p1, p2) = {
                let mut matchgroup_paths = group.contained_paths();
                (
                    matchgroup_paths.next().unwrap(),
                    matchgroup_paths.next().unwrap(),
                )
            };

            let mut keep = true;
            let c = &cfg.matchdb_cfg;

            if c.remove_known_matches || c.remove_unknown_matches {
                let is_confirmed = match_db.is_confirmed(p1, p2);
                if c.remove_known_matches && is_confirmed {
                    keep = false;
                    num_confirmed_removed += 1;
                } else if cfg.matchdb_cfg.remove_unknown_matches && !is_confirmed {
                    keep = false;
                    num_unconfirmed_removed += 1;
                }
            }

            if c.remove_falsepos && match_db.is_falsepos(p1, p2) {
                keep = false;
                num_falsepos_removed += 1;
            }

            keep
        });

        matchset = filtered_out_matchset.collect();

        #[cfg(feature = "print_timings")]
        let match_db_coalesce_start = Instant::now();

        let uncoalesced_output = SearchOutput::new(matchset);
        let (search_output, coalesce_errs) = uncoalesced_output.coalesce(cache);

        for err in coalesce_errs {
            warn!("coalesce had a problem: {err}");
        }

        #[cfg(feature = "print_timings")]
        println!(
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

            if cfg.matchdb_cfg.remove_known_matches {
                println!("Removed {num_confirmed_removed} already-known matches.");
            }

            if cfg.matchdb_cfg.remove_falsepos {
                println!("Removed {num_falsepos_removed} false positive matches.");
            }

            if cfg.matchdb_cfg.remove_unknown_matches {
                println!("Removed {num_unconfirmed_removed} unconfirmed matches.");
            }
        }

        search_output
    };

    #[cfg(feature = "print_timings")]
    println!(
        "match_db_filter time: {}",
        match_db_filter_start.elapsed().as_secs_f64()
    );

    search_output
}

fn display_match_db_output(
    cfg: &AppCfg,
    match_db: &MatchDb,
    cand_projection: &FileProjection,
    ref_projection: &FileProjection,
) -> SearchOutput {
    fn display_match_db_matches(
        match_db: &MatchDb,
        cand_projection: &FileProjection,
        ref_projection: &FileProjection,
    ) -> Vec<MatchGroup> {
        //If the search is being done without references, then filter all items in the match db
        //that are in teh cand projection, and return all where there are at least two entries
        if ref_projection.is_empty() {
            let groups = match_db.all_confirmed();
            let groups_filtered = groups.map(|group| {
                group
                    .filter(|src_path| cand_projection.contains(src_path))
                    .collect::<Vec<_>>()
            });

            groups_filtered
                .filter_map(|g| (g.len() >= 2).then(|| MatchGroup::from(g)))
                .collect::<Vec<_>>()

        //References are more complex because if a group contains multiple references then more
        //than one group must be returned (one group for each reference)
        } else {
            enum CandRef2<T> {
                Cand(T),
                Ref(T),
            }
            use CandRef2::{Cand, Ref};

            let groups = match_db.all_confirmed();
            let groups_filtered = groups.map(|group| {
                group.filter_map(|src_path| {
                    if cand_projection.contains(src_path) {
                        Some(Cand(src_path))
                    } else if ref_projection.contains(src_path) {
                        Some(Ref(src_path))
                    } else {
                        None
                    }
                })
            });

            let mut ret: Vec<MatchGroup> = vec![];
            for group in groups_filtered {
                let (cand_paths, ref_paths): (Vec<_>, Vec<_>) =
                    group.partition_map(|src_path| match src_path {
                        Cand(x) => Either::Left(x),
                        Ref(x) => Either::Right(x),
                    });

                // There must be at least one ref and one cand to be able to form a group
                if cand_paths.is_empty() || ref_paths.is_empty() {
                    continue;
                }

                for ref_path in ref_paths {
                    let new_group = MatchGroup::new_with_reference(
                        ref_path.to_path_buf(),
                        cand_paths.iter().map(PathBuf::from),
                    );
                    ret.push(new_group)
                }
            }

            ret
        }
    }

    fn display_match_db_falsepos(
        match_db: &MatchDb,
        cand_projection: &FileProjection,
        ref_projection: &FileProjection,
    ) -> Vec<MatchGroup> {
        if ref_projection.is_empty() {
            let all_falsepos = match_db.all_falsepos();
            let filtered_falsepos = all_falsepos.filter(|group| {
                group
                    .iter()
                    .all(|src_path| cand_projection.contains(src_path))
            });

            filtered_falsepos.map(MatchGroup::from).collect::<Vec<_>>()
        } else {
            let all_falsepos = match_db.all_falsepos();
            let filtered_falsepos = all_falsepos.filter_map(|group| {
                let mut reference = None;
                let mut cand = None;

                if cand_projection.contains(group[0]) {
                    cand = Some(group[0]);

                    if ref_projection.contains(group[1]) {
                        reference = Some(group[1]);
                    }
                } else if cand_projection.contains(group[1]) {
                    cand = Some(group[1]);

                    if ref_projection.contains(group[0]) {
                        reference = Some(group[0]);
                    }
                }

                match (reference, cand) {
                    (Some(r), Some(c)) => Some((r, c)),
                    _ => None,
                }
            });

            filtered_falsepos
                .map(|(reference, cand)| {
                    MatchGroup::new_with_reference(
                        reference.to_path_buf(),
                        std::iter::once(cand).map(PathBuf::from),
                    )
                })
                .collect::<Vec<_>>()
        }
    }

    fn display_match_db_validation_failures(match_db: &MatchDb) -> Vec<MatchGroup> {
        match_db
            .confirmed_and_falsepos_entries()
            .map(|(e1, e2)| MatchGroup::from([e1.to_path_buf(), e2.to_path_buf()]))
            .collect::<Vec<_>>()
    }

    let matchset = if cfg.display_match_db_matches {
        display_match_db_matches(match_db, cand_projection, ref_projection)
    } else if cfg.display_match_db_falsepos {
        display_match_db_falsepos(match_db, cand_projection, ref_projection)
    } else if cfg.display_match_db_validation_failures {
        display_match_db_validation_failures(match_db)
    } else {
        unreachable!()
    };

    SearchOutput::new(matchset)
}

fn update_hash_cache(
    cands: &mut FileProjection,
    refs: &mut FileProjection,

    nonfatal_errs: &mut Vec<AppError>,
    reload_err_vids: bool,
    cache: &VideoHashFilesystemCache,
) -> Result<(), AppError> {
    #[cfg(feature = "print_timings")]
    let cache_update_start = Instant::now();

    use AppError as Ae;
    use FileProjectionError as Fpe;

    let cand_projection_errs = match cands.project_using_fs() {
        Ok(recoverable_errs) => recoverable_errs,
        Err(Fpe::SrcPathNotFound(paths)) => {
            return Err(Ae::CandPathNotFoundError(paths));
        }
        Err(Fpe::ExclPathNotFound(paths)) => {
            return Err(Ae::ExclPathNotFoundError(paths));
        }
        Err(_) => unreachable!(),
    };

    let ref_projection_errs = match refs.project_using_fs() {
        Ok(recoverable_errs) => recoverable_errs,
        Err(Fpe::SrcPathNotFound(paths)) => {
            return Err(Ae::RefPathNotFoundError(paths));
        }
        Err(Fpe::ExclPathNotFound(paths)) => {
            return Err(Ae::ExclPathNotFoundError(paths));
        }
        Err(_) => unreachable!(),
    };

    for recoverable_err in cand_projection_errs
        .into_iter()
        .chain(ref_projection_errs.into_iter())
    {
        match recoverable_err {
            FileProjectionError::Enumeration(err_string) => {
                nonfatal_errs.push(AppError::FileSearchError(err_string))
            }
            _ => unreachable!(),
        }
    }

    let loading_paths = cands.projected_files().chain(refs.projected_files());

    let loading_errs = cache.update_using_fs(loading_paths, false)?;
    nonfatal_errs.extend(loading_errs.into_iter().map(AppError::from));

    if reload_err_vids {
        let reload_paths = cache.error_paths();

        let loading_errs = cache.update_using_fs(reload_paths, true)?;
        nonfatal_errs.extend(loading_errs.into_iter().map(AppError::from));
    }

    cache.save()?;

    #[cfg(feature = "print_timings")]
    println!(
        "cache_update time: {}",
        cache_update_start.elapsed().as_secs_f64()
    );

    Ok(())
}

fn print_fatal_err(fatal_err: &AppError, verbosity: ReportVerbosity) {
    error!(target: "app-errorlog", "{}", fatal_err);

    if verbosity == ReportVerbosity::Verbose {
        let mut source: Option<&(dyn Error + 'static)> = fatal_err.source();
        while let Some(e) = source {
            error!(target: "app-errorlog", "    caused by: {}", e);
            source = e.source();
        }
    }
}

fn print_nonfatal_errs(nonfatal_errs: &[AppError]) {
    for err in nonfatal_errs
        .iter()
        .filter(|err| !matches!(err, AppError::CacheErrror(_)))
    {
        warn!("{}", err);
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
