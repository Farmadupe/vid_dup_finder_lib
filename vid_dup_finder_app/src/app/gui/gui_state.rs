use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    num::NonZeroU32,
    path::{Path, PathBuf},
    sync::RwLock,
    time::Duration,
};

use gdk_pixbuf::Pixbuf;
use glib::clone;
use gtk4::{prelude::*, Button};
use image::GrayImage;
#[cfg(feature = "parallel_loading")]
use rayon::iter::{ParallelBridge, ParallelIterator};

use ffmpeg_gst_wrapper::ffmpeg_gst;

use super::{
    gui_thumbnail_set::{GuiThumbnailSet, ThumbChoice},
    gui_zoom::{ZoomState, ZoomValue},
};
use crate::app::*;

pub struct GuiEntryState {
    stuffs: RwLock<HashMap<PathBuf, VideoStats>>,
    thumbs: GuiThumbnailSet,

    thumbs_pixbuf: Option<HashMap<PathBuf, Pixbuf>>,

    thunk: ResolutionThunk,
    single_mode: bool,
    entry_idx: usize,

    excludes: HashSet<PathBuf>,
}

impl GuiEntryState {
    pub fn new(
        thunk: ResolutionThunk,
        single_mode: bool,
        thumb_choice: ThumbChoice,
        zoom: ZoomState,
    ) -> Self {
        let info = thunk
            .entries()
            .into_iter()
            .map(|src_path| (src_path, thunk.hash(src_path)))
            .collect::<Vec<_>>();

        let thumbs = GuiThumbnailSet::new(info, zoom, thumb_choice);

        let mut ret = Self {
            stuffs: RwLock::new(HashMap::new()),
            thumbs,
            thumbs_pixbuf: None,
            thunk,
            single_mode,
            entry_idx: 0,
            excludes: HashSet::default(),
        };

        ret.regen_thumbs_pixbuf();

        //trace!("entry creation: single={}", ret.single_mode);

        ret
    }

    pub fn increment(&mut self) {
        if self.entry_idx < self.thunk.len() - 1 {
            self.entry_idx += 1;
        } else {
            self.entry_idx = 0;
        }

        let name_of_next = *self.thunk.entries().get(self.entry_idx).unwrap();
        if self.excludes.contains(name_of_next) {
            self.increment();
        }
    }

    pub fn decrement(&mut self) {
        if self.entry_idx > 0 {
            self.entry_idx -= 1;
        } else {
            self.entry_idx = self.thunk.len() - 1;
        }

        let name_of_next = *self.thunk.entries().get(self.entry_idx).unwrap();
        if self.excludes.contains(name_of_next) {
            self.decrement();
        }
    }

    pub fn set_single_mode(&mut self, val: bool) {
        self.single_mode = val;
        self.entry_idx = 0;
    }

    pub fn render_current_entry(&self) -> gtk4::Box {
        self.render_entry(self.entry_idx)
    }

    pub fn render(&self) -> gtk4::Box {
        if self.single_mode {
            self.render_current_entry()
        } else {
            self.render_whole_thunk()
        }
    }

    pub fn render_whole_thunk(&self) -> gtk4::Box {
        let entry_box = gtk4::Box::new(gtk4::Orientation::Vertical, 25);

        for (i, filename) in self.thunk.entries().iter().enumerate() {
            if !self.excludes.contains(*filename) {
                let row = self.render_entry(i);
                entry_box.append(&row);
            }
        }

        entry_box
    }

    pub fn distance(&self) -> String {
        match self.thunk.distance() {
            // Format the normalized distance as a percentage
            Some(distance) => {
                let similarity = ((1.0 - distance) * 100.0) as u32;
                format!("Similarity: {similarity}%")
            }
            None => "?????".to_string(),
        }
    }

