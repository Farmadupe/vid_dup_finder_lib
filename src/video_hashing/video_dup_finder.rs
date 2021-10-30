use crate::*;

use super::search::Search;

/// Search for duplicates within the given hashes, within the given tolerance. Returns groups for all the matching videos.
/// Each group may have multiple entries if multiple videos are duplicates of each other.
pub fn search(hashes: impl IntoIterator<Item = VideoHash>, tolerance: NormalizedTolerance) -> Vec<MatchGroup> {
    Search::from(hashes)
        .search_self(RawTolerance::from(&tolerance))
        .into_iter()
        .map(Into::into)
        .collect()
}

/// Given a set of 'reference' videos, search new_hashes for all duplicate videos. Returns a set of groups,
/// one group for each reference video that was matched.
pub fn search_with_references(
    ref_hashes: impl IntoIterator<Item = VideoHash>,
    new_hashes: impl IntoIterator<Item = VideoHash>,
    tolerance: NormalizedTolerance,
) -> Vec<MatchGroup> {
    let mut search_struct = Search::from(new_hashes);
    ref_hashes
        .into_iter()
        .filter_map(|new_hash| {
            let search_result =
                search_struct.search_with_references(&[&new_hash], RawTolerance::from(&tolerance), false);
            let search_result = search_result.get(0).unwrap();
            if search_result.is_empty() {
                None
            } else {
                Some(MatchGroup::new_with_reference(new_hash, search_result.iter().cloned()))
            }
        })
        .collect()
}
