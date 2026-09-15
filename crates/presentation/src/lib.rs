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

enum Shell {
    Toplevel {
        xdg: XdgSurface,
        toplevel: XdgToplevel,
    },
    Layer(ZwlrLayerSurfaceV1),
}

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

fn device(size: Size, scale: i32) -> (u32, u32) {
    let s = scale.max(1) as f32;
    (
        ((size.width * s).round() as u32).max(1),
        ((size.height * s).round() as u32).max(1),
    )
}

/// Forget both buffers' owner rows, then destroy the buffers and the pool.
fn teardown_buffers(buffers: Buffers, owner: &mut HashMap<ObjectId, NodeId>, wl: &mut Wayland) {
    for s in &buffers.slots {
        owner.remove(&s.buffer.id());
    }
    buffers.destroy(wl);
}

/// Drop the buffers, if any, and make new ones at the entry's size and
/// scale, owned by `w`.
fn replace_buffers(
    entry: &mut Entry,
    owner: &mut HashMap<ObjectId, NodeId>,
    wl: &mut Wayland,
    shm: WlShm,
    w: NodeId,
) {
    if let Some(old) = entry.buffers.take() {
        teardown_buffers(old, owner, wl);
    }
    let (dw, dh) = device(entry.size, entry.scale);
    let buffers = Buffers::create(wl, shm, dw, dh);
    for s in &buffers.slots {
        owner.insert(s.buffer.id(), w);
    }
    entry.buffers = Some(buffers);
}

/// The shell configured `w`: settle the size, make buffers if the device
/// size changed, report `Resized`, and want a frame.
fn settle(app: &mut App, w: NodeId, proposed: (i32, i32)) {
    let layout = app
        .component::<Layout>(w)
        .map(|l| l.rect.size)
        .unwrap_or(Size::ZERO);
    let pick = |p: i32, l: f32, d: f32| {
        if p > 0 {
            p as f32
        } else if l > 0.0 {
            l
        } else {
            d
        }
    };
    let size = Size::new(
        pick(proposed.0, layout.width, DEFAULT_SIZE.width),
        pick(proposed.1, layout.height, DEFAULT_SIZE.height),
    );
    let shm = *app.resource::<WlShm>();
    {
        let (mut surfaces, mut wl) = app.query::<(ResMut<Surfaces>, ResMut<Wayland>)>();
        let s = &mut *surfaces;
        let Some(entry) = s.entries.get_mut(&w) else {
            return;
        };
        entry.size = size;
        let (dw, dh) = device(size, entry.scale);
        let stale = entry
            .buffers
            .as_ref()
            .is_none_or(|b| (b.width, b.height) != (dw, dh));
        if stale {
            replace_buffers(entry, &mut s.owner, &mut wl, shm, w);
        }
        entry.configured = true;
        entry.wanting = true;
    }
    app.emit(Resized { size }, w);
    kick(app, w);
}

/// The one decision: configured, no callback outstanding, a buffer free
/// and wanting gives `Frame(w)`. Otherwise the next configure, `done` or
/// `release` kicks again.
fn kick(app: &mut App, w: NodeId) {
    let ready = app.resource::<Surfaces>().entries.get(&w).is_some_and(|e| {
        e.configured
            && e.callback.is_none()
            && e.wanting
            && e.buffers.as_ref().is_some_and(|b| b.free_slot().is_some())
    });
    if ready {
        app.resource_mut::<Surfaces>()
            .entries
            .get_mut(&w)
            .unwrap()
            .wanting = false;
        app.signal(Frame(w));
    }
}

fn on_toplevel(app: &mut App, e: &XdgToplevelEvent) {
    match e {
        XdgToplevelEvent::Configure {
            toplevel,
            width,
            height,
            ..
        } => {
            if let Some((_, entry)) = app.resource_mut::<Surfaces>().entry_of(toplevel.id()) {
                entry.proposed = (*width, *height);
            }
        }
        XdgToplevelEvent::Close { toplevel } => {
            if let Some(w) = app
                .resource::<Surfaces>()
                .owner
                .get(&toplevel.id())
                .copied()
            {
                app.emit(CloseRequested, w);
            }
        }
        _ => {}
    }
}

fn on_xdg_surface(app: &mut App, e: &XdgSurfaceEvent) {
    let XdgSurfaceEvent::Configure { surface, serial } = e;
    let found = {
        let (mut surfaces, mut wl) = app.query::<(ResMut<Surfaces>, ResMut<Wayland>)>();
        surfaces.entry_of(surface.id()).map(|(w, entry)| {
            surface.ack_configure(&mut wl, *serial);
            (w, entry.proposed)
        })
    };
    if let Some((w, proposed)) = found {
        settle(app, w, proposed);
    }
}

fn on_layer_surface(app: &mut App, e: &ZwlrLayerSurfaceV1Event) {
    match e {
        ZwlrLayerSurfaceV1Event::Configure {
            layer_surface,
            serial,
            width,
            height,
        } => {
            let found = {
                let (mut surfaces, mut wl) = app.query::<(ResMut<Surfaces>, ResMut<Wayland>)>();
                surfaces.entry_of(layer_surface.id()).map(|(w, _)| {
                    layer_surface.ack_configure(&mut wl, *serial);
                    w
                })
            };
            if let Some(w) = found {
                settle(app, w, (*width as i32, *height as i32));
            }
        }
        ZwlrLayerSurfaceV1Event::Closed { layer_surface } => {
            if let Some(w) = app
                .resource::<Surfaces>()
                .owner
                .get(&layer_surface.id())
                .copied()
            {
                app.emit(CloseRequested, w);
            }
        }
    }
}

