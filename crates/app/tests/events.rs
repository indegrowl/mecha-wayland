//! Signals and systems, events and handlers, the queues, and the flush
//! order.

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

/// Spawns two `Leaf` children, so one spawn makes three nodes.
struct Branch;
impl Build for Branch {
    type Widget = Branch;
}
impl Widget for Branch {
    type Builder = Branch;
    fn build(b: Branch, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        s.spawn(me, Leaf);
        s.spawn(me, Leaf);
        b
    }
}

struct Ping;
impl Signal for Ping {}

struct Pong;
impl Signal for Pong {}

// ── systems ─────────────────────────────────────────────────────────────

#[test]
fn systems_run_in_registration_order_for_their_signal_only() {
    fn a(_: &mut App, _: &Ping) {
        log("a");
    }
    fn b(_: &mut App, _: &Ping) {
        log("b");
    }
    fn other(_: &mut App, _: &Pong) {
        log("pong");
    }

    let mut app = App::new();
    app.system(b).system(a).system(other);
    app.signal(Ping);
    assert!(take_log().is_empty(), "nothing runs before flush");
    app.flush();
    assert_eq!(take_log(), ["b", "a"]);
}

#[test]
fn the_same_system_twice_runs_twice() {
    fn a(_: &mut App, _: &Ping) {
        log("a");
    }
    let mut app = App::new();
    app.system(a).system(a);
    app.signal(Ping);
    app.flush();
    assert_eq!(take_log(), ["a", "a"]);
}

#[test]
fn a_signal_with_no_systems_is_dropped_when_sent() {
    fn a(_: &mut App, _: &Ping) {
        log("a");
    }
    let mut app = App::new();
    app.signal(Ping);
    // Registered after the send, before the flush: too late.
    app.system(a);
    app.flush();
    assert!(take_log().is_empty());
}

#[test]
fn a_system_registered_mid_pass_runs_in_that_pass() {
    fn first(app: &mut App, _: &Ping) {
        log("first");
        app.system(second);
    }
    fn second(_: &mut App, _: &Ping) {
        log("second");
    }
    let mut app = App::new();
    app.system(first);
    app.signal(Ping);
    app.flush();
    assert_eq!(take_log(), ["first", "second"]);
}

#[test]
fn a_system_can_spawn_remove_and_signal_and_it_all_runs_in_one_flush() {
    fn on_ping(app: &mut App, _: &Ping) {
        let root = app.root();
        app.spawn(root, Leaf);
        app.signal(Pong);
    }
    fn on_pong(app: &mut App, _: &Pong) {
        let ids: Vec<NodeId> = app.widgets::<Leaf>().map(|(id, _)| id).collect();
        log(format!("pong sees {}", ids.len()));
        for id in ids {
            app.remove(id);
        }
    }
    let mut app = App::new();
    app.system(on_ping).system(on_pong);
    app.signal(Ping);
    app.flush();
    assert_eq!(take_log(), ["pong sees 1"]);
    assert_eq!(app.widgets::<Leaf>().count(), 0);
}

#[test]
fn spawned_arrives_after_the_widget_is_stored_children_before_parent() {
    fn on_spawned(app: &mut App, s: &Spawned) {
        let kind = if app.widget::<Branch>(s.id).is_some() {
            "branch"
        } else if app.widget::<Leaf>(s.id).is_some() {
            "leaf"
        } else {
            "unfinished"
        };
        log(format!("{kind} under root={}", s.parent == app.root()));
    }
    let mut app = App::new();
    app.system(on_spawned);
    app.spawn(app.root(), Branch);
    app.flush();
    assert_eq!(
        take_log(),
        [
            "leaf under root=false",
            "leaf under root=false",
            "branch under root=true"
        ]
    );
}

#[test]
fn removed_arrives_once_per_remove_with_stale_ids() {
    fn on_removed(app: &mut App, r: &Removed) {
        log(format!(
            "removed live={} parent_is_root={}",
            app.is_live(r.id),
            r.parent == app.root()
        ));
    }
    let mut app = App::new();
    app.system(on_removed);
    let branch = app.spawn(app.root(), Branch);
    app.remove(branch);
    app.remove(branch); // stale: no signal
    app.flush();
    assert_eq!(take_log(), ["removed live=false parent_is_root=true"]);
}

#[test]
fn flush_on_empty_queues_is_a_noop() {
    let mut app = App::new();
    app.flush();
    app.flush();
}
