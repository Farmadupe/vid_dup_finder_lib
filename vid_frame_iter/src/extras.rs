//use gstreamer::prelude::*;

//Manually copy/pasted from https://gstreamer.freedesktop.org/documentation/nvcodec/?gi-language=c
static _NVIDIA_GPU_CODECS: [&str; 13] = [
    "nvav1dec",
    "nvh264dec",
    "nvh264sldec",
    "nvh265dec",
    "nvh265sldec",
    "nvjpegdec",
    "nvmpeg2videodec",
    "nvmpeg4videodec",
    "nvmpegvideodec",
    "nvvp8dec",
    "nvvp8sldec",
    "nvvp9dec",
    "nvvp9sldec",
];

/// Make gstreamer prefer using your Nvidia GPU to decode video files.
/// Has no effect if you do not have an Nvidia GPU.
/// This function goes not guarantee to always use an Nvidia GPU.
pub fn prioritize_nvidia_gpu_decoding() {
    // let registry = gstreamer::Registry::get();

    // for codec in NVIDIA_GPU_CODECS {
    //     use gstreamer::Rank::__Unknown;
    //     if let Some(feature) = registry.lookup_feature(codec) {
    //         feature.set_rank(__Unknown(99999));
    //     }
    // }
}

/// Make gstreamer avoid using your Nvidia GPU to decode video files.
/// Has no effect if you do not have an Nvidia GPU.
/// This function goes not guarantee to always avoid using an Nvidia GPU.
pub fn deprioritize_nvidia_gpu_decoding() {
    // let registry = gstreamer::Registry::get();

    // for codec in NVIDIA_GPU_CODECS {
    //     if let Some(feature) = registry.lookup_feature(codec) {
    //         feature.set_rank(gstreamer::Rank::None);
    //     }
    // }
}
