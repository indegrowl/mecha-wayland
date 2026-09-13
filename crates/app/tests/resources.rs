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

// ── Data ────────────────────────────────────────────────────────────────

#[test]
fn split_lends_the_tree_a_column_and_a_resource_together() {
    let mut app = App::new();
    app.register_component::<Layout>();
    app.insert_resource(Score(0));
    let a = app.spawn(app.root(), Leaf);
    let b = app.spawn(a, Leaf);
    {
        let (tree, mut data) = app.split();
        let mut layout = data.query::<&mut Layout>();
        for id in tree.descendants(tree.root()) {
            layout.get_mut(id).unwrap().0 = tree.ancestors(id).count() as u32;
        }
        drop(layout);
        data.resource_mut::<Score>().0 = tree.descendants(tree.root()).count() as u32;
        assert_eq!(data.resource::<Score>(), &Score(2));
        assert_eq!(data.component::<Layout>(b), Some(&Layout(2)));
    }
    assert_eq!(app.resource::<Score>(), &Score(2));
    assert_eq!(
        app.take_changed::<Layout>().collect::<Vec<_>>(),
        vec![a.id(), b.id()]
    );
}

// ── queries over both arenas ────────────────────────────────────────────

#[test]
fn a_query_mixes_columns_and_resources() {
    let mut app = App::new();
    app.register_component::<Layout>();
    app.insert_resource(Score(1));
    app.insert_resource(Title("t".into()));
    // Inserts flag; start clean so the query's own effect is visible.
    app.take_resource_changed::<Score>();
    app.take_resource_changed::<Title>();
    let a = app.spawn(app.root(), Leaf);
    {
        let (mut layout, score, mut title) =
            app.query::<(&mut Layout, Res<Score>, ResMut<Title>)>();
        layout.get_mut(a).unwrap().0 = score.0 + 1;
        title.0.push('!');
    }
    assert_eq!(app.component::<Layout>(a), Some(&Layout(2)));
    assert_eq!(app.resource::<Title>().0, "t!");
    assert_eq!(
        app.take_changed::<Layout>().collect::<Vec<_>>(),
        vec![a.id()]
    );
    assert!(app.take_resource_changed::<Title>());
    assert!(
        !app.take_resource_changed::<Score>(),
        "a read through Res does not flag"
    );
}

#[test]
fn a_single_resource_element_is_a_query() {
    let mut app = App::new();
    app.insert_resource(Score(4));
    assert_eq!(app.query::<Res<Score>>(), &Score(4));
    app.query::<ResMut<Score>>().0 = 5;
    assert_eq!(app.resource::<Score>(), &Score(5));
}

#[test]
fn a_resource_may_repeat_when_every_occurrence_is_shared() {
    let mut app = App::new();
    app.insert_resource(Score(4));
    let (first, second) = app.query::<(Res<Score>, Res<Score>)>();
    assert_eq!(first, second);
}

#[test]
#[should_panic(expected = "same place twice")]
fn a_resource_shared_and_mutable_panics() {
    let mut app = App::new();
    app.insert_resource(Score(0));
    let _ = app.query::<(ResMut<Score>, Res<Score>)>();
}

#[test]
#[should_panic(expected = "same place twice")]
fn a_resource_twice_mutably_panics() {
    let mut app = App::new();
    app.insert_resource(Score(0));
    let _ = app.query::<(ResMut<Score>, ResMut<Score>)>();
}

#[test]
#[should_panic(expected = "not inserted")]
fn a_query_naming_an_absent_resource_panics() {
    let mut app = App::new();
    app.insert_resource(Score(0));
    let _ = app.query::<(Res<Score>, ResMut<Never>)>();
}

#[test]
fn a_type_that_is_both_component_and_resource_is_two_places() {
    #[derive(Default, Debug, PartialEq)]
    struct Both(u32);
    impl Component for Both {}
    impl Resource for Both {}

    let mut app = App::new();
    app.register_component::<Both>();
    app.insert_resource(Both(10));
    let root = app.root();
    {
        let (mut column, mut single) = app.query::<(&mut Both, ResMut<Both>)>();
        column.get_mut(root).unwrap().0 = 1;
        single.0 = 11;
    }
    assert_eq!(app.component::<Both>(root), Some(&Both(1)));
    assert_eq!(app.resource::<Both>(), &Both(11));
}

