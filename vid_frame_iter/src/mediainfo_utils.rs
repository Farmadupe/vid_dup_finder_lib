use gstreamer::ClockTime;
use gstreamer_pbutils::{Discoverer, DiscovererInfo};

fn media_info(uri: impl AsRef<str>) -> Result<DiscovererInfo, glib::Error> {
    let timeout = ClockTime::from_seconds(15);

    let ret = Discoverer::new(timeout)?.discover_uri(uri.as_ref())?;
    Ok(ret)
}

/// Get the duration of the given video file in seconds, or None
/// if the file contains no video streams.
pub fn duration(uri: impl AsRef<str>) -> Result<Option<std::time::Duration>, glib::Error> {
    //First find out if the file is actually a media file
    let info = match media_info(uri.as_ref()) {
        Ok(info) => info,
        Err(e) => {
            //println!("{e:?}");
            return Err(e);
        }
    };

    //Find out if the media file is actually a video file
    if info.video_streams().is_empty() {
        return Ok(None);
    }

    let ret = info
        .duration()
        .map(|duration| std::time::Duration::from_nanos(duration.nseconds()));

    Ok(ret)
}

/// Returns the dimensions of the video. or None if there are no video streams.
/// If there is more than one video stream, then the largest dimensions are returned.
pub fn dimensions(uri: impl AsRef<str>) -> Result<Option<(u32, u32)>, glib::Error> {
    //Iterate through all video streams, returning the stream with the largest resolution.
    let info = media_info(uri)?;
    let video_streams = info.video_streams().into_iter();

    let resolutions = video_streams.map(|vstream| (vstream.width(), vstream.height()));

    let ret = resolutions.reduce(|best_res, curr_res| {
        if curr_res.0 * curr_res.1 > best_res.0 * best_res.1 {
            curr_res
        } else {
            best_res
        }
    });

    Ok(ret)
}

//Get the frame rate of a video.
pub fn frame_rate(uri: impl AsRef<str>) -> Result<Option<f64>, glib::Error> {
    let info = media_info(uri)?;
    let ret = info
        .video_streams()
        .into_iter()
        .map(|s| {
            let frac = s.framerate();
            frac.numer() as f64 / frac.denom() as f64
        })
        .next();

    Ok(ret)
}

// struct ContainerIter {
//     x: Option<gstreamer_pbutils::DiscovererStreamInfo>,
// }

// impl Iterator for ContainerIter {
//     type Item = gstreamer_pbutils::DiscovererStreamInfo;

//     fn next(&mut self) -> Option<Self::Item> {
//         if let Some(x) = self.x.as_ref() {
//             self.x = x.next()
//         }

//         self.x.clone()
//     }
// }

// pub fn codec(uri: impl AsRef<str>) -> Result<Option<String>, glib::Error> {
//     let info = media_info(uri)?;

//     let calc_ret = move || -> Option<String> {
//         let mut ret = None;
//         let mut container_opt = info.stream_info();

//         while let Some(container) = container_opt {
//             let x = container
//                 .downcast::<DiscovererContainerInfo>()
//                 .expect("infallible");

//             for stream in x.streams() {
//                 let mut caps = stream.caps()?;
//                 let caps = caps.make_mut();

//                 let desc =
//                     gstreamer_pbutils::functions::pb_utils_get_codec_description(caps).to_string();

//                 ret = Some(desc);
//             }

//             container_opt = DiscovererContainerInfo::next(&x);
//         }
//         ret
//     };

//     Ok(calc_ret())
// }

// pub fn bit_rate(uri: impl AsRef<str>) -> Result<Option<u32>, glib::Error> {
//     let info = media_info(uri)?;

//     let ret = info.video_streams().into_iter().map(|s| s.bitrate()).next();

//     Ok(ret)
// }
