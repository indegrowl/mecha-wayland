use crate::wayland::proto::{WaylandInterface, WaylandParse, WaylandSend};
use crate::wire::{MessageBuilder, MessageReader};

pub struct WlKeyboard;

impl WaylandInterface for WlKeyboard {
    const NAME: &'static str = "wl_keyboard";
    const VERSION: u32 = 7;
}

pub mod event {
    use super::*;

    pub struct Keymap {
        pub format: u32,
        pub fd: i32,
        pub size: u32,
    }

    impl WaylandParse for Keymap {
        const OPCODE: u16 = 0;
        fn deserialize(body: &[u8]) -> Option<Self> {
            let mut fds = vec![];
            let mut r = MessageReader::new(body, &mut fds);
            Some(Self { format: r.read_u32()?, fd: r.read_fd()?, size: r.read_u32()? })
        }
    }

    pub struct Enter {
        pub serial: u32,
        pub surface: u32,
        pub keys: Vec<u32>,
    }

    impl WaylandParse for Enter {
        const OPCODE: u16 = 1;
        fn deserialize(body: &[u8]) -> Option<Self> {
            let mut fds = vec![];
            let mut r = MessageReader::new(body, &mut fds);
            let serial = r.read_u32()?;
            let surface = r.read_u32()?;
            let keys_data = r.read_array()?;
            let keys = keys_data
                .chunks_exact(4)
                .map(|c| u32::from_ne_bytes(c.try_into().unwrap()))
                .collect();
            Some(Self { serial, surface, keys })
        }
    }

    pub struct Leave {
        pub serial: u32,
        pub surface: u32,
    }

    impl WaylandParse for Leave {
        const OPCODE: u16 = 2;
        fn deserialize(body: &[u8]) -> Option<Self> {
            let mut fds = vec![];
            let mut r = MessageReader::new(body, &mut fds);
            Some(Self { serial: r.read_u32()?, surface: r.read_u32()? })
        }
    }

    pub struct Key {
        pub serial: u32,
        pub time: u32,
        pub key: u32,
        pub state: u32,
    }

    impl WaylandParse for Key {
        const OPCODE: u16 = 3;
        fn deserialize(body: &[u8]) -> Option<Self> {
            let mut fds = vec![];
            let mut r = MessageReader::new(body, &mut fds);
            Some(Self {
                serial: r.read_u32()?,
                time: r.read_u32()?,
                key: r.read_u32()?,
                state: r.read_u32()?,
            })
        }
    }

    pub struct Modifiers {
        pub serial: u32,
        pub mods_depressed: u32,
        pub mods_latched: u32,
        pub mods_locked: u32,
        pub group: u32,
    }

    impl WaylandParse for Modifiers {
        const OPCODE: u16 = 4;
        fn deserialize(body: &[u8]) -> Option<Self> {
            let mut fds = vec![];
            let mut r = MessageReader::new(body, &mut fds);
            Some(Self {
                serial: r.read_u32()?,
                mods_depressed: r.read_u32()?,
                mods_latched: r.read_u32()?,
                mods_locked: r.read_u32()?,
                group: r.read_u32()?,
            })
        }
    }

    pub struct RepeatInfo {
        pub rate: i32,
        pub delay: i32,
    }

    impl WaylandParse for RepeatInfo {
        const OPCODE: u16 = 5;
        fn deserialize(body: &[u8]) -> Option<Self> {
            let mut fds = vec![];
            let mut r = MessageReader::new(body, &mut fds);
            Some(Self { rate: r.read_i32()?, delay: r.read_i32()? })
        }
    }
}

pub mod request {
    use super::*;

    pub struct Release;

    impl WaylandSend for Release {
        type Interface = WlKeyboard;
        const OPCODE: u16 = 0;
        fn serialize(&self, builder: MessageBuilder) {
            builder.build();
        }
    }
}
