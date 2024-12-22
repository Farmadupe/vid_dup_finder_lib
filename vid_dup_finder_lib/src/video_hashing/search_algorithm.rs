use std::path::PathBuf;

use crate::{definitions::TOLERANCE_SCALING_FACTOR, VideoHash};
#[derive(Debug, Default)]
struct Entry {
    matched: bool,
    value: VideoHash,
}

impl From<VideoHash> for Entry {
    fn from(val: VideoHash) -> Self {
        Self {
            matched: false,
            value: val,
        }
    }
}

/// A data structure for performing duplicate video searches.
#[derive(Debug, Default)]
pub(super) struct Search {
    entries: Vec<Entry>,
}

impl Search {
    const fn new() -> Self {
        Self { entries: vec![] }
    }

    ///Add video hashes into the Search, for use in searches.
    pub fn seed(&mut self, new_entries: impl IntoIterator<Item = VideoHash>) {
        self.entries.extend(new_entries.into_iter().map(Into::into));
        self.sort();
    }

    ///Search all seeded items for duplicates to a set of reference videos, within the given tolerance.
    ///
    ///if consume is true, a seeded value can be matched against a maximum of one reference video.
    ///Otherwise seeded values can occur in many matches.
    pub fn search_with_references<R>(
        &mut self,
        references: &[R],
        tolerance: f64,
        consume: bool,
    ) -> Vec<Vec<PathBuf>>
    where
        R: AsRef<VideoHash> + Send + Sync,
    {
        references
            .iter()
            .map(|target| self.search_one(target.as_ref(), tolerance, consume))
            .collect()
    }

    fn sort(&mut self) {
        //in order to be fully deterministic, it is necessary to use the src_path as a secondary key.
        //however we have to use .to_owned() which forces an allocation nlogn times during the sort.
        //Note: src_path is not necessary for correctness, only determinism.
        self.entries
            .sort_by_key(|entry| (entry.value.duration(), entry.value.src_path().to_owned()));
    }

    fn search_one(&mut self, target: &VideoHash, tolerance: f64, consume: bool) -> Vec<PathBuf> {
        let tolerance_int = (tolerance * TOLERANCE_SCALING_FACTOR) as u32;
        let mut ret = vec![];

        for entry in self.duration_slice(target.duration()) {
            if !entry.matched && target.hamming_distance(&entry.value) <= tolerance_int {
                ret.push(entry.value.src_path().to_path_buf());
                if consume {
                    entry.matched = true;
                }
            }
        }

        ret
    }

    /// Search within all seeded videos for duplicates, within the given tolerance.
    /// Each video will be matched a maximum of once.
    pub fn search_self(&mut self, tolerance: f64) -> Vec<Vec<PathBuf>> {
        let tolerance_int = (tolerance * TOLERANCE_SCALING_FACTOR) as u32;

        let mut lhs = 0;
        let mut rhs = 0;

        //base case: If there are 0  entries then nothing can be found, so exit
        //(searching nothing also causes a panic..)
        if self.entries.is_empty() {
            return vec![];
        }

        let advance_rhs = |lhs: usize, mut rhs: usize, entries: &Vec<Entry>| -> Option<usize> {
            let lhs_duration = entries
                .get(lhs)
                .expect("lhs is always a valid index")
                .value
                .duration();
            let thresh_duration = (f64::from(lhs_duration) * 1.1) as u32;
            loop {
                #[rustfmt::skip]
                match entries.get(rhs) {
                    None =>
                        return Some(rhs ),

                    Some(Entry { matched: true, .. }) =>
                        rhs += 1,

                    Some(Entry { matched: false, value: hash, }) => {
                        if hash.duration() > thresh_duration {
                            return Some(rhs);
                        }
                        rhs += 1;
                    }
                };
            }
        };

        let advance_lhs = |mut lhs: usize, entries: &Vec<Entry>| -> Option<usize> {
            loop {
                lhs += 1;
                #[rustfmt::skip]
                match entries.get(lhs) {
                    None => return None,
                    Some(Entry { matched: true, value: _, }) => (),
                    Some(Entry { matched: false, value: _, }) => return Some(lhs),
                };
            }
        };

        let mut ret = vec![];
        loop {
            if let Some(next_rhs) = advance_rhs(lhs, rhs, &self.entries) {
                rhs = next_rhs;
            } else {
                ret.reverse();
                return ret;
            }

            if lhs < rhs {
                let slice_to_search = &mut self.entries[lhs..rhs];

                let mut it = slice_to_search.iter_mut();
                let target = it
                    .next()
                    .expect("iterator can't be at the end in this branch because lhs < rhs");
                target.matched = true;

                let mut match_vec = vec![];
                for cand in it {
                    if !cand.matched && target.value.hamming_distance(&cand.value) <= tolerance_int
                    {
                        match_vec.push(cand.value.src_path().to_path_buf());
                        cand.matched = true;
                    }
                }

                if !match_vec.is_empty() {
                    match_vec.push(target.value.src_path().to_path_buf());
                    ret.push(match_vec);
                }
            }

            if let Some(next_lhs) = advance_lhs(lhs, &self.entries) {
                lhs = next_lhs;
            } else {
                ret.reverse();
                return ret;
            }
        }
    }

    fn duration_slice(&mut self, duration_secs: u32) -> &mut [Entry] {
        let lhs_duration = (f64::from(duration_secs) * 0.95) as u32;
        let lhs = self
            .entries
            .partition_point(|entry| entry.value.duration() < lhs_duration);

        let rhs_duration = (f64::from(duration_secs) * 1.05) as u32;
        let rhs = self
            .entries
            .partition_point(|entry| entry.value.duration() <= rhs_duration);

        &mut self.entries[lhs..rhs]
    }
}

impl<I> std::convert::From<I> for Search
where
    I: IntoIterator<Item = VideoHash>,
{
    fn from(v: I) -> Self {
        let mut ret = Self::new();

        ret.seed(v);

        ret
    }
}
#[cfg(test)]
mod test {
    use crate::search;
    #[test]
    fn test_searching_nothing_returns_empty_vec() {
        let no_hashes = vec![];
        let matchgroups = search(no_hashes, 1.0);
        assert!(matchgroups.is_empty());
    }
}
