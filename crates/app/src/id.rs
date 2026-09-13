use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

/// Identifies one node in an [`App`](crate::App) tree.
///
/// Opaque: outside this crate a `NodeId` is a token to hand back to the
/// app. Inside, it carries three things:
///
/// | field         | meaning                                              |
/// |---------------|------------------------------------------------------|
/// | `index`       | the slot shared by every arena in the app            |
/// | `generation`  | the slot's generation when this id was handed out    |
/// | `widget_type` | the column of the widget's type, so a typed lookup   |
/// |               | needs no hash                                        |
///
/// An id is live while its slot is occupied and the generations match.
/// After [`App::remove`](crate::App::remove) every id in the removed
/// subtree is stale and resolves to nothing, even once the slot is reused.
/// Only `App` constructs ids, so a live id is never forged.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct NodeId {
    generation: u32,
    widget_type: u32,
    index: u64,
}

impl NodeId {
    /// The root: slot 0, generation 0, column 0. Never freed, so its
    /// generation never moves.
    pub(crate) const ROOT: NodeId = NodeId::new(0, 0, 0);

    pub(crate) const fn new(generation: u32, widget_type: u32, index: u64) -> Self {
        Self {
            generation,
            widget_type,
            index,
        }
    }

    #[inline]
    pub(crate) const fn generation(self) -> u32 {
        self.generation
    }

    #[inline]
    pub(crate) const fn widget_type(self) -> u32 {
        self.widget_type
    }

    /// The slot this id names: the arena index shared by every column in
    /// the app. Reused once the node is removed, so it keys a side table
    /// only for ids known to be live; a stale id's slot may already be
    /// someone else's.
    #[inline]
    pub const fn slot(self) -> u64 {
        self.index
    }
}

/// A [`NodeId`] that remembers which widget type lives at the node.
///
/// Returned by [`App::spawn`](crate::App::spawn) and
/// [`Spawner::spawn`](crate::Spawner::spawn). `Copy` regardless of `W`.
/// Converts into a `NodeId` with [`Handle::id`] or `.into()`, so anything
/// that takes a `NodeId` takes a handle.
pub struct Handle<W> {
    id: NodeId,
    _widget: PhantomData<fn() -> W>,
}

impl<W> Handle<W> {
    pub(crate) const fn new(id: NodeId) -> Self {
        Self {
            id,
            _widget: PhantomData,
        }
    }

    #[inline]
    pub const fn id(self) -> NodeId {
        self.id
    }
}

impl<W> From<Handle<W>> for NodeId {
    fn from(h: Handle<W>) -> NodeId {
        h.id
    }
}

// Manual impls: derives would add `W: Copy` etc. bounds, but the phantom
// carries no `W` value.
impl<W> Clone for Handle<W> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<W> Copy for Handle<W> {}
impl<W> PartialEq for Handle<W> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl<W> Eq for Handle<W> {}
impl<W> Hash for Handle<W> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state)
    }
}
impl<W> fmt::Debug for Handle<W> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Handle<{}>({:?})", std::any::type_name::<W>(), self.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_round_trip_through_the_accessors() {
        let id = NodeId::new(1, 2, 3);
        assert_eq!(id.generation(), 1);
        assert_eq!(id.widget_type(), 2);
        assert_eq!(id.slot(), 3);
        assert_eq!(id, NodeId::new(1, 2, 3));
        assert_ne!(id, NodeId::new(0, 2, 3));
    }

    #[test]
    fn root_id_is_slot_zero_generation_zero_column_zero() {
        assert_eq!(NodeId::ROOT, NodeId::new(0, 0, 0));
    }

    #[test]
    fn handle_is_copy_for_a_non_copy_widget_and_converts() {
        struct NotCopy(#[allow(dead_code)] String);
        let id = NodeId::new(1, 2, 3);
        let h: Handle<NotCopy> = Handle::new(id);
        let h2 = h;
        assert_eq!(h, h2);
        let back: NodeId = h.into();
        assert_eq!(back, h.id());
        assert_eq!(back, id);
    }
}
