//! One toplevel and one layer surface on a live compositor, each cleared
//! to a colour, until the toplevel is closed. Run under a Wayland session:
//! `cargo run -p presentation --example clear`.

use app::prelude::*;
use atlas::Atlas;
use geometry::Color;
use gles::Budget;
use layout::prelude::*;
use paint::prelude::*;
use presentation::prelude::*;
use render::prelude::*;
use ring::prelude::*;
use wayland::prelude::*;
use window::prelude::*;

/// Spawns the toplevel under the app root and stops the app when the
/// shell asks it to close.
struct Shell;
struct ShellBuilder {
    root: NodeId,
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
                .title("clear")
                .clear(Color::rgb(0.2, 0.4, 0.8))
                .layout(LayoutStyle::default().column().size(px(400.0), px(300.0))),
        );
        s.on::<CloseRequested>(win, |ctx, _| ctx.signal(Stop));
        Shell
    }
}

fn main() {
    let mut app = App::new();
    app.add_module(LayoutModule)
        .add_module(PaintModule)
        .add_module(WindowModule)
        .add_module(RenderModule::default());
    app.insert_resource(Atlas::new());
    app.add_module(RingModule::default())
        .add_module(
            WaylandModule::new()
                .bind::<WlCompositor>()
                .bind::<ZwpLinuxDmabufV1>()
                .bind::<XdgWmBase>(),
        )
        .add_module(PresentationModule {
            app_id: "mecha.clear".into(),
            budget: Budget::default(),
        });
    let root = app.root();
    app.spawn(root, ShellBuilder { root });
    let bar = Role::Layer(LayerRole {
        layer: Layer::Top,
        anchor: Anchor::TOP | Anchor::LEFT | Anchor::RIGHT,
        exclusive_zone: 24,
        namespace: "mecha-bar".into(),
        keyboard_interactivity: KeyboardInteractivity::None,
    });
    app.spawn_with(
        root,
        window()
            .title("bar")
            .clear(Color::rgb(0.9, 0.3, 0.2))
            .layout(LayoutStyle::default().column().size(auto(), px(24.0))),
        (bar,),
    );
    app.run();
}
