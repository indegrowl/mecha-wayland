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
