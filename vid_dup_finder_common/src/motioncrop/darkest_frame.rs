use image::{GrayImage, Luma};
use imageproc::distance_transform::Norm;

use crate::{
    motioncrop::utils::{
        boolean_and, maskize_regions, regionize_image, regions_in_mask, retain_regions,
    },
    Crop,
};

/// Given a sequence of frames from a single video, keeps track of the darkest
/// pixel in each frame.
#[derive(Debug, Clone)]
pub struct DarkestFrame {
    frame: GrayImage,
    processed_frame: Option<GrayImage>,
}

impl DarkestFrame {
    pub fn try_from_seq(seq: &[GrayImage]) -> Option<Self> {
        let mut ret: Option<Self> = None;
        for frame in seq {
            match ret.as_mut() {
                Some(ref mut ret) => ret.add_frame(frame),
                None => ret = Some(Self::new(frame)),
            }
        }

        ret
    }

    pub fn new(first_frame: &GrayImage) -> Self {
        let (x, y) = first_frame.dimensions();

        let mut ret = Self {
            frame: GrayImage::from_pixel(x, y, Luma([255])),
            processed_frame: None,
        };
        ret.add_frame(first_frame);
        ret
    }

    pub fn add_frame(&mut self, frame: &GrayImage) {
        // Update the darkest pix image
        for (Luma([src_pix]), Luma([dark_pix])) in frame.pixels().zip(self.frame.pixels_mut()) {
            *dark_pix = *src_pix.min(dark_pix);
        }
        self.processed_frame = None;
    }

    fn postprocess(&mut self) {
        if self.processed_frame.is_some() {
            return;
        }
        let min_white = 210;

        //clamp all pixels above min_white to 255.
        let mut tmp = self.frame.clone();
        for Luma([pix]) in tmp.pixels_mut() {
            if *pix >= min_white {
                *pix = 0
            } else {
                *pix = 255
            }
        }
        let bin = imageproc::contrast::ThresholdType::Binary;
        imageproc::contrast::threshold_mut(&mut tmp, min_white - 1, bin);
        self.processed_frame = Some(tmp)
    }

    pub fn mask_out_area(&mut self, area: &Crop) {
        for (x, y) in area.enumerate_coords() {
            //dbg!(x, y);
            self.frame.put_pixel(x, y, Luma([255]))
        }
        self.processed_frame = None;
    }

    pub fn inner(&mut self) -> &GrayImage {
        self.postprocess();
        self.processed_frame.as_ref().unwrap()
    }

    pub fn largest_dark_region_with_motion(
        &mut self,
        motion_frame: &GrayImage,
    ) -> Option<GrayImage> {
        self.postprocess();
        let pp_frame = self.processed_frame.as_ref().unwrap();

        //open can destroy small images
        let erode_thr = (pp_frame.height() / 10).min(10);
        let erode_thr = u8::try_from(erode_thr).unwrap();

        let pp_frame = if pp_frame.height() > 100 {
            imageproc::morphology::open(pp_frame, Norm::LInf, erode_thr)
        } else {
            pp_frame.clone()
        };

        let anded_image = boolean_and(&pp_frame, motion_frame);

        let (pp_regions, _num_regions) = regionize_image(&pp_frame);
        let preserved_regions_idxs = regions_in_mask(&pp_regions, &anded_image);
        let preserved_regions = retain_regions(&pp_regions, &preserved_regions_idxs);

        let largest_region_idx = super::utils::largest_region(&preserved_regions)?;
        let largest_region = retain_regions(&preserved_regions, &[largest_region_idx]);
        let largest_region_masked = maskize_regions(&largest_region);
        Some(largest_region_masked)
    }
}
