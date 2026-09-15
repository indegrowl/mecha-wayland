//! Windows as nodes: the widget, membership, the live list, resize, scale,
//! and the frame request loop, end to end on an `App`.

use std::cell::RefCell;

use app::prelude::*;
use geometry::{Rect, Size};
use layout::prelude::*;
use window::prelude::*;

// ── fixtures ────────────────────────────────────────────────────────────

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

/// A node whose build spawns one `Leaf` child, so a test can see a child
/// whose `Spawned` arrives before its parent's.
struct Panel {
    child: Handle<Leaf>,
}
struct PanelBuilder;
impl Build for PanelBuilder {
    type Widget = Panel;
}
impl Widget for Panel {
    type Builder = PanelBuilder;
    fn build(_: PanelBuilder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        let child = s.spawn(me, Leaf);
        Panel { child }
    }
}

/// A node outside every window, under the app root, with one child. Neither
/// is in a window.
struct Owner {
    child: Handle<Leaf>,
}
struct OwnerBuilder;
impl Build for OwnerBuilder {
    type Widget = Owner;
}
impl Widget for Owner {
    type Builder = OwnerBuilder;
    fn build(_: OwnerBuilder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        let child = s.spawn(me, Leaf);
        Owner { child }
    }
}

// Systems cannot capture, so the tests log through thread-locals. Each
// test runs on its own thread, so logs never mix.
thread_local! {
    /// Every `FrameRequested` seen, in order.
    static REQUESTED: RefCell<Vec<NodeId>> = const { RefCell::new(Vec::new()) };
    /// Every `Frame` seen, in order.
    static FRAMES: RefCell<Vec<NodeId>> = const { RefCell::new(Vec::new()) };
    /// How many `OnChanged<Windows>` were seen.
    static WINDOWS_CHANGED: RefCell<u32> = const { RefCell::new(0) };
    /// Every `LayoutDone` seen, in order: its `roots`.
    static DONE: RefCell<Vec<Vec<NodeId>>> = const { RefCell::new(Vec::new()) };
}

fn log_requested(_: &mut App, r: &FrameRequested) {
    REQUESTED.with(|l| l.borrow_mut().push(r.0));
}
fn log_frame(_: &mut App, f: &Frame) {
    FRAMES.with(|l| l.borrow_mut().push(f.0));
}
fn log_windows_changed(_: &mut App, _: &OnChanged<Windows>) {
    WINDOWS_CHANGED.with(|n| *n.borrow_mut() += 1);
}
fn log_done(_: &mut App, d: &LayoutDone) {
    DONE.with(|l| l.borrow_mut().push(d.roots.clone()));
}
fn take_done() -> Vec<Vec<NodeId>> {
    DONE.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

#[allow(dead_code)]
fn take_requested() -> Vec<NodeId> {
    REQUESTED.with(|l| std::mem::take(&mut *l.borrow_mut()))
}
#[allow(dead_code)]
fn take_frames() -> Vec<NodeId> {
    FRAMES.with(|l| std::mem::take(&mut *l.borrow_mut()))
}
#[allow(dead_code)]
fn take_windows_changed() -> u32 {
    WINDOWS_CHANGED.with(|n| std::mem::take(&mut *n.borrow_mut()))
}

/// The WSI stand-in: answers every `FrameRequested` with a `Frame`.
#[allow(dead_code)]
fn answer(app: &mut App, r: &FrameRequested) {
    app.signal(Frame(r.0));
}

/// An app with layout, window and the logging systems. The loop is open:
/// nothing answers `FrameRequested`.
fn app() -> App {
    let mut app = App::new();
    app.add_module(LayoutModule)
        .add_module(WindowModule)
        .system(log_requested)
        .system(log_frame)
        .system(log_windows_changed)
        .system(log_done);
    app
}

/// `app()` with the loop closed by `answer`.
#[allow(dead_code)]
fn app_closed() -> App {
    let mut app = app();
    app.system(answer);
    app
}

/// A window asking for a fixed box.
fn a_window() -> WindowBuilder {
    window()
        .title("w")
        .layout(LayoutStyle::default().size(px(480.0), px(240.0)))
}

// ── 1: the widget ───────────────────────────────────────────────────────

#[test]
fn a_spawned_window_is_a_root_with_the_builders_style_and_is_laid_out() {
    let mut app = app();
    let win = app.spawn(app.root(), a_window());
    app.tick();

    assert_eq!(app.widget::<Window>(win).unwrap().title, "w");
    assert_eq!(app.component::<LayoutRoot>(win), Some(&LayoutRoot(true)));
    let style = app.component::<LayoutStyle>(win).unwrap();
    assert_eq!((style.width, style.height), (px(480.0), px(240.0)));
    assert_eq!(
        app.component::<InWindow>(win),
        Some(&InWindow(Some(win.id())))
    );
    assert_eq!(
        app.component::<Layout>(win).unwrap().rect,
        Rect::new(0.0, 0.0, 480.0, 240.0)
    );
    assert_eq!(take_done(), vec![vec![win.id()]]);
    assert!(!app.widget::<Window>(win).unwrap().is_pending());
    assert_eq!(app.widget::<Window>(win).unwrap().scale(), 1.0);
}

#[test]
fn the_default_style_is_a_column_sized_to_content() {
    let mut app = app();
    let win = app.spawn(app.root(), window());
    app.spawn_with(win, Leaf, (Measure::fixed(Size::new(48.0, 32.0)),));
    app.spawn_with(win, Leaf, (Measure::fixed(Size::new(20.0, 10.0)),));
    app.tick();

    let style = app.component::<LayoutStyle>(win).unwrap();
    assert_eq!(style.direction, Direction::Column);
    assert_eq!((style.width, style.height), (auto(), auto()));
    assert_eq!(
        app.component::<Layout>(win).unwrap().rect,
        Rect::new(0.0, 0.0, 48.0, 42.0),
        "as wide as the widest child, as tall as both stacked"
    );
}

// ── 2: InWindow ─────────────────────────────────────────────────────────

#[test]
fn in_window_is_filled_for_every_node_under_a_window_and_none_elsewhere() {
    let mut app = app();
    let win = app.spawn(app.root(), a_window());
    let panel = app.spawn(win, PanelBuilder);
    let grandchild = app.widget::<Panel>(panel).unwrap().child;
    let owner = app.spawn(app.root(), OwnerBuilder);
    let owned = app.widget::<Owner>(owner).unwrap().child;
    app.flush();
    let later = app.spawn(win, Leaf);
    let outside = app.spawn(app.root(), Leaf);
    app.flush();

    let in_window = |id: NodeId| app.component::<InWindow>(id).unwrap().0;
    assert_eq!(in_window(win.id()), Some(win.id()), "a window is in itself");
    assert_eq!(
        in_window(panel.id()),
        Some(win.id()),
        "spawned under the window"
    );
    assert_eq!(
        in_window(grandchild.id()),
        Some(win.id()),
        "spawned in the panel's build, so its Spawned came before the panel's"
    );
    assert_eq!(
        in_window(later.id()),
        Some(win.id()),
        "spawned after a flush"
    );
    assert_eq!(in_window(owner.id()), None, "an owner under the app root");
    assert_eq!(in_window(owned.id()), None, "the owner's child");
    assert_eq!(in_window(outside.id()), None, "a leaf under the app root");
    assert_eq!(in_window(app.root()), None, "the app root");
}
