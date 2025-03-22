use std::{
    collections::{HashMap, VecDeque},
    num::{NonZero, NonZeroU32},
    os::unix::fs::MetadataExt,
    sync::Arc,
    thread::{self, JoinHandle},
    time::Duration,
};

use crossbeam_channel::{Receiver, Select, Sender};

use ffmpeg_gst_wrapper::{get_duration, get_resolution};

use image::{
    buffer::ConvertBuffer,
    codecs::{avif::AvifEncoder, jpeg::JpegEncoder},
    ImageEncoder, RgbImage,
};
use lru::LruCache;
use parking_lot::Mutex;
use rayon::prelude::*;
use slint::SharedPixelBuffer;

use crate::app::ResolutionThunk;

use super::{prerender, CacheEntry, GuiCmd, GuiRsp, SlintImage};

struct Cache(lru::LruCache<CacheEntry, Vec<SharedPixelBuffer<slint::Rgb8Pixel>>>);
impl Cache {
    pub fn new() -> Self {
        let size = NonZero::new(50).unwrap();
        Self(LruCache::new(size))
    }

    pub fn clear_thumbs(&mut self, thunk: &ResolutionThunk) {
        let matching_entries = self
            .0
            .iter()
            .filter(|entry| entry.0.thunk == *thunk)
            .map(|(entry, _imgs)| entry.clone())
            .collect::<Vec<_>>();

        for e in matching_entries {
            let _ = self.0.pop(&e);
        }
    }

    pub fn promote(&mut self, entry: &CacheEntry) {
        self.0.promote(entry)
    }

    pub fn contains(&self, entry: &CacheEntry) -> bool {
        self.0.contains(entry)
    }

    pub fn get(&mut self, entry: &CacheEntry) -> Option<&Vec<SlintImage>> {
        self.0.get(entry)
    }

    pub fn put(&mut self, entry: CacheEntry, imgs: Vec<SlintImage>) {
        let _ = self.0.put(entry, imgs);
    }
}

type PngSizeCache = HashMap<CacheEntry, Vec<u64>>;
type AvifSizeCache = HashMap<CacheEntry, Vec<u64>>;
type JpgSizeCache = HashMap<CacheEntry, Vec<u64>>;
type CannySizeCache = HashMap<CacheEntry, Vec<u64>>;
type FileSizeCache = HashMap<CacheEntry, Vec<u64>>;
type ResolutionCache = HashMap<CacheEntry, Vec<(u32, u32)>>;
type DurationCache = HashMap<CacheEntry, Vec<Duration>>;
// type LenCache = HashMap<CacheEntry, Vec<u64>>;

