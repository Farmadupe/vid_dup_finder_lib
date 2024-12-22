mod lru_cache;
mod prerender;

use std::{path::PathBuf, time::Duration};

use bytesize::ByteSize;
mod modulo;
mod vlc_thread;
use itertools::Itertools;
use lru_cache::start_cache_thread;
use modulo::Modulo;
use slint::{Model, ModelRc, SharedString, VecModel, Weak};
use vlc_thread::start_vlc_thread;

use super::{ResolutionError, ResolutionThunk};

slint::include_modules!();

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum GuiCmd {
    Generate(CacheEntry),
    Fetch(CacheEntry),
    Clear(ResolutionThunk),
    FetchPngSize(CacheEntry),
    FetchAvifSize(CacheEntry),
    FetchJpgSize(CacheEntry),
    FetchCannySize(CacheEntry),
    FetchFileSize(CacheEntry),
    FetchVidDuration(CacheEntry),
    FetchVidResolution(CacheEntry),
    StatsEn(bool),
}

#[derive(Debug)]
enum GuiRsp {
    Fetched((CacheEntry, Vec<SlintImage>)),
    Wait,
    IncQLen,
    DecQLen,
    ResolvedResult(Result<(), ResolutionError>),
    VlcOpened,
    VlcClosed,
    PngSize(CacheEntry, Vec<u64>),
    AvifSize(CacheEntry, Vec<u64>),
    JpgSize(CacheEntry, Vec<u64>),
    CannySize(CacheEntry, Vec<u64>),
    FileSize(CacheEntry, Vec<u64>),
    VidDuration(CacheEntry, Vec<Duration>),
    VidResolution(CacheEntry, Vec<(u32, u32)>),
    IncQQueue,
    IncPngQueue,
    DecPngQueue,
    IncJpgQueue,
    DecJpgQueue,
    IncAvifQueue,
    DecAvifQueue,
    IncCannyQueue,
    DecCannyQueue,
}

