use geometry::Color;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorScheme {
    // Primary
    pub primary: Color,
    pub on_primary: Color,
    pub primary_container: Color,
    pub on_primary_container: Color,

    // Secondary
    pub secondary: Color,
    pub on_secondary: Color,
    pub secondary_container: Color,
    pub on_secondary_container: Color,

    // Error
    pub error: Color,
    pub on_error: Color,
    pub error_container: Color,
    pub on_error_container: Color,

    // Success
    pub success: Color,
    pub on_success: Color,
    pub success_container: Color,
    pub on_success_container: Color,

    // Fixed
    pub primary_fixed: Color,
    pub primary_fixed_dim: Color,
    pub on_primary_fixed: Color,
    pub on_primary_fixed_variant: Color,

    pub secondary_fixed: Color,
    pub secondary_fixed_dim: Color,
    pub on_secondary_fixed: Color,
    pub on_secondary_fixed_variant: Color,

    // Surface
    pub surface_dim: Color,
    pub surface: Color,
    pub surface_bright: Color,

    pub surface_container_lowest: Color,
    pub surface_container_low: Color,
    pub surface_container: Color,
    pub surface_container_high: Color,
    pub surface_container_highest: Color,

    pub on_surface: Color,
    pub on_surface_variant: Color,

    // Outline
    pub outline: Color,
    pub outline_variant: Color,

    // Inverse
    pub inverse_surface: Color,
    pub inverse_on_surface: Color,
    pub inverse_primary: Color,

    // Effects
    pub scrim: Color,
    pub shadow: Color,

    // Background
    pub background: Color,
    pub on_background: Color,

    // Additional
    pub surface_tint: Color,
}

impl ColorScheme {
    pub const fn baseline_light() -> Self {
        Self {
            // Primary
            primary: Color::from_hex("#ff7230"),
            on_primary: Color::from_hex("#050505"),
            primary_container: Color::from_hex("#e15b0e"),
            on_primary_container: Color::from_hex("#f4f4f5"),

            // Secondary
            secondary: Color::from_hex("#f4f4f5"),
            on_secondary: Color::from_hex("#ff7230"),
            secondary_container: Color::from_hex("#dbdbdc"),
            on_secondary_container: Color::from_hex("#75757a"),

            // Error
            error: Color::from_hex("#ff4242"),
            on_error: Color::from_hex("#f4f4f5"),
            error_container: Color::from_hex("#b80000"),
            on_error_container: Color::from_hex("#ffebeb"),

            // Success
            success: Color::from_hex("#3cde18"),
            on_success: Color::from_hex("#ffffff"),
            success_container: Color::from_hex("#8cf075"),
            on_success_container: Color::from_hex("#f0fded"),

            // Fixed
            primary_fixed: Color::from_hex("#e15b0e"),
            primary_fixed_dim: Color::from_hex("#aa470e"),
            on_primary_fixed: Color::from_hex("#414144"),
            on_primary_fixed_variant: Color::from_hex("#dbdbdc"),

            secondary_fixed: Color::from_hex("#dbdbdc"),
            secondary_fixed_dim: Color::from_hex("#c8c8cb"),
            on_secondary_fixed: Color::from_hex("#49494b"),
            on_secondary_fixed_variant: Color::from_hex("#49494b"),

            // Surface
            surface_dim: Color::from_hex("#ffffff"),
            surface: Color::from_hex("#dbdbdc"),
            surface_bright: Color::from_hex("#ffffff"),

            surface_container_lowest: Color::from_hex("#f4f4f5"),
            surface_container_low: Color::from_hex("#ededee"),
            surface_container: Color::from_hex("#ededee"),
            surface_container_high: Color::from_hex("#f4f4f5"),
            surface_container_highest: Color::from_hex("#ffffff"),

            on_surface: Color::from_hex("#414144"),
            on_surface_variant: Color::from_hex("#a5a5a7"),

            // Outline
            outline: Color::from_hex("#c8c8cb"),
            outline_variant: Color::from_hex("#f4f4f5"),

            // Inverse
            inverse_surface: Color::from_hex("#050505"),
            inverse_on_surface: Color::from_hex("#ededee"),
            inverse_primary: Color::from_hex("#242424"),

            // Effects
            scrim: Color::from_hex("#a5a5a7"),
            shadow: Color::from_hex("#a5a5a7"),

            // Background
            background: Color::from_hex("#f4f4f5"),
            on_background: Color::from_hex("#242424"),

            // Additional
            surface_tint: Color::from_hex("#c8c8cb"),
        }
    }

