use std::collections::HashSet;

use app::event::Event;

use crate::wayland::proto::wl_buffer as proto;
use crate::wayland::{SharedConnection, WaylandRawEvent, parse};

#[derive(Debug)]
pub enum WlBufferEvent {
    Release { id: u32 },
}

impl Event for WlBufferEvent {}

pub struct WlBuffer {
    _conn: SharedConnection,
    ids: HashSet<u32>,
}

impl WlBuffer {
    pub fn new(conn: SharedConnection) -> Self {
        Self { _conn: conn, ids: HashSet::new() }
    }

    pub fn register(&mut self, id: u32) {
        self.ids.insert(id);
    }

    pub fn process(&mut self, raw: &WaylandRawEvent) -> Option<WlBufferEvent> {
        if !self.ids.contains(&raw.sender_id) {
            return None;
        }
        parse::<proto::event::Release>(raw)
            .map(|_| WlBufferEvent::Release { id: raw.sender_id })
    }
}

#[macro_export]
macro_rules! register_wl_buffer {
    () => {
        app::module::Module::<crate::wayland::WlBuffer>::new().processor(
            |b: &mut crate::wayland::WlBuffer, ev: &crate::wayland::WaylandRawEvent| b.process(ev),
        )
    };
}