#[allow(clippy::type_complexity)]
// #[rustfmt::max_width(200)]
pub fn start_cache_thread(
    gui_cmd_rx: Receiver<GuiCmd>,
    gui_rsp_tx: Sender<GuiRsp>,
) -> JoinHandle<()> {
    let thread_main = move || {
        let (_gen_thread, gen_cmd_tx, gen_rsp_rx) = prerender::start_prerender_thread();

        ///////////////////////////////////////////////////////////////
        // need to merge commands from the gui and responses from the worker
        enum MergedMsg {
            FromGui(GuiCmd),
            FromGen((CacheEntry, Vec<RgbImage>)),
        }

        let mut get_next_msg = {
            let mut inputs_merge_rx = Select::new();
            let gen_rx_idx = inputs_merge_rx.recv(&gen_rsp_rx);
            let cache_rx_idx = inputs_merge_rx.recv(&gui_cmd_rx);

            let gen_rsp_rx = gen_rsp_rx.clone();
            let gui_cmd_rx = gui_cmd_rx.clone();
            move || match inputs_merge_rx.ready() {
                i if i == gen_rx_idx => MergedMsg::FromGen(gen_rsp_rx.try_recv().unwrap()),
                i if i == cache_rx_idx => MergedMsg::FromGui(gui_cmd_rx.try_recv().unwrap()),
                _ => unreachable!(),
            }
        };

        let gen_q = Arc::new(Mutex::new(VecDeque::new()));
        let mut fetch_req = None;
        let mut cache = Cache::new();
        let png_size_cache = Arc::new(Mutex::new(PngSizeCache::new()));
        let avif_size_cache = Arc::new(Mutex::new(AvifSizeCache::new()));
        let jpg_size_cache = Arc::new(Mutex::new(JpgSizeCache::new()));
        let canny_size_cache = Arc::new(Mutex::new(CannySizeCache::new()));
        let file_size_cache = Arc::new(Mutex::new(FileSizeCache::new()));
        let duration_cache = Arc::new(Mutex::new(DurationCache::new()));
        let resolution_cache = Arc::new(Mutex::new(ResolutionCache::new()));
        let mut stats_en = true;
        // let vid_len_cache = Arc::new(Mutex::new(LenCache::new()));

        loop {
            use GuiCmd::*;
            use MergedMsg::*;

            let next_msg = get_next_msg();
            let mut gen_q = gen_q.lock();

            match next_msg {
                FromGui(Clear(thunk)) => cache.clear_thumbs(&thunk),
                FromGui(Generate(entry)) => {
                    //bump to the top of list if exists in cache
                    if cache.contains(&entry) {
                        cache.promote(&entry)
                    } else {
                        gen_q.push_back(entry);
                        gui_rsp_tx.send(GuiRsp::IncQLen).unwrap()
                    }
                }
                FromGui(Fetch(entry)) => {
                    for _ in 0..gen_q.len() {
                        gui_rsp_tx.send(GuiRsp::DecQLen).unwrap();
                    }
                    gen_q.clear();
                    if let Some(imgs) = cache.get(&entry) {
                        fetch_req = None;
                        gui_rsp_tx
                            .send(GuiRsp::Fetched((entry.clone(), imgs.clone())))
                            .unwrap();
                    } else {
                        fetch_req = Some(entry.clone());
                        gui_rsp_tx.send(GuiRsp::Wait).unwrap();
                        gen_q.push_back(entry);
                        gui_rsp_tx.send(GuiRsp::IncQLen).unwrap()
                    }
                }

                FromGui(FetchPngSize(entry)) => {
                    if let Some(sizes) = png_size_cache.lock().get(&entry) {
                        gui_rsp_tx
                            .send(GuiRsp::PngSize(entry.clone(), sizes.clone()))
                            .unwrap();
                    }
                }

                FromGui(FetchAvifSize(entry)) => {
                    if let Some(sizes) = avif_size_cache.lock().get(&entry) {
                        gui_rsp_tx
                            .send(GuiRsp::AvifSize(entry.clone(), sizes.clone()))
                            .unwrap();
                    }
                }

                FromGui(FetchJpgSize(entry)) => {
                    if let Some(sizes) = jpg_size_cache.lock().get(&entry) {
                        gui_rsp_tx
                            .send(GuiRsp::JpgSize(entry.clone(), sizes.clone()))
                            .unwrap();
                    }
                }

                FromGui(FetchCannySize(entry)) => {
                    if let Some(sizes) = canny_size_cache.lock().get(&entry) {
                        gui_rsp_tx
                            .send(GuiRsp::CannySize(entry.clone(), sizes.clone()))
                            .unwrap();
                    }
                }

                FromGui(FetchFileSize(entry)) => {
                    if let Some(sizes) = file_size_cache.lock().get(&entry) {
                        gui_rsp_tx
                            .send(GuiRsp::FileSize(entry.clone(), sizes.clone()))
                            .unwrap();
                    }
                }

                FromGui(FetchVidDuration(entry)) => {
                    if let Some(sizes) = duration_cache.lock().get(&entry) {
                        gui_rsp_tx
                            .send(GuiRsp::VidDuration(entry.clone(), sizes.clone()))
                            .unwrap();
                    }
                }

                FromGui(FetchVidResolution(entry)) => {
                    if let Some(resolutions) = resolution_cache.lock().get(&entry) {
                        gui_rsp_tx
                            .send(GuiRsp::VidResolution(entry.clone(), resolutions.clone()))
                            .unwrap();
                    }
                }

                FromGui(StatsEn(val)) => {
                    stats_en = val;
                }

                FromGen((entry, imgs)) => {
                    gui_rsp_tx.send(GuiRsp::DecQLen).unwrap();

                    let imgbufs_slint = imgs
                        .iter()
                        .map(|img| {
                            slint::SharedPixelBuffer::<slint::Rgb8Pixel>::clone_from_slice(
                                img.as_raw(),
                                img.width(),
                                img.height(),
                            )
                        })
                        .collect::<Vec<_>>();

                    cache.put(entry.clone(), imgbufs_slint.clone());

                    if let Some(ref fetch_req_val) = fetch_req {
                        if *fetch_req_val == entry {
                            fetch_req = None;
                            gui_rsp_tx
                                .send(GuiRsp::Fetched((entry.clone(), imgbufs_slint.clone())))
                                .unwrap();
                        }
                    }

                    if stats_en {
                        thread::spawn({
                            let entry = entry.clone();
                            let png_size_cache = png_size_cache.clone();
                            let gui_rsp_tx = gui_rsp_tx.clone();
                            let imgs = imgs.clone();
                            move || {
                                gui_rsp_tx.send(GuiRsp::IncQQueue).unwrap();
                                thread_priority::set_current_thread_priority(
                                    thread_priority::ThreadPriority::Min,
                                )
                                .unwrap();
                                // loop {
                                //     thread::sleep(Duration::from_millis(100));
                                //     let gen_q = gen_q.lock();
                                //     if gen_q.len() < 1 {
                                //         break;
                                //     }
                                // }
                                gui_rsp_tx.send(GuiRsp::IncPngQueue).unwrap();

                                let png_sizes =
                                    imgs.par_iter().map(calc_png_size).collect::<Vec<_>>();

                                let _ = png_size_cache
                                    .lock()
                                    .insert(entry.clone(), png_sizes.clone());

                                gui_rsp_tx.send(GuiRsp::PngSize(entry, png_sizes)).unwrap();
                                gui_rsp_tx.send(GuiRsp::DecPngQueue).unwrap();
                            }
                        });

                        thread::spawn({
                            let entry = entry.clone();
                            let jpg_size_cache = jpg_size_cache.clone();
                            let gui_rsp_tx = gui_rsp_tx.clone();
                            let imgs = imgs.clone();
                            move || {
                                gui_rsp_tx.send(GuiRsp::IncQQueue).unwrap();
                                thread_priority::set_current_thread_priority(
                                    thread_priority::ThreadPriority::Min,
                                )
                                .unwrap();
                                // loop {
                                //     thread::sleep(Duration::from_millis(100));
                                //     let gen_q = gen_q.lock();
                                //     if gen_q.len() < 1 {
                                //         break;
                                //     }
                                // }
                                gui_rsp_tx.send(GuiRsp::IncJpgQueue).unwrap();

                                let jpg_sizes =
                                    imgs.par_iter().map(calc_jpg_size).collect::<Vec<_>>();

                                let _ = jpg_size_cache
                                    .lock()
                                    .insert(entry.clone(), jpg_sizes.clone());

                                gui_rsp_tx.send(GuiRsp::JpgSize(entry, jpg_sizes)).unwrap();
                                gui_rsp_tx.send(GuiRsp::DecJpgQueue).unwrap();
                            }
                        });

                        thread::spawn({
                            let entry = entry.clone();
                            let avif_size_cache = avif_size_cache.clone();
                            let gui_rsp_tx = gui_rsp_tx.clone();
                            let imgs = imgs.clone();
                            move || {
                                thread_priority::set_current_thread_priority(
                                    thread_priority::ThreadPriority::Min,
                                )
                                .unwrap();
                                gui_rsp_tx.send(GuiRsp::IncQQueue).unwrap();
                                // loop {
                                //     thread::sleep(Duration::from_millis(100));
                                //     let gen_q = gen_q.lock();
                                //     if gen_q.len() < 1 {
                                //         break;
                                //     }
                                // }
                                gui_rsp_tx.send(GuiRsp::IncAvifQueue).unwrap();

                                let avif_sizes =
                                    imgs.par_iter().map(calc_avif_size).collect::<Vec<_>>();

                                let _ = avif_size_cache
                                    .lock()
                                    .insert(entry.clone(), avif_sizes.clone());

                                gui_rsp_tx
                                    .send(GuiRsp::AvifSize(entry, avif_sizes))
                                    .unwrap();
                                gui_rsp_tx.send(GuiRsp::DecAvifQueue).unwrap();
                            }
                        });

                        thread::spawn({
                            let entry = entry.clone();
                            let canny_size_cache = canny_size_cache.clone();
                            let gui_rsp_tx = gui_rsp_tx.clone();
                            move || {
                                gui_rsp_tx.send(GuiRsp::IncQQueue).unwrap();
                                thread_priority::set_current_thread_priority(
                                    thread_priority::ThreadPriority::Min,
                                )
                                .unwrap();
                                // loop {
                                //     std::thread::sleep(Duration::from_millis(100));
                                //     let gen_q = gen_q.lock();
                                //     if gen_q.len() < 1 {
                                //         break;
                                //     }
                                // }
                                gui_rsp_tx.send(GuiRsp::IncCannyQueue).unwrap();
                                let canny_sizes =
                                    imgs.par_iter().map(calc_canny_size).collect::<Vec<_>>();

                                let _ = canny_size_cache
                                    .lock()
                                    .insert(entry.clone(), canny_sizes.clone());

                                gui_rsp_tx
                                    .send(GuiRsp::CannySize(entry, canny_sizes))
                                    .unwrap();
                                gui_rsp_tx.send(GuiRsp::DecCannyQueue).unwrap();
                            }
                        });
                    }

                    thread::spawn({
                        let entry = entry.clone();
                        let file_size_cache = file_size_cache.clone();
                        let duration_cache = duration_cache.clone();
                        let resolution_cache = resolution_cache.clone();
                        let gui_rsp_tx = gui_rsp_tx.clone();
                        move || {
                            let file_sizes = entry
                                .thunk
                                .entries()
                                .iter()
                                .map(|p| {
                                    std::fs::metadata(p)
                                        .map(|metadata| metadata.size())
                                        .unwrap_or_default()
                                })
                                .collect::<Vec<_>>();

                            let _ = file_size_cache
                                .lock()
                                .insert(entry.clone(), file_sizes.clone());

                            gui_rsp_tx
                                .send(GuiRsp::FileSize(entry.clone(), file_sizes))
                                .unwrap();

                            let durations = entry
                                .thunk
                                .entries()
                                .iter()
                                .map(|p| get_duration(&p).unwrap_or_default())
                                .collect::<Vec<_>>();

                            let _ = duration_cache
                                .lock()
                                .insert(entry.clone(), durations.clone());

                            gui_rsp_tx
                                .send(GuiRsp::VidDuration(entry.clone(), durations))
                                .unwrap();

                            let resolutions = entry
                                .thunk
                                .entries()
                                .iter()
                                .map(|p| get_resolution(&p).unwrap_or_default())
                                .collect::<Vec<_>>();

                            let _ = resolution_cache
                                .lock()
                                .insert(entry.clone(), resolutions.clone());

                            gui_rsp_tx
                                .send(GuiRsp::VidResolution(entry, resolutions))
                                .unwrap();
                        }
                    });
                }
            };

            if !gen_cmd_tx.is_full() {
                if let Some(cmd) = gen_q.pop_front() {
                    gen_cmd_tx.try_send(cmd).unwrap();
                }
            }

            // gui_rsp_tx.send(GuiRsp::QLen(gen_q.len() as i32)).unwrap()
        }
    };

    std::thread::spawn(thread_main)
}

