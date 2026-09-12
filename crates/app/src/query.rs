//! Column views, the write guard, and the `Query` that fetches views.

use std::cell::Cell;
use std::ops::{Deref, DerefMut, Index};

use crate::component::{AnyColumn, Column, Components};
use crate::slots::Slots;
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

// ── whole-column views ───────────────────────────────────────────────────

/// Shared access to one component column. Reads hand out plain `&C`;
/// nothing here flags anything.
pub struct Comps<'a, C: Component> {
    slots: &'a Slots,
    column: &'a Column<C>,
}

impl<C: Component> Comps<'_, C> {
    /// `None` for a stale id.
    pub fn get(&self, id: impl Into<NodeId>) -> Option<&C> {
        self.column.get(self.slots, id.into())
    }

    /// Live nodes only, in slot order. Slot order is not tree order.
    pub fn iter(&self) -> impl Iterator<Item = (NodeId, &C)> {
        self.column.iter(self.slots)
    }
}

/// Panics on a stale id: indexing a dead node is a caller bug.
impl<C: Component, I: Into<NodeId>> Index<I> for Comps<'_, C> {
    type Output = C;

    fn index(&self, id: I) -> &C {
        let id = id.into();
        self.get(id)
            .unwrap_or_else(|| panic!("component of a stale id: {id:?}"))
    }
}

/// Exclusive access to one component column. Writes come out as
/// [`CompMut`] guards; there is no `IndexMut`, which would hand out a bare
/// `&mut C` past the guard.
pub struct CompsMut<'a, C: Component> {
    slots: &'a Slots,
    column: &'a mut Column<C>,
}

impl<C: Component> CompsMut<'_, C> {
    /// `None` for a stale id.
    pub fn get(&self, id: impl Into<NodeId>) -> Option<&C> {
        self.column.get(self.slots, id.into())
    }

    /// `None` for a stale id.
    pub fn get_mut(&mut self, id: impl Into<NodeId>) -> Option<CompMut<'_, C>> {
        self.column.get_mut(self.slots, id.into())
    }

    /// Live nodes only, in slot order.
    pub fn iter(&self) -> impl Iterator<Item = (NodeId, &C)> {
        self.column.iter(self.slots)
    }

    /// Live nodes only, in slot order, as write guards that may coexist.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (NodeId, CompMut<'_, C>)> {
        self.column.iter_mut(self.slots)
    }
}

/// Panics on a stale id: indexing a dead node is a caller bug.
impl<C: Component, I: Into<NodeId>> Index<I> for CompsMut<'_, C> {
    type Output = C;

    fn index(&self, id: I) -> &C {
        let id = id.into();
        self.get(id)
            .unwrap_or_else(|| panic!("component of a stale id: {id:?}"))
    }
}

// ── the component side of the app ────────────────────────────────────────

/// Every component access without the tree: what a [`Query`] fetches
/// from, and the second half of [`App::split`](crate::App::split).
pub struct Columns<'a> {
    slots: &'a Slots,
    components: &'a mut Components,
}

impl<'a> Columns<'a> {
    pub(crate) fn new(slots: &'a Slots, components: &'a mut Components) -> Self {
        Self { slots, components }
    }

    /// Fetch the views `Q` names. See [`App::components`](crate::App::components).
    pub fn components<Q: Query>(&mut self) -> Q::Out<'_> {
        Q::fetch(Columns {
            slots: self.slots,
            components: &mut *self.components,
        })
    }
}

// ── queries ──────────────────────────────────────────────────────────────

mod sealed {
    pub trait Sealed {}
}

/// What [`App::components`](crate::App::components) fetches: `&C` for a
/// [`Comps`], `&mut C` for a [`CompsMut`], or a tuple of one to six of
/// those for a tuple of views. Sealed; the impls here are the whole set.
///
/// Aliasing follows Rust's rule at column granularity: one type may
/// appear twice only if both occurrences are `&C`. Anything else panics,
/// since the type system cannot see that `A` is `A`.
pub trait Query: sealed::Sealed {
    type Out<'a>;

    #[doc(hidden)]
    fn fetch<'a>(columns: Columns<'a>) -> Self::Out<'a>;
}

/// One column of a query. Only the tuple impls of [`Query`] use it.
#[doc(hidden)]
pub trait Element: sealed::Sealed {
    const MUTABLE: bool;
    type Out<'a>;
    fn column(columns: &Columns<'_>) -> u32;
    fn view<'a>(fetched: Fetched<'a>) -> Self::Out<'a>;
}

/// One column, fetched shared or exclusive, with the slots to validate
/// ids against. The hand-off from `fetch_columns` to [`Element::view`].
#[doc(hidden)]
pub struct Fetched<'a> {
    slots: &'a Slots,
    column: Col<'a>,
}

