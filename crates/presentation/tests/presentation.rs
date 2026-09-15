// The fixture below (ids, opcodes, event opcodes, `configured`, `attaches`,
// `log`, the logging systems) serves every task in this slice. Tasks 7 and
// 8's tests use most of it now; a few items (the destroy opcodes, `CLOSE`,
// `LAYER_CLOSED`, `PREFERRED_SCALE`, `SET_BUFFER_SCALE`) wait on later
// tasks' tests, so the module stays unused-allowed until then.
#![allow(dead_code)]

use app::prelude::*;
use geometry::Color;
use layout::prelude::*;
use presentation::prelude::*;
use wayland::fake::Fake;
use wayland::prelude::*;
use window::prelude::*;

const GLOBALS: &[(&str, u32)] = &[
    ("wl_compositor", 6),
    ("wl_shm", 2),
    ("xdg_wm_base", 7),
    ("zwlr_layer_shell_v1", 5),
];

// Deterministic ids: registry 2, sync callback 3, compositor 4, shm 5,
// wm_base 6, layer shell 7 (bound by presentation's install). The first
// window: surface 8, xdg surface 9, toplevel 10 (or layer surface 9).
// Its buffers after configure: pool 11, buffers 12 and 13, callback 14.
const SURFACE: u32 = 8;
const XDG: u32 = 9;
const TOPLEVEL: u32 = 10;
const POOL: u32 = 11;
const BUF_A: u32 = 12;
const BUF_B: u32 = 13;
const CALLBACK: u32 = 14;

// Opcodes, from the XML order.
mod op {
    pub const CREATE_SURFACE: u16 = 0;
    pub const CREATE_POOL: u16 = 0;
    pub const CREATE_BUFFER: u16 = 0;
    pub const POOL_DESTROY: u16 = 1;
    pub const BUFFER_DESTROY: u16 = 0;
    pub const SURFACE_DESTROY: u16 = 0;
    pub const ATTACH: u16 = 1;
    pub const FRAME: u16 = 3;
    pub const COMMIT: u16 = 6;
    pub const SET_BUFFER_SCALE: u16 = 8;
    pub const DAMAGE_BUFFER: u16 = 9;
    pub const GET_XDG_SURFACE: u16 = 2;
    pub const PONG: u16 = 3;
    pub const XDG_DESTROY: u16 = 0;
    pub const GET_TOPLEVEL: u16 = 1;
    pub const ACK_CONFIGURE: u16 = 4;
    pub const TOPLEVEL_DESTROY: u16 = 0;
    pub const SET_TITLE: u16 = 2;
    pub const SET_APP_ID: u16 = 3;
    pub const GET_LAYER_SURFACE: u16 = 0;
    pub const LAYER_SET_SIZE: u16 = 0;
    pub const LAYER_SET_ANCHOR: u16 = 1;
    pub const LAYER_SET_ZONE: u16 = 2;
    pub const LAYER_SET_KEYBOARD: u16 = 4;
    pub const LAYER_ACK: u16 = 6;
    pub const LAYER_DESTROY: u16 = 7;
}
mod ev {
    pub const PING: u16 = 0;
    pub const XDG_CONFIGURE: u16 = 0;
    pub const TOPLEVEL_CONFIGURE: u16 = 0;
    pub const CLOSE: u16 = 1;
    pub const LAYER_CONFIGURE: u16 = 0;
    pub const LAYER_CLOSED: u16 = 1;
    pub const DONE: u16 = 0;
    pub const RELEASE: u16 = 0;
    pub const PREFERRED_SCALE: u16 = 2;
}

#[derive(Default)]
struct Log(Vec<String>);
impl Resource for Log {}
fn log_resized(app: &mut App, e: &Emitted<Resized>) {
    app.resource_mut::<Log>().0.push(format!(
        "Resized {}x{}",
        e.event.size.width, e.event.size.height
    ));
}
fn log_scale(app: &mut App, e: &Emitted<ScaleFactorChanged>) {
    app.resource_mut::<Log>()
        .0
        .push(format!("Scale {}", e.event.scale));
}
fn log_close(app: &mut App, e: &Emitted<CloseRequested>) {
    let target = e.targets.iter().next().copied();
    app.resource_mut::<Log>()
        .0
        .push(format!("Close {target:?}"));
}
fn log_frame(app: &mut App, f: &Frame) {
    app.resource_mut::<Log>().0.push(format!("Frame {:?}", f.0));
}

