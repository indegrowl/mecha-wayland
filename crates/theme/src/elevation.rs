#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Elevation {
    /// 0 dp
    Level0 = 0,
    /// 1 dp
    Level1 = 1,
    /// 3 dp
    Level2 = 3,
    /// 6 dp
    Level3 = 6,
    /// 8 dp
    Level4 = 8,
    /// 12 dp
    Level5 = 12,
}

impl Elevation {
    #[inline]
    pub const fn value(self) -> u8 {
        self as u8
    }

    #[inline]
    pub const fn dp(self) -> f32 {
        self.value() as f32
    }
}
