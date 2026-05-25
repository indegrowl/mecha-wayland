use crate::wayland::proto::{WaylandInterface, WaylandParse, WaylandSend};
use crate::wire::{MessageBuilder, MessageReader};

pub struct WlPointer;

impl WaylandInterface for WlPointer {
    const NAME: &'static str = "wl_pointer";
    const VERSION: u32 = 7;
}

pub mod event {
    use super::*;

    pub struct Enter {
        pub serial: u32,
        pub surface: u32,
        pub surface_x: f64,
        pub surface_y: f64,
    }

    impl WaylandParse for Enter {
        const OPCODE: u16 = 0;
        fn deserialize(body: &[u8]) -> Option<Self> {
            let mut fds = vec![];
            let mut r = MessageReader::new(body, &mut fds);
            Some(Self {
                serial: r.read_u32()?,
                surface: r.read_u32()?,
                surface_x: r.read_fixed()?,
                surface_y: r.read_fixed()?,
            })
        }
    }

    pub struct Leave {
        pub serial: u32,
        pub surface: u32,
    }

    impl WaylandParse for Leave {
        const OPCODE: u16 = 1;
        fn deserialize(body: &[u8]) -> Option<Self> {
            let mut fds = vec![];
            let mut r = MessageReader::new(body, &mut fds);
            Some(Self { serial: r.read_u32()?, surface: r.read_u32()? })
        }
    }

    pub struct Motion {
        pub time: u32,
        pub surface_x: f64,
        pub surface_y: f64,
    }

    impl WaylandParse for Motion {
        const OPCODE: u16 = 2;
        fn deserialize(body: &[u8]) -> Option<Self> {
            let mut fds = vec![];
            let mut r = MessageReader::new(body, &mut fds);
            Some(Self { time: r.read_u32()?, surface_x: r.read_fixed()?, surface_y: r.read_fixed()? })
        }
    }

    pub struct Button {
        pub serial: u32,
        pub time: u32,
        pub button: u32,
        pub state: u32,
    }

    impl WaylandParse for Button {
        const OPCODE: u16 = 3;
        fn deserialize(body: &[u8]) -> Option<Self> {
            let mut fds = vec![];
            let mut r = MessageReader::new(body, &mut fds);
            Some(Self {
                serial: r.read_u32()?,
                time: r.read_u32()?,
                button: r.read_u32()?,
                state: r.read_u32()?,
            })
        }
    }

    pub struct Axis {
        pub time: u32,
        pub axis: u32,
        pub value: f64,
    }

    impl WaylandParse for Axis {
        const OPCODE: u16 = 4;
        fn deserialize(body: &[u8]) -> Option<Self> {
            let mut fds = vec![];
            let mut r = MessageReader::new(body, &mut fds);
            Some(Self { time: r.read_u32()?, axis: r.read_u32()?, value: r.read_fixed()? })
        }
    }

    pub struct Frame;

    impl WaylandParse for Frame {
        const OPCODE: u16 = 5;
        fn deserialize(_body: &[u8]) -> Option<Self> {
            Some(Self)
        }
    }

    pub struct AxisSource {
        pub axis_source: u32,
    }

    impl WaylandParse for AxisSource {
        const OPCODE: u16 = 6;
        fn deserialize(body: &[u8]) -> Option<Self> {
            let mut fds = vec![];
            let mut r = MessageReader::new(body, &mut fds);
            Some(Self { axis_source: r.read_u32()? })
        }
    }

    pub struct AxisStop {
        pub time: u32,
        pub axis: u32,
    }

    impl WaylandParse for AxisStop {
        const OPCODE: u16 = 7;
        fn deserialize(body: &[u8]) -> Option<Self> {
            let mut fds = vec![];
            let mut r = MessageReader::new(body, &mut fds);
            Some(Self { time: r.read_u32()?, axis: r.read_u32()? })
        }
    }

    pub struct AxisDiscrete {
        pub axis: u32,
        pub discrete: i32,
    }

    impl WaylandParse for AxisDiscrete {
        const OPCODE: u16 = 8;
        fn deserialize(body: &[u8]) -> Option<Self> {
            let mut fds = vec![];
            let mut r = MessageReader::new(body, &mut fds);
            Some(Self { axis: r.read_u32()?, discrete: r.read_i32()? })
        }
    }
}

pub mod request {
    use super::*;

    pub struct SetCursor {
        pub serial: u32,
        pub surface: u32,
        pub hotspot_x: i32,
        pub hotspot_y: i32,
    }

    impl WaylandSend for SetCursor {
        type Interface = WlPointer;
        const OPCODE: u16 = 0;
        fn serialize(&self, builder: MessageBuilder) {
            builder
                .write_u32(self.serial)
                .write_u32(self.surface)
                .write_i32(self.hotspot_x)
                .write_i32(self.hotspot_y)
                .build();
        }
    }

    pub struct Release;

    impl WaylandSend for Release {
        type Interface = WlPointer;
        const OPCODE: u16 = 1;
        fn serialize(&self, builder: MessageBuilder) {
            builder.build();
        }
    }
}
