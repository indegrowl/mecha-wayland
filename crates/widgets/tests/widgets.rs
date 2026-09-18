//! The four primitive widgets end to end: spawned, ticked, their
//! `Context` setters exercised, `OnChanged<Paint>`/`OnChanged<Layout>`
//! observed.

use std::cell::RefCell;

use app::prelude::*;
use atlas::prelude::*;
use geometry::{Color, Rect};
use layout::prelude::*;
use paint::prelude::*;
use widgets::prelude::*;

// ── logging ─────────────────────────────────────────────────────────────

thread_local! {
    /// Every `Emitted<OnChanged<Paint>>` seen: its targets.
    static PAINTED: RefCell<Vec<Vec<NodeId>>> = const { RefCell::new(Vec::new()) };
    /// Every `Emitted<OnChanged<Layout>>` seen: its targets.
    static MOVED: RefCell<Vec<Vec<NodeId>>> = const { RefCell::new(Vec::new()) };
}

fn log_painted(_: &mut App, e: &Emitted<OnChanged<Paint>>) {
    PAINTED.with(|l| l.borrow_mut().push(e.targets.to_vec()));
}
fn log_moved(_: &mut App, e: &Emitted<OnChanged<Layout>>) {
    MOVED.with(|l| l.borrow_mut().push(e.targets.to_vec()));
}
fn take_painted() -> Vec<Vec<NodeId>> {
    PAINTED.with(|l| std::mem::take(&mut *l.borrow_mut()))
}
fn take_moved() -> Vec<Vec<NodeId>> {
    MOVED.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

// ── fixtures ────────────────────────────────────────────────────────────

/// This test file's one event: run a stored action against a widget's
/// own `Context`. `Context` cannot be built directly outside `app`, so
/// every setter call below goes through a real dispatch.
struct Poke;
impl Event for Poke {}

/// Hosts one action against another node's `Context`, reached with
/// `Context::at`. Not generic over the target widget type: the target
/// type is fixed inside each test's own closure, so one concrete
/// `Controller` covers `Div`, `Text`, `Icon` and `Image` alike.
struct Controller;
struct ControllerBuilder(Box<dyn FnMut(&mut Context<'_, Controller>)>);
impl Build for ControllerBuilder {
    type Widget = Controller;
}
impl Widget for Controller {
    type Builder = ControllerBuilder;
    fn build(b: ControllerBuilder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        let mut action = b.0;
        s.on::<Poke>(me, move |ctx, _| action(ctx));
        Controller
    }
}

fn poke(app: &mut App, id: NodeId) {
    app.emit(Poke, id);
    app.flush();
}

fn app() -> App {
    let mut app = App::new();
    app.add_module(LayoutModule)
        .add_module(PaintModule)
        .system(log_painted)
        .system(log_moved);
    app.insert_resource(Atlas::new());
    app
}

/// A `LayoutRoot` sized `width` by `height`, so a child's `auto` box
/// resolves against it.
fn root(app: &mut App, width: f32, height: f32) -> NodeId {
    app.spawn_with(
        app.root(),
        div().style(LayoutStyle::default().size(px(width), px(height))),
        (LayoutRoot(true),),
    )
    .id()
}

fn rect(app: &App, id: NodeId) -> Rect {
    app.component::<Layout>(id).unwrap().rect
}

// ── Div ─────────────────────────────────────────────────────────────────

#[test]
fn a_div_with_a_background_paints_a_quad_and_a_bare_div_paints_nothing() {
    let mut app = app();
    let root = root(&mut app, 100.0, 100.0);
    let panel = app
        .spawn(
            root,
            div()
                .style(LayoutStyle::default().size(px(40.0), px(20.0)))
                .background(Color::rgb(0.2, 0.4, 0.8))
                .radius(4.0),
        )
        .id();
    let bare = app.spawn(root, div()).id();

    app.tick();
    assert_eq!(
        app.component::<Paint>(panel),
        Some(&Paint::Quad(
            Quad::new(Color::rgb(0.2, 0.4, 0.8)).radius(4.0)
        ))
    );
    assert_eq!(app.component::<Paint>(bare), Some(&Paint::None));
}

#[test]
fn div_context_set_background_fires_on_change_only() {
    let mut app = app();
    let root = root(&mut app, 100.0, 100.0);
    let panel: Handle<Div> = app.spawn(root, div().background(Color::BLACK));
    app.tick();
    take_painted();

    let controller = app.spawn(
        app.root(),
        ControllerBuilder(Box::new(move |ctx: &mut Context<'_, Controller>| {
            ctx.at(panel).unwrap().set_background(Color::WHITE);
        })),
    );
    poke(&mut app, controller.id());
    app.tick();
    assert_eq!(
        app.component::<Paint>(panel),
        Some(&Paint::Quad(Quad::new(Color::WHITE)))
    );
    assert_eq!(take_painted(), vec![vec![panel.id()]]);

    poke(&mut app, controller.id());
    app.tick();
    assert!(
        take_painted().is_empty(),
        "an equal colour fires no OnChanged<Paint>"
    );
}

#[test]
fn div_context_set_radius_and_set_border_rewrite_their_own_field() {
    let mut app = app();
    let root = root(&mut app, 100.0, 100.0);
    let panel: Handle<Div> = app.spawn(root, div().background(Color::BLACK));

    let radius = app.spawn(
        app.root(),
        ControllerBuilder(Box::new(move |ctx: &mut Context<'_, Controller>| {
            ctx.at(panel).unwrap().set_radius(6.0);
        })),
    );
    poke(&mut app, radius.id());
    assert_eq!(
        app.component::<Paint>(panel),
        Some(&Paint::Quad(Quad::new(Color::BLACK).radius(6.0)))
    );

    let border = app.spawn(
        app.root(),
        ControllerBuilder(Box::new(move |ctx: &mut Context<'_, Controller>| {
            ctx.at(panel).unwrap().set_border(2.0, Color::WHITE);
        })),
    );
    poke(&mut app, border.id());
    assert_eq!(
        app.component::<Paint>(panel),
        Some(&Paint::Quad(
            Quad::new(Color::BLACK)
                .radius(6.0)
                .border(2.0, Color::WHITE)
        ))
    );
}
