use std::any::Any;

/// The one arena shape: a value per slot, indexed by the slot every arena
/// in the app shares. A vacant slot holds `T::default()`.
///
/// It does not validate: `Slots` has already accepted the id. Grows on
/// demand to the arena length and never shrinks; every accessor indexes
/// directly, so the caller grows first. `Nodes` is a `Store<Node>`,
/// `Widgets` holds a `Store<Option<W>>` per widget type, and later slices
/// add `Store<C>` columns for components.
pub(crate) struct Store<T: Default> {
    slots: Vec<T>,
}

impl<T: Default> Store<T> {
    pub fn new() -> Self {
        Self { slots: Vec::new() }
    }

    /// Make every index below `len` addressable. Never shrinks.
    pub fn grow(&mut self, len: u64) {
        let len = len as usize;
        if len > self.slots.len() {
            self.slots.resize_with(len, T::default);
        }
    }

    pub fn get(&self, index: u64) -> &T {
        &self.slots[index as usize]
    }

    pub fn get_mut(&mut self, index: u64) -> &mut T {
        &mut self.slots[index as usize]
    }

    /// `None` if `index` is out of range, instead of panicking. For a
    /// caller that cannot trust the index was grown to, such as a drop
    /// impl that must not panic during unwinding.
    pub fn get_mut_checked(&mut self, index: u64) -> Option<&mut T> {
        self.slots.get_mut(index as usize)
    }

    pub fn set(&mut self, index: u64, value: T) {
        self.slots[index as usize] = value;
    }

    /// Swap the default back into the slot and return what was there.
    pub fn take(&mut self, index: u64) -> T {
        std::mem::take(&mut self.slots[index as usize])
    }

    /// Every slot with its index, in slot order, vacant ones included.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (u64, &mut T)> {
        self.slots
            .iter_mut()
            .enumerate()
            .map(|(i, v)| (i as u64, v))
    }

    /// Every slot in order, vacant ones included.
    pub fn as_slice(&self) -> &[T] {
        &self.slots
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.slots
    }
}

/// Object-safe view of a [`Store`], so a holder of many columns can grow
/// and free them without knowing their types.
pub(crate) trait AnyStore {
    /// Resize to the arena length, filling with defaults. Never shrinks.
    fn grow(&mut self, len: u64);
    /// Reset the slot to the default. Out of range is a no-op.
    fn free(&mut self, index: u64);
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: Default + 'static> AnyStore for Store<T> {
    fn grow(&mut self, len: u64) {
        Store::grow(self, len);
    }

    fn free(&mut self, index: u64) {
        if (index as usize) < self.slots.len() {
            self.take(index);
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grows_with_defaults_and_never_shrinks() {
        let mut store = Store::<u8>::new();
        store.grow(3);
        assert_eq!(*store.get(2), 0, "vacant slot holds the default");
        store.set(2, 7);
        store.grow(1);
        assert_eq!(*store.get(2), 7, "growing smaller changes nothing");
    }

    #[test]
    fn get_mut_checked_is_none_out_of_range() {
        let mut store = Store::<u8>::new();
        store.grow(2);
        *store.get_mut_checked(1).unwrap() = 9;
        assert_eq!(*store.get(1), 9);
        assert!(store.get_mut_checked(2).is_none());
    }

    #[test]
    fn set_get_take_and_iter() {
        let mut store = Store::<Option<u8>>::new();
        store.grow(4);
        store.set(1, Some(10));
        store.set(3, Some(30));
        *store.get_mut(1).as_mut().unwrap() += 1;
        let seen: Vec<(u64, Option<u8>)> = store.iter_mut().map(|(i, v)| (i, *v)).collect();
        assert_eq!(
            seen,
            vec![(0, None), (1, Some(11)), (2, None), (3, Some(30))],
            "every slot, in order"
        );
        assert_eq!(store.take(1), Some(11));
        assert_eq!(store.take(1), None, "taken slot is back to the default");
        assert!(store.get(1).is_none());
    }

    #[test]
    fn any_store_frees_and_downcasts() {
        let mut store = Store::<u8>::new();
        store.grow(2);
        store.set(1, 5);
        let erased: &mut dyn AnyStore = &mut store;
        erased.grow(3);
        erased.free(1);
        erased.free(50); // out of range is a no-op
        let back = erased.as_any().downcast_ref::<Store<u8>>().unwrap();
        assert_eq!(*back.get(1), 0, "freed slot is back to the default");
        assert_eq!(*back.get(2), 0);
        assert!(erased.as_any_mut().downcast_mut::<Store<u16>>().is_none());
    }

    #[test]
    fn slices_expose_every_slot_in_order() {
        let mut store = Store::<u8>::new();
        store.grow(3);
        store.as_mut_slice()[1] = 4;
        assert_eq!(store.as_slice(), &[0, 4, 0]);
    }
}
