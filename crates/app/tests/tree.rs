use app::prelude::*;

// ── fixtures ────────────────────────────────────────────────────────────

/// A leaf holding a string.
struct Label(String);
struct LabelBuilder(&'static str);
impl Build for LabelBuilder {
    type Widget = Label;
}
impl Widget for Label {
    type Builder = LabelBuilder;
    fn build(b: LabelBuilder, _me: Handle<Self>, _s: &mut Spawner<'_>) -> Self {
        Label(b.0.to_string())
    }
}

/// Builds two `Label`s under itself, so one spawn makes three nodes.
struct Pair {
    left: Handle<Label>,
    right: Handle<Label>,
}
struct PairBuilder;
impl Build for PairBuilder {
    type Widget = Pair;
}
impl Widget for Pair {
    type Builder = PairBuilder;
    fn build(_b: PairBuilder, me: Handle<Self>, s: &mut Spawner<'_>) -> Self {
        let left = s.spawn(me, LabelBuilder("left"));
        let right = s.spawn(me, LabelBuilder("right"));
        Pair { left, right }
    }
}

/// Never spawned by any test.
struct Ghost;
impl Build for Ghost {
    type Widget = Ghost;
}
impl Widget for Ghost {
    type Builder = Ghost;
    fn build(b: Ghost, _me: Handle<Self>, _s: &mut Spawner<'_>) -> Self {
        b
    }
}

// ── root ────────────────────────────────────────────────────────────────

#[test]
fn new_app_has_a_live_root_that_is_its_own_parent() {
    let app = App::new();
    let root = app.root();
    assert!(app.is_live(root));
    assert_eq!(app.parent(root), Some(root));
    assert_eq!(app.children(root), Some(&[][..]));
    assert_eq!(App::default().root(), root);
}

// ── spawn ───────────────────────────────────────────────────────────────

#[test]
fn spawn_appends_children_in_call_order() {
    let mut app = App::new();
    let root = app.root();
    let a = app.spawn(root, LabelBuilder("a"));
    let b = app.spawn(root, LabelBuilder("b"));
    let c = app.spawn(a, LabelBuilder("c"));
    assert_ne!(a.id(), b.id());
    assert_eq!(app.children(root), Some(&[a.id(), b.id()][..]));
    assert_eq!(app.children(a), Some(&[c.id()][..]));
    assert_eq!(app.children(c), Some(&[][..]));
    assert_eq!(app.parent(a), Some(root));
    assert_eq!(app.parent(c), Some(a.id()));
    assert!(app.is_live(a) && app.is_live(b) && app.is_live(c));
}

#[test]
fn nested_build_attaches_grandchildren_under_the_right_parent() {
    let mut app = App::new();
    let pair = app.spawn(app.root(), PairBuilder);
    assert_eq!(app.children(app.root()), Some(&[pair.id()][..]));
    let kids = app.children(pair).unwrap().to_vec();
    assert_eq!(kids.len(), 2);
    assert_eq!(app.parent(kids[0]), Some(pair.id()));
    assert_eq!(app.parent(kids[1]), Some(pair.id()));
}

#[test]
fn every_spawn_gets_a_distinct_id() {
    let mut app = App::new();
    let root = app.root();
    let mut seen = std::collections::HashSet::new();
    seen.insert(root);
    for _ in 0..10 {
        assert!(seen.insert(app.spawn(root, LabelBuilder("n")).id()));
    }
    assert_eq!(app.children(root).unwrap().len(), 10);
}

// ── widgets ─────────────────────────────────────────────────────────────

#[test]
fn widget_lookup_by_handle_and_by_id() {
    let mut app = App::new();
    let a = app.spawn(app.root(), LabelBuilder("a"));
    assert_eq!(app.widget::<Label>(a).map(|l| l.0.as_str()), Some("a"));
    app.widget_mut::<Label>(a).unwrap().0.push('!');
    assert_eq!(
        app.widget::<Label>(a.id()).map(|l| l.0.as_str()),
        Some("a!")
    );
}

#[test]
fn widget_of_another_type_is_none() {
    let mut app = App::new();
    let a = app.spawn(app.root(), LabelBuilder("a"));
    assert!(app.widget::<Pair>(a).is_none());
    assert!(app.widget_mut::<Pair>(a).is_none());
    assert!(app.widget::<Ghost>(a).is_none(), "type with no column");
}

#[test]
fn a_build_can_keep_handles_to_its_children() {
    let mut app = App::new();
    let pair = app.spawn(app.root(), PairBuilder);
    let (left, right) = {
        let p = app.widget::<Pair>(pair).unwrap();
        (p.left, p.right)
    };
    assert_eq!(app.widget::<Label>(left).unwrap().0, "left");
    assert_eq!(app.widget::<Label>(right).unwrap().0, "right");
    assert_eq!(app.children(pair), Some(&[left.id(), right.id()][..]));
}

#[test]
fn widgets_by_type_lists_every_live_one_in_spawn_order() {
    let mut app = App::new();
    let root = app.root();
    let a = app.spawn(root, LabelBuilder("a"));
    let _pair = app.spawn(root, PairBuilder);
    let b = app.spawn(root, LabelBuilder("b"));

    let labels: Vec<(NodeId, String)> = app
        .widgets::<Label>()
        .map(|(id, l)| (id, l.0.clone()))
        .collect();
    let names: Vec<&str> = labels.iter().map(|(_, s)| s.as_str()).collect();
    assert_eq!(
        names,
        ["a", "left", "right", "b"],
        "fresh slots come in spawn order"
    );
    assert_eq!(labels[0].0, a.id());
    assert_eq!(labels[3].0, b.id());

    for (_, l) in app.widgets::<Label>() {
        l.0.make_ascii_uppercase();
    }
    assert_eq!(app.widget::<Label>(a).unwrap().0, "A");
    assert_eq!(app.widgets::<Pair>().count(), 1);
    assert_eq!(app.widgets::<Ghost>().count(), 0, "never spawned");
}
