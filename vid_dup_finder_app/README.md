# Video Duplicate Finder
Video Duplicate Finder is a command-line program (and linux-only GUI) to search for duplicate and near-duplicate video files. It is capable of detecting duplicates even when the videos have been:
 * Resized (including changes of aspect ratio)
 * Watermarked
 * Letterboxed
 

Video duplicate finder contains:
* A command line program for listing unique/dupliacte files in a filesystem.
* An optional linux-only GUI (written in GTK) to allow users to examine duplicates and mark them for deletion


## How it works
Video Duplicate finder extracts several frames from the first minute of each video. It creates a "perceptual hash" from these frames using 'Spatial' and 'Temporal' information from those frames:
* The spatial component describes the parts of each frame that are bright and dark. It is generated using the pHash algorithm described in [here](http://hackerfactor.com/blog/index.php%3F/archives/432-Looks-Like-It.html)
* The temporal component describes the parts of each frame that are brighter/darker than the previous frame. (It is calculated directly from the bits of the spatial hash)

The resulting hashes can then be compared according to their hamming distance. Shorter distances represent similar videos.
 

## Requirements
Ffmpeg must be installed on your system and be accessible on the command line.

* Debian-based systems: # apt-get install ffmpeg
* Yum-based systems:    # yum install ffmpeg
* Windows:
    1) Download the correct installer from <https://ffmpeg.org/download.html>
    2) Run the installer and install ffmpeg to any directory
    3) Add the directory into the PATH environment variable

## Examples
To find all duplicate videos in directory "dog_vids":
* vid_dup_finder --files dog_vids

To find all videos which are not duplicates in "dog_vids":
* vid_dup_finder --files dog_vids --search-unique

To find videos in "dog_vids" that have accidentally been replicated into "cat_vids"
* vid_dup_finder --files cat_vids --with-refs dog_vids

To exclude a file or directory from a search, e.g "dog_vids/beagles"
* vid_dup_finder --files dog_vids --exclude dog_vids/beagles

To run the gui to examine duplicates:
* vid_dup_finder --files dog_vids --gui



## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

