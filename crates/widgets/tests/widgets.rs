//! The four primitive widgets end to end: spawned, ticked, their
//! `Context` setters exercised, `OnChanged<Paint>`/`OnChanged<Layout>`
//! observed.

use std::cell::RefCell;

use app::prelude::*;
use atlas::prelude::*;
use geometry::{Color, Corners, Rect};
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

// ── Text ────────────────────────────────────────────────────────────────

const INTER: &[u8] = include_bytes!("../../atlas/tests/fixtures/Inter-Regular.ttf");

#[test]
fn a_text_paints_a_monochrome_run_sized_to_its_content() {
    let mut app = app();
    let root = root(&mut app, 200.0, 100.0);
    let font = app.resource_mut::<Atlas>().add_font(INTER).unwrap();
    let label: Handle<Text> = app.spawn(root, text(font, "hi"));
    app.tick();
    match app.component::<Paint>(label).unwrap() {
        Paint::Monochrome(sprites) => assert_eq!(sprites.len(), 2),
        other => panic!("expected a Monochrome run, got {other:?}"),
    }
    assert!(rect(&app, label.id()).width() > 0.0);
    assert!(rect(&app, label.id()).height() > 0.0);
}

#[test]
fn text_context_set_text_reshapes_and_relayouts() {
    let mut app = app();
    let root = root(&mut app, 200.0, 100.0);
    let font = app.resource_mut::<Atlas>().add_font(INTER).unwrap();
    let label: Handle<Text> = app.spawn(root, text(font, "hi"));
    app.tick();
    let before = rect(&app, label.id()).width();
    take_painted();
    take_moved();

    let controller = app.spawn(
        app.root(),
        ControllerBuilder(Box::new(move |ctx: &mut Context<'_, Controller>| {
            ctx.at(label).unwrap().set_text("a much longer label");
        })),
    );
    poke(&mut app, controller.id());
    app.tick();
    let after = rect(&app, label.id()).width();
    assert!(after > before, "a longer string is a wider box");
    assert_eq!(take_painted(), vec![vec![label.id()]]);
    assert_eq!(take_moved(), vec![vec![label.id()]]);
}

#[test]
fn text_context_set_color_retints_without_reshaping_or_relayout() {
    let mut app = app();
    let root = root(&mut app, 200.0, 100.0);
    let font = app.resource_mut::<Atlas>().add_font(INTER).unwrap();
    let label: Handle<Text> = app.spawn(root, text(font, "hi").color(Color::WHITE));
    app.tick();
    let before = rect(&app, label.id());
    take_painted();
    take_moved();

    let controller = app.spawn(
        app.root(),
        ControllerBuilder(Box::new(move |ctx: &mut Context<'_, Controller>| {
            ctx.at(label).unwrap().set_color(Color::BLACK);
        })),
    );
    poke(&mut app, controller.id());
    app.tick();
    assert_eq!(
        take_painted(),
        vec![vec![label.id()]],
        "the retint is a paint change"
    );
    assert!(
        take_moved().is_empty(),
        "no relayout: the glyphs did not move"
    );
    assert_eq!(rect(&app, label.id()), before);
    match app.component::<Paint>(label).unwrap() {
        Paint::Monochrome(sprites) => {
            assert!(sprites.iter().all(|s| s.color == Color::BLACK));
        }
        other => panic!("expected Monochrome, got {other:?}"),
    }
}

#[test]
fn empty_text_paints_an_empty_run() {
    let mut app = app();
    let root = root(&mut app, 200.0, 100.0);
    let font = app.resource_mut::<Atlas>().add_font(INTER).unwrap();
    let label: Handle<Text> = app.spawn(root, text(font, ""));
    app.tick();
    assert_eq!(
        app.component::<Paint>(label),
        Some(&Paint::Monochrome(Vec::new()))
    );
    assert_eq!(rect(&app, label.id()).width(), 0.0);
}

// ── Icon ────────────────────────────────────────────────────────────────

