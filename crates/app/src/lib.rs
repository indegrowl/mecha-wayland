//! The UI runtime: a tree of nodes, each backed by a widget.
//!
//! Crate documentation is completed in the last task of the plan.

mod app;
mod id;
mod nodes;
mod slots;
mod store;
mod widgets;

pub use app::{App, Spawner};
pub use id::{Handle, NodeId};
pub use widgets::{Build, Widget};

pub mod prelude {
    pub use crate::{App, Build, Handle, NodeId, Spawner, Widget};
}
