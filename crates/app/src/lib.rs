#![forbid(unsafe_code)]
//! The UI runtime: a tree of nodes, each backed by a widget.
//!
//! # Model
//!
//! - [`App`] owns the tree. It is never empty: the root exists from
//!   [`App::new`], is its own parent, and cannot be removed.
//! - A [`NodeId`] names a node. It is opaque: a token to hand back to the
//!   app. A [`Handle`] is a `NodeId` that also knows the widget type. A
//!   removed node's id is stale for good, even once its slot is reused.
//! - A [`Widget`] is the per-node value. Widgets are stored by type, one
//!   column per type. The widget names its builder through
//!   [`Widget::Builder`] and owns the build: [`Widget::build`] runs once,
//!   after the node is linked into the tree and before the widget is
//!   stored, with a [`Spawner`] that attaches children under it.
//! - A [`Build`] is the marker a builder implements so that
//!   [`App::spawn`] can infer the widget type from the builder.
//!
//! # Failure
//!
//! Reads return `None` for a stale id or a wrong widget type.
//! [`App::remove`] returns `false` for a stale id. Only two things panic:
//! spawning under a dead parent and removing the root. Both are caller
//! bugs, not states to recover from.
//!
//! # Quick start
//!
//! ```
//! use app::prelude::*;
//!
//! struct Counter(u32);
//! struct CounterBuilder { start: u32 }
//! impl Build for CounterBuilder { type Widget = Counter; }
//! impl Widget for Counter {
//!     type Builder = CounterBuilder;
//!     fn build(b: CounterBuilder, _me: Handle<Self>, _s: &mut Spawner<'_>) -> Self {
//!         Counter(b.start)
//!     }
//! }
//!
//! struct Row;
//! struct RowBuilder { counters: u32 }
//! impl Build for RowBuilder { type Widget = Row; }
//! impl Widget for Row {
//!     type Builder = RowBuilder;
//!     fn build(b: RowBuilder, me: Handle<Self>, s: &mut Spawner<'_>) -> Self {
//!         for start in 0..b.counters {
//!             s.spawn(me, CounterBuilder { start });
//!         }
//!         Row
//!     }
//! }
//!
//! let mut app = App::new();
//! let row = app.spawn(app.root(), RowBuilder { counters: 3 });
//! assert_eq!(app.children(row).unwrap().len(), 3);
//!
//! for (_, counter) in app.widgets::<Counter>() {
//!     counter.0 += 10;
//! }
//! let first = app.children(row).unwrap()[0];
//! assert_eq!(app.widget::<Counter>(first).unwrap().0, 10);
//!
//! assert!(app.remove(row));
//! assert!(app.widget::<Counter>(first).is_none());
//! assert_eq!(app.widgets::<Counter>().count(), 0);
//! ```

mod app;
mod component;
mod id;
mod nodes;
mod query;
mod slots;
mod store;
mod tree;
mod widgets;

pub use app::{App, Spawner};
pub use component::Component;
pub use id::{Handle, NodeId};
pub use query::CompMut;
pub use tree::Tree;
pub use widgets::{Build, Widget};

pub mod prelude {
    pub use crate::{App, Build, Component, Handle, NodeId, Spawner, Tree, Widget};
}
