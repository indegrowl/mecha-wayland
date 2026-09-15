#![deny(unsafe_code)]
//! The one place the runner waits.
//!
//! # Model
//!
//! - [`Ring`] is a resource over one io_uring. A module submits a typed
//!   op, [`Ring::recvmsg`] or [`Ring::sendmsg`], and gets a [`Token`].
//!   The ring owns the op's buffers until [`Ring::finish`] gives them
//!   back as a [`Completed`]; the submitter keeps the fd open until then.
//! - Every completion is one [`IoEvent`] signal. The module that owns
//!   the token finishes it; every other module ignores it.
//! - [`Ring::turn`] is one pass: [`BeforeWait`] (the last chance to
//!   submit), block for at least one completion, one `IoEvent` each,
//!   flush. The runner [`RingModule`] sets is tick, turn, repeat, until
//!   a [`Stop`] signal.
//! - A turn with no op in flight blocks forever: the Wayland read is
//!   always armed, so an app with `WaylandModule` never waits on
//!   nothing.
//!
//! This is the one crate in the workspace that allows unsafe code, in
//! `op.rs` alone: the `msghdr`s the kernel reads and the submission push.
//!
//! # Quick start
//!
//! ```
//! use std::os::fd::AsFd;
//! use std::os::unix::net::UnixStream;
//! use app::prelude::*;
//! use ring::prelude::*;
//!
//! #[derive(Default)]
//! struct Seen(Vec<IoEvent>);
//! impl Resource for Seen {}
//! fn note(app: &mut App, e: &IoEvent) { app.resource_mut::<Seen>().0.push(*e); }
//!
//! let (a, _b) = UnixStream::pair().unwrap();
//! let mut app = App::new();
//! app.add_module(RingModule::default()).init_resource::<Seen>().system(note);
//! let token = app.resource_mut::<Ring>().sendmsg(a.as_fd(), b"hi".to_vec(), vec![]);
//! Ring::turn(&mut app);
//! assert_eq!(app.resource::<Seen>().0[0].token, token);
//! let done = app.resource_mut::<Ring>().finish(token).unwrap();
//! assert_eq!(done.buf, b"hi");
//! ```

use std::collections::HashMap;
use std::io;
use std::os::fd::{AsRawFd, BorrowedFd, OwnedFd};

use app::prelude::*;
use io_uring::IoUring;

mod op;

use op::Op;

pub mod prelude {
    pub use crate::{BeforeWait, Completed, IoEvent, Ring, RingModule, Stop, Token};
}

/// The name of one submitted op. Never reused within a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Token(u64);

impl Token {
    /// A token by number, for a test that wants one the ring never issued.
    pub fn from_raw(raw: u64) -> Self {
        Token(raw)
    }
}

/// One completion. `result` is the raw io_uring result: bytes moved, or
/// a negative errno.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IoEvent {
    pub token: Token,
    pub result: i32,
}
impl Signal for IoEvent {}

/// The last chance to submit before the runner blocks.
#[derive(Debug, Clone, Copy)]
pub struct BeforeWait;
impl Signal for BeforeWait {}

/// End the loop after this turn.
#[derive(Debug, Clone, Copy)]
pub struct Stop;
impl Signal for Stop {}

/// What an op gives back when it is finished.
#[derive(Debug)]
pub struct Completed {
    pub buf: Vec<u8>,
    pub fds: Vec<OwnedFd>,
}

/// The io_uring and every op in flight. Ops hold their buffers until
/// [`Ring::finish`]; the submitter keeps the fd open until then.
///
/// Dropping the ring cancels every op still in flight and drains its
/// completions before the ring, and the boxed ops the kernel may still be
/// writing into, are freed.
pub struct Ring {
    uring: IoUring,
    ops: HashMap<u64, Box<Op>>,
    next: u64,
    done: Vec<IoEvent>,
    stopped: bool,
}
impl Resource for Ring {}

impl Ring {
    /// An io_uring with `entries` submission slots.
    ///
    /// # Panics
    ///
    /// If the kernel refuses the ring.
    pub fn new(entries: u32) -> Self {
        Ring {
            uring: IoUring::new(entries).expect("io_uring setup"),
            ops: HashMap::new(),
            next: 1,
            done: Vec::new(),
            stopped: false,
        }
    }

