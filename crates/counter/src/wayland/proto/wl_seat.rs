use crate::wayland::proto::{WaylandInterface, WaylandParse, WaylandSend};
use crate::wire::{MessageBuilder, MessageReader};

pub struct WlSeat;

impl WaylandInterface for WlSeat {
    const NAME: &'static str = "wl_seat";
    const VERSION: u32 = 7;
}

pub mod event {
    use super::*;

    pub struct Capabilities {
        pub capabilities: u32,
    }

    impl WaylandParse for Capabilities {
        const OPCODE: u16 = 0;
        fn deserialize(body: &[u8]) -> Option<Self> {
            let mut fds = vec![];
            let mut r = MessageReader::new(body, &mut fds);
            Some(Self { capabilities: r.read_u32()? })
        }
    }

    pub struct Name {
        pub name: String,
    }

    impl WaylandParse for Name {
        const OPCODE: u16 = 1;
        fn deserialize(body: &[u8]) -> Option<Self> {
            let mut fds = vec![];
            let mut r = MessageReader::new(body, &mut fds);
            Some(Self { name: r.read_string()?.to_string() })
        }
    }
}

pub mod request {
    use super::*;

    pub struct GetPointer {
        pub id: u32,
    }

    impl WaylandSend for GetPointer {
        type Interface = WlSeat;
        const OPCODE: u16 = 0;
        fn serialize(&self, builder: MessageBuilder) {
            builder.write_u32(self.id).build();
        }
    }

    pub struct GetKeyboard {
        pub id: u32,
    }

    impl WaylandSend for GetKeyboard {
        type Interface = WlSeat;
        const OPCODE: u16 = 1;
        fn serialize(&self, builder: MessageBuilder) {
            builder.write_u32(self.id).build();
        }
    }

    pub struct GetTouch {
        pub id: u32,
    }

    impl WaylandSend for GetTouch {
        type Interface = WlSeat;
        const OPCODE: u16 = 2;
        fn serialize(&self, builder: MessageBuilder) {
            builder.write_u32(self.id).build();
        }
    }

    pub struct Release;

    impl WaylandSend for Release {
        type Interface = WlSeat;
        const OPCODE: u16 = 3;
        fn serialize(&self, builder: MessageBuilder) {
            builder.build();
        }
    }
}
