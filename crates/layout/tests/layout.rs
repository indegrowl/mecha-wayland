//! Layout end to end: the module installed on an `App`, roots marked by
//! hand, styles written, ticks taken, boxes read.

use std::cell::RefCell;

use app::prelude::*;
use geometry::{Insets, Rect, Size};
use layout::prelude::*;

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

// Systems cannot capture, so the tests log through thread-locals. Each
// test runs on its own thread, so logs never mix.
thread_local! {
    /// Every `LayoutDone` seen, in order: its `roots`.
    static DONE: RefCell<Vec<Vec<NodeId>>> = const { RefCell::new(Vec::new()) };
    /// Every `Emitted<OnChanged<Layout>>` seen: its targets.
    static MOVED: RefCell<Vec<Vec<NodeId>>> = const { RefCell::new(Vec::new()) };
    /// Every `Emitted<OnChanged<LayoutStyle>>` seen: its targets.
    static RESTYLED: RefCell<Vec<Vec<NodeId>>> = const { RefCell::new(Vec::new()) };
}

fn log_done(_: &mut App, d: &LayoutDone) {
    DONE.with(|l| l.borrow_mut().push(d.roots.clone()));
}
fn log_moved(_: &mut App, e: &Emitted<OnChanged<Layout>>) {
    MOVED.with(|l| l.borrow_mut().push(e.targets.to_vec()));
}
fn log_restyled(_: &mut App, e: &Emitted<OnChanged<LayoutStyle>>) {
    RESTYLED.with(|l| l.borrow_mut().push(e.targets.to_vec()));
}
fn take_done() -> Vec<Vec<NodeId>> {
    DONE.with(|l| std::mem::take(&mut *l.borrow_mut()))
}
fn take_moved() -> Vec<Vec<NodeId>> {
    MOVED.with(|l| std::mem::take(&mut *l.borrow_mut()))
}
fn take_restyled() -> Vec<Vec<NodeId>> {
    RESTYLED.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

/// An app with the module and the three logging systems.
fn app() -> App {
    let mut app = App::new();
    app.add_module(LayoutModule)
        .system(log_done)
        .system(log_moved)
        .system(log_restyled);
    app
}

/// A root of fixed size, a flex row with children aligned to the start so
/// a leaf keeps the height it asked for.
fn root_style(width: f32, height: f32) -> LayoutStyle {
    LayoutStyle::default()
        .size(px(width), px(height))
        .align_items(Align::Start)
}

/// Spawn a marked root under the app root.
fn root(app: &mut App, width: f32, height: f32) -> NodeId {
    app.spawn_with(
        app.root(),
        Leaf,
        (LayoutRoot(true), root_style(width, height)),
    )
    .id()
}

fn child(app: &mut App, parent: NodeId, style: LayoutStyle) -> NodeId {
    app.spawn_with(parent, Leaf, (style,)).id()
}

// Used from Task 5 or 6; the allow goes with it.
#[allow(dead_code)]
fn rect(app: &App, id: NodeId) -> Rect {
    app.component::<Layout>(id).unwrap().rect
}

fn fixed(w: f32, h: f32) -> LayoutStyle {
    LayoutStyle::default().size(px(w), px(h))
}

// Used from Task 5 or 6; the allow goes with it.
#[allow(dead_code)]
fn sorted(mut v: Vec<NodeId>) -> Vec<NodeId> {
    v.sort_by_key(|id| id.slot());
    v
}

// ── the module and the drain ────────────────────────────────────────────

#[test]
fn a_tick_with_nothing_to_do_signals_an_empty_layout_done() {
    let mut app = app();
    app.tick();
    app.tick();
    assert_eq!(take_done(), vec![Vec::<NodeId>::new(), Vec::new()]);
    assert!(take_moved().is_empty());
}

#[test]
fn on_changed_layout_style_never_fires_while_the_module_is_installed() {
    let mut app = app();
    let leaf = app.spawn(app.root(), Leaf).id();
    app.component_mut::<LayoutStyle>(leaf).unwrap().width = px(10.0);
    app.tick();
    assert!(
        take_restyled().is_empty(),
        "layout took the record before the drain"
    );
}

#[test]
fn measure_helpers() {
    let c = Constraints {
        known_width: Some(200.0),
        known_height: None,
        available_width: Available::Definite(200.0),
        available_height: Available::MaxContent,
    };
    assert!(!Measure::none().is_set());
    assert_eq!(Measure::none().measure(c), Size::ZERO);
    assert_eq!(Measure::default().measure(c), Size::ZERO);
    let f = Measure::fixed(Size::new(30.0, 20.0));
    assert!(f.is_set());
    assert_eq!(f.measure(c), Size::new(30.0, 20.0));
    let w = Measure::with(|c| Size::new(c.known_width.unwrap_or(0.0), 5.0));
    assert_eq!(w.measure(c), Size::new(200.0, 5.0));
    assert_eq!(format!("{:?}", Measure::none()), "Measure(unset)");
    assert_eq!(format!("{w:?}"), "Measure(set)");
}

#[test]
fn layout_content_is_inside_padding_and_border() {
    let l = Layout {
        rect: Rect::new(10.0, 20.0, 100.0, 50.0),
        padding: Insets::new(1.0, 2.0, 3.0, 4.0),
        border: Insets::all(1.0),
    };
    assert_eq!(l.content(), Rect::new(15.0, 22.0, 92.0, 44.0));
    let tiny = Layout {
        rect: Rect::new(0.0, 0.0, 10.0, 10.0),
        padding: Insets::all(8.0),
        border: Insets::all(0.0),
    };
    assert_eq!(tiny.content(), Rect::new(8.0, 8.0, 0.0, 0.0));
    assert!(!LayoutDone { roots: vec![] }.recomputed());
    assert!(
        LayoutDone {
            roots: vec![tiny_id()]
        }
        .recomputed()
    );
}

/// Any id will do for the signal's `recomputed`; the app root is always live.
fn tiny_id() -> NodeId {
    App::new().root()
}

// ── dirtiness ───────────────────────────────────────────────────────────

#[test]
fn a_style_write_under_a_root_dirties_that_root_once() {
    let mut app = app();
    let r = root(&mut app, 300.0, 100.0);
    let a = child(&mut app, r, fixed(50.0, 40.0));
    let b = child(&mut app, r, fixed(50.0, 40.0));
    app.tick();
    assert_eq!(take_done(), vec![vec![r]], "the spawn dirtied the root");

    app.component_mut::<LayoutStyle>(a).unwrap().width = px(80.0);
    app.component_mut::<LayoutStyle>(b).unwrap().width = px(80.0);
    app.tick();
    assert_eq!(take_done(), vec![vec![r]], "two writes, one root, once");

    app.tick();
    assert_eq!(take_done(), vec![vec![]], "nothing pending");
}

#[test]
fn a_measure_write_dirties_the_root() {
    let mut app = app();
    let r = root(&mut app, 300.0, 100.0);
    let a = child(&mut app, r, LayoutStyle::default());
    app.tick();
    take_done();
    *app.component_mut::<Measure>(a).unwrap() = Measure::fixed(Size::new(30.0, 20.0));
    app.tick();
    assert_eq!(take_done(), vec![vec![r]]);
}

#[test]
fn a_write_outside_every_root_dirties_nothing() {
    let mut app = app();
    let stray = app.spawn(app.root(), Leaf).id();
    app.tick();
    take_done();
    app.component_mut::<LayoutStyle>(stray).unwrap().width = px(10.0);
    app.tick();
    assert_eq!(take_done(), vec![vec![]]);
}

#[test]
fn two_roots_are_dirtied_independently() {
    let mut app = app();
    let r1 = root(&mut app, 300.0, 100.0);
    let r2 = root(&mut app, 300.0, 100.0);
    let a = child(&mut app, r1, fixed(50.0, 40.0));
    let _b = child(&mut app, r2, fixed(50.0, 40.0));
    app.tick();
    assert_eq!(
        take_done(),
        vec![vec![r1, r2]],
        "both marked, in first-write order"
    );

    app.component_mut::<LayoutStyle>(a).unwrap().width = px(80.0);
    app.tick();
    assert_eq!(take_done(), vec![vec![r1]]);
}

#[test]
fn spawn_and_remove_under_a_root_dirty_it() {
    let mut app = app();
    let r = root(&mut app, 300.0, 100.0);
    let a = child(&mut app, r, fixed(50.0, 40.0));
    app.tick();
    take_done();

    let b = child(&mut app, r, fixed(50.0, 40.0));
    app.tick();
    assert_eq!(take_done(), vec![vec![r]], "spawn");

    assert!(app.remove(a));
    app.tick();
    assert_eq!(take_done(), vec![vec![r]], "remove");

    assert!(app.remove(b));
    app.tick();
    assert_eq!(take_done(), vec![vec![r]], "remove the last child");
}

#[test]
fn removing_a_whole_root_dirties_nothing_and_does_not_panic() {
    let mut app = app();
    let r = root(&mut app, 300.0, 100.0);
    let _a = child(&mut app, r, fixed(50.0, 40.0));
    app.tick();
    take_done();
    assert!(app.remove(r));
    app.tick();
    assert_eq!(
        take_done(),
        vec![vec![]],
        "the root is gone, nothing to recompute"
    );
}

#[test]
fn marking_a_node_as_a_root_makes_it_its_own_dirty_root() {
    let mut app = app();
    let later = app
        .spawn_with(app.root(), Leaf, (root_style(200.0, 100.0),))
        .id();
    let _inner = child(&mut app, later, fixed(50.0, 40.0));
    app.tick();
    assert_eq!(take_done(), vec![vec![]], "not a root yet");
    app.component_mut::<LayoutRoot>(later).unwrap().0 = true;
    app.tick();
    assert_eq!(take_done(), vec![vec![later]]);
}
