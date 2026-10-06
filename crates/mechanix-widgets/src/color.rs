use geometry::Color;
use theme::{ColorRole, ThemeReader};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColorSource {
    Exact(Color),
    Role(ColorRole),
}

impl ColorSource {
    #[inline]
    pub fn resolve(self, reader: &impl ThemeReader) -> Color {
        match self {
            Self::Exact(color) => color,
            Self::Role(role) => reader.color(role),
        }
    }
}

impl From<Color> for ColorSource {
    #[inline]
    fn from(color: Color) -> Self {
        Self::Exact(color)
    }
}

impl From<ColorRole> for ColorSource {
    #[inline]
    fn from(role: ColorRole) -> Self {
        Self::Role(role)
    }
}

pub trait IntoColorSource {
    fn into_color_source(self) -> Option<ColorSource>;
}

impl IntoColorSource for ColorSource {
    #[inline]
    fn into_color_source(self) -> Option<ColorSource> {
        Some(self)
    }
}

impl IntoColorSource for Color {
    #[inline]
    fn into_color_source(self) -> Option<ColorSource> {
        Some(ColorSource::Exact(self))
    }
}

impl IntoColorSource for ColorRole {
    #[inline]
    fn into_color_source(self) -> Option<ColorSource> {
        Some(ColorSource::Role(self))
    }
}

impl IntoColorSource for Option<ColorSource> {
    #[inline]
    fn into_color_source(self) -> Option<ColorSource> {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_source(val: impl IntoColorSource) -> Option<ColorSource> {
        val.into_color_source()
    }

    #[test]
    fn conversions() {
        assert_eq!(
            ColorSource::from(Color::WHITE),
            ColorSource::Exact(Color::WHITE)
        );
        assert_eq!(
            ColorSource::from(ColorRole::Primary),
            ColorSource::Role(ColorRole::Primary)
        );

        assert_eq!(
            to_source(Color::WHITE),
            Some(ColorSource::Exact(Color::WHITE))
        );
        assert_eq!(
            to_source(ColorRole::Primary),
            Some(ColorSource::Role(ColorRole::Primary))
        );
        assert_eq!(
            to_source(ColorSource::Exact(Color::BLACK)),
            Some(ColorSource::Exact(Color::BLACK))
        );
        assert_eq!(
            to_source(Some(ColorSource::Role(ColorRole::Secondary))),
            Some(ColorSource::Role(ColorRole::Secondary))
        );
        assert_eq!(to_source(None), None);
    }
}