    fn calc_png_size(src_path: &Path) -> Option<u64> {
        //iterate through the same frames from the video as are used to generate hashes.
        let frames = vid_dup_finder_lib::VideoHash::iterate_video_frames(
            src_path,
            vid_dup_finder_lib::DEFAULT_HASH_CREATION_OPTIONS.skip_forward_amount,
        )
        .ok()?;

        //to save time, only process every 8th frame.
        let frames = frames.skip(8).take(8);

        //resize the frames to a constant size for consistency between videos with different resolutions.
        let resize_and_get_len = |frame: GrayImage| -> Option<u64> {
            let size = NonZeroU32::try_from(500).expect("literal");
            let frame_constant_size =
                vid_dup_finder_common::resize_gray::resize_frame(frame, size, size);

            let mut buf = std::io::Cursor::new(vec![]);

            frame_constant_size
                .write_to(&mut buf, image::ImageOutputFormat::Png)
                .map(|()| buf.into_inner().len() as u64)
                .ok()
        };

        #[cfg(feature = "parallel_loading")]
        let total_size: Option<u64> = frames
            .par_bridge()
            .map(|x| resize_and_get_len(x.frame_owned()))
            .try_reduce(|| 0, |acc, curr| Some(acc + curr));

        #[cfg(not(feature = "parallel_loading"))]
        let total_size: Option<u64> = frames
            .map(|x| resize_and_get_len(x.frame_owned()))
            .reduce(|acc, next| match (acc, next) {
                (Some(val1), Some(val2)) => Some(val1 + val2),
                _ => None,
            })
            .flatten();

        total_size
    }

    fn calc_stats(src_path: &Path) -> Option<VideoStats> {
        let resolution = ffmpeg_gst::resolution(src_path);

        let duration: Option<Duration> = ffmpeg_gst::duration(src_path);

        let png_size = Self::calc_png_size(src_path);

        let ret = VideoStats {
            resolution,
            png_size,
            duration,
        };

        Some(ret)
    }

    fn render_entry(&self, i: usize) -> gtk4::Box {
        let entry_box = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
        let text_stack = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
        text_stack.set_size_request(300, -1);

        let entries = self.thunk.entries();
        let src_path = *entries.get(i).unwrap();

        let i_label = gtk4::Label::new(Some(&i.to_string()));
        i_label.set_width_chars(2);
        i_label.set_halign(gtk4::Align::Start);

        let winning_stats = self.thunk.calc_winning_stats(src_path);

        let ref_label = gtk4::Label::new(Some(if winning_stats.is_reference {
            "REF"
        } else {
            "   "
        }));
        ref_label.set_width_chars(3);

        let VideoStats {
            resolution,
            png_size,
            duration,
        } = if self.stuffs.read().unwrap().contains_key(src_path) {
            *self.stuffs.read().unwrap().get(src_path).unwrap()
        } else if let Some(stats) = Self::calc_stats(src_path) {
            self.stuffs
                .write()
                .unwrap()
                .insert(src_path.to_path_buf(), stats);

            stats
        } else {
            VideoStats {
                resolution: None,
                png_size: None,
                duration: None,
            }
        };

        let duration_text = match duration {
            Some(d) => {
                let total_secs = d.as_secs();

                let hours = total_secs / 3600;
                let mins = (total_secs % 3600) / 60;
                let secs = total_secs % 60;

                let hours_txt = if hours == 0 {
                    "".to_string()
                } else {
                    format!("{hours}:")
                };
                format!("{hours_txt}{mins:02}:{secs:02}")
            }
            None => "???".to_string(),
        };
        let duration_label = gtk4::Label::new(Some(&duration_text));
        duration_label.set_halign(gtk4::Align::Start);

        let pngsize_label =
            gtk4::Label::new(Some(if winning_stats.pngsize { "PNG" } else { "   " }));
        pngsize_label.set_width_chars(3);

        let res_label = gtk4::Label::new(Some(if winning_stats.res { "RES" } else { "   " }));
        res_label.set_width_chars(3);

        let png_size_text = match png_size {
            Some(png_size) => format!("{:>9}", png_size as f64 / 1000.0),
            None => "???".to_owned(),
        };
        let details_1 = format!("p_sz: {png_size_text}");
        let details_label_1 = gtk4::Label::new(Some(&details_1));
        details_label_1.set_halign(gtk4::Align::Start);

        let resolution_text = match resolution {
            Some((x, y)) => format!("({x}, {y})"),
            None => "???".to_owned(),
        };
        let details_2 = format!("res: {resolution_text}");
        let details_label_2 = gtk4::Label::new(Some(&details_2));
        details_label_2.set_halign(gtk4::Align::Start);

        let win_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);

        win_row.append(&ref_label);
        win_row.append(&pngsize_label);
        win_row.append(&res_label);
        text_stack.append(&i_label);
        text_stack.append(&win_row);
        text_stack.append(&duration_label);
        text_stack.append(&details_label_1);
        text_stack.append(&details_label_2);

