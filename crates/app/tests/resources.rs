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

// ── OnChanged<R> ────────────────────────────────────────────────────────

fn on_score(app: &mut App, _: &OnChanged<Score>) {
    log(format!("score {}", app.resource::<Score>().0));
}

#[test]
fn an_insert_fires_on_the_next_tick_and_an_init_does_not() {
    let mut app = App::new();
    app.system(on_score);
    app.insert_resource(Score(3));
    app.tick();
    assert_eq!(take_log(), ["score 3"]);
    app.tick();
    assert!(take_log().is_empty());

    let mut app = App::new();
    app.system(on_score);
    app.init_resource::<Score>();
    app.tick();
    assert!(take_log().is_empty());
}

#[test]
fn a_write_fires_once_per_tick_and_a_read_does_not() {
    let mut app = App::new();
    app.init_resource::<Score>().system(on_score);
    app.resource_mut::<Score>().0 = 1;
    app.resource_mut::<Score>().0 = 2;
    app.tick();
    assert_eq!(take_log(), ["score 2"], "two writes, one signal");
    let _ = app.resource::<Score>();
    app.tick();
    assert!(take_log().is_empty());
}

#[test]
fn a_replace_fires_and_installs_no_second_drain() {
    let mut app = App::new();
    app.insert_resource(Score(1));
    app.system(on_score);
    app.tick();
    take_log();
    app.insert_resource(Score(2));
    app.tick();
    assert_eq!(take_log(), ["score 2"], "one drain, one signal");
}

#[test]
fn a_write_during_tick_fires_in_the_same_tick() {
    fn bump(app: &mut App, _: &Tick) {
        app.resource_mut::<Score>().0 += 1;
    }
    let mut app = App::new();
    app.init_resource::<Score>().system(bump).system(on_score);
    app.tick();
    assert_eq!(take_log(), ["score 1"]);
    app.tick();
    assert_eq!(take_log(), ["score 2"]);
}

#[test]
fn a_signal_with_no_system_is_dropped_and_the_drain_still_clears() {
    let mut app = App::new();
    app.insert_resource(Score(1));
    app.tick();
    assert!(!app.take_resource_changed::<Score>());
}

/// Handles `OnChanged<Layout>` on itself: logs, and writes `Score`.
struct Echo;
struct EchoBuilder;
impl Build for EchoBuilder {
    type Widget = Echo;
}
impl Widget for Echo {
    type Builder = EchoBuilder;
    fn build(_b: EchoBuilder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        s.on::<OnChanged<Layout>>(me, |ctx, _| {
            let l = ctx.component::<Layout>().unwrap().0;
            log(format!("handler layout {l}"));
            ctx.resource_mut::<Score>().0 = 10 + l;
        });
        Echo
    }
}

#[test]
fn component_change_handlers_run_before_resource_change_systems() {
    let mut app = App::new();
    app.register_component::<Layout>()
        .init_resource::<Score>()
        .system(on_score);
    let e = app.spawn(app.root(), EchoBuilder);
    app.component_mut::<Layout>(e).unwrap().0 = 1;
    app.resource_mut::<Score>().0 = 1;
    app.tick();
    // The drains ran in PostTick: Layout's emitted an event, Score's
    // queued a signal. The event ran first, and its handler's write is
    // what the signal's system then reads.
    assert_eq!(take_log(), ["handler layout 1", "score 11"]);
    app.tick();
    // The handler's write landed after Score's drain, so it fires now.
    assert_eq!(take_log(), ["score 11"]);
    app.tick();
    assert!(take_log().is_empty());
}
