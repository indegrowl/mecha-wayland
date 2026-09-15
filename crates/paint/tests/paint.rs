//! Paint end to end: the module installed on an `App`, paints written,
//! ticks taken, `OnChanged<Paint>` observed.

use std::cell::{Cell, RefCell};

use app::prelude::*;
use geometry::{Point, Rect, Size};
use paint::prelude::*;

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
    /// Every `Emitted<OnChanged<Paint>>` seen: its targets.
    static CHANGED: RefCell<Vec<Vec<NodeId>>> = const { RefCell::new(Vec::new()) };
    /// The node a `Tick` system paints, when a test sets one.
    static TARGET: Cell<Option<NodeId>> = const { Cell::new(None) };
}

fn log_changed(_: &mut App, e: &Emitted<OnChanged<Paint>>) {
    CHANGED.with(|l| l.borrow_mut().push(e.targets.to_vec()));
}
fn take_changed() -> Vec<Vec<NodeId>> {
    CHANGED.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

/// A `Tick` system that paints `TARGET` black, if one is set.
fn paint_target_on_tick(app: &mut App, _: &Tick) {
    if let Some(id) = TARGET.with(|t| t.get()) {
        *app.component_mut::<Paint>(id).unwrap() = Paint::Quad(Quad::new(Color::BLACK));
    }
}

/// An app with the module and the logging system.
fn app() -> App {
    let mut app = App::new();
    app.add_module(PaintModule).system(log_changed);
    app
}

fn black() -> Paint {
    Paint::Quad(Quad::new(Color::BLACK))
}

fn glyph(color: Color) -> MonochromeSprite {
    MonochromeSprite {
        tile: AtlasTile {
            atlas: AtlasId(0),
            bounds: Rect::new(0.0, 0.0, 8.0, 8.0),
        },
        offset: Point::ZERO,
        size: Size::new(8.0, 8.0),
        color,
        is_opaque: true,
    }
}

fn set(app: &mut App, id: impl Into<NodeId>, paint: Paint) {
    *app.component_mut::<Paint>(id).unwrap() = paint;
}

// ── tests ───────────────────────────────────────────────────────────────

#[test]
fn every_node_defaults_to_none_and_a_clean_tick_reports_nothing() {
    let mut app = app();
    let a = app.spawn(app.root(), Leaf);
    let b = app.spawn(a, Leaf);
    assert_eq!(app.component::<Paint>(app.root()), Some(&Paint::None));
    assert_eq!(app.component::<Paint>(a), Some(&Paint::None));
    assert_eq!(app.component::<Paint>(b), Some(&Paint::None));
    app.tick();
    assert!(take_changed().is_empty());
}

#[test]
fn a_write_between_ticks_is_reported_once_for_that_node_only() {
    let mut app = app();
    let a = app.spawn(app.root(), Leaf);
    let _b = app.spawn(app.root(), Leaf);
    set(&mut app, a, black());
    assert!(take_changed().is_empty(), "nothing until the tick");
    app.tick();
    assert_eq!(take_changed(), vec![vec![a.id()]]);
    assert_eq!(app.component::<Paint>(a), Some(&black()));
    app.tick();
    assert!(take_changed().is_empty(), "the tick after reports nothing");
}

#[test]
fn a_write_by_a_tick_system_is_reported_in_the_same_tick() {
    let mut app = app();
    app.system(paint_target_on_tick);
    let a = app.spawn(app.root(), Leaf);
    TARGET.with(|t| t.set(Some(a.id())));
    app.tick();
    assert_eq!(take_changed(), vec![vec![a.id()]]);
    assert_eq!(app.component::<Paint>(a), Some(&black()));
}

#[test]
fn set_if_neq_reports_a_change_and_not_an_equal_write() {
    let mut app = app();
    let a = app.spawn(app.root(), Leaf);
    set(&mut app, a, black());
    app.tick();
    take_changed();

    assert!(!app.component_mut::<Paint>(a).unwrap().set_if_neq(black()));
    app.tick();
    assert!(take_changed().is_empty(), "an equal paint is not a change");

    let run = Paint::Monochrome(vec![glyph(Color::BLACK)]);
    assert!(
        app.component_mut::<Paint>(a)
            .unwrap()
            .set_if_neq(run.clone())
    );
    app.tick();
    assert_eq!(take_changed(), vec![vec![a.id()]]);

    let recolored = Paint::Monochrome(vec![glyph(Color::WHITE)]);
    assert!(app.component_mut::<Paint>(a).unwrap().set_if_neq(recolored));
    app.tick();
    assert_eq!(
        take_changed(),
        vec![vec![a.id()]],
        "a changed run is a change"
    );
}

#[test]
fn several_writes_are_one_target_per_node_in_first_write_order() {
    let mut app = app();
    let a = app.spawn(app.root(), Leaf);
    let b = app.spawn(app.root(), Leaf);
    let c = app.spawn(app.root(), Leaf);
    set(&mut app, b, black());
    set(&mut app, a, black());
    set(&mut app, b, Paint::Quad(Quad::new(Color::WHITE)));
    set(&mut app, c, black());
    app.tick();
    assert_eq!(take_changed(), vec![vec![b.id(), a.id(), c.id()]]);
}

#[test]
fn a_paint_in_a_bundle_at_spawn_is_reported_on_the_first_tick() {
    let mut app = app();
    let a = app.spawn_with(app.root(), Leaf, (black(),));
    assert_eq!(app.component::<Paint>(a), Some(&black()));
    app.tick();
    assert_eq!(take_changed(), vec![vec![a.id()]]);
}

#[test]
fn a_removed_node_is_not_reported_and_its_slot_starts_at_none() {
    let mut app = app();
    let old = app.spawn(app.root(), Leaf);
    set(&mut app, old, black());
    app.remove(old);
    // The only free slot is `old`'s, so `new` reuses it.
    let new = app.spawn(app.root(), Leaf);
    assert_eq!(new.id().slot(), old.id().slot(), "the slot is reused");
    assert_eq!(app.component::<Paint>(new), Some(&Paint::None));
    app.tick();
    assert!(take_changed().is_empty());
}

#[test]
#[should_panic]
fn installing_the_module_twice_panics() {
    let mut app = App::new();
    app.add_module(PaintModule).add_module(PaintModule);
}
