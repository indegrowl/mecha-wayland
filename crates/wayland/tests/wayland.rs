use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;

use app::prelude::*;
use ring::prelude::*;
use wayland::fake::Fake;
use wayland::prelude::*;

const GLOBALS: &[(&str, u32)] = &[("wl_compositor", 6), ("wl_shm", 2), ("wl_seat", 9)];

#[derive(Default)]
struct Log(Vec<String>);
impl Resource for Log {}

fn log_surface(app: &mut App, e: &WlSurfaceEvent) {
    app.resource_mut::<Log>().0.push(format!("{e:?}"));
}
fn log_keyboard(app: &mut App, e: &WlKeyboardEvent) {
    app.resource_mut::<Log>().0.push(format!("{e:?}"));
}
fn log_registry(app: &mut App, e: &WlRegistryEvent) {
    app.resource_mut::<Log>().0.push(format!("{e:?}"));
}

fn fake() -> Fake {
    let mut f = Fake::new(GLOBALS, |m| m.bind::<WlCompositor>().bind::<WlShm>());
    f.app
        .init_resource::<Log>()
        .system(log_surface)
        .system(log_keyboard)
        .system(log_registry);
    f.requests();
    f
}

fn log(f: &Fake) -> Vec<String> {
    f.app.resource::<Log>().0.clone()
}

// ── I/O ──────────────────────────────────────────────────────────────────

#[test]
fn requests_go_out_in_one_send_at_before_wait() {
    let mut f = fake();
    let compositor = *f.app.resource::<WlCompositor>();
    let a = compositor.create_surface(&mut f.app.resource_mut::<Wayland>());
    let b = compositor.create_surface(&mut f.app.resource_mut::<Wayland>());
    assert!(f.requests().is_empty(), "nothing before a turn");
    Ring::turn(&mut f.app);
    f.turn();
    let reqs = f.requests();
    assert_eq!(reqs.len(), 2);
    assert_eq!(reqs[0].reader().object(), Some(a.id()));
    assert_eq!(reqs[1].reader().object(), Some(b.id()));
    f.turn();
    assert!(
        f.requests().is_empty(),
        "an empty buffer sends nothing and a turn is not made"
    );
}

#[test]
fn events_decode_in_order_and_across_a_split_read() {
    let mut f = fake();
    let compositor = *f.app.resource::<WlCompositor>();
    let surface = compositor.create_surface(&mut f.app.resource_mut::<Wayland>());
    f.turn();
    let s = surface.id().0;
    f.send(s, 0, |w| w.object(ObjectId(77)));
    f.send(s, 2, |w| w.int(2));
    f.turn();
    let l = log(&f);
    assert_eq!(l.len(), 2);
    assert!(l[0].starts_with("Enter"), "{l:?}");
    assert!(l[1].starts_with("PreferredBufferScale"), "{l:?}");

    // A message split across two reads: the first half alone does nothing.
    let mut whole = Vec::new();
    let mut fds = Vec::new();
    wayland::wire::Writer::begin(&mut whole, &mut fds, ObjectId(s), 2).int(3);
    let (head, tail) = whole.split_at(6);
    let peer_fd = f.peer().try_clone_to_owned().unwrap();
    let mut peer = UnixStream::from(peer_fd);
    std::io::Write::write_all(&mut peer, head).unwrap();
    Ring::turn(&mut f.app);
    assert_eq!(log(&f).len(), 2, "half a message is carried, not signalled");
    std::io::Write::write_all(&mut peer, tail).unwrap();
    Ring::turn(&mut f.app);
    assert_eq!(log(&f).len(), 3);
    assert!(log(&f)[2].contains("factor: 3"));
}

#[test]
fn an_event_with_an_fd_pops_the_received_fd() {
    let mut f = fake();
    let seat_global = f
        .app
        .resource::<Globals>()
        .find("wl_seat")
        .cloned()
        .unwrap();
    let seat: WlSeat = {
        let (globals, mut wl) = f.app.query::<(Res<Globals>, ResMut<Wayland>)>();
        globals.bind::<WlSeat>(&seat_global, &mut wl)
    };
    let keyboard = seat.get_keyboard(&mut f.app.resource_mut::<Wayland>());
    f.turn();
    let (_r, w) = std::io::pipe().unwrap();
    let mut bytes = Vec::new();
    let mut none = Vec::new();
    {
        let mut wr = wayland::wire::Writer::begin(&mut bytes, &mut none, keyboard.id(), 0);
        wr.uint(1);
        wr.uint(64);
    }
    let peer = f.peer().try_clone_to_owned().unwrap();
    let token = f
        .app
        .resource_mut::<Ring>()
        .sendmsg(peer.as_fd(), bytes, vec![w.into()]);
    Ring::turn(&mut f.app);
    f.app.resource_mut::<Ring>().finish(token);
    let l = log(&f);
    assert_eq!(l.len(), 1);
    assert!(l[0].starts_with("Keymap"), "{l:?}");
    assert!(l[0].contains("size: 64"));
}