fn icon_sprite(app: &mut App) -> SpriteId {
    app.resource_mut::<Atlas>()
        .insert(
            Class::Icon,
            &Bitmap {
                width: 8,
                height: 8,
                format: Format::R8,
                pixels: vec![255; 64],
            },
        )
        .unwrap()
}

#[test]
fn an_icon_paints_one_tinted_sprite_at_its_master_size() {
    let mut app = app();
    let root = root(&mut app, 100.0, 100.0);
    let sprite = icon_sprite(&mut app);
    let glyph: Handle<Icon> = app.spawn(root, icon(sprite).color(Color::rgb(1.0, 0.0, 0.0)));
    app.tick();
    match app.component::<Paint>(glyph).unwrap() {
        Paint::Monochrome(sprites) => {
            assert_eq!(sprites.len(), 1);
            assert_eq!(sprites[0].color, Color::rgb(1.0, 0.0, 0.0));
            assert_eq!(sprites[0].size, geometry::Size::new(8.0, 8.0));
        }
        other => panic!("expected Monochrome, got {other:?}"),
    }
    assert_eq!(rect(&app, glyph.id()).width(), 8.0);
}

#[test]
fn icon_context_set_color_does_not_move_the_box() {
    let mut app = app();
    let root = root(&mut app, 100.0, 100.0);
    let sprite = icon_sprite(&mut app);
    let glyph: Handle<Icon> = app.spawn(root, icon(sprite));
    app.tick();
    take_painted();
    take_moved();

    let controller = app.spawn(
        app.root(),
        ControllerBuilder(Box::new(move |ctx: &mut Context<'_, Controller>| {
            ctx.at(glyph).unwrap().set_color(Color::rgb(0.0, 1.0, 0.0));
        })),
    );
    poke(&mut app, controller.id());
    app.tick();
    assert_eq!(take_painted(), vec![vec![glyph.id()]]);
    assert!(
        take_moved().is_empty(),
        "a retint alone does not move the box"
    );
}

#[test]
fn icon_context_set_size_resizes_without_touching_the_atlas() {
    let mut app = app();
    let root = root(&mut app, 100.0, 100.0);
    let sprite = icon_sprite(&mut app);
    let glyph: Handle<Icon> = app.spawn(root, icon(sprite));
    app.tick();
    take_moved();

    let controller = app.spawn(
        app.root(),
        ControllerBuilder(Box::new(move |ctx: &mut Context<'_, Controller>| {
            ctx.at(glyph)
                .unwrap()
                .set_size(geometry::Size::new(16.0, 16.0));
        })),
    );
    poke(&mut app, controller.id());
    app.tick();
    assert_eq!(rect(&app, glyph.id()).width(), 16.0);
    assert_eq!(take_moved(), vec![vec![glyph.id()]]);
}

#[test]
fn icon_context_set_sprite_swaps_the_tile_and_keeps_color() {
    let mut app = app();
    let root = root(&mut app, 100.0, 100.0);
    let sprite = icon_sprite(&mut app);
    let glyph: Handle<Icon> = app.spawn(root, icon(sprite).color(Color::rgb(1.0, 0.0, 0.0)));
    app.tick();
    let other_sprite = icon_sprite(&mut app);

    let controller = app.spawn(
        app.root(),
        ControllerBuilder(Box::new(move |ctx: &mut Context<'_, Controller>| {
            ctx.at(glyph).unwrap().set_sprite(other_sprite);
        })),
    );
    poke(&mut app, controller.id());
    app.tick();
    assert_eq!(
        rect(&app, glyph.id()).width(),
        8.0,
        "the new sprite's own master size"
    );
    match app.component::<Paint>(glyph).unwrap() {
        Paint::Monochrome(sprites) => assert_eq!(sprites[0].color, Color::rgb(1.0, 0.0, 0.0)),
        other => panic!("expected Monochrome, got {other:?}"),
    }
}

// ── Image ───────────────────────────────────────────────────────────────

