//! Resources: insert and init, single access, the write guard, the
//! `OnChanged<R>` signal, queries and fetches over both arenas, and the
//! `Context` and `Spawner` surfaces.

use std::cell::RefCell;

use app::prelude::*;

// ── fixtures ────────────────────────────────────────────────────────────

// Systems cannot capture, so every test logs through a thread-local.
// Each test runs on its own thread, so logs never mix.
thread_local! {
    static LOG: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn log(s: impl Into<String>) {
    LOG.with(|l| l.borrow_mut().push(s.into()));
}

fn take_log() -> Vec<String> {
    LOG.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

#[derive(Default, Debug, Clone, Copy, PartialEq)]
struct Score(u32);
impl Resource for Score {}

/// Not `Default`: must be inserted by value.
#[derive(Debug, PartialEq)]
struct Title(String);
impl Resource for Title {}

/// Never inserted by any test.
#[derive(Default, Debug)]
struct Never;
impl Resource for Never {}

#[derive(Default, Debug, Clone, Copy, PartialEq)]
struct Layout(u32);
impl Component for Layout {}

/// A widget with nothing in it.
struct Leaf;
impl Build for Leaf {
    type Widget = Leaf;
}
impl Widget for Leaf {
    type Builder = Leaf;
    fn build(b: Leaf, _me: Handle<Self>, _s: &mut Spawner<'_, Self>) -> Self {
        b
    }
}

// ── insert, init, single access ─────────────────────────────────────────

#[test]
fn insert_returns_none_then_the_old_value() {
    let mut app = App::new();
    assert_eq!(app.insert_resource(Score(1)), None);
    assert_eq!(app.resource::<Score>(), &Score(1));
    assert_eq!(app.insert_resource(Score(2)), Some(Score(1)));
    assert_eq!(app.resource::<Score>(), &Score(2));
}

#[test]
fn a_non_default_resource_is_inserted_by_value() {
    let mut app = App::new();
    app.insert_resource(Title("hi".into()));
    assert_eq!(app.resource::<Title>().0, "hi");
}

#[test]
fn init_inserts_default_once_and_keeps_an_existing_value() {
    let mut app = App::new();
    assert!(!app.has_resource::<Score>());
    app.init_resource::<Score>();
    assert!(app.has_resource::<Score>());
    assert_eq!(app.resource::<Score>(), &Score(0));
    app.resource_mut::<Score>().0 = 5;
    app.init_resource::<Score>();
    assert_eq!(
        app.resource::<Score>(),
        &Score(5),
        "init is a no-op when present"
    );
    app.insert_resource(Score(7));
    app.init_resource::<Score>();
    assert_eq!(app.resource::<Score>(), &Score(7));
}

#[test]
fn init_chains_with_register_component_and_system() {
    fn noop(_: &mut App, _: &Tick) {}
    let mut app = App::new();
    app.init_resource::<Score>()
        .register_component::<Layout>()
        .system(noop);
    assert!(app.has_resource::<Score>());
}

#[test]
#[should_panic(expected = "not inserted")]
fn reading_an_absent_resource_panics() {
    let app = App::new();
    let _ = app.resource::<Never>();
}

#[test]
#[should_panic(expected = "not inserted")]
fn writing_an_absent_resource_panics() {
    let mut app = App::new();
    let _ = app.resource_mut::<Never>();
}

#[test]
fn the_guard_flags_on_the_first_deref_mut_only() {
    let mut app = App::new();
    app.init_resource::<Score>();
    assert!(!app.take_resource_changed::<Score>(), "init does not flag");
    {
        let guard = app.resource_mut::<Score>();
        let _ = guard.0;
    }
    assert!(
        !app.take_resource_changed::<Score>(),
        "a read does not flag"
    );
    {
        let mut guard = app.resource_mut::<Score>();
        guard.0 = 1;
        guard.0 = 2;
    }
    assert!(app.take_resource_changed::<Score>());
    assert!(!app.take_resource_changed::<Score>(), "take clears");
}

#[test]
fn insert_flags_and_a_replace_flags_again() {
    let mut app = App::new();
    app.insert_resource(Score(1));
    assert!(app.take_resource_changed::<Score>());
    app.insert_resource(Score(2));
    assert!(app.take_resource_changed::<Score>());
}

#[test]
fn set_if_neq_flags_only_on_a_difference() {
    let mut app = App::new();
    app.insert_resource(Score(1));
    app.take_resource_changed::<Score>();
    assert!(!app.resource_mut::<Score>().set_if_neq(Score(1)));
    assert!(!app.take_resource_changed::<Score>());
    assert!(app.resource_mut::<Score>().set_if_neq(Score(2)));
    assert!(app.take_resource_changed::<Score>());
    assert_eq!(app.resource::<Score>(), &Score(2));
}
