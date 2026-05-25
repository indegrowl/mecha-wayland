use crate::wayland::proto::{WaylandInterface, WaylandParse, WaylandSend};
use crate::wire::{MessageBuilder, MessageReader};

pub struct WlShm;

impl WaylandInterface for WlShm {
    const NAME: &'static str = "wl_shm";
    const VERSION: u32 = 1;
}

pub mod event {
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
}

pub mod request {
    use super::*;
    use std::os::fd::RawFd;

    pub struct CreatePool {
        pub id: u32,
        pub fd: RawFd,
        pub size: i32,
    }

    impl WaylandSend for CreatePool {
        type Interface = WlShm;
        const OPCODE: u16 = 0;
        fn serialize(&self, builder: MessageBuilder) {
            builder.write_u32(self.id).write_fd(self.fd).write_i32(self.size).build();
        }
    }
}

// ── wl_shm_pool (no XML marker type needed — methods called directly on the pool id) ──

pub mod pool_request {
    use crate::wayland::proto::{WaylandInterface, WaylandSend};
    use crate::wire::MessageBuilder;

    pub struct WlShmPool;

    impl WaylandInterface for WlShmPool {
        const NAME: &'static str = "wl_shm_pool";
        const VERSION: u32 = 1;
    }

    pub struct CreateBuffer {
        pub id: u32,
        pub offset: i32,
        pub width: i32,
        pub height: i32,
        pub stride: i32,
        pub format: u32,
    }

    impl WaylandSend for CreateBuffer {
        type Interface = WlShmPool;
        const OPCODE: u16 = 0;
        fn serialize(&self, builder: MessageBuilder) {
            builder
                .write_u32(self.id)
                .write_i32(self.offset)
                .write_i32(self.width)
                .write_i32(self.height)
                .write_i32(self.stride)
                .write_u32(self.format)
                .build();
        }
    }

    pub struct Destroy;

    impl WaylandSend for Destroy {
        type Interface = WlShmPool;
        const OPCODE: u16 = 1;
        fn serialize(&self, builder: MessageBuilder) {
            builder.build();
        }
    }
}
