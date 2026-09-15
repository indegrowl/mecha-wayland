#![forbid(unsafe_code)]
//! The Wayland client as a resource. Crate docs are completed in Task 6.

use std::collections::{HashMap, VecDeque};
use std::mem;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use app::prelude::*;
use ring::prelude::*;

#[cfg(feature = "fake")]
pub mod fake;

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
    sending: Option<Token>,
    reading: Option<Token>,
    inbox: Vec<u8>,
    in_fds: VecDeque<OwnedFd>,
    pub(crate) synced: bool,
    pub(crate) sync: Option<WlCallback>,
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
            sending: None,
            reading: None,
            inbox: Vec::new(),
            in_fds: VecDeque::new(),
            synced: false,
            sync: None,
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
        !self.out.is_empty() || self.sending.is_some()
    }

    /// Arm the one receive. `buf` should have 64 KiB of capacity; it is
    /// the buffer the last read gave back.
    pub fn arm_read(&mut self, ring: &mut Ring, buf: Vec<u8>) {
        debug_assert!(self.reading.is_none(), "wayland: a read is already armed");
        self.reading = Some(ring.recvmsg(self.fd.as_fd(), buf, 28));
    }

    /// Send what is buffered, if nothing is in flight.
    pub fn flush(&mut self, ring: &mut Ring) {
        if self.out.is_empty() || self.sending.is_some() {
            return;
        }
        let buf = mem::take(&mut self.out);
        let fds = mem::take(&mut self.out_fds);
        self.sending = Some(ring.sendmsg(self.fd.as_fd(), buf, fds));
    }
}

pub(crate) fn on_before_wait(app: &mut App, _: &BeforeWait) {
    let (mut wl, mut ring) = app.query::<(ResMut<Wayland>, ResMut<Ring>)>();
    wl.flush(&mut ring);
}

enum Io {
    Read,
    Send,
}

pub(crate) fn on_io(app: &mut App, e: &IoEvent) {
    let io = {
        let wl = app.resource::<Wayland>();
        if wl.reading == Some(e.token) {
            Io::Read
        } else if wl.sending == Some(e.token) {
            Io::Send
        } else {
            return;
        }
    };
    let done = app
        .resource_mut::<Ring>()
        .finish(e.token)
        .expect("wayland: a completed op is finishable");
    match io {
        Io::Send => {
            assert!(e.result >= 0, "wayland: send failed, errno {}", -e.result);
            let sent = e.result as usize;
            let mut wl = app.resource_mut::<Wayland>();
            wl.sending = None;
            if sent < done.buf.len() {
                let mut tail = done.buf[sent..].to_vec();
                tail.append(&mut wl.out);
                wl.out = tail;
            }
        }
        Io::Read => {
            assert!(
                e.result > 0,
                "wayland: the compositor went away (read returned {})",
                e.result
            );
            let (mut bytes, mut fds) = {
                let (mut wl, mut ring) = app.query::<(ResMut<Wayland>, ResMut<Ring>)>();
                wl.reading = None;
                wl.inbox.extend_from_slice(&done.buf);
                wl.in_fds.extend(done.fds);
                wl.arm_read(&mut ring, done.buf);
                (mem::take(&mut wl.inbox), mem::take(&mut wl.in_fds))
            };
            let consumed = dispatch_all(app, &bytes, &mut fds);
            bytes.drain(..consumed);
            let mut wl = app.resource_mut::<Wayland>();
            wl.inbox = bytes;
            wl.in_fds = fds;
        }
    }
}

/// Signal every complete message in `bytes`, in order; return how many
/// bytes were consumed. A message for an object the table does not know
/// is skipped whole.
fn dispatch_all(app: &mut App, bytes: &[u8], fds: &mut VecDeque<OwnedFd>) -> usize {
    let mut o = 0;
    while let Some(h) = wire::header(&bytes[o..]) {
        assert!(
            h.size >= wire::HEADER,
            "wayland: a message shorter than its header"
        );
        if o + h.size > bytes.len() {
            break;
        }
        let body = &bytes[o + wire::HEADER..o + h.size];
        if let Some((info, _)) = app.resource::<Wayland>().info(h.sender) {
            (info.dispatch)(app, h.sender, h.opcode, body, fds);
        }
        o += h.size;
    }
    o
}

pub(crate) fn on_display(app: &mut App, e: &WlDisplayEvent) {
    match e {
        WlDisplayEvent::Error {
            object_id,
            code,
            message,
            ..
        } => panic!("wl_display error on {object_id:?} code {code}: {message}"),
        WlDisplayEvent::DeleteId { id, .. } => app.resource_mut::<Wayland>().free(ObjectId(*id)),
    }
}

