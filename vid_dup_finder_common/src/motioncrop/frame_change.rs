type GrayImageU16 = ImageBuffer<Luma<u16>, Vec<u16>>;

use image::{buffer::ConvertBuffer, GrayImage, ImageBuffer, Luma};
use imageproc::definitions::Image;

use super::utils::{largest_region, regionize_image, retain_regions};

#[derive(Debug, Clone)]
pub struct FrameChange {
    sum_frame: GrayImageU16,
    sum_frame_u8: Option<GrayImage>,
}

impl FrameChange {
    pub fn try_from_iter<'a>(
        iter: impl IntoIterator<Item = (&'a GrayImage, &'a GrayImage)>,
    ) -> Option<Self> {
        let mut ret: Option<Self> = None;
        for x in iter {
            match ret.as_mut() {
                Some(ref mut ret) => ret.add_frame(x),
                None => ret = Some(Self::new(x)),
            }
        }

        ret
    }

    pub fn new((frame_a, frame_b): (&GrayImage, &GrayImage)) -> Self {
        let width = frame_a.width();
        let height = frame_b.height();

        let mut ret = Self {
            sum_frame: GrayImageU16::new(width, height),
            sum_frame_u8: None,
        };
        ret.update_sum_frame(frame_a, frame_b);
        ret
    }

    //updates the movement intensity frame with the differences between a pair of images.
    fn update_sum_frame(&mut self, frame_a: &GrayImage, frame_b: &GrayImage) {
        //tweakable to ignore small differences (recording noise, compression noise etc)
        let thresh = 8;

        //NOTE: Hot code. This zip().zip() was the only way I could get the loop to vectorize
        for (&Luma([ref a_pix]), (&Luma([ref b_pix]), &mut Luma([ref mut sum_pix]))) in frame_a
            .pixels()
            .zip(frame_b.pixels().zip(self.sum_frame.pixels_mut()))
        {
            let diff = u16::from(a_pix.abs_diff(*b_pix));
            let diff = if diff >= thresh { diff } else { 0 };

            *sum_pix += diff;
        }

        self.sum_frame_u8 = None;
    }

    pub fn add_frame(&mut self, (frame_a, frame_b): (&GrayImage, &GrayImage)) {
        //check that the new frame is the same size as all previous frames.
        //Diff diff the two frames if so.

        assert!(frame_a.dimensions() == frame_b.dimensions());
        self.update_sum_frame(frame_a, frame_b);
    }

    fn postprocess(&mut self) {
        use imageproc::distance_transform::Norm::LInf;
        if self.sum_frame_u8.is_some() {
            return;
        }
        //after the last image has been inserted, we blur the sum_frame, and then
        //normalize it to fit into the full range.
        let mut tmp = self.sum_frame.clone().convert();
        normalize_u16(&mut tmp);
        let tmp = tmp.convert();
        let mut tmp = image::imageops::blur(&tmp, 2.0);
        let bin = imageproc::contrast::ThresholdType::Binary;
        imageproc::contrast::threshold_mut(&mut tmp, 20, bin);
        let tmp = imageproc::morphology::close(&tmp, LInf, 5);

        self.sum_frame_u8 = Some(tmp);
    }

    // pub fn inner_unblurred(&mut self) -> GrayImage {
    //     assert!(!self.processed);

    //     let mut temp_frame = self.sum_frame.clone();
    //     normalize_u16(&mut temp_frame);
    //     temp_frame.convert()
    // }

    pub fn largest_region_with_motion(&mut self) -> &GrayImage {
        self.postprocess();
        self.sum_frame_u8.as_ref().unwrap()
    }

    pub fn largest_area(&mut self) -> Option<Image<Luma<u32>>> {
        self.postprocess();
        let img = self.sum_frame_u8.as_ref()?;
        let (regions, _num_regions) = regionize_image(img);
        let biggest_region_idx = largest_region(&regions)?;
        let biggest_region = retain_regions(&regions, &[biggest_region_idx]);
        Some(biggest_region)
    }
}

fn normalize_u16(frame: &mut GrayImageU16) {
    fn brightest_pix_u16(img: &GrayImageU16) -> Luma<u16> {
        img.pixels()
            .copied()
            .reduce(|Luma([acc]), Luma([pix])| Luma([acc.max(pix)]))
            .unwrap()
    }

    fn darkest_pix_u16(img: &GrayImageU16) -> Luma<u16> {
        img.pixels()
            .copied()
            .reduce(|Luma([acc]), Luma([pix])| Luma([acc.min(pix)]))
            .unwrap()
    }
    let Luma([max]) = brightest_pix_u16(frame);
    let Luma([min]) = darkest_pix_u16(frame);
    let scaling_factor = f64::from(u16::MAX) / f64::from(min.abs_diff(max));

    for &mut Luma([ref mut pix]) in frame.pixels_mut() {
        let new_val = f64::from(*pix - min) * scaling_factor;
        let new_val = new_val.clamp(f64::from(u16::MIN), f64::from(u16::MAX));
        let new_val = new_val as u16;
        *pix = new_val;
    }
}
