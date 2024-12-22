#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct Modulo {
    val: u64,
    size: u64,
}

impl Modulo {
    pub fn new(val: u64, size: u64) -> Self {
        // assert!(val <= size);
        Self { val, size }
    }

    pub fn add(self, rhs: u64) -> Self {
        let mut ret = u128::from(self.val).wrapping_add(u128::from(rhs));
        while ret >= u128::from(self.size) {
            ret = ret.wrapping_sub(u128::from(self.size));
        }

        Self {
            val: ret as u64,
            size: self.size,
        }
    }

    pub fn sub(self, rhs: u64) -> Self {
        let mut ret = u128::from(self.val).wrapping_sub(u128::from(rhs));
        while ret > u128::from(self.size) {
            ret = ret.wrapping_add(u128::from(self.size));
        }

        Self {
            val: ret as u64,
            size: self.size,
        }
    }

    pub fn val(self) -> u64 {
        self.val
    }
}

#[cfg(test)]
mod test {
    use crate::app::gui_slint::modulo::Modulo;

    #[test]
    fn test_1() {
        let m = Modulo::new(0, 2);
        assert_eq!(m.add(0).val(), 0);
        assert_eq!(m.add(1).val(), 1);
        assert_eq!(m.add(2).val(), 0);
        assert_eq!(m.add(3).val(), 1);
        assert_eq!(m.add(4).val(), 0);

        assert_eq!(m.sub(0).val(), 0);
        assert_eq!(m.sub(1).val(), 1);
        assert_eq!(m.sub(2).val(), 0);
        assert_eq!(m.sub(3).val(), 1);
        assert_eq!(m.sub(4).val(), 0);
    }

    #[test]
    fn test_2() {
        let m = Modulo::new(0, 3);
        assert_eq!(m.add(0).val(), 0);
        assert_eq!(m.add(1).val(), 1);
        assert_eq!(m.add(2).val(), 2);
        assert_eq!(m.add(3).val(), 0);
        assert_eq!(m.add(4).val(), 1);

        assert_eq!(m.sub(0).val(), 0);
        assert_eq!(m.sub(1).val(), 2);
        assert_eq!(m.sub(2).val(), 1);
        assert_eq!(m.sub(3).val(), 0);
        assert_eq!(m.sub(4).val(), 2);
    }
}
