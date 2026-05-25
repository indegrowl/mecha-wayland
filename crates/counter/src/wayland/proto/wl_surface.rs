use crate::wayland::proto::{WaylandInterface, WaylandParse, WaylandSend};
use crate::wire::{MessageBuilder, MessageReader};

pub struct WlSurface;

impl WaylandInterface for WlSurface {
    const NAME: &'static str = "wl_surface";
    const VERSION: u32 = 4;
}

pub mod event {
    use super::*;

    pub struct Enter {
        pub output: u32,
    }

    impl WaylandParse for Enter {
        const OPCODE: u16 = 0;
        fn deserialize(body: &[u8]) -> Option<Self> {
            let mut fds = vec![];
            let mut r = MessageReader::new(body, &mut fds);
            Some(Self { output: r.read_u32()? })
        }
    }

    pub struct Leave {
        pub output: u32,
    }

    impl WaylandParse for Leave {
        const OPCODE: u16 = 1;
        fn deserialize(body: &[u8]) -> Option<Self> {
            let mut fds = vec![];
            let mut r = MessageReader::new(body, &mut fds);
            Some(Self { output: r.read_u32()? })
        }
    }
}

pub mod request {
    use super::*;

    pub struct Attach {
        pub buffer: u32,
        pub x: i32,
        pub y: i32,
    }

    impl WaylandSend for Attach {
        type Interface = WlSurface;
        const OPCODE: u16 = 1;
        fn serialize(&self, builder: MessageBuilder) {
            builder.write_u32(self.buffer).write_i32(self.x).write_i32(self.y).build();
        }
    }

    pub struct Damage {
        pub x: i32,
        pub y: i32,
        pub width: i32,
        pub height: i32,
    }

    impl WaylandSend for Damage {
        type Interface = WlSurface;
        const OPCODE: u16 = 2;
        fn serialize(&self, builder: MessageBuilder) {
            builder
                .write_i32(self.x)
                .write_i32(self.y)
                .write_i32(self.width)
                .write_i32(self.height)
                .build();
        }
    }

    pub struct Frame {
        pub callback: u32,
    }

    impl WaylandSend for Frame {
        type Interface = WlSurface;
        const OPCODE: u16 = 3;
        fn serialize(&self, builder: MessageBuilder) {
            builder.write_u32(self.callback).build();
        }
    }

    pub struct Commit;

    impl WaylandSend for Commit {
        type Interface = WlSurface;
        const OPCODE: u16 = 6;
        fn serialize(&self, builder: MessageBuilder) {
            builder.build();
        }
    }
}