type SlintImage = slint::SharedPixelBuffer<slint::Rgb8Pixel>;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
struct RenderDetails {
    pub cropdetect: bool,
    pub is_current: bool,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct CacheEntry {
    render_details: RenderDetails,
    thunk: ResolutionThunk,
}

pub fn run_gui_slint(thunks: Vec<ResolutionThunk>) -> Result<(), slint::PlatformError> {
    let (gui_cmd_tx, gui_cmd_rx) = crossbeam_channel::unbounded::<GuiCmd>();
    let (gui_rsp_tx, gui_rsp_rx) = crossbeam_channel::unbounded::<GuiRsp>();

    let _cache_thread = start_cache_thread(gui_cmd_rx.clone(), gui_rsp_tx.clone());

    let (vlc_tx, vlc_rx) = crossbeam_channel::unbounded::<PathBuf>();
    start_vlc_thread(vlc_rx, gui_rsp_tx.clone());

    let ui = MainWindow::new()?;

    gui_cmd_tx
        .send(GuiCmd::StatsEn(!ui.get_disable_stats()))
        .unwrap();
    ui.on_set_stats_en({
        let gui_cmd_tx = gui_cmd_tx.clone();
        move |val| {
            gui_cmd_tx.send(GuiCmd::StatsEn(val)).unwrap();
        }
    });

    ui.on_request_next_thunk({
        let ui_handle = ui.as_weak();
        let thunks = thunks.clone();
        move || {
            let ui = ui_handle.unwrap();
            let Some(mut next_idx) = (ui.get_thunk_idx() as usize).checked_add(1) else {
                return;
            };

            if next_idx > thunks.len() - 1 {
                next_idx = 0;
            }

            ui.set_thunk_idx(next_idx as i32);
            ui.set_curr_vid(0);
            ui.invoke_regen_images();
        }
    });

    ui.on_regen_images({
        let ui_handle = ui.as_weak();
        let thunks = thunks.clone();
        let gui_cmd_tx = gui_cmd_tx.clone();
        move || issue_prerender_commands(&ui_handle, &thunks, &gui_cmd_tx)
    });

    ui.on_request_prev_thunk({
        let ui_handle = ui.as_weak();
        let thunks = thunks.clone();
        move || {
            let ui = ui_handle.unwrap();
            let next_idx = match (ui.get_thunk_idx() as usize).checked_sub(1) {
                Some(val) => val,
                None => thunks.len() - 1,
            };
            ui.set_thunk_idx(next_idx as i32);
            ui.set_curr_vid(0);
            ui.invoke_regen_images();
        }
    });

    // ui.on_jump_to_thunk({
    //     let ui = ui.as_weak();
    //     move || {
    //         let ui = ui.unwrap();
    //     }
    // });

    ui.on_accept_idx_input({
        let ui_handle = ui.as_weak();
        let thunks = thunks.clone();
        let gui_cmd_tx = gui_cmd_tx.clone();
        move |s| {
            let ui = ui_handle.unwrap();
            let Ok(next_idx) = s.parse::<usize>() else {
                return;
            };

            let Some(_) = thunks.get(next_idx) else {
                return;
            };
            ui.set_curr_vid(0);
            ui.set_thunk_idx(next_idx as i32);
            issue_prerender_commands(&ui.as_weak(), &thunks, &gui_cmd_tx);
        }
    });

    ui.on_accept_idx_keep({
        let ui_handle = ui.as_weak();
        let thunks = thunks.clone();
        let gui_cmd_tx = gui_cmd_tx.clone();
        move |s| {
            let ui = ui_handle.unwrap();
            let thunk_idx = ui.get_thunk_idx() as usize;

            let thunk = thunks.get(thunk_idx).unwrap().clone();
            let s = s.to_string();
            gui_cmd_tx.send(GuiCmd::Clear(thunk.clone())).unwrap();
            let gui_rsp_tx = gui_rsp_tx.clone();
            ui.invoke_set_resolved_ok_colour("blue".into());
            std::thread::spawn(move || {
                // std::thread::sleep(std::time::Duration::from_secs(1));
                gui_rsp_tx
                    .send(GuiRsp::ResolvedResult(thunk.resolve_2(s)))
                    .unwrap()
            });

            ui.invoke_request_next_thunk();
            issue_prerender_commands(&ui.as_weak(), &thunks, &gui_cmd_tx);
        }
    });

    ui.on_exclude_curr_vid({
        let ui_handle = ui.as_weak();
        let thunks = thunks.clone();
        move || {
            let ui = ui_handle.unwrap();

            //make sure we can't exlude the last vid
            if ui.get_thunk_entries().iter().filter(|e| !e.enabled).count() < 1 {
                return;
            }

            let vid_idx = ui.get_curr_vid() as usize;

            ui.get_thunk_entries().row_data(vid_idx).unwrap().enabled = false;
            ui.set_curr_vid(incr_curr_vid(&ui.as_weak(), &thunks));
        }
    });

    // ui.on_accept_idx_exclude({
    //     let ui_handle = ui.as_weak();
    //     let thunks = thunks.clone();
    //     let gui_cmd_tx = gui_cmd_tx.clone();
    //     move |s| {
    //         let ui = ui_handle.unwrap();
    //         let thunk_idx = ui.get_thunk_idx() as usize;

    //         let thunk = thunks.get(thunk_idx).unwrap().clone();
    //         let Ok(idx): Result<i32 = s.to_string().parse() else {
    //             return;
    //         };

    //         let gui_rsp_tx = gui_rsp_tx.clone();

    //         issue_prerender_commands(&ui.as_weak(), &thunks, &gui_cmd_tx);
    //     }
    // });

    ui.on_view_curr_vid({
        let vlc_tx = vlc_tx.clone();
        let ui_handle = ui.as_weak();
        move |p| {
            ui_handle.unwrap().invoke_set_vlc_colour("red".into());
            vlc_tx.send(PathBuf::from(p.to_string())).unwrap();
        }
        // move |p| {
        //     let binary = "autocrop-vid.sh";
        //     let p = p.to_string();
        //     let _ = std::process::Command::new(binary).arg(p).spawn();
        // }
    });

    ui.on_browse_curr_vid({
        move |p| {
            let binary = "nautilus";
            let p = p.to_string();
            let _ = std::process::Command::new(binary).arg(p).spawn();
        }
    });

    ui.on_view_top_vid({
        let ui_handle = ui.as_weak();
        let thunks = thunks.clone();
        let vlc_tx = vlc_tx.clone();
        move || {
            let ui = ui_handle.unwrap();
            if let Some(curr_vid) = thunks.get(ui.get_thunk_idx() as usize).and_then(|thunk| {
                let entries = thunk.entries().to_owned();
                entries
                    .get(ui.get_curr_vid() as usize)
                    .map(|x| x.to_path_buf())
            }) {
                ui_handle.unwrap().invoke_set_vlc_colour("red".into());
                vlc_tx.send(curr_vid).unwrap();
                // let binary = "autocrop-vid.sh";
                // let _ = std::process::Command::new(binary)
                //     .arg(curr_vid.to_string_lossy().to_string())
                //     .spawn();
            }
        }
    });

    ui.on_browse_top_vid({
        let ui_handle = ui.as_weak();
        let thunks = thunks.clone();
        move || {
            let ui = ui_handle.unwrap();
            if let Some(curr_vid) = thunks.get(ui.get_thunk_idx() as usize).and_then(|thunk| {
                let entries = thunk.entries().to_owned();
                entries
                    .get(ui.get_curr_vid() as usize)
                    .map(|x| x.to_path_buf())
            }) {
                let binary = "nautilus";

                let _ = std::thread::spawn({
                    let curr_vid = curr_vid.clone();
                    move || {
                        std::process::Command::new(binary)
                            .arg(curr_vid.to_string_lossy().to_string())
                            .spawn()
                            .unwrap()
                            .wait()
                    }
                });
            }
        }
    });

    ui.on_key_callback({
        let ui_handle = ui.as_weak();
        let thunks = thunks.clone();
        move |event| {
            let ui = ui_handle.unwrap();
            // dbg!(&event);

            let next_char: char = event.text.as_str().chars().next().unwrap();
            // dbg!(next_char);
            match next_char {
                '\u{f701}' => {
                    ui.set_view_many(false);
                    ui.set_curr_vid(incr_curr_vid(&ui.as_weak(), &thunks));
                    ui.invoke_regen_images();
                }
                '\u{f700}' => {
                    ui.set_view_many(false);
                    ui.set_curr_vid(decr_curr_vid(&ui.as_weak(), &thunks));
                    ui.invoke_regen_images();
                }
                '/' if event.modifiers.control => ui.invoke_accept_idx_keep("0".into()),
                '\n' if event.modifiers.control => {
                    ui.invoke_accept_idx_keep(ui.get_curr_vid().to_string().into())
                }
                'z' if event.modifiers.control => {
                    let resolution_command = format!("u{}", ui.get_curr_vid());
                    ui.invoke_accept_idx_keep(resolution_command.into())
                }
                '\u{f703}' => ui.invoke_request_next_thunk(),
                '\'' if event.modifiers.control => ui.invoke_accept_idx_keep("1".into()),
                '\u{f702}' => ui.invoke_request_prev_thunk(),
                '\u{f72c}' => {
                    ui.set_view_many(true);
                    ui.invoke_regen_images();
                }
                '\u{f72d}' => {
                    ui.set_view_many(false);
                    ui.invoke_regen_images();
                }

                '=' if event.modifiers.control => {
                    ui.set_zoom_val((ui.get_zoom_val() + 50.0).clamp(100.0, 1800.0))
                }
                '-' if event.modifiers.control => {
                    ui.set_zoom_val((ui.get_zoom_val() - 50.0).clamp(100.0, 1800.0))
                }
                'j' if event.modifiers.control => {
                    ui.invoke_focus_jumpbox();
                }
                'k' if event.modifiers.control => {
                    ui.invoke_focus_keep();
                }
                'c' if event.modifiers.control => {
                    ui.set_cropdetect(!ui.get_cropdetect());
                    ui.invoke_regen_images();
                }
                's' if event.modifiers.control => {
                    ui.set_square(!ui.get_square());
                }
                'x' if event.modifiers.control => {
                    ui.invoke_exclude_curr_vid();
                }
                'w' if event.modifiers.control => {
                    ui.invoke_view_top_vid();
                }
                'b' if event.modifiers.control => {
                    ui.invoke_browse_top_vid();
                }
                _ => (),
            }
        }
    });

    // ui.on_winit_window_event()

    let ui_weak = ui.as_weak();

    #[allow(clippy::cmp_owned)] //false positive
    #[allow(clippy::useless_conversion)] //false positive
    let _fetch_thread = std::thread::spawn({
        let gui_cmd_tx = gui_cmd_tx.clone();
        move || loop {
            for resp in gui_rsp_rx.iter() {
                let ui = ui_weak.clone();
                use GuiRsp::*;

                slint::invoke_from_event_loop({
                    let gui_cmd_tx = gui_cmd_tx.clone();

                    if !matches!(resp, Fetched(_)) {
                        // dbg!(&resp);
                    }

                    move || match resp {
                        Fetched((thunk, imgs)) => {
                            let x = gen_gui_data(imgs, thunk.clone(), &ui);

                            let ui = ui.unwrap();

                            ui.set_thunk_entries(x);

                            gui_cmd_tx
                                .send(GuiCmd::FetchPngSize(thunk.clone()))
                                .unwrap();
                            gui_cmd_tx
                                .send(GuiCmd::FetchAvifSize(thunk.clone()))
                                .unwrap();
                            gui_cmd_tx
                                .send(GuiCmd::FetchJpgSize(thunk.clone()))
                                .unwrap();
                            gui_cmd_tx
                                .send(GuiCmd::FetchCannySize(thunk.clone()))
                                .unwrap();
                            gui_cmd_tx
                                .send(GuiCmd::FetchFileSize(thunk.clone()))
                                .unwrap();
                            gui_cmd_tx
                                .send(GuiCmd::FetchVidDuration(thunk.clone()))
                                .unwrap();

                            gui_cmd_tx
                                .send(GuiCmd::FetchVidResolution(thunk.clone()))
                                .unwrap();
                        }
                        Wait => {
                            ui.unwrap()
                                .set_thunk_entries(ModelRc::new(VecModel::from(vec![])));
                        }

                        IncQLen => {
                            let ui = ui.unwrap();
                            ui.set_proc_q_len(ui.get_proc_q_len() + 1);
                        }

                        DecQLen => {
                            let ui = ui.unwrap();
                            ui.set_proc_q_len(ui.get_proc_q_len() - 1);
                        }

                        ResolvedResult(Ok(())) => {
                            ui.unwrap().invoke_set_resolved_ok_colour("black".into())
                        }
                        ResolvedResult(Err(_)) => {
                            ui.unwrap().invoke_set_resolved_ok_colour("red".into())
                        }
                        VlcOpened => ui.unwrap().invoke_set_vlc_colour("blue".into()),
                        VlcClosed => ui.unwrap().invoke_set_vlc_colour("black".into()),

                        IncQQueue => {
                            let ui = ui.unwrap();
                            ui.set_q_q_len(ui.get_q_q_len() + 1);
                        }

                        IncPngQueue => {
                            let ui = ui.unwrap();
                            ui.set_png_q_len(ui.get_png_q_len() + 1);
                            ui.set_q_q_len((ui.get_q_q_len() - 1).max(0));
                        }
                        DecPngQueue => {
                            let ui = ui.unwrap();
                            ui.set_png_q_len(ui.get_png_q_len() - 1);
                        }
                        IncJpgQueue => {
                            let ui = ui.unwrap();
                            ui.set_jpg_q_len(ui.get_jpg_q_len() + 1);
                            ui.set_q_q_len((ui.get_q_q_len() - 1).max(0));
                        }
                        DecJpgQueue => {
                            let ui = ui.unwrap();
                            ui.set_jpg_q_len(ui.get_jpg_q_len() - 1);
                        }
                        IncAvifQueue => {
                            let ui = ui.unwrap();
                            ui.set_avif_q_len(ui.get_avif_q_len() + 1);
                            ui.set_q_q_len((ui.get_q_q_len() - 1).max(0));
                        }
                        DecAvifQueue => {
                            let ui = ui.unwrap();
                            ui.set_avif_q_len(ui.get_avif_q_len() - 1);
                        }
                        IncCannyQueue => {
                            let ui = ui.unwrap();
                            ui.set_canny_q_len(ui.get_canny_q_len() + 1);
                            ui.set_q_q_len((ui.get_q_q_len() - 1).max(0));
                        }
                        DecCannyQueue => {
                            let ui = ui.unwrap();
                            ui.set_canny_q_len(ui.get_canny_q_len() - 1);
                        }

                        PngSize(cache_entries, size) => {
                            let ui = ui.unwrap();

                            #[allow(clippy::collapsible_if)]
                            if cache_entries.thunk.entries().iter().any(|p1| {
                                ui.get_thunk_entries()
                                    .map(|x| x.path)
                                    .iter()
                                    .any(|p2| *p1.to_string_lossy() == *p2)
                            }) {
                                if ui.get_cropdetect() == cache_entries.render_details.cropdetect {
                                    let size = size
                                        .iter()
                                        .cloned()
                                        .map(|x| x.try_into().unwrap_or(i32::MAX))
                                        .collect::<Vec<_>>();
                                    let mut gui_entries =
                                        ui.get_thunk_entries().iter().collect::<Vec<_>>();

                                    for gui_entry in gui_entries.iter_mut() {
                                        let matching_entry = cache_entries
                                            .thunk
                                            .entries()
                                            .iter()
                                            .cloned()
                                            .enumerate()
                                            .find(|(_idx, cache_entry)| {
                                                cache_entry.to_string_lossy()
                                                    == gui_entry.path.to_string()
                                            });
                                        if let Some((cache_entry_idx, _cache_entry)) =
                                            matching_entry
                                        {
                                            gui_entry.png_size = ByteSize::b(
                                                size.get(cache_entry_idx).cloned().unwrap_or(0)
                                                    as u64,
                                            )
                                            .to_string()
                                            .into();

                                            gui_entry.png_size_int = size
                                                .get(cache_entry_idx)
                                                .cloned()
                                                .unwrap_or(0)
                                                .try_into()
                                                .unwrap_or(i32::MAX);
                                        }
                                    }
                                    do_png_goodness(&mut gui_entries, &size);
                                    ui.set_thunk_entries(ModelRc::new(VecModel::from(gui_entries)));
                                }
                            }
                        }

                        AvifSize(cache_entries, size) => {
                            let ui = ui.unwrap();

                            #[allow(clippy::collapsible_if)]
                            if cache_entries.thunk.entries().iter().any(|p1| {
                                ui.get_thunk_entries()
                                    .map(|x| x.path)
                                    .iter()
                                    .any(|p2| *p1.to_string_lossy() == *p2)
                            }) {
                                if ui.get_cropdetect() == cache_entries.render_details.cropdetect {
                                    let size = size
                                        .iter()
                                        .cloned()
                                        .map(|x| x.try_into().unwrap_or(i32::MAX))
                                        .collect::<Vec<_>>();
                                    let mut gui_entries =
                                        ui.get_thunk_entries().iter().collect::<Vec<_>>();

                                    for gui_entry in gui_entries.iter_mut() {
                                        let matching_entry = cache_entries
                                            .thunk
                                            .entries()
                                            .iter()
                                            .cloned()
                                            .enumerate()
                                            .find(|(_idx, cache_entry)| {
                                                cache_entry.to_string_lossy()
                                                    == gui_entry.path.to_string()
                                            });
                                        if let Some((cache_entry_idx, _cache_entry)) =
                                            matching_entry
                                        {
                                            gui_entry.avif_size = ByteSize::b(
                                                size.get(cache_entry_idx).cloned().unwrap_or(0)
                                                    as u64,
                                            )
                                            .to_string()
                                            .into();

                                            gui_entry.avif_size_int = size
                                                .get(cache_entry_idx)
                                                .cloned()
                                                .unwrap_or(0)
                                                .try_into()
                                                .unwrap_or(i32::MAX);
                                        }
                                    }
                                    do_avif_goodness(&mut gui_entries, &size);
                                    ui.set_thunk_entries(ModelRc::new(VecModel::from(gui_entries)));
                                }
                            }
                        }

                        JpgSize(cache_entries, size) => {
                            let ui = ui.unwrap();

                            #[allow(clippy::collapsible_if)]
                            if cache_entries.thunk.entries().iter().any(|p1| {
                                ui.get_thunk_entries()
                                    .map(|x| x.path)
                                    .iter()
                                    .any(|p2| *p1.to_string_lossy() == *p2)
                            }) {
                                if ui.get_cropdetect() == cache_entries.render_details.cropdetect {
                                    let size = size
                                        .iter()
                                        .cloned()
                                        .map(|x| x.try_into().unwrap_or(i32::MAX))
                                        .collect::<Vec<_>>();
                                    let mut gui_entries =
                                        ui.get_thunk_entries().iter().collect::<Vec<_>>();

                                    for gui_entry in gui_entries.iter_mut() {
                                        let matching_entry = cache_entries
                                            .thunk
                                            .entries()
                                            .iter()
                                            .cloned()
                                            .enumerate()
                                            .find(|(_idx, cache_entry)| {
                                                cache_entry.to_string_lossy()
                                                    == gui_entry.path.to_string()
                                            });
                                        if let Some((cache_entry_idx, _cache_entry)) =
                                            matching_entry
                                        {
                                            gui_entry.jpg_size = ByteSize::b(
                                                size.get(cache_entry_idx).cloned().unwrap_or(0)
                                                    as u64,
                                            )
                                            .to_string()
                                            .into();

                                            gui_entry.jpg_size_int = size
                                                .get(cache_entry_idx)
                                                .cloned()
                                                .unwrap_or(0)
                                                .try_into()
                                                .unwrap_or(i32::MAX);
                                        }
                                    }
                                    do_jpg_goodness(&mut gui_entries, &size);
                                    ui.set_thunk_entries(ModelRc::new(VecModel::from(gui_entries)));
                                }
                            }
                        }

                        CannySize(cache_entries, size) => {
                            let ui = ui.unwrap();

                            #[allow(clippy::collapsible_if)]
                            if cache_entries.thunk.entries().iter().any(|p1| {
                                ui.get_thunk_entries()
                                    .map(|x| x.path)
                                    .iter()
                                    .any(|p2| *p1.to_string_lossy() == *p2)
                            }) {
                                if ui.get_cropdetect() == cache_entries.render_details.cropdetect {
                                    let size = size
                                        .iter()
                                        .cloned()
                                        .map(|x| x.try_into().unwrap_or(i32::MAX))
                                        .collect::<Vec<_>>();
                                    let mut gui_entries =
                                        ui.get_thunk_entries().iter().collect::<Vec<_>>();

                                    for gui_entry in gui_entries.iter_mut() {
                                        let matching_entry = cache_entries
                                            .thunk
                                            .entries()
                                            .iter()
                                            .cloned()
                                            .enumerate()
                                            .find(|(_idx, cache_entry)| {
                                                cache_entry.to_string_lossy()
                                                    == gui_entry.path.to_string()
                                            });
                                        if let Some((cache_entry_idx, _cache_entry)) =
                                            matching_entry
                                        {
                                            gui_entry.canny_size = ByteSize::b(
                                                size.get(cache_entry_idx).cloned().unwrap_or(0)
                                                    as u64,
                                            )
                                            .to_string()
                                            .into();

                                            gui_entry.canny_size_int = size
                                                .get(cache_entry_idx)
                                                .cloned()
                                                .unwrap_or(0)
                                                .try_into()
                                                .unwrap_or(i32::MAX);
                                        }
                                    }
                                    do_canny_goodness(&mut gui_entries, &size);
                                    ui.set_thunk_entries(ModelRc::new(VecModel::from(gui_entries)));
                                }
                            }
                        }

                        FileSize(cache_entries, size) => {
                            let ui = ui.unwrap();

                            #[allow(clippy::collapsible_if)]
                            if cache_entries.thunk.entries().iter().any(|p1| {
                                ui.get_thunk_entries()
                                    .map(|x| x.path)
                                    .iter()
                                    .any(|p2| *p1.to_string_lossy() == *p2)
                            }) {
                                if ui.get_cropdetect() == cache_entries.render_details.cropdetect {
                                    let size = size
                                        .iter()
                                        .cloned()
                                        .map(|x| x.try_into().unwrap_or(i32::MAX))
                                        .collect::<Vec<_>>();
                                    let mut gui_entries =
                                        ui.get_thunk_entries().iter().collect::<Vec<_>>();

                                    for gui_entry in gui_entries.iter_mut() {
                                        let matching_entry = cache_entries
                                            .thunk
                                            .entries()
                                            .iter()
                                            .cloned()
                                            .enumerate()
                                            .find(|(_idx, cache_entry)| {
                                                cache_entry.to_string_lossy()
                                                    == gui_entry.path.to_string()
                                            });
                                        if let Some((cache_entry_idx, _cache_entry)) =
                                            matching_entry
                                        {
                                            gui_entry.file_size = ByteSize::b(
                                                size.get(cache_entry_idx).cloned().unwrap_or(0)
                                                    as u64,
                                            )
                                            .to_string()
                                            .into();
                                        }
                                    }
                                    ui.set_thunk_entries(ModelRc::new(VecModel::from(gui_entries)));
                                }
                            }
                        }

                        VidDuration(cache_entries, durations) => {
                            let ui = ui.unwrap();

                            #[allow(clippy::collapsible_if)]
                            if cache_entries.thunk.entries().iter().any(|p1| {
                                ui.get_thunk_entries()
                                    .map(|x| x.path)
                                    .iter()
                                    .any(|p2| *p1.to_string_lossy() == *p2)
                            }) {
                                if ui.get_cropdetect() == cache_entries.render_details.cropdetect {
                                    let mut gui_entries =
                                        ui.get_thunk_entries().iter().collect::<Vec<_>>();

                                    for gui_entry in gui_entries.iter_mut() {
                                        let matching_entry = cache_entries
                                            .thunk
                                            .entries()
                                            .iter()
                                            .cloned()
                                            .enumerate()
                                            .find(|(_idx, cache_entry)| {
                                                cache_entry.to_string_lossy()
                                                    == gui_entry.path.to_string()
                                            });
                                        if let Some((cache_entry_idx, _cache_entry)) =
                                            matching_entry
                                        {
                                            let mut dur = durations
                                                .get(cache_entry_idx)
                                                .cloned()
                                                .unwrap_or_else(|| Duration::from_secs(0))
                                                .as_secs();

                                            let hours = dur / 3600;
                                            dur -= hours * 3600;

                                            let mins = dur / 60;
                                            dur -= mins * 60;

                                            let secs = dur;

                                            gui_entry.vid_duration =
                                                format!("{hours:>02}:{mins:>02}:{secs:>02}").into();
                                        }
                                    }
                                    ui.set_thunk_entries(ModelRc::new(VecModel::from(gui_entries)));
                                }
                            }
                        }

                        VidResolution(cache_entries, resolutions) => {
                            let ui = ui.unwrap();

                            #[allow(clippy::collapsible_if)]
                            if cache_entries.thunk.entries().iter().any(|p1| {
                                ui.get_thunk_entries()
                                    .map(|x| x.path)
                                    .iter()
                                    .any(|p2| *p1.to_string_lossy() == *p2)
                            }) {
                                if ui.get_cropdetect() == cache_entries.render_details.cropdetect {
                                    let mut gui_entries =
                                        ui.get_thunk_entries().iter().collect::<Vec<_>>();

                                    for gui_entry in gui_entries.iter_mut() {
                                        let matching_entry = cache_entries
                                            .thunk
                                            .entries()
                                            .iter()
                                            .cloned()
                                            .enumerate()
                                            .find(|(_idx, cache_entry)| {
                                                cache_entry.to_string_lossy()
                                                    == gui_entry.path.to_string()
                                            });
                                        if let Some((cache_entry_idx, _cache_entry)) =
                                            matching_entry
                                        {
                                            let (x, y) =
                                                resolutions.get(cache_entry_idx).unwrap_or(&(0, 0));
                                            gui_entry.vid_resolution = format!("{x}x{y}",).into();
                                        }
                                    }
                                    ui.set_thunk_entries(ModelRc::new(VecModel::from(gui_entries)));
                                }
                            }
                        }
                    }
                })
                .unwrap();
            }
        }
    });

    issue_prerender_commands(&ui.as_weak(), &thunks, &gui_cmd_tx.clone());
    ui.invoke_focus_default();
    ui.set_max_idx((thunks.len() - 1).max(0) as i32);
    ui.run()
}

fn incr_curr_vid(ui: &Weak<MainWindow>, thunks: &[ResolutionThunk]) -> i32 {
    let ui = ui.unwrap();
    let mut curr_thunk = ui.get_thunk_idx() as usize;

    if thunks.get(curr_thunk).is_none() {
        curr_thunk = 0;
    }

    let start_vid = ui.get_curr_vid();
    let max_vid = thunks.get(curr_thunk).unwrap().entries().len() - 1;

    let mut curr_vid = start_vid;
    loop {
        let next_vid = Modulo::new(curr_vid as u64, max_vid as u64 + 1)
            .add(1)
            .val() as i32;
        // dbg!(start_vid, max_vid, curr_vid, next_vid);
        if next_vid == start_vid {
            return start_vid;
        } else if ui
            .get_thunk_entries()
            .row_data(next_vid as usize)
            .map(|x| x.enabled)
            .unwrap_or(true)
        {
            return next_vid;
        } else {
            curr_vid = next_vid;
        }
    }
}

fn decr_curr_vid(ui: &Weak<MainWindow>, thunks: &[ResolutionThunk]) -> i32 {
    let ui = ui.unwrap();
    let mut curr_thunk = ui.get_thunk_idx() as usize;

    if thunks.get(curr_thunk).is_none() {
        curr_thunk = 0;
    }

    let start_vid = ui.get_curr_vid();
    let max_vid = thunks.get(curr_thunk).unwrap().entries().len() - 1;

    let mut curr_vid = start_vid;
    loop {
        let next_vid = Modulo::new(curr_vid as u64, max_vid as u64 + 1)
            .sub(1)
            .val() as i32;
        // dbg!(start_vid, max_vid, curr_vid, next_vid);
        if next_vid == start_vid {
            return start_vid;
        } else if ui
            .get_thunk_entries()
            .row_data(next_vid as usize)
            .map(|x| x.enabled)
            .unwrap_or(true)
        {
            return next_vid;
        } else {
            curr_vid = next_vid;
        }
    }
}

fn gen_gui_data(
    imgs: Vec<SlintImage>,
    thunk: CacheEntry,
    ui: &Weak<MainWindow>,
) -> ModelRc<ThunkGuiData> {
    let ui = ui.unwrap();
    let view_many = ui.get_view_many();
    let curr_vid = ui.get_curr_vid();

    let mut imgs = imgs
        .into_iter()
        .zip(thunk.thunk.entries())
        .enumerate()
        .map(|(i, (thumb_imgbuf, entry))| {
            let image = slint::Image::from_rgb8(thumb_imgbuf);

            #[allow(clippy::cast_lossless)]
            let aspect_ratio = (image.size().width as f64 / image.size().height as f64) as f32;

            ThunkGuiData {
                path: SharedString::from(entry.to_string_lossy().to_string()),
                idx: i as i32,
                enabled: true,
                thumb: image,
                aspect_ratio,
                vid_duration: "00:00:00".into(),
                vid_resolution: "0x0".into(),
                file_size: "0".into(),
                png_size: "0".into(),
                png_size_int: 0,
                png_rank_proportion: 0.0,
                canny_size: "0".into(),
                canny_size_int: 0,
                canny_rank_proportion: 0.0,
                avif_size: "0".into(),
                avif_size_int: 0,
                avif_rank_proportion: 0.0,
                jpg_size: "0".into(),
                jpg_size_int: 0,
                jpg_rank_proportion: 0.0,
            }
        })
        .collect::<Vec<ThunkGuiData>>();

    let zeroes = &imgs.iter().map(|t| t.png_size_int).collect::<Vec<_>>();
    do_png_goodness(&mut imgs, zeroes);

    if !view_many {
        imgs = vec![imgs.get(curr_vid as usize).unwrap().clone()];
    }

    let x: ModelRc<ThunkGuiData> = ModelRc::new(VecModel::from(imgs));
    x
}

enum Direction {
    Forwards,
    Backwards,
}

fn issue_prerender_commands(
    ui: &Weak<MainWindow>,
    thunks: &[ResolutionThunk],
    cmd_tx: &crossbeam_channel::Sender<GuiCmd>,
) {
    let inner = || -> Option<()> {
        let ui = ui.unwrap();

        let idx = ui.get_thunk_idx() as usize;

        let thunk = thunks.get(idx)?.clone();

        let direction = if ui.get_is_forwards() {
            Direction::Forwards
        } else {
            Direction::Backwards
        };

        let details = RenderDetails {
            is_current: true,
            cropdetect: ui.get_cropdetect(),
        };

        let entry = CacheEntry {
            render_details: details,
            thunk,
        };

        cmd_tx.send(GuiCmd::Fetch(entry)).unwrap();

        //get the next and the previous
        //with opposite cropdetect
        {
            if let Some(next_thunk) = idx.checked_add(0).and_then(|idx| thunks.get(idx)) {
                let entry = CacheEntry {
                    render_details: RenderDetails {
                        is_current: false,
                        cropdetect: !ui.get_cropdetect(),
                    },
                    thunk: next_thunk.clone(),
                };
                cmd_tx.send(GuiCmd::Generate(entry)).unwrap();
            }
            if let Some(next_thunk) = idx.checked_add(1).and_then(|idx| thunks.get(idx)) {
                let entry = CacheEntry {
                    render_details: RenderDetails {
                        is_current: false,
                        cropdetect: ui.get_cropdetect(),
                    },
                    thunk: next_thunk.clone(),
                };
                cmd_tx.send(GuiCmd::Generate(entry)).unwrap();
            }
            if let Some(next_thunk) = idx.checked_sub(1).and_then(|idx| thunks.get(idx)) {
                let entry = CacheEntry {
                    render_details: RenderDetails {
                        is_current: false,
                        cropdetect: ui.get_cropdetect(),
                    },
                    thunk: next_thunk.clone(),
                };
                cmd_tx.send(GuiCmd::Generate(entry)).unwrap();
            }
            // if let Some(next_thunk) = idx.checked_add(1).and_then(|idx| thunks.get(idx)) {
            //     let entry = CacheEntry {
            //         render_details: RenderDetails {
            //             is_current: false,
            //             cropdetect: !ui.get_cropdetect(),
            //         },
            //         thunk: next_thunk.clone(),
            //     };
            //     cmd_tx.send(GuiCmd::Generate(entry)).unwrap();
            // }
            // if let Some(next_thunk) = idx.checked_sub(1).and_then(|idx| thunks.get(idx)) {
            //     let entry = CacheEntry {
            //         render_details: RenderDetails {
            //             is_current: false,
            //             cropdetect: !ui.get_cropdetect(),
            //         },
            //         thunk: next_thunk.clone(),
            //     };
            //     cmd_tx.send(GuiCmd::Generate(entry)).unwrap();
            // }
        }

        let max_thunk_idx = thunks.len() as u64 - 1;
        let fw_prefetch_idxs = (2..=5)
            .map(|i| Modulo::new(idx as u64, max_thunk_idx).add(i).val() as usize)
            .collect::<Vec<_>>();
        let bw_prefetch_idxs = (2..=2)
            .map(|i| Modulo::new(idx as u64, max_thunk_idx).sub(i).val() as usize)
            .collect::<Vec<_>>();

        let prefetch_cmds = match direction {
            Direction::Forwards => fw_prefetch_idxs.into_iter().chain(bw_prefetch_idxs),
            Direction::Backwards => bw_prefetch_idxs.into_iter().chain(fw_prefetch_idxs),
        }
        .filter_map(|i| thunks.get(i));

        for thunk in prefetch_cmds {
            let entry = CacheEntry {
                render_details: details,
                thunk: thunk.clone(),
            };
            cmd_tx.send(GuiCmd::Generate(entry)).unwrap();
        }

        Some(())
    };
    let _ = inner();
}

fn do_png_goodness(data: &mut [ThunkGuiData], all_sizes: &[i32]) {
    let png_ranks = all_sizes.iter().sorted().cloned().collect::<Vec<_>>();
    if png_ranks.iter().all(|r| *r == 0) {
        return;
    }
    // dbg!(&png_ranks);
    for datum in data {
        let png_size = datum.png_size_int;
        let rank = png_ranks
            .iter()
            .position(|rank| *rank == png_size)
            .unwrap_or(0);
        // dbg!(rank, png_size);
        datum.png_rank_proportion = rank as f32 * (1.0 / png_ranks.len() as f32);
    }
}

fn do_avif_goodness(data: &mut [ThunkGuiData], all_sizes: &[i32]) {
    let avif_ranks = all_sizes.iter().sorted().cloned().collect::<Vec<_>>();
    if avif_ranks.iter().all(|r| *r == 0) {
        return;
    }
    // dbg!(&avif_ranks);
    for datum in data {
        let avif_size = datum.avif_size_int;
        let rank = avif_ranks
            .iter()
            .position(|rank| *rank == avif_size)
            .unwrap_or(0);
        // dbg!(rank, avif_size);
        datum.avif_rank_proportion = rank as f32 * (1.0 / avif_ranks.len() as f32);
    }
}

fn do_jpg_goodness(data: &mut [ThunkGuiData], all_sizes: &[i32]) {
    let jpg_ranks = all_sizes.iter().sorted().cloned().collect::<Vec<_>>();
    if jpg_ranks.iter().all(|r| *r == 0) {
        return;
    }
    // dbg!(&jpg_ranks);
    for datum in data {
        let jpg_size = datum.jpg_size_int;
        let rank = jpg_ranks
            .iter()
            .position(|rank| *rank == jpg_size)
            .unwrap_or(0);
        // dbg!(rank, jpg_size);
        datum.jpg_rank_proportion = rank as f32 * (1.0 / jpg_ranks.len() as f32);
    }
}

fn do_canny_goodness(data: &mut [ThunkGuiData], all_sizes: &[i32]) {
    let canny_ranks = all_sizes.iter().sorted().cloned().collect::<Vec<_>>();
    if canny_ranks.iter().all(|r| *r == 0) {
        return;
    }
    // dbg!(&canny_ranks);
    for datum in data {
        let canny_size = datum.canny_size_int;
        let rank = canny_ranks
            .iter()
            .position(|rank| *rank == canny_size)
            .unwrap_or(0);
        // dbg!(rank, canny_size);
        datum.canny_rank_proportion = rank as f32 * (1.0 / canny_ranks.len() as f32);
    }
}
