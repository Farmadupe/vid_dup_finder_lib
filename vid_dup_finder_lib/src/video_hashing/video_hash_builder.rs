use std::path::{Path, PathBuf};

use ffmpeg_gst_wrapper::{get_duration, FrameReadCfg};
use image::GrayImage;
use vid_dup_finder_common::video_frames_gray::{
    cropdetect_letterbox, cropdetect_motion, cropdetect_none, VdfFrameExt,
};
use vid_dup_finder_common::Crop;

use crate::definitions::{DCT_SIZE, DEFAULT_VID_HASH_DURATION};
use crate::{Cropdetect, VideoHash, VideoHashResult, DEFAULT_VID_HASH_SKIP_FORWARD};

use crate::Error;

/// Options for how videos will be processed when generating hashes. Can be used
/// to ensure that starting credits are skipped.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct CreationOptions {
    /// The amount of time to skip past when before extracting video frames. Used to skip past
    /// title credits and/or overlays at the beginning of videos.
    /// Higher numbers extend hasing time (because seeking to this point in videos must be
    /// done accurately). Lower numbers risk not skipping far enough to avoid title credits etc.
    ///
    /// If any video is shorter than this duration, then the amount skipped will be reduced to ensure
    /// a hash can be generated.
    ///
    /// Unit: Seconds
    ///
    /// Reccomended range: 0-300.
    pub skip_forward_amount: f64,

    /// The amount time at the start of the video to generate hashes from.
    /// Lower values speed up the hashing process because less video data needs to be extracted.
    /// Higher values produce slightly more reliable hashes.
    ///
    /// If any video is shorter than this duration, then hashes will be generated from the entire video.
    ///
    /// Unit: Seconds
    ///
    /// reccomended range: 2-60.
    pub duration: f64,

    /// The time at the start of the video to generate hashes from.
    /// Lower values speed up the hashing process because less video data needs to be extracted.
    /// Higher values produce slightly more reliable hashes.
    ///
    /// If any video is shorter than this duration, then hashes will be generated from the entire video.
    ///
    /// Unit: Seconds
    ///
    /// reccomended range: 2-60.
    pub cropdetect: Cropdetect,
}

impl std::default::Default for CreationOptions {
    fn default() -> Self {
        Self {
            skip_forward_amount: DEFAULT_VID_HASH_SKIP_FORWARD,
            duration: DEFAULT_VID_HASH_DURATION,
            cropdetect: Cropdetect::Letterbox,
        }
    }
}

/// A factory for video hashes, using the gstreamer backend (This is backend is approximately 10% faster but is harder to integrate and is vulnerable to crashes in plugins and libglib etc)
///
/// Reccomend to always use the the default constructor [`gstreamer::VideoHashBuilder::default`] unless supplying custom options

#[derive(Default)]
pub struct VideoHashBuilder {
    options: CreationOptions,
}

impl VideoHashBuilder {
    /// Create a video hash builder with the selected [`CreationOptions`]
    pub fn from_options(options: CreationOptions) -> Self {
        Self { options }
    }

    pub fn hash(&self, src_path: PathBuf) -> VideoHashResult<VideoHash> {
        gen_hash(src_path, self.options)
    }
}

