#![forbid(unsafe_code)]
//! The window system integration: a surface and a shell role per
//! `Window`, configure to `Resized`, the frame callback to `Frame`, and
//! in v0 the drawer too. Crate docs are completed in Task 10.

use std::collections::HashMap;

use app::prelude::*;
use geometry::Size;
use layout::prelude::*;
use wayland::prelude::*;
use window::prelude::*;

mod shm;

use shm::Buffers;

pub use wayland::{
    ZwlrLayerShellV1Layer as Layer, ZwlrLayerSurfaceV1Anchor as Anchor,
    ZwlrLayerSurfaceV1KeyboardInteractivity as KeyboardInteractivity,
};

pub mod prelude {
    pub use crate::{
        Anchor, DEFAULT_SIZE, KeyboardInteractivity, Layer, LayerRole, PresentationModule, Role,
        Surfaces,
    };
}

/// The size of a toplevel the compositor leaves to us when the window
/// has no content to size it.
pub const DEFAULT_SIZE: Size = Size::new(640.0, 480.0);

/// What a window is to the shell. Read once, at the window's `Spawned`;
/// set it through the spawn bundle. Rewriting it later changes nothing.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Role {
    #[default]
    Toplevel,
    Layer(LayerRole),
}
impl Component for Role {}

#[derive(Debug, Clone, PartialEq)]
pub struct LayerRole {
    pub layer: Layer,
    pub anchor: Anchor,
    pub exclusive_zone: i32,
    pub namespace: String,
    pub keyboard_interactivity: KeyboardInteractivity,
}

// `Shell`'s fields and most of `Entry`'s are read starting Task 8 (`kick`,
// `settle`) and Task 9 (`on_surface`, `on_removed`); until then they are
// written but not read.
#[allow(dead_code)]
enum Shell {
    Toplevel {
        xdg: XdgSurface,
        toplevel: XdgToplevel,
    },
    Layer(ZwlrLayerSurfaceV1),
}

#[allow(dead_code)]
struct Entry {
    surface: WlSurface,
    shell: Shell,
    /// The toplevel's last proposal; zero means ours to choose.
    proposed: (i32, i32),
    /// The settled logical size.
    size: Size,
    scale: i32,
    scale_sent: i32,
    configured: bool,
    callback: Option<WlCallback>,
    wanting: bool,
    buffers: Option<Buffers>,
    last_slot: usize,
}

/// Presentation's own state: one entry per window it put on screen, and
/// the owner of every object it created.
pub struct Surfaces {
    entries: HashMap<NodeId, Entry>,
    owner: HashMap<ObjectId, NodeId>,
    layer_shell: Option<ZwlrLayerShellV1>,
    app_id: String,
}
impl Resource for Surfaces {}

impl Surfaces {
    pub fn surface_of(&self, window: NodeId) -> Option<WlSurface> {
        self.entries.get(&window).map(|e| e.surface)
    }

    pub fn is_configured(&self, window: NodeId) -> bool {
        self.entries.get(&window).is_some_and(|e| e.configured)
    }

    /// One pixel of the buffer last attached for `window`, as `XRGB8888`.
    /// `None` before the first attach. For tests and debugging.
    pub fn buffer_pixel(&self, window: NodeId, x: u32, y: u32) -> Option<u32> {
        let e = self.entries.get(&window)?;
        e.buffers.as_ref()?.pixel(e.last_slot, x, y)
    }

    /// Read starting Task 8, by the systems that answer wire events.
    #[allow(dead_code)]
    fn entry_of(&mut self, object: ObjectId) -> Option<(NodeId, &mut Entry)> {
        let w = *self.owner.get(&object)?;
        self.entries.get_mut(&w).map(|e| (w, e))
    }
}

/// Registers `Role`, inserts `Surfaces`, binds the layer shell if the
/// compositor offers one. Installs last of everything.
///
/// # Panics
///
/// If `WlCompositor`, `WlShm` or `XdgWmBase` were not bound by
/// `WaylandModule`, or `wl_compositor` is below version 4.
pub struct PresentationModule {
    pub app_id: String,
}

