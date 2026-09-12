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