fn fake() -> Fake {
    let mut f = Fake::new(GLOBALS, |m| {
        m.bind::<WlCompositor>().bind::<WlShm>().bind::<XdgWmBase>()
    });
    f.app
        .add_module(LayoutModule)
        .add_module(WindowModule)
        .init_resource::<Log>()
        .system(log_resized)
        .system(log_scale)
        .system(log_close)
        .system(log_frame)
        .add_module(PresentationModule {
            app_id: "test".into(),
        });
    f.turn();
    f.requests();
    f
}

fn log(f: &Fake) -> Vec<String> {
    f.app.resource::<Log>().0.clone()
}

/// A toplevel of the given style, spawned, ticked and flushed to the fake.
fn spawn(f: &mut Fake, style: LayoutStyle) -> NodeId {
    let root = f.app.root();
    let win = f
        .app
        .spawn(root, window().title("hello").layout(style))
        .id();
    f.app.tick();
    f.turn();
    win
}

/// A toplevel configured to `width` by `height`, its first frame drawn.
fn configured(f: &mut Fake, width: i32, height: i32) -> NodeId {
    let win = spawn(f, LayoutStyle::default().column());
    f.requests();
    f.send(TOPLEVEL, ev::TOPLEVEL_CONFIGURE, |w| {
        w.int(width);
        w.int(height);
        w.array(&[]);
    });
    f.send(XDG, ev::XDG_CONFIGURE, |w| w.uint(1));
    f.turn();
    win
}

fn attaches(f: &mut Fake) -> Vec<u32> {
    f.requests()
        .iter()
        .filter(|r| r.sender == ObjectId(SURFACE) && r.opcode == op::ATTACH)
        .map(|r| r.reader().object().unwrap().0)
        .collect()
}

// ── Task 7 ───────────────────────────────────────────────────────────────

#[test]
fn a_toplevel_window_gets_a_surface_a_role_and_an_initial_commit() {
    let mut f = fake();
    spawn(&mut f, LayoutStyle::default().column());
    let reqs = f.requests();
    let shape: Vec<(u32, u16)> = reqs.iter().map(|r| (r.sender.0, r.opcode)).collect();
    assert_eq!(
        shape,
        vec![
            (4, op::CREATE_SURFACE),
            (6, op::GET_XDG_SURFACE),
            (XDG, op::GET_TOPLEVEL),
            (TOPLEVEL, op::SET_TITLE),
            (TOPLEVEL, op::SET_APP_ID),
            (SURFACE, op::COMMIT),
        ]
    );
    assert_eq!(reqs[0].reader().object(), Some(ObjectId(SURFACE)));
    let mut r = reqs[1].reader();
    assert_eq!(
        (r.object(), r.object()),
        (Some(ObjectId(XDG)), Some(ObjectId(SURFACE)))
    );
    assert_eq!(reqs[3].reader().string().as_deref(), Some("hello"));
    assert_eq!(reqs[4].reader().string().as_deref(), Some("test"));
    let win = *f
        .app
        .resource::<Windows>()
        .iter()
        .collect::<Vec<_>>()
        .first()
        .unwrap();
    let surfaces = f.app.resource::<Surfaces>();
    assert_eq!(surfaces.surface_of(win), Some(WlSurface(ObjectId(SURFACE))));
    assert!(!surfaces.is_configured(win));
}

#[test]
fn a_layer_window_gets_a_layer_surface_with_its_role() {
    let mut f = fake();
    let root = f.app.root();
    let role = Role::Layer(LayerRole {
        layer: Layer::Top,
        anchor: Anchor::TOP | Anchor::LEFT | Anchor::RIGHT,
        exclusive_zone: 32,
        namespace: "bar".into(),
        keyboard_interactivity: KeyboardInteractivity::None,
    });
    f.app.spawn_with(
        root,
        window().layout(LayoutStyle::default().column().size(auto(), px(32.0))),
        (role,),
    );
    f.app.tick();
    f.turn();
    let reqs = f.requests();
    let shape: Vec<(u32, u16)> = reqs.iter().map(|r| (r.sender.0, r.opcode)).collect();
    assert_eq!(
        shape,
        vec![
            (4, op::CREATE_SURFACE),
            (7, op::GET_LAYER_SURFACE),
            (XDG, op::LAYER_SET_ANCHOR),
            (XDG, op::LAYER_SET_ZONE),
            (XDG, op::LAYER_SET_KEYBOARD),
            (XDG, op::LAYER_SET_SIZE),
            (SURFACE, op::COMMIT),
        ]
    );
    let mut r = reqs[1].reader();
    assert_eq!(r.object(), Some(ObjectId(XDG)));
    assert_eq!(r.object(), Some(ObjectId(SURFACE)));
    assert_eq!(r.object_opt(), Some(None), "no output");
    assert_eq!(r.uint(), Some(2), "top");
    assert_eq!(r.string().as_deref(), Some("bar"));
    assert_eq!(reqs[2].reader().uint(), Some(1 | 4 | 8));
    assert_eq!(reqs[3].reader().int(), Some(32));
    let mut r = reqs[5].reader();
    assert_eq!((r.uint(), r.uint()), (Some(0), Some(32)));
}

