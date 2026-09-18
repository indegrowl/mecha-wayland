//! The `widgets` crate: `Div`, `Text`, `Icon`, `Image` — the primitive
//! widgets a spawn site names instead of hand-writing `LayoutStyle` and
//! `Paint` itself. See
//! `docs/superpowers/specs/2026-09-18-widgets-design.md` for the design.

mod div;
mod text;

pub use div::{Div, DivBuilder, DivContext, div};
pub use text::{Text, TextBuilder, TextContext, text};

pub mod prelude {
    pub use crate::{Div, DivBuilder, DivContext, Text, TextBuilder, TextContext, div, text};
}
