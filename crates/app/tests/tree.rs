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

/* Uncommented in Task 5, which adds `Spawner::spawn`.
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
*/

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
