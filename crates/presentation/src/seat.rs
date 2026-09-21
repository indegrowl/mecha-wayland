//! `wl_seat`'s pointer and touch, reduced to `interactivity::ContactInput`.
//! `Seat` is presentation's own state for this: the bound `wl_seat`, the
//! `wl_pointer`/`wl_touch` objects once requested, and (from later in
//! this file) the small bit of focus/position bookkeeping a reducer
//! needs that `interactivity::Contacts` cannot supply in time — see the
//! design doc's "Why presentation tracks position itself".

use app::prelude::*;
use geometry::Point;
use interactivity::{ContactId, ContactInput, ContactPhase};
use wayland::prelude::*;

use crate::Surfaces;

pub(crate) struct Seat {
    seat: WlSeat,
    pointer: Option<WlPointer>,
    touch: Option<WlTouch>,
    /// The window the pointer last entered, and its position there.
    /// `None` between a `Leave` and the next `Enter`.
    pointer_focus: Option<(NodeId, Point)>,
}
impl Resource for Seat {}

impl Seat {
    pub(crate) fn new(seat: WlSeat) -> Seat {
        Seat {
            seat,
            pointer: None,
            touch: None,
            pointer_focus: None,
        }
    }
}

/// The seat's capabilities changed (or were reported for the first
/// time): request a pointer/touch object the first time each bit is
/// seen. Never released — capability loss at runtime is out of scope,
/// per the design doc.
pub(crate) fn on_seat(app: &mut App, e: &WlSeatEvent) {
    let WlSeatEvent::Capabilities { capabilities, .. } = e else {
        return;
    };
    let (mut seat, mut wl) = app.query::<(ResMut<Seat>, ResMut<Wayland>)>();
    let s = seat.seat;
    if capabilities.contains(WlSeatCapability::POINTER) && seat.pointer.is_none() {
        seat.pointer = Some(s.get_pointer(&mut wl));
    }
    if capabilities.contains(WlSeatCapability::TOUCH) && seat.touch.is_none() {
        seat.touch = Some(s.get_touch(&mut wl));
    }
}

/// `wl_pointer` reduced to `ContactInput`. `Button` and `Leave` carry no
/// position of their own — the protocol's own words for `Button` are
/// "the location of the click is given by the last motion or enter
/// event", which is exactly `pointer_focus`, always fresh by
/// construction since every `Motion`/`Enter` updates it first.
pub(crate) fn on_pointer(app: &mut App, e: &WlPointerEvent) {
    match e {
        WlPointerEvent::Enter {
            surface,
            surface_x,
            surface_y,
            ..
        } => {
            let Some(window) = app.resource::<Surfaces>().window_of(surface.id()) else {
                return;
            };
            let position = Point::new(*surface_x, *surface_y);
            app.resource_mut::<Seat>().pointer_focus = Some((window, position));
            app.signal(ContactInput {
                window,
                contact: ContactId::Mouse,
                phase: ContactPhase::Moved,
                position,
            });
        }
        WlPointerEvent::Motion {
            surface_x,
            surface_y,
            ..
        } => {
            let Some((window, _)) = app.resource::<Seat>().pointer_focus else {
                return;
            };
            let position = Point::new(*surface_x, *surface_y);
            app.resource_mut::<Seat>().pointer_focus = Some((window, position));
            app.signal(ContactInput {
                window,
                contact: ContactId::Mouse,
                phase: ContactPhase::Moved,
                position,
            });
        }
        WlPointerEvent::Button { state, .. } => {
            let Some((window, position)) = app.resource::<Seat>().pointer_focus else {
                return;
            };
            let phase = match state {
                WlPointerButtonState::Pressed => ContactPhase::Pressed,
                WlPointerButtonState::Released => ContactPhase::Released,
            };
            app.signal(ContactInput {
                window,
                contact: ContactId::Mouse,
                phase,
                position,
            });
        }
        WlPointerEvent::Leave { .. } => {
            let Some((window, position)) = app.resource_mut::<Seat>().pointer_focus.take() else {
                return;
            };
            app.signal(ContactInput {
                window,
                contact: ContactId::Mouse,
                phase: ContactPhase::Cancelled,
                position,
            });
        }
        _ => {}
    }
}
