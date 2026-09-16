// The fixture below (ids, opcodes, event opcodes, `configured`, `attaches`,
// `log`, the logging systems) serves every task in this slice. Every test
// needs the GPU: `fake()` returns `None` and prints a skip line without one.

use std::sync::{Mutex, MutexGuard};

use app::prelude::*;
use atlas::Atlas;
use geometry::Color;
use gles::{Budget, Device, Error};
use layout::prelude::*;
use paint::prelude::*;
use presentation::prelude::*;
use render::prelude::*;
use wayland::fake::Fake;
use wayland::prelude::*;
use window::prelude::*;

const GLOBALS: &[(&str, u32)] = &[
    ("wl_compositor", 6),
    ("zwp_linux_dmabuf_v1", 3),
    ("xdg_wm_base", 7),
    ("zwlr_layer_shell_v1", 5),
];

// Deterministic ids: registry 2, sync callback 3, compositor 4, dmabuf 5,
// wm_base 6, layer shell 7 (bound by presentation's install). The first
// window: surface 8, xdg surface 9, toplevel 10 (or layer surface 9).
// Its slots after configure: params 11 and buffer 12, params 13 and
// buffer 14; the first frame callback is 15. Ids are never reused: the
// fake sends no `delete_id`.
const DMABUF: u32 = 5;
const SURFACE: u32 = 8;
const XDG: u32 = 9;
const TOPLEVEL: u32 = 10;
const PARAMS_A: u32 = 11;
const BUF_A: u32 = 12;
const PARAMS_B: u32 = 13;
const BUF_B: u32 = 14;
const CALLBACK: u32 = 15;

/// `DRM_FORMAT_XRGB8888`.
const XRGB: u32 = 0x3432_5258;