        let button = Button::with_label(&src_path.to_string_lossy());
        button.set_halign(gtk4::Align::Start);
        let src_path = src_path.to_path_buf();
        button
            .connect_clicked(clone!(@strong src_path => move |_|Self::vlc_video_inner(&src_path)));

        let thumb = self.thumbs_pixbuf.as_ref().unwrap().get(&src_path).unwrap();

        let image = gtk4::Picture::for_pixbuf(thumb);
        image.set_can_shrink(false);
        image.set_keep_aspect_ratio(true);

        image.set_halign(gtk4::Align::Start);

        // image.set_height_request(900);
        // image.set_width_request(900);

        let text_then_image = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        text_then_image.append(&text_stack);
        text_then_image.append(&image);
        // text_then_image.set_height_request(300);
        // text_then_image.set_width_request(300);

        let separator = gtk4::Separator::new(gtk4::Orientation::Horizontal);

        entry_box.append(&separator);
        entry_box.append(&button);
        entry_box.append(&text_then_image);

        entry_box
    }

    pub fn set_zoom(&mut self, val: ZoomState) {
        self.thumbs.set_zoom(val);
        self.regen_thumbs_pixbuf();
    }

    pub fn set_choice(&mut self, val: ThumbChoice) {
        self.thumbs.set_choice(val);
        self.regen_thumbs_pixbuf();
    }

    pub fn vlc_video(&self, idx: usize) {
        if let Some(filename) = self.thunk.entries().get(idx) {
            Self::vlc_video_inner(filename);
        }
    }

    pub fn nautilus_file(&self, idx: usize) {
        if let Some(filename) = self.thunk.entries().get(idx) {
            Self::nautilus_file_inner(filename);
        }
    }

    pub fn exclude(&mut self, idx: usize) {
        if let Some(filename) = self.thunk.entries().get(idx) {
            if self.excludes.len() < self.thunk.entries().len() - 1 {
                self.excludes.insert(filename.to_path_buf());
            }
        }

        if idx == self.entry_idx {
            self.increment();
        }
    }

    pub fn include(&mut self, idx: usize) {
        trace!("Including video {}", idx);
        if let Some(filename) = self.thunk.entries().get(idx) {
            self.excludes.remove(*filename);
        }
    }

    pub fn resolve(&mut self, resolution: &str) {
        trace!("Resolving! with {}", resolution);
        if let Err(e) = self.thunk.resolve(resolution) {
            warn!("{}", e.to_string());
        }
    }

    pub fn vlc_all_slave(&self) {
        let mut path_iter = self.thunk.entries().into_iter();

        //let first_arg = shell_words::quote(&path_iter.next().unwrap()).to_string();
        let main_vid = path_iter.next().unwrap();
        let follow_vid = path_iter.next().unwrap();

        let mut follow_arg = OsString::from("--input_slave=");
        follow_arg.push(follow_vid);
        let mut command = std::process::Command::new("vlc");
        let command = command.arg(main_vid).arg(&follow_arg);

        if let Err(e) = command.spawn() {
            warn!(
                "Failed to start vlc at {}: {}",
                follow_arg.to_string_lossy(),
                e
            );
        }
    }

    pub fn vlc_all_seq(&self) {
        let mut command = std::process::Command::new("vlc");
        for entry in self.thunk.entries() {
            command.arg(entry);
        }

        if let Err(e) = command.spawn() {
            warn!("Failed to start vlc: {}", e);
        }
    }

    fn nautilus_file_inner(path: &Path) {
        if let Err(e) = std::process::Command::new("nautilus").arg(path).spawn() {
            warn!("Failed to start nautilus at {}: {}", path.display(), e);
        }
    }

    fn vlc_video_inner(path: &Path) {
        if let Err(e) = std::process::Command::new("vlc").arg(path).spawn() {
            warn!("Failed to start vlc at {}: {}", path.display(), e);
        }
    }

    fn regen_thumbs_pixbuf(&mut self) {
        self.thumbs_pixbuf = Some(self.thumbs.get_pixbufs());
    }
}

#[derive(Debug, PartialEq)]
enum KeypressState {
    Exclude,
    Include,
    View,
    JumpTo,
    Resolve,
    Nautilus,
    VlcAllSlave,
    VlcAllSeq,
}

