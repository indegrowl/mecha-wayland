#![forbid(unsafe_code)]
//! The layout module. Crate docs arrive in a later task.

mod style;

use std::fmt;

use app::{App, Component, Module, NodeId, PostTick, Resource, Signal};
use geometry::{Insets, Rect, Size};
use taffy::AvailableSpace;
use taffy::geometry::Size as TSize;

pub use style::{
    Align, Direction, Display, Justify, LayoutStyle, Position, Val, Wrap, auto, percent, px,
};

pub mod prelude {
    pub use crate::{
        Align, Available, Constraints, Direction, Display, Justify, Layout, LayoutDone,
        LayoutModule, LayoutRoot, LayoutStyle, Measure, Position, Val, Wrap, auto, percent, px,
    };
}

// ---------------------------------------------------------------------------
// Measure
// ---------------------------------------------------------------------------

/// The space offered on one axis when a leaf is measured.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Available {
    /// This many pixels.
    Definite(f32),
    /// As little as possible: the leaf's smallest acceptable size.
    MinContent,
    /// As much as it wants: the leaf's natural size.
    MaxContent,
}

impl Available {
    #[allow(dead_code)]
    fn from_taffy(a: AvailableSpace) -> Self {
        match a {
            AvailableSpace::Definite(v) => Available::Definite(v),
            AvailableSpace::MinContent => Available::MinContent,
            AvailableSpace::MaxContent => Available::MaxContent,
        }
    }
}

/// What a leaf is measured under: the dimensions already fixed by its
/// style or its parent, and the space available on each axis. The exact
/// contract taffy offers a leaf (`compute_leaf_layout` in
/// `taffy/src/compute/leaf.rs`), in our own types.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Constraints {
    pub known_width: Option<f32>,
    pub known_height: Option<f32>,
    pub available_width: Available,
    pub available_height: Available,
}

impl Constraints {
    #[allow(dead_code)]
    pub(crate) fn from_taffy(known: TSize<Option<f32>>, available: TSize<AvailableSpace>) -> Self {
        Self {
            known_width: known.width,
            known_height: known.height,
            available_width: Available::from_taffy(available.width),
            available_height: Available::from_taffy(available.height),
        }
    }
}

/// A leaf's size, asked for under [`Constraints`]. Dense: every node has
/// one, unset by default, and only a leaf's is read. A known dimension is
/// one the style or the parent already fixed; return it on that axis.
///
/// The closure is `'static`, so it captures clones or `Arc`s, never
/// borrows. There is no `PartialEq`: every write is a change, which is
/// what a widget that rewrote its text wants. Taffy caches what it
/// measured, so the closure runs a few times per pass for a node whose
/// constraints it has not seen, and not at all for a cached one.
#[derive(Default)]
pub struct Measure(Option<Box<dyn Fn(Constraints) -> Size>>);

impl Component for Measure {}

impl Measure {
    /// Unset: measures as zero.
    pub fn none() -> Self {
        Self(None)
    }

    /// Always `size`, whatever the constraints.
    pub fn fixed(size: Size) -> Self {
        Self::with(move |_| size)
    }

    pub fn with(f: impl Fn(Constraints) -> Size + 'static) -> Self {
        Self(Some(Box::new(f)))
    }

    pub fn is_set(&self) -> bool {
        self.0.is_some()
    }

    /// The leaf's size under `c`; `Size::ZERO` when unset.
    pub fn measure(&self, c: Constraints) -> Size {
        match &self.0 {
            Some(f) => f(c),
            None => Size::ZERO,
        }
    }
}

impl fmt::Debug for Measure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(_) => f.write_str("Measure(set)"),
            None => f.write_str("Measure(unset)"),
        }
    }
}

// ---------------------------------------------------------------------------
// LayoutRoot, Layout
// ---------------------------------------------------------------------------

/// Marks the top of one independently laid-out tree. Only the subtree
/// under a true root is laid out, with the root's box at the origin of
/// its own coordinates, sized by its own style. A node under no root keeps
/// its default [`Layout`]. A root's subtree must not contain another root.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LayoutRoot(pub bool);

impl Component for LayoutRoot {}

