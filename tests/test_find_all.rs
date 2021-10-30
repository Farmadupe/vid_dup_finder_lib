use itertools::Itertools;
use rand::prelude::*;
use vid_dup_finder_lib::*;

fn tol_from_raw(dist: u32) -> NormalizedTolerance {
    NormalizedTolerance::new(dist as f64 / (64.0 * 19.0))
}

/// A "start hash", and a set of hashes with a set absolute distance from the start hash.
/// because these hashes satisfy the triangle inequality, the following properties hold:
/// * ∀ member ∈ self.members: self.start_hash.distance(member) = self.distance_from_start;
/// * ∀ member1 ∈ self.members: ∀ member2 ∈ self.members: member1.distance(member2) <= self.distance_from_start * 2
#[derive(Clone, Debug)]
struct HashesWithDistance {
    pub start_hash: VideoHash,
    pub members: Vec<VideoHash>,
    pub _distance_from_start: u32,
}

impl HashesWithDistance {
    #[allow(dead_code)]
    ///Create a new set of hashes within the specified distance, The start hash is randomly chosen
    pub fn new(distance_from_start: u32, num_hashes: u32, rng: &mut StdRng) -> Self {
        //for now these tests only support full-length hashes.
        let num_frames = 10;

        let start_hash = VideoHash::random_hash_with_len(rng, num_frames);

        Self::with_start_hash(start_hash, distance_from_start, num_hashes, rng)
    }

    //crate a new set of hashes within the specified distance from the start_hash
    pub fn with_start_hash(start_hash: VideoHash, distance_from_start: u32, num_hashes: u32, rng: &mut StdRng) -> Self {
        let members = (0..num_hashes)
            .map(|_i| start_hash.hash_with_spatial_distance(distance_from_start, rng))
            .collect::<Vec<_>>();

        //sanity check: Make sure that the distance between the hashes never exceeds distance_from_start*2.
        for pair in members.iter().permutations(2) {
            let hash_1 = pair[0];
            let hash_2 = pair[1];
            let distance = hash_1.levenshtein_distance(hash_2).u32_value();
            assert!(distance <= distance_from_start * 2);
        }

        Self {
            start_hash,
            members,
            _distance_from_start: distance_from_start,
        }
    }

    pub fn members(&self, rng: &mut StdRng) -> Vec<VideoHash> {
        let mut ret = self.members.clone();
        ret.shuffle(rng);
        ret
    }
}

/// a collection of HashesWithDistance, where each member of the collection is guaranteed (hopefully) to be
///
struct HashesWithDistanceSet {
    groups: Vec<HashesWithDistance>,
}

impl HashesWithDistanceSet {
    pub fn new(
        num_groups: u32,
        mut hashes_per_group: u32,
        intergroup_distance: u32,
        intragroup_distance: u32,
        rng: &mut StdRng,
    ) -> Self {
        //for now these tests only support full length hashes.
        let num_frames = 10;

        // To uphold the properties of the struct, intergroup_distance must be greater than double
        // intragroup_distance
        assert!(
            intragroup_distance * 2 < intergroup_distance,
            "intergroup distance ({}) must be more than double intragroup distance ({})",
            intergroup_distance,
            intragroup_distance
        );

        // The algorithm for separating requires (maximum distance / num_groups) to be less than
        // intergroup_distance. Otherwise it will not be possible to maintain separation.
        assert!(
            ((19 * 64) / num_groups) > intergroup_distance,
            "Cannot fit num_groups ({}) HashesWithDistance with separation of intergroup_distance ({})",
            num_groups,
            intergroup_distance
        );

        //pick a random start hash
        let start_hash = VideoHash::random_hash_with_len(rng, num_frames);

        let mut current_group_distance = 0;
        let groups = (0..num_groups)
            .map(|_group_no| {
                let group_start_hash = start_hash.hash_with_spatial_distance(current_group_distance, rng);
                current_group_distance += intergroup_distance;
                let group_size = hashes_per_group;
                hashes_per_group += 10;
                HashesWithDistance::with_start_hash(group_start_hash, intragroup_distance, group_size, rng)
            })
            .collect::<Vec<_>>();

        Self { groups }
    }

    pub fn all_members(&self, rng: &mut StdRng) -> Vec<VideoHash> {
        let mut all_members_vec = self
            .groups
            .iter()
            .flat_map(|group| group.members(rng))
            .collect::<Vec<_>>();
        all_members_vec.shuffle(rng);
        all_members_vec
    }

    #[allow(dead_code)]
    pub fn groups(&self) -> Vec<&HashesWithDistance> {
        self.groups.iter().collect()
    }
}

