//! Handlers: closures attached to `(node, event type)`, stored one typed
//! column per event type, and the dispatch that runs them.

use std::any::TypeId;
use std::collections::HashMap;
use std::ops::Deref;

use smallvec::SmallVec;

use crate::store::{AnyStore, Store};
use crate::{App, Event, Handle, NodeId};

/// The nodes an emit goes to, in order. Built from one id or handle, an
/// array, a slice or a vector; four fit inline. Derefs to a slice, which
/// is how a system reading `Emitted::targets` sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Targets(SmallVec<[NodeId; 4]>);

/// One stored handler: the user's closure wrapped so it takes the app,
/// the target the event landed on, and the event. See `Spawner::on`.
pub(crate) type Handler<E> = Box<dyn FnMut(&mut App, NodeId, &E)>;

/// One node's handlers for one event type, in registration order. Two
/// inline before the first heap allocation. What a vacant slot holds.
pub(crate) struct Handlers<E: Event>(pub(crate) SmallVec<[Handler<E>; 2]>);

/// Every handler column: one `Store<Handlers<E>>` per event type,
/// allocated on the first `on::<E>` and indexed by the shared slot.
/// Grown by `spawn`, freed by `remove`, like component columns.
pub(crate) struct HandlerColumns {
    columns: HashMap<TypeId, Box<dyn AnyStore>>,
}

impl From<NodeId> for Targets {
    fn from(id: NodeId) -> Self {
        Targets(SmallVec::from_elem(id, 1))
    }
}

impl<W> From<Handle<W>> for Targets {
    fn from(handle: Handle<W>) -> Self {
        Targets::from(handle.id())
    }
}

impl<const N: usize> From<[NodeId; N]> for Targets {
    fn from(ids: [NodeId; N]) -> Self {
        Targets(SmallVec::from_iter(ids))
    }
}

impl From<&[NodeId]> for Targets {
    fn from(ids: &[NodeId]) -> Self {
        Targets(SmallVec::from_slice(ids))
    }
}

impl From<Vec<NodeId>> for Targets {
    fn from(ids: Vec<NodeId>) -> Self {
        Targets(SmallVec::from_vec(ids))
    }
}

impl Deref for Targets {
    type Target = [NodeId];

    fn deref(&self) -> &[NodeId] {
        &self.0
    }
}

impl<E: Event> Default for Handlers<E> {
    fn default() -> Self {
        Handlers(SmallVec::new())
    }
}

impl HandlerColumns {
    pub fn new() -> Self {
        Self {
            columns: HashMap::new(),
        }
    }

    /// The column for `E`, allocated on first sight and grown to `len`.
    pub fn column_mut<E: Event>(&mut self, len: u64) -> &mut Store<Handlers<E>> {
        self.columns
            .entry(TypeId::of::<E>())
            .or_insert_with(|| {
                let mut store = Store::<Handlers<E>>::new();
                store.grow(len);
                Box::new(store)
            })
            .as_any_mut()
            .downcast_mut()
            .expect("a column holds its event type")
    }

    /// The column for `E` if any `on::<E>` ever ran.
    pub fn column_of<E: Event>(&mut self) -> Option<&mut Store<Handlers<E>>> {
        self.columns
            .get_mut(&TypeId::of::<E>())?
            .as_any_mut()
            .downcast_mut()
    }

    /// Grow every column to the arena length.
    pub fn grow(&mut self, len: u64) {
        for column in self.columns.values_mut() {
            column.grow(len);
        }
    }

    /// Empty one slot in every column, dropping the closures.
    pub fn free(&mut self, index: u64) {
        for column in self.columns.values_mut() {
            column.free(index);
        }
    }
}

