use crate::wayland::proto::{WaylandInterface, WaylandParse};
use crate::wire::MessageReader;

pub struct WlCallback;

impl WaylandInterface for WlCallback {
    const NAME: &'static str = "wl_callback";
    const VERSION: u32 = 1;
}

pub mod event {
    use super::*;

    pub struct Done {
        pub callback_data: u32,
    }

    impl WaylandParse for Done {
        const OPCODE: u16 = 0;
        fn deserialize(body: &[u8]) -> Option<Self> {
            let mut fds = vec![];
            let mut r = MessageReader::new(body, &mut fds);
            Some(Self { callback_data: r.read_u32()? })
        }
    }
}
