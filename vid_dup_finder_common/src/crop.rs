// use image::flat::SampleLayout;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Crop {
    pub orig_res: (u32, u32),
    pub left: u32,
    pub right: u32,
    pub top: u32,
    pub bottom: u32,
}

impl Crop {
    #[must_use]
    pub fn from_edge_offsets(
        orig_res: (u32, u32),
        left: u32,
        right: u32,
        top: u32,
        bottom: u32,
    ) -> Self {
        assert!((left + right) < orig_res.0);
        assert!((top + bottom) < orig_res.1);
        Self {
            orig_res,
            left,
            right,
            top,
            bottom,
        }
    }

    pub fn from_topleft_and_dims(
        (orig_width, orig_height): (u32, u32),
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> Self {
        let left = x;
        let right = orig_width - width - x;
        let top = y;
        let bottom = orig_height - height - y;
        Self {
            orig_res: (orig_width, orig_height),
            left,
            right,
            top,
            bottom,
        }
    }

    #[must_use]
    pub fn union(&self, other: &Self) -> Self {
        use std::cmp::min;

        if self.orig_res != other.orig_res {
            //assert!(self.orig_res == other.orig_res);
        }

        let ret = Self::from_edge_offsets(
            self.orig_res,
            min(self.left, other.left),
            min(self.right, other.right),
            min(self.top, other.top),
            min(self.bottom, other.bottom),
        );
        ret
    }

    #[must_use]
    pub fn biggest_crop(&self, other: &Self) -> Self {
        //println!("biggest_crop: Self: {self:#?}; Other: #{other:#?}");

        assert!(self.orig_res == other.orig_res);

        let t_w = self.right.abs_diff(self.left);
        let t_h = self.bottom.abs_diff(self.top);
        let t_dim = t_w * t_h;

        let o_w = other.right.abs_diff(other.left);
        let o_h = other.bottom.abs_diff(other.top);
        let o_dim = o_w * o_h;

        if t_dim < o_dim {
            *self
        } else {
            *other
        }
    }

    #[must_use]
    pub fn as_view_args(&self) -> (u32, u32, u32, u32) {
        let (orig_width, orig_height) = self.orig_res;
        let coord_x = self.left;
        let coord_y = self.top;
        // let width = orig_width - (self.left + self.right);
        // let height = orig_height - (self.top + self.bottom);

        let width = orig_width.checked_sub(self.left + self.right).unwrap();
        let height = orig_height.checked_sub(self.top + self.bottom).unwrap();

        (coord_x, coord_y, width, height)
    }

    pub fn width(&self) -> u32 {
        self.orig_res.0 - (self.left + self.right)
    }

    pub fn height(&self) -> u32 {
        self.orig_res.1 - (self.top + self.bottom)
    }

    pub fn area(&self) -> u32 {
        self.width() * self.height()
    }

    pub fn aspect_ratio(&self) -> f64 {
        f64::from(self.width()) / f64::from(self.height())
    }

    pub fn enumerate_coords(&self) -> impl Iterator<Item = (u32, u32)> {
        let (orig_x, orig_y) = self.orig_res;

        let first_x_pix = self.left;
        let last_x_pix = orig_x - self.right;
        let xs = first_x_pix..last_x_pix;

        let first_y_pix = self.top;
        let last_y_pix = orig_y - self.bottom;
        let ys = first_y_pix..last_y_pix;

        xs.flat_map(move |x| ys.clone().map(move |y| (x, y)))
    }

    pub fn enumerate_coords_excluded(&self) -> impl Iterator<Item = (u32, u32)> + use<> {
        let (orig_x, orig_y) = self.orig_res;

        let x0 = 0;
        let x1 = self.left;
        let x2 = orig_x - self.right;
        let x3 = orig_x;

        let y0 = 0;
        let y1 = self.top;
        let y2 = orig_y - self.bottom;
        let y3 = orig_y;

        //clockwise starting at topleft (tl)
        let tl = (x0..x1).flat_map(move |x| (y0..y1).map(move |y| (x, y)));
        let tm = (x1..x2).flat_map(move |x| (y0..y1).map(move |y| (x, y)));
        let tr = (x2..x3).flat_map(move |x| (y0..y1).map(move |y| (x, y)));
        let mr = (x2..x3).flat_map(move |x| (y1..y2).map(move |y| (x, y)));
        let bl = (x0..x1).flat_map(move |x| (y2..y3).map(move |y| (x, y)));
        let bm = (x1..x2).flat_map(move |x| (y2..y3).map(move |y| (x, y)));
        let br = (x2..x3).flat_map(move |x| (y2..y3).map(move |y| (x, y)));
        let ml = (x0..x1).flat_map(move |x| (y1..y2).map(move |y| (x, y)));

        tl.chain(tm.chain(tr.chain(mr.chain(bl.chain(bm.chain(br.chain(ml)))))))
    }

    pub fn eroded(self) -> Option<Self> {
        let mut ret = self;
        ret.left += 1;
        ret.right += 1;
        ret.top += 1;
        ret.bottom += 1;

        if ret.left + ret.right >= ret.orig_res.0 {
            return None;
        }
        if ret.top + ret.bottom >= ret.orig_res.1 {
            return None;
        }

        Some(ret)
    }

    pub fn is_uncropped(&self) -> bool {
        (self.left == 0) && (self.right == 0) && (self.top == 0) && (self.bottom == 0)
    }
}

//pub fn as_cropped(layout: SampleLayout, crop: Crop) -> SampleLayout {}

impl Default for Crop {
    //an arbitrary 'enormous' crop suitable for initializing a fold/reduce
    #[must_use]
    fn default() -> Self {
        Self {
            orig_res: (u32::MAX, u32::MAX),
            left: u32::MAX / 8,
            right: u32::MAX / 8,
            top: u32::MAX / 8,
            bottom: u32::MAX / 8,
        }
    }
}

#[cfg(test)]
mod test {
    use itertools::Itertools;