#[test]
fn data_queries_resources_beside_the_tree() {
    let mut app = App::new();
    app.register_component::<Layout>();
    app.insert_resource(Score(3));
    let a = app.spawn(app.root(), Leaf);
    let (tree, mut data) = app.split();
    let parent = tree.parent(a).unwrap();
    let (mut layout, score) = data.query::<(&mut Layout, Res<Score>)>();
    layout.get_mut(parent).unwrap().0 = score.0;
    drop((layout, score));
    assert_eq!(data.component::<Layout>(parent), Some(&Layout(3)));
}

// ── fetch ───────────────────────────────────────────────────────────────

#[derive(Default, Debug, Clone, Copy, PartialEq)]
struct Size(u32);
impl Component for Size {}

#[test]
fn fetch_holds_several_guards_on_one_node_at_once() {
    let mut app = App::new();
    app.register_component::<Layout>()
        .register_component::<Size>();
    app.insert_resource(Score(3));
    let a = app.spawn(app.root(), Leaf);
    {
        let (mut layout, mut size, mut score) =
            app.fetch::<(&mut Layout, &mut Size, ResMut<Score>)>(a);
        layout.0 = score.0;
        size.0 = score.0 * 2;
        score.0 += 1;
    }
    assert_eq!(app.component::<Layout>(a), Some(&Layout(3)));
    assert_eq!(app.component::<Size>(a), Some(&Size(6)));
    assert_eq!(app.resource::<Score>(), &Score(4));
    assert_eq!(
        app.take_changed::<Layout>().collect::<Vec<_>>(),
        vec![a.id()]
    );
    assert_eq!(app.take_changed::<Size>().collect::<Vec<_>>(), vec![a.id()]);
}

#[test]
fn fetch_shared_elements_are_plain_references_and_do_not_flag() {
    let mut app = App::new();
    app.register_component::<Layout>();
    app.insert_resource(Score(3));
    app.take_resource_changed::<Score>();
    let a = app.spawn(app.root(), Leaf);
    let (layout, score): (&Layout, &Score) = app.fetch::<(&Layout, Res<Score>)>(a);
    assert_eq!((layout, score), (&Layout(0), &Score(3)));
    assert!(app.take_changed::<Layout>().next().is_none());
    assert!(!app.take_resource_changed::<Score>());
}

#[test]
fn a_single_element_fetch_is_the_value_itself() {
    let mut app = App::new();
    app.register_component::<Layout>();
    let a = app.spawn(app.root(), Leaf);
    app.fetch::<&mut Layout>(a).0 = 9;
    assert_eq!(app.fetch::<&Layout>(a), &Layout(9));
    assert_eq!(
        app.take_changed::<Layout>().collect::<Vec<_>>(),
        vec![a.id()]
    );
}

#[test]
#[should_panic(expected = "same place twice")]
fn fetch_with_a_repeated_mutable_place_panics() {
    let mut app = App::new();
    app.register_component::<Layout>();
    let root = app.root();
    let _ = app.fetch::<(&mut Layout, &mut Layout)>(root);
}

#[test]
#[should_panic(expected = "stale id")]
fn fetch_of_a_stale_id_panics() {
    let mut app = App::new();
    app.register_component::<Layout>();
    let a = app.spawn(app.root(), Leaf);
    app.remove(a);
    let _ = app.fetch::<&Layout>(a);
}

#[test]
fn data_fetches_beside_the_tree() {
    let mut app = App::new();
    app.register_component::<Layout>();
    app.insert_resource(Score(1));
    let a = app.spawn(app.root(), Leaf);
    let (tree, mut data) = app.split();
    let parent = tree.parent(a).unwrap();
    let (mut layout, score) = data.fetch::<(&mut Layout, Res<Score>)>(parent);
    layout.0 = score.0;
    drop((layout, score));
    assert_eq!(data.component::<Layout>(parent), Some(&Layout(1)));
}

// ── Context ─────────────────────────────────────────────────────────────

struct Poke;
impl Event for Poke {}

/// On `Poke`: reads `Score`, writes its own `Layout` and `Score` through
/// one fetch, writes `Score` again through the guard, and logs.
struct Player;
struct PlayerBuilder;
impl Build for PlayerBuilder {
    type Widget = Player;
}
impl Widget for Player {
    type Builder = PlayerBuilder;
    fn build(_b: PlayerBuilder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        s.on::<Poke>(me, |ctx, _| {
            let before = ctx.resource::<Score>().0;
            {
                let (mut layout, mut score) = ctx.fetch::<(&mut Layout, ResMut<Score>)>();
                layout.0 = score.0;
                score.0 += 1;
            }
            ctx.resource_mut::<Score>().0 += 10;
            log(format!("poke {before} -> {}", ctx.resource::<Score>().0));
        });
        Player
    }
}