#[test]
fn a_request_with_an_fd_sends_it_as_ancillary_data() {
    let mut f = fake();
    let shm = *f.app.resource::<WlShm>();
    let (_r, w) = std::io::pipe().unwrap();
    shm.create_pool(&mut f.app.resource_mut::<Wayland>(), w.as_fd(), 4096);
    let peer = f.peer().try_clone_to_owned().unwrap();
    let token = f
        .app
        .resource_mut::<Ring>()
        .recvmsg(peer.as_fd(), Vec::with_capacity(256), 4);
    Ring::turn(&mut f.app);
    let done = f.app.resource_mut::<Ring>().finish(token).unwrap();
    assert_eq!(done.fds.len(), 1);
    let h = wayland::wire::header(&done.buf).unwrap();
    assert_eq!((h.sender, h.opcode), (shm.id(), 0));
}

#[test]
fn delete_id_frees_the_id_for_reuse() {
    let mut f = fake();
    let compositor = *f.app.resource::<WlCompositor>();
    let a = compositor.create_surface(&mut f.app.resource_mut::<Wayland>());
    f.turn();
    a.destroy(&mut f.app.resource_mut::<Wayland>());
    f.send(1, 1, |w| w.uint(a.id().0));
    f.turn();
    assert!(f.app.resource::<Wayland>().info(a.id()).is_none());
    let b = compositor.create_surface(&mut f.app.resource_mut::<Wayland>());
    assert_eq!(b.id(), a.id());
}

#[test]
#[should_panic(expected = "wl_display error")]
fn a_protocol_error_panics() {
    let mut f = fake();
    f.send(1, 0, |w| {
        w.object(ObjectId(4));
        w.uint(0);
        w.string("bad");
    });
    f.turn();
}

#[test]
#[should_panic(expected = "went away")]
fn a_closed_socket_panics() {
    let f = fake();
    let mut app = f.hang_up();
    Ring::turn(&mut app);
}

// ── globals ──────────────────────────────────────────────────────────────

#[test]
fn install_binds_the_named_globals_as_resources() {
    let mut f = Fake::new(GLOBALS, |m| m.bind::<WlCompositor>().bind::<WlShm>());
    let compositor = *f.app.resource::<WlCompositor>();
    let shm = *f.app.resource::<WlShm>();
    assert_eq!((compositor.id(), shm.id()), (ObjectId(4), ObjectId(5)));
    let wl = f.app.resource::<Wayland>();
    assert_eq!(
        wl.version(compositor),
        6,
        "the lesser of advertised 6 and XML 7"
    );
    assert_eq!(wl.version(shm), 2);
    let globals = f.app.resource::<Globals>();
    assert_eq!(globals.iter().count(), 3);
    assert_eq!(globals.registry().id(), ObjectId(2));
    let seat = globals.find("wl_seat").unwrap();
    assert_eq!((seat.name, seat.version), (3, 9));

    let bind = f.expect(2, 0);
    let mut r = bind.reader();
    assert_eq!(r.uint(), Some(1));
    assert_eq!(r.string().as_deref(), Some("wl_compositor"));
    assert_eq!(r.uint(), Some(6));
    assert_eq!(r.object(), Some(ObjectId(4)));
}

#[test]
fn a_global_above_the_xml_version_binds_at_the_xml_version() {
    let f = Fake::new(&[("wl_shm", 9)], |m| m.bind::<WlShm>());
    let shm = *f.app.resource::<WlShm>();
    assert_eq!(f.app.resource::<Wayland>().version(shm), WlShm::VERSION);
}

#[test]
#[should_panic(expected = "does not advertise xdg_wm_base")]
fn a_missing_global_panics_at_install() {
    Fake::new(GLOBALS, |m| m.bind::<XdgWmBase>());
}

