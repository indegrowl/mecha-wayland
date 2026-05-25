use std::collections::HashMap;

use app::event::Event;

use crate::wayland::proto::zwlr_layer_shell as proto;
use crate::wayland::proto::Handle;
use crate::wayland::{SharedConnection, WaylandRawEvent, parse, send};

pub use proto::{ANCHOR_BOTTOM, ANCHOR_LEFT, ANCHOR_RIGHT, ANCHOR_TOP};
pub use proto::{LAYER_BACKGROUND, LAYER_BOTTOM, LAYER_OVERLAY, LAYER_TOP};

#[derive(Debug)]
pub enum LayerSurfaceEvent {
    Configured { id: u32, serial: u32, width: u32, height: u32 },
    Closed { id: u32 },
}

impl Event for LayerSurfaceEvent {}

// ── ZwlrLayerShellV1 ─────────────────────────────────────────────────────────

pub struct ZwlrLayerShellV1 {
    conn: SharedConnection,
    handle: Handle<proto::ZwlrLayerShellV1>,
}

impl ZwlrLayerShellV1 {
    pub fn new(conn: SharedConnection) -> Self {
        Self { conn, handle: Handle::new(0) }
    }

    pub fn set_id(&mut self, id: u32) {
        self.handle = Handle::new(id);
    }

    pub fn get_layer_surface(
        &self,
        surface_id: u32,
        output_id: u32,
        layer: u32,
        namespace: &str,
    ) -> u32 {
        let layer_surface_id = self.conn.borrow_mut().alloc_id();
        send(
            &self.conn,
            &self.handle,
            &proto::layer_shell_request::GetLayerSurface {
                id: layer_surface_id,
                surface: surface_id,
                output: output_id,
                layer,
                namespace: namespace.to_string(),
            },
        );
        layer_surface_id
    }
}

// ── ZwlrLayerSurfaceV1 ────────────────────────────────────────────────────────

pub struct LayerSurfaceState {
    pub closed: bool,
    pub width: u32,
    pub height: u32,
}

pub struct ZwlrLayerSurfaceV1 {
    conn: SharedConnection,
    surfaces: HashMap<u32, LayerSurfaceState>,
}

impl ZwlrLayerSurfaceV1 {
    pub fn new(conn: SharedConnection) -> Self {
        Self { conn, surfaces: HashMap::new() }
    }

    pub fn register(&mut self, id: u32) {
        self.surfaces.insert(id, LayerSurfaceState { closed: false, width: 0, height: 0 });
    }

    pub fn set_size(&self, id: u32, width: u32, height: u32) {
        let h = Handle::<proto::ZwlrLayerSurfaceV1>::new(id);
        send(&self.conn, &h, &proto::layer_surface_request::SetSize { width, height });
    }

    pub fn set_anchor(&self, id: u32, anchor: u32) {
        let h = Handle::<proto::ZwlrLayerSurfaceV1>::new(id);
        send(&self.conn, &h, &proto::layer_surface_request::SetAnchor { anchor });
    }

    pub fn set_exclusive_zone(&self, id: u32, zone: i32) {
        let h = Handle::<proto::ZwlrLayerSurfaceV1>::new(id);
        send(&self.conn, &h, &proto::layer_surface_request::SetExclusiveZone { zone });
    }

    pub fn set_margin(&self, id: u32, top: i32, right: i32, bottom: i32, left: i32) {
        let h = Handle::<proto::ZwlrLayerSurfaceV1>::new(id);
        send(&self.conn, &h, &proto::layer_surface_request::SetMargin { top, right, bottom, left });
    }

    pub fn set_keyboard_interactivity(&self, id: u32, interactivity: u32) {
        let h = Handle::<proto::ZwlrLayerSurfaceV1>::new(id);
        send(&self.conn, &h, &proto::layer_surface_request::SetKeyboardInteractivity { interactivity });
    }

    pub fn ack_configure(&self, id: u32, serial: u32) {
        let h = Handle::<proto::ZwlrLayerSurfaceV1>::new(id);
        send(&self.conn, &h, &proto::layer_surface_request::AckConfigure { serial });
    }

    pub fn process(&mut self, raw: &WaylandRawEvent) -> Option<LayerSurfaceEvent> {
        let state = self.surfaces.get_mut(&raw.sender_id)?;
        let id = raw.sender_id;
        let ev = if let Some(e) = parse::<proto::layer_surface_event::Configure>(raw) {
            state.width = e.width;
            state.height = e.height;
            LayerSurfaceEvent::Configured { id, serial: e.serial, width: e.width, height: e.height }
        } else if parse::<proto::layer_surface_event::Closed>(raw).is_some() {
            state.closed = true;
            LayerSurfaceEvent::Closed { id }
        } else {
            return None;
        };
        println!("[zwlr_layer_surface] {:?}", ev);
        Some(ev)
    }
}

#[macro_export]
macro_rules! register_zwlr_layer_surface {
    () => {
        app::module::Module::<crate::wayland::ZwlrLayerSurfaceV1>::new().processor(
            |ls: &mut crate::wayland::ZwlrLayerSurfaceV1, ev: &crate::wayland::WaylandRawEvent| {
                ls.process(ev)
            },
        )
    };
}