fn calc_png_size(img: &RgbImage) -> u64 {
    let size = NonZeroU32::try_from(500).expect("literal");

    let resized = vid_dup_finder_common::resize_rgb::resize_img_rgb(img, size, size);

    let mut buf = std::io::Cursor::new(vec![]);

    let buf_len = resized
        .write_to(&mut buf, image::ImageFormat::Png)
        .map(|()| buf.into_inner().len() as u64)
        .unwrap();

    buf_len
}

fn calc_avif_size(img: &RgbImage) -> u64 {
    let size = NonZeroU32::try_from(500).expect("literal");

    let resized = vid_dup_finder_common::resize_rgb::resize_img_rgb(img, size, size);

    let mut buf = std::io::Cursor::new(vec![]);
    let encoder = AvifEncoder::new_with_speed_quality(&mut buf, 7, 98).with_num_threads(Some(1));
    // let mut encoder = JpegEncoder::new_with_quality(&mut buf, 80);

    encoder
        .write_image(
            resized.as_raw(),
            resized.width(),
            resized.height(),
            image::ExtendedColorType::Rgb8,
        )
        .unwrap();

    // encoder.encode_image(&resized).unwrap();

    buf.into_inner().len() as u64
}

fn calc_jpg_size(img: &RgbImage) -> u64 {
    let size = NonZeroU32::try_from(500).expect("literal");

    let resized = vid_dup_finder_common::resize_rgb::resize_img_rgb(img, size, size);

    let mut buf = std::io::Cursor::new(vec![]);

    let mut encoder = JpegEncoder::new_with_quality(&mut buf, 95);

    encoder.encode_image(&resized).unwrap();

    buf.into_inner().len() as u64
}

fn calc_canny_size(img: &RgbImage) -> u64 {
    const CANNY_MIN: f32 = 5.0;
    const CANNY_MAX: f32 = 30.0;

    let gray_frame = img.convert();

    //normalize image size
    let norm_frame = vid_dup_finder_common::resize_gray::resize_frame(
        gray_frame,
        NonZeroU32::new(800).unwrap(),
        NonZeroU32::new(800).unwrap(),
    );

    let new_img = imageproc::edges::canny(&norm_frame, CANNY_MIN, CANNY_MAX);
    new_img.pixels().filter(|pix| pix.0[0] > 0).count() as u64
}