#[test]
fn a_ping_is_answered_with_a_pong() {
    let mut f = fake();
    f.send(6, ev::PING, |w| w.uint(77));
    f.turn();
    let pong = f.expect(6, op::PONG);
    assert_eq!(pong.reader().uint(), Some(77));
}

// ── Task 8 ───────────────────────────────────────────────────────────────

// A layer window allocates only surface 8 and layer surface 9 (no
// toplevel), so its pool is 10 and its buffers are 11 and 12.
const LAYER_BUF_A: u32 = 11;

#[test]
fn a_configure_acks_resizes_and_draws_the_first_frame() {
    let mut f = fake();
    let root = f.app.root();
    let win = f
        .app
        .spawn(
            root,
            window()
                .clear(Color::rgb(1.0, 0.0, 0.0))
                .layout(LayoutStyle::default().column()),
        )
        .id();
    f.app.tick();
    f.turn();
    f.requests();
    f.send(TOPLEVEL, ev::TOPLEVEL_CONFIGURE, |w| {
        w.int(320);
        w.int(200);
        w.array(&[]);
    });
    f.send(XDG, ev::XDG_CONFIGURE, |w| w.uint(5));
    f.turn();

    assert_eq!(f.expect(XDG, op::ACK_CONFIGURE).reader().uint(), Some(5));
    let l = log(&f);
    assert!(l.contains(&"Resized 320x200".to_string()), "{l:?}");
    assert!(l.contains(&format!("Frame {win:?}")), "{l:?}");
    let pool = f.expect(5, op::CREATE_POOL);
    let mut r = pool.reader();
    assert_eq!(r.object(), Some(ObjectId(POOL)));
    assert_eq!(r.int(), Some(320 * 200 * 4 * 2));
    let a = f.expect(POOL, op::CREATE_BUFFER);
    let mut r = a.reader();
    assert_eq!(r.object(), Some(ObjectId(BUF_A)));
    assert_eq!(
        (r.int(), r.int(), r.int(), r.int(), r.uint()),
        (Some(0), Some(320), Some(200), Some(1280), Some(1))
    );
    let b = f.expect(POOL, op::CREATE_BUFFER);
    assert_eq!(b.reader().object(), Some(ObjectId(BUF_B)));
    let attach = f.expect(SURFACE, op::ATTACH);
    assert_eq!(attach.reader().object(), Some(ObjectId(BUF_A)));
    let damage = f.expect(SURFACE, op::DAMAGE_BUFFER);
    let mut r = damage.reader();
    assert_eq!(
        (r.int(), r.int(), r.int(), r.int()),
        (Some(0), Some(0), Some(320), Some(200))
    );
    assert_eq!(
        f.expect(SURFACE, op::FRAME).reader().object(),
        Some(ObjectId(CALLBACK))
    );
    f.expect(SURFACE, op::COMMIT);
    assert!(f.app.resource::<Surfaces>().is_configured(win));
    assert_eq!(
        f.app.resource::<Surfaces>().buffer_pixel(win, 0, 0),
        Some(0xffff_0000)
    );
    assert_eq!(
        f.app.resource::<Surfaces>().buffer_pixel(win, 319, 199),
        Some(0xffff_0000)
    );
    assert_eq!(
        f.app.component::<LayoutStyle>(win).unwrap().width,
        px(320.0)
    );
}

#[test]
fn a_zero_configure_settles_on_the_layout_size_or_the_default() {
    let mut f = fake();
    spawn(
        &mut f,
        LayoutStyle::default().column().size(px(100.0), px(50.0)),
    );
    f.requests();
    f.send(TOPLEVEL, ev::TOPLEVEL_CONFIGURE, |w| {
        w.int(0);
        w.int(0);
        w.array(&[]);
    });
    f.send(XDG, ev::XDG_CONFIGURE, |w| w.uint(1));
    f.turn();
    assert!(log(&f).contains(&"Resized 100x50".to_string()));

    let mut f = fake();
    spawn(&mut f, LayoutStyle::default().column());
    f.requests();
    f.send(TOPLEVEL, ev::TOPLEVEL_CONFIGURE, |w| {
        w.int(0);
        w.int(0);
        w.array(&[]);
    });
    f.send(XDG, ev::XDG_CONFIGURE, |w| w.uint(1));
    f.turn();
    assert!(log(&f).contains(&"Resized 640x480".to_string()));
}