pub(crate) fn on_callback(app: &mut App, e: &WlCallbackEvent) {
    let WlCallbackEvent::Done { callback, .. } = e;
    let mut wl = app.resource_mut::<Wayland>();
    if wl.sync == Some(*callback) {
        wl.sync = None;
        wl.synced = true;
    }
}

/// One advertised global.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Global {
    pub name: u32,
    pub interface: String,
    pub version: u32,
}

/// The registry and every global it currently advertises, exact after
/// every flush. Singletons named on [`WaylandModule::bind`] are bound at
/// install and inserted as resources; anything else is bound here.
pub struct Globals {
    registry: WlRegistry,
    list: Vec<Global>,
}
impl Resource for Globals {}

impl Globals {
    pub fn registry(&self) -> WlRegistry {
        self.registry
    }
    pub fn iter(&self) -> impl Iterator<Item = &Global> {
        self.list.iter()
    }
    /// The first advertised global of that interface.
    pub fn find(&self, interface: &str) -> Option<&Global> {
        self.list.iter().find(|g| g.interface == interface)
    }
    /// Bind `global` as `I`, at the lesser of its version and `I::VERSION`.
    pub fn bind<I: Interface>(&self, global: &Global, wl: &mut Wayland) -> I {
        debug_assert_eq!(global.interface, I::NAME);
        self.registry
            .bind::<I>(wl, global.name, global.version.min(I::VERSION))
    }
}

pub(crate) fn on_registry(app: &mut App, e: &WlRegistryEvent) {
    let mut globals = app.resource_mut::<Globals>();
    match e {
        WlRegistryEvent::Global {
            name,
            interface,
            version,
            ..
        } => globals.list.push(Global {
            name: *name,
            interface: interface.clone(),
            version: *version,
        }),
        WlRegistryEvent::GlobalRemove { name, .. } => globals.list.retain(|g| g.name != *name),
    }
}

type Bind = Box<dyn FnOnce(&mut App)>;

/// Connects, binds the named singletons after a blocking sync, and keeps
/// the registry followed. Installs after `RingModule`.
pub struct WaylandModule {
    stream: Option<UnixStream>,
    binds: Vec<Bind>,
}

impl Default for WaylandModule {
    fn default() -> Self {
        Self::new()
    }
}

impl WaylandModule {
    /// Connect with [`Wayland::connect`] at install.
    pub fn new() -> Self {
        WaylandModule {
            stream: None,
            binds: Vec::new(),
        }
    }

    /// Or over this stream: a test's socketpair.
    pub fn over(stream: UnixStream) -> Self {
        WaylandModule {
            stream: Some(stream),
            binds: Vec::new(),
        }
    }

    /// A singleton global to bind at install and keep as the resource
    /// `I`. Bound in the order named, at the lesser of the advertised
    /// and the XML version.
    ///
    /// # Panics
    ///
    /// At install, if the compositor does not advertise `I::NAME`.
    pub fn bind<I: Interface + Resource>(mut self) -> Self {
        self.binds.push(Box::new(|app: &mut App| {
            let global = app
                .resource::<Globals>()
                .find(I::NAME)
                .cloned()
                .unwrap_or_else(|| {
                    panic!("wayland: the compositor does not advertise {}", I::NAME)
                });
            let object: I = {
                let (globals, mut wl) = app.query::<(Res<Globals>, ResMut<Wayland>)>();
                globals.bind::<I>(&global, &mut wl)
            };
            app.insert_resource(object);
        }));
        self
    }
}

impl Module for WaylandModule {
    fn install(self, app: &mut App) {
        let mut wl = match self.stream {
            Some(stream) => Wayland::over(stream),
            None => Wayland::connect(),
        };
        app.system(on_before_wait)
            .system(on_io)
            .system(on_display)
            .system(on_callback)
            .system(on_registry);
        let display = wl.display();
        let registry = display.get_registry(&mut wl);
        wl.sync = Some(display.sync(&mut wl));
        wl.arm_read(
            &mut app.resource_mut::<Ring>(),
            Vec::with_capacity(64 * 1024),
        );
        app.insert_resource(wl);
        app.insert_resource(Globals {
            registry,
            list: Vec::new(),
        });
        while !app.resource::<Wayland>().synced {
            Ring::turn(app);
        }
        for bind in self.binds {
            bind(app);
        }
        // The binds above only buffer requests; send them now rather than
        // leaving them for the next `BeforeWait`, so a bound global is
        // usable (and its bind visible on the wire) the moment `install`
        // returns.
        if app.resource::<Wayland>().has_pending() {
            Ring::turn(app);
        }
    }
}

pub mod prelude {
    pub use crate::display::{
        WlCallback, WlCallbackEvent, WlDisplay, WlDisplayEvent, WlRegistry, WlRegistryEvent,
    };
    pub use crate::generated::*;
    pub use crate::{Global, Globals, Interface, ObjectId, Wayland, WaylandModule};
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
