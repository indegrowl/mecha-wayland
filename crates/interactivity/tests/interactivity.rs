//! Pointer and touch input, end to end on an `App`. There is no
//! `presentation` yet, so every test sends `ContactInput` by hand.

use std::cell::RefCell;

use app::prelude::*;
use geometry::Point;
use interactivity::prelude::*;

thread_local! {
    static SEEN: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
}
fn log_contact_input(_: &mut App, input: &ContactInput) {
    SEEN.with(|l| {
        l.borrow_mut().push(match input.phase {
            ContactPhase::Moved => "moved",
            ContactPhase::Pressed => "pressed",
            ContactPhase::Released => "released",
            ContactPhase::Cancelled => "cancelled",
        })
    });
}

#[test]
fn contact_input_is_a_plain_signal() {
    let mut app = App::new();
    app.system(log_contact_input);
    let root = app.root();
    app.signal(ContactInput {
        window: root,
        contact: ContactId::Mouse,
        phase: ContactPhase::Moved,
        position: Point::new(1.0, 2.0),
    });
    app.flush();
    assert_eq!(SEEN.with(|l| l.borrow().clone()), vec!["moved"]);
}

#[test]
fn every_event_carries_its_contact_and_position_and_reaches_its_target() {
    struct Watcher;
    struct WatcherBuilder(std::rc::Rc<std::cell::Cell<Option<ContactId>>>);
    impl Build for WatcherBuilder {
        type Widget = Watcher;
    }
    impl Widget for Watcher {
        type Builder = WatcherBuilder;
        fn build(b: WatcherBuilder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
            let flag = b.0.clone();
            s.on::<Press>(me, move |_, e| flag.set(Some(e.contact)));
            Watcher
        }
    }

    let mut app = App::new();
    let seen = std::rc::Rc::new(std::cell::Cell::new(None));
    let watcher = app.spawn(app.root(), WatcherBuilder(seen.clone()));
    let pos = Point::new(3.0, 4.0);
    app.emit(
        Press {
            contact: ContactId::Touch(9),
            position: pos,
        },
        watcher,
    );
    app.flush();
    assert_eq!(seen.get(), Some(ContactId::Touch(9)));
}
