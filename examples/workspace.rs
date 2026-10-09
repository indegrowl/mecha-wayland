use layout::Available::Definite;
use mecha_wayland::prelude::*;

const BAR_HEIGHT: f32 = 32.0;

// builder is to set the components styles for widget type
#[derive(Default)]
struct Workspaces {
    workspaces: Vec<Workspace>,
}

struct Workspace {
    name: String,
    active: bool,
    handle: ExtWorkspaceHandleV1,
}

impl Resource for Workspaces {}

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
        let bar = Role::Layer(LayerRole {
            layer: Layer::Top,
            anchor: Anchor::TOP | Anchor::LEFT | Anchor::RIGHT,
            exclusive_zone: BAR_HEIGHT as i32,
            namespace: "bar".into(),
            keyboard_interactivity: KeyboardInteractivity::None,
        });
        let win = s.spawn_with(
            b.root,
            window()
                .title("counter")
                .layout(LayoutStyle::default().center().size(auto(), px(BAR_HEIGHT))),
            (bar,),
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
        .add_module(InteractivityModule)
        .add_module(RenderModule::default())
        .insert_resource(Atlas::new());

    app.add_module(RingModule::default())
        .add_module(
            WaylandModule::new()
                .bind::<WlCompositor>()
                .bind::<ExtWorkspaceManagerV1>()
                .bind::<ZwpLinuxDmabufV1>()
                .bind::<XdgWmBase>()
                .bind::<WlSeat>(),
        )
        .add_module(PresentationModule {
            app_id: "workspaces".into(),
            budget: Budget::default(),
        });

    //app.system(on_workspace_manager);
    //app.system(on_workspace_group);
    app.system(on_workspace_handle);
    app.init_resource::<Workspaces>();

    let root = app.root();
    app.spawn(root, ShellBuilder { root });
    app.run();
}

fn on_workspace_manager(app: &mut App, e: &ExtWorkspaceManagerV1Event) {
    println!("{:?}", e);
    match e {
        Workspace => {
            //println!("{:?}", Workspace);
            //app.resource_mut::<Workspaces>()
        }
        _ => {}
    }
}

fn on_workspace_group(app: &mut App, e: &ExtWorkspaceGroupHandleV1Event) {
    println!("{:?}", e);
}
fn on_workspace_handle(app: &mut App, e: &ExtWorkspaceHandleV1Event) {
    let mut ws = app.resource_mut::<Workspaces>();
    match e {
        ExtWorkspaceHandleV1Event::Name {
            workspace_handle,
            name,
        } => {
            ws.workspaces.push(Workspace {
                name: *name,
            });
        }
        _ => {}
    }
    println!("{:?}", e);
}
