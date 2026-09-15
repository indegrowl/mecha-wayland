#![forbid(unsafe_code)]
//! The paint module: what a node looks like, held as one [`Paint`] per
//! node.
//!
//! # Model
//!
//! - A `Paint` says *how* a node looks and never *where*; the where is the
//!   node's `Layout`, and whoever draws reads the two together. It is
//!   resolved, not descriptive: nothing interprets it beyond placing it.
//! - The set of primitives is closed: a [`Quad`] that fills the node's
//!   box, a run of [`MonochromeSprite`]s (glyphs, icons) placed inside its
//!   content box, or one [`PolychromeSprite`] (an image) stretched over
//!   it. A node draws one kind; a node that wants two visuals is two nodes.
//! - Widgets write `Paint`: it is their decision how they are painted. A
//!   text widget resolves its string and font into a sprite run when
//!   either changes, so paint never sees a string.
//! - A sprite names its pixels through an [`AtlasTile`]: an [`AtlasId`]
//!   and bounds in atlas pixels. Paint knows nothing else about atlases;
//!   the id is a placeholder for the atlas module to shape.
//! - [`PaintModule`] registers the component and nothing else. A write is
//!   reported by the core's `OnChanged<Paint>` at the next `PostTick`, an
//!   equal write through `set_if_neq` is not, and nothing here compares
//!   paints on `Tick`.
//!
//! # Quick start
//!
//! ```
//! use app::prelude::*;
//! use paint::prelude::*;
//!
//! # struct Leaf;
//! # impl Build for Leaf { type Widget = Leaf; }
//! # impl Widget for Leaf {
//! #     type Builder = Leaf;
//! #     fn build(b: Leaf, _: Handle<Self>, _: &mut Spawner<'_, Self>) -> Self { b }
//! # }
//! # use std::cell::RefCell;
//! # thread_local! { static SEEN: RefCell<Vec<NodeId>> = const { RefCell::new(Vec::new()) }; }
//! fn on_repaint(_: &mut App, e: &Emitted<OnChanged<Paint>>) {
//!     SEEN.with(|s| s.borrow_mut().extend(e.targets.iter().copied()));
//! }
//!
//! let mut app = App::new();
//! app.add_module(PaintModule).system(on_repaint);
//!
//! // A panel painted at spawn.
//! let panel = app.spawn_with(
//!     app.root(),
//!     Leaf,
//!     (Paint::Quad(Quad::new(Color::from_rgb8(30, 30, 40)).radius(6.0)),),
//! );
//!
//! app.tick();
//! assert_eq!(SEEN.with(|s| s.borrow().clone()), vec![panel.id()]);
//! assert!(!app.component::<Paint>(panel).unwrap().is_invisible());
//!
//! // Repaint between ticks; the next tick reports it.
//! *app.component_mut::<Paint>(panel).unwrap() = Paint::None;
//! app.tick();
//! assert_eq!(SEEN.with(|s| s.borrow().len()), 2);
//! ```

use app::{App, Component, Module};
use geometry::{Color, Corners, Insets, Point, Rect, Size};

pub mod prelude {
    pub use crate::{
        AtlasId, AtlasTile, MonochromeSprite, Paint, PaintModule, PolychromeSprite, Quad,
    };
    pub use geometry::{Color, Corners};
}

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

/// A glyph or an icon: one channel of coverage from an atlas, tinted by
/// `color`, stretched into `size` at `offset` from the top left of the
/// node's content box (`Layout::content()`). Logical pixels; at a scale
/// factor other than one the tile is larger than `size` by that factor,
/// which is the atlas's business when it rasterizes and a renderer's when
/// it samples.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonochromeSprite {
    pub tile: AtlasTile,
    pub offset: Point,
    pub size: Size,
    pub color: Color,
}

impl MonochromeSprite {
    /// True when a renderer would leave no pixel behind: a transparent
    /// tint or an empty box.
    pub fn is_invisible(&self) -> bool {
        self.color.is_transparent() || self.size.width <= 0.0 || self.size.height <= 0.0
    }
}

/// An image: a full-colour tile stretched over the node's content box,
/// with rounded corners, an opacity and a grayscale switch. No offset and
/// no size: an image that wants a place is a node with a `LayoutStyle`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PolychromeSprite {
    pub tile: AtlasTile,
    pub radii: Corners<f32>,
    /// `0.0..=1.0`; `1.0` is opaque.
    pub opacity: f32,
    pub grayscale: bool,
}

/// The verbs, by value.
impl PolychromeSprite {
    /// That tile, square, opaque, in colour.
    pub const fn new(tile: AtlasTile) -> Self {
        Self {
            tile,
            radii: Corners::all(0.0),
            opacity: 1.0,
            grayscale: false,
        }
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

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }

    pub fn grayscale(mut self, grayscale: bool) -> Self {
        self.grayscale = grayscale;
        self
    }

