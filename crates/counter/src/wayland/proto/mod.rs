// ── Generated layer ───────────────────────────────────────────────────────────
// This module and its submodules are the "generated" part of the wayland
// protocol stack. Everything here is derived mechanically from the XML.
//
// Hand-written code lives in the parent wayland/ modules.

pub mod wl_buffer;
pub mod wl_callback;
pub mod wl_compositor;
pub mod wl_display;
pub mod wl_keyboard;
pub mod wl_pointer;
pub mod wl_registry;
pub mod wl_seat;
pub mod wl_shm;
pub mod wl_surface;
pub mod wl_touch;
pub mod zwlr_layer_shell;
pub mod zwp_linux_dmabuf;

use std::marker::PhantomData;

use crate::wire::MessageBuilder;

// ── Core traits ───────────────────────────────────────────────────────────────

pub trait WaylandInterface {
    const NAME: &'static str;
    const VERSION: u32;
}

pub trait WaylandSend {
    type Interface: WaylandInterface;
    const OPCODE: u16;
    fn serialize(&self, builder: MessageBuilder);
}

pub trait WaylandParse: Sized {
    const OPCODE: u16;
    fn deserialize(body: &[u8]) -> Option<Self>;
}

// ── Handle ────────────────────────────────────────────────────────────────────

pub struct Handle<T: WaylandInterface> {
    pub id: u32,
    _marker: PhantomData<T>,
}

impl<T: WaylandInterface> Handle<T> {
    pub fn new(id: u32) -> Self {
        Self { id, _marker: PhantomData }
    }
}

impl<T: WaylandInterface> Clone for Handle<T> {
    fn clone(&self) -> Self {
        Self::new(self.id)
    }
}

impl<T: WaylandInterface> Copy for Handle<T> {}
