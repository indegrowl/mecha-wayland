#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Spacing {
    /// 0 dp
    Space0 = 0,
    /// 2 dp
    Space25 = 2,
    /// 4 dp
    Space50 = 4,
    /// 6 dp
    Space75 = 6,
    /// 8 dp
    Space100 = 8,
    /// 10 dp
    Space125 = 10,
    /// 12 dp
    Space150 = 12,
    /// 14 dp
    Space175 = 14,
    /// 16 dp
    Space200 = 16,
    /// 20 dp
    Space250 = 20,
    /// 24 dp
    Space300 = 24,
    /// 32 dp
    Space400 = 32,
    /// 36 dp
    Space450 = 36,
    /// 40 dp
    Space500 = 40,
    /// 48 dp
    Space600 = 48,
    /// 56 dp
    Space700 = 56,
    /// 64 dp
    Space800 = 64,
    /// 72 dp
    Space900 = 72,
}

impl Spacing {
    #[inline]
    pub const fn value(self) -> u8 {
        self as u8
    }

    #[inline]
    pub const fn dp(self) -> f32 {
        self.value() as f32
    }
}
