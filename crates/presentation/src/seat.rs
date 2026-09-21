//! `wl_seat`'s pointer and touch, reduced to `interactivity::ContactInput`.
//! `Seat` is presentation's own state for this: the bound `wl_seat`, the
//! `wl_pointer`/`wl_touch` objects once requested, and (from later in
//! this file) the small bit of focus/position bookkeeping a reducer
//! needs that `interactivity::Contacts` cannot supply in time — see the
//! design doc's "Why presentation tracks position itself".

use app::prelude::*;
use wayland::prelude::*;

pub(crate) struct Seat {
    seat: WlSeat,
    pointer: Option<WlPointer>,
    touch: Option<WlTouch>,
}
impl Resource for Seat {}

impl Seat {
    pub(crate) fn new(seat: WlSeat) -> Seat {
        Seat {
            seat,
            pointer: None,
            touch: None,
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
