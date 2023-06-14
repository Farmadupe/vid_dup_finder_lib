#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Crop {
    orig_res: (u32, u32),
    left: u32,
    right: u32,
    top: u32,
    bottom: u32,
}

impl Crop {
    #[must_use]
    pub fn new(orig_res: (u32, u32), left: u32, right: u32, top: u32, bottom: u32) -> Self {
        Self {
            orig_res,
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

        let ret = Self::new(
            self.orig_res,
            min(self.left, other.left),
            min(self.right, other.right),
            min(self.top, other.top),
            min(self.bottom, other.bottom),
        );
        ret
        // match ret.validate() {
        //     Ok(ret) => ret,
        //     Err(()) => panic!("\n\ncrop union failed:\n    Self: {self:#?}, other: {other:#?}, ret: {ret:#?}"),
        // }
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
        let width = orig_width - (self.left + self.right);
        let height = orig_height - (self.top + self.bottom);

        (coord_x, coord_y, width, height)
    }

    // fn validate(self) -> Result<Self, ()> {
    //     let (width, height) = self.orig_res;
    //     let valid = self.left + self.right <= width && self.top + self.bottom <= height;
    //     if valid {
    //         Ok(self)
    //     } else {
    //         Err(())
    //     }
    // }
}

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
        // .validate()
        // .unwrap()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_as_view_args_nocrop() {
        let crop = Crop::new((100, 100), 0, 0, 0, 0);
        let exp: (u32, u32, u32, u32) = (0, 0, 100, 100);
        let act = crop.as_view_args();
        assert!(act == exp);
    }

    #[test]
    fn test_as_view_args_1pix_left() {
        let crop = Crop::new((100, 100), 1, 0, 0, 0);
        let exp: (u32, u32, u32, u32) = (1, 0, 99, 100);
        let act = crop.as_view_args();
        assert!(act == exp);
    }

    #[test]
    fn test_as_view_args_1pix_right() {
        let crop = Crop::new((100, 100), 0, 1, 0, 0);
        let exp: (u32, u32, u32, u32) = (0, 0, 99, 100);
        let act = crop.as_view_args();
        assert!(act == exp);
    }

    #[test]
    fn test_as_view_args_1pix_top() {
        let crop = Crop::new((100, 100), 0, 0, 1, 0);
        let exp: (u32, u32, u32, u32) = (0, 1, 100, 99);
        let act = crop.as_view_args();
        assert!(act == exp);
    }

    #[test]
    fn test_as_view_args_1pix_bot() {
        let crop = Crop::new((100, 100), 0, 0, 0, 1);
        let exp: (u32, u32, u32, u32) = (0, 0, 100, 99);
        let act = crop.as_view_args();
        assert!(act == exp);
    }

    #[test]
    fn test_as_view_args_four_values() {
        let crop = Crop::new((100, 100), 25, 25, 25, 25);
        let exp: (u32, u32, u32, u32) = (25, 25, 50, 50);
        let act = crop.as_view_args();
        assert!(act == exp);
    }

    #[test]
    fn test_as_view_args_noneleft() {
        let crop = Crop::new((100, 100), 50, 50, 50, 50);
        let exp: (u32, u32, u32, u32) = (50, 50, 0, 0);
        let act = crop.as_view_args();
        assert!(act == exp);
    }
}
