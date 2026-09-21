#![forbid(unsafe_code)]
//! Pointer and touch input in, per-node events out.
//!
//! `interactivity` knows no compositor and no protocol: `presentation`
//! (or a test, until `presentation` exists) reduces `wl_pointer` and
//! `wl_touch` to one signal, [`ContactInput`], and this crate turns that
//! into [`Press`], [`Release`], [`Enter`], [`Exit`] and [`Clicked`] at the
//! nodes under the contact's position.
//!
//! # Quick start
//!
//! ```
//! use std::cell::Cell;
//! use std::rc::Rc;
//!
//! use app::prelude::*;
//! use geometry::Point;
//! use layout::prelude::*;
//! use window::prelude::*;
//! use interactivity::prelude::*;
//!
//! struct Button {
//!     clicked: Rc<Cell<bool>>,
//! }
//! struct ButtonBuilder;
//! impl Build for ButtonBuilder {
//!     type Widget = Button;
//! }
//! impl Widget for Button {
//!     type Builder = ButtonBuilder;
//!     fn build(_: ButtonBuilder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
//!         let clicked = Rc::new(Cell::new(false));
//!         let flag = clicked.clone();
//!         s.on::<Clicked>(me, move |_, _| flag.set(true));
//!         Button { clicked }
//!     }
//! }
//!
//! let mut app = App::new();
//! app.add_module(LayoutModule)
//!     .add_module(WindowModule)
//!     .add_module(InteractivityModule);
//! let win = app.spawn(app.root(), window());
//! let button = app.spawn_with(
//!     win,
//!     ButtonBuilder,
//!     (LayoutStyle::default().size(px(40.0), px(40.0)),),
//! );
//! app.tick();
//!
//! let pos = Point::ZERO; // the button sits at the window's content origin
//! app.signal(ContactInput { window: win.id(), contact: ContactId::Mouse, phase: ContactPhase::Pressed, position: pos });
//! app.signal(ContactInput { window: win.id(), contact: ContactId::Mouse, phase: ContactPhase::Released, position: pos });
//! app.flush();
//!
//! assert!(app.widget::<Button>(button).unwrap().clicked.get());
//! ```

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
