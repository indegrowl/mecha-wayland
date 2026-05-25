use crate::wayland::proto::{WaylandInterface, WaylandParse, WaylandSend};
use crate::wire::{MessageBuilder, MessageReader};

// ── ZwlrLayerShellV1 ──────────────────────────────────────────────────────────

pub struct ZwlrLayerShellV1;

impl WaylandInterface for ZwlrLayerShellV1 {
    const NAME: &'static str = "zwlr_layer_shell_v1";
    const VERSION: u32 = 4;
}

pub mod layer_shell_request {
    use super::*;

    pub struct GetLayerSurface {
        pub id: u32,
        pub surface: u32,
        pub output: u32,
        pub layer: u32,
        pub namespace: String,
    }

    impl WaylandSend for GetLayerSurface {
        type Interface = ZwlrLayerShellV1;
        const OPCODE: u16 = 0;
        fn serialize(&self, builder: MessageBuilder) {
            builder
                .write_u32(self.id)
                .write_u32(self.surface)
                .write_u32(self.output)
                .write_u32(self.layer)
                .write_string(&self.namespace)
                .build();
        }
    }
}

// ── ZwlrLayerSurfaceV1 ────────────────────────────────────────────────────────

pub struct ZwlrLayerSurfaceV1;

impl WaylandInterface for ZwlrLayerSurfaceV1 {
    const NAME: &'static str = "zwlr_layer_surface_v1";
    const VERSION: u32 = 4;
}

pub mod layer_surface_event {
    use super::*;

    pub struct Configure {
        pub serial: u32,
        pub width: u32,
        pub height: u32,
    }

    impl WaylandParse for Configure {
        const OPCODE: u16 = 0;
        fn deserialize(body: &[u8]) -> Option<Self> {
            let mut fds = vec![];
            let mut r = MessageReader::new(body, &mut fds);
            Some(Self {
                serial: r.read_u32()?,
                width: r.read_u32()?,
                height: r.read_u32()?,
            })
        }
    }

    pub struct Closed;

    impl WaylandParse for Closed {
        const OPCODE: u16 = 1;
        fn deserialize(_body: &[u8]) -> Option<Self> {
            Some(Self)
        }
    }
}

pub mod layer_surface_request {
    use super::*;

    pub struct SetSize {
        pub width: u32,
        pub height: u32,
    }

    impl WaylandSend for SetSize {
        type Interface = ZwlrLayerSurfaceV1;
        const OPCODE: u16 = 0;
        fn serialize(&self, builder: MessageBuilder) {
            builder.write_u32(self.width).write_u32(self.height).build();
        }
    }

    pub struct SetAnchor {
        pub anchor: u32,
    }

    impl WaylandSend for SetAnchor {
        type Interface = ZwlrLayerSurfaceV1;
        const OPCODE: u16 = 1;
        fn serialize(&self, builder: MessageBuilder) {
            builder.write_u32(self.anchor).build();
        }
    }

    pub struct SetExclusiveZone {
        pub zone: i32,
    }

    impl WaylandSend for SetExclusiveZone {
        type Interface = ZwlrLayerSurfaceV1;
        const OPCODE: u16 = 2;
        fn serialize(&self, builder: MessageBuilder) {
            builder.write_i32(self.zone).build();
        }
    }

    pub struct SetMargin {
        pub top: i32,
        pub right: i32,
        pub bottom: i32,
        pub left: i32,
    }

    impl WaylandSend for SetMargin {
        type Interface = ZwlrLayerSurfaceV1;
        const OPCODE: u16 = 3;
        fn serialize(&self, builder: MessageBuilder) {
            builder
                .write_i32(self.top)
                .write_i32(self.right)
                .write_i32(self.bottom)
                .write_i32(self.left)
                .build();
        }
    }

    pub struct SetKeyboardInteractivity {
        pub interactivity: u32,
    }

    impl WaylandSend for SetKeyboardInteractivity {
        type Interface = ZwlrLayerSurfaceV1;
        const OPCODE: u16 = 4;
        fn serialize(&self, builder: MessageBuilder) {
            builder.write_u32(self.interactivity).build();
        }
    }

    pub struct AckConfigure {
        pub serial: u32,
    }

    impl WaylandSend for AckConfigure {
        type Interface = ZwlrLayerSurfaceV1;
        const OPCODE: u16 = 6;
        fn serialize(&self, builder: MessageBuilder) {
            builder.write_u32(self.serial).build();
        }
    }
}

// ── Layer and anchor constants ────────────────────────────────────────────────

pub const LAYER_BACKGROUND: u32 = 0;
pub const LAYER_BOTTOM: u32 = 1;
pub const LAYER_TOP: u32 = 2;
pub const LAYER_OVERLAY: u32 = 3;

pub const ANCHOR_TOP: u32 = 1;
pub const ANCHOR_BOTTOM: u32 = 2;
pub const ANCHOR_LEFT: u32 = 4;
pub const ANCHOR_RIGHT: u32 = 8;
