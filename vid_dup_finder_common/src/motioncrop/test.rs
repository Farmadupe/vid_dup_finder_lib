use std::rc::Rc;

use crate::Crop;
use image::GrayImage;

use super::autocrop_frames::MotiondetectCrop;

//test that if there is nothing to crop due to static image, then nothing is returned
#[test]
fn test_nocrop() {
    #[rustfmt::skip]
    let pixen = [
        vec![
            255, 255, 255,
            255, 255, 255,
            255, 255, 255,
        ],
        vec![
            255, 255, 255,
            255, 255, 255,
            255, 255, 255,
        ]
    ];

    let imgs = util_generate_frames(3, 3, pixen);

    let exp = Some(Crop::from_edge_offsets((3, 3), 0, 0, 0, 0));
    let act = MotiondetectCrop::from_frames(imgs);

    assert_eq!(exp, act);
}

//test that if there is no motion, but there is a letterbox, then
//the letterbox is removed.
#[test]
fn test_letterbox_static() {
    #[rustfmt::skip]
    let pixen = [
        vec![
            0,   0,   0,   0, 0,
            0, 255, 255, 255, 0,
            0, 255, 255, 255, 0,
            0, 255, 255, 255, 0,
            0,   0,   0,   0, 0,
            0,   0,   0,   0, 0,
        ],
        vec![
            0,   0,   0,   0, 0,
            0, 255, 255, 255, 0,
            0, 255, 255, 255, 0,
            0, 255, 255, 255, 0,
            0,   0,   0,   0, 0,
            0,   0,   0,   0, 0,
        ]
    ];

    let imgs = util_generate_frames(5, 6, pixen);

    let exp = Some(Crop::from_edge_offsets((5, 6), 1, 1, 1, 2));
    let act = MotiondetectCrop::from_frames(imgs);

    assert_eq!(exp, act);
}

#[test]
fn test_2pixsquareinthemiddle() {
    #[rustfmt::skip]
    let pixen = [
        vec![
            255, 220, 220, 255,
            220,  80,  80, 220,
            220,  80,  80, 220,
            255, 255, 255, 255,
        ],
        vec![
            255, 220, 220, 255,
            220,  27,  27, 220,
            220,  27,  27, 220,
            255, 255, 255, 255,
        ]

    ];

    let imgs = util_generate_frames(4, 4, pixen);

    let exp = Some(Crop::from_edge_offsets((4, 4), 1, 1, 1, 1));
    let act = MotiondetectCrop::from_frames(imgs);

    assert_eq!(exp, act);
}

#[test]
fn test_prefer_bigger_region() {
    #[rustfmt::skip]
    let pixen = [
        vec![
            255, 220, 220, 255,
            220,  80, 255, 220,
            220, 255, 255, 220,
            255, 255, 255, 255,
            255, 220, 220, 255,
            220,  80,  80, 220,
            220,  80,  80, 220,
            255, 255, 255, 255,
        ],
        vec![
            255, 220, 220, 255,
            220,  20, 255, 220,
            220, 255, 255, 220,
            255, 255, 255, 255,
            255, 220, 220, 255,
            220,  20,  20, 220,
            220,  20,  20, 220,
            255, 255, 255, 255,
        ]
    ];

    let imgs = util_generate_frames(4, 8, pixen);

    let exp = Some(Crop::from_edge_offsets((4, 8), 1, 1, 5, 1));
    let act = MotiondetectCrop::from_frames(imgs);

    assert_eq!(exp, act);
}

#[test]
fn test_prefer_upper_region() {
    #[rustfmt::skip]
    let pixen = [
        vec![
            255, 220, 220, 255,
            220,  80,  80, 220,
            220, 255,  80, 220,
            255, 255, 255, 255,
            255, 220, 220, 255,
            220,  80,  80, 220,
            220,  80,  80, 220,
            255, 255, 255, 255,
        ],
        vec![
            255, 220, 220, 255,
            220,  20, 255, 220,
            220,  20, 255, 220,
            255, 255, 255, 255,
            255, 220, 220, 255,
            220,  20,  20, 220,
            220,  20,  20, 220,
            255, 255, 255, 255,
        ]
    ];

    let imgs = util_generate_frames(4, 8, pixen);

    let exp = Some(Crop::from_edge_offsets((4, 8), 1, 1, 1, 5));
    let act = MotiondetectCrop::from_frames(imgs);

    assert_eq!(exp, act);
}

#[test]
fn test_detect_topleft() {
    #[rustfmt::skip]
    let pixen = [
        vec![
            80 , 220, 220, 255,
            220, 255, 255, 220,
            220, 255, 255, 220,
            255, 255, 255, 255,
            255, 220, 220, 255,
            220, 255, 255, 220,
            220, 255, 255, 220,
            255, 220, 255, 255,
        ],
        vec![
             20, 220, 220, 255,
            220, 255, 255, 220,
            220, 255, 255, 220,
            255, 255, 255, 255,
            255, 220, 220, 255,
            220, 255, 255, 220,
            220, 255, 255, 220,
            255, 255, 255, 255,
        ]
    ];

    let imgs = util_generate_frames(4, 8, pixen);

    let exp = Some(Crop::from_edge_offsets((4, 8), 0, 3, 0, 7));
    let act = MotiondetectCrop::from_frames(imgs);

    assert_eq!(exp, act);
}

#[test]
fn test_detect_botright() {
    #[rustfmt::skip]
    let pixen = [
        vec![
            255, 220, 220, 255,
            220, 255, 255, 220,
            220, 255, 255, 220,
            255, 255, 255, 255,
            255, 220, 220, 255,
            220, 255, 255, 220,
            220, 255,  20,  20,
            255, 255,  20,  20,
        ],
        vec![
            255, 220, 220, 255,
            220, 255, 255, 220,
            220, 255, 255, 220,
            255, 255, 255, 255,
            255, 220, 220, 255,
            220, 255, 255, 220,
            220, 255,  40,  20,
            255, 255,  20,  40,
        ]
    ];

    let imgs = util_generate_frames(4, 8, pixen);

    let exp = Some(Crop::from_edge_offsets((4, 8), 2, 0, 6, 0));
    let act = MotiondetectCrop::from_frames(imgs);

    assert_eq!(exp, act);
}

//takes a series of vectors describing an image, and turns them into a sequence of images for running
//the autocrop algorithm over
fn util_generate_frames(
    x: u32,
    y: u32,
    pixen: impl IntoIterator<Item = Vec<u8>>,
) -> Vec<Rc<GrayImage>> {
    let raw = pixen
        .into_iter()
        .map(|pixen| GrayImage::from_vec(x, y, pixen).unwrap())
        .map(Rc::new)
        .collect::<Vec<_>>();

    raw.iter().cloned().cycle().take(2).collect()
}
