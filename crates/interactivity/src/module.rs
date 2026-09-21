//! The one signal in, driving `Contacts` and emitting the five events.
//!
//! Registered as four systems on the same `ContactInput`, each filtering
//! on its own phase, rather than one function with a match over all four:
//! `Moved` is independent hover tracking; `Pressed`, `Released` and
//! `Cancelled` are one capture lifecycle. Splitting them this way let
//! each half land as its own reviewable task.

use app::{App, Module, NodeId};

use crate::contact::{ContactInput, ContactPhase};
use crate::contacts::{Contacts, HitSet};
use crate::events::{Enter, Exit};
use crate::hit_test::hit_test;

pub struct InteractivityModule;

impl Module for InteractivityModule {
    fn install(self, app: &mut App) {
        app.init_resource::<Contacts>().system(on_moved);
    }
}

fn diff(old: &[NodeId], new: &[NodeId]) -> (HitSet, HitSet) {
    let entered = new.iter().copied().filter(|id| !old.contains(id)).collect();
    let exited = old.iter().copied().filter(|id| !new.contains(id)).collect();
    (entered, exited)
}

fn on_moved(app: &mut App, input: &ContactInput) {
    if input.phase != ContactPhase::Moved {
        return;
    }
    let hit = hit_test(app, input.window, input.position);
    let (entered, exited) = {
        let mut contacts = app.resource_mut::<Contacts>();
        let state = contacts.entry(input.contact, input.window);
        let (entered, exited) = diff(&state.hit, &hit);
        state.hit = hit;
        state.position = input.position;
        (entered, exited)
    };
    if !exited.is_empty() {
        app.emit(
            Exit {
                contact: input.contact,
                position: input.position,
            },
            &exited[..],
        );
    }
    if !entered.is_empty() {
        app.emit(
            Enter {
                contact: input.contact,
                position: input.position,
            },
            &entered[..],
        );
    }
}
