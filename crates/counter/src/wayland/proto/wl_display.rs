use crate::wayland::proto::{Handle, WaylandInterface, WaylandParse, WaylandSend};
use crate::wire::{MessageBuilder, MessageReader};

pub struct WlDisplay;

impl WaylandInterface for WlDisplay {
    const NAME: &'static str = "wl_display";
    const VERSION: u32 = 1;
}

pub mod event {
    use super::*;

    pub struct Error {
        pub object_id: u32,
        pub code: u32,
        pub message: String,
    }

    impl WaylandParse for Error {
        const OPCODE: u16 = 0;
        fn deserialize(body: &[u8]) -> Option<Self> {
            let mut fds = vec![];
            let mut r = MessageReader::new(body, &mut fds);
            Some(Self {
                object_id: r.read_u32()?,
                code: r.read_u32()?,
                message: r.read_string()?.to_string(),
            })
        }
    }

    pub struct DeleteId {
        pub id: u32,
    }

    impl WaylandParse for DeleteId {
        const OPCODE: u16 = 1;
        fn deserialize(body: &[u8]) -> Option<Self> {
            let mut fds = vec![];
            let mut r = MessageReader::new(body, &mut fds);
            Some(Self { id: r.read_u32()? })
        }
    }
}

pub mod request {
    use super::*;

    pub struct Sync {
        pub callback: u32,
    }

    impl WaylandSend for Sync {
        type Interface = WlDisplay;
        const OPCODE: u16 = 0;
        fn serialize(&self, builder: MessageBuilder) {
            builder.write_u32(self.callback).build();
        }
    }

    pub struct GetRegistry {
        pub registry: u32,
    }

    impl WaylandSend for GetRegistry {
        type Interface = WlDisplay;
        const OPCODE: u16 = 1;
        fn serialize(&self, builder: MessageBuilder) {
            builder.write_u32(self.registry).build();
        }
    }
}
