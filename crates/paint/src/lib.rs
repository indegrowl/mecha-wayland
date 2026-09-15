#![forbid(unsafe_code)]
//! The paint module: what a node looks like, held as one [`Paint`] per
//! node. Crate docs are completed in a later task.

use geometry::{Color, Corners, Insets, Rect};

/// Which atlas a tile is in. A placeholder: an empty struct, so that a
/// sprite can name an atlas today and the atlas spec can give the id its
/// real shape without a widget or a renderer changing which field it
/// reads. Minted by the atlas, never by paint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct AtlasId;

/// One rectangle of an atlas: `bounds` in whole atlas pixels, padding
/// excluded, held as a `Rect` because that is the shared type. A tile's
/// bounds are stable while its atlas exists; an atlas grows by adding
/// textures, never by moving tiles. Paint never sees a texture's size: a
/// renderer divides `bounds` by the size of the texture it uploaded.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AtlasTile {
    pub atlas: AtlasId,
    pub bounds: Rect,
}

/// A filled box: a colour, a radius per corner, a border width per side
/// and one border colour. It always fills the node's `Layout` rect, so it
/// carries no position or size. The border is drawn inside the rect.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quad {
    pub color: Color,
    pub radii: Corners<f32>,
    pub border: Insets<f32>,
    pub border_color: Color,
}

impl Default for Quad {
    /// A quad that draws nothing: transparent, square, unbordered. What a
    /// positioning-only container carries.
    fn default() -> Self {
        Self::new(Color::TRANSPARENT)
    }
}

/// The verbs, by value, so a quad is written in one expression.
impl Quad {
    /// That fill, no radii, no border.
    pub const fn new(color: Color) -> Self {
        Self {
            color,
            radii: Corners::all(0.0),
            border: Insets::all(0.0),
            border_color: Color::TRANSPARENT,
        }
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// The same radius at every corner.
    pub fn radius(mut self, radius: f32) -> Self {
        self.radii = Corners::all(radius);
        self
    }

    pub fn radii(mut self, radii: Corners<f32>) -> Self {
        self.radii = radii;
        self
    }

    /// The same width on every side, in that colour.
    pub fn border(mut self, width: f32, color: Color) -> Self {
        self.border = Insets::all(width);
        self.border_color = color;
        self
    }

    pub fn border_widths(mut self, widths: Insets<f32>) -> Self {
        self.border = widths;
        self
    }

    pub fn border_color(mut self, color: Color) -> Self {
        self.border_color = color;
        self
    }

    /// True when a renderer would leave no pixel behind: a transparent
    /// fill, and a border that has no width on any side or no colour.
    pub fn is_invisible(&self) -> bool {
        let b = self.border;
        let no_border = (b.top <= 0.0 && b.right <= 0.0 && b.bottom <= 0.0 && b.left <= 0.0)
            || self.border_color.is_transparent();
        self.color.is_transparent() && no_border
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quad_verbs_compose_by_value() {
        let q = Quad::new(Color::WHITE)
            .color(Color::BLACK)
            .radius(4.0)
            .border(2.0, Color::WHITE);
        assert_eq!(q.color, Color::BLACK);
        assert_eq!(q.radii, Corners::all(4.0));
        assert_eq!(q.border, Insets::all(2.0));
        assert_eq!(q.border_color, Color::WHITE);

        let q = q
            .radii(Corners::new(1.0, 2.0, 3.0, 4.0))
            .border_widths(Insets::new(1.0, 0.0, 1.0, 0.0))
            .border_color(Color::BLACK);
        assert_eq!(q.radii, Corners::new(1.0, 2.0, 3.0, 4.0));
        assert_eq!(q.border, Insets::new(1.0, 0.0, 1.0, 0.0));
        assert_eq!(q.border_color, Color::BLACK);
    }

    #[test]
    fn a_default_quad_draws_nothing() {
        assert_eq!(Quad::default(), Quad::new(Color::TRANSPARENT));
        assert!(Quad::default().is_invisible());
    }

    #[test]
    fn quad_visibility() {
        assert!(!Quad::new(Color::BLACK).is_invisible(), "a fill is visible");
        assert!(
            !Quad::default().border(1.0, Color::BLACK).is_invisible(),
            "a border alone is visible"
        );
        assert!(
            Quad::default().border(0.0, Color::BLACK).is_invisible(),
            "a zero-width border is not"
        );
        assert!(
            Quad::default()
                .border(1.0, Color::TRANSPARENT)
                .is_invisible(),
            "nor a transparent one"
        );
        assert!(
            Quad::new(Color::WHITE.with_alpha(0.0))
                .radius(8.0)
                .is_invisible(),
            "radii do not make a transparent quad visible"
        );
    }

    #[test]
    fn an_atlas_tile_is_plain_data() {
        let tile = AtlasTile {
            atlas: AtlasId,
            bounds: Rect::new(0.0, 0.0, 16.0, 16.0),
        };
        assert_eq!(tile, tile);
        assert_eq!(tile.atlas, AtlasId::default());
    }
}