    pub const fn baseline_dark() -> Self {
        Self {
            // Primary
            primary: Color::from_hex("#f9640d"),
            on_primary: Color::from_hex("#ffffff"),
            primary_container: Color::from_hex("#fca67b"),
            on_primary_container: Color::from_hex("#141415"),

            // Secondary
            secondary: Color::from_hex("#141415"),
            on_secondary: Color::from_hex("#ff7230"),
            secondary_container: Color::from_hex("#232325"),
            on_secondary_container: Color::from_hex("#a1a1a5"),

            // Error
            error: Color::from_hex("#ff4242"),
            on_error: Color::from_hex("#f5f5f5"),
            error_container: Color::from_hex("#a80404"),
            on_error_container: Color::from_hex("#fee9e9"),

            // Success
            success: Color::from_hex("#40e830"),
            on_success: Color::from_hex("#f5f5f5"),
            success_container: Color::from_hex("#15730d"),
            on_success_container: Color::from_hex("#fbfffc"),

            // Fixed
            primary_fixed: Color::from_hex("#fc8845"),
            primary_fixed_dim: Color::from_hex("#fca67b"),
            on_primary_fixed: Color::from_hex("#e0e0e1"),
            on_primary_fixed_variant: Color::from_hex("#232325"),

            secondary_fixed: Color::from_hex("#19191a"),
            secondary_fixed_dim: Color::from_hex("#373739"),
            on_secondary_fixed: Color::from_hex("#8d8d91"),
            on_secondary_fixed_variant: Color::from_hex("#a1a1a5"),

            // Surface
            surface_dim: Color::from_hex("#050505"),
            surface: Color::from_hex("#232325"),
            surface_bright: Color::from_hex("#232325"),

            surface_container_lowest: Color::from_hex("#050505"),
            surface_container_low: Color::from_hex("#141415"),
            surface_container: Color::from_hex("#19191a"),
            surface_container_high: Color::from_hex("#232325"),
            surface_container_highest: Color::from_hex("#373739"),

            on_surface: Color::from_hex("#f5f5f5"),
            on_surface_variant: Color::from_hex("#646468"),

            // Outline
            outline: Color::from_hex("#373739"),
            outline_variant: Color::from_hex("#141415"),

            // Inverse
            inverse_surface: Color::from_hex("#ffffff"),
            inverse_on_surface: Color::from_hex("#19191a"),
            inverse_primary: Color::from_hex("#f5f5f5"),

            // Effects
            scrim: Color::from_hex("#050505"),
            shadow: Color::from_hex("#050505"),

            // Background
            background: Color::from_hex("#141415"),
            on_background: Color::from_hex("#f5f5f5"),

            // Additional
            surface_tint: Color::from_hex("#373739"),
        }
    }
}

