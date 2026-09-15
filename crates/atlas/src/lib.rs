#![forbid(unsafe_code)]
//! The atlas: every pixel a sprite can name, and the ids that name them.
//! Crate docs are completed in a later task.

use app::Resource;
use geometry::{Rect, Size};

pub mod prelude {
    pub use crate::{
        Atlas, AtlasId, AtlasTile, Bitmap, Cell, Class, Error, FontId, Format, Glyph, Line, Page,
        Sprite, SpriteId,
    };
}

mod bitmap;
mod font;
mod mip;
mod page;

pub use font::{Glyph, Line};
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

/// An icon or an image: its tile and the size it was inserted at, its
/// master. It is drawn at that size or smaller.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sprite {
    pub tile: AtlasTile,
    pub size: Size,
}

/// What an `AtlasId` names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Owner {
    /// A page: its class and its index in that class's list.
    Page(Class, u32),
    /// An external: its index in the external list. Filled in by a later task.
    #[allow(dead_code)]
    External(u32),
}

/// Every pixel a sprite can name, and the ids that name them. A plain
/// resource: no system, no event, no signal of its own. Writers call it
/// directly and get a tile back in the same call; a backend drains it in
/// a system on the core's `OnChanged<Atlas>`, sent at the `PostTick` of
/// any tick that took `resource_mut::<Atlas>()`, reading through
/// `resource::<Atlas>()` so the drain is not itself a write.
#[derive(Debug)]
pub struct Atlas {
    /// Indexed by `Class as usize` for the three page classes.
    pages: [Vec<Page>; 3],
    /// Indexed by `AtlasId`.
    owners: Vec<Owner>,
    /// Indexed by `SpriteId`.
    sprites: Vec<Sprite>,
    /// Indexed by `FontId`.
    fonts: Vec<font::Font>,
    /// Indexed by the value a font's per-size index vector holds.
    glyphs: Vec<Glyph>,
}

impl Resource for Atlas {}

impl Default for Atlas {
    fn default() -> Self {
        Self::new()
    }
}

impl Atlas {
    /// Empty: no page, no font, no sprite.
    pub fn new() -> Atlas {
        Atlas {
            pages: [Vec::new(), Vec::new(), Vec::new()],
            owners: Vec::new(),
            sprites: Vec::new(),
            fonts: Vec::new(),
            glyphs: Vec::new(),
        }
    }

    /// The next id, owned by `owner`.
    fn mint(&mut self, owner: Owner) -> AtlasId {
        let id = AtlasId(self.owners.len() as u32);
        self.owners.push(owner);
        id
    }

    /// Pack `bitmap` into a page of `class`, opening one when no existing
    /// page takes it, write the pixels, regenerate the mips under it and
    /// mark its cells. `bitmap.format` must be the class's.
    fn pack(&mut self, class: Class, bitmap: &Bitmap) -> Result<AtlasTile, Error> {
        if bitmap.format != class.format() {
            return Err(Error::Class);
        }
        if !Page::fits(class, bitmap.width, bitmap.height) {
            return Err(Error::TooLarge {
                width: bitmap.width,
                height: bitmap.height,
                max: page::PAGE - 2 * class.align(),
            });
        }
        let list = class as usize;
        let mut placed = None;
        for (i, page) in self.pages[list].iter_mut().enumerate() {
            if let Some(rect) = page.pack(bitmap.width, bitmap.height) {
                placed = Some((i, rect));
                break;
            }
        }
        let (i, rect) = match placed {
            Some(p) => p,
            None => {
                let i = self.pages[list].len();
                let id = self.mint(Owner::Page(class, i as u32));
                let mut page = Page::new(id, class);
                let rect = page
                    .pack(bitmap.width, bitmap.height)
                    .expect("an empty page takes what fits");
                self.pages[list].push(page);
                (i, rect)
            }
        };
        let page = &mut self.pages[list][i];
        page.write(rect, bitmap);
        if class.levels() > 1 {
            page.regenerate(rect);
        }
        Ok(AtlasTile {
            atlas: page.id(),
            bounds: rect,
        })
    }

    /// Insert an icon (`Class::Icon`, an `R8` bitmap) or an image
    /// (`Class::Image`, `Rgba8`) at its master size. There is no key: the
    /// caller keeps the id. `Error::Class` for any other class or a format
    /// the class does not take; `Error::TooLarge` for a bitmap no empty
    /// page of the class fits, which `Bitmap::fit` cures.
    pub fn insert(&mut self, class: Class, bitmap: &Bitmap) -> Result<SpriteId, Error> {
        if !matches!(class, Class::Icon | Class::Image) {
            return Err(Error::Class);
        }
        let tile = self.pack(class, bitmap)?;
        let id = SpriteId(self.sprites.len() as u32);
        self.sprites.push(Sprite {
            tile,
            size: Size::new(bitmap.width as f32, bitmap.height as f32),
        });
        Ok(id)
    }

    /// The sprite `id` names. Panics on an id this atlas did not mint.
    pub fn sprite(&self, id: SpriteId) -> Sprite {
        self.sprites[id.0 as usize]
    }

    /// What `id` is: how a backend picks a format, a mip filter and an
    /// import path, and checks a tile reached the right primitive. Panics
    /// on an id this atlas did not mint.
    pub fn class(&self, id: AtlasId) -> Class {
        match self.owners[id.0 as usize] {
            Owner::Page(class, _) => class,
            Owner::External(_) => Class::External,
        }
    }

    /// Every page, in id order.
    pub fn pages(&self) -> impl Iterator<Item = &Page> {
        self.owners.iter().filter_map(|o| match *o {
            Owner::Page(class, i) => Some(&self.pages[class as usize][i as usize]),
            Owner::External(_) => None,
        })
    }

    /// Take every page's dirty mask and call `f` once per set cell with the
    /// page: class by class, each class's pages in creation order, cells
    /// rows then columns. A clean page costs one read. `&self`, so a
    /// backend calls it through `resource::<Atlas>()` and the drain does
    /// not re-arm `OnChanged<Atlas>`.
    pub fn drain_dirty(&self, mut f: impl FnMut(&Page, Cell)) {
        for list in &self.pages {
            for page in list.iter() {
                let mask = page.take_dirty();
                if mask == [0; 4] {
                    continue;
                }
                for cell in Page::cells(mask) {
                    f(page, cell);
                }
            }
        }
    }

    /// How many glyph records exist. For tests.
    #[cfg(test)]
    pub(crate) fn glyph_count(&self) -> usize {
        self.glyphs.len()
    }
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
    /// The bytes are not a PNG this crate decodes.
    Png(png::DecodingError),
    /// The bytes are not an SVG resvg parses.
    #[cfg(feature = "svg")]
    Svg(resvg::usvg::Error),
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