fn on_frame_requested(app: &mut App, r: &FrameRequested) {
    if let Some(entry) = app.resource_mut::<Surfaces>().entries.get_mut(&r.0) {
        entry.wanting = true;
    }
    kick(app, r.0);
}

/// The drawer in v0, and the commit after every drawer for good: the last
/// `Frame` system to run.
fn on_frame(app: &mut App, f: &Frame) {
    let w = f.0;
    let Some(clear) = app.widget::<Window>(w).map(|win| win.clear()) else {
        return;
    };
    let (mut surfaces, mut wl) = app.query::<(ResMut<Surfaces>, ResMut<Wayland>)>();
    let s = &mut *surfaces;
    let Some(entry) = s.entries.get_mut(&w) else {
        return;
    };
    if !entry.configured {
        return;
    }
    if entry.callback.is_some() {
        entry.wanting = true;
        return;
    }
    let Some(buffers) = entry.buffers.as_mut() else {
        return;
    };
    let Some(slot) = buffers.free_slot() else {
        entry.wanting = true;
        return;
    };
    buffers.fill(slot, clear);
    if entry.scale != entry.scale_sent {
        entry.surface.set_buffer_scale(&mut wl, entry.scale);
        entry.scale_sent = entry.scale;
    }
    entry
        .surface
        .attach(&mut wl, Some(buffers.slots[slot].buffer), 0, 0);
    entry
        .surface
        .damage_buffer(&mut wl, 0, 0, buffers.width as i32, buffers.height as i32);
    let callback = entry.surface.frame(&mut wl);
    s.owner.insert(callback.id(), w);
    entry.callback = Some(callback);
    entry.surface.commit(&mut wl);
    buffers.slots[slot].held = true;
    entry.last_slot = slot;
}

fn on_callback(app: &mut App, e: &WlCallbackEvent) {
    let WlCallbackEvent::Done { callback, .. } = e;
    let w = {
        let mut surfaces = app.resource_mut::<Surfaces>();
        let Some(w) = surfaces.owner.remove(&callback.id()) else {
            return;
        };
        if let Some(entry) = surfaces.entries.get_mut(&w)
            && entry.callback == Some(*callback)
        {
            entry.callback = None;
        }
        w
    };
    kick(app, w);
}

fn on_buffer(app: &mut App, e: &WlBufferEvent) {
    let WlBufferEvent::Release { buffer } = e;
    let w = {
        let mut surfaces = app.resource_mut::<Surfaces>();
        let Some((w, entry)) = surfaces.entry_of(buffer.id()) else {
            return;
        };
        if let Some(b) = entry.buffers.as_mut()
            && let Some(slot) = b.slots.iter_mut().find(|s| s.buffer == *buffer)
        {
            slot.held = false;
        }
        w
    };
    kick(app, w);
}

/// A new preferred scale: report it, and if configured rescale the
/// buffers and want a frame. The same scale again does nothing.
fn on_surface(app: &mut App, e: &WlSurfaceEvent) {
    let WlSurfaceEvent::PreferredBufferScale { surface, factor } = e else {
        return;
    };
    let shm = *app.resource::<WlShm>();
    let changed = {
        let (mut surfaces, mut wl) = app.query::<(ResMut<Surfaces>, ResMut<Wayland>)>();
        let s = &mut *surfaces;
        let Some(w) = s.owner.get(&surface.id()).copied() else {
            return;
        };
        let entry = s.entries.get_mut(&w).expect("owned objects have entries");
        if entry.scale == *factor {
            None
        } else {
            entry.scale = *factor;
            if entry.configured {
                replace_buffers(entry, &mut s.owner, &mut wl, shm, w);
                entry.wanting = true;
            }
            Some(w)
        }
    };
    if let Some(w) = changed {
        app.emit(
            ScaleFactorChanged {
                scale: *factor as f32,
            },
            w,
        );
        kick(app, w);
    }
}

/// Every entry whose window is gone is torn down: the role objects, the
/// buffers and the pool, then the surface. A late event for any of them
/// finds no owner and is skipped.
fn on_removed(app: &mut App, _: &Removed) {
    let gone: Vec<NodeId> = {
        let surfaces = app.resource::<Surfaces>();
        surfaces
            .entries
            .keys()
            .copied()
            .filter(|&w| !app.is_live(w))
            .collect()
    };
    if gone.is_empty() {
        return;
    }
    let (mut surfaces, mut wl) = app.query::<(ResMut<Surfaces>, ResMut<Wayland>)>();
    let s = &mut *surfaces;
    for w in gone {
        let entry = s.entries.remove(&w).unwrap();
        match entry.shell {
            Shell::Toplevel { xdg, toplevel } => {
                toplevel.destroy(&mut wl);
                xdg.destroy(&mut wl);
                s.owner.remove(&toplevel.id());
                s.owner.remove(&xdg.id());
            }
            Shell::Layer(ls) => {
                ls.destroy(&mut wl);
                s.owner.remove(&ls.id());
            }
        }
        if let Some(buffers) = entry.buffers {
            teardown_buffers(buffers, &mut s.owner, &mut wl);
        }
        entry.surface.destroy(&mut wl);
        s.owner.remove(&entry.surface.id());
        if let Some(cb) = entry.callback {
            s.owner.remove(&cb.id());
        }
    }
}
