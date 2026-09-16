//! The `mecha-wayland` facade: one crate to depend on, one prelude to import.
//! Each member crate's prelude is folded into [`prelude`] as it is rewritten
//! against the new `app` core.

// `atlas` and `gles` each have their own `Error` and `Plane`; the facade
// carries both, so an unqualified use of either name is ambiguous here
// and wants an `atlas::` or `gles::` prefix.
#[allow(ambiguous_glob_reexports)]
pub mod prelude {
    pub use app::prelude::*;
    pub use atlas::prelude::*;
    pub use geometry::prelude::*;
    pub use gles::prelude::*;
    pub use layout::prelude::*;
    pub use paint::prelude::*;
    pub use presentation::prelude::*;
    pub use render::prelude::*;
    pub use ring::prelude::*;
    pub use wayland::prelude::*;
    pub use window::prelude::*;
}
