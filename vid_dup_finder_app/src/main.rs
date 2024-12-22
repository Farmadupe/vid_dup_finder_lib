#![allow(clippy::let_and_return)]
#![allow(clippy::len_without_is_empty)]
#![warn(clippy::cast_lossless)]
#![warn(clippy::print_stdout)]
#![warn(clippy::print_stderr)]
// #![warn(clippy::todo)]
// #![warn(clippy::unimplemented)]
// #![warn(clippy::unwrap_used)]
// #![warn(clippy::expect_used)]
//#![warn(clippy::pedantic)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]

#[macro_use]
extern crate log;

#[cfg(not(target_env = "msvc"))]
use jemallocator::Jemalloc;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

#[path = "comment_fix_issue_1/src/lib.rs"]
mod comment_fix_issue_1;

mod app;
mod video_hash_filesystem_cache;

fn main() {
    //seems to struggle on my machine with some file formats..
    //#[cfg(feature = "gstreamer_backend")]
    //ffmpeg_gst::deprioritize_nvidia_gpu_decoding();

    let return_code = app::run_app();
    std::process::exit(return_code)
}
