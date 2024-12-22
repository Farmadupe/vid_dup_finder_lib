use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    default::Default,
    io::{BufReader, Write},
    path::{Path, PathBuf},
};

use itertools::Itertools;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use vid_dup_finder_lib::MatchGroup;

use crate::video_hash_filesystem_cache::file_hash_filesystem_cache::{
    FileContentCache, FileContentCacheErrorKind,
};

use super::disjoint_set::DisjointSet;

#[derive(Error, Debug)]
pub enum MatchDbError {
    #[error("IO error while reading raw DB file: {0}")]
    IdxReadIoError(PathBuf, #[source] std::io::Error),

    #[error("JSON deserialization error while reading raw DB file: {0}")]
    IdxDeserialize(PathBuf, #[source] serde_json::Error),

    #[error("Could not read confirmed file at location: {0}")]
    ConfirmedFileMissing(PathBuf),

    #[error("Could not read confirmed file at location: {0}")]
    FalseposFileMissing(PathBuf),

    #[error("IO error while reading confirmed items: {0}")]
    ConfirmedDirIoError(PathBuf, #[source] std::io::Error),

    #[error("Could not extract valid match number: filename: {0}. Index file: {1}")]
    IndexProcError(PathBuf, PathBuf),

    #[error("Match number is out of range: number {0}, max: {1}, Index file: {2}")]
    IndexRangeError(usize, usize, PathBuf),

    #[error("MatchDb failed to process new raw item at path: path {0}, error: {1}")]
    FileContentCacheError(PathBuf, #[source] FileContentCacheErrorKind),

    #[error("File Content cache error: path ??, error: {0}")]
    FileContentCacheErrorNoPath(#[from] FileContentCacheErrorKind),

    #[error("Unable to read confirmed entries file at location: {0}")]
    ConfirmedFileDeserializeError(PathBuf),

    #[error("Unable to read confirmed entries file at location: {0}")]
    FalseposFileDeserializeError(PathBuf),
}

pub type MatchDbResult<T> = Result<T, MatchDbError>;
pub type ContentHash = [u8; 32];

#[derive(Debug, Default, Clone, Serialize, Deserialize, Ord, PartialEq, PartialOrd, Eq, Hash)]
pub struct MatchMapEntry {
    pub path: PathBuf,
    pub content_hash: ContentHash,
}

//Maps filesystem paths into other paths (those which contain duplicates)
//
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct MatchMap {
    map: DisjointSet<PathBuf>,
    file_hashes: HashMap<PathBuf, ContentHash>,
}

impl MatchMap {
    pub fn insert(&mut self, e1: MatchMapEntry, e2: MatchMapEntry) {
        //for now, calculate both hashes.
        self.map.insert(e1.path.clone(), e2.path.clone());
        self.file_hashes.insert(e1.path, e1.content_hash);
        self.file_hashes.insert(e2.path, e2.content_hash);
    }

    pub fn all_groups(&self) -> impl Iterator<Item = MatchGroup> {
        let ret = self
            .map
            .all_sets()
            .filter_map(|it| {
                let paths = it.map(|p| p.to_path_buf());
                MatchGroup::new(paths).ok()
            })
            .collect::<Vec<_>>();

        ret.into_iter()
    }

    // pub fn is_confirmed(&self, p1: &MatchMapEntry, p2: &MatchMapEntry) -> bool {
    //     self.0.contains_pair(p1, p2)
    // }

    pub fn is_confirmed(&self, p1: impl AsRef<Path>, p2: impl AsRef<Path>) -> bool {
        let p1 = p1.as_ref();
        let p2 = p2.as_ref();
        self.map.contains_pair(p1, p2)
    }

    #[allow(dead_code)]
    pub fn remove_path(&mut self, p: impl AsRef<Path>) {
        let p = p.as_ref();

        // println!("removing {p:?}");

        //Check that the path actually exists in a group
        self.map.remove_item(p);
        let _ = self.file_hashes.remove(p);
    }

    //iterates through every entry, and checks that each file inside actually
    //exists on disk. If not, then removes the entry
    pub fn remove_deleted_items(&mut self) {
        //for each path in the list of entries, remove it.
        let paths_deleted_from_fs = self
            .map
            .all_items()
            .filter(|e| !e.exists())
            .cloned()
            .unique()
            .collect::<Vec<_>>();

        for entry in paths_deleted_from_fs {
            // dbg!(&entry.path);
            self.map.remove_item(&entry);
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct FalseposMap {
    map: BTreeMap<PathBuf, BTreeSet<PathBuf>>,
}

impl FalseposMap {
    pub fn insert<I: Iterator<Item = impl Clone + AsRef<Path>>>(&mut self, filenames: I)
    where
        <I as IntoIterator>::IntoIter: Clone,
    {
        //When given a set of false positives, each entry is a falsepos with each other entry.
        //This can be got from tuple_combinations. For each combinations, we have to register the
        //falsepos from p2->p1, and also from p1->p2.
        let all_falsepos_pairs = filenames
            .into_iter()
            .tuple_combinations::<(_, _)>()
            .flat_map(|(p1, p2)| [(p1.clone(), p2.clone()), (p2, p1)]);

        for (p1, p2) in all_falsepos_pairs {
            //check if we have to add brand new entries, or if one already exists.
            match self.map.get_mut(p1.as_ref()) {
                None => {
                    let mut falsepos_set = BTreeSet::new();
                    falsepos_set.insert(p2.as_ref().to_path_buf());
                    assert!(self
                        .map
                        .insert(p1.as_ref().to_path_buf(), falsepos_set)
                        .is_none());
                }
                Some(falsepos_set) => {
                    falsepos_set.insert(p2.as_ref().to_path_buf());
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

    pub fn remove_deleted_items(&mut self) {
        //temporary -- somehow there are single length entries.. so remove them now.
        self.map.retain(|_path, entry| entry.len() >= 2);
        assert!(self.map.values().all(|entry| entry.len() >= 2));

        let all_paths = self.map.keys().cloned().collect::<Vec<_>>();

        //for each path in the list of entries, remove it.
        let paths_deleted_from_fs = all_paths
            .iter()
            .map(PathBuf::as_path)
            .filter(|p| !p.exists())
            .flatten();

        for path in paths_deleted_from_fs {
            self.remove_path(path);
        }
        assert!(self.map.values().all(|entry| entry.len() >= 2));
    }
}

pub struct MatchDb {
    pub content_cache: FileContentCache,
    db_path: PathBuf,
    confirmed: MatchMap,
    falsepos: FalseposMap,
}

impl MatchDb {
    pub fn exists_on_disk(db_path: impl AsRef<Path>) -> bool {
        let db_path = db_path.as_ref();

        Self::confirmed_db_path(db_path).exists() && Self::falsepos_db_path(db_path).exists()
    }

    fn confirmed_db_path(db_path: impl AsRef<Path>) -> PathBuf {
        db_path.as_ref().join("confirmed.bin")
    }

    fn falsepos_db_path(db_path: impl AsRef<Path>) -> PathBuf {
        db_path.as_ref().join("falsepos.bin")
    }

    pub fn content_cache_path(db_path: impl AsRef<Path>) -> PathBuf {
        db_path.as_ref().join("content_cache.bin")
    }

    pub fn raw_data_path(db_path: impl AsRef<Path>) -> PathBuf {
        db_path.as_ref().join("../manual_inputs")
    }

    pub fn new(db_path: impl AsRef<Path>) -> Self {
        Self {
            content_cache: FileContentCache::new(200, Self::content_cache_path(&db_path)).unwrap(),
            db_path: db_path.as_ref().to_owned(),

            confirmed: MatchMap::default(),
            falsepos: FalseposMap::default(),
        }
    }

    pub fn confirmed_and_falsepos_entries(&self) -> impl Iterator<Item = (&PathBuf, &PathBuf)> {
        //collect all pairs of videos that are both confirmed and falsepos.
        // let all_entries = self.all_confirmed().flat_map(|confirmed_files| {
        //     confirmed_files
        //         .combinations(2)
        //         .map(|combination| (combination[0], combination[1]))
        // });

        // all_entries.filter(|(p1, p2)| self.is_falsepos(p1, p2))

        todo!();
        #[allow(unreachable_code)]
        std::iter::empty()
    }

    pub fn is_confirmed(&self, p1: impl AsRef<Path>, p2: impl AsRef<Path>) -> bool {
        self.confirmed.is_confirmed(p1, p2)
    }

    pub fn all_confirmed(
        &self,
        paths: impl IntoIterator<Item = impl AsRef<Path>>,
        cand_path: impl AsRef<Path>,
    ) -> bool {
        paths
            .into_iter()
            .all(|group_path| self.is_confirmed(group_path, &cand_path))
    }

    pub fn is_falsepos(&self, p1: impl AsRef<Path>, p2: impl AsRef<Path>) -> bool {
        let p1_falsepos = match self.falsepos.get_entries(&p1) {
            Some(mut entries) => entries.contains(&p2.as_ref()),
            None => false,
        };

        let p2_falsepos = match self.falsepos.get_entries(&p2) {
            Some(mut entries) => entries.contains(&p1.as_ref()),
            None => false,
        };

        p1_falsepos || p2_falsepos
    }

    pub fn confirmed_groups(&self) -> impl Iterator<Item = MatchGroup> {
        self.confirmed.all_groups()
    }

    pub fn falsepos_groups(&self) -> impl Iterator<Item = MatchGroup> {
        let ret = self
            .falsepos
            .all_entries()
            .filter_map(|e| MatchGroup::new(e.iter().map(|x| x.to_path_buf())).ok())
            .collect::<Vec<_>>();
        ret.into_iter()
    }

    pub(super) fn all_falsepos_entries(&self) -> Vec<[&std::path::Path; 2]> {
        self.falsepos.all_entries().collect::<Vec<_>>()
    }

    pub fn remove_deleted_items(&mut self) {
        self.confirmed.remove_deleted_items();
        self.falsepos.remove_deleted_items();
        for path in self.content_cache.all_cached_paths() {
            if let Ok(false) = path.try_exists() {
                self.content_cache.force_update(path).unwrap();
            }
        }
        self.content_cache.save().unwrap();
    }

    pub fn insert_confirmed_pair(&mut self, e1: MatchMapEntry, e2: MatchMapEntry) {
        self.confirmed.insert(e1, e2)
    }

    pub fn _remove_path(&mut self, p: impl AsRef<Path>) {
        self.confirmed.remove_path(p);
    }

    ////////////////////////////////////////////////////////////////////////////////
    // Serialization/Deserialization
    ////////////////////////////////////////////////////////////////////////////////

    pub fn to_disk(&self) {
        //first make sure the match db directory can be created.
        std::fs::create_dir_all(&self.db_path).expect("Unable to create match database directory");

        //create a backup of the db if it already exists
        {
            let confirmed_path = Self::confirmed_db_path(&self.db_path);
            // dbg!(&confirmed_path);
            if confirmed_path.exists() {
                //append the unix time onto the file name.
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("couldn't get system time while creating falsepos backup: {e}")
                    .as_secs();

                let backup_filename =
                    confirmed_path.with_file_name(format!("confirmed.{timestamp}.bak.bin"));

                std::fs::copy(confirmed_path, backup_filename).unwrap();
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
                .confirmed_groups()
                .map(|x| {
                    x.contained_paths()
                        .map(|x| x.to_path_buf())
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<Vec<_>>>();

            //serde_json::to_writer_pretty(w, &data).expect(&err_msg);
            bincode::serialize_into(w, &data).unwrap();

            f.flush().expect(&err_msg);
        }

        //write the confirmed entries to disk.
        {
            let confirmed_path = Self::confirmed_db_path(&self.db_path).with_extension("json");

            let err_msg = format!(
                "Unable to write confirmed database to {}",
                confirmed_path.display()
            );

            let mut f = std::fs::File::create(confirmed_path).expect(&err_msg);
            let w = std::io::BufWriter::new(&f);
            let data = self
                .confirmed_groups()
                .map(|group| {
                    group
                        .contained_paths()
                        .map(|x| x.to_path_buf())
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<Vec<_>>>();

            //serde_json::to_writer_pretty(w, &data).expect(&err_msg);
            serde_json::to_writer_pretty(w, &data).unwrap();

            f.flush().expect(&err_msg);
        }

        //if the database already exists, then create a backup
        {
            let falsepos_db_path = Self::falsepos_db_path(&self.db_path);
            if falsepos_db_path.exists() {
                //append the unix time onto the file name.
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("couldn't get system time while creating falsepos backup: {e}")
                    .as_secs();

                let backup_filename =
                    falsepos_db_path.with_file_name(format!("falsepos.{timestamp}.bak.bin"));

                std::fs::copy(falsepos_db_path, backup_filename).unwrap();
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
            let data = self.all_falsepos_entries();
            // serde_json::to_writer_pretty(w, &data).expect(&err_msg);
            bincode::serialize_into(w, &data).unwrap();
            f.flush().expect(&err_msg);
        }
    }

    pub fn from_disk(db_path: impl AsRef<Path>) -> MatchDbResult<Self> {
        let db_path = db_path.as_ref();

        let content_cache = FileContentCache::new(200, Self::content_cache_path(db_path)).unwrap();

        //read confirmed entries from disk
        let confirmed = {
            let confirmed_path = db_path.join("confirmed.bin");

            let f = std::fs::File::open(&confirmed_path)
                .map_err(|_e| MatchDbError::ConfirmedFileMissing(confirmed_path.clone()))?;
            let r = std::io::BufReader::new(f);

            let data: Vec<Vec<MatchMapEntry>> = bincode::deserialize_from(r).map_err(|_e| {
                MatchDbError::ConfirmedFileDeserializeError(confirmed_path.clone())
            })?;

            let mut confirmed = MatchMap::default();
            for entry in data {
                for (p1, p2) in entry.into_iter().tuple_combinations() {
                    confirmed.insert(p1, p2);
                }
            }
            confirmed
        };

        let falsepos = {
            let falsepos_path = db_path.join("falsepos.bin");

            let f = std::fs::File::open(&falsepos_path)
                .map_err(|_e| MatchDbError::FalseposFileMissing(falsepos_path.clone()))?;
            let r = std::io::BufReader::new(f);
            let data: Vec<[PathBuf; 2]> = bincode::deserialize_from(r)
                .map_err(|_e| MatchDbError::FalseposFileDeserializeError(falsepos_path.clone()))?;
            let mut falsepos = FalseposMap::default();
            for entry in data {
                falsepos.insert(entry.iter());
            }
            falsepos
        };

        // assert!(confirmed.all_sets().all(|e| e.count() >= 2));

        // dbg!(&falsepos);

        let ret = Self {
            content_cache,
            db_path: db_path.to_path_buf(),
            confirmed,
            falsepos,
        };

        Ok(ret)
    }

    ////////////////////////////////////////////////////////////////////////////////
    // Inputting new confirmed/falsepos files
    ////////////////////////////////////////////////////////////////////////////////

    fn create_match_map_entry(&self, p: PathBuf) -> Result<MatchMapEntry, MatchDbError> {
        match self.content_cache.fetch(&p) {
            Err(e) => Err(MatchDbError::FileContentCacheError(p, e)),
            Ok(hash) => Ok(MatchMapEntry {
                path: p,
                content_hash: *hash.as_bytes(),
            }),
        }
    }

    pub fn load_new_inputs(&mut self) -> Result<(), MatchDbError> {
        let raw_db_path = Self::raw_data_path(&self.db_path);
        let idx_file_path = raw_db_path.join("idx.json");
        let confirmed_path = raw_db_path.join("confirmed");
        let falsepos_path = raw_db_path.join("falsepos");
        let unmatch_path = raw_db_path.join("unmatch");

        //if there are actually no paths, then quietly return
        //dbg!(idx_file_path);
        if !idx_file_path.exists() {
            let msg = format!("no matchdb raw paths actually found at {:?}", idx_file_path);
            warn!("{}", msg);
            return Ok(());
        }

        //process confirmed items

        let confirmed_path_entries = Self::load_raw_from_disk(&idx_file_path, &confirmed_path)?;
        let falsepos_path_entries = Self::load_raw_from_disk(&idx_file_path, &falsepos_path)?;
        let unmatch_path_entries = Self::load_raw_from_disk(&idx_file_path, &unmatch_path)?;

        for entry in confirmed_path_entries {
            self.load_one(&(true, entry))?;
        }

        for entry in falsepos_path_entries {
            self.load_one(&(false, entry))?;
        }

        for group in unmatch_path_entries {
            let all_pairs_to_unmatch = group
                .iter()
                .tuple_combinations::<(_, _)>()
                .collect::<Vec<_>>();

            let mut new_match_map = MatchMap::default();
            for group in self.confirmed.all_groups() {
                if !all_pairs_to_unmatch
                    .iter()
                    .any(|(unmatch_path_1, unmatch_path_2)| {
                        let unmatch_path_1_found = group
                            .contained_paths()
                            .any(|p| p == unmatch_path_1.as_path());
                        let unmatch_path_2_found = group
                            .contained_paths()
                            .any(|p| p == unmatch_path_2.as_path());

                        unmatch_path_1_found && unmatch_path_2_found
                    })
                {
                    let paths = group.contained_paths().collect::<Vec<_>>();
                    for (e1, e2) in paths.iter().tuple_combinations::<(_, _)>() {
                        let e1 = self.create_match_map_entry(e1.to_path_buf())?;
                        let e2 = self.create_match_map_entry(e2.to_path_buf())?;
                        new_match_map.insert(e1, e2);
                    }
                }
            }
            self.confirmed = new_match_map;
        }

        Ok(())
    }

    fn load_one(
        &mut self,

        (is_confirmed, paths): &(bool, Vec<PathBuf>),
    ) -> Result<(), MatchDbError> {
        if *is_confirmed {
            for (p1, p2) in paths.iter().tuple_combinations() {
                let p1_entry = self.create_match_map_entry(p1.clone())?;
                let p2_entry = self.create_match_map_entry(p2.clone())?;

                self.confirmed.insert(p1_entry, p2_entry);
            }
        } else {
            self.falsepos.insert(paths.iter());
        }

        Ok(())
    }

    fn load_raw_from_disk(
        idx_file_path: &Path,
        num_files_path: &Path,
    ) -> MatchDbResult<Vec<Vec<PathBuf>>> {
        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct IdxAndMatchSet {
            pub idx: u64,
            pub matchset: Vec<PathBuf>,
        }

        let f = match std::fs::File::open(idx_file_path) {
            Ok(f) => BufReader::new(f),
            Err(e) => return Err(MatchDbError::IdxReadIoError(idx_file_path.to_path_buf(), e)),
        };

        let mapping: Vec<IdxAndMatchSet> = match serde_json::from_reader(f) {
            Ok(x) => x,
            Err(e) => return Err(MatchDbError::IdxDeserialize(idx_file_path.to_path_buf(), e)),
        };

        let get_file_nums_from_directory = |p: &Path| -> MatchDbResult<Vec<u64>> {
            let get_num_from_file_name = |p: &Path| -> MatchDbResult<u64> {
                let err =
                    || MatchDbError::IndexProcError(p.to_path_buf(), idx_file_path.to_path_buf());

                let stem = p.file_stem().ok_or_else(err)?;

                stem.to_string_lossy().parse::<u64>().ok().ok_or_else(err)
            };

            let all_entries = walkdir::WalkDir::new(p).into_iter().map(|entry| {
                entry.map_err(|e| MatchDbError::ConfirmedDirIoError(p.to_owned(), e.into()))
            });

            let all_files = all_entries.filter(|e| match e {
                Ok(dir_entry) => dir_entry.file_type().is_file(),
                Err(_) => true,
            });

            let file_nums = all_files.map(|e| e.and_then(|e| get_num_from_file_name(e.path())));

            file_nums.collect()
        };

        //given a path (of files representing either confirmed or falsepos indices)
        //read it from disk
        let process_path = |p: &Path| -> MatchDbResult<Vec<Vec<PathBuf>>> {
            let file_nums = get_file_nums_from_directory(p)?;

            let files_of_index = file_nums
                .into_iter()
                .map(|num| match mapping.get(num as usize) {
                    None => Err(MatchDbError::IndexRangeError(
                        num as usize,
                        mapping.len() - 1,
                        idx_file_path.to_path_buf(),
                    )),
                    Some(entry) => Ok(entry.matchset.clone()),
                })
                .collect();

            files_of_index
        };

        process_path(num_files_path)
    }

    pub fn update_file_content_cache<T>(
        &mut self,
        paths: T,
    ) -> Result<Vec<MatchDbError>, MatchDbError>
    where
        T: IntoIterator<Item = PathBuf>,
        <T as IntoIterator>::IntoIter: Send,
    {
        let loading_errs = self
            .content_cache
            .update_using_fs(paths, false)
            .map_err(MatchDbError::FileContentCacheErrorNoPath)?
            .into_iter()
            .map(MatchDbError::from)
            .collect();
        self.content_cache.save()?;
        Ok(loading_errs)
    }

    pub fn fix_moved_files(&mut self) -> Result<(), MatchDbError> {
        self.remove_deleted_items();

        let mut all_db_entries = BTreeSet::new();
        for group in self.confirmed_groups() {
            for path in group.contained_paths() {
                all_db_entries.insert(path.to_path_buf());
            }
        }

        let all_content_cache_entries = self
            .content_cache
            .all_cached_paths()
            .into_iter()
            .collect::<BTreeSet<_>>();

        let unmatched_entries = all_content_cache_entries.difference(&all_db_entries);

        for unmatched_entry in unmatched_entries {
            //we need to ignore entries which aren't in the content cache
            let Ok(unmatched_hash) = self.content_cache.fetch(unmatched_entry) else {
                warn!("item missing from content cache: {unmatched_entry:?}");
                continue;
            };

            let confirmed_groups = self.confirmed_groups().collect::<Vec<_>>();
            for group in confirmed_groups {
                if let Some(matching_entry) = group.contained_paths().find(|p| {
                    let Ok(cand_hash) = self.content_cache.fetch(p) else {
                        warn!("item missing from content cache: {p:?}");
                        return false;
                    };

                    cand_hash == unmatched_hash
                }) {
                    let new_entry = self.create_match_map_entry(unmatched_entry.clone())?;
                    let matching_entry =
                        self.create_match_map_entry(matching_entry.to_path_buf())?;

                    info!(
                        "Adding identical file to matchdb: {:?}, {:?}",
                        &new_entry.path, &matching_entry,
                    );
                    self.insert_confirmed_pair(new_entry, matching_entry)
                }
            }
        }

        #[cfg(feature = "print_timings")]
        println!(
            "unmatched fix time: {}",
            unmatched_fix_start.elapsed().as_secs_f64()
        );
        Ok(())
    }
}

// #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
// struct EntryNotInMatchDb {}

// trait MatchGroupMatchDbExt {
//     fn all_paths_match(&self, p: impl AsRef<Path>, db: &MatchDb)
//         -> Result<bool, EntryNotInMatchDb>;
// }

// impl MatchGroupMatchDbExt for MatchGroup {
//     fn is_confirmed(
//         &self,
//         p: impl AsRef<Path>,
//         db: &MatchDb,
//     ) -> Result<bool, EntryNotInMatchDb> {
//         let p = p.as_ref();

//         self.contained_paths().all(|group_path| group_)
//     }
// }
