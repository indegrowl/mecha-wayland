use std::io::{Read, Write};
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::sync::mpsc;
use std::time::Duration;

use ring::{Completed, Ring};

/// Wait until both tokens completed, at most ten waits.
fn wait_for(ring: &mut Ring, tokens: &[ring::Token]) {
    let mut seen = Vec::new();
    for _ in 0..10 {
        for e in ring.wait() {
            seen.push(e.token);
        }
        if tokens.iter().all(|t| seen.contains(t)) {
            return;
        }
    }
    panic!("ops did not complete: seen {seen:?}");
}

#[test]
fn sendmsg_and_recvmsg_move_bytes_and_fds() {
    let (a, b) = UnixStream::pair().unwrap();
    let (mut reader, writer) = std::io::pipe().unwrap();
    let mut ring = Ring::new(8);

    let sent = ring.sendmsg(a.as_fd(), b"hello".to_vec(), vec![OwnedFd::from(writer)]);
    let recv = ring.recvmsg(b.as_fd(), Vec::with_capacity(64), 4);
    wait_for(&mut ring, &[sent, recv]);

    // The send op owns its fds until finished; drop them first, or the
    // pipe never reaches EOF below.
    let Completed { buf, fds } = ring.finish(sent).unwrap();
    assert_eq!(buf, b"hello", "the send buffer comes back for reuse");
    assert_eq!(fds.len(), 1, "the sent fds come back to be dropped");
    drop(fds);

    let Completed { buf, fds } = ring.finish(recv).unwrap();
    assert_eq!(buf, b"hello");
    assert_eq!(fds.len(), 1);
    let mut got = std::io::PipeWriter::from(fds.into_iter().next().unwrap());
    got.write_all(b"via fd").unwrap();
    drop(got);
    let mut s = String::new();
    reader.read_to_string(&mut s).unwrap();
    assert_eq!(s, "via fd");
}

#[test]
fn recvmsg_with_no_fd_room_gets_the_bytes_only() {
    let (a, b) = UnixStream::pair().unwrap();
    let (_reader, writer) = std::io::pipe().unwrap();
    let mut ring = Ring::new(8);
    let sent = ring.sendmsg(a.as_fd(), b"x".to_vec(), vec![OwnedFd::from(writer)]);
    let recv = ring.recvmsg(b.as_fd(), Vec::with_capacity(8), 0);
    wait_for(&mut ring, &[sent, recv]);
    let Completed { buf, fds } = ring.finish(recv).unwrap();
    assert_eq!(buf, b"x");
    assert!(fds.is_empty());
}

#[test]
fn finish_is_once_and_only_for_own_tokens() {
    let (a, _b) = UnixStream::pair().unwrap();
    let mut ring = Ring::new(8);
    let sent = ring.sendmsg(a.as_fd(), b"x".to_vec(), vec![]);
    wait_for(&mut ring, &[sent]);
    assert!(ring.finish(sent).is_some());
    assert!(ring.finish(sent).is_none());
    assert!(ring.finish(ring::Token::from_raw(999)).is_none());
}

#[test]
fn dropping_a_ring_with_a_pending_recv_returns_instead_of_hanging() {
    // `Ring` holds raw pointers (it is not `Send`), so the ring itself is
    // built and dropped inside the watched thread; only the completion
    // signal crosses the thread boundary.
    let (done, waited) = mpsc::channel();
    std::thread::spawn(move || {
        let (_a, b) = UnixStream::pair().unwrap();
        let mut ring = Ring::new(8);
        // Armed but never completed: nothing is ever sent on `_a`.
        let _recv = ring.recvmsg(b.as_fd(), Vec::with_capacity(64), 0);
        drop(ring);
        let _ = done.send(());
    });
    waited
        .recv_timeout(Duration::from_secs(5))
        .expect("dropping the ring hung with a recv still in flight");
}

#[test]
fn dropping_a_ring_before_finishing_a_completed_recv_closes_both_copies_of_the_fd() {
    let (a, b) = UnixStream::pair().unwrap();
    let (mut reader, writer) = std::io::pipe().unwrap();
    let mut ring = Ring::new(8);

    let sent = ring.sendmsg(a.as_fd(), b"hello".to_vec(), vec![OwnedFd::from(writer)]);
    let recv = ring.recvmsg(b.as_fd(), Vec::with_capacity(64), 4);
    wait_for(&mut ring, &[sent, recv]);

    // Neither op is finished: the sent fd is still held by the send op,
    // and the received fd is still undecoded in the recv op's control
    // buffer. Dropping the ring must close both on its own.
    drop(ring);

    let mut s = String::new();
    reader.read_to_string(&mut s).unwrap();
    assert_eq!(s, "");
}
