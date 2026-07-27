pub mod decoration;

use crate::state::State;
use smithay::desktop::{
    PopupKeyboardGrab, PopupKind, PopupPointerGrab, PopupUngrabStrategy, Window,
    find_popup_root_surface, get_popup_toplevel_coords,
};
use smithay::input::{Seat, pointer::Focus};
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::reexports::wayland_server::protocol::wl_seat;
use smithay::utils::{Point, Serial};
use smithay::wayland::shell::xdg::{
    PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
};

impl XdgShellHandler for State {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        if let Some(output) = self.space.outputs().next() {
            if let Some(geometry) = self.space.output_geometry(output) {
                if surface.parent().is_some() {
                    surface.with_pending_state(|state| {
                        state.bounds = Some(geometry.size);
                    });
                } else {
                    self.maximize_toplevel(&surface);
                }
            }
        }

        let window = Window::new_wayland_window(surface);
        self.space.map_element(window, (0, 0), true);
    }

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        self.unconstrain_popup(&surface);
        let _ = self.popups.track_popup(PopupKind::Xdg(surface));
    }

    fn grab(&mut self, surface: PopupSurface, seat: wl_seat::WlSeat, serial: Serial) {
        let seat = Seat::from_resource(&seat).unwrap();
        let kind = PopupKind::Xdg(surface);
        let Ok(root) = find_popup_root_surface(&kind) else {
            return;
        };
        let ret = self.popups.grab_popup(root, kind, &seat, serial);

        if let Ok(mut grab) = ret {
            if let Some(keyboard) = seat.get_keyboard() {
                if keyboard.is_grabbed()
                    && !(keyboard.has_grab(serial)
                        || grab.previous_serial().is_none_or(|s| keyboard.has_grab(s)))
                {
                    grab.ungrab(PopupUngrabStrategy::All);
                    return;
                }
                keyboard.set_focus(self, grab.current_grab(), serial);
                keyboard.set_grab(self, PopupKeyboardGrab::new(&grab), serial);
            }
            if let Some(pointer) = seat.get_pointer() {
                if pointer.is_grabbed()
                    && !(pointer.has_grab(serial)
                        || grab.previous_serial().is_none_or(|s| pointer.has_grab(s)))
                {
                    grab.ungrab(PopupUngrabStrategy::All);
                    return;
                }
                pointer.set_grab(self, PopupPointerGrab::new(&grab), serial, Focus::Keep);
            }
        }
    }

    fn reposition_request(
        &mut self,
        surface: PopupSurface,
        positioner: PositionerState,
        token: u32,
    ) {
        surface.with_pending_state(|state| {
            state.geometry = positioner.get_geometry();
            state.positioner = positioner;
        });
        self.unconstrain_popup(&surface);
        surface.send_repositioned(token);
    }

    fn maximize_request(&mut self, surface: ToplevelSurface) {
        if surface.parent().is_some() {
            surface.send_configure();
            return;
        }
        self.maximize_toplevel(&surface);
        surface.send_configure();
    }
}

impl State {
    fn maximize_toplevel(&self, surface: &ToplevelSurface) {
        if let Some(output) = self.space.outputs().next() {
            if let Some(geometry) = self.space.output_geometry(output) {
                surface.with_pending_state(|state| {
                    state.states.set(xdg_toplevel::State::Maximized);
                    state.size = Some(geometry.size);
                });
            }
        }
    }

    fn unconstrain_popup(&self, popup: &PopupSurface) {
        let Ok(root) = find_popup_root_surface(&PopupKind::Xdg(popup.clone())) else {
            return;
        };
        let Some(window) = self
            .space
            .elements()
            .find(|w| w.toplevel().unwrap().wl_surface() == &root)
        else {
            return;
        };

        let Some(output) = self.space.outputs().next() else {
            return;
        };
        let Some(output_geo) = self.space.output_geometry(output) else {
            return;
        };
        let Some(window_geo) = self.space.element_geometry(window) else {
            return;
        };

        let mut target = output_geo;
        target.loc -= get_popup_toplevel_coords(&PopupKind::Xdg(popup.clone()));
        target.loc -= window_geo.loc;

        popup.with_pending_state(|state| {
            state.geometry = state.positioner.get_unconstrained_geometry(target);
        });
    }

    pub fn center_child_toplevel(&mut self, window: &Window) {
        let Some(toplevel) = window.toplevel() else {
            return;
        };
        let Some(parent_surface) = toplevel.parent() else {
            return;
        };

        let Some(parent_window) = self
            .space
            .elements()
            .find(|w| w.toplevel().unwrap().wl_surface() == &parent_surface)
        else {
            return;
        };

        let Some(parent_geo) = self.space.element_geometry(parent_window) else {
            return;
        };
        let child_geo = window.geometry();

        let new_pos = Point::from((
            parent_geo.loc.x + (parent_geo.size.w - child_geo.size.w) / 2,
            parent_geo.loc.y + (parent_geo.size.h - child_geo.size.h) / 2,
        ));
        self.space.map_element(window.clone(), new_pos, false);
    }
}
