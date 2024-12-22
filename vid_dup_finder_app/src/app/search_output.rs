use std::{
    io::{prelude::*, BufWriter},
    path::{Path, PathBuf},
};

use itertools::Itertools;
use serde::Serialize;
use serde_json::json;
use vid_dup_finder_lib::MatchGroup;

use crate::app::match_group_ext::MatchGroupExt;

//#[cfg(feature = "gui")]
use crate::video_hash_filesystem_cache::VideoHashFilesystemCache;

#[cfg(all(target_family = "unix", feature = "gui_slint",))]
use crate::app::ResolutionThunk;

use super::Sorting;

#[derive(Debug, Clone)]
pub struct SearchOutput {
    dup_groups: Vec<MatchGroup>,
}

impl SearchOutput {
    pub fn new(dup_groups: Vec<MatchGroup>) -> Self {
        Self { dup_groups }
    }

    pub fn len(&self) -> usize {
        self.dup_groups.len()
    }

    pub fn dup_groups(&self) -> impl Iterator<Item = &MatchGroup> {
        self.dup_groups.iter()
    }

    pub fn dup_paths(&self) -> impl Iterator<Item = &Path> {
        self.dup_groups.iter().flat_map(MatchGroup::duplicates)
    }

    pub fn sort(&mut self, sorting: Sorting, cache: &VideoHashFilesystemCache) {
        let sort_num_matches = |g: &MatchGroup| u32::MAX - g.len() as u32;
        let sort_distance = |g: &MatchGroup| {
            g.contained_paths()
                .map(|path| cache.fetch(path))
                .collect::<Vec<_>>()
                .iter()
                .tuple_combinations::<(_, _)>()
                .map(|comb| {
                    if let (Ok(h1), Ok(h2)) = comb {
                        h1.hamming_distance(h2)
                    } else {
                        u32::MAX
                    }
                })
                .max()
                .unwrap()
        };

        let sort_duration = |g: &MatchGroup| match g.contained_paths().next() {
            Some(first_vid) => match cache.fetch(first_vid) {
                Ok(hash) => u32::MAX - hash.duration(),
                Err(_) => u32::MAX / 2,
            },
            None => u32::MIN,
        };
        let key_fn = |g: &MatchGroup| match sorting {
            Sorting::NumMatches => sort_num_matches(g),
            Sorting::RevNumMatches => u32::MAX - sort_num_matches(g),
            Sorting::Distance => sort_distance(g),
            Sorting::RevDistance => u32::MAX - sort_distance(g),
            Sorting::Duration => sort_duration(g),
            Sorting::RevDuration => u32::MAX - sort_duration(g),
        };

        self.dup_groups.sort_by_key(key_fn)
    }

    pub fn save_debug_imgs(&self, output_thumbs_dir: impl AsRef<Path>) {
        #[cfg(feature = "parallel_loading")]
        use rayon::prelude::*;

        let output_thumbs_dir = output_thumbs_dir.as_ref();

        //first save a json file which maps image names to the duplicate videos within.
        {
            //dump indexed log of matches
            #[derive(Serialize)]
            struct IdxAndMatchSet {
                pub idx: u64,
                pub matchset: Vec<PathBuf>,
            }
            let json_vec: Vec<_> = self
                .dup_groups
                .iter()
                .enumerate()
                .map(|(i, matchset)| IdxAndMatchSet {
                    idx: i as u64,
                    matchset: matchset.contained_paths().map(PathBuf::from).collect(),
                })
                .collect();

            let s = serde_json::to_string_pretty(&json!(json_vec)).unwrap();
            let dest_file_name = output_thumbs_dir.join("idx.json");
            std::fs::create_dir_all(output_thumbs_dir).unwrap();
            let f = std::fs::File::create(dest_file_name).unwrap();
            let mut f = BufWriter::new(f);
            f.write_all(s.as_bytes()).unwrap();
        }

        #[cfg(feature = "parallel_loading")]
        let it = self
            .dup_groups
            .iter()
            .enumerate()
            .par_bridge()
            .into_par_iter();

        #[cfg(not(feature = "parallel_loading"))]
        let it = self.dup_groups.iter().enumerate();

        it.for_each(|(i, match_group)| {
            let output_path = output_thumbs_dir.join(format!("{i}.jpg"));

            info!(
                target: "write_image",
                    "Writing match image to {}", output_path.display()
            );

            match match_group.to_image() {
                Ok(img) => {
                    std::fs::create_dir_all(output_path.parent().unwrap()).unwrap();
                    img.save(output_path).unwrap();
                }
                Err(msg) => {
                    let dup_group_paths = match_group
                    .contained_paths()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect::<Vec<_>>();
                    warn!(
                        "failed to save output images: {msg}, dup_group_paths: {dup_group_paths:?}, img_path: {output_path:?}"
                    );
                    //panic!();
                }
            };

            });
    }

    #[cfg(all(target_family = "unix", feature = "gui_slint"))]
    pub fn resolution_thunks(
        &self,
        cache: &VideoHashFilesystemCache,
        gui_trash_path: Option<&Path>,
    ) -> Vec<ResolutionThunk> {
        self.dup_groups
            .iter()
            .map(|group| ResolutionThunk::from_matchgroup(group, cache, gui_trash_path))
            .collect::<Vec<_>>()
    }
}