    /// Receive into `buf`'s whole capacity (give it `Vec::with_capacity`),
    /// with room for `max_fds` passed fds. On finish, `buf` holds the
    /// bytes received and `fds` the fds that came with them.
    pub fn recvmsg(&mut self, fd: BorrowedFd<'_>, buf: Vec<u8>, max_fds: usize) -> Token {
        self.submit(fd, Op::recv(buf, max_fds))
    }

    /// Send `buf` with `fds` attached. On finish, both come back so the
    /// caller can reuse the buffer and drop the fds.
    pub fn sendmsg(&mut self, fd: BorrowedFd<'_>, buf: Vec<u8>, fds: Vec<OwnedFd>) -> Token {
        self.submit(fd, Op::send(buf, fds))
    }

    fn submit(&mut self, fd: BorrowedFd<'_>, mut op: Box<Op>) -> Token {
        let token = self.next;
        self.next += 1;
        let entry = op.entry(fd.as_raw_fd()).user_data(token);
        self.ops.insert(token, op);
        op::push(&mut self.uring, entry);
        Token(token)
    }

    /// Take the op's buffers after its `IoEvent`. `None` if the token is
    /// not an op of this ring or was finished already.
    pub fn finish(&mut self, token: Token) -> Option<Completed> {
        let op = self.ops.get(&token.0)?;
        op.result?;
        Some(self.ops.remove(&token.0).unwrap().complete())
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped
    }

    /// Submit and block for at least one completion; return every
    /// completion in completion order. `EINTR` is retried.
    pub fn wait(&mut self) -> Vec<IoEvent> {
        loop {
            match self.uring.submit_and_wait(1) {
                Ok(_) => break,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => panic!("io_uring submit_and_wait: {e}"),
            }
        }
        let mut out = std::mem::take(&mut self.done);
        for cqe in self.uring.completion() {
            let token = cqe.user_data();
            let result = cqe.result();
            if let Some(op) = self.ops.get_mut(&token) {
                op.result = Some(result);
            }
            out.push(IoEvent {
                token: Token(token),
                result,
            });
        }
        out
    }
}

impl Drop for Ring {
    /// Cancels every op still in flight and drains its completion before
    /// the boxed ops (the receive buffers, iovecs and msghdrs the kernel
    /// may still be writing into) are freed. Never panics: `submit_and_wait`
    /// failing for any reason other than `EINTR` simply ends the drain,
    /// at the cost of the invariant it was enforcing.
    fn drop(&mut self) {
        if self.ops.is_empty() {
            return;
        }
        let pending: Vec<u64> = self
            .ops
            .iter()
            .filter(|(_, op)| op.result.is_none())
            .map(|(&token, _)| token)
            .collect();
        for token in pending {
            op::push(&mut self.uring, op::cancel_entry(token));
        }
        while self.ops.values().any(|op| op.result.is_none()) {
            match self.uring.submit_and_wait(1) {
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => return,
            }
            for cqe in self.uring.completion() {
                let token = cqe.user_data();
                let result = cqe.result();
                if let Some(op) = self.ops.get_mut(&token) {
                    op.result = Some(result);
                }
            }
        }
    }
}

impl Ring {
    /// One turn: signal [`BeforeWait`] and flush; submit and block for at
    /// least one completion; signal one [`IoEvent`] per completion, in
    /// completion order; flush. Blocks forever if no op is in flight.
    pub fn turn(app: &mut App) {
        app.signal(BeforeWait);
        app.flush();
        let mut events = app.resource_mut::<Ring>().wait();
        for e in events.drain(..) {
            app.signal(e);
        }
        app.flush();
        app.resource_mut::<Ring>().done = events;
    }
}

fn on_stop(app: &mut App, _: &Stop) {
    app.resource_mut::<Ring>().stopped = true;
}

/// Tick, turn, and stop when told. The runner every presentation module
/// relies on: a tick per wake, nothing between wakes.
fn run(mut app: App) {
    loop {
        app.tick();
        Ring::turn(&mut app);
        if app.resource::<Ring>().is_stopped() {
            return;
        }
    }
}

/// Inserts [`Ring`] and sets the runner. Installs first among the
/// platform modules; a second runner-setting module panics.
pub struct RingModule {
    /// Submission slots. 64 by default.
    pub entries: u32,
}

impl Default for RingModule {
    fn default() -> Self {
        RingModule { entries: 64 }
    }
}

impl Module for RingModule {
    fn install(self, app: &mut App) {
        app.insert_resource(Ring::new(self.entries));
        app.system(on_stop).set_runner(run);
    }
}