#[derive(Clone, Copy, Debug)]
pub struct VideoStats {
    resolution: Option<(u32, u32)>,
    png_size: Option<u64>,
    duration: Option<Duration>,
}

pub struct GuiState {
    thunks: Vec<ResolutionThunk>,
    single_mode: bool,
    zoom: ZoomState,
    thumb_choice: ThumbChoice,
    thunk_idx: usize,
    current_thunk: GuiEntryState,
    keypress_string: String,
}

impl GuiState {
    pub fn new(thunks: Vec<ResolutionThunk>, single_mode: bool) -> Self {
        let default_zoom_state = ZoomState::new(50, 2000, 50, 50);

        let current_entry = GuiEntryState::new(
            thunks.first().unwrap().clone(),
            single_mode,
            ThumbChoice::Video,
            default_zoom_state,
        );

        Self {
            thunks,
            single_mode,
            zoom: default_zoom_state,
            thunk_idx: 0,
            current_thunk: current_entry,

            thumb_choice: ThumbChoice::Video,

            keypress_string: "".to_string(),
        }
    }

    pub fn next_thunk(&mut self) {
        self.thunk_idx = if (self.thunk_idx + 1) <= (self.last_thunk_idx()) {
            self.thunk_idx + 1
        } else {
            0
        };
        self.gen_thunk();
    }

    pub fn prev_thunk(&mut self) {
        self.thunk_idx = if self.thunk_idx > 0 {
            self.thunk_idx - 1
        } else {
            self.last_thunk_idx()
        };
        self.gen_thunk();
    }

    fn last_thunk_idx(&self) -> usize {
        self.thunks.len() - 1
    }

    pub fn render(&self) -> gtk4::Box {
        let b = gtk4::Box::new(gtk4::Orientation::Vertical, 6);

        let label_text = self.keypress_string.clone();

        let the_label = gtk4::Label::new(Some(&label_text));
        the_label.set_halign(gtk4::Align::Start);
        b.append(&the_label);

        let entries = self.current_thunk.render();
        b.append(&entries);

        b
    }

    pub fn increment_thunk_entry(&mut self) {
        self.current_thunk.increment();
    }

    pub fn decrement_thunk_entry(&mut self) {
        self.current_thunk.decrement();
    }

    pub fn set_single_mode(&mut self, val: bool) {
        self.single_mode = val;
        self.current_thunk.set_single_mode(self.single_mode);
    }

    pub const fn get_single_mode(&self) -> bool {
        self.single_mode
    }

    pub fn zoom_in(&mut self) {
        self.zoom = self.zoom.zoom_in();
        self.current_thunk.set_zoom(self.zoom);
    }

    pub fn zoom_out(&mut self) {
        self.zoom = self.zoom.zoom_out();
        self.current_thunk.set_zoom(self.zoom);
    }

    pub fn set_native(&mut self, val: bool) {
        self.zoom = self.zoom.set_native(val);
        self.current_thunk.set_zoom(self.zoom);
    }

    pub fn get_native(&self) -> bool {
        self.zoom.get() == ZoomValue::Native
    }

    pub fn set_view_spatial(&mut self, val: bool) {
        if val {
            self.thumb_choice = ThumbChoice::HashBits;
        } else {
            self.thumb_choice = ThumbChoice::Video;
        }

        self.current_thunk.set_choice(self.thumb_choice);
    }

    pub fn set_view_reddened(&mut self, val: bool) {
        if val {
            self.thumb_choice = ThumbChoice::Reddened;
        } else {
            self.thumb_choice = ThumbChoice::Video;
        }
        self.current_thunk.set_choice(self.thumb_choice);
    }

    pub fn set_view_rebuilt(&mut self, val: bool) {
        if val {
            self.thumb_choice = ThumbChoice::Rebuilt;
        } else {
            self.thumb_choice = ThumbChoice::Video;
        }
        self.current_thunk.set_choice(self.thumb_choice);
    }

    pub fn set_view_cropdetect(&mut self, val: bool) {
        if val {
            self.thumb_choice = ThumbChoice::CropdetectVideo;
        } else {
            self.thumb_choice = ThumbChoice::Video;
        }
        self.current_thunk.set_choice(self.thumb_choice);
    }

