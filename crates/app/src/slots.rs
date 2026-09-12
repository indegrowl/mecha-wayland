use std::collections::VecDeque;

use crate::NodeId;

#[derive(Clone, Copy, Debug)]
struct Slot {
    /// Bumped every time the slot is freed. An id is live only if its
    /// generation matches the slot's *and* the slot is live.
    ///
    /// Wraps on overflow (`wrapping_add`): a stale id from 2^32 frees of
    /// this slot ago would then read as live again. Accepted deliberately;
    /// that many frees of one slot is not a case worth guarding against.
    generation: u32,
    /// The widget column of the node in this slot. Set on `alloc`, left in
    /// place on `free`. Lets any arena rebuild a full id from a slot index
    /// without consulting the widget columns.
    widget_type: u32,
    is_live: bool,
}

/// The one allocator every arena in the app shares. Hands out slot
/// indices, tracks which are live, and validates ids. Freed slots go to
/// the back of a FIFO, so a freed slot stays cold for a while before it
/// is reused.
pub(crate) struct Slots {
    slots: Vec<Slot>,
    free: VecDeque<u64>,
}

impl Slots {
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            free: VecDeque::new(),
        }
    }

    /// Claim a slot for a node of widget column `widget_type` and mark it
    /// live. Returns `(index, generation)`.
    pub fn alloc(&mut self, widget_type: u32) -> (u64, u32) {
        if let Some(index) = self.free.pop_front() {
            let slot = &mut self.slots[index as usize];
            debug_assert!(!slot.is_live);
            slot.is_live = true;
            slot.widget_type = widget_type;
            return (index, slot.generation);
        }
        self.slots.push(Slot {
            generation: 0,
            widget_type,
            is_live: true,
        });
        (self.slots.len() as u64 - 1, 0)
    }

    /// Mark the slot dead, bump its generation, and queue it for reuse.
    /// Every id that named it is stale from here on.
    pub fn free(&mut self, index: u64) {
        let slot = &mut self.slots[index as usize];
        debug_assert!(slot.is_live, "freeing a dead slot");
        slot.is_live = false;
        slot.generation = slot.generation.wrapping_add(1);
        self.free.push_back(index);
    }

    /// One indexing: the slot exists, is live, and the generations match.
    pub fn is_live(&self, id: NodeId) -> bool {
        match self.slots.get(id.index() as usize) {
            Some(slot) => slot.is_live && slot.generation == id.generation(),
            None => false,
        }
    }

    /// The full id of the node in a live slot. `None` for a dead or
    /// out-of-range slot. What a column uses to name the node at an index
    /// it is iterating.
    pub fn id(&self, index: u64) -> Option<NodeId> {
        let slot = self.slots.get(index as usize)?;
        slot.is_live
            .then(|| NodeId::new(slot.generation, slot.widget_type, index))
    }

    /// The current generation of a slot. Used to rebuild an id for a slot
    /// already known to be live.
    pub fn generation(&self, index: u64) -> u32 {
        self.slots[index as usize].generation
    }

    /// Total slot count, live or not. Every store is sized to it.
    pub fn len(&self) -> u64 {
        self.slots.len() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alloc_grows_then_reuses_freed_slots_fifo() {
        let mut s = Slots::new();
        assert_eq!(s.alloc(0), (0, 0));
        assert_eq!(s.alloc(0), (1, 0));
        assert_eq!(s.alloc(0), (2, 0));
        s.free(1);
        s.free(0);
        // Freed first is reused first, each with a bumped generation.
        assert_eq!(s.alloc(0), (1, 1));
        assert_eq!(s.alloc(0), (0, 1));
        // Free list empty again: the arena grows.
        assert_eq!(s.alloc(0), (3, 0));
        assert_eq!(s.len(), 4);
    }

    #[test]
    fn is_live_rejects_stale_generation_and_out_of_range() {
        let mut s = Slots::new();
        let (index, generation) = s.alloc(0);
        let id = NodeId::new(generation, 0, index);
        assert!(s.is_live(id));
        s.free(index);
        assert!(!s.is_live(id));
        let (again, generation2) = s.alloc(0);
        assert_eq!(again, index);
        assert!(!s.is_live(id), "old id stays stale after slot reuse");
        assert!(s.is_live(NodeId::new(generation2, 0, index)));
        assert_eq!(s.generation(index), generation2);
        assert!(!s.is_live(NodeId::new(0, 0, 99)));
    }

    #[test]
    fn id_rebuilds_a_live_slot_and_is_none_for_a_dead_one() {
        let mut s = Slots::new();
        let (index, generation) = s.alloc(7);
        assert_eq!(s.id(index), Some(NodeId::new(generation, 7, index)));
        s.free(index);
        assert_eq!(s.id(index), None, "a dead slot has no id");
        let (again, generation2) = s.alloc(3);
        assert_eq!(again, index);
        assert_eq!(
            s.id(index),
            Some(NodeId::new(generation2, 3, index)),
            "reuse: new generation and new widget type"
        );
        assert_eq!(s.id(99), None, "out of range");
    }
}
