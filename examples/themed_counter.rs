//! A counter that demonstrates theme-aware colors and runtime theme switching.
//! Run under a Wayland session: `cargo run --example themed_counter`.

use mecha_wayland::prelude::*;

struct Button;

fn button(font: FontId, label: impl Into<String>) -> ButtonBuilder {
    ButtonBuilder {
        font,
        label: label.into(),
    }
}

struct ButtonBuilder {
    font: FontId,
    label: String,
}

impl Build for ButtonBuilder {
    type Widget = Button;
}

impl Widget for Button {
    type Builder = ButtonBuilder;
    fn build(b: ButtonBuilder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        *s.component_mut::<LayoutStyle>(me).unwrap() =
            LayoutStyle::default().center().padding_all(px(12.0));
        *s.component_mut::<Paint>(me).unwrap() =
            Paint::Quad(Quad::new(s.color(ColorRole::PrimaryContainer)).radius(6.0));
        let label = s.spawn(
            me,
            text(b.font, b.label)
                .color(s.color(ColorRole::OnPrimaryContainer))
                .size(18),
        );

        s.on_theme(me, move |ctx| {
            let bg = ctx.color(ColorRole::PrimaryContainer);
            let fg = ctx.color(ColorRole::OnPrimaryContainer);
            ctx.set_paint(Paint::Quad(Quad::new(bg).radius(6.0)));
            ctx.at(label).unwrap().set_color(fg);
        });

        Button
    }
}

struct Counter {
    count: i32,
}

fn counter(font: FontId) -> CounterBuilder {
    CounterBuilder { font }
}

struct CounterBuilder {
    font: FontId,
}

impl Build for CounterBuilder {
    type Widget = Counter;
}

impl Widget for Counter {
    type Builder = CounterBuilder;
    fn build(b: CounterBuilder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        *s.component_mut::<LayoutStyle>(me).unwrap() =
            LayoutStyle::default().column().center().fill().gap(px(12.0));
        *s.component_mut::<Paint>(me).unwrap() =
            Paint::Quad(Quad::new(s.color(ColorRole::Surface)));

        let label = s.spawn(
            me,
            text(b.font, "0")
                .color(s.color(ColorRole::OnSurface))
                .size(24),
        );
        let row = s.spawn(
            me,
            div().style(LayoutStyle::default().row().center().gap(px(8.0))),
        );
        let minus = s.spawn(row, button(b.font, "-"));
        let plus = s.spawn(row, button(b.font, "+"));
        let toggle = s.spawn(me, button(b.font, "Toggle Theme"));

        s.on::<Clicked>(minus, move |ctx, _| {
            ctx.me().count -= 1;
            let val = ctx.me().count;
            ctx.at(label).unwrap().set_text(val.to_string());
        });

        s.on::<Clicked>(plus, move |ctx, _| {
            ctx.me().count += 1;
            let val = ctx.me().count;
            ctx.at(label).unwrap().set_text(val.to_string());
        });

        s.on::<Clicked>(toggle, move |ctx, _| {
            let next = match ctx.theme().mode() {
                ThemeMode::Dark => MechanixTheme::light(),
                ThemeMode::Light => MechanixTheme::dark(),
            };
            ctx.set_theme(next);
        });

        s.on_theme(me, move |ctx| {
            let bg = ctx.color(ColorRole::Surface);
            let fg = ctx.color(ColorRole::OnSurface);
            ctx.set_paint(Paint::Quad(Quad::new(bg)));
            ctx.at(label).unwrap().set_color(fg);
        });

        Counter { count: 0 }
    }
}

struct Shell;
struct ShellBuilder {
    root: NodeId,
    font: FontId,
}
impl Build for ShellBuilder {
    type Widget = Shell;
}
impl Widget for Shell {
    type Builder = ShellBuilder;
    fn build(b: ShellBuilder, _me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        let win = s.spawn(
            b.root,
            window()
                .title("themed counter")
                .layout(LayoutStyle::default().center().size(px(240.0), px(180.0))),
        );
        s.on::<CloseRequested>(win, |ctx, _| ctx.signal(Stop));
        s.spawn(win, counter(b.font));
        Shell
    }
}

fn main() {
    let mut app = App::new();
    app.add_module(LayoutModule)
        .add_module(PaintModule)
        .add_module(WindowModule)
        .add_module(InteractivityModule)
        .add_module(RenderModule::default())
        .insert_resource(Atlas::new());

    app.add_module(MechanixTheme::dark());

    app.add_module(RingModule::default())
        .add_module(
            WaylandModule::new()
                .bind::<WlCompositor>()
                .bind::<ZwpLinuxDmabufV1>()
                .bind::<XdgWmBase>()
                .bind::<WlSeat>(),
        )
        .add_module(PresentationModule {
            app_id: "mecha.themed_counter".into(),
            budget: Budget::default(),
        });

    let font = app
        .resource_mut::<Atlas>()
        .add_font(include_bytes!(
            "../crates/atlas/tests/fixtures/Inter-Regular.ttf"
        ))
        .expect("Inter loads");

    let root = app.root();
    app.spawn(root, ShellBuilder { root, font });
    app.run();
}
