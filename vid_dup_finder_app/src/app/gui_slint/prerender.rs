use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use crossbeam_channel::{self, Receiver, Sender};

use image::{buffer::ConvertBuffer, RgbImage};

use rayon::prelude::*;
use vid_dup_finder_common::{
    row_images, video_frames_gray::VdfFrameSeqExt, FrameSeqRgb, VideoFramesGray,
};
use vid_dup_finder_lib::{debug_util::build_frame_reader, CreationOptions};

use super::{CacheEntry, RenderDetails};

#[allow(clippy::type_complexity)]
pub fn start_prerender_thread() -> (
    Vec<JoinHandle<()>>,
    Sender<CacheEntry>,
    Receiver<(CacheEntry, Vec<RgbImage>)>,
) {
    let (cmd_tx, cmd_rx) = crossbeam_channel::bounded::<CacheEntry>(1);
    let (rsp_tx, rsp_rx) = crossbeam_channel::bounded(1);
    let dbg_count = Arc::<AtomicUsize>::new(0.into());
    let rendering_current_vid = Arc::<AtomicBool>::new(false.into());
    let handles = (0..=3)
        .map(|_| {
            worker_thread(
                cmd_rx.clone(),
                rsp_tx.clone(),
                dbg_count.clone(),
                rendering_current_vid.clone(),
            )
        })
        .collect::<Vec<_>>();

    (handles, cmd_tx, rsp_rx)
}

fn worker_thread(
    cmd_rx: Receiver<CacheEntry>,
    rsp_tx: Sender<(CacheEntry, Vec<RgbImage>)>,
    dbg_count: Arc<AtomicUsize>,
    rendering_current_vid: Arc<AtomicBool>,
) -> JoinHandle<()> {
    std::thread::spawn({
        move || {
            for entry in cmd_rx.iter() {
                let _active_threads = dbg_count.fetch_add(1, Ordering::SeqCst) + 1;
                // dbg!(active_threads);

                if entry.render_details.is_current {
                    rendering_current_vid.store(true, Ordering::SeqCst);
                }

                if !entry.render_details.is_current {
                    while rendering_current_vid.load(Ordering::SeqCst) {
                        thread::sleep(Duration::from_millis(100));
                    }
                }

                let thumbs = entry
                    .thunk
                    .entries()
                    .into_par_iter()
                    .map(|p| render_thumbs(p, entry.render_details))
                    .collect();

                if entry.render_details.is_current {
                    rendering_current_vid.store(false, Ordering::SeqCst);
                }

                rsp_tx.send((entry, thumbs)).unwrap();
                // let active_threads = dbg_count.fetch_sub(1, Ordering::SeqCst) - 1;

                // if cmd_rx.is_empty() {
                //     dbg!(active_threads);
                // }
            }
        }
    })
}

fn fallback_images() -> RgbImage {
    RgbImage::new(100, 100)
}

fn render_thumbs(src_path: &Path, render_details: RenderDetails) -> RgbImage {
    let max_thumbs = 3;
    let opts = CreationOptions::default();

    let cfg = build_frame_reader(src_path, opts);

    let frame_iter_cfg = match cfg {
        Ok(obj) => obj,
        Err(_e) => return fallback_images(),
    };

    let mut frame_iter = frame_iter_cfg.spawn_rgb().peekable();

    if matches!(frame_iter.peek(), None | Some(Err(_))) {
        return fallback_images();
    }

    let thumbs = frame_iter
        .step_by(8)
        .filter_map(Result::ok)
        .take(max_thumbs as usize)
        .collect::<Vec<_>>();

    if render_details.cropdetect {
        let uncropped_frames =
            VideoFramesGray::from_images(thumbs.iter().map(|t| t.convert())).unwrap();
        let crop = uncropped_frames.motiondetect_crop();
        //dbg!(crop);
        let frames = FrameSeqRgb::from_images(thumbs).unwrap().crop(crop);

        row_images(frames.into_inner().iter()).unwrap()
    } else {
        row_images(thumbs.iter()).unwrap()
    }
}
