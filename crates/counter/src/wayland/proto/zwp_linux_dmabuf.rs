use crate::wayland::proto::{WaylandInterface, WaylandParse, WaylandSend};
use crate::wire::{MessageBuilder, MessageReader};
use std::os::fd::RawFd;

// ── ZwpLinuxDmabufV1 ──────────────────────────────────────────────────────────

pub struct ZwpLinuxDmabufV1;

impl WaylandInterface for ZwpLinuxDmabufV1 {
    const NAME: &'static str = "zwp_linux_dmabuf_v1";
    const VERSION: u32 = 3;
}

pub mod dmabuf_event {
    use super::*;

    pub struct Format {
        pub format: u32,
    }

    impl WaylandParse for Format {
        const OPCODE: u16 = 0;
        fn deserialize(body: &[u8]) -> Option<Self> {
            let mut fds = vec![];
            let mut r = MessageReader::new(body, &mut fds);
            Some(Self { format: r.read_u32()? })
        }
    }

    pub struct Modifier {
        pub format: u32,
        pub modifier_hi: u32,
        pub modifier_lo: u32,
    }

    impl WaylandParse for Modifier {
        const OPCODE: u16 = 1;
        fn deserialize(body: &[u8]) -> Option<Self> {
            let mut fds = vec![];
            let mut r = MessageReader::new(body, &mut fds);
            Some(Self {
                format: r.read_u32()?,
                modifier_hi: r.read_u32()?,
                modifier_lo: r.read_u32()?,
            })
        }
    }
}

pub mod dmabuf_request {
    use super::*;

    pub struct CreateParams {
        pub params_id: u32,
    }

    impl WaylandSend for CreateParams {
        type Interface = ZwpLinuxDmabufV1;
        const OPCODE: u16 = 1;
        fn serialize(&self, builder: MessageBuilder) {
            builder.write_u32(self.params_id).build();
        }
    }
}

// ── ZwpLinuxBufferParamsV1 ────────────────────────────────────────────────────

pub struct ZwpLinuxBufferParamsV1;

impl WaylandInterface for ZwpLinuxBufferParamsV1 {
    const NAME: &'static str = "zwp_linux_buffer_params_v1";
    const VERSION: u32 = 3;
}

pub mod params_request {
    use super::*;

    pub struct Destroy;

    impl WaylandSend for Destroy {
        type Interface = ZwpLinuxBufferParamsV1;
        const OPCODE: u16 = 0;
        fn serialize(&self, builder: MessageBuilder) {
            builder.build();
        }
    }

    pub struct Add {
        pub fd: RawFd,
        pub plane_idx: u32,
        pub offset: u32,
        pub stride: u32,
        pub modifier_hi: u32,
        pub modifier_lo: u32,
    }

    impl WaylandSend for Add {
        type Interface = ZwpLinuxBufferParamsV1;
        const OPCODE: u16 = 1;
        fn serialize(&self, builder: MessageBuilder) {
            builder
                .write_fd(self.fd)
                .write_u32(self.plane_idx)
                .write_u32(self.offset)
                .write_u32(self.stride)
                .write_u32(self.modifier_hi)
                .write_u32(self.modifier_lo)
                .build();
        }
    }

    pub struct CreateImmed {
        pub buffer_id: u32,
        pub width: i32,
        pub height: i32,
        pub format: u32,
        pub flags: u32,
    }

    impl WaylandSend for CreateImmed {
        type Interface = ZwpLinuxBufferParamsV1;
        const OPCODE: u16 = 3;
        fn serialize(&self, builder: MessageBuilder) {
            builder
                .write_u32(self.buffer_id)
                .write_i32(self.width)
                .write_i32(self.height)
                .write_u32(self.format)
                .write_u32(self.flags)
                .build();
        }
    }
}
