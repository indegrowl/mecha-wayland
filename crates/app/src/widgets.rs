use std::any::TypeId;
use std::collections::HashMap;

use crate::store::{AnyStore, Store};
use crate::{Handle, Spawner};

/// A widget: the per-node value a node is spawned with.
///
/// The widget names its builder and owns the build: [`Widget::build`]
/// runs once, after the node is linked into the tree and before the
/// widget is stored. `me` is the handle the enclosing spawn returns, and
/// `s` attaches children under it.
pub trait Widget: Sized + 'static {
    type Builder: Build<Widget = Self>;
    fn build(builder: Self::Builder, me: Handle<Self>, s: &mut Spawner<'_>) -> Self;
}

/// Marker linking a builder back to its widget so `spawn(parent, builder)`
/// infers the widget type. Associated types are not injective, so without
/// it every spawn would need a turbofish.
pub trait Build: 'static {
    type Widget: Widget<Builder = Self>;
}

/// The root's widget. Private, so nothing outside the crate can look it
/// up; column 0, slot 0.
pub(crate) struct Root;

impl Build for Root {
    type Widget = Root;
}

impl Widget for Root {
    type Builder = Root;
    fn build(builder: Root, _: Handle<Root>, _: &mut Spawner<'_>) -> Root {
        builder
    }
}

/// Every widget column, plus the column numbering. One
/// `Store<Option<W>>` per widget type, all indexed by the shared slot;
/// `None` where no `W` lives.
pub(crate) struct Widgets {
    /// Type to column. Used on spawn and on iteration by type only.
    columns: HashMap<TypeId, u32>,
    /// Column to type. Used on lookup by id: one indexing and one compare.
    types: Vec<TypeId>,
    /// Column to store.
    stores: Vec<Box<dyn AnyStore>>,
}

impl Widgets {
    pub fn new() -> Self {
        Self {
            columns: HashMap::new(),
            types: Vec::new(),
            stores: Vec::new(),
        }
    }

    /// The column for `W`, allocated on first sight with a store grown to
    /// `len`.
    pub fn column<W: Widget>(&mut self, len: u64) -> u32 {
        let type_id = TypeId::of::<W>();
        if let Some(&column) = self.columns.get(&type_id) {
            return column;
        }
        let column = self.stores.len() as u32;
        let mut store = Store::<Option<W>>::new();
        store.grow(len);
        self.stores.push(Box::new(store));
        self.types.push(type_id);
        self.columns.insert(type_id, column);
        column
    }

    /// The column for `W` if one was ever allocated.
    pub fn column_of<W: Widget>(&self) -> Option<u32> {
        self.columns.get(&TypeId::of::<W>()).copied()
    }

    /// Grow every store to the arena length.
    pub fn grow(&mut self, len: u64) {
        for store in &mut self.stores {
            store.grow(len);
        }
    }

    /// Empty one slot of one column. A column that does not exist is a
    /// no-op.
    pub fn free(&mut self, column: u32, index: u64) {
        if let Some(store) = self.stores.get_mut(column as usize) {
            store.free(index);
        }
    }

    /// The store at `column` if it holds `W`. `None` for no such column or
    /// a column of another type.
    pub fn store<W: Widget>(&self, column: u32) -> Option<&Store<Option<W>>> {
        if self.types.get(column as usize) != Some(&TypeId::of::<W>()) {
            return None;
        }
        self.stores[column as usize].as_any().downcast_ref()
    }

    pub fn store_mut<W: Widget>(&mut self, column: u32) -> Option<&mut Store<Option<W>>> {
        if self.types.get(column as usize) != Some(&TypeId::of::<W>()) {
            return None;
        }
        self.stores[column as usize].as_any_mut().downcast_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Spawner;

    struct A(u8);
    struct ABuilder(u8);
    impl Build for ABuilder {
        type Widget = A;
    }
    impl Widget for A {
        type Builder = ABuilder;
        fn build(b: ABuilder, _: Handle<A>, _: &mut Spawner<'_>) -> A {
            A(b.0)
        }
    }

    struct B;
    impl Build for B {
        type Widget = B;
    }
    impl Widget for B {
        type Builder = B;
        fn build(b: B, _: Handle<B>, _: &mut Spawner<'_>) -> B {
            b
        }
    }

    #[test]
    fn columns_are_assigned_on_first_sight_and_typed() {
        let mut w = Widgets::new();
        assert_eq!(w.column_of::<A>(), None);
        assert_eq!(w.column::<A>(1), 0);
        assert_eq!(w.column::<B>(1), 1);
        assert_eq!(w.column::<A>(1), 0, "second sight reuses the column");
        assert_eq!(w.column_of::<B>(), Some(1));
        assert!(w.store::<A>(0).is_some());
        assert!(w.store_mut::<A>(0).is_some());
        assert!(w.store::<B>(0).is_none(), "wrong type for the column");
        assert!(w.store::<A>(5).is_none(), "no such column");
    }

    #[test]
    fn grow_and_free_reach_every_column() {
        let mut w = Widgets::new();
        w.column::<A>(1);
        w.column::<B>(1);
        w.grow(4);
        w.store_mut::<A>(0).unwrap().set(3, Some(A(1)));
        w.store_mut::<B>(1).unwrap().set(3, Some(B));
        assert_eq!(
            w.store::<A>(0).unwrap().get(3).as_ref().map(|a| a.0),
            Some(1)
        );
        w.free(0, 3);
        assert!(w.store::<A>(0).unwrap().get(3).is_none());
        assert!(
            w.store::<B>(1).unwrap().get(3).is_some(),
            "other column untouched"
        );
        w.free(9, 3); // no such column is a no-op
    }
}
