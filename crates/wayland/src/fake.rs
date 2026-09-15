//! A scripted compositor on the other end of a socketpair, for tests
//! that need no real one. The fake writes events into its end and reads
//! the requests that came out, single-threaded, one `Ring::turn` at a
//! time. Client ids are deterministic: the registry is 2, the install
//! sync's callback 3, and the binds count on from 4 in the order the
//! module named them. The fake never sends `delete_id` for the sync's
//! callback, so 3 stays taken. The requests install made (`get_registry`,
//! `sync`, the binds) are left in the list for a test to inspect or drain.

use std::io::{ErrorKind, Read, Write};
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::net::UnixStream;

use app::prelude::*;
use ring::prelude::*;

use crate::wire::{self, Reader, Writer};
use crate::{ObjectId, Wayland, WaylandModule};

/// One request the app sent, decoded to its header; the body is read
/// with [`Request::reader`].
#[derive(Debug)]
pub struct Request {
    pub sender: ObjectId,
    pub opcode: u16,
    pub body: Vec<u8>,
}

impl Request {
    pub fn reader(&self) -> Reader<'_> {
        Reader::new(&self.body)
    }
}

pub struct Fake {
    pub app: App,
    peer: UnixStream,
    inbox: Vec<u8>,
    outbox: Vec<u8>,
    wrote: bool,
    requests: Vec<Request>,
}

impl Fake {
    /// An app with `RingModule` and the configured `WaylandModule`
    /// installed over a socketpair, after the fake advertised `globals`
    /// (names 1, 2, … in order) and answered the install sync.
    pub fn new(
        globals: &[(&str, u32)],
        configure: impl FnOnce(WaylandModule) -> WaylandModule,
    ) -> Fake {
        let (ours, theirs) = UnixStream::pair().expect("socketpair");
        theirs.set_nonblocking(true).expect("nonblocking peer");
        let mut app = App::new();
        app.add_module(RingModule::default());
        let mut fake = Fake {
            app,
            peer: theirs,
            inbox: Vec::new(),
            outbox: Vec::new(),
            wrote: false,
            requests: Vec::new(),
        };
        for (i, (interface, version)) in globals.iter().enumerate() {
            fake.send(2, 0, |w| {
                w.uint(i as u32 + 1);
                w.string(interface);
                w.uint(*version);
            });
        }
        fake.send(3, 0, |w| w.uint(0));
        fake.flush_outbox();
        fake.wrote = false;
        fake.app.add_module(configure(WaylandModule::over(ours)));
        fake.read_peer();
        fake
    }

    /// Queue one event to write into the socket; every event queued since
    /// the last [`Fake::turn`] goes out in one write, so the app's single
    /// armed read sees them together rather than racing a read completion
    /// against a later write in this batch.
    pub fn send(&mut self, sender: u32, opcode: u16, args: impl FnOnce(&mut Writer<'_>)) {
        let mut fds = Vec::new();
        args(&mut Writer::begin(
            &mut self.outbox,
            &mut fds,
            ObjectId(sender),
            opcode,
        ));
        assert!(
            fds.is_empty(),
            "send an fd through Ring::sendmsg on `peer()`"
        );
        self.wrote = true;
    }

    /// Turn while the app has something to send or the fake wrote
    /// something for it to read, then read what the app sent. A turn with
    /// neither would block forever, so none is made.
    pub fn turn(&mut self) {
        self.flush_outbox();
        let mut turns = 0;
        while self.wrote || self.app.resource::<Wayland>().has_pending() {
            self.wrote = false;
            Ring::turn(&mut self.app);
            turns += 1;
            assert!(turns < 32, "the fake and the app keep talking");
        }
        self.read_peer();
    }

    /// Write every event queued by [`Fake::send`] since the last flush, in
    /// one write.
    fn flush_outbox(&mut self) {
        if self.outbox.is_empty() {
            return;
        }
        let buf = std::mem::take(&mut self.outbox);
        self.peer.write_all(&buf).expect("write to the app");
    }

    /// Every request received so far, in order, drained.
    pub fn requests(&mut self) -> Vec<Request> {
        std::mem::take(&mut self.requests)
    }

    /// The first request from `sender` with `opcode`, removed from the
    /// list. Panics listing what was received if there is none.
    pub fn expect(&mut self, sender: u32, opcode: u16) -> Request {
        let at = self
            .requests
            .iter()
            .position(|r| r.sender == ObjectId(sender) && r.opcode == opcode)
            .unwrap_or_else(|| panic!("no request {sender}.{opcode} among {:?}", self.requests));
        self.requests.remove(at)
    }

    /// The fake's end, for a test that moves fds with the ring.
    pub fn peer(&self) -> BorrowedFd<'_> {
        self.peer.as_fd()
    }

    /// Close the fake's end: the app's next read returns zero.
    pub fn hang_up(self) -> App {
        drop(self.peer);
        self.app
    }

    fn read_peer(&mut self) {
        let mut tmp = [0u8; 4096];
        loop {
            match self.peer.read(&mut tmp) {
                Ok(0) => break,
                Ok(n) => self.inbox.extend_from_slice(&tmp[..n]),
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(e) => panic!("read from the app: {e}"),
            }
        }
        let mut o = 0;
        while let Some(h) = wire::header(&self.inbox[o..]) {
            if o + h.size > self.inbox.len() {
                break;
            }
            self.requests.push(Request {
                sender: h.sender,
                opcode: h.opcode,
                body: self.inbox[o + wire::HEADER..o + h.size].to_vec(),
            });
            o += h.size;
        }
        self.inbox.drain(..o);
    }
}
