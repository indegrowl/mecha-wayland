pub use widgets::WidgetState;

use geometry::Color;
use theme::{ColorRole, ColorScheme};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StateLayer {
    pub role: ColorRole,
    pub opacity: f32,
}

impl StateLayer {
    #[inline]
    pub const fn new(role: ColorRole, opacity: f32) -> Self {
        Self { role, opacity }
    }

    /// Resolves this layer into an overlay [`Color`] using the given colour scheme.
    #[inline]
    pub fn resolve(self, scheme: &ColorScheme) -> Color {
        let tint = self.role.resolve(scheme);
        Color::rgba(tint.r, tint.g, tint.b, self.opacity)
    }

    /// Blends this layer over `base_color` using the given colour scheme.
    #[inline]
    pub fn blend_over(self, base_color: Color, scheme: &ColorScheme) -> Color {
        self.resolve(scheme).over(base_color)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blends_over_base_color() {
        let scheme = ColorScheme::baseline_dark();
        let layer = StateLayer::new(ColorRole::Primary, 0.08);
        let blended = layer.blend_over(Color::BLACK, &scheme);
        assert_ne!(blended, Color::BLACK);
    }
}