impl Default for ColorScheme {
    #[inline]
    fn default() -> Self {
        Self::baseline_dark()
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorRole {
    // Primary
    Primary,
    OnPrimary,
    PrimaryContainer,
    OnPrimaryContainer,

    // Secondary
    Secondary,
    OnSecondary,
    SecondaryContainer,
    OnSecondaryContainer,

    // Error
    Error,
    OnError,
    ErrorContainer,
    OnErrorContainer,

    // Success
    Success,
    OnSuccess,
    SuccessContainer,
    OnSuccessContainer,

    // Fixed
    PrimaryFixed,
    PrimaryFixedDim,
    OnPrimaryFixed,
    OnPrimaryFixedVariant,

    SecondaryFixed,
    SecondaryFixedDim,
    OnSecondaryFixed,
    OnSecondaryFixedVariant,

    // Surface
    SurfaceDim,
    Surface,
    SurfaceBright,

    SurfaceContainerLowest,
    SurfaceContainerLow,
    SurfaceContainer,
    SurfaceContainerHigh,
    SurfaceContainerHighest,

    OnSurface,
    OnSurfaceVariant,

    // Outline
    Outline,
    OutlineVariant,

    // Inverse
    InverseSurface,
    InverseOnSurface,
    InversePrimary,

    // Effects
    Scrim,
    Shadow,

    // Background
    Background,
    OnBackground,

    // Additional
    SurfaceTint,
}

impl ColorRole {
    #[inline]
    pub fn resolve(self, scheme: &ColorScheme) -> Color {
        match self {
            // Primary
            Self::Primary => scheme.primary,
            Self::OnPrimary => scheme.on_primary,
            Self::PrimaryContainer => scheme.primary_container,
            Self::OnPrimaryContainer => scheme.on_primary_container,

            // Secondary
            Self::Secondary => scheme.secondary,
            Self::OnSecondary => scheme.on_secondary,
            Self::SecondaryContainer => scheme.secondary_container,
            Self::OnSecondaryContainer => scheme.on_secondary_container,

            // Error
            Self::Error => scheme.error,
            Self::OnError => scheme.on_error,
            Self::ErrorContainer => scheme.error_container,
            Self::OnErrorContainer => scheme.on_error_container,

            // Success
            Self::Success => scheme.success,
            Self::OnSuccess => scheme.on_success,
            Self::SuccessContainer => scheme.success_container,
            Self::OnSuccessContainer => scheme.on_success_container,

            // Fixed
            Self::PrimaryFixed => scheme.primary_fixed,
            Self::PrimaryFixedDim => scheme.primary_fixed_dim,
            Self::OnPrimaryFixed => scheme.on_primary_fixed,
            Self::OnPrimaryFixedVariant => scheme.on_primary_fixed_variant,

            Self::SecondaryFixed => scheme.secondary_fixed,
            Self::SecondaryFixedDim => scheme.secondary_fixed_dim,
            Self::OnSecondaryFixed => scheme.on_secondary_fixed,
            Self::OnSecondaryFixedVariant => scheme.on_secondary_fixed_variant,

            // Surface
            Self::SurfaceDim => scheme.surface_dim,
            Self::Surface => scheme.surface,
            Self::SurfaceBright => scheme.surface_bright,

            Self::SurfaceContainerLowest => scheme.surface_container_lowest,
            Self::SurfaceContainerLow => scheme.surface_container_low,
            Self::SurfaceContainer => scheme.surface_container,
            Self::SurfaceContainerHigh => scheme.surface_container_high,
            Self::SurfaceContainerHighest => scheme.surface_container_highest,

            Self::OnSurface => scheme.on_surface,
            Self::OnSurfaceVariant => scheme.on_surface_variant,

            // Outline
            Self::Outline => scheme.outline,
            Self::OutlineVariant => scheme.outline_variant,

            // Inverse
            Self::InverseSurface => scheme.inverse_surface,
            Self::InverseOnSurface => scheme.inverse_on_surface,
            Self::InversePrimary => scheme.inverse_primary,

            // Effects
            Self::Scrim => scheme.scrim,
            Self::Shadow => scheme.shadow,

            // Background
            Self::Background => scheme.background,
            Self::OnBackground => scheme.on_background,

            // Additional
            Self::SurfaceTint => scheme.surface_tint,
        }
    }
}
