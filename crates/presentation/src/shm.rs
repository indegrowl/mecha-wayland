//! Two `wl_shm` buffers per window over one unlinked file, and the fill.
//! Written in Task 8.

pub(crate) struct Buffers;

impl Buffers {
    pub(crate) fn pixel(&self, _slot: usize, _x: u32, _y: u32) -> Option<u32> {
        None
    }
}
