use std::{
    io::{prelude::*, BufWriter},
    path::{Path, PathBuf},
};

use serde::Serialize;
use serde_json::json;
use vid_dup_finder_lib::*;

use crate::app::match_group_ext::MatchGroupExt;
use video_hash_filesystem_cache::VideoHashFilesystemCache;

#[cfg(all(target_family = "unix", feature = "gui"))]
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

    pub fn sort(&mut self, sorting: Sorting) {
        let key_fn = match sorting {
            Sorting::NumMatches => |g: &MatchGroup| usize::MAX - g.len(),
            Sorting::Distance => unimplemented!(),
            Sorting::Duration => unimplemented!(),
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

        let chunk_size = 250;
        self.dup_groups
            .chunks(chunk_size)
            .enumerate()
            .for_each(|(chunk_no, chunk)| {
                let chunk_offset = chunk_no * chunk_size;

                #[cfg(feature = "parallel_loading")]
                let it = chunk.into_par_iter();

                #[cfg(not(feature = "parallel_loading"))]
                let it = chunk.iter();

                it.enumerate().for_each(|(group_no, match_group)| {
                    let i = chunk_offset + group_no;
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
            });
    }

    #[cfg(all(target_family = "unix", feature = "gui"))]
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

    //for all dup groups within, merges all groups that contain dups
    pub fn coalesce(&self, cache: &VideoHashFilesystemCache) -> (Self, Vec<String>) {
        let mut new_groups = self.dup_groups.clone();
        let mut uncoalescable_groups = vec![];
        let mut idx = 0;

        let fetch_hashes = |group: &MatchGroup| {
            let (oks, errs): (Vec<_>, Vec<_>) = group
                .contained_paths()
                .map(|ref_path| cache.fetch(ref_path).map_err(|e| e.to_string()))
                .partition(Result::is_ok);
            let oks = oks.into_iter().map(Result::unwrap).collect::<Vec<_>>();
            let errs = errs.into_iter().map(Result::unwrap_err).collect::<Vec<_>>();

            (oks, errs)
        };

        let find_match = |group_1: &[VideoHash], group_2: &[VideoHash]| {
            group_1.iter().any(|hash_1| {
                group_2
                    .iter()
                    .any(|hash_2| hash_1.normalized_hamming_distance(hash_2) < 0.3)
            })
        };

        let mut errs: Vec<String> = vec![];

        while idx < new_groups.len() {
            let ref_group = &new_groups[idx];
            let (ref_hashes, errs_temp) = fetch_hashes(ref_group);

            if !errs_temp.is_empty() {
                errs.extend(errs_temp);
                let ref_group_owned = new_groups.swap_remove(idx);
                uncoalescable_groups.push(ref_group_owned);
                continue;
            }

            for rev_idx in (idx + 1..new_groups.len()).rev() {
                let cand_group = &new_groups[rev_idx];
                let (cand_hashes, errs_temp) = fetch_hashes(cand_group);
                if !errs_temp.is_empty() {
                    errs.extend(errs_temp);
                    let cand_group_owned = new_groups.swap_remove(rev_idx);
                    uncoalescable_groups.push(cand_group_owned);
                    continue;
                }

                let match_found = find_match(&ref_hashes, &cand_hashes);
                if match_found {
                    let mut new_paths = ref_hashes
                        .iter()
                        .map(|hash| hash.src_path().to_path_buf())
                        .collect::<Vec<_>>();
                    new_paths.extend(cand_hashes.iter().map(|hash| hash.src_path().to_path_buf()));
                    new_paths.sort();
                    new_paths.dedup();

                    new_groups[idx] = MatchGroup::from(new_paths);
                    new_groups.remove(rev_idx);
                }
            }
            idx += 1;
        }

        new_groups.extend(uncoalescable_groups);

        let ret = Self {
            dup_groups: new_groups,
        };

        (ret, errs)
    }
}
