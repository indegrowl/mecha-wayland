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
