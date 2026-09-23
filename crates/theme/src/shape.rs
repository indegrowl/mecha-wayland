#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Shape {
    #[default]
    None,
    /// 4dp
    ExtraSmall,
    /// 8dp
    Small,
    /// 12dp
    Medium,
    /// 16dp
    Large,
    /// 20dp
    LargeIncreased,
    /// 28dp
    ExtraLarge,
    /// 32dp
    ExtraLargeIncreased,
    /// 48dp
    ExtraExtraLarge,
    /// Fully rounded pill shape
    Full,
}

impl Shape {
    #[inline]
    pub fn resolve_radius_dp(self, component_height: f32) -> f32 {
        match self {
            Self::None => 0.0,
            Self::ExtraSmall => 4.0,
            Self::Small => 8.0,
            Self::Medium => 12.0,
            Self::Large => 16.0,
            Self::LargeIncreased => 20.0,
            Self::ExtraLarge => 28.0,
            Self::ExtraLargeIncreased => 32.0,
            Self::ExtraExtraLarge => 48.0,
            Self::Full => component_height / 2.0,
        }
    }
}