/// The box resolved for a node: rounded to whole pixels, in its root's
/// coordinates. `padding` and `border` are what the content box is inset
/// by. Written by the pass only, and only when it changed, so
/// `OnChanged<Layout>` names exactly the nodes whose box moved.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Layout {
    pub rect: Rect,
    pub padding: Insets<f32>,
    pub border: Insets<f32>,
}

impl Component for Layout {}

impl Layout {
    /// The rect inside padding and border, each dimension clamped at zero.
    pub fn content(&self) -> Rect {
        self.rect.inset(Insets::new(
            self.padding.top + self.border.top,
            self.padding.right + self.border.right,
            self.padding.bottom + self.border.bottom,
            self.padding.left + self.border.left,
        ))
    }
}

// ---------------------------------------------------------------------------
// Private state
// ---------------------------------------------------------------------------

/// The pass's working state per node, both halves touched together and
/// only by the pass. `cache` is what taffy measured this node at, per
/// constraints; clearing it is how a node is marked dirty. `unrounded` is
/// the box before rounding, read back by the rounding walk so rounding
/// errors do not accumulate down the tree. Both persist across ticks
/// because a cached subtree is not revisited.
// Read from Task 6; the allow goes with it.
#[allow(dead_code)]
#[derive(Default)]
pub(crate) struct Scratch {
    pub(crate) cache: taffy::tree::Cache,
    pub(crate) unrounded: taffy::tree::Layout,
}

impl Component for Scratch {}

/// The roots with a dirty subtree, in the order they were marked.
/// Deduplicated on push by a linear scan: roots are windows, a handful.
#[derive(Default)]
pub(crate) struct DirtyRoots(pub(crate) Vec<NodeId>);

impl Resource for DirtyRoots {}

// ---------------------------------------------------------------------------
// Signal
// ---------------------------------------------------------------------------

/// Layout has run for this tick. Sent once per tick, whatever happened.
/// `roots` is the roots recomputed this tick, in the order they were
/// marked dirty; empty when none was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutDone {
    pub roots: Vec<NodeId>,
}

impl Signal for LayoutDone {}

impl LayoutDone {
    pub fn recomputed(&self) -> bool {
        !self.roots.is_empty()
    }
}

// ---------------------------------------------------------------------------
// The module
// ---------------------------------------------------------------------------

/// Registers the columns, the resource and the systems. The `PostTick`
/// system is registered *before* the components on purpose: each
/// `register_component` installs a `PostTick` drain for `OnChanged<C>`,
/// and systems for one signal run in registration order, so `pass` sees
/// the change records before the drains do and takes them. As a result
/// `OnChanged<LayoutStyle>`, `OnChanged<Measure>` and
/// `OnChanged<LayoutRoot>` never fire while this module is installed.
pub struct LayoutModule;

impl Module for LayoutModule {
    fn install(self, app: &mut App) {
        app.system(pass)
            .register_component::<LayoutStyle>()
            .register_component::<Measure>()
            .register_component::<LayoutRoot>()
            .register_component::<Layout>()
            .register_component::<Scratch>()
            .init_resource::<DirtyRoots>();
    }
}

/// The `PostTick` system: take the change records, lay out the dirty
/// roots, signal `LayoutDone`.
fn pass(app: &mut App, _: &PostTick) {
    drain(app);
    let roots = take_dirty_roots(app);
    // The pass over each root arrives in a later task.
    app.take_changed::<Scratch>().for_each(drop);
    app.signal(LayoutDone { roots });
}

/// Take the three input records. Invalidating each id arrives in a later
/// task; for now the records are taken so the core's drains find them
/// empty.
fn drain(app: &mut App) {
    app.take_changed::<LayoutStyle>().for_each(drop);
    app.take_changed::<Measure>().for_each(drop);
    app.take_changed::<LayoutRoot>().for_each(drop);
}

/// The dirty roots that are still live and still marked, in marking
/// order, leaving the resource empty.
fn take_dirty_roots(app: &mut App) -> Vec<NodeId> {
    let taken = std::mem::take(&mut app.resource_mut::<DirtyRoots>().0);
    taken
        .into_iter()
        .filter(|&root| app.is_live(root) && app.component::<LayoutRoot>(root).is_some_and(|m| m.0))
        .collect()
}
