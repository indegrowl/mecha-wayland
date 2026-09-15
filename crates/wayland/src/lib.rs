#![forbid(unsafe_code)]
//! The Wayland client as a resource. Crate docs are completed in Task 6.

use std::collections::{HashMap, VecDeque};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use app::prelude::*;

pub mod display;
pub mod generated;
pub mod wire;

pub use display::{
    WlCallback, WlCallbackEvent, WlDisplay, WlDisplayEvent, WlRegistry, WlRegistryEvent,
};
pub use generated::*;

use wire::Writer;

/// A protocol object's id. Client ids count up from 2; the server's
/// start at [`SERVER_ID_BASE`]; 1 is the display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectId(pub u32);

pub const DISPLAY: ObjectId = ObjectId(1);
pub const SERVER_ID_BASE: u32 = 0xff00_0000;

/// What the connection knows about an interface: its name, its XML
/// version and the decoder that turns one event into a signal.
pub struct Info {
    pub name: &'static str,
    pub version: u32,
    /// Decode one event of an object of this interface and signal it:
    /// the sender, the opcode, the body after the header, and the
    /// received fds to pop `fd` arguments from. A malformed body is
    /// skipped.
    pub dispatch: fn(&mut App, ObjectId, u16, &[u8], &mut VecDeque<OwnedFd>),
}

/// One interface: a `Copy` newtype over [`ObjectId`] with generated
/// request methods over `&mut Wayland`.
pub trait Interface: Copy + 'static {
    const NAME: &'static str;
    /// The XML's version; a bind never exceeds it.
    const VERSION: u32;
    const INFO: &'static Info;
    fn id(self) -> ObjectId;
    fn from_id(id: ObjectId) -> Self;
}

struct Object {
    info: &'static Info,
    version: u32,
}

/// The connection: the socket, the request buffer, the object table.
/// Requests append to the buffer; `BeforeWait` sends it (Task 5).
pub struct Wayland {
    fd: OwnedFd,
    out: Vec<u8>,
    out_fds: Vec<OwnedFd>,
    objects: Vec<Option<Object>>,
    server: HashMap<u32, Object>,
    free: Vec<u32>,
}
impl Resource for Wayland {}

impl Wayland {
    /// Over a stream the caller made: a test's socketpair.
    pub fn over(stream: UnixStream) -> Self {
        let display = Object {
            info: display::WlDisplay::INFO,
            version: 1,
        };
        Wayland {
            fd: OwnedFd::from(stream),
            out: Vec::with_capacity(4096),
            out_fds: Vec::new(),
            objects: vec![None, Some(display)],
            server: HashMap::new(),
            free: Vec::new(),
        }
    }

