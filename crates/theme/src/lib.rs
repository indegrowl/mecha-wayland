pub mod color;
pub mod elevation;
pub mod ext;
pub mod shape;
pub mod spacing;
pub mod typography;

pub use color::{ColorRole, ColorScheme};
pub use elevation::Elevation;
pub use ext::{
    AppThemeExt, ApplyTheme, ContextThemeExt, SpawnerThemeExt, ThemeChanged, ThemeReader,
};
pub use shape::Shape;
pub use spacing::Spacing;
pub use typography::{FontWeight, TextVariant, Typography, TypographyStyle};

use app::{App, Module};
use ext::on_theme_changed;
use geometry::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum ThemeMode {
    Light,
    #[default]
    Dark,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MechanixTheme {
    mode: ThemeMode,
    pub colors: ColorScheme,
    pub typography: Typography,
}

impl app::Resource for MechanixTheme {}

impl Default for MechanixTheme {
    #[inline]
    fn default() -> Self {
        Self::dark()
    }
}

impl MechanixTheme {
    pub fn new(mode: ThemeMode, colors: ColorScheme, typography: Typography) -> Self {
        Self {
            mode,
            colors,
            typography,
        }
    }

    pub fn dark() -> Self {
        Self {
            mode: ThemeMode::Dark,
            colors: ColorScheme::baseline_dark(),
            typography: Typography::baseline(),
        }
    }

    pub fn light() -> Self {
        Self {
            mode: ThemeMode::Light,
            colors: ColorScheme::baseline_light(),
            typography: Typography::baseline(),
        }
    }

    #[inline]
    pub const fn mode(&self) -> ThemeMode {
        self.mode
    }

    #[inline]
    pub fn color(&self, role: ColorRole) -> Color {
        role.resolve(&self.colors)
    }

    #[inline]
    pub fn typography(&self, variant: TextVariant) -> TypographyStyle {
        variant.resolve(&self.typography)
    }

    #[inline]
    pub fn with_colors(mut self, colors: ColorScheme) -> Self {
        self.colors = colors;
        self
    }

    #[inline]
    pub fn with_typography(mut self, typography: Typography) -> Self {
        self.typography = typography;
        self
    }
}

impl Module for MechanixTheme {
    fn install(self, app: &mut App) {
        app.insert_resource(self);
        app.system(on_theme_changed);
    }
}

pub mod prelude {
    pub use crate::ext::{AppThemeExt, ContextThemeExt, SpawnerThemeExt, ThemeReader};
    pub use crate::{
        ApplyTheme, ColorRole, ColorScheme, Elevation, FontWeight, MechanixTheme, Shape, Spacing,
        TextVariant, ThemeChanged, ThemeMode, Typography, TypographyStyle,
    };
}
