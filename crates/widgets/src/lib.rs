//! The `widgets` crate: `Div`, `Text`, `Icon`, `Image` — the primitive
//! widgets a spawn site names instead of hand-writing `LayoutStyle` and
//! `Paint` itself. See
//! `docs/superpowers/specs/2026-09-18-widgets-design.md` for the design.
//!
//! # Quick start
//!
//! ```
//! use app::prelude::*;
//! use atlas::prelude::*;
//! use layout::prelude::*;
//! use paint::prelude::*;
//! use widgets::prelude::*;
//!
//! let mut app = App::new();
//! app.add_module(LayoutModule).add_module(PaintModule);
//! app.insert_resource(Atlas::new());
//!
//! let root = app.spawn_with(
//!     app.root(),
//!     div().style(LayoutStyle::default().size(px(200.0), px(100.0))),
//!     (LayoutRoot(true),),
//! );
//!
//! let font = app
//!     .resource_mut::<Atlas>()
//!     .add_font(include_bytes!("../../atlas/tests/fixtures/Inter-Regular.ttf"))
//!     .unwrap();
//! let sprite = app
//!     .resource_mut::<Atlas>()
//!     .insert(
//!         Class::Icon,
//!         &Bitmap { width: 8, height: 8, format: Format::R8, pixels: vec![255; 64] },
//!     )
//!     .unwrap();
//!
//! let label = app.spawn(root, text(font, "hi"));
//! let glyph = app.spawn(root, icon(sprite));
//!
//! app.tick();
//! assert!(app.component::<Layout>(label).unwrap().rect.width() > 0.0);
//! assert!(app.component::<Layout>(glyph).unwrap().rect.width() > 0.0);
//! ```

mod div;
mod icon;
mod image;
mod text;

pub use div::{Div, DivBuilder, DivContext, div};
pub use icon::{Icon, IconBuilder, IconContext, icon};
pub use image::{Image, ImageBuilder, ImageContext, image};
pub use text::{Text, TextBuilder, TextContext, text};

pub mod prelude {
    pub use crate::{
        Div, DivBuilder, DivContext, Icon, IconBuilder, IconContext, Image, ImageBuilder,
        ImageContext, Text, TextBuilder, TextContext, div, icon, image, text,
    };
}