    /// `$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY`, or `WAYLAND_DISPLAY` alone if
    /// it is absolute; `wayland-0` when unset.
    ///
    /// # Panics
    ///
    /// If `XDG_RUNTIME_DIR` is unset when needed, or the socket does not
    /// connect.
    pub fn connect() -> Self {
        let display = std::env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "wayland-0".into());
        let path = if display.starts_with('/') {
            PathBuf::from(display)
        } else {
            let dir =
                std::env::var("XDG_RUNTIME_DIR").expect("wayland: XDG_RUNTIME_DIR is not set");
            PathBuf::from(dir).join(display)
        };
        let stream = UnixStream::connect(&path)
            .unwrap_or_else(|e| panic!("wayland: cannot connect to {}: {e}", path.display()));
        Self::over(stream)
    }

    pub fn display(&self) -> display::WlDisplay {
        display::WlDisplay(DISPLAY)
    }

    pub fn fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }

    /// A new client object of `I` at `version`: a freed id if any, else
    /// the next.
    pub fn alloc<I: Interface>(&mut self, version: u32) -> I {
        let id = match self.free.pop() {
            Some(id) => id,
            None => {
                self.objects.push(None);
                (self.objects.len() - 1) as u32
            }
        };
        self.objects[id as usize] = Some(Object {
            info: I::INFO,
            version,
        });
        I::from_id(ObjectId(id))
    }

    /// Record an object the server created (an event's `new_id`), or any
    /// id the caller wants known.
    pub fn register(&mut self, id: ObjectId, info: &'static Info, version: u32) {
        let object = Object { info, version };
        if id.0 >= SERVER_ID_BASE {
            self.server.insert(id.0, object);
        } else {
            let i = id.0 as usize;
            if self.objects.len() <= i {
                self.objects.resize_with(i + 1, || None);
            }
            self.objects[i] = Some(object);
        }
    }

    pub fn info(&self, id: ObjectId) -> Option<(&'static Info, u32)> {
        let object = if id.0 >= SERVER_ID_BASE {
            self.server.get(&id.0)
        } else {
            self.objects.get(id.0 as usize)?.as_ref()
        };
        object.map(|o| (o.info, o.version))
    }

    /// The version an object was created with.
    ///
    /// # Panics
    ///
    /// If the object is not known: a request on a destroyed object.
    pub fn version(&self, object: impl Interface) -> u32 {
        self.info(object.id())
            .unwrap_or_else(|| panic!("wayland: unknown object {:?}", object.id()))
            .1
    }

    /// Forget an id; a client id goes back to the free list.
    pub fn free(&mut self, id: ObjectId) {
        if id.0 >= SERVER_ID_BASE {
            self.server.remove(&id.0);
        } else if let Some(slot) = self.objects.get_mut(id.0 as usize)
            && slot.take().is_some()
        {
            self.free.push(id.0);
        }
    }

    /// Append one request. Generated methods call this.
    pub fn request(&mut self, sender: ObjectId, opcode: u16, args: impl FnOnce(&mut Writer<'_>)) {
        let mut w = Writer::begin(&mut self.out, &mut self.out_fds, sender, opcode);
        args(&mut w);
    }

    /// Whether requests are buffered and not yet sent.
    pub fn has_pending(&self) -> bool {
        !self.out.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixStream;

    #[derive(Clone, Copy)]
    struct Probe(ObjectId);
    fn no_dispatch(_: &mut App, _: ObjectId, _: u16, _: &[u8], _: &mut VecDeque<OwnedFd>) {}
    impl Interface for Probe {
        const NAME: &'static str = "probe";
        const VERSION: u32 = 3;
        const INFO: &'static Info = &Info {
            name: "probe",
            version: 3,
            dispatch: no_dispatch,
        };
        fn id(self) -> ObjectId {
            self.0
        }
        fn from_id(id: ObjectId) -> Self {
            Probe(id)
        }
    }

    fn wl() -> Wayland {
        let (a, _b) = UnixStream::pair().unwrap();
        Wayland::over(a)
    }

    #[test]
    fn ids_start_at_two_and_the_free_list_is_reused() {
        let mut wl = wl();
        assert_eq!(wl.display().0, DISPLAY);
        let a = wl.alloc::<Probe>(2);
        let b = wl.alloc::<Probe>(1);
        assert_eq!((a.0, b.0), (ObjectId(2), ObjectId(3)));
        assert_eq!(wl.version(a), 2);
        assert_eq!(wl.info(a.0).map(|(i, v)| (i.name, v)), Some(("probe", 2)));
        wl.free(a.0);
        assert!(wl.info(a.0).is_none());
        assert_eq!(
            wl.alloc::<Probe>(1).0,
            ObjectId(2),
            "freed ids come back first"
        );
        assert_eq!(wl.alloc::<Probe>(1).0, ObjectId(4));
    }

    #[test]
    fn server_ids_live_in_their_own_table() {
        let mut wl = wl();
        let id = ObjectId(SERVER_ID_BASE + 1);
        wl.register(id, Probe::INFO, 1);
        assert_eq!(wl.info(id).map(|(i, _)| i.name), Some("probe"));
        assert_eq!(wl.alloc::<Probe>(1).0, ObjectId(2), "unaffected");
    }

    #[test]
    fn a_request_is_buffered_until_flushed() {
        let mut wl = wl();
        assert!(!wl.has_pending());
        wl.request(DISPLAY, 1, |w| w.uint(2));
        assert!(wl.has_pending());
        assert_eq!(wl.out.len(), 12);
        assert_eq!(wire::header(&wl.out).unwrap().opcode, 1);
    }
}