    use super::*;

    #[test]
    fn test_as_view_args_nocrop() {
        let crop = Crop::from_edge_offsets((100, 100), 0, 0, 0, 0);
        let exp: (u32, u32, u32, u32) = (0, 0, 100, 100);
        let act = crop.as_view_args();
        assert!(act == exp);
    }

    #[test]
    fn test_as_view_args_1pix_left() {
        let crop = Crop::from_edge_offsets((100, 100), 1, 0, 0, 0);
        let exp: (u32, u32, u32, u32) = (1, 0, 99, 100);
        let act = crop.as_view_args();
        assert!(act == exp);
    }

    #[test]
    fn test_as_view_args_1pix_right() {
        let crop = Crop::from_edge_offsets((100, 100), 0, 1, 0, 0);
        let exp: (u32, u32, u32, u32) = (0, 0, 99, 100);
        let act = crop.as_view_args();
        assert!(act == exp);
    }

    #[test]
    fn test_as_view_args_1pix_top() {
        let crop = Crop::from_edge_offsets((100, 100), 0, 0, 1, 0);
        let exp: (u32, u32, u32, u32) = (0, 1, 100, 99);
        let act = crop.as_view_args();
        assert!(act == exp);
    }

    #[test]
    fn test_as_view_args_1pix_bot() {
        let crop = Crop::from_edge_offsets((100, 100), 0, 0, 0, 1);
        let exp: (u32, u32, u32, u32) = (0, 0, 100, 99);
        let act = crop.as_view_args();
        assert!(act == exp);
    }

    #[test]
    fn test_as_view_args_four_values() {
        let crop = Crop::from_edge_offsets((100, 100), 25, 25, 25, 25);
        let exp: (u32, u32, u32, u32) = (25, 25, 50, 50);
        let act = crop.as_view_args();
        assert!(act == exp);
    }

    #[test]
    fn test_as_view_args_four_more() {
        let crop = Crop::from_edge_offsets((768, 432), 96, 96, 0, 0);
        let exp: (u32, u32, u32, u32) = (96, 0, 576, 432);
        let act = crop.as_view_args();
        assert!(act == exp);
    }

    // #[test]
    // #[should_panic]
    // fn test_as_view_args_noneleft() {
    //     let _ = Crop::from_edge_offsets((100, 100), 50, 50, 50, 50);
    // }

    #[test]
    fn test_from_offset_and_dims() {
        let crop = Crop::from_topleft_and_dims((100, 100), 11, 12, 13, 14);
        assert!(crop.as_view_args() == (11, 12, 13, 14));
    }

    #[test]
    fn test_enumerate_coords_nocrop() {
        let crop = Crop::from_edge_offsets((3, 3), 0, 0, 0, 0);
        assert!(crop.enumerate_coords().count() == 9);
        assert!(crop.enumerate_coords_excluded().count() == 0);
    }

    #[test]
    fn test_enumerate_coords_1pixinthemiddle() {
        let crop = Crop::from_edge_offsets((3, 3), 1, 1, 1, 1);

        //test included
        {
            let exp = vec![(1, 1)];
            let act = crop.enumerate_coords().collect::<Vec<_>>();
            assert_eq!(exp, act);
        }

        //test exluced
        {
            #[rustfmt::skip]
            let exp = vec![
                (0, 0), (1, 0), (2, 0),
                (0, 1),         (2, 1),
                (0, 2), (1, 2), (2, 2),
            ].into_iter().sorted().collect::<Vec<_>>();
            let act = crop
                .enumerate_coords_excluded()
                .sorted()
                .collect::<Vec<_>>();

            assert!(exp == act);
        }
    }

    #[test]
    fn test_enumerate_coords_1pixinthetop() {
        let crop = Crop::from_edge_offsets((3, 3), 1, 1, 0, 2);

        //test included
        {
            let exp = vec![(1, 0)];
            let act = crop.enumerate_coords().collect::<Vec<_>>();
            assert_eq!(exp, act);
        }

        //test exluced
        {
            #[rustfmt::skip]
            let exp = vec![
                (0, 0),         (2, 0),
                (0, 1), (1, 1), (2, 1),
                (0, 2), (1, 2), (2, 2),
            ].into_iter().sorted().collect::<Vec<_>>();
            let act = crop
                .enumerate_coords_excluded()
                .sorted()
                .collect::<Vec<_>>();

            assert_eq!(exp, act);
        }
    }

    #[test]
    fn test_enumerate_coords_1pixintheright() {
        let crop = Crop::from_edge_offsets((3, 3), 2, 0, 2, 0);

        //(also test alternate constructor)
        let other_crop = Crop::from_topleft_and_dims((3, 3), 2, 2, 1, 1);
        assert_eq!(crop, other_crop);

        //test included
        {
            let exp = vec![(2, 2)];
            let act = crop.enumerate_coords().collect::<Vec<_>>();
            assert_eq!(exp, act);
        }

        //test exluced
        {
            #[rustfmt::skip]
            let exp = vec![
                (0, 0), (1, 0), (2, 0),
                (0, 1), (1, 1), (2, 1),
                (0, 2), (1, 2),
            ].into_iter().sorted().collect::<Vec<_>>();
            let act = crop
                .enumerate_coords_excluded()
                .sorted()
                .collect::<Vec<_>>();

            assert_eq!(exp, act);
        }
    }
}
