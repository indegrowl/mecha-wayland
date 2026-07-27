use crate::state::State;
use smithay::backend::renderer::utils::on_commit_buffer_handler;
use smithay::desktop::{PopupKind, WindowSurfaceType, layer_map_for_output};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::compositor::{
    CompositorHandler, CompositorState, get_parent, is_sync_subsurface, with_states,
};
use smithay::wayland::shell::wlr_layer::LayerSurfaceData;

impl CompositorHandler for State {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(
        &self,
        client: &'a smithay::reexports::wayland_server::Client,
    ) -> &'a smithay::wayland::compositor::CompositorClientState {
        &client
            .get_data::<crate::state::ClientState>()
            .unwrap()
            .compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler::<Self>(surface);

        let mut child_to_center: Option<smithay::desktop::Window> = None;

        if !is_sync_subsurface(surface) {
            let mut root = surface.clone();
            while let Some(parent) = get_parent(&root) {
                root = parent;
            }
            if let Some(window) = self
                .space
                .elements()
                .find(|w| w.toplevel().unwrap().wl_surface() == &root)
                .cloned()
            {
                window.on_commit();

                if !window.toplevel().unwrap().is_initial_configure_sent() {
                    window.toplevel().unwrap().send_configure();
                }

                if window.toplevel().unwrap().parent().is_some() {
                    child_to_center = Some(window);
                }
            }
        }

        self.popups.commit(surface);

        if let Some(popup) = self.popups.find_popup(surface) {
            if let PopupKind::Xdg(ref xdg) = popup {
                if !xdg.is_initial_configure_sent() {
                    xdg.send_configure().expect("initial configure failed");
                }
            }
        }

        ensure_layer_initial_configure(surface, &mut self.space);

        if let Some(window) = child_to_center {
            self.center_child_toplevel(&window);
        }
    }
}

fn ensure_layer_initial_configure(
    surface: &WlSurface,
    space: &mut smithay::desktop::Space<smithay::desktop::Window>,
) {
    let Some(output) = space.outputs().find(|o| {
        let map = layer_map_for_output(o);
        map.layer_for_surface(surface, WindowSurfaceType::TOPLEVEL)
            .is_some()
    }) else {
        return;
    };

    let initial_configure_sent = with_states(surface, |states| {
        states
            .data_map
            .get::<LayerSurfaceData>()
            .unwrap()
            .lock()
            .unwrap()
            .initial_configure_sent
    });

    if initial_configure_sent {
        return;
    }

    let mut map = layer_map_for_output(output);
    map.arrange();

    let layer = map
        .layer_for_surface(surface, WindowSurfaceType::TOPLEVEL)
        .unwrap();
    layer.layer_surface().send_configure();
}