// Opcodes, from the XML order.
mod op {
    pub const CREATE_SURFACE: u16 = 0;
    pub const CREATE_PARAMS: u16 = 1;
    pub const PARAMS_DESTROY: u16 = 0;
    pub const PARAMS_ADD: u16 = 1;
    pub const PARAMS_CREATE_IMMED: u16 = 3;
    /// Both a buffer's and a target's teardown; used by the scale test
    /// here and by the removal test (Task 7).
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
    /// No test removes a layer window (only a toplevel's removal is
    /// covered), so this opcode names a request no test sends.
    #[allow(dead_code)]
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
    pub const DMABUF_MODIFIER: u16 = 1;
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

static GPU: Mutex<()> = Mutex::new(());

/// The fake compositor with the whole stack installed, behind the GPU
/// lock, after it advertised the linear modifier for XRGB8888. `None`
/// with a skip line when there is no render node.
fn fake() -> Option<(MutexGuard<'static, ()>, Fake)> {
    let guard = GPU.lock().unwrap_or_else(|e| e.into_inner());
    match Device::try_open(Budget::default()) {
        Ok(_) => {}
        Err(Error::NoDevice) => {
            eprintln!("skip: no render node");
            return None;
        }
        Err(e) => panic!("the device did not open: {e:?}"),
    }
    let mut f = Fake::new(GLOBALS, |m| {
        m.bind::<WlCompositor>()
            .bind::<ZwpLinuxDmabufV1>()
            .bind::<XdgWmBase>()
    });
    f.send(DMABUF, ev::DMABUF_MODIFIER, |w| {
        w.uint(XRGB);
        w.uint(0);
        w.uint(0);
    });
    f.app
        .add_module(LayoutModule)
        .add_module(PaintModule)
        .add_module(WindowModule)
        .add_module(RenderModule::default())
        .insert_resource(Atlas::new());
    f.app
        .init_resource::<Log>()
        .system(log_resized)
        .system(log_scale)
        .system(log_close)
        .system(log_frame)
        .add_module(PresentationModule {
            app_id: "test".into(),
            budget: Budget::default(),
        });
    f.turn();
    f.requests();
    Some((guard, f))
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

/// A toplevel configured to `width` by `height` and drawn: the configure
/// changes the layout, so the frame comes one tick later.
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
    f.app.tick();
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

/// The `(sender, opcode)` shape of the requests, drained.
fn shape(f: &mut Fake) -> Vec<(u32, u16)> {
    f.requests()
        .iter()
        .map(|r| (r.sender.0, r.opcode))
        .collect()
}

/// Changes the window's clear colour through a child quad, so render
/// requests a frame for a real change.
fn repaint(f: &mut Fake, win: NodeId) {
    struct Leaf;
    impl Build for Leaf {
        type Widget = Leaf;
    }
    impl Widget for Leaf {
        type Builder = Leaf;
        fn build(b: Leaf, _: Handle<Self>, _: &mut Spawner<'_, Self>) -> Self {
            b
        }
    }
    f.app.spawn_with(
        win,
        Leaf,
        (
            LayoutStyle::default().size(px(10.0), px(10.0)),
            Paint::Quad(Quad::new(Color::rgb(1.0, 0.0, 0.0))),
        ),
    );
    f.app.tick();
}

// ── Task 7 ───────────────────────────────────────────────────────────────

#[test]
fn a_toplevel_window_gets_a_surface_a_role_and_an_initial_commit() {
    let Some((_gpu, mut f)) = fake() else { return };
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
    let Some((_gpu, mut f)) = fake() else { return };
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
    let Some((_gpu, mut f)) = fake() else { return };
    f.send(6, ev::PING, |w| w.uint(77));
    f.turn();
    let pong = f.expect(6, op::PONG);
    assert_eq!(pong.reader().uint(), Some(77));
}

// ── Task 8 ───────────────────────────────────────────────────────────────

// A layer window allocates only surface 8 and layer surface 9 (no
// toplevel), so its first slot's params is 10 and buffer 11.
const LAYER_BUF_A: u32 = 11;

#[test]
fn a_configure_acks_makes_two_dmabuf_slots_and_the_layout_frame_draws() {
    let Some((_gpu, mut f)) = fake() else { return };
    let win = spawn(&mut f, LayoutStyle::default().column());
    f.requests();
    f.send(TOPLEVEL, ev::TOPLEVEL_CONFIGURE, |w| {
        w.int(320);
        w.int(200);
        w.array(&[]);
    });
    f.send(XDG, ev::XDG_CONFIGURE, |w| w.uint(1));
    f.turn();
    // The ack and the slots, but no frame yet: the size changed the layout.
    let reqs = f.requests();
    let s: Vec<(u32, u16)> = reqs.iter().map(|r| (r.sender.0, r.opcode)).collect();
    assert_eq!(
        s,
        vec![
            (XDG, op::ACK_CONFIGURE),
            (DMABUF, op::CREATE_PARAMS),
            (PARAMS_A, op::PARAMS_ADD),
            (PARAMS_A, op::PARAMS_CREATE_IMMED),
            (PARAMS_A, op::PARAMS_DESTROY),
            (DMABUF, op::CREATE_PARAMS),
            (PARAMS_B, op::PARAMS_ADD),
            (PARAMS_B, op::PARAMS_CREATE_IMMED),
            (PARAMS_B, op::PARAMS_DESTROY),
        ]
    );
    let mut add = reqs[2].reader();
    assert_eq!(add.uint(), Some(0), "plane 0");
    assert_eq!(add.uint(), Some(0), "offset 0");
    assert!(add.uint().unwrap() >= 320 * 4, "stride covers the row");
    assert_eq!(
        (add.uint(), add.uint()),
        (Some(0), Some(0)),
        "the linear modifier"
    );
    let mut immed = reqs[3].reader();
    assert_eq!(immed.object(), Some(ObjectId(BUF_A)));
    assert_eq!((immed.int(), immed.int()), (Some(320), Some(200)));
    assert_eq!(immed.uint(), Some(XRGB));
    assert_eq!(log(&f), vec!["Resized 320x200".to_string()]);
    assert!(f.app.resource::<Surfaces>().is_configured(win));
    assert_eq!(
        f.app.component::<LayoutStyle>(win).unwrap().width,
        px(320.0)
    );

    // The layout change requests the frame; it is drawn on the next tick.
    f.app.tick();
    f.turn();
    let reqs = f.requests();
    let s: Vec<(u32, u16)> = reqs.iter().map(|r| (r.sender.0, r.opcode)).collect();
    assert_eq!(
        s,
        vec![
            (SURFACE, op::ATTACH),
            (SURFACE, op::DAMAGE_BUFFER),
            (SURFACE, op::FRAME),
            (SURFACE, op::COMMIT),
        ]
    );
    assert_eq!(reqs[0].reader().object(), Some(ObjectId(BUF_A)));
    let mut dmg = reqs[1].reader();
    assert_eq!(
        (dmg.int(), dmg.int(), dmg.int(), dmg.int()),
        (Some(0), Some(0), Some(320), Some(200))
    );
    assert_eq!(reqs[2].reader().object(), Some(ObjectId(CALLBACK)));
    assert_eq!(
        log(&f),
        vec!["Resized 320x200".to_string(), format!("Frame {win:?}")]
    );
}

#[test]
fn a_zero_configure_settles_on_the_layout_size_and_draws_at_once() {
    let Some((_gpu, mut f)) = fake() else { return };
    let win = spawn(
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
    // The settled size equals the layout's, so the frame is kicked now.
    let s = shape(&mut f);
    assert_eq!(s[0], (XDG, op::ACK_CONFIGURE));
    assert!(s.contains(&(SURFACE, op::ATTACH)), "{s:?}");
    assert!(s.contains(&(SURFACE, op::COMMIT)), "{s:?}");
    // `kick`'s `app.signal(Frame(w))` is queued directly, ahead of
    // `Resized`'s `Emitted<Resized>`, which only reaches the signal queue
    // after `app.flush` drains the `Resized` event job itself: "Frame"
    // logs before "Resized" every time settle's own kick fires.
    assert_eq!(
        log(&f),
        vec![format!("Frame {win:?}"), "Resized 100x50".to_string()]
    );
}

#[test]
fn a_zero_configure_with_no_layout_size_takes_the_default() {
    let Some((_gpu, mut f)) = fake() else { return };
    spawn(&mut f, LayoutStyle::default().column());
    f.requests();
    f.send(TOPLEVEL, ev::TOPLEVEL_CONFIGURE, |w| {
        w.int(0);
        w.int(0);
        w.array(&[]);
    });
    f.send(XDG, ev::XDG_CONFIGURE, |w| w.uint(1));
    f.turn();
    assert_eq!(log(&f), vec!["Resized 640x480".to_string()]);
    let reqs = f.requests();
    let immed = reqs
        .iter()
        .find(|r| r.opcode == op::PARAMS_CREATE_IMMED)
        .unwrap();
    let mut r = immed.reader();
    r.object();
    assert_eq!((r.int(), r.int()), (Some(640), Some(480)));
}

#[test]
fn a_layer_configure_acks_and_draws_too() {
    let Some((_gpu, mut f)) = fake() else { return };
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
    f.app.tick();
    f.turn();
    assert!(log(&f).contains(&"Resized 800x32".to_string()));
    let reqs = f.requests();
    let s: Vec<(u32, u16)> = reqs.iter().map(|r| (r.sender.0, r.opcode)).collect();
    let ack = reqs
        .iter()
        .find(|r| r.sender == ObjectId(XDG) && r.opcode == op::LAYER_ACK)
        .unwrap();
    assert_eq!(ack.reader().uint(), Some(3));
    assert!(s.contains(&(SURFACE, op::ATTACH)), "{s:?}");
    assert!(s.contains(&(SURFACE, op::COMMIT)), "{s:?}");
    let attach = reqs
        .iter()
        .find(|r| r.sender == ObjectId(SURFACE) && r.opcode == op::ATTACH)
        .unwrap();
    assert_eq!(attach.reader().object(), Some(ObjectId(LAYER_BUF_A)));
}

#[test]
fn frames_are_throttled_by_the_callback() {
    let Some((_gpu, mut f)) = fake() else { return };
    let win = configured(&mut f, 320, 200);
    f.requests();
    repaint(&mut f, win);
    f.turn();
    assert!(attaches(&mut f).is_empty(), "the callback is outstanding");
    f.send(CALLBACK, ev::DONE, |w| w.uint(1));
    f.turn();
    assert_eq!(attaches(&mut f), vec![BUF_B]);

    f.send(CALLBACK + 1, ev::DONE, |w| w.uint(0));
    f.turn();
    assert!(attaches(&mut f).is_empty(), "nothing wanted");
}

#[test]
fn with_both_buffers_held_a_release_draws() {
    let Some((_gpu, mut f)) = fake() else { return };
    let win = configured(&mut f, 64, 64);
    f.requests();
    repaint(&mut f, win);
    f.send(CALLBACK, ev::DONE, |w| w.uint(0));
    f.turn();
    assert_eq!(attaches(&mut f), vec![BUF_B]);

    repaint(&mut f, win);
    f.send(CALLBACK + 1, ev::DONE, |w| w.uint(0));
    f.turn();
    assert!(attaches(&mut f).is_empty(), "both buffers are held");

    f.send(BUF_A, ev::RELEASE, |_| {});
    f.turn();
    assert_eq!(attaches(&mut f), vec![BUF_A]);
}

#[test]
fn two_configures_in_one_turn_draw_one_frame() {
    let Some((_gpu, mut f)) = fake() else { return };
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
    f.app.tick();
    f.turn();
    assert_eq!(attaches(&mut f).len(), 1, "one frame for the whole burst");

    f.send(CALLBACK, ev::DONE, |w| w.uint(0));
    f.turn();
    assert_eq!(attaches(&mut f).len(), 0, "nothing wanted yet");
}

// ── Task 9 ───────────────────────────────────────────────────────────────

#[test]
fn a_preferred_scale_rescales_the_slots_and_redraws_at_once() {
    let Some((_gpu, mut f)) = fake() else { return };
    let win = configured(&mut f, 320, 200);
    f.requests();
    f.send(CALLBACK, ev::DONE, |w| w.uint(1));
    f.turn();
    f.requests();
    f.send(SURFACE, ev::PREFERRED_SCALE, |w| w.int(2));
    f.turn();
    let reqs = f.requests();
    let s: Vec<(u32, u16)> = reqs.iter().map(|r| (r.sender.0, r.opcode)).collect();
    // Old buffers destroyed, two new slots made, then the frame at scale 2.
    assert_eq!(
        &s[..2],
        &[(BUF_A, op::BUFFER_DESTROY), (BUF_B, op::BUFFER_DESTROY)]
    );
    // `zwp_linux_buffer_params_v1.create_immed` and `wl_surface.frame` are
    // both opcode 3 in the real protocol, and the kick draws a frame in
    // this same turn, so the opcode alone is ambiguous: exclude `SURFACE`.
    let immed: Vec<&wayland::fake::Request> = reqs
        .iter()
        .filter(|r| r.opcode == op::PARAMS_CREATE_IMMED && r.sender != ObjectId(SURFACE))
        .collect();
    assert_eq!(immed.len(), 2);
    let mut r = immed[0].reader();
    r.object();
    assert_eq!(
        (r.int(), r.int()),
        (Some(640), Some(400)),
        "device pixels at scale 2"
    );
    let tail: Vec<(u32, u16)> = s[s.len() - 5..].to_vec();
    assert_eq!(
        tail,
        vec![
            (SURFACE, op::SET_BUFFER_SCALE),
            (SURFACE, op::ATTACH),
            (SURFACE, op::DAMAGE_BUFFER),
            (SURFACE, op::FRAME),
            (SURFACE, op::COMMIT),
        ]
    );
    let mut dmg = reqs[reqs.len() - 3].reader();
    assert_eq!(
        (dmg.int(), dmg.int(), dmg.int(), dmg.int()),
        (Some(0), Some(0), Some(640), Some(400)),
        "the whole window at the new scale"
    );
    // `kick`'s `app.signal(Frame(w))` inside `on_surface` queues directly,
    // ahead of `ScaleFactorChanged`'s `Emitted<ScaleFactorChanged>`, which
    // only reaches the signal queue after the event that carried it is
    // fully flushed: the same ordering Task 5 hit with `Resized`, so
    // "Frame" logs before "Scale 2" even though `on_surface` emitted the
    // scale change first.
    assert_eq!(
        log(&f),
        vec![
            "Resized 320x200".to_string(),
            format!("Frame {win:?}"),
            format!("Frame {win:?}"),
            "Scale 2".to_string(),
        ]
    );
    assert_eq!(
        f.app.component::<LayoutStyle>(win).unwrap().width,
        px(320.0),
        "logical size unchanged"
    );
}

#[test]
fn a_configure_to_a_new_size_waits_for_the_layout_and_draws_the_new_size() {
    let Some((_gpu, mut f)) = fake() else { return };
    let win = configured(&mut f, 320, 200);
    f.requests();
    f.send(CALLBACK, ev::DONE, |w| w.uint(1));
    f.turn();
    f.requests();
    f.send(TOPLEVEL, ev::TOPLEVEL_CONFIGURE, |w| {
        w.int(400);
        w.int(300);
        w.array(&[]);
    });
    f.send(XDG, ev::XDG_CONFIGURE, |w| w.uint(2));
    f.turn();
    let s = shape(&mut f);
    assert!(
        !s.contains(&(SURFACE, op::ATTACH)),
        "no frame before the layout: {s:?}"
    );
    assert_eq!(
        s.iter().filter(|r| r.1 == op::PARAMS_CREATE_IMMED).count(),
        2,
        "new slots"
    );
    f.app.tick();
    f.turn();
    let reqs = f.requests();
    let attach = reqs
        .iter()
        .find(|r| r.opcode == op::ATTACH)
        .expect("the layout's frame");
    let dmg = reqs.iter().find(|r| r.opcode == op::DAMAGE_BUFFER).unwrap();
    let mut d = dmg.reader();
    assert_eq!(
        (d.int(), d.int(), d.int(), d.int()),
        (Some(0), Some(0), Some(400), Some(300))
    );
    let _ = attach;
    assert_eq!(
        f.app.component::<LayoutStyle>(win).unwrap().width,
        px(400.0)
    );
}

#[test]
fn close_and_closed_are_advice_at_the_window() {
    let Some((_gpu, mut f)) = fake() else { return };
    let win = configured(&mut f, 64, 64);
    f.send(TOPLEVEL, ev::CLOSE, |_| {});
    f.turn();
    assert_eq!(log(&f).last().unwrap(), &format!("Close Some({win:?})"));
    assert!(f.app.is_live(win), "presentation removes nothing");
    // The second `fake()` locks the same GPU mutex; drop this one's guard
    // first or the second `fake()` deadlocks against itself.
    drop(f);
    drop(_gpu);

    let Some((_gpu, mut f)) = fake() else { return };
    let root = f.app.root();
    let role = Role::Layer(LayerRole {
        layer: Layer::Bottom,
        anchor: Anchor::BOTTOM,
        exclusive_zone: -1,
        namespace: "wall".into(),
        keyboard_interactivity: KeyboardInteractivity::None,
    });
    let win = f.app.spawn_with(root, window(), (role,)).id();
    f.app.tick();
    f.turn();
    f.send(XDG, ev::LAYER_CLOSED, |_| {});
    f.turn();
    assert_eq!(log(&f).last().unwrap(), &format!("Close Some({win:?})"));
}

#[test]
fn removing_a_window_destroys_its_objects_in_order_and_forgets_them() {
    let Some((_gpu, mut f)) = fake() else { return };
    let win = configured(&mut f, 320, 200);
    f.requests();
    f.app.remove(win);
    f.app.tick();
    f.turn();
    let s = shape(&mut f);
    assert_eq!(
        s,
        vec![
            (TOPLEVEL, op::TOPLEVEL_DESTROY),
            (XDG, op::XDG_DESTROY),
            (BUF_A, op::BUFFER_DESTROY),
            (BUF_B, op::BUFFER_DESTROY),
            (SURFACE, op::SURFACE_DESTROY),
        ]
    );
    assert!(f.app.resource::<Surfaces>().surface_of(win).is_none());
    // A late release for a destroyed buffer is ignored.
    f.send(BUF_A, ev::RELEASE, |_| {});
    f.turn();
    assert!(shape(&mut f).is_empty());
}

// ── Task 7 (atlas + last_frame) ─────────────────────────────────────────

#[test]
fn the_first_frame_holds_the_clear_colour() {
    let Some((_gpu, mut f)) = fake() else { return };
    let root = f.app.root();
    let win = f
        .app
        .spawn(
            root,
            window()
                .title("hello")
                .clear(Color::rgb(0.0, 0.0, 1.0))
                .layout(LayoutStyle::default().column().size(px(20.0), px(10.0))),
        )
        .id();
    f.app.tick();
    f.turn();
    f.requests();
    f.send(TOPLEVEL, ev::TOPLEVEL_CONFIGURE, |w| {
        w.int(0);
        w.int(0);
        w.array(&[]);
    });
    f.send(XDG, ev::XDG_CONFIGURE, |w| w.uint(1));
    f.turn();
    let (w, h, px) = f
        .app
        .resource_mut::<Surfaces>()
        .last_frame(win)
        .expect("a frame was drawn");
    assert_eq!((w, h), (20, 10));
    assert_eq!(&px[..4], &[0, 0, 255, 255]);
    assert_eq!(&px[px.len() - 4..], &[0, 0, 255, 255]);
}

#[test]
fn a_glyph_resolved_in_a_tick_is_on_the_gpu_before_that_ticks_frame() {
    let Some((_gpu, mut f)) = fake() else { return };
    let win = configured(&mut f, 64, 64);
    f.requests();
    f.send(CALLBACK, ev::DONE, |w| w.uint(1));
    f.turn();
    // Resolve a glyph and paint it in one tick: the atlas upload runs on
    // OnChanged<Atlas> at PostTick, before render's request for the frame.
    let tile = {
        let mut atlas = f.app.resource_mut::<Atlas>();
        let inter = atlas
            .add_font(include_bytes!(
                "../../atlas/tests/fixtures/Inter-Regular.ttf"
            ))
            .unwrap();
        let (font, id) = atlas.lookup(&[inter], 'a').unwrap();
        atlas.glyph(font, id, 40).tile
    };
    struct Leaf;
    impl Build for Leaf {
        type Widget = Leaf;
    }
    impl Widget for Leaf {
        type Builder = Leaf;
        fn build(b: Leaf, _: Handle<Self>, _: &mut Spawner<'_, Self>) -> Self {
            b
        }
    }
    let size = tile.bounds.size;
    f.app.spawn_with(
        win,
        Leaf,
        (
            LayoutStyle::default().size(px(size.width), px(size.height)),
            Paint::Monochrome(vec![MonochromeSprite::new(
                tile,
                geometry::Point::ZERO,
                size,
                Color::WHITE,
            )]),
        ),
    );
    f.app.tick();
    f.turn();
    assert!(!attaches(&mut f).is_empty(), "the glyph frame was drawn");
    let (w, _h, px) = f.app.resource_mut::<Surfaces>().last_frame(win).unwrap();
    let brightest = px.chunks(4).map(|p| p[0]).max().unwrap();
    assert!(
        brightest > 200,
        "the glyph's ink is white on black: {brightest}"
    );
    let _ = w;
}

// ── final review ─────────────────────────────────────────────────────────

#[test]
fn a_frame_with_nothing_changed_commits_nothing() {
    let Some((_gpu, mut f)) = fake() else { return };
    let win = configured(&mut f, 320, 200);
    f.requests();
    f.send(CALLBACK, ev::DONE, |w| w.uint(1));
    // The first draw's slot is released too, so the second draw reuses it
    // at age 1 rather than the other, never-drawn slot: a never-drawn
    // slot's age is 0, which `render::Scenes` always treats as "the whole
    // window", so nothing-changed could never be demonstrated on it.
    f.send(BUF_A, ev::RELEASE, |_| {});
    f.turn();
    // A second configure of the same size kicks a Frame; render's scissor
    // is empty, so nothing is attached or committed.
    f.send(TOPLEVEL, ev::TOPLEVEL_CONFIGURE, |w| {
        w.int(320);
        w.int(200);
        w.array(&[]);
    });
    f.send(XDG, ev::XDG_CONFIGURE, |w| w.uint(2));
    f.turn();
    let s = shape(&mut f);
    assert_eq!(s, vec![(XDG, op::ACK_CONFIGURE)]);
    assert!(log(&f).contains(&format!("Frame {win:?}")), "{:?}", log(&f));
}
