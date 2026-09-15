#![forbid(unsafe_code)]
//! The atlas: every pixel a sprite can name, and the ids that name them.
//! Crate docs are completed in a later task.

use geometry::Rect;

pub mod prelude {
    pub use crate::{AtlasId, AtlasTile, Bitmap, Cell, Class, FontId, Format, Page, SpriteId};
}

mod mip;
mod page;

pub use page::{CELL, CELLS, Cell, PAGE, Page};

/// Which texture a tile is in: a dense index the atlas mints, in creation
/// order, across every page of every class and every external, so a
/// backend keys its textures by it with a `Vec`. `default()` is id zero,
/// the first page ever made. `repr(C)` because a render command embeds
/// it and is uploaded as is.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct AtlasId(pub u32);

/// One rectangle of a texture: `bounds` in whole pixels of that texture,
/// padding excluded, held as a `Rect` because that is the shared type. A
/// tile's bounds are stable for the app's life: the atlas grows by adding
/// pages and never moves a tile. A renderer divides `bounds` by the size
/// of the texture it uploaded. `repr(C)` because a render command embeds
/// it and is uploaded as is.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AtlasTile {
    pub atlas: AtlasId,
    pub bounds: Rect,
}

/// What a page holds and how. `Glyph` and `Icon` are coverage, `Image` is
/// colour, `External` is a texture the atlas names but does not hold.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Glyph = 0,
    Icon = 1,
    Image = 2,
    External = 3,
}

/// The pixel format of a page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    R8,
    Rgba8,
}

impl Format {
    /// Bytes per pixel.
    pub const fn bytes(self) -> usize {
        match self {
            Format::R8 => 1,
            Format::Rgba8 => 4,
        }
    }
}

impl Class {
    /// `Glyph` and `Icon` are `R8`; `Image` is `Rgba8`; `External` is
    /// nominally `Rgba8`: a backend reads its descriptor's fourcc.
    pub const fn format(self) -> Format {
        match self {
            Class::Glyph | Class::Icon => Format::R8,
            Class::Image | Class::External => Format::Rgba8,
        }
    }

    /// Mip levels a page of this class holds, the base included: one for
    /// `Glyph` and `External`, five for `Icon` and `Image`.
    pub const fn levels(self) -> u32 {
        match self {
            Class::Icon | Class::Image => 5,
            Class::Glyph | Class::External => 1,
        }
    }

    /// The grid a tile's origin and size sit on, which is also the zero
    /// border around it: 16 for the mipmapped classes, 1 otherwise.
    pub const fn align(self) -> u32 {
        match self {
            Class::Icon | Class::Image => 16,
            Class::Glyph | Class::External => 1,
        }
    }
}

/// An icon or an image: an index into the atlas's sprite list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpriteId(pub u32);

/// A font: an index into the atlas's font list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontId(pub u16);

/// Pixels to insert: `width * height * format.bytes()` bytes, row-major,
/// tightly packed, straight alpha.
#[derive(Debug, Clone, PartialEq)]
pub struct Bitmap {
    pub width: u32,
    pub height: u32,
    pub format: Format,
    pub pixels: Vec<u8>,
}

/// What can go wrong with input from outside. Ids the atlas minted are
/// never wrong, so nothing that takes one returns this.
#[derive(Debug)]
pub enum Error {
    /// fontdue rejected the bytes.
    Font(&'static str),
    /// `insert`: the class does not take that format, or is `Glyph` or
    /// `External`.
    Class,
    /// `insert`: the bitmap cannot fit an empty page of its class.
    TooLarge { width: u32, height: u32, max: u32 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes_describe_their_pages() {
        assert_eq!(Class::Glyph.format(), Format::R8);
        assert_eq!(Class::Icon.format(), Format::R8);
        assert_eq!(Class::Image.format(), Format::Rgba8);
        assert_eq!(Class::External.format(), Format::Rgba8);

        assert_eq!(Class::Glyph.levels(), 1);
        assert_eq!(Class::Icon.levels(), 5);
        assert_eq!(Class::Image.levels(), 5);
        assert_eq!(Class::External.levels(), 1);

        assert_eq!(Class::Glyph.align(), 1);
        assert_eq!(Class::Icon.align(), 16);
        assert_eq!(Class::Image.align(), 16);
        assert_eq!(Class::External.align(), 1);

        assert_eq!(Format::R8.bytes(), 1);
        assert_eq!(Format::Rgba8.bytes(), 4);
    }

    #[test]
    fn the_seam_types_are_plain_data() {
        assert_eq!(std::mem::size_of::<AtlasId>(), 4);
        assert_eq!(
            std::mem::size_of::<AtlasTile>(),
            20,
            "an id and four floats, no padding"
        );
        assert_eq!(AtlasId::default(), AtlasId(0));
        let a = AtlasTile {
            atlas: AtlasId(1),
            bounds: Rect::new(0.0, 0.0, 8.0, 8.0),
        };
        let b = AtlasTile {
            atlas: AtlasId(1),
            bounds: Rect::new(8.0, 0.0, 8.0, 8.0),
        };
        assert_ne!(a, b, "different bounds are different tiles");
    }
}
