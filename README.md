# Video Duplicate Finder
vid_dup_finder finds near-duplicate video files on disk. It detects videos whose
  frames look similar, and where the videos are roughly the same length (within
  ~5%). 

  vid_dup_finder will work with most common video file formats (any format 
  supported by FFMPEG.)
 
## How it works
Video Duplicate finder extracts several frames from the first minute of each video. It creates a "perceptual hash" from these frames using 'Spatial' and 'Temporal' information from those frames:
* The spatial component describes the parts of each frame that are bright and dark. It is generated using the pHash algorithm described in [here](http://hackerfactor.com/blog/index.php%3F/archives/432-Looks-Like-It.html)
* The temporal component describes the parts of each frame that are brighter/darker than the previous frame. (It is calculated directly from the bits of the spatial hash)

The resulting hashes can then be compared according to their hamming distance. Shorter distances represent similar videos.

## Requirements
Ffmpeg must be installed on your system and be accessible on the command line. You can do this by:
* Debian-based systems: # apt install ffmpeg
* Yum-based systems:    # yum install ffmpeg
* Windows:
    1) Download the correct installer from <https://ffmpeg.org/download.html>
    2) Run the installer and install ffmpeg to any directory
    3) Add the directory into the PATH environment variable

## Limitations
vid_dup_finder will find duplicates if minor changes have been made to the 
video, such as resizing, small colour corrections, small crops or faint 
watermarks. It will not find duplicates if there are larger changes (flipping or
rotation, embedding in a corner of a different video etc)

To save processing time when working on large datasets, vid_dup_finder uses only
frames from the first 30 seconds of any video. vid_dup_finder may return false
positives when used on content of the same length and and a common first-30-
seconds (for example a series of cartoons with a fixed into sequence)

## False Positives
Because this library only checks the first 30 seconds of each video, if two videos are the same
length and share the first 30 seconds of video content, they will be reported as a false match. This
may occur for TV shows which contain opening credits.

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