#[test]
fn a_layer_configure_acks_and_draws_too() {
    let mut f = fake();
    let root = f.app.root();
    let role = Role::Layer(LayerRole {
        layer: Layer::Top,
        anchor: Anchor::TOP,
        exclusive_zone: 0,
        namespace: "bar".into(),
        keyboard_interactivity: KeyboardInteractivity::None,
    });
    f.app.spawn_with(
        root,
        window().layout(LayoutStyle::default().column()),
        (role,),
    );
    f.app.tick();
    f.turn();
    f.requests();
    f.send(XDG, ev::LAYER_CONFIGURE, |w| {
        w.uint(3);
        w.uint(800);
        w.uint(32);
    });
    f.turn();
    assert_eq!(f.expect(XDG, op::LAYER_ACK).reader().uint(), Some(3));
    assert!(log(&f).contains(&"Resized 800x32".to_string()));
    assert_eq!(attaches(&mut f), vec![LAYER_BUF_A]);
}

#[test]
fn frames_are_throttled_by_the_callback() {
    let mut f = fake();
    let win = spawn(&mut f, LayoutStyle::default().column());
    f.requests();
    f.app.signal(RequestFrame(win));
    f.app.flush();
    f.turn();
    assert!(attaches(&mut f).is_empty(), "nothing before configure");

    f.send(TOPLEVEL, ev::TOPLEVEL_CONFIGURE, |w| {
        w.int(64);
        w.int(64);
        w.array(&[]);
    });
    f.send(XDG, ev::XDG_CONFIGURE, |w| w.uint(1));
    f.turn();
    assert_eq!(
        attaches(&mut f),
        vec![BUF_A],
        "one frame for the configure and the request together"
    );

    f.app.signal(RequestFrame(win));
    f.app.flush();
    f.turn();
    assert!(attaches(&mut f).is_empty(), "the callback is outstanding");

    f.send(CALLBACK, ev::DONE, |w| w.uint(0));
    f.turn();
    assert_eq!(
        attaches(&mut f),
        vec![BUF_B],
        "the other buffer, once the callback fired"
    );

    f.send(CALLBACK + 1, ev::DONE, |w| w.uint(0));
    f.turn();
    assert!(attaches(&mut f).is_empty(), "nothing wanted");
}

#[test]
fn with_both_buffers_held_a_release_draws() {
    let mut f = fake();
    let win = configured(&mut f, 64, 64);
    f.requests();
    f.app.signal(RequestFrame(win));
    f.app.flush();
    f.send(CALLBACK, ev::DONE, |w| w.uint(0));
    f.turn();
    assert_eq!(attaches(&mut f), vec![BUF_B]);

    f.app.signal(RequestFrame(win));
    f.app.flush();
    f.send(CALLBACK + 1, ev::DONE, |w| w.uint(0));
    f.turn();
    assert!(attaches(&mut f).is_empty(), "both buffers are held");

    f.send(BUF_A, ev::RELEASE, |_| {});
    f.turn();
    assert_eq!(attaches(&mut f), vec![BUF_A]);
}

#[test]
fn two_configures_in_one_turn_draw_one_frame() {
    let mut f = fake();
    spawn(&mut f, LayoutStyle::default().column());
    f.requests();
    f.send(TOPLEVEL, ev::TOPLEVEL_CONFIGURE, |w| {
        w.int(64);
        w.int(64);
        w.array(&[]);
    });
    f.send(XDG, ev::XDG_CONFIGURE, |w| w.uint(1));
    f.send(TOPLEVEL, ev::TOPLEVEL_CONFIGURE, |w| {
        w.int(64);
        w.int(64);
        w.array(&[]);
    });
    f.send(XDG, ev::XDG_CONFIGURE, |w| w.uint(2));
    f.turn();
    assert_eq!(attaches(&mut f).len(), 1, "one frame for the whole burst");

    f.send(CALLBACK, ev::DONE, |w| w.uint(0));
    f.turn();
    assert_eq!(
        attaches(&mut f).len(),
        1,
        "the second frame, once the callback fired"
    );
}
