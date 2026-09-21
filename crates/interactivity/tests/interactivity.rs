//! Pointer and touch input, end to end on an `App`. There is no
//! `presentation` yet, so every test sends `ContactInput` by hand.

use std::cell::RefCell;

use app::prelude::*;
use geometry::Point;
use interactivity::prelude::*;
use layout::prelude::*;
use window::prelude::*;

thread_local! {
    static SEEN: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
}
fn log_contact_input(_: &mut App, input: &ContactInput) {
    SEEN.with(|l| {
        l.borrow_mut().push(match input.phase {
            ContactPhase::Moved => "moved",
            ContactPhase::Pressed => "pressed",
            ContactPhase::Released => "released",
            ContactPhase::Cancelled => "cancelled",
        })
    });
}

#[test]
fn contact_input_is_a_plain_signal() {
    let mut app = App::new();
    app.system(log_contact_input);
    let root = app.root();
    app.signal(ContactInput {
        window: root,
        contact: ContactId::Mouse,
        phase: ContactPhase::Moved,
        position: Point::new(1.0, 2.0),
    });
    app.flush();
    assert_eq!(SEEN.with(|l| l.borrow().clone()), vec!["moved"]);
}

#[test]
fn every_event_carries_its_contact_and_position_and_reaches_its_target() {
    struct Watcher;
    struct WatcherBuilder(std::rc::Rc<std::cell::Cell<Option<ContactId>>>);
    impl Build for WatcherBuilder {
        type Widget = Watcher;
    }
    impl Widget for Watcher {
        type Builder = WatcherBuilder;
        fn build(b: WatcherBuilder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
            let flag = b.0.clone();
            s.on::<Press>(me, move |_, e| flag.set(Some(e.contact)));
            Watcher
        }
    }

    let mut app = App::new();
    let seen = std::rc::Rc::new(std::cell::Cell::new(None));
    let watcher = app.spawn(app.root(), WatcherBuilder(seen.clone()));
    let pos = Point::new(3.0, 4.0);
    app.emit(
        Press {
            contact: ContactId::Touch(9),
            position: pos,
        },
        watcher,
    );
    app.flush();
    assert_eq!(seen.get(), Some(ContactId::Touch(9)));
}

// ── fixtures for the dispatch tests ────────────────────────────────────

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

thread_local! {
    static ENTER: RefCell<Vec<NodeId>> = const { RefCell::new(Vec::new()) };
    static EXIT: RefCell<Vec<NodeId>> = const { RefCell::new(Vec::new()) };
}
fn log_enter(_: &mut App, e: &Emitted<Enter>) {
    ENTER.with(|l| l.borrow_mut().extend(e.targets.iter().copied()));
}
fn log_exit(_: &mut App, e: &Emitted<Exit>) {
    EXIT.with(|l| l.borrow_mut().extend(e.targets.iter().copied()));
}
fn take_enter() -> Vec<NodeId> {
    ENTER.with(|l| std::mem::take(&mut *l.borrow_mut()))
}
fn take_exit() -> Vec<NodeId> {
    EXIT.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

fn app() -> App {
    let mut app = App::new();
    app.add_module(LayoutModule)
        .add_module(WindowModule)
        .add_module(InteractivityModule)
        .system(log_enter)
        .system(log_exit);
    app
}

/// A 200x200 window at the origin.
fn a_window() -> WindowBuilder {
    window().layout(LayoutStyle::default().size(px(200.0), px(200.0)))
}

/// An absolutely positioned box at `(x, y)`, `w` by `h`, relative to its
/// containing block.
fn at(x: f32, y: f32, w: f32, h: f32) -> LayoutStyle {
    LayoutStyle::default()
        .absolute()
        .inset(geometry::Insets::new(px(y), auto(), auto(), px(x)))
        .size(px(w), px(h))
}

fn moved(window: impl Into<NodeId>, contact: ContactId, position: Point) -> ContactInput {
    ContactInput {
        window: window.into(),
        contact,
        phase: ContactPhase::Moved,
        position,
    }
}

/// A 120x120 card absolutely positioned at (20, 20) in `win`, and a
/// 40x40 button, the card's one flow child — window-relative rect
/// (20, 20, 40, 40).
fn card_and_button(app: &mut App, win: Handle<Window>) -> (Handle<Leaf>, Handle<Leaf>) {
    let card = app.spawn_with(win, Leaf, (at(20.0, 20.0, 120.0, 120.0),));
    let button = app.spawn_with(
        card,
        Leaf,
        (LayoutStyle::default().size(px(40.0), px(40.0)),),
    );
    (card, button)
}

// ── Moved: Enter / Exit ─────────────────────────────────────────────────

#[test]
fn moved_onto_a_nested_node_enters_deepest_first_window_last() {
    let mut app = app();
    let win = app.spawn(app.root(), a_window());
    let (card, button) = card_and_button(&mut app, win);
    app.tick();

    app.signal(moved(win, ContactId::Mouse, Point::new(30.0, 30.0)));
    app.flush();

    assert_eq!(take_enter(), vec![button.id(), card.id(), win.id()]);
    assert!(take_exit().is_empty());
}

#[test]
fn a_second_moved_still_inside_fires_nothing_again() {
    let mut app = app();
    let win = app.spawn(app.root(), a_window());
    let (_, _) = card_and_button(&mut app, win);
    app.tick();

    app.signal(moved(win, ContactId::Mouse, Point::new(30.0, 30.0)));
    app.flush();
    take_enter();

    app.signal(moved(win, ContactId::Mouse, Point::new(35.0, 35.0)));
    app.flush();
    assert!(take_enter().is_empty());
    assert!(take_exit().is_empty());
}

#[test]
fn moving_off_every_node_exits_deepest_first_window_last() {
    let mut app = app();
    let win = app.spawn(app.root(), a_window());
    let (card, button) = card_and_button(&mut app, win);
    app.tick();

    app.signal(moved(win, ContactId::Mouse, Point::new(30.0, 30.0)));
    app.flush();
    take_enter();

    app.signal(moved(win, ContactId::Mouse, Point::new(199.0, 199.0)));
    app.flush();
    assert_eq!(take_exit(), vec![button.id(), card.id()]);
    assert!(
        take_enter().is_empty(),
        "the window is still under the point"
    );
}

#[test]
fn moving_directly_from_one_leaf_to_a_sibling_only_touches_their_own_chains() {
    let mut app = app();
    let win = app.spawn(app.root(), a_window());
    let left = app.spawn_with(win, Leaf, (at(0.0, 0.0, 50.0, 50.0),));
    let right = app.spawn_with(win, Leaf, (at(100.0, 0.0, 50.0, 50.0),));
    app.tick();

    app.signal(moved(win, ContactId::Mouse, Point::new(25.0, 25.0)));
    app.flush();
    assert_eq!(take_enter(), vec![left.id(), win.id()]);

    app.signal(moved(win, ContactId::Mouse, Point::new(125.0, 25.0)));
    app.flush();
    assert_eq!(
        take_exit(),
        vec![left.id()],
        "the window stayed in both hit-sets"
    );
    assert_eq!(take_enter(), vec![right.id()]);
}
