#![forbid(unsafe_code)]
//! The window module: the `Window` widget, which window a node belongs to,
//! the list of live windows, and the frame request loop between the app and
//! whatever presents a window. Crate docs are completed in a later task.

use app::prelude::*;
use layout::prelude::*;

pub mod prelude {
    pub use crate::{InWindow, Window, WindowBuilder, WindowModule, Windows, window};
}

// ---------------------------------------------------------------------------
// Widget
// ---------------------------------------------------------------------------

/// A node presented on its own surface. Everything only a window has lives
/// here: the WSI and the renderer read one window at a time through
/// `app.widget::<Window>(w)`.
pub struct Window {
    pub title: String,
    /// The WSI's scale for the window's output; `1.0` until it says.
    scale: f32,
    /// A `FrameRequested` went out and its `Frame` has not come back.
    pending: bool,
}

impl Window {
    /// The scale factor the WSI last reported. Layout is in logical
    /// pixels; the drawer multiplies by this.
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// Whether a frame request is out and unanswered.
    pub fn is_pending(&self) -> bool {
        self.pending
    }
}

/// A window with no title and the default window style: a column, sized
/// to its content.
pub fn window() -> WindowBuilder {
    WindowBuilder {
        title: String::new(),
        layout: LayoutStyle::default().column(),
    }
}

/// What the spawn site knows about the window: its title and the box it
/// asks for. No style verbs; a whole `LayoutStyle` is given.
pub struct WindowBuilder {
    title: String,
    layout: LayoutStyle,
}

impl WindowBuilder {
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// The box the window asks for. Its size is a request until the WSI
    /// answers with `Resized`.
    pub fn layout(mut self, style: LayoutStyle) -> Self {
        self.layout = style;
        self
    }
}

impl Build for WindowBuilder {
    type Widget = Window;
}

impl Widget for Window {
    type Builder = WindowBuilder;

    /// Writes the style and the root mark, claims itself as its own
    /// window, and returns the widget.
    fn build(b: WindowBuilder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        *s.component_mut::<LayoutStyle>(me).unwrap() = b.layout;
        *s.component_mut::<LayoutRoot>(me).unwrap() = LayoutRoot(true);
        *s.component_mut::<InWindow>(me).unwrap() = InWindow(Some(me.id()));
        s.resource_mut::<Windows>().ids.push(me.id());
        Window {
            title: b.title,
            scale: 1.0,
            pending: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/// The window a node belongs to. A window's is itself; `None` on the app
/// root and on anything hung outside every window. Written once, at spawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InWindow(pub Option<NodeId>);
impl Component for InWindow {}

// ---------------------------------------------------------------------------
// Resource
// ---------------------------------------------------------------------------

/// Every live window, in spawn order. A window's build pushes it and
/// `on_removed` drops what is gone, so the list is exact after every
/// flush. Any tick that changed it fires `OnChanged<Windows>` once.
#[derive(Debug, Default)]
pub struct Windows {
    ids: Vec<NodeId>,
}
impl Resource for Windows {}

impl Windows {
    pub fn iter(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.ids.iter().copied()
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    pub fn contains(&self, id: NodeId) -> bool {
        self.ids.contains(&id)
    }
}

// ---------------------------------------------------------------------------
// Module
// ---------------------------------------------------------------------------

/// Registers `InWindow` and attaches the systems. Installs after
/// `LayoutModule`, whose components a window's build writes.
pub struct WindowModule;

impl Module for WindowModule {
    fn install(self, app: &mut App) {
        app.register_component::<InWindow>()
            .init_resource::<Windows>()
            .system(on_spawned)
            .system(on_removed);
    }
}

/// A window set its own `InWindow` in `build`; every other node copies the
/// nearest ancestor's. The walk is needed because a subtree's `Spawned`
/// signals arrive children before parent, so a parent's own value may not
/// be written yet; a window's always is, so the walk ends at the right one.
fn on_spawned(app: &mut App, s: &Spawned) {
    if app
        .component::<InWindow>(s.id)
        .is_some_and(|w| w.0.is_some())
    {
        debug_assert_eq!(
            s.parent,
            app.root(),
            "a window must be a child of the app root: {:?}",
            s.id
        );
        return;
    }
    let found = app
        .ancestors(s.id)
        .find_map(|a| app.component::<InWindow>(a).and_then(|w| w.0));
    if let Some(w) = found
        && let Some(mut slot) = app.component_mut::<InWindow>(s.id)
    {
        slot.0 = Some(w);
    }
}

/// The removed id is stale and may have had windows anywhere under it, so
/// drop every stale id. The write guard is taken only if something is
/// stale, so `OnChanged<Windows>` fires only when the list shrank.
fn on_removed(app: &mut App, _: &Removed) {
    let any_stale = app.resource::<Windows>().iter().any(|w| !app.is_live(w));
    if !any_stale {
        return;
    }
    let live: Vec<NodeId> = app
        .resource::<Windows>()
        .iter()
        .filter(|&w| app.is_live(w))
        .collect();
    app.resource_mut::<Windows>().ids = live;
}
