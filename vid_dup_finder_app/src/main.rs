#![warn(clippy::cast_lossless)]
#![allow(clippy::let_and_return)] // stylistic preference

#[macro_use]
extern crate log;

#[cfg(all(target_family = "unix", feature = "gui"))]
extern crate lazy_static;

// #[cfg(not(target_env = "msvc"))]
// use jemallocator::Jemalloc;
//
// #[cfg(not(target_env = "msvc"))]
// #[global_allocator]
// static GLOBAL: Jemalloc = Jemalloc;

mod app;

fn main() {
    vid_dup_finder_lib::init_gstreamer();

    //seems to struggle on my machine with some file formats..
    #[cfg(feature = "gstreamer_backend")]
    ffmpeg_gst::deprioritize_nvidia_gpu_decoding();

    let return_code = app::run_app();
    std::process::exit(return_code)
}
