use app::event::Event;

use crate::wayland::proto::wl_keyboard as proto;
use crate::wayland::proto::Handle;
use crate::wayland::{SharedConnection, WaylandRawEvent, parse};

#[derive(Debug, Clone)]
pub enum KeyboardEvent {
    Keymap { format: u32, fd: i32, size: u32 },
    Enter { serial: u32, surface: u32, keys: Vec<u32> },
    Leave { serial: u32, surface: u32 },
    Key { serial: u32, time: u32, key: u32, state: u32 },
    Modifiers { serial: u32, mods_depressed: u32, mods_latched: u32, mods_locked: u32, group: u32 },
    RepeatInfo { rate: i32, delay: i32 },
}

impl Event for KeyboardEvent {}

pub struct WlKeyboard {
    _conn: SharedConnection,
    handle: Handle<proto::WlKeyboard>,
}

impl WlKeyboard {
    pub fn new(conn: SharedConnection) -> Self {
        Self { _conn: conn, handle: Handle::new(0) }
    }

    pub fn id(&self) -> u32 {
        self.handle.id
    }

    pub fn set_id(&mut self, id: u32) {
        self.handle = Handle::new(id);
    }

    pub fn process(&mut self, raw: &WaylandRawEvent) -> Option<KeyboardEvent> {
        if raw.sender_id != self.handle.id {
            return None;
        }
        let ev = if let Some(e) = parse::<proto::event::Keymap>(raw) {
            KeyboardEvent::Keymap { format: e.format, fd: e.fd, size: e.size }
        } else if let Some(e) = parse::<proto::event::Enter>(raw) {
            KeyboardEvent::Enter { serial: e.serial, surface: e.surface, keys: e.keys }
        } else if let Some(e) = parse::<proto::event::Leave>(raw) {
            KeyboardEvent::Leave { serial: e.serial, surface: e.surface }
        } else if let Some(e) = parse::<proto::event::Key>(raw) {
            KeyboardEvent::Key { serial: e.serial, time: e.time, key: e.key, state: e.state }
        } else if let Some(e) = parse::<proto::event::Modifiers>(raw) {
            KeyboardEvent::Modifiers {
                serial: e.serial,
                mods_depressed: e.mods_depressed,
                mods_latched: e.mods_latched,
                mods_locked: e.mods_locked,
                group: e.group,
            }
        } else if let Some(e) = parse::<proto::event::RepeatInfo>(raw) {
            KeyboardEvent::RepeatInfo { rate: e.rate, delay: e.delay }
        } else {
            return None;
        };
        println!("[wl_keyboard] {:?}", ev);
        Some(ev)
    }
}

#[macro_export]
macro_rules! register_wl_keyboard {
    () => {
        app::module::Module::<crate::wayland::WlKeyboard>::new().processor(
            |s: &mut crate::wayland::WlKeyboard, ev: &crate::wayland::WaylandRawEvent| s.process(ev),
        )
    };
}
