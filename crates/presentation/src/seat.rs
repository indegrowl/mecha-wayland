//! `wl_seat`'s pointer and touch, reduced to `interactivity::ContactInput`.
//! `Seat` is presentation's own state for this: the bound `wl_seat`, the
//! `wl_pointer`/`wl_touch` objects once requested, and (from later in
//! this file) the small bit of focus/position bookkeeping a reducer
//! needs that `interactivity::Contacts` cannot supply in time — see the
//! design doc's "Why presentation tracks position itself".

use std::collections::HashMap;

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
    /// Every touch point currently down: `wl_touch`'s id to the window
    /// it started on and its last known position.
    touches: HashMap<i32, (NodeId, Point)>,
}
impl Resource for Seat {}

impl Seat {
    pub(crate) fn new(seat: WlSeat) -> Seat {
        Seat {
            seat,
            pointer: None,
            touch: None,
            pointer_focus: None,
            touches: HashMap::new(),
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

/// `wl_touch` reduced to `ContactInput`. A `Down` is two signals in
/// order — `Moved` then `Pressed`, both at the same position —
/// `interactivity` depends on exactly that ordering (see
/// `ContactPhase::Moved`'s doc). `Cancel` carries no `id` at all: the
/// protocol says it "applies to all touch points currently active on
/// this client", so every tracked touch is cancelled at once.
pub(crate) fn on_touch(app: &mut App, e: &WlTouchEvent) {
    match e {
        WlTouchEvent::Down {
            surface, id, x, y, ..
        } => {
            let Some(window) = app.resource::<Surfaces>().window_of(surface.id()) else {
                return;
            };
            let position = Point::new(*x, *y);
            app.resource_mut::<Seat>()
                .touches
                .insert(*id, (window, position));
            app.signal(ContactInput {
                window,
                contact: ContactId::Touch(*id as u32),
                phase: ContactPhase::Moved,
                position,
            });
            app.signal(ContactInput {
                window,
                contact: ContactId::Touch(*id as u32),
                phase: ContactPhase::Pressed,
                position,
            });
        }
        WlTouchEvent::Motion { id, x, y, .. } => {
            let Some((window, _)) = app.resource::<Seat>().touches.get(id).copied() else {
                return;
            };
            let position = Point::new(*x, *y);
            app.resource_mut::<Seat>()
                .touches
                .insert(*id, (window, position));
            app.signal(ContactInput {
                window,
                contact: ContactId::Touch(*id as u32),
                phase: ContactPhase::Moved,
                position,
            });
        }
        WlTouchEvent::Up { id, .. } => {
            let Some((window, position)) = app.resource_mut::<Seat>().touches.remove(id) else {
                return;
            };
            app.signal(ContactInput {
                window,
                contact: ContactId::Touch(*id as u32),
                phase: ContactPhase::Released,
                position,
            });
        }
        WlTouchEvent::Cancel { .. } => {
            let touches = std::mem::take(&mut app.resource_mut::<Seat>().touches);
            for (id, (window, position)) in touches {
                app.signal(ContactInput {
                    window,
                    contact: ContactId::Touch(id as u32),
                    phase: ContactPhase::Cancelled,
                    position,
                });
            }
        }
        _ => {}
    }
}