pub fn build_frame_reader(
    src_path: impl AsRef<Path>,
    opts: CreationOptions,
) -> Result<FrameReadCfg, Error>
where
{
    let src_path = src_path.as_ref();
    let mut builder = FrameReadCfg::from_path(src_path);

    let vid_duration = get_duration(src_path)
        .map_err(|_e| Error::NotVideo)?
        .as_secs_f64();

    //println!("duration: {vid_duration}");

    let max_seek_amount = opts.skip_forward_amount;
    let max_hash_duration = opts.duration;

    let fps;
    let seek_amount;

    // If the video is really short then set the FPS really high and
    // try get whatever frames are available. This might not succeed
    // because a really short video might not have 64 total frames,
    //
    // But don't sweat over this corner because for degernerately short videos
    // a duplicate-image utility might work just as well instead.
    if vid_duration < 2.0 {
        //println!("sub 2 sec");
        fps = 64.0;
        seek_amount = 0f64;

    //Otherwise if the video is shorter than the desired runtime for building
    //a hash, set the FPS to evenly sample frames across the length of the video
    //
    //(But to avoid the effect of cumulative rounding errors, try and make
    //the last frame be 2 seconds before the end. Otherwise sometimes we only
    //get 63 frames.)
    } else if vid_duration < max_hash_duration {
        //println!("sub {max_hash_duration} sec");
        fps = 64.0 / (vid_duration - 2.0);
        seek_amount = 0f64;

    //If the video is long enough to sample max_hash_duration's worth of content
    //to build the hash, but not long enough that we can apply the full skip,
    //then skip forwards as far as possible.
    } else if vid_duration < max_seek_amount + max_hash_duration {
        //println!("sub {} sec", max_seek_amount + max_hash_duration);

        fps = 64.0 / max_hash_duration;
        seek_amount = vid_duration - max_hash_duration - 2.0;

    //Otherwise the video is long enough to do what we want.
    } else {
        //println!("more than {} sec", max_seek_amount + max_hash_duration);
        fps = 64.0 / max_hash_duration;
        seek_amount = max_seek_amount;
    }

    //gstreamer expects framerates to be expressed as integer fractions, so
    //scale the float framerate by a large number and convert to integer.
    let fps = ((fps * 16384.0) as u64, 16384);

    //Spawn gstreamer pipeline to begin getting video frames.

    //println!("calculated fps for capturing: {fps:?}, seek_amount: {seek_amount}");
    builder.fps(fps);
    if seek_amount > 0f64 {
        builder.start_offset(seek_amount);
    }

    Ok(builder)
}

fn iterate_video_frames(cfg: &FrameReadCfg) -> Result<impl Iterator<Item = GrayImage>, String> {
    let mut it = cfg.clone().spawn_gray().peekable();

    match it.peek() {
        Some(Err(e)) => Err(format!("{e:?}")),
        None => Err("None".to_string()),
        Some(Ok(_frame)) => Ok(it.filter_map(Result::ok).take(DCT_SIZE as usize)),
    }
}

fn are_all_frames_same_size<'a, T>(frames: T) -> VideoHashResult<()>
where
    T: Iterator<Item = &'a GrayImage>,
{
    use itertools::Itertools;
    for (f1, f2) in frames.tuple_windows::<(_, _)>() {
        if f1.dimensions() != f2.dimensions() {
            let msg = format!(
                "frames not all same size: Expected {:?}, Actual {:?}",
                f1.dimensions(),
                f2.dimensions()
            );
            return Err(crate::Error::VidProc(msg));
        }
    }

    Ok(())
}

fn crop_video_frames<T>(frames: T, cropdetect_algo: Cropdetect) -> VideoHashResult<Vec<GrayImage>>
where
    T: Iterator<Item = GrayImage>,
{
    let frames = frames.collect::<Vec<_>>();

    are_all_frames_same_size(frames.iter())?;

    let crop = detect_crop(&frames, cropdetect_algo).ok_or(crate::Error::NotEnoughFrames)?;

    let cropped_frames = frames
        .into_iter()
        .map(|f| f.cropped(crop).to_image())
        .collect::<Vec<_>>();

    Ok(cropped_frames)
}

fn detect_crop(frames: &[GrayImage], detect_method: Cropdetect) -> Option<Crop> {
    match detect_method {
        Cropdetect::None => cropdetect_none(frames),
        Cropdetect::Letterbox => cropdetect_letterbox(frames),
        Cropdetect::Motion => cropdetect_motion(frames),
    }
}

pub fn gen_hash(src_path: PathBuf, opts: CreationOptions) -> Result<VideoHash, crate::Error> {
    use crate::Error::VidProc;
    let frame_read_cfg = build_frame_reader(src_path.clone(), opts)?;
    let frames = iterate_video_frames(&frame_read_cfg).map_err(VidProc)?;
    let frames = crop_video_frames(frames, opts.cropdetect)?;

    let duration = get_duration(&src_path).map_err(|e| VidProc(format!("{e:?}")))?;

    VideoHash::from_frames(frames, src_path, duration.as_secs() as u32)
}
