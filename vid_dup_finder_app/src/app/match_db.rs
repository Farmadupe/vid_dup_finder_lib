use std::{
    collections::{BTreeMap, BTreeSet},
    default::Default,
    io::{BufReader, Write},
    path::{Path, PathBuf},
};

use itertools::Itertools;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MatchDbError {
    #[error("IO error while reading raw DB file: {0}")]
    IdxReadIoError(PathBuf, #[source] std::io::Error),

    #[error("JSON deserialization error while reading raw DB file: {0}")]
    IdxDeserialize(PathBuf, #[source] serde_json::Error),

    #[error("IO error while reading confirmed items: {0}")]
    ConfirmedDirIoError(PathBuf, #[source] std::io::Error),

    #[error("Could not extract valid match number: filename: {0}. Index file: {1}")]
    IndexProcError(PathBuf, PathBuf),

    #[error("Match number is out of range: number {0}, max: {1}, Index file: {2}")]
    IndexRangeError(usize, usize, PathBuf),
}

pub type MatchDbResult<T> = Result<T, MatchDbError>;

//Maps filesystem paths into other paths (those which contain duplicates)
//
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct MatchMap {
    //Maps a path into the index of "entries" containing all duplicates
    map: BTreeMap<PathBuf, usize>,
    entries: Vec<BTreeSet<PathBuf>>,
}

impl MatchMap {
    pub fn insert(&mut self, p1: impl AsRef<Path>, p2: impl AsRef<Path>) {
        let (p1, p2) = (p1.as_ref(), p2.as_ref());

        let (p1_idx, p2_idx) = (self.map.get(p1).copied(), self.map.get(p2).copied());

        //If we're lucky, both entries might already be in the same group. If so then
        //there is nothing to do.
        if p1_idx.is_some() && p1_idx == p2_idx {
            return;
        }

        match (p1_idx, p2_idx) {
            //no existing entry found, so add a new one
            (None, None) => self.insert_known_new_entry(&[p1, p2]),

            //one entry found, so append to it
            (None, Some(idx)) | (Some(idx), None) => self.append_to_entry(idx, &[p1, p2]),

            //one entry found for each filename, so merge them and insert into it.
            (Some(idx_1), Some(idx_2)) => {
                let (keep_idx, _) = self.merge_entries(idx_1, idx_2);
                self.append_to_entry(keep_idx, &[p1, p2]);
            }
        }
    }

    fn append_to_entry(&mut self, idx: usize, filenames: &[&Path]) {
        let entry = self.entries.get_mut(idx).unwrap();
        for path in filenames {
            entry.insert(path.to_path_buf());
            self.map.insert(path.to_path_buf(), idx);
        }
    }

    fn insert_known_new_entry(&mut self, filenames: &[&Path]) {
        //the new entry will got at the back of the entry list
        let idx = self.entries.len();
        for filename in filenames {
            self.map.insert(filename.to_path_buf(), idx);
        }

        //create the new entry and ptu it there.
        let entry = filenames
            .iter()
            .map(|p| p.to_path_buf())
            .collect::<BTreeSet<_>>();
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
    fn remove_entry(&mut self, idx: usize) -> (BTreeSet<PathBuf>, Option<usize>) {
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

    fn remove_path(&mut self, path: impl AsRef<Path>) {
        let path = path.as_ref();

        self.map.remove(path);

        let idxs_to_remove_path_from = self
            .entries
            .iter()
            .enumerate()
            .rev()
            .filter_map(|(idx, entry)| entry.contains(path).then_some(idx))
            .collect::<Vec<_>>();

        for idx in idxs_to_remove_path_from {
            let entry_len = self.entries.get(idx).unwrap().len();
            if entry_len <= 1 {
                unreachable!("MatchMap should never have an entry with lengths less than 2.")
            } else if entry_len == 2 {
                self.remove_entry(idx);
            } else {
                let entry = self.entries.get_mut(idx).unwrap();
                //println!("{entry:#?}");
                assert!(entry.remove(path));
            }
        }
    }

    fn get_entries(&self, p: impl AsRef<Path>) -> Option<&BTreeSet<PathBuf>> {
        let p = p.as_ref();

        let entry_idx = self.map.get(p);
        let dups = entry_idx.map(|entry_idx| self.entries.get(*entry_idx));

        match dups {
            // MatchMap knew nothing about this file
            // ...probably a match on a new video
            None => None,

            // Found a list of entries that match this video.
            Some(Some(stuff)) => Some(stuff),

            // Should never occur... self.map says that this item is a known dup,
            // but when we tried to find out what the other videos are, self.entries
            // did not have an entry
            Some(None) => {
                panic!(
                    "error: requested to get entries for {} at idx that does not exist: {entry_idx:?}",
                    p.display()
                );
            }
        }
    }

    pub fn all_entries(&self) -> impl Iterator<Item = impl Iterator<Item = &Path> + Clone> + Clone {
        self.entries.iter().map(|x| x.iter().map(&PathBuf::as_path))
    }

    //iterates through every entry, and checks that each file inside actually
    //exists on disk. If not, then removes the entry
    pub fn remove_deleted_items(&mut self) -> Result<(), std::io::Error> {
        assert!(self.entries.iter().all(|e| e.len() >= 2));

        let mut err = Ok(());
        let all_paths = self.map.keys().cloned().collect::<Vec<_>>();

        //for each path in the list of entries, remove it.
        let paths_deleted_from_fs = all_paths
            .iter()
            .map(PathBuf::as_path)
            .scan(&mut err, |err, path| match file_exists(path) {
                Ok(true) => Some(None),
                Ok(false) => Some(Some(path)),
                Err(e) => {
                    **err = Err(e);
                    None
                }
            })
            .flatten();

        for path in paths_deleted_from_fs {
            self.remove_path(path);
        }

        err?;

        assert!(self.entries.iter().all(|e| e.len() >= 2));
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct FalseposMap {
    map: BTreeMap<PathBuf, BTreeSet<PathBuf>>,
}

impl FalseposMap {
    pub fn insert<I: IntoIterator<Item = impl AsRef<Path>>>(&mut self, filenames: I)
    where
        <I as IntoIterator>::IntoIter: Clone,
    {
        let filenames = filenames.into_iter().map(|x| x.as_ref().to_path_buf());

        //When given a set of false positives, each entry is a falsepos with each other entry.
        //This can be got from tuple_combinations. For each combinations, we have to register the
        //falsepos from p2->p1, and also from p1->p2.
        let all_falsepos_pairs = filenames
            .tuple_combinations::<(_, _)>()
            .flat_map(|(p1, p2)| [(p1.clone(), p2.clone()), (p2, p1)]);

        for (p1, p2) in all_falsepos_pairs {
            //check if we have to add brand new entries, or if one already exists.
            match self.map.get_mut(&p1) {
                None => {
                    let mut falsepos_set = BTreeSet::new();
                    falsepos_set.insert(p2.clone());
                    assert!(self.map.insert(p1.clone(), falsepos_set).is_none());
                }
                Some(falsepos_set) => {
                    falsepos_set.insert(p2.clone());
                }
            }
        }
    }

    pub fn all_entries(&self) -> impl Iterator<Item = [&Path; 2]> {
        // for each key and its falsepos entries, get pairs with all its duplicates.
        // Because this structure is a two way mapping, this would return each falsepos
        // pair exactly twice i.e for the pair (p1, p2), it would return Vec[entry_1: [v1, v2], entry_2: [v2, v1]].
        // To avoid this we will filter out all entries according to the Ord trait.. Because if
        // (entry_1[0] > entry_1[0]) is true, then (entry_2[0] > entry_2[1]) is false.
        self.map.iter().flat_map(|(p1, dups)| {
            dups.iter()
                .map(move |p2| (p1, p2))
                .filter(|(p1, p2)| p1 > p2)
                .map(|(p1, p2)| [p1.as_path(), p2.as_path()])
        })
    }

    pub fn get_entries(&self, p: impl AsRef<Path>) -> Option<impl Iterator<Item = &Path>> {
        self.map
            .get(p.as_ref())
            .map(|dups| dups.iter().map(PathBuf::as_path))
    }

    fn remove_path(&mut self, path_to_remove: impl AsRef<Path>) {
        let path_to_remove = path_to_remove.as_ref();
        let mut entries_to_remove = vec![];

        for (entry_path, entry) in self.map.iter_mut().rev() {
            if entry.contains(path_to_remove) {
                if entry.len() <= 1 {
                    unreachable!("FalseposMap should never have an entry with lengths less than 2.")
                } else if entry.len() == 2 {
                    entries_to_remove.push(entry_path.clone());
                } else {
                    assert!(entry.remove(path_to_remove));
                }
            }
        }

        for entry in entries_to_remove {
            assert!(self.map.remove(&entry).is_some());
        }
    }

    pub fn remove_deleted_items(&mut self) -> Result<(), std::io::Error> {
        //temporary -- somehow there are single length entries.. so remove them now.
        self.map.retain(|_path, entry| entry.len() >= 2);
        assert!(self.map.values().all(|entry| entry.len() >= 2));

        let mut err = Ok(());
        let all_paths = self.map.keys().cloned().collect::<Vec<_>>();

        //for each path in the list of entries, remove it.
        let paths_deleted_from_fs = all_paths
            .iter()
            .map(PathBuf::as_path)
            .scan(&mut err, |err, path| match file_exists(path) {
                Ok(true) => Some(None),
                Ok(false) => Some(Some(path)),

                Err(e) => {
                    **err = Err(e);
                    None
                }
            })
            .flatten();

        for path in paths_deleted_from_fs {
            self.remove_path(path);
        }
        err?;
        assert!(self.map.values().all(|entry| entry.len() >= 2));
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct MatchDb {
    db_path: PathBuf,
    confirmed: MatchMap,
    falsepos: FalseposMap,
}

impl MatchDb {
    pub fn exists_on_disk(db_path: impl AsRef<Path>) -> bool {
        let db_path = db_path.as_ref();

        let confirmed_path = Self::confirmed_db_path(db_path);
        let confirmed_db_exists = match std::fs::metadata(confirmed_path) {
            Ok(metadata) => metadata.is_file(),
            Err(_) => false,
        };

        let falsepos_path = Self::falsepos_db_path(db_path);
        let falsepos_db_exists = match std::fs::metadata(falsepos_path) {
            Ok(metadata) => metadata.is_file(),
            Err(_) => false,
        };

        confirmed_db_exists && falsepos_db_exists
    }

    fn confirmed_db_path(db_path: impl AsRef<Path>) -> PathBuf {
        db_path.as_ref().join("confirmed.json")
    }

    fn falsepos_db_path(db_path: impl AsRef<Path>) -> PathBuf {
        db_path.as_ref().join("falsepos.json")
    }

    pub fn new(db_path: impl AsRef<Path>) -> Self {
        Self {
            db_path: db_path.as_ref().to_owned(),

            confirmed: MatchMap::default(),
            falsepos: FalseposMap::default(),
        }
    }

    pub fn load_raw(
        &mut self,
        raw_db_paths: impl IntoIterator<Item = impl AsRef<Path>>,
    ) -> MatchDbResult<()> {
        let raw_db_paths: Vec<PathBuf> = raw_db_paths
            .into_iter()
            .map(|p| p.as_ref().to_path_buf())
            .collect();

        let raw_db_entries = raw_db_paths.iter().flat_map(Self::process_raw_db);

        for result in raw_db_entries {
            let (is_confirmed, entries) = result?;
            if is_confirmed {
                for (p1, p2) in entries.iter().tuple_combinations() {
                    self.confirmed.insert(p1, p2);
                }
            } else {
                self.falsepos.insert(entries);
            }
        }

        Ok(())
    }

    pub fn confirmed_and_falsepos_entries(&self) -> impl Iterator<Item = (&Path, &Path)> {
        //collect all pairs of videos that are both confirmed and falsepos.
        let all_entries = self.all_confirmed().flat_map(|confirmed_files| {
            confirmed_files
                .combinations(2)
                .map(|combination| (combination[0], combination[1]))
        });

        all_entries.filter(|(p1, p2)| self.is_falsepos(p1, p2))
    }

    pub fn to_disk(&self) {
        //first make sure the match db directory can be created.
        std::fs::create_dir_all(&self.db_path).expect("Unable to create match database directory");

        //create a backup of the db if it already exists
        {
            let confirmed_path = Self::confirmed_db_path(&self.db_path);
            match file_exists(&confirmed_path) {
                Err(e) => panic!("Error while creating backup of match database: {e}"),
                Ok(false) => (),
                Ok(true) => {
                    //append the unix time onto the file name.
                    let timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .expect("couldn't get system time while creating falsepos backup: {e}")
                        .as_secs();

                    let backup_filename =
                        confirmed_path.with_file_name(format!("confirmed.{timestamp}.json"));

                    std::fs::copy(confirmed_path, backup_filename).unwrap();
                }
            }
        }

        //write the confirmed entries to disk.
        {
            let confirmed_path = Self::confirmed_db_path(&self.db_path);

            let err_msg = format!(
                "Unable to write confirmed database to {}",
                confirmed_path.display()
            );

            let mut f = std::fs::File::create(confirmed_path).expect(&err_msg);
            let w = std::io::BufWriter::new(&f);
            let data = self
                .all_confirmed()
                .map(|x| x.collect::<Vec<_>>())
                .collect::<Vec<Vec<_>>>();

            serde_json::to_writer_pretty(w, &data).expect(&err_msg);
            f.flush().expect(&err_msg);
        }
        //if the database already exists, then create a backup
        {
            let falsepos_db_path = Self::falsepos_db_path(&self.db_path);
            match file_exists(&falsepos_db_path) {
                Err(e) => panic!("Error while creating falsepos backup file: {e}"),
                Ok(false) => (),
                Ok(true) => {
                    //append the unix time onto the file name.
                    let timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .expect("couldn't get system time while creating falsepos backup: {e}")
                        .as_secs();

                    let backup_filename =
                        falsepos_db_path.with_file_name(format!("falsepos.{timestamp}.json"));

                    std::fs::copy(falsepos_db_path, backup_filename).unwrap();
                }
            }
        }

        //write the falsepos entries to disk.
        {
            let falsepos_db_path = Self::falsepos_db_path(&self.db_path);
            let err_msg = format!(
                "Unable to write falsepos database to {}",
                falsepos_db_path.display()
            );

            let mut f = std::fs::File::create(falsepos_db_path).expect(&err_msg);
            let w = std::io::BufWriter::new(&f);
            let data = self.all_falsepos().collect::<Vec<_>>();
            serde_json::to_writer_pretty(w, &data).expect(&err_msg);
            f.flush().expect(&err_msg);
        }
    }

    pub fn from_disk(db_path: impl AsRef<Path>) -> Self {
        let db_path = db_path.as_ref();
        //read confirmed entries from disk
        let confirmed = {
            let confirmed_path = db_path.join("confirmed.json");
            let err_msg = format!(
                "Unable to read confirmed database from {}",
                confirmed_path.display()
            );

            let f = std::fs::File::open(confirmed_path).expect(&err_msg);
            let r = std::io::BufReader::new(f);

            let data: Vec<Vec<PathBuf>> = serde_json::from_reader(r).expect(&err_msg);

            let mut confirmed = MatchMap::default();
            for entry in data {
                for (p1, p2) in entry.into_iter().tuple_combinations() {
                    confirmed.insert(p1, p2);
                }
            }
            confirmed
        };

        let falsepos = {
            let falsepos_path = db_path.join("falsepos.json");
            let err_msg = format!(
                "Unable to read falsepos database from {}",
                falsepos_path.display()
            );

            let f = std::fs::File::open(falsepos_path).expect(&err_msg);
            let r = std::io::BufReader::new(f);
            let data: Vec<[PathBuf; 2]> = serde_json::from_reader(r).expect(&err_msg);
            let mut falsepos = FalseposMap::default();
            for entry in data {
                falsepos.insert(&entry);
            }
            falsepos
        };

        debug_assert!(confirmed.all_entries().all(|e| e.count() >= 2));

        Self {
            db_path: db_path.to_path_buf(),
            confirmed,
            falsepos,
        }
    }

    //pub fn from_disk(db_path: impl AsRef<Path>) -> Self {}

    pub fn is_confirmed(&self, p1: impl AsRef<Path>, p2: impl AsRef<Path>) -> bool {
        match self.confirmed.get_entries(p1) {
            Some(entries) => entries.contains(p2.as_ref()),
            None => false,
        }
    }

    pub fn is_falsepos(&self, p1: impl AsRef<Path>, p2: impl AsRef<Path>) -> bool {
        match self.falsepos.get_entries(p1) {
            Some(mut entries) => entries.contains(&p2.as_ref()),
            None => false,
        }
    }

    pub fn all_confirmed(
        &self,
    ) -> impl Iterator<Item = impl Iterator<Item = &Path> + Clone> + Clone {
        self.confirmed.all_entries()
    }

    pub fn all_falsepos(&self) -> impl Iterator<Item = [&Path; 2]> {
        self.falsepos.all_entries()
    }

    fn process_raw_db(raw_db_path: impl AsRef<Path>) -> Vec<MatchDbResult<(bool, Vec<PathBuf>)>> {
        #[derive(Deserialize)]
        struct IdxAndMatchSet {
            pub _idx: u64,
            pub matchset: Vec<PathBuf>,
        }

        //deserialize the idx file
        let raw_db_path = raw_db_path.as_ref();
        let idx_file_path = raw_db_path.join("idx.json");

        let f = match std::fs::File::open(&idx_file_path) {
            Ok(f) => BufReader::new(f),
            Err(e) => return vec![Err(MatchDbError::IdxReadIoError(idx_file_path, e))],
        };

        let index: Vec<IdxAndMatchSet> = match serde_json::from_reader(f) {
            Ok(x) => x,
            Err(e) => return vec![Err(MatchDbError::IdxDeserialize(idx_file_path, e))],
        };

        let process_path = |p: &Path| -> MatchDbResult<Vec<Vec<PathBuf>>> {
            walkdir::WalkDir::new(p)
                .into_iter()
                .filter(|entry| match entry {
                    Ok(entry) => entry.file_type().is_file(),
                    Err(_) => true,
                })
                .map(|entry| {
                    let entry = match entry {
                        Err(e) => {
                            return Err(MatchDbError::ConfirmedDirIoError(p.to_owned(), e.into()))
                        }
                        Ok(entry) => entry,
                    };

                    let filename = entry.path();
                    let number = match filename.file_stem() {
                        None => {
                            return Err(MatchDbError::IndexProcError(
                                filename.to_owned(),
                                idx_file_path.clone(),
                            ))
                        }
                        Some(number) => number,
                    };

                    let number = match number.to_string_lossy().parse::<usize>() {
                        Err(_e) => {
                            return Err(MatchDbError::IndexProcError(
                                filename.to_owned(),
                                idx_file_path.clone(),
                            ))
                        }
                        Ok(number) => number,
                    };

                    let idx_entry: Vec<PathBuf> = match index.get(number) {
                        None => {
                            return Err(MatchDbError::IndexRangeError(
                                number,
                                index.len() - 1,
                                idx_file_path.clone(),
                            ))
                        }
                        Some(entry) => entry.matchset.clone(),
                    };

                    if idx_entry.iter().any(|path| {
                        let p = path.to_string_lossy();
                        p.contains("qqqqqqqqqqqqqqqqqqqqqqqq")
                    }) {
                        todo!() //assert!(true)
                    }

                    Ok(idx_entry)
                })
                .collect()
        };

        //process confirmed items
        let confirmed_path = raw_db_path.join("confirmed");
        let confirmed_path_entries = match process_path(&confirmed_path) {
            Err(e) => return vec![Err(e)],
            Ok(x) => x,
        };

        let falsepos_path = raw_db_path.join("falsepos");
        let falsepos_path_entries = match process_path(&falsepos_path) {
            Err(e) => return vec![Err(e)],
            Ok(x) => x,
        };

        confirmed_path_entries
            .into_iter()
            .map(|x| Ok((true, x)))
            .chain(falsepos_path_entries.into_iter().map(|x| Ok((false, x))))
            .collect()
    }

    pub fn remove_deleted_items(&mut self) {
        self.confirmed.remove_deleted_items().unwrap();
        self.falsepos.remove_deleted_items().unwrap();
    }
}

fn file_exists(path: impl AsRef<Path>) -> Result<bool, std::io::Error> {
    match std::fs::metadata(path) {
        Ok(metadata) => {
            let acceptable = metadata.is_file() || metadata.is_symlink();
            Ok(acceptable)
        }
        Err(e) => match e.kind() {
            std::io::ErrorKind::NotFound => Ok(false),
            _ => Err(e),
        },
    }
}