enum Col<'a> {
    Shared(&'a dyn AnyColumn),
    Exclusive(&'a mut dyn AnyColumn),
}

impl<C: Component> sealed::Sealed for &C {}
impl<C: Component> Element for &C {
    const MUTABLE: bool = false;
    type Out<'a> = Comps<'a, C>;

    fn column(columns: &Columns<'_>) -> u32 {
        columns.components.column_of::<C>()
    }

    fn view<'a>(fetched: Fetched<'a>) -> Comps<'a, C> {
        let erased: &'a dyn AnyColumn = match fetched.column {
            Col::Shared(shared) => shared,
            Col::Exclusive(exclusive) => &*exclusive,
        };
        let column = erased
            .as_any()
            .downcast_ref::<Column<C>>()
            .expect("a column holds its registered type");
        Comps {
            slots: fetched.slots,
            column,
        }
    }
}

impl<C: Component> sealed::Sealed for &mut C {}
impl<C: Component> Element for &mut C {
    const MUTABLE: bool = true;
    type Out<'a> = CompsMut<'a, C>;

    fn column(columns: &Columns<'_>) -> u32 {
        columns.components.column_of::<C>()
    }

    fn view<'a>(fetched: Fetched<'a>) -> CompsMut<'a, C> {
        let Col::Exclusive(erased) = fetched.column else {
            unreachable!("a `&mut` element is always fetched exclusively")
        };
        let column = erased
            .as_any_mut()
            .downcast_mut::<Column<C>>()
            .expect("a column holds its registered type");
        CompsMut {
            slots: fetched.slots,
            column,
        }
    }
}

impl<C: Component> Query for &C {
    type Out<'a> = Comps<'a, C>;

    fn fetch<'a>(columns: Columns<'a>) -> Comps<'a, C> {
        <(Self,) as Query>::fetch(columns).0
    }
}

impl<C: Component> Query for &mut C {
    type Out<'a> = CompsMut<'a, C>;

    fn fetch<'a>(columns: Columns<'a>) -> CompsMut<'a, C> {
        <(Self,) as Query>::fetch(columns).0
    }
}

/// Borrow `N` columns at once, shared or exclusive as `want` says, with
/// no aliasing: a column wanted exclusively appears once; a column wanted
/// shared may appear any number of times.
///
/// Sorts the elements by column and splits the store slice front to back,
/// so every borrow is a disjoint sub-slice and no unsafe is needed.
///
/// # Panics
///
/// If one column appears twice and at least one of them is exclusive.
fn fetch_columns<'a, const N: usize>(
    slots: &'a Slots,
    stores: &'a mut [Box<dyn AnyColumn>],
    want: [(u32, bool); N],
) -> [Fetched<'a>; N] {
    for i in 0..N {
        for j in (i + 1)..N {
            assert!(
                want[i].0 != want[j].0 || !(want[i].1 || want[j].1),
                "a query names the same component twice, at least once with `&mut`"
            );
        }
    }

    let mut order: [usize; N] = std::array::from_fn(|i| i);
    order.sort_unstable_by_key(|&i| want[i].0);

    let mut out: [Option<Fetched<'a>>; N] = [const { None }; N];
    let mut rest: &'a mut [Box<dyn AnyColumn>] = stores;
    let mut consumed = 0usize; // columns already split off the front
    let mut k = 0;
    while k < N {
        let column = want[order[k]].0 as usize;
        let (_, tail) = std::mem::take(&mut rest).split_at_mut(column - consumed);
        let (this, tail) = tail
            .split_first_mut()
            .expect("a registered column is in range");
        rest = tail;
        consumed = column + 1;

        // Elements on this column are contiguous in `order`.
        let mut end = k + 1;
        while end < N && want[order[end]].0 as usize == column {
            end += 1;
        }

        if want[order[k]].1 {
            debug_assert_eq!(end, k + 1, "an exclusive column has one element");
            out[order[k]] = Some(Fetched {
                slots,
                column: Col::Exclusive(&mut **this),
            });
        } else {
            let shared: &'a dyn AnyColumn = &**this;
            for e in k..end {
                out[order[e]] = Some(Fetched {
                    slots,
                    column: Col::Shared(shared),
                });
            }
        }
        k = end;
    }
    out.map(|fetched| fetched.expect("every element was fetched"))
}

/// `Query` for a tuple of `Element`s: fetch every column in one go, then
/// build each view.
macro_rules! tuple_query {
    ($($T:ident),+) => {
        impl<$($T: Element),+> sealed::Sealed for ($($T,)+) {}

        impl<$($T: Element),+> Query for ($($T,)+) {
            type Out<'a> = ($($T::Out<'a>,)+);

            #[allow(non_snake_case)]
            fn fetch<'a>(columns: Columns<'a>) -> Self::Out<'a> {
                let want = [$(($T::column(&columns), $T::MUTABLE)),+];
                let Columns { slots, components } = columns;
                let [$($T,)+] = fetch_columns(slots, components.stores_mut(), want);
                ($($T::view($T),)+)
            }
        }
    };
}

tuple_query!(A);
tuple_query!(A, B);
tuple_query!(A, B, C);
tuple_query!(A, B, C, D);
tuple_query!(A, B, C, D, E);
tuple_query!(A, B, C, D, E, F);
