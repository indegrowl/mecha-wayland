use std::any::{Any, TypeId};
use std::collections::HashMap;

use crate::{App, Signal};

/// A system: a plain function run for every signal of type `S`, in the
/// order systems were registered. No captures, never removed.
pub type System<S> = fn(&mut App, &S);

/// Every system, by signal type: one `Vec<System<S>>` behind `Any` per
/// type. One hash and one downcast per signal; then a contiguous run of
/// fn pointers.
pub(crate) struct Systems {
    map: HashMap<TypeId, Box<dyn Any>>,
}

impl Systems {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Append `system` to the run for `S`, starting the run on first
    /// sight. The same fn twice runs twice.
    pub fn add<S: Signal>(&mut self, system: System<S>) {
        self.map
            .entry(TypeId::of::<S>())
            .or_insert_with(|| Box::new(Vec::<System<S>>::new()))
            .downcast_mut::<Vec<System<S>>>()
            .expect("a run holds its signal type")
            .push(system);
    }

    /// Whether any system is registered for `S`. A run is never empty,
    /// so this is one hash.
    pub fn has<S: Signal>(&self) -> bool {
        self.map.contains_key(&TypeId::of::<S>())
    }

    /// The `i`th system for `S`; `None` past the end, or when no run
    /// exists. By index rather than by iterator so a run can grow while
    /// it is being walked: a system that registers another for the same
    /// signal sees it run in the same pass.
    pub fn get<S: Signal>(&self, i: usize) -> Option<System<S>> {
        self.map
            .get(&TypeId::of::<S>())?
            .downcast_ref::<Vec<System<S>>>()?
            .get(i)
            .copied()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    thread_local! {
        static LOG: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
    }
    fn log(s: &'static str) {
        LOG.with(|l| l.borrow_mut().push(s));
    }
    fn take_log() -> Vec<&'static str> {
        LOG.with(|l| std::mem::take(&mut *l.borrow_mut()))
    }

    struct Ping;
    impl Signal for Ping {}
    struct Pong;
    impl Signal for Pong {}
    struct Never;
    impl Signal for Never {}

    fn a(_: &mut App, _: &Ping) {
        log("a");
    }
    fn b(_: &mut App, _: &Ping) {
        log("b");
    }
    fn pong(_: &mut App, _: &Pong) {
        log("pong");
    }

    #[test]
    fn runs_are_per_signal_type_in_registration_order() {
        let mut app = App::new();
        let mut s = Systems::new();
        assert!(!s.has::<Ping>());
        s.add(b);
        s.add(a);
        s.add(pong);
        assert!(s.has::<Ping>());
        assert!(s.has::<Pong>());
        assert!(!s.has::<Never>());

        let mut i = 0;
        while let Some(system) = s.get::<Ping>(i) {
            system(&mut app, &Ping);
            i += 1;
        }
        assert_eq!(take_log(), ["b", "a"]);

        s.get::<Pong>(0).unwrap()(&mut app, &Pong);
        assert_eq!(take_log(), ["pong"]);
        assert!(s.get::<Pong>(1).is_none(), "past the end");
        assert!(s.get::<Never>(0).is_none(), "no run at all");
    }
}
