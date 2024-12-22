use crate::{MatchGroup, VideoHash};

use super::search_algorithm::Search;

/// Search for duplicates within the given hashes, within the given tolerance. Returns groups for all the matching videos.
/// Each group may have multiple entries if multiple videos are duplicates of each other.
pub fn search(hashes: impl IntoIterator<Item = VideoHash>, tolerance: f64) -> Vec<MatchGroup> {
    Search::from(hashes)
        .search_self(tolerance)
        .into_iter()
        .filter_map(|x| MatchGroup::new(x).ok())
        .collect()
}

/// Search new_hashes for all videos that are duplicates of videos in ref_hashes. Returns a set of groups,
/// one group for each reference video that was matched.
/// # Panics
/// Should only panic due to internal implementation error
pub fn search_with_references(
    ref_hashes: impl IntoIterator<Item = VideoHash>,
    new_hashes: impl IntoIterator<Item = VideoHash>,
    tolerance: f64,
) -> Vec<MatchGroup> {
    let mut search_struct = Search::from(new_hashes);
    ref_hashes
        .into_iter()
        .filter_map(|ref_hash| {
            let mut search_result =
                search_struct.search_with_references(&[&ref_hash], tolerance, false);

            // Because we search with only a single reference video at a time, the above
            // returns a vec of length exactly 1. If there are any matches then the 0th
            // element contains the matches.
            let search_result = search_result
                .pop()
                .expect("search always returns exactly 1 element");

            if search_result.is_empty() {
                None
            } else {
                MatchGroup::new_with_reference(ref_hash.src_path().to_path_buf(), search_result)
                    .ok()
            }
        })
        .collect()
}