impl Module for PresentationModule {
    fn install(self, app: &mut App) {
        let compositor = *app.resource::<WlCompositor>();
        let _ = app.resource::<WlShm>();
        let _ = app.resource::<XdgWmBase>();
        assert!(
            app.resource::<Wayland>().version(compositor) >= 4,
            "presentation: wl_compositor must be version 4 or above for damage_buffer"
        );
        let layer_shell = app
            .resource::<Globals>()
            .find(ZwlrLayerShellV1::NAME)
            .cloned()
            .map(|g| {
                let (globals, mut wl) = app.query::<(Res<Globals>, ResMut<Wayland>)>();
                globals.bind::<ZwlrLayerShellV1>(&g, &mut wl)
            });
        app.register_component::<Role>().insert_resource(Surfaces {
            entries: HashMap::new(),
            owner: HashMap::new(),
            layer_shell,
            app_id: self.app_id,
        });
        app.system(on_spawned)
            .system(on_wm_base)
            .system(on_xdg_surface)
            .system(on_toplevel)
            .system(on_layer_surface)
            .system(on_surface)
            .system(on_frame_requested)
            .system(on_callback)
            .system(on_buffer)
            .system(on_removed)
            .system(on_frame);
    }
}

fn px_or_zero(v: Val) -> u32 {
    match v {
        Val::Px(p) => p.max(0.0) as u32,
        _ => 0,
    }
}

/// A window (its `InWindow` is itself) gets a surface, its role objects
/// and the initial commit with no buffer.
fn on_spawned(app: &mut App, s: &Spawned) {
    let is_window = app
        .component::<InWindow>(s.id)
        .is_some_and(|w| w.0 == Some(s.id));
    if !is_window {
        return;
    }
    let role = app.component::<Role>(s.id).cloned().unwrap_or_default();
    let title = app
        .widget::<Window>(s.id)
        .map(|w| w.title.clone())
        .unwrap_or_default();
    let requested = {
        let style = app
            .component::<LayoutStyle>(s.id)
            .expect("a window has a style");
        (px_or_zero(style.width), px_or_zero(style.height))
    };
    let compositor = *app.resource::<WlCompositor>();
    let wm_base = *app.resource::<XdgWmBase>();

    let (mut surfaces, mut wl) = app.query::<(ResMut<Surfaces>, ResMut<Wayland>)>();
    let surface = compositor.create_surface(&mut wl);
    let shell = match role {
        Role::Toplevel => {
            let xdg = wm_base.get_xdg_surface(&mut wl, surface);
            let toplevel = xdg.get_toplevel(&mut wl);
            toplevel.set_title(&mut wl, &title);
            toplevel.set_app_id(&mut wl, &surfaces.app_id);
            surfaces.owner.insert(xdg.id(), s.id);
            surfaces.owner.insert(toplevel.id(), s.id);
            Shell::Toplevel { xdg, toplevel }
        }
        Role::Layer(l) => {
            let shell = surfaces.layer_shell.expect(
                "presentation: a Role::Layer window needs zwlr_layer_shell_v1, which the compositor does not offer",
            );
            let ls = shell.get_layer_surface(&mut wl, surface, None, l.layer, &l.namespace);
            ls.set_anchor(&mut wl, l.anchor);
            ls.set_exclusive_zone(&mut wl, l.exclusive_zone);
            ls.set_keyboard_interactivity(&mut wl, l.keyboard_interactivity);
            ls.set_size(&mut wl, requested.0, requested.1);
            surfaces.owner.insert(ls.id(), s.id);
            Shell::Layer(ls)
        }
    };
    surface.commit(&mut wl);
    surfaces.owner.insert(surface.id(), s.id);
    surfaces.entries.insert(
        s.id,
        Entry {
            surface,
            shell,
            proposed: (0, 0),
            size: Size::ZERO,
            scale: 1,
            scale_sent: 1,
            configured: false,
            callback: None,
            wanting: false,
            buffers: None,
            last_slot: 0,
        },
    );
}

fn on_wm_base(app: &mut App, e: &XdgWmBaseEvent) {
    let XdgWmBaseEvent::Ping { wm_base, serial } = e;
    wm_base.pong(&mut app.resource_mut::<Wayland>(), *serial);
}

// The remaining systems are written in Tasks 8 and 9; until then they are
// the stubs below so the module compiles.
fn on_xdg_surface(_: &mut App, _: &XdgSurfaceEvent) {}
fn on_toplevel(_: &mut App, _: &XdgToplevelEvent) {}
fn on_layer_surface(_: &mut App, _: &ZwlrLayerSurfaceV1Event) {}
fn on_surface(_: &mut App, _: &WlSurfaceEvent) {}
fn on_frame_requested(_: &mut App, _: &FrameRequested) {}
fn on_callback(_: &mut App, _: &WlCallbackEvent) {}
fn on_buffer(_: &mut App, _: &WlBufferEvent) {}
fn on_removed(_: &mut App, _: &Removed) {}
fn on_frame(_: &mut App, _: &Frame) {}