/// Run every handler for `(target, E)`, for each target in order. A
/// stale target is skipped. Nothing at all happens if no `on::<E>` ever
/// ran.
///
/// A target's liveness is checked once, before its handler list is taken:
/// if a handler removes the target, the later handlers in the same list
/// still run, seeing a stale `target`. The owner check, by contrast, is
/// per handler.
pub(crate) fn dispatch<E: Event>(app: &mut App, event: &E, targets: &[NodeId]) {
    for &target in targets {
        if !app.is_live(target) {
            continue;
        }
        let taken = match app.handlers.column_of::<E>() {
            Some(column) => column.take(target.index()),
            None => return,
        };
        if taken.0.is_empty() {
            continue;
        }
        let mut restore = Restore {
            app: &mut *app,
            target,
            handlers: taken,
        };
        let Restore { app, handlers, .. } = &mut restore;
        for handler in handlers.0.iter_mut() {
            handler(app, target, event);
        }
    }
}

/// Holds a node's handler list while its handlers run and puts it back on
/// drop, unwinding included. The put-back merges handlers attached to the
/// same slot during the call after the taken ones, and does nothing if
/// the target is no longer live: removed, or removed and its slot reused
/// by a node that must not inherit the list.
///
/// Runs on the unwind path when a handler panics, so `drop` must not
/// itself be able to panic: every lookup here is non-panicking, and a
/// missing column or an out-of-range slot is treated the same as a dead
/// target, dropping the taken handlers instead of restoring them.
struct Restore<'a, E: Event> {
    app: &'a mut App,
    target: NodeId,
    handlers: Handlers<E>,
}

impl<E: Event> Drop for Restore<'_, E> {
    fn drop(&mut self) {
        if !self.app.is_live(self.target) {
            return;
        }
        let Some(column) = self.app.handlers.column_of::<E>() else {
            return;
        };
        let Some(slot) = column.get_mut_checked(self.target.index()) else {
            return;
        };
        let added = std::mem::take(slot);
        let mut list = std::mem::take(&mut self.handlers);
        list.0.extend(added.0);
        *slot = list;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Click;
    impl Event for Click {}
    struct Key;
    impl Event for Key {}

    fn noop<E: Event>() -> Handler<E> {
        Box::new(|_, _, _| {})
    }

    #[test]
    fn targets_keep_order_from_every_source_and_spill_past_four() {
        let ids: Vec<NodeId> = (0..5).map(|i| NodeId::new(0, 0, i)).collect();
        assert_eq!(&*Targets::from(ids[0]), &ids[..1]);
        assert_eq!(&*Targets::from(Handle::<u8>::new(ids[1])), &ids[1..2]);
        assert_eq!(&*Targets::from([ids[0], ids[1]]), &ids[..2]);
        assert_eq!(&*Targets::from(&ids[..3]), &ids[..3]);
        let five = Targets::from(ids.clone());
        assert_eq!(&*five, &ids[..]);
        assert!(five.0.spilled(), "five is one more than fits inline");
        assert!(!Targets::from(&ids[..4]).0.spilled());
    }

    #[test]
    fn a_column_is_allocated_on_first_sight_grown_to_len() {
        let mut h = HandlerColumns::new();
        assert!(h.column_of::<Click>().is_none());
        h.column_mut::<Click>(3).get_mut(2).0.push(noop());
        assert_eq!(h.column_of::<Click>().unwrap().get(2).0.len(), 1);
        assert!(h.column_of::<Key>().is_none(), "other types untouched");
        h.column_mut::<Click>(1).get_mut(2).0.push(noop());
        assert_eq!(
            h.column_of::<Click>().unwrap().get(2).0.len(),
            2,
            "second sight reuses the column and never shrinks"
        );
    }

    #[test]
    fn grow_and_free_reach_every_column() {
        let mut h = HandlerColumns::new();
        h.column_mut::<Click>(1);
        h.column_mut::<Key>(1);
        h.grow(4);
        h.column_mut::<Click>(4).get_mut(3).0.push(noop());
        h.column_mut::<Key>(4).get_mut(3).0.push(noop());
        h.column_mut::<Key>(4).get_mut(1).0.push(noop());
        h.free(3);
        assert!(h.column_of::<Click>().unwrap().get(3).0.is_empty());
        assert!(h.column_of::<Key>().unwrap().get(3).0.is_empty());
        assert_eq!(
            h.column_of::<Key>().unwrap().get(1).0.len(),
            1,
            "other slot untouched"
        );
        h.free(50); // out of range is a no-op
    }
}