#[test]
fn a_handler_reads_writes_and_fetches_resources() {
    let mut app = App::new();
    app.register_component::<Layout>();
    app.insert_resource(Score(1));
    let p = app.spawn(app.root(), PlayerBuilder);
    app.emit(Poke, p);
    app.flush();
    assert_eq!(take_log(), ["poke 1 -> 12"]);
    assert_eq!(app.component::<Layout>(p), Some(&Layout(1)));
    assert_eq!(app.resource::<Score>(), &Score(12));
}

#[test]
fn a_handler_walks_the_tree_through_the_view() {
    struct Counter;
    struct CounterBuilder;
    impl Build for CounterBuilder {
        type Widget = Counter;
    }
    impl Widget for Counter {
        type Builder = CounterBuilder;
        fn build(_b: CounterBuilder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
            s.on::<Poke>(me, |ctx, _| {
                let tree = ctx.tree();
                let n = tree.children(tree.root()).unwrap().len();
                let parent = tree.parent(ctx.handle()).unwrap();
                log(format!(
                    "{n} under root, parent is root: {}",
                    parent == tree.root()
                ));
            });
            Counter
        }
    }
    let mut app = App::new();
    let c = app.spawn(app.root(), CounterBuilder);
    app.spawn(app.root(), Leaf);
    app.emit(Poke, c);
    app.flush();
    assert_eq!(take_log(), ["2 under root, parent is root: true"]);
}

#[test]
#[should_panic(expected = "stale id")]
fn fetch_after_removing_the_owner_panics() {
    struct SelfDestruct;
    struct SelfDestructBuilder;
    impl Build for SelfDestructBuilder {
        type Widget = SelfDestruct;
    }
    impl Widget for SelfDestruct {
        type Builder = SelfDestructBuilder;
        fn build(_b: SelfDestructBuilder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
            s.on::<Poke>(me, |ctx, _| {
                let handle = ctx.handle();
                ctx.remove(handle);
                let _ = ctx.fetch::<&Layout>();
            });
            SelfDestruct
        }
    }
    let mut app = App::new();
    app.register_component::<Layout>();
    let d = app.spawn(app.root(), SelfDestructBuilder);
    app.emit(Poke, d);
    app.flush();
}

// ── Spawner ─────────────────────────────────────────────────────────────

/// Reads `Score` during its build, writes `Layout` on itself and on a
/// child through `fetch`, and bumps `Score` twice.
struct Seeded {
    child: Handle<Leaf>,
}
struct SeededBuilder;
impl Build for SeededBuilder {
    type Widget = Seeded;
}
impl Widget for Seeded {
    type Builder = SeededBuilder;
    fn build(_b: SeededBuilder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        let base = s.resource::<Score>().0;
        let child = s.spawn(me, Leaf);
        {
            let (mut layout, mut score) = s.fetch::<(&mut Layout, ResMut<Score>)>(me);
            layout.0 = base;
            score.0 += 1;
        }
        s.fetch::<&mut Layout>(child).0 = base + 100;
        s.resource_mut::<Score>().0 += 1;
        Seeded { child }
    }
}

#[test]
fn a_build_reads_writes_and_fetches_resources() {
    let mut app = App::new();
    app.register_component::<Layout>();
    app.insert_resource(Score(5));
    let s = app.spawn(app.root(), SeededBuilder);
    let child = app.widget::<Seeded>(s).unwrap().child;
    assert_eq!(app.component::<Layout>(s), Some(&Layout(5)));
    assert_eq!(app.component::<Layout>(child), Some(&Layout(105)));
    assert_eq!(app.resource::<Score>(), &Score(7));
}

#[test]
#[should_panic(expected = "not inserted")]
fn a_build_reading_an_absent_resource_panics() {
    struct Needy;
    struct NeedyBuilder;
    impl Build for NeedyBuilder {
        type Widget = Needy;
    }
    impl Widget for Needy {
        type Builder = NeedyBuilder;
        fn build(_b: NeedyBuilder, _me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
            let _ = s.resource::<Never>();
            Needy
        }
    }
    let mut app = App::new();
    app.spawn(app.root(), NeedyBuilder);
}
