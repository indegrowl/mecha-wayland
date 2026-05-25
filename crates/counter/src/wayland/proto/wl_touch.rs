use crate::wayland::proto::{WaylandInterface, WaylandParse, WaylandSend};
use crate::wire::{MessageBuilder, MessageReader};

pub struct WlTouch;

impl WaylandInterface for WlTouch {
    const NAME: &'static str = "wl_touch";
    const VERSION: u32 = 7;
}

pub mod event {
    use super::*;

    pub struct Down {
        pub serial: u32,
        pub time: u32,
        pub surface: u32,
        pub id: i32,
        pub x: f64,
        pub y: f64,
    }

    impl WaylandParse for Down {
        const OPCODE: u16 = 0;
        fn deserialize(body: &[u8]) -> Option<Self> {
            let mut fds = vec![];
            let mut r = MessageReader::new(body, &mut fds);
            Some(Self {
                serial: r.read_u32()?,
                time: r.read_u32()?,
                surface: r.read_u32()?,
                id: r.read_i32()?,
                x: r.read_fixed()?,
                y: r.read_fixed()?,
            })
        }
    }

    pub struct Up {
        pub serial: u32,
        pub time: u32,
        pub id: i32,
    }

    impl WaylandParse for Up {
        const OPCODE: u16 = 1;
        fn deserialize(body: &[u8]) -> Option<Self> {
            let mut fds = vec![];
            let mut r = MessageReader::new(body, &mut fds);
            Some(Self { serial: r.read_u32()?, time: r.read_u32()?, id: r.read_i32()? })
        }
    }

    pub struct Motion {
        pub time: u32,
        pub id: i32,
        pub x: f64,
        pub y: f64,
    }

    impl WaylandParse for Motion {
        const OPCODE: u16 = 2;
        fn deserialize(body: &[u8]) -> Option<Self> {
            let mut fds = vec![];
            let mut r = MessageReader::new(body, &mut fds);
            Some(Self { time: r.read_u32()?, id: r.read_i32()?, x: r.read_fixed()?, y: r.read_fixed()? })
        }
    }

    pub struct Frame;

    impl WaylandParse for Frame {
        const OPCODE: u16 = 3;
        fn deserialize(_body: &[u8]) -> Option<Self> {
            Some(Self)
        }
    }

    pub struct Cancel;

    impl WaylandParse for Cancel {
        const OPCODE: u16 = 4;
        fn deserialize(_body: &[u8]) -> Option<Self> {
            Some(Self)
        }
    }

    pub struct Shape {
        pub id: i32,
        pub major: f64,
        pub minor: f64,
    }

    impl WaylandParse for Shape {
        const OPCODE: u16 = 5;
        fn deserialize(body: &[u8]) -> Option<Self> {
            let mut fds = vec![];
            let mut r = MessageReader::new(body, &mut fds);
            Some(Self { id: r.read_i32()?, major: r.read_fixed()?, minor: r.read_fixed()? })
        }
    }

    pub struct Orientation {
        pub id: i32,
        pub orientation: f64,
    }

    impl WaylandParse for Orientation {
        const OPCODE: u16 = 6;
        fn deserialize(body: &[u8]) -> Option<Self> {
            let mut fds = vec![];
            let mut r = MessageReader::new(body, &mut fds);
            Some(Self { id: r.read_i32()?, orientation: r.read_fixed()? })
        }
    }
}

pub mod request {
    use super::*;

    pub struct Release;

    impl WaylandSend for Release {
        type Interface = WlTouch;
        const OPCODE: u16 = 0;
        fn serialize(&self, builder: MessageBuilder) {
            builder.build();
        }
    }
}
