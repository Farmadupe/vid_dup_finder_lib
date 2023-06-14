#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomValue {
    User(u32),
    Native,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZoomState {
    user: ZoomRange,
    native: bool,
}

impl ZoomState {
    pub fn new(min: u32, max: u32, increment: u32, start: u32) -> Self {
        Self {
            user: ZoomRange::new(min, max, increment, start),
            native: false,
        }
    }

    pub const fn zoom_in(&self) -> Self {
        if self.native {
            *self
        } else {
            Self {
                user: self.user.zoom_in(),
                native: self.native,
            }
        }
    }

    pub const fn zoom_out(&self) -> Self {
        if self.native {
            *self
        } else {
            Self {
                user: self.user.zoom_out(),
                native: self.native,
            }
        }
    }

    pub const fn set_native(&self, val: bool) -> Self {
        Self {
            user: self.user,
            native: val,
        }
    }

    pub const fn get(&self) -> ZoomValue {
        if self.native {
            ZoomValue::Native
        } else {
            ZoomValue::User(self.user.get())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZoomRange {
    min: u32,
    max: u32,
    increment: u32,

    current: u32,
}

impl ZoomRange {
    pub fn new(min: u32, max: u32, increment: u32, start: u32) -> Self {
        //check max min and start are integer divislbe by increment.
        assert!((max % increment == 0) && (min % increment == 0) && (start % increment == 0));

        assert!(max >= min);

        assert!(start <= max);
        assert!(start >= min);

        Self {
            min,
            max,
            increment,
            current: start,
        }
    }

    pub const fn get(&self) -> u32 {
        self.current
    }

    // pub fn is_max(&self) -> bool {
    //     self.current >= self.max
    // }

    // pub fn max(&self) -> Self {
    //     Self {
    //         max: self.max,
    //         min: self.min,
    //         increment: self.increment,
    //         current: self.max,
    //     }
    // }

    pub const fn zoom_in(&self) -> Self {
        Self {
            max: self.max,
            min: self.min,
            increment: self.increment,
            current: if self.current + self.increment <= self.max {
                self.current + self.increment
            } else {
                self.max
            },
        }
    }

    pub const fn zoom_out(&self) -> Self {
        Self {
            max: self.max,
            min: self.min,
            increment: self.increment,
            current: if self.current - self.increment >= self.min {
                self.current - self.min
            } else {
                self.min
            },
        }
    }
}
