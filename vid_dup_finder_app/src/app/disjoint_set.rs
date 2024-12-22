use std::{
    borrow::Borrow,
    collections::{BTreeMap, BTreeSet, HashSet},
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DisjointSet<T>
where
    T: Ord,
{
    //Maps a path into the index of "entries" containing all duplicates
    map: BTreeMap<T, usize>,
    entries: Vec<BTreeSet<T>>,
}

impl<T> DisjointSet<T>
where
    T: Ord + Clone + std::hash::Hash,
{
    pub fn insert(&mut self, p1: T, p2: T) {
        let (p1_idx, p2_idx) = (self.map.get(&p1).copied(), self.map.get(&p2).copied());

        //If we're lucky, both entries might already be in the same group. If so then
        //there is nothing to do.
        if p1_idx.is_some() && p1_idx == p2_idx {
            return;
        }

        match (p1_idx, p2_idx) {
            //no existing entry found, so add a new one
            (None, None) => self.insert_known_new_entry([p1, p2]),

            //one entry found, so append to it
            (None, Some(idx)) | (Some(idx), None) => self.append_to_entry(idx, [p1, p2]),

            //one entry found for each filename, so merge them and insert into it.
            (Some(idx_1), Some(idx_2)) => {
                let (keep_idx, _) = self.merge_entries(idx_1, idx_2);
                self.append_to_entry(keep_idx, [p1, p2]);
            }
        }
    }

    fn append_to_entry(&mut self, idx: usize, items: impl IntoIterator<Item = T>) {
        let entry = self.entries.get_mut(idx).unwrap();
        for item in items {
            entry.insert(item.clone());
            self.map.insert(item, idx);
        }
    }

    fn insert_known_new_entry(&mut self, items: impl IntoIterator<Item = T>) {
        let entry = items.into_iter().collect::<BTreeSet<_>>();

        //the new entry will got at the back of the entry list
        let idx = self.entries.len();
        for item in entry.iter().cloned() {
            self.map.insert(item, idx);
        }

        //create the new entry and ptu it there.
        self.entries.push(entry);
    }

    //merge two entries. this operation may move other elements in Self.map and
    //self.entries. Return the index of whichever index was kept, and if the last item
    //was moved, the new location of that item.
    fn merge_entries(&mut self, idx_1: usize, idx_2: usize) -> (usize, Option<usize>) {
        //Remove one entry and preserve the other. To make sure
        //that the entry that is preserved is not renumbered as
        //part of the removal process, make sure that if either
        //is the last entry, then that is the one that is removed.
        let (preserve_idx, remove_idx) = if idx_1 < idx_2 {
            (idx_1, idx_2)
        } else {
            (idx_2, idx_1)
        };

        //add the filenames of the removed entry back into the list of entries,
        //with the preserve_entries
        let (filenames_to_merge, new_idx_for_last_item) = self.remove_entry(remove_idx);
        for filename in filenames_to_merge {
            self.map.insert(filename.clone(), preserve_idx);
            self.entries[preserve_idx].insert(filename);
        }

        (preserve_idx, new_idx_for_last_item)
    }

    //removes an entry. This option may move the previous last item to another location.
    //If so, return the new location of that item.
    fn remove_entry(&mut self, idx: usize) -> (BTreeSet<T>, Option<usize>) {
        let last_idx = self.entries.len() - 1;

        //If the entry is the last in the list, then delete it and remove its filenames from the map
        let remove_filenames;
        let ret_idx;
        let mut reorder_filenames = None;
        if idx == last_idx {
            ret_idx = None;
            remove_filenames = self.entries.remove(idx);

        //If it's not the last, then delete it with swap_remove, and fix the indices of the entry that
        //was swapped.
        } else {
            ret_idx = Some(idx);
            remove_filenames = self.entries.swap_remove(idx);
            reorder_filenames = Some(&self.entries[last_idx - 1]);
        };

        for filename in &remove_filenames {
            self.map.remove(filename);
        }

        if let Some(reorder_filenames) = reorder_filenames {
            for filename in reorder_filenames {
                self.map.insert(filename.clone(), idx);
            }
        }

        (remove_filenames, ret_idx)
    }

    pub fn all_items(&self) -> impl Iterator<Item = &T> {
        self.entries.iter().flat_map(|x| x.iter())
    }

    pub fn all_sets(&self) -> impl Iterator<Item = impl Iterator<Item = &T> + Clone> + Clone {
        self.entries.iter().map(|x| x.iter())
    }

    pub fn remove_item<T1>(&mut self, item: &T1)
    where
        T: Borrow<T1>,
        T1: Ord,
        T1: ?Sized,
    {
        // let x = self.map.remove(&item).is_some();
        // debug_assert!(x);

        let idxs_to_remove_path_from = self
            .entries
            .iter()
            .enumerate()
            .rev()
            .filter_map(|(idx, entry)| entry.contains(item).then_some(idx))
            .collect::<Vec<_>>();

        // let x= idxs_to_remove_path_from.len() <= 1;
        // debug_assert!(x);

        for idx in idxs_to_remove_path_from {
            let entry = self.entries.get_mut(idx).unwrap();
            if entry.len() <= 1 {
                unreachable!("MatchMap should never have an entry with lengths less than 2.")
            } else if entry.len() == 2 {
                self.remove_entry(idx);
            } else {
                assert!(entry.remove(item));
            }
        }
    }

    pub fn contains_pair<T1>(&self, i1: &T1, i2: &T1) -> bool
    where
        T: Borrow<T1>,
        T1: Ord,
        T1: ?Sized,
    {
        let (Some(p1_idx), Some(p2_idx)) = (self.map.get(i1.borrow()), self.map.get(i2.borrow()))
        else {
            return false;
        };

        p1_idx == p2_idx
    }

    #[allow(dead_code)]
    pub fn dbg_check_integrity(&self) {
        let stuff_in_map_but_not_entries = || -> Vec<T> {
            self.map
                .keys()
                .collect::<HashSet<_>>()
                .difference(
                    &self
                        .entries
                        .iter()
                        .flat_map(|x| x.iter())
                        .collect::<HashSet<_>>(),
                )
                .map(|x| (*x).clone())
                .collect::<Vec<_>>()
        };

        let stuff_in_entries_but_not_map = || -> Vec<T> {
            self.entries
                .iter()
                .flat_map(|x| x.iter())
                .collect::<HashSet<_>>()
                .difference(&self.map.keys().collect::<HashSet<_>>())
                .map(|x| (*x).clone())
                .collect::<Vec<_>>()
        };

        let lone_entries = stuff_in_entries_but_not_map();
        // dbg!(&lone_entries);
        assert!(lone_entries.is_empty());

        let lone_mapitems = stuff_in_map_but_not_entries();
        // dbg!(&lone_mapitems);
        assert!(lone_mapitems.is_empty());
    }
}

#[cfg(test)]
mod test {
    use super::DisjointSet;

    #[test]
    pub fn test_insert() {
        let mut set = DisjointSet::<usize>::default();
        set.insert(1, 2);
        check_entries_equal(&set, &[1, 2]);
    }

    #[test]
    pub fn test_insert_extra_item_to_single_set() {
        let mut set = DisjointSet::<usize>::default();
        set.insert(1, 2);
        set.insert(2, 3);
        set.insert(3, 3);
        check_entries_equal(&set, &[1, 2, 3]);
        assert!(set.all_sets().count() == 1);
    }

    #[test]
    pub fn test_insert_two_sets() {
        let mut set = DisjointSet::<usize>::default();
        set.insert(1, 2);
        set.insert(2, 3);
        set.insert(11, 12);
        check_entries_equal(&set, &[1, 2, 3, 11, 12]);
        assert!(set.all_sets().count() == 2);
    }

    #[test]
    pub fn test_remove_item() {
        let mut set = DisjointSet::<usize>::default();
        set.insert(1, 2);
        check_entries_equal(&set, &[1, 2]);
        assert!(set.all_sets().count() == 1);

        set.remove_item(&1);
        check_entries_equal(&set, &[]);
        assert!(set.all_sets().count() == 0);

        set.insert(1, 2);
        set.insert(1, 3);
        set.insert(11, 12);
        set.insert(11, 13);
        check_entries_equal(&set, &[1, 2, 3, 11, 12, 13]);
        assert!(set.all_sets().count() == 2);

        set.remove_item(&2);
        check_entries_equal(&set, &[1, 3, 11, 12, 13]);
        assert!(set.all_sets().count() == 2);

        set.remove_item(&1);
        check_entries_equal(&set, &[11, 12, 13]);
        assert!(set.all_sets().count() == 1);
    }

    #[test]
    pub fn test_remove_middle_set() {
        let mut set = DisjointSet::<usize>::default();
        set.insert(1, 2);
        set.insert(1, 3);
        set.insert(11, 12);
        set.insert(11, 13);
        set.insert(11, 14);
        set.insert(21, 22);
        set.insert(21, 23);
        check_entries_equal(&set, &[1, 2, 3, 11, 12, 13, 14, 21, 22, 23]);
        assert!(set.all_sets().count() == 3);

        set.remove_item(&14);
        check_entries_equal(&set, &[1, 2, 3, 11, 12, 13, 21, 22, 23]);
        assert!(set.all_sets().count() == 3);

        set.remove_item(&13);
        check_entries_equal(&set, &[1, 2, 3, 11, 12, 21, 22, 23]);
        assert!(set.all_sets().count() == 3);

        set.remove_item(&12);
        check_entries_equal(&set, &[1, 2, 3, 21, 22, 23]);
        assert!(set.all_sets().count() == 2);
    }

    #[test]
    pub fn test_contains_pair() {
        let mut set = DisjointSet::<usize>::default();
        assert!(!set.contains_pair(&1, &2));
        set.insert(1, 2);
        check_entries_equal(&set, &[1, 2]);
        assert!(set.contains_pair(&1, &2));

        set.insert(1, 3);
        assert!(set.contains_pair(&1, &3));
        assert!(set.contains_pair(&2, &3));

        set.insert(11, 12);
        assert!(set.contains_pair(&11, &12));
        assert!(!set.contains_pair(&1, &11));
    }

    fn check_entries_equal<T>(set: &DisjointSet<T>, exp: &[T])
    where
        T: Ord + Clone + std::hash::Hash,
    {
        let mut act = set.all_items().cloned().collect::<Vec<_>>();
        act.sort();

        let mut exp = exp.to_vec();
        exp.sort();

        assert!(act.len() == exp.len());
        for (a, e) in act.into_iter().zip(exp.into_iter()) {
            if a != e {
                panic!()
            }
        }
    }
}
