use crate::state::State;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::shell::wlr_layer::{
    Layer, LayerSurface, LayerSurfaceConfigure, WlrLayerShellHandler, WlrLayerShellState,
};

impl WlrLayerShellHandler for State {
    fn shell_state(&mut self) -> &mut WlrLayerShellState {
        &mut self.layer_shell_state
    }

    fn new_layer_surface(
        &mut self,
        surface: LayerSurface,
        output: Option<WlOutput>,
        layer: Layer,
        namespace: String,
    ) {
        println!(
            "[wlr_layer_shell] New layer surface created! Layer: {:?}, Namespace: '{}'",
            layer, namespace
        );
        let _ = (surface, output);
    }

    fn layer_destroyed(&mut self, surface: LayerSurface) {
        println!("[wlr_layer_shell] Layer surface destroyed");
        let _ = surface;
    }

    fn ack_configure(&mut self, surface: WlSurface, configure: LayerSurfaceConfigure) {
        let _ = (surface, configure);
    }
}