#[test]
fn the_registry_keeps_being_followed_after_install() {
    let mut f = fake();
    f.send(2, 0, |w| {
        w.uint(9);
        w.string("wl_output");
        w.uint(4);
    });
    f.turn();
    assert!(f.app.resource::<Globals>().find("wl_output").is_some());
    assert!(log(&f)[0].starts_with("Global {"));
    f.send(2, 1, |w| w.uint(9));
    f.turn();
    assert!(f.app.resource::<Globals>().find("wl_output").is_none());
    assert_eq!(f.app.resource::<Globals>().iter().count(), 3);
}

// ── final review ─────────────────────────────────────────────────────────

/// A `wl_keyboard`, bound through the seat the fixture advertises.
fn keyboard(f: &mut Fake) -> WlKeyboard {
    let global = f
        .app
        .resource::<Globals>()
        .find("wl_seat")
        .cloned()
        .unwrap();
    let seat: WlSeat = {
        let (globals, mut wl) = f.app.query::<(Res<Globals>, ResMut<Wayland>)>();
        globals.bind::<WlSeat>(&global, &mut wl)
    };
    let keyboard = seat.get_keyboard(&mut f.app.resource_mut::<Wayland>());
    f.turn();
    keyboard
}

/// Logs the first byte readable on a `keymap` event's fd, so a test can
/// say which of the fds it sent the event carries.
fn log_keymap_fd(app: &mut App, e: &WlKeyboardEvent) {
    let WlKeyboardEvent::Keymap { fd, .. } = e else {
        return;
    };
    let mut byte = [0u8; 1];
    let mut file = std::fs::File::from(fd.try_clone().expect("dup the keymap fd"));
    std::io::Read::read_exact(&mut file, &mut byte).expect("read the keymap fd");
    app.resource_mut::<Log>()
        .0
        .push(format!("keymap fd {}", byte[0] as char));
}

/// A pipe whose read end is ready to hand over, with `mark` in it. The
/// write end is returned so it stays open: an fd the app drops without
/// reading would otherwise be indistinguishable from one it read.
fn marked_pipe(mark: u8) -> (std::io::PipeReader, std::io::PipeWriter) {
    let (r, mut w) = std::io::pipe().expect("pipe");
    std::io::Write::write_all(&mut w, &[mark]).expect("mark the pipe");
    (r, w)
}

#[test]
fn an_event_whose_body_does_not_decode_still_consumes_its_own_fd() {
    let mut f = fake();
    f.app.system(log_keymap_fd);
    let keyboard = keyboard(&mut f);
    let (first, _a) = marked_pipe(b'1');
    let (second, _b) = marked_pipe(b'2');

    let mut bytes = Vec::new();
    let mut none = Vec::new();
    {
        // Not a `wl_keyboard.keymap_format`: `decode` bails on the enum,
        // before it would have reached the fd.
        let mut w = wayland::wire::Writer::begin(&mut bytes, &mut none, keyboard.id(), 0);
        w.uint(99);
        w.uint(64);
    }
    {
        let mut w = wayland::wire::Writer::begin(&mut bytes, &mut none, keyboard.id(), 0);
        w.uint(1);
        w.uint(65);
    }
    let peer = f.peer().try_clone_to_owned().unwrap();
    let token = f.app.resource_mut::<Ring>().sendmsg(
        peer.as_fd(),
        bytes,
        vec![first.into(), second.into()],
    );
    Ring::turn(&mut f.app);
    f.app.resource_mut::<Ring>().finish(token);

    let l = log(&f);
    let fds: Vec<&String> = l.iter().filter(|s| s.starts_with("keymap fd ")).collect();
    assert_eq!(
        fds,
        vec!["keymap fd 2"],
        "the skipped event took its own fd with it: {l:?}"
    );
    assert_eq!(
        l.iter().filter(|s| s.starts_with("Keymap")).count(),
        1,
        "only the second event decoded: {l:?}"
    );
}

