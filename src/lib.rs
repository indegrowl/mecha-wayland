//! The `mecha-wayland` facade: one crate to depend on, one prelude to import.
//! Each member crate's prelude is folded into [`prelude`] as it is rewritten
//! against the new `app` core.

pub mod prelude {
    pub use app::prelude::*;
}
