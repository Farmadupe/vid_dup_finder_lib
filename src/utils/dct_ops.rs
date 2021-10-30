use image::DynamicImage;
use rustdct::DctPlanner;
use transpose::transpose_inplace;

use crate::definitions::*;

pub fn perform_dct(image: &DynamicImage) -> Vec<f64> {
    let mut raw_bytes = bytify(image);

    let dimension = (raw_bytes.len() as f64).sqrt() as usize;
    assert!(
        RESIZE_IMAGE_X as usize == dimension,
        "actual x: {}, RESIZE_IMAGE_X: {}",
        dimension,
        RESIZE_IMAGE_X
    );

    assert!(RESIZE_IMAGE_Y as usize == dimension);

    //setup the DCT.....
    let mut planner = DctPlanner::new();
    let dct = planner.plan_dct2(dimension);

    //perform round 1 of the DCT (on rows):
    raw_bytes.chunks_exact_mut(dimension).for_each(|row| {
        dct.process_dct2(row);
    });

    //now tranpose...
    let mut scratch = vec![0f64; dimension];
    transpose_inplace(&mut raw_bytes, &mut scratch, dimension, dimension);

    //perform round 1 of the DCT (on cols):
    raw_bytes.chunks_exact_mut(dimension).for_each(|col| {
        dct.process_dct2(col);
    });

    //now tranpose...
    transpose_inplace(&mut raw_bytes, &mut scratch, dimension, dimension);

    //and finally, normalize (has no effect, but may be useful in the future if further processing is required.)
    for val in raw_bytes.iter_mut() {
        *val *= 4f64 / (HASH_IMAGE_X as f64 * HASH_IMAGE_Y as f64);
    }

    raw_bytes
}

#[allow(dead_code)]
pub fn inverse_dct(raw_bytes: &[f64]) -> image::DynamicImage {
    let dimension = (raw_bytes.len() as f64).sqrt() as usize;
    let mut raw_bytes = raw_bytes.to_vec();

    //setup the DCT.....
    let mut planner = DctPlanner::new();
    let dct = planner.plan_dct3(dimension);

    //perform round 1
    raw_bytes.chunks_mut(dimension).for_each(|row| {
        dct.process_dct3(row);
    });

    //now tranpose...
    let mut scratch = vec![0f64; dimension];
    transpose::transpose_inplace(&mut raw_bytes, &mut scratch, dimension, dimension);

    //perform round 2 of the DCT
    raw_bytes.chunks_mut(dimension).for_each(|col| {
        dct.process_dct3(col);
    });

    //now tranpose...
    transpose::transpose_inplace(&mut raw_bytes, &mut scratch, dimension, dimension);

    imagify(&raw_bytes)
}

fn bytify(image: &DynamicImage) -> Vec<f64> {
    //Now extract the raw data, convert and scale into f64, in preparation for DCT.
    image
        .to_bytes()
        .into_iter()
        .map(|x| x as f64 - 128.0)
        .collect::<Vec<_>>()
}

#[allow(dead_code)]
fn imagify(raw_bytes: &[f64]) -> DynamicImage {
    let dimension = (raw_bytes.len() as f64).sqrt() as u32;
    //now build an image
    let image_bytes_u8 = raw_bytes
        .iter()
        //.map(|float_val| ((float_val + 1.0) * 128.0) as u8)
        .map(|float_val| {
            //println!("{}", float_val);
            (*float_val + 128.0) as u8
        })
        .collect::<Vec<_>>();

    let buf: image::ImageBuffer<image::Luma<u8>, Vec<u8>> =
        image::ImageBuffer::from_vec(dimension, dimension, image_bytes_u8).unwrap();

    DynamicImage::ImageLuma8(buf)
}

// pub(crate) fn window_dct(dct: &[f64], window_size: usize) -> Vec<f64> {
//     let dimension = (dct.len() as f64).sqrt() as usize;
//     let mut ret = Vec::with_capacity(dimension * dimension);

//     for row in dct.chunks(dimension).take(window_size) {
//         let slice = &row[0..window_size];
//         ret.extend(slice)
//     }

//     if ret.len() != window_size * window_size {
//         panic!(
//             "failed here: got: {}, expected: {}",
//             ret.len(),
//             window_size * window_size
//         )
//     }

//     ret
// }
