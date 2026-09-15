#![deny(unsafe_code)]
//! The one place the runner waits: an io_uring as a resource, typed
//! operations that own their buffers until finished, one [`IoEvent`]
//! signal per completion. Crate docs are completed in Task 2.

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

/// Placeholder until Task 2 sets the runner.
pub struct RingModule {
    pub entries: u32,
}
