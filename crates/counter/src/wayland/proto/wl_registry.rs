use crate::wayland::proto::{WaylandInterface, WaylandParse, WaylandSend};
use crate::wire::{MessageBuilder, MessageReader};

pub struct WlRegistry;

impl WaylandInterface for WlRegistry {
    const NAME: &'static str = "wl_registry";
    const VERSION: u32 = 1;
}

pub mod event {
    use super::*;

    pub struct Global {
        pub name: u32,
        pub interface: String,
        pub version: u32,
    }

    impl WaylandParse for Global {
        const OPCODE: u16 = 0;
        fn deserialize(body: &[u8]) -> Option<Self> {
            let mut fds = vec![];
            let mut r = MessageReader::new(body, &mut fds);
            Some(Self {
                name: r.read_u32()?,
                interface: r.read_string()?.to_string(),
                version: r.read_u32()?,
            })
        }
    }

    pub struct GlobalRemove {
        pub name: u32,
    }

    impl WaylandParse for GlobalRemove {
        const OPCODE: u16 = 1;
        fn deserialize(body: &[u8]) -> Option<Self> {
            let mut fds = vec![];
            let mut r = MessageReader::new(body, &mut fds);
            Some(Self { name: r.read_u32()? })
        }
    }
}

pub mod request {
    use super::*;

    // Special case: bind's new_id has no fixed interface in the XML.
    // The interface name and version are passed explicitly on the wire.
    pub struct Bind {
        pub name: u32,
        pub interface: String,
        pub version: u32,
        pub new_id: u32,
    }

    impl WaylandSend for Bind {
        type Interface = WlRegistry;
        const OPCODE: u16 = 0;
        fn serialize(&self, builder: MessageBuilder) {
            builder
                .write_u32(self.name)
                .write_string(&self.interface)
                .write_u32(self.version)
                .write_u32(self.new_id)
                .build();
        }
    }
}
