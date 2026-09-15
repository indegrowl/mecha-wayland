//! `wl_display`, `wl_registry` and `wl_callback` by hand: the three
//! interfaces whose logic the connection itself owns.

use std::collections::VecDeque;
use std::os::fd::OwnedFd;

use app::prelude::*;

use crate::wire::Reader;
use crate::{Info, Interface, ObjectId, Wayland};

macro_rules! interface {
    ($t:ident, $name:literal, $dispatch:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $t(pub ObjectId);
        impl Interface for $t {
            const NAME: &'static str = $name;
            const VERSION: u32 = 1;
            const INFO: &'static Info = &Info {
                name: $name,
                version: 1,
                dispatch: $dispatch,
            };
            fn id(self) -> ObjectId {
                self.0
            }
            fn from_id(id: ObjectId) -> Self {
                $t(id)
            }
        }
        impl Resource for $t {}
    };
}

interface!(WlDisplay, "wl_display", dispatch_display);
interface!(WlRegistry, "wl_registry", dispatch_registry);
interface!(WlCallback, "wl_callback", dispatch_callback);

#[derive(Debug)]
pub enum WlDisplayEvent {
    /// A protocol error: fatal.
    Error {
        display: WlDisplay,
        object_id: ObjectId,
        code: u32,
        message: String,
    },
    /// The server released a client id after its destructor.
    DeleteId { display: WlDisplay, id: u32 },
}
impl Signal for WlDisplayEvent {}

#[derive(Debug)]
pub enum WlRegistryEvent {
    Global {
        registry: WlRegistry,
        name: u32,
        interface: String,
        version: u32,
    },
    GlobalRemove {
        registry: WlRegistry,
        name: u32,
    },
}
impl Signal for WlRegistryEvent {}

#[derive(Debug)]
pub enum WlCallbackEvent {
    Done {
        callback: WlCallback,
        callback_data: u32,
    },
}
impl Signal for WlCallbackEvent {}

impl WlDisplay {
    pub fn sync(self, wl: &mut Wayland) -> WlCallback {
        let callback: WlCallback = wl.alloc(1);
        wl.request(self.0, 0, |w| w.new_id(callback.0));
        callback
    }

    pub fn get_registry(self, wl: &mut Wayland) -> WlRegistry {
        let registry: WlRegistry = wl.alloc(1);
        wl.request(self.0, 1, |w| w.new_id(registry.0));
        registry
    }
}

impl WlRegistry {
    /// Bind global `name` as `I` at `version`, which must not exceed the
    /// advertised one; `Globals::bind` clamps.
    pub fn bind<I: Interface>(self, wl: &mut Wayland, name: u32, version: u32) -> I {
        let object: I = wl.alloc(version);
        wl.request(self.0, 0, |w| {
            w.uint(name);
            w.string(I::NAME);
            w.uint(version);
            w.new_id(object.id());
        });
        object
    }
}

impl WlDisplayEvent {
    pub fn decode(sender: ObjectId, opcode: u16, body: &[u8]) -> Option<Self> {
        let display = WlDisplay(sender);
        let mut r = Reader::new(body);
        match opcode {
            0 => Some(WlDisplayEvent::Error {
                display,
                object_id: r.object()?,
                code: r.uint()?,
                message: r.string()?,
            }),
            1 => Some(WlDisplayEvent::DeleteId {
                display,
                id: r.uint()?,
            }),
            _ => None,
        }
    }
}

impl WlRegistryEvent {
    pub fn decode(sender: ObjectId, opcode: u16, body: &[u8]) -> Option<Self> {
        let registry = WlRegistry(sender);
        let mut r = Reader::new(body);
        match opcode {
            0 => Some(WlRegistryEvent::Global {
                registry,
                name: r.uint()?,
                interface: r.string()?,
                version: r.uint()?,
            }),
            1 => Some(WlRegistryEvent::GlobalRemove {
                registry,
                name: r.uint()?,
            }),
            _ => None,
        }
    }
}

impl WlCallbackEvent {
    pub fn decode(sender: ObjectId, opcode: u16, body: &[u8]) -> Option<Self> {
        let callback = WlCallback(sender);
        let mut r = Reader::new(body);
        match opcode {
            0 => Some(WlCallbackEvent::Done {
                callback,
                callback_data: r.uint()?,
            }),
            _ => None,
        }
    }
}

fn dispatch_display(
    app: &mut App,
    sender: ObjectId,
    opcode: u16,
    body: &[u8],
    _: &mut VecDeque<OwnedFd>,
) {
    if let Some(e) = WlDisplayEvent::decode(sender, opcode, body) {
        app.signal(e);
    }
}

fn dispatch_registry(
    app: &mut App,
    sender: ObjectId,
    opcode: u16,
    body: &[u8],
    _: &mut VecDeque<OwnedFd>,
) {
    if let Some(e) = WlRegistryEvent::decode(sender, opcode, body) {
        app.signal(e);
    }
}

fn dispatch_callback(
    app: &mut App,
    sender: ObjectId,
    opcode: u16,
    body: &[u8],
    _: &mut VecDeque<OwnedFd>,
) {
    if let Some(e) = WlCallbackEvent::decode(sender, opcode, body) {
        app.signal(e);
    }
}