    /// True when a renderer would leave no pixel behind: fully transparent.
    pub fn is_invisible(&self) -> bool {
        self.opacity <= 0.0
    }
}

/// The one primitive a node draws, or nothing. Dense: every node has one,
/// and the default draws nothing. A node draws one primitive kind; a node
/// that wants two visuals, a background and a label, is two nodes. Text
/// is a `Monochrome` run of many, an icon a run of one, an image a
/// `Polychrome`. The two sprite variants are the atlas kind a renderer
/// picks: a coverage texture behind `Monochrome`, a colour texture behind
/// `Polychrome`; the atlas guarantees a tile it mints for one is never
/// handed to the other.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Paint {
    #[default]
    None,
    Quad(Quad),
    Monochrome(Vec<MonochromeSprite>),
    Polychrome(PolychromeSprite),
}

impl Component for Paint {}

impl Paint {
    /// True when a renderer would leave no pixel behind. An empty run is
    /// invisible.
    pub fn is_invisible(&self) -> bool {
        match self {
            Paint::None => true,
            Paint::Quad(quad) => quad.is_invisible(),
            Paint::Monochrome(run) => run.iter().all(MonochromeSprite::is_invisible),
            Paint::Polychrome(sprite) => sprite.is_invisible(),
        }
    }
}

/// Registers [`Paint`] and nothing else. No system, no resource, no
/// signal: a write is reported by the core's own `OnChanged<Paint>` drain
/// at the next `PostTick`. Installs after nothing in particular. Installing
/// it twice panics on the second registration, as the core specifies for
/// any module.
pub struct PaintModule;

impl Module for PaintModule {
    fn install(self, app: &mut App) {
        app.register_component::<Paint>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tile() -> AtlasTile {
        AtlasTile {
            atlas: AtlasId,
            bounds: Rect::new(0.0, 0.0, 8.0, 8.0),
        }
    }

    fn glyph(color: Color) -> MonochromeSprite {
        MonochromeSprite {
            tile: tile(),
            offset: Point::new(1.0, 2.0),
            size: Size::new(8.0, 8.0),
            color,
        }
    }

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
        let other = AtlasTile {
            atlas: AtlasId,
            bounds: Rect::new(16.0, 0.0, 16.0, 16.0),
        };
        assert_ne!(tile, other, "different bounds are different tiles");
        assert_eq!(tile.atlas, AtlasId::default());
    }

    #[test]
    fn monochrome_sprite_visibility() {
        assert!(!glyph(Color::BLACK).is_invisible());
        assert!(
            glyph(Color::TRANSPARENT).is_invisible(),
            "a transparent tint"
        );
        let mut flat = glyph(Color::BLACK);
        flat.size = Size::new(8.0, 0.0);
        assert!(flat.is_invisible(), "a zero height");
        flat.size = Size::new(0.0, 8.0);
        assert!(flat.is_invisible(), "a zero width");
    }

    #[test]
    fn polychrome_sprite_verbs_and_visibility() {
        let image = PolychromeSprite::new(tile());
        assert_eq!(image.radii, Corners::all(0.0));
        assert_eq!(image.opacity, 1.0);
        assert!(!image.grayscale);
        assert!(!image.is_invisible());

        let image = image.radius(3.0).opacity(0.5).grayscale(true);
        assert_eq!(image.radii, Corners::all(3.0));
        assert_eq!(image.opacity, 0.5);
        assert!(image.grayscale);
        assert!(!image.is_invisible());

        let image = image.radii(Corners::new(1.0, 2.0, 3.0, 4.0)).opacity(0.0);
        assert_eq!(image.radii, Corners::new(1.0, 2.0, 3.0, 4.0));
        assert!(image.is_invisible(), "zero opacity");
    }

    #[test]
    fn paint_defaults_to_none_and_is_invisible() {
        assert_eq!(Paint::default(), Paint::None);
        assert!(Paint::None.is_invisible());
    }

    #[test]
    fn paint_visibility_follows_its_primitive() {
        assert!(Paint::Quad(Quad::default()).is_invisible());
        assert!(!Paint::Quad(Quad::new(Color::BLACK)).is_invisible());
        assert!(Paint::Monochrome(Vec::new()).is_invisible(), "an empty run");
        assert!(
            Paint::Monochrome(vec![glyph(Color::TRANSPARENT), glyph(Color::TRANSPARENT)])
                .is_invisible(),
            "a run of invisible sprites"
        );
        assert!(
            !Paint::Monochrome(vec![glyph(Color::TRANSPARENT), glyph(Color::BLACK)]).is_invisible(),
            "one visible sprite is enough"
        );
        assert!(Paint::Polychrome(PolychromeSprite::new(tile()).opacity(0.0)).is_invisible());
        assert!(!Paint::Polychrome(PolychromeSprite::new(tile())).is_invisible());
    }
}