#[test]
fn a_flush_sends_no_more_than_twenty_eight_fds_and_keeps_the_rest() {
    let mut f = fake();
    let shm = *f.app.resource::<WlShm>();
    let (_r, w) = std::io::pipe().unwrap();
    {
        let mut wl = f.app.resource_mut::<Wayland>();
        for size in 1..=30 {
            shm.create_pool(&mut wl, w.as_fd(), size);
        }
    }

    let peer = f.peer().try_clone_to_owned().unwrap();
    let token = f
        .app
        .resource_mut::<Ring>()
        .recvmsg(peer.as_fd(), Vec::with_capacity(4096), 64);
    Ring::turn(&mut f.app);
    let first = f.app.resource_mut::<Ring>().finish(token).unwrap();
    assert_eq!(first.fds.len(), 28, "libwayland's MAX_FDS_OUT");
    assert!(
        f.app.resource::<Wayland>().has_pending(),
        "the two messages past the cut are still buffered"
    );

    let token = f
        .app
        .resource_mut::<Ring>()
        .recvmsg(peer.as_fd(), Vec::with_capacity(4096), 64);
    Ring::turn(&mut f.app);
    let second = f.app.resource_mut::<Ring>().finish(token).unwrap();
    assert_eq!(second.fds.len(), 2, "the rest, in the next send");

    let mut bytes = first.buf;
    bytes.extend_from_slice(&second.buf);
    assert_eq!(
        create_pool_sizes(&bytes, shm.id()),
        (1..=30).collect::<Vec<_>>(),
        "every request whole and in order"
    );
}

/// The `size` argument of each `wl_shm.create_pool` in `bytes`, which
/// must be nothing but whole `create_pool` requests from `shm`.
fn create_pool_sizes(bytes: &[u8], shm: ObjectId) -> Vec<i32> {
    let mut sizes = Vec::new();
    let mut o = 0;
    while let Some(h) = wayland::wire::header(&bytes[o..]) {
        assert!(o + h.size <= bytes.len(), "a truncated request");
        assert_eq!((h.sender, h.opcode), (shm, 0));
        let mut r = wayland::wire::Reader::new(&bytes[o + 8..o + h.size]);
        r.object().expect("the new pool id");
        sizes.push(r.int().expect("the pool size"));
        o += h.size;
    }
    assert_eq!(o, bytes.len(), "a trailing part of a request");
    sizes
}

#[test]
fn a_short_send_keeps_its_tail_and_finishes_it_on_a_later_turn() {
    let mut f = fake();
    let compositor = *f.app.resource::<WlCompositor>();
    let surface = compositor.create_surface(&mut f.app.resource_mut::<Wayland>());
    f.turn();

    // Far more than a unix socket's send buffer (~208 KiB by default), in
    // messages the wire's 16-bit size word can hold.
    const MESSAGES: u32 = 32;
    let payload = vec![0xabu8; 32000];
    let one = 8 + 4 + 4 + payload.len();
    {
        let mut wl = f.app.resource_mut::<Wayland>();
        for i in 0..MESSAGES {
            wl.request(surface.id(), 9, |w| {
                w.uint(i);
                w.array(&payload);
            });
        }
    }

    let peer_fd = f.peer().try_clone_to_owned().unwrap();
    let mut peer = UnixStream::from(peer_fd);
    let mut got: Vec<u8> = Vec::new();
    let drain = |peer: &mut UnixStream, got: &mut Vec<u8>| {
        let mut tmp = vec![0u8; 64 * 1024];
        loop {
            match std::io::Read::read(peer, &mut tmp) {
                Ok(0) => break,
                Ok(n) => got.extend_from_slice(&tmp[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => panic!("read from the app: {e}"),
            }
        }
    };

    Ring::turn(&mut f.app);
    assert!(
        f.app.resource::<Wayland>().has_pending(),
        "one send cannot have taken it all"
    );
    drain(&mut peer, &mut got);
    assert!(
        got.len() < MESSAGES as usize * one,
        "the send was short: {} of {} bytes",
        got.len(),
        MESSAGES as usize * one
    );

    // Draining lets the rest go out; the tail must go before anything the
    // next turn would otherwise send first.
    let mut turns = 0;
    while f.app.resource::<Wayland>().has_pending() {
        Ring::turn(&mut f.app);
        drain(&mut peer, &mut got);
        turns += 1;
        assert!(turns < 64, "the app never finishes sending");
    }
    assert_eq!(got.len(), MESSAGES as usize * one, "every byte, once");

    let mut o = 0;
    let mut seen = Vec::new();
    while let Some(h) = wayland::wire::header(&got[o..]) {
        assert!(o + h.size <= got.len(), "a truncated request");
        assert_eq!((h.sender, h.opcode), (surface.id(), 9));
        let mut r = wayland::wire::Reader::new(&got[o + 8..o + h.size]);
        seen.push(r.uint().expect("the index"));
        assert_eq!(r.array().as_deref(), Some(&payload[..]), "a whole body");
        o += h.size;
    }
    assert_eq!(
        seen,
        (0..MESSAGES).collect::<Vec<_>>(),
        "whole and in order"
    );
}
