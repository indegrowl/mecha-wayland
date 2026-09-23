#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum FontWeight {
    Thin = 100,
    ExtraLight = 200,
    Light = 300,
    Regular = 400,
    Medium = 500,
    SemiBold = 600,
    Bold = 700,
    ExtraBold = 800,
}

impl FontWeight {
    #[inline]
    pub const fn value(self) -> u16 {
        self as u16
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypographyStyle {
    pub font_size: f32,
    pub line_height: f32,
    pub letter_spacing: f32,
    pub weight: FontWeight,
    pub weight_emphasised: FontWeight,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Typography {
    // Display
    pub display_large: TypographyStyle,
    pub display_medium: TypographyStyle,
    pub display_small: TypographyStyle,

    // Headline
    pub headline_large: TypographyStyle,
    pub headline_medium: TypographyStyle,
    pub headline_small: TypographyStyle,

    // Title
    pub title_large: TypographyStyle,
    pub title_medium: TypographyStyle,
    pub title_small: TypographyStyle,

    // Body
    pub body_large: TypographyStyle,
    pub body_medium: TypographyStyle,
    pub body_small: TypographyStyle,

    // Label
    pub label_large: TypographyStyle,
    pub label_medium: TypographyStyle,
    pub label_small: TypographyStyle,
}

impl Typography {
    pub const fn baseline() -> Self {
        Self {
            // Display
            display_large: TypographyStyle {
                font_size: 52.0,
                line_height: 66.0,
                letter_spacing: 0.0,
                weight: FontWeight::Light,
                weight_emphasised: FontWeight::Medium,
            },
            display_medium: TypographyStyle {
                font_size: 45.0,
                line_height: 52.0,
                letter_spacing: 0.0,
                weight: FontWeight::Light,
                weight_emphasised: FontWeight::Medium,
            },
            display_small: TypographyStyle {
                font_size: 36.0,
                line_height: 44.0,
                letter_spacing: 0.0,
                weight: FontWeight::Light,
                weight_emphasised: FontWeight::Medium,
            },

            // Headline
            headline_large: TypographyStyle {
                font_size: 32.0,
                line_height: 40.0,
                letter_spacing: 0.0,
                weight: FontWeight::Light,
                weight_emphasised: FontWeight::Regular,
            },
            headline_medium: TypographyStyle {
                font_size: 24.0,
                line_height: 36.0,
                letter_spacing: 0.0,
                weight: FontWeight::Light,
                weight_emphasised: FontWeight::Regular,
            },
            headline_small: TypographyStyle {
                font_size: 22.0,
                line_height: 26.0,
                letter_spacing: 0.0,
                weight: FontWeight::Light,
                weight_emphasised: FontWeight::Regular,
            },

            // Title
            title_large: TypographyStyle {
                font_size: 20.0,
                line_height: 26.0,
                letter_spacing: 0.0,
                weight: FontWeight::Regular,
                weight_emphasised: FontWeight::Medium,
            },
            title_medium: TypographyStyle {
                font_size: 18.0,
                line_height: 23.0,
                letter_spacing: 0.0,
                weight: FontWeight::Light,
                weight_emphasised: FontWeight::Regular,
            },
            title_small: TypographyStyle {
                font_size: 16.0,
                line_height: 20.0,
                letter_spacing: 1.0,
                weight: FontWeight::Light,
                weight_emphasised: FontWeight::Regular,
            },

            // Label
            label_large: TypographyStyle {
                font_size: 14.0,
                line_height: 20.0,
                letter_spacing: -0.5,
                weight: FontWeight::Regular,
                weight_emphasised: FontWeight::Medium,
            },
            label_medium: TypographyStyle {
                font_size: 12.0,
                line_height: 16.0,
                letter_spacing: 0.5,
                weight: FontWeight::Light,
                weight_emphasised: FontWeight::Regular,
            },
            label_small: TypographyStyle {
                font_size: 11.0,
                line_height: 14.0,
                letter_spacing: 0.5,
                weight: FontWeight::Light,
                weight_emphasised: FontWeight::Regular,
            },

            // Body
            body_large: TypographyStyle {
                font_size: 16.0,
                line_height: 22.0,
                letter_spacing: 0.0,
                weight: FontWeight::Regular,
                weight_emphasised: FontWeight::SemiBold,
            },
            body_medium: TypographyStyle {
                font_size: 14.0,
                line_height: 18.0,
                letter_spacing: -0.25,
                weight: FontWeight::Light,
                weight_emphasised: FontWeight::Medium,
            },
            body_small: TypographyStyle {
                font_size: 12.0,
                line_height: 16.0,
                letter_spacing: 0.4,
                weight: FontWeight::Light,
                weight_emphasised: FontWeight::Medium,
            },
        }
    }
}

impl Default for Typography {
    #[inline]
    fn default() -> Self {
        Self::baseline()
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum TextVariant {
    DisplayLarge,
    DisplayMedium,
    DisplaySmall,
    HeadlineLarge,
    HeadlineMedium,
    HeadlineSmall,
    TitleLarge,
    TitleMedium,
    TitleSmall,
    BodyLarge,
    #[default]
    BodyMedium,
    BodySmall,
    LabelLarge,
    LabelMedium,
    LabelSmall,
}

impl TextVariant {
    #[inline]
    pub fn resolve(self, typography: &Typography) -> TypographyStyle {
        match self {
            Self::DisplayLarge => typography.display_large,
            Self::DisplayMedium => typography.display_medium,
            Self::DisplaySmall => typography.display_small,
            Self::HeadlineLarge => typography.headline_large,
            Self::HeadlineMedium => typography.headline_medium,
            Self::HeadlineSmall => typography.headline_small,
            Self::TitleLarge => typography.title_large,
            Self::TitleMedium => typography.title_medium,
            Self::TitleSmall => typography.title_small,
            Self::BodyLarge => typography.body_large,
            Self::BodyMedium => typography.body_medium,
            Self::BodySmall => typography.body_small,
            Self::LabelLarge => typography.label_large,
            Self::LabelMedium => typography.label_medium,
            Self::LabelSmall => typography.label_small,
        }
    }
}
