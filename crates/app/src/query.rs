//! Column views and the write guard. Task 6 adds the `Query` trait and
//! the whole-column views here.

use std::cell::Cell;
use std::ops::{Deref, DerefMut};

use crate::{Component, NodeId};

/// Exclusive access to one node's `C`, and the only way to a `&mut C`.
///
/// Derefs both ways. The first `DerefMut` records the node as changed:
/// one bit and one push. Every later `DerefMut` on the same guard is a
/// load and a branch; a hot loop hoists it with `let v = &mut *guard;`.
///
/// Assigning an equal value through `DerefMut` still counts as a change.
/// Use [`CompMut::set_if_neq`] when that matters.
pub struct CompMut<'a, C> {
    id: NodeId,
    value: &'a mut C,
    bit: &'a mut bool,
    list: &'a Cell<Vec<NodeId>>,
}

impl<'a, C: Component> CompMut<'a, C> {
    pub(crate) fn new(
        id: NodeId,
        value: &'a mut C,
        bit: &'a mut bool,
        list: &'a Cell<Vec<NodeId>>,
    ) -> Self {
        Self {
            id,
            value,
            bit,
            list,
        }
    }

    /// The node this guard belongs to.
    pub fn id(&self) -> NodeId {
        self.id
    }

    /// Write `value` only if it differs from what is there, flagging the
    /// node only then. Returns whether it wrote.
    pub fn set_if_neq(&mut self, value: C) -> bool
    where
        C: PartialEq,
    {
        if *self.value == value {
            return false;
        }
        *self.value = value;
        self.flag();
        true
    }

    /// Bit first, then the push, so a set bit always has its entry.
    fn flag(&mut self) {
        if !*self.bit {
            *self.bit = true;
            let mut list = self.list.take();
            list.push(self.id);
            self.list.set(list);
        }
    }
}

impl<C: Component> Deref for CompMut<'_, C> {
    type Target = C;

    fn deref(&self) -> &C {
        self.value
    }
}

impl<C: Component> DerefMut for CompMut<'_, C> {
    fn deref_mut(&mut self) -> &mut C {
        self.flag();
        self.value
    }
}
