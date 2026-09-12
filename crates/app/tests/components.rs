use app::prelude::*;

// ── fixtures ────────────────────────────────────────────────────────────

#[derive(Default, Debug, Clone, Copy, PartialEq)]
struct Pos {
    x: i32,
    y: i32,
}
impl Component for Pos {}

#[derive(Default, Debug, Clone, Copy, PartialEq)]
struct Size {
    w: u32,
    h: u32,
}
impl Component for Size {}

/// Never registered by any test.
#[derive(Default, Debug, PartialEq)]
struct Never;
impl Component for Never {}

/// A widget with nothing in it.
struct Leaf;
impl Build for Leaf {
    type Widget = Leaf;
}
impl Widget for Leaf {
    type Builder = Leaf;
    fn build(b: Leaf, _me: Handle<Self>, _s: &mut Spawner<'_>) -> Self {
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
    fn build(b: Branch, me: Handle<Self>, s: &mut Spawner<'_>) -> Self {
        s.spawn(me, Leaf);
        s.spawn(me, Leaf);
        b
    }
}

/// An app with `Pos` and `Size` registered.
fn app() -> App {
    let mut app = App::new();
    app.register_component::<Pos>();
    app.register_component::<Size>();
    app
}

// ── registration and reads ──────────────────────────────────────────────

#[test]
fn register_backfills_the_root_and_live_nodes_with_default() {
    let mut app = App::new();
    let before = app.spawn(app.root(), Leaf);
    app.register_component::<Pos>();
    assert_eq!(app.component::<Pos>(app.root()), Some(&Pos::default()));
    assert_eq!(app.component::<Pos>(before), Some(&Pos::default()));
    let after = app.spawn(app.root(), Leaf);
    assert_eq!(
        app.component::<Pos>(after),
        Some(&Pos::default()),
        "spawned after registration"
    );
}

#[test]
#[should_panic(expected = "already registered")]
fn registering_a_component_twice_panics() {
    let mut app = app();
    app.register_component::<Pos>();
}

#[test]
#[should_panic(expected = "not registered")]
fn reading_an_unregistered_component_panics() {
    let app = app();
    let _ = app.component::<Never>(app.root());
}

#[test]
fn component_of_a_stale_id_is_none() {
    let mut app = app();
    let a = app.spawn(app.root(), Leaf);
    assert!(app.component::<Pos>(a).is_some());
    app.remove(a);
    assert_eq!(app.component::<Pos>(a), None);
}

// ── writes and change tracking ──────────────────────────────────────────

#[test]
fn writes_through_component_mut_are_visible_and_flagged_once() {
    let mut app = app();
    let a = app.spawn(app.root(), Leaf);
    {
        let mut pos = app.component_mut::<Pos>(a).unwrap();
        assert_eq!(pos.id(), a.id());
        pos.x = 1;
        pos.y = 2;
    }
    assert_eq!(app.component::<Pos>(a), Some(&Pos { x: 1, y: 2 }));
    // A second guard to the same node before a drain adds no second entry.
    app.component_mut::<Pos>(a).unwrap().x = 3;
    assert_eq!(app.take_changed::<Pos>().collect::<Vec<_>>(), vec![a.id()]);
    assert_eq!(app.take_changed::<Pos>().count(), 0, "drained");
}

#[test]
fn deref_does_not_flag() {
    let mut app = app();
    let a = app.spawn(app.root(), Leaf);
    let guard = app.component_mut::<Pos>(a).unwrap();
    let _read = guard.x;
    drop(guard);
    assert_eq!(app.take_changed::<Pos>().count(), 0);
}

#[test]
fn take_changed_is_in_first_write_order() {
    let mut app = app();
    let a = app.spawn(app.root(), Leaf);
    let b = app.spawn(app.root(), Leaf);
    let c = app.spawn(app.root(), Leaf);
    app.component_mut::<Pos>(c).unwrap().x = 1;
    app.component_mut::<Pos>(a).unwrap().x = 1;
    app.component_mut::<Pos>(c).unwrap().x = 2;
    let _untouched = b;
    assert_eq!(
        app.take_changed::<Pos>().collect::<Vec<_>>(),
        vec![c.id(), a.id()]
    );
}

#[test]
fn a_node_removed_before_the_drain_is_skipped() {
    let mut app = app();
    let a = app.spawn(app.root(), Leaf);
    let b = app.spawn(app.root(), Leaf);
    app.component_mut::<Pos>(a).unwrap().x = 1;
    app.component_mut::<Pos>(b).unwrap().x = 1;
    app.remove(a);
    assert_eq!(app.take_changed::<Pos>().collect::<Vec<_>>(), vec![b.id()]);
}

#[test]
fn a_reused_slot_drains_only_the_new_id_once() {
    let mut app = app();
    let old = app.spawn(app.root(), Leaf);
    app.component_mut::<Pos>(old).unwrap().x = 1;
    app.remove(old);
    // The only free slot is `old`'s, so `new` reuses it.
    let new = app.spawn(app.root(), Leaf);
    assert_eq!(
        app.component::<Pos>(new),
        Some(&Pos::default()),
        "a reused slot starts at default"
    );
    app.component_mut::<Pos>(new).unwrap().x = 2;
    assert_eq!(
        app.take_changed::<Pos>().collect::<Vec<_>>(),
        vec![new.id()]
    );
}

#[test]
fn dropping_the_drain_early_still_clears() {
    let mut app = app();
    let a = app.spawn(app.root(), Leaf);
    let b = app.spawn(app.root(), Leaf);
    app.component_mut::<Pos>(a).unwrap().x = 1;
    app.component_mut::<Pos>(b).unwrap().x = 1;
    {
        let mut drain = app.take_changed::<Pos>();
        assert_eq!(drain.next(), Some(a.id()));
        // dropped with `b` still pending
    }
    assert_eq!(app.take_changed::<Pos>().count(), 0, "nothing left over");
    // `b`'s bit is clear again, so a new write is recorded.
    app.component_mut::<Pos>(b).unwrap().x = 5;
    assert_eq!(app.take_changed::<Pos>().collect::<Vec<_>>(), vec![b.id()]);
}

#[test]
fn set_if_neq_skips_equal_values() {
    let mut app = app();
    let a = app.spawn(app.root(), Leaf);
    assert!(
        !app.component_mut::<Pos>(a)
            .unwrap()
            .set_if_neq(Pos::default())
    );
    assert_eq!(app.take_changed::<Pos>().count(), 0, "equal: not flagged");
    assert!(
        app.component_mut::<Pos>(a)
            .unwrap()
            .set_if_neq(Pos { x: 1, y: 0 })
    );
    assert_eq!(app.component::<Pos>(a), Some(&Pos { x: 1, y: 0 }));
    assert_eq!(app.take_changed::<Pos>().collect::<Vec<_>>(), vec![a.id()]);
}

#[test]
fn remove_resets_the_whole_subtree_to_default() {
    let mut app = app();
    let branch = app.spawn(app.root(), Branch);
    let kids = app.children(branch).unwrap().to_vec();
    app.component_mut::<Pos>(branch).unwrap().x = 1;
    app.component_mut::<Pos>(kids[0]).unwrap().x = 2;
    app.component_mut::<Pos>(kids[1]).unwrap().x = 3;
    app.remove(branch);
    // Three slots are free; three spawns reuse them all.
    let reused = [
        app.spawn(app.root(), Leaf),
        app.spawn(app.root(), Leaf),
        app.spawn(app.root(), Leaf),
    ];
    for id in reused {
        assert_eq!(app.component::<Pos>(id), Some(&Pos::default()));
    }
    assert_eq!(
        app.take_changed::<Pos>().count(),
        0,
        "stale ids are skipped"
    );
}

#[test]
fn component_mut_of_a_stale_id_is_none() {
    let mut app = app();
    let a = app.spawn(app.root(), Leaf);
    app.remove(a);
    assert!(app.component_mut::<Pos>(a).is_none());
}

#[test]
#[should_panic(expected = "not registered")]
fn draining_an_unregistered_type_panics() {
    let mut app = app();
    let _ = app.take_changed::<Never>().count();
}