    fn thunk_idx_is_valid(&self, idx: usize) -> bool {
        idx < (self.thunks.len())
    }

    fn vid_idx_action(&mut self, action: KeypressState) {
        use KeypressState as Ks;

        let vid_idx = self.keypress_string.parse::<usize>().ok();

        #[rustfmt::skip]
        #[allow(clippy::let_unit_value)]
        let _ = match (action, vid_idx) {
            (Ks::Exclude,     Some(idx)) => self.current_thunk.exclude(idx),
            (Ks::Include,     Some(idx)) => self.current_thunk.include(idx),
            (Ks::View,        Some(idx)) => self.current_thunk.vlc_video(idx),
            (Ks::Resolve,     _)         => { self.current_thunk.resolve(&self.keypress_string); self.next_thunk(); }
            (Ks::Nautilus,    Some(idx)) => self.current_thunk.nautilus_file(idx),
            (Ks::VlcAllSeq,   _)         => self.current_thunk.vlc_all_seq(),
            (Ks::VlcAllSlave, _)         => self.current_thunk.vlc_all_slave(),
            (Ks::JumpTo,      Some(idx)) => {
                //println!("idx: {idx}, {}", self.thunk_idx_is_valid(idx));
                if self.thunk_idx_is_valid(idx) {
                    self.thunk_idx = idx;
                    self.gen_thunk();
                }
            }
            _ => {}
        };

        self.keypress_string.clear();
    }

    pub fn press_key(&mut self, key: gdk4::Key) {
        use gdk4::Key;
        use KeypressState as Ks;

        //println!("gui_state keypress: {key}");

        let mut push_key = || match key.to_unicode() {
            Some(k) => self.keypress_string.push(k),
            None => self.keypress_string.push(std::char::REPLACEMENT_CHARACTER),
        };

        match key {
            Key::i | Key::I => self.vid_idx_action(Ks::Include),
            Key::q | Key::Q => self.keypress_string.clear(),
            Key::j | Key::J => self.vid_idx_action(Ks::JumpTo),
            Key::k | Key::K => self.vid_idx_action(Ks::Resolve),
            Key::b | Key::B => self.vid_idx_action(Ks::VlcAllSlave),
            Key::m | Key::M => self.vid_idx_action(Ks::VlcAllSeq),
            Key::n | Key::N => self.vid_idx_action(Ks::Nautilus),
            Key::v | Key::V => self.vid_idx_action(Ks::View),
            Key::x | Key::X => self.vid_idx_action(Ks::Exclude),



            //for parsing "as" and "at"
            Key::a
            | Key::A
            | Key::s
            | Key::S
            | Key::t
            | Key::T

            //numbers
            | Key::KP_0
            | Key::KP_1
            | Key::KP_2
            | Key::KP_3
            | Key::KP_4
            | Key::KP_5
            | Key::KP_6
            | Key::KP_7
            | Key::KP_8
            | Key::KP_9
            | Key::_0
            | Key::_1
            | Key::_2
            | Key::_3
            | Key::_4
            | Key::_5
            | Key::_6
            | Key::_7
            | Key::_8
            | Key::_9 => push_key(),

            Key::space => push_key(),

            Key::BackSpace => {
                self.keypress_string.pop();
            }

            _ => {
                debug!("state: Unhandled keypress: {}", key);
            }
        }
    }

    pub const fn current_idx(&self) -> usize {
        self.thunk_idx
    }

    pub fn idx_len(&self) -> usize {
        self.thunks.len()
    }

    pub fn current_distance(&self) -> String {
        self.current_thunk.distance()
    }

    fn gen_thunk(&mut self) {
        trace!("Moving to thunk {}", self.thunk_idx);
        self.current_thunk = GuiEntryState::new(
            self.thunks.get(self.thunk_idx).unwrap().clone(),
            self.single_mode,
            self.thumb_choice,
            self.zoom,
        );
    }
}

// let hours = duration.hours();
// let minutes = duration.minutes() % 60;
// let seconds = duration.seconds() % 60;
// if hours != 0 {
//     format!("{hours}:{minutes:02}:{seconds:02}")
// } else {
//     format!("{minutes:02}:{seconds:02}")
// }

// .map(|acc| format!("{:>9}", acc as f64 / 1000.0))
//                         .ok()
