// The fixture below (ids, opcodes, event opcodes, `configured`, `attaches`,
// `log`, the logging systems) serves every task in this slice; only Task 7's
// three tests use it so far, so most of it is unused until Tasks 8-10 add
// their tests to this file.
#![allow(dead_code)]

use app::prelude::*;
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
