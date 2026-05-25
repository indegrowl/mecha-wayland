use crate::wayland::proto::{WaylandInterface, WaylandParse};
use crate::wire::MessageReader;

pub struct WlBuffer;

impl WaylandInterface for WlBuffer {
    const NAME: &'static str = "wl_buffer";
    const VERSION: u32 = 1;
}

pub mod event {
    use super::*;

    pub struct Release;

    impl WaylandParse for Release {
        const OPCODE: u16 = 0;
        fn deserialize(_body: &[u8]) -> Option<Self> {
            Some(Self)
        }
    }
}