fn image_sprite(app: &mut App) -> SpriteId {
    app.resource_mut::<Atlas>()
        .insert(
            Class::Image,
            &Bitmap {
                width: 4,
                height: 4,
                format: Format::Rgba8,
                pixels: vec![255; 4 * 4 * 4],
            },
        )
        .unwrap()
}

#[test]
fn an_image_paints_one_polychrome_sprite_with_the_builders_look() {
    let mut app = app();
    let root = root(&mut app, 100.0, 100.0);
    let sprite = image_sprite(&mut app);
    let photo: Handle<Image> =
        app.spawn(root, image(sprite).radius(2.0).opacity(0.5).grayscale(true));
    app.tick();
    match app.component::<Paint>(photo).unwrap() {
        Paint::Polychrome(p) => {
            assert_eq!(p.radii, Corners::all(2.0));
            assert_eq!(p.opacity, 0.5);
            assert!(p.grayscale);
        }
        other => panic!("expected Polychrome, got {other:?}"),
    }
    assert_eq!(rect(&app, photo.id()).width(), 4.0);
}

#[test]
fn image_context_set_opacity_and_set_grayscale_change_only_their_own_field() {
    let mut app = app();
    let root = root(&mut app, 100.0, 100.0);
    let sprite = image_sprite(&mut app);
    let photo: Handle<Image> = app.spawn(root, image(sprite).radius(2.0));
    app.tick();
    take_painted();
    take_moved();

    let controller = app.spawn(
        app.root(),
        ControllerBuilder(Box::new(move |ctx: &mut Context<'_, Controller>| {
            ctx.at(photo).unwrap().set_opacity(0.25);
        })),
    );
    poke(&mut app, controller.id());
    app.tick();
    match app.component::<Paint>(photo).unwrap() {
        Paint::Polychrome(p) => {
            assert_eq!(p.opacity, 0.25);
            assert_eq!(
                p.radii,
                Corners::all(2.0),
                "set_opacity did not touch radii"
            );
        }
        other => panic!("expected Polychrome, got {other:?}"),
    }
    assert_eq!(take_painted(), vec![vec![photo.id()]]);
    assert!(
        take_moved().is_empty(),
        "opacity alone does not move the box"
    );

    let controller2 = app.spawn(
        app.root(),
        ControllerBuilder(Box::new(move |ctx: &mut Context<'_, Controller>| {
            ctx.at(photo).unwrap().set_grayscale(true);
        })),
    );
    poke(&mut app, controller2.id());
    match app.component::<Paint>(photo).unwrap() {
        Paint::Polychrome(p) => {
            assert!(p.grayscale);
            assert_eq!(p.opacity, 0.25, "set_grayscale did not touch opacity");
        }
        other => panic!("expected Polychrome, got {other:?}"),
    }
}

#[test]
fn image_context_set_sprite_changes_the_box_and_keeps_the_look() {
    let mut app = app();
    let root = root(&mut app, 100.0, 100.0);
    let sprite = image_sprite(&mut app);
    let photo: Handle<Image> = app.spawn(root, image(sprite).opacity(0.5));
    app.tick();
    take_moved();

    let bigger = app
        .resource_mut::<Atlas>()
        .insert(
            Class::Image,
            &Bitmap {
                width: 8,
                height: 8,
                format: Format::Rgba8,
                pixels: vec![255; 8 * 8 * 4],
            },
        )
        .unwrap();

    let controller = app.spawn(
        app.root(),
        ControllerBuilder(Box::new(move |ctx: &mut Context<'_, Controller>| {
            ctx.at(photo).unwrap().set_sprite(bigger);
        })),
    );
    poke(&mut app, controller.id());
    app.tick();
    assert_eq!(rect(&app, photo.id()).width(), 8.0);
    match app.component::<Paint>(photo).unwrap() {
        Paint::Polychrome(p) => assert_eq!(p.opacity, 0.5, "set_sprite kept the opacity"),
        other => panic!("expected Polychrome, got {other:?}"),
    }
    assert_eq!(take_moved(), vec![vec![photo.id()]]);
}
