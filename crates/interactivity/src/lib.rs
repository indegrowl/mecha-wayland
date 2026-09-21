#![forbid(unsafe_code)]
//! Pointer and touch input in, per-node events out.
//!
//! `interactivity` knows no compositor and no protocol: `presentation`
//! (or a test, until `presentation` exists) reduces `wl_pointer` and
//! `wl_touch` to one signal, [`ContactInput`], and this crate turns that
//! into [`Press`], [`Release`], [`Enter`], [`Exit`] and [`Clicked`] at the
//! nodes under the contact's position.

mod contact;
mod contacts;
mod events;
mod hit_test;
mod module;

pub use contact::{ContactId, ContactInput, ContactPhase};
pub use contacts::Contacts;
pub use events::{Clicked, Enter, Exit, Press, Release};
pub use module::InteractivityModule;

pub mod prelude {
    pub use crate::{
        Clicked, ContactId, ContactInput, ContactPhase, Contacts, Enter, Exit, InteractivityModule,
        Press, Release,
    };
}
