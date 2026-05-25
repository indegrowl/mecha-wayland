use crate::wayland::proto::{WaylandInterface, WaylandSend};
use crate::wire::MessageBuilder;

pub struct WlCompositor;

impl WaylandInterface for WlCompositor {
    const NAME: &'static str = "wl_compositor";
    const VERSION: u32 = 4;
}

pub mod request {
    use super::*;

    pub struct CreateSurface {
        pub id: u32,
    }

    impl WaylandSend for CreateSurface {
        type Interface = WlCompositor;
        const OPCODE: u16 = 0;
        fn serialize(&self, builder: MessageBuilder) {
            builder.write_u32(self.id).build();
        }
    }
}
