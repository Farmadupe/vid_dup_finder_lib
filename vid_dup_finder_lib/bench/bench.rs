use vid_dup_finder_lib::*;

/// Example usage of Vid Dup Finder (without caching)
/// In directory processed_vids there are three copies of a "Cat" video
/// and three copies of a "Dog" video. Use Vid Dup Finder to create hashes
/// from all of the videos and then use those hashes to find the copies.
pub fn main() {
    unimplemented!()
    //inner()
}

#[test]
fn test() {
    inner()
}

fn inner() {
    let cat_vids = &[
        "examples/vids/cat.1.mp4",
        "examples/vids/cat.2.mp4",
        "examples/vids/cat.3.webm",
    ];
    let dog_vids = &[
        "examples/vids/dog.1.mp4",
        "examples/vids/dog.2.mp4",
        "examples/vids/dog.3.webm",
        "examples/vids/dog.watermark.mp4",
    ];

    //form an absolute path from the above list.
    let all_vids = cat_vids
        .iter()
        .chain(dog_vids.iter())
        .map(|vid| std::env::current_dir().unwrap().join(vid));

    // Get hashes from the videos. Hopefully there will be no errors
    // but if there are, print them to screen.
    let hashes: Vec<VideoHash> = all_vids
        .filter_map(|ref fname| {
            println!("Loading {}", fname.to_string_lossy());
            match VideoHash::from_path(fname) {
                Ok(hash) => Some(hash),
                Err(e) => {
                    println!(
                        "failed to create hash from {}. Error: {}.",
                        fname.to_string_lossy(),
                        e
                    );
                    panic!()
                }
            }
        })
        .collect();

    // Get a collection of duplicate groups, using the default search configuration. One should contain all dog vids. One should contain all cat vids.
    let tol = NormalizedTolerance::default();
    let dup_groups = search(hashes, tol);

    //Print what was found
    println!("found {} duplicate groups", dup_groups.len());
    for (i, dup_group) in dup_groups.iter().enumerate() {
        println!("\nGroup: {}, entries: {}", i, dup_group.len());
        for entry in dup_group.duplicates() {
            println!("    {}", entry.display())
        }
    }

    //some assertions to check that the example still works
    assert_eq!(dup_groups.len(), 2);
    assert_eq!(dup_groups[0].len(), 3);
    assert_eq!(dup_groups[1].len(), 3);
}