#[test]
// Provide a group of hashes that should all match each other. Test that find_dups finds all of them.
fn test_find_dups_finds_a_known_group() {
    let mut rng = rand::rngs::StdRng::seed_from_u64(1);

    let intragroup_distance = 100;
    let intergroup_distance = (intragroup_distance * 2) + 1; //unused argument because there is only 1 group.
    let group_size = 50;

    //get a set of hashes clustered within a small distance.
    let groups = HashesWithDistanceSet::new(1, group_size, intergroup_distance, intragroup_distance, &mut rng);
    let members = groups.all_members(&mut rng);

    let dups = search(members, tol_from_raw(intragroup_distance * 2));

    assert!(
        dups.len() == 1,
        "expected match groups: 1, actual match_groups: {}",
        dups.len()
    );
    assert!(
        dups[0].len() == 50,
        "expected group size: 50, actual group size: {}",
        dups[0].len()
    );
}

#[test]
///Provide a group of hashes that have matchable hashes but differing durations (50 sec and 250 sec).
///Check that find_dups returns two groups.. One containing all 50 sec hashes, and the other containing
///all 250 sec hashes.
fn test_find_dups_discriminates_by_duration() {
    let mut rng = rand::rngs::StdRng::seed_from_u64(2);

    let intragroup_distance = 100;
    let intergroup_distance = (intragroup_distance * 2) + 1; //unused argument because the second group will be cloned from the first..
    let group_size = 100;

    let groups = HashesWithDistanceSet::new(1, group_size, intergroup_distance, intragroup_distance, &mut rng);
    let short_group = groups.groups[0]
        .members(&mut rng)
        .into_iter()
        .map(|h| h.with_duration(50))
        .collect::<Vec<_>>();

    let long_group = short_group
        .iter()
        .take(50)
        .map(|short_hash| short_hash.with_duration(250))
        .collect::<Vec<_>>();

    //shuffle the hashes to ensure that the library isn't just finding consecutive groups.
    let mut all_hashes = short_group;
    all_hashes.extend(long_group);

    all_hashes.shuffle(&mut rng);
    let mut dups = search(all_hashes, tol_from_raw(intragroup_distance * 2));

    //sort the dups by len -- so that if everything worked the first group is the one with the shorter videos.
    dups.sort_by_key(MatchGroup::len);

    assert!(
        dups.len() == 2,
        "Expected dup groups: 2 (one for short videos and one for long videos). Actual dup groups: {}",
        dups.len()
    );

    //debug: filter out any long_* videos in the short group and print them.
    let short_group = &dups[1];
    assert!(
        short_group.len() == 100,
        "Expected short matchgroup size: {}. Actual short matchgroup size: {}",
        100,
        short_group.len()
    );

    //debug: filter out any short_* videos in the long group and print them.
    let long_group = &dups[0];
    assert!(
        long_group.len() == 50,
        "Expected long matchgroup size: {}. Actual long matchgroup size: {}",
        50,
        long_group.len()
    );
}

#[test]
///Provide two sets of matchable groups, where both groups have the same duration.
///Test that find_dups separates out the two separate groups correctly.
fn test_find_dups_discriminates_by_distance() {
    let mut rng = rand::rngs::StdRng::seed_from_u64(3);

    let intragroup_distance = 50;
    let intergroup_distance = 150;
    let group_size = 100;

    let hash_groups = HashesWithDistanceSet::new(2, group_size, intergroup_distance, intragroup_distance, &mut rng);

    let all_hashes = hash_groups.all_members(&mut rng);
    let mut dups = search(all_hashes, tol_from_raw(intragroup_distance * 2));
    dups.sort_by_key(MatchGroup::len);

    assert!(dups.len() == 2);
    assert!(dups[0].len() == 100);
    assert!(dups[1].len() == 110);
}

#[test]
fn test_find_with_refs() {
    let mut rng = rand::rngs::StdRng::seed_from_u64(4);

    let intragroup_distance = 50;
    let intergroup_distance = 150;
    let group_size = 100;
    let num_groups = 5;

    let hash_groups = HashesWithDistanceSet::new(
        num_groups,
        group_size,
        intergroup_distance,
        intragroup_distance,
        &mut rng,
    );

    let start_hash = &hash_groups.groups()[3].start_hash;

    let cand_hashes = hash_groups.all_members(&mut rng);
    assert_eq!(cand_hashes.len(), 100 + 110 + 120 + 130 + 140);
    let dups = search_with_references(
        [start_hash.clone()],
        cand_hashes.clone(),
        tol_from_raw(intragroup_distance),
    );

    assert_eq!(dups.len(), 1);
    assert_eq!(dups[0].len(), 130);

    //now try with two references. Check there are two groups.
    let start_hashes = [
        hash_groups.groups()[0].start_hash.clone(),
        hash_groups.groups()[4].start_hash.clone(),
    ];
    let dups2 = search_with_references(start_hashes, cand_hashes, tol_from_raw(intragroup_distance));
    assert_eq!(dups2.len(), 2);
    assert_eq!(dups2[0].len(), 100);
    assert_eq!(dups2[1].len(), 140);
}
