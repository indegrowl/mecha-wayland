//! Column views, the write guard, and the `Query` that fetches views.

use std::any::{Any, TypeId};
use std::cell::Cell;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut, Index};

use crate::component::{AnyColumn, Column, Components};
use crate::resource::{Entry, ResourceMut, Resources};
use crate::slots::Slots;
use crate::{Component, NodeId, Resource};

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

    /// Push first, then the bit, so a set bit always has its entry.
    /// Nothing between the two can unwind.
    fn flag(&mut self) {
        if !*self.bit {
            let mut list = self.list.take();
            list.push(self.id);
            self.list.set(list);
            *self.bit = true;
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

// ── the data side of the app ─────────────────────────────────────────────

/// Every component and resource access without the tree: what a
/// [`Query`] fetches from, and the second half of
/// [`App::split`](crate::App::split).
pub struct Data<'a> {
    slots: &'a Slots,
    components: &'a mut Components,
    resources: &'a mut Resources,
}

impl<'a> Data<'a> {
    pub(crate) fn new(
        slots: &'a Slots,
        components: &'a mut Components,
        resources: &'a mut Resources,
    ) -> Self {
        Self {
            slots,
            components,
            resources,
        }
    }

    /// Fetch the views `Q` names. See [`App::query`](crate::App::query).
    pub fn query<Q: Query>(&mut self) -> Q::Out<'_> {
        Q::fetch(Data {
            slots: self.slots,
            components: &mut *self.components,
            resources: &mut *self.resources,
        })
    }

    /// The values and guards `Q` names, at one node. See
    /// [`App::fetch`](crate::App::fetch).
    pub fn fetch<Q: Query>(&mut self, id: impl Into<NodeId>) -> Q::One<'_> {
        Q::fetch_one(
            Data {
                slots: self.slots,
                components: &mut *self.components,
                resources: &mut *self.resources,
            },
            id.into(),
        )
    }

    /// One node's `C`. `None` for a stale id. Panics if `C` is not
    /// registered.
    pub fn component<C: Component>(&self, id: impl Into<NodeId>) -> Option<&C> {
        self.components.column::<C>().get(self.slots, id.into())
    }

    /// A write guard for one node's `C`. `None` for a stale id. Panics if
    /// `C` is not registered.
    pub fn component_mut<C: Component>(&mut self, id: impl Into<NodeId>) -> Option<CompMut<'_, C>> {
        self.components
            .column_mut::<C>()
            .get_mut(self.slots, id.into())
    }

    /// The app's `R`. See [`App::resource`](crate::App::resource). Panics
    /// if `R` is not inserted.
    pub fn resource<R: Resource>(&self) -> &R {
        self.resources.get::<R>()
    }

    /// A write guard for the app's `R`. See
    /// [`App::resource_mut`](crate::App::resource_mut). Panics if `R` is
    /// not inserted.
    pub fn resource_mut<R: Resource>(&mut self) -> ResourceMut<'_, R> {
        self.resources.get_mut::<R>()
    }
}

// ── queries ──────────────────────────────────────────────────────────────

mod sealed {
    pub trait Sealed {}
}

/// Names a resource inside a query, shared: yields `&R`. A lifetime-free
/// marker so it fits a turbofish. Plain `&R` cannot be an element: a
/// blanket impl for resources would overlap the one for components,
/// since a type may be both.
pub struct Res<R: Resource>(PhantomData<fn() -> R>);

/// Names a resource inside a query, exclusive: yields a [`ResourceMut`].
pub struct ResMut<R: Resource>(PhantomData<fn() -> R>);

/// What [`App::query`](crate::App::query) fetches: `&C` for a
/// [`Comps`], `&mut C` for a [`CompsMut`], [`Res<R>`] for a `&R`,
/// [`ResMut<R>`] for a [`ResourceMut`], or a tuple of one to six of those
/// for a tuple of views. Sealed; the impls here are the whole set.
///
/// Aliasing follows Rust's rule per place, a column or a resource: one
/// place may appear twice only if every occurrence is shared. Anything
/// else panics, since the type system cannot see that `A` is `A`. A type
/// that is both a component and a resource is two places.
pub trait Query: sealed::Sealed {
    /// The views [`App::query`](crate::App::query) returns.
    type Out<'a>;
    /// The values and guards [`App::fetch`](crate::App::fetch) returns
    /// for one node: `&C`, [`CompMut<C>`], `&R`, [`ResourceMut<R>`].
    type One<'a>;

    #[doc(hidden)]
    fn fetch<'a>(data: Data<'a>) -> Self::Out<'a>;

    #[doc(hidden)]
    fn fetch_one<'a>(data: Data<'a>, id: NodeId) -> Self::One<'a>;
}

/// Where an element's data lives: a component column by number, or a
/// resource entry by key.
#[doc(hidden)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Place {
    Column(u32),
    Resource(TypeId),
}

/// One element of a query. Only the tuple impls of [`Query`] use it.
#[doc(hidden)]
pub trait Element: sealed::Sealed {
    const MUTABLE: bool;
    type Out<'a>;
    /// Panics if the type is not registered or inserted.
    fn place(data: &Data<'_>) -> Place;
    fn view<'a>(fetched: Fetched<'a>) -> Self::Out<'a>;

    type One<'a>;
    /// `id` was checked live by the caller.
    fn one<'a>(fetched: Fetched<'a>, id: NodeId) -> Self::One<'a>;
}

/// A shared or exclusive borrow of one erased column or entry.
enum Borrowed<'a, T: ?Sized> {
    Shared(&'a T),
    Exclusive(&'a mut T),
}

impl<'a, T: ?Sized> Borrowed<'a, T> {
    fn shared(self) -> &'a T {
        match self {
            Borrowed::Shared(shared) => shared,
            Borrowed::Exclusive(exclusive) => exclusive,
        }
    }
}

/// One element's data, fetched. The hand-off from `fetch_places` to
/// [`Element::view`]. A struct around a private enum, so the crate's
/// private types stay out of the public interface.
#[doc(hidden)]
pub struct Fetched<'a> {
    inner: Inner<'a>,
}

/// A column comes with the slots its view validates ids against; a
/// resource has no ids to validate.
enum Inner<'a> {
    Column {
        slots: &'a Slots,
        col: Borrowed<'a, dyn AnyColumn + 'a>,
    },
    Resource(Borrowed<'a, dyn Any>),
}

impl<C: Component> sealed::Sealed for &C {}
impl<C: Component> Element for &C {
    const MUTABLE: bool = false;
    type Out<'a> = Comps<'a, C>;

    fn place(data: &Data<'_>) -> Place {
        Place::Column(data.components.column_of::<C>())
    }

    fn view<'a>(fetched: Fetched<'a>) -> Comps<'a, C> {
        let Inner::Column { slots, col } = fetched.inner else {
            unreachable!("a component element is fetched from a column")
        };
        let column = col
            .shared()
            .as_any()
            .downcast_ref::<Column<C>>()
            .expect("a column holds its registered type");
        Comps { slots, column }
    }

    type One<'a> = &'a C;

    fn one<'a>(fetched: Fetched<'a>, id: NodeId) -> &'a C {
        let Inner::Column { slots, col } = fetched.inner else {
            unreachable!("a component element is fetched from a column")
        };
        let column = col
            .shared()
            .as_any()
            .downcast_ref::<Column<C>>()
            .expect("a column holds its registered type");
        column.get(slots, id).expect("fetch checked the id is live")
    }
}

impl<C: Component> sealed::Sealed for &mut C {}
impl<C: Component> Element for &mut C {
    const MUTABLE: bool = true;
    type Out<'a> = CompsMut<'a, C>;

    fn place(data: &Data<'_>) -> Place {
        Place::Column(data.components.column_of::<C>())
    }

    fn view<'a>(fetched: Fetched<'a>) -> CompsMut<'a, C> {
        let Inner::Column {
            slots,
            col: Borrowed::Exclusive(erased),
        } = fetched.inner
        else {
            unreachable!("a `&mut` element is always fetched exclusively from a column")
        };
        let column = erased
            .as_any_mut()
            .downcast_mut::<Column<C>>()
            .expect("a column holds its registered type");
        CompsMut { slots, column }
    }

    type One<'a> = CompMut<'a, C>;

    fn one<'a>(fetched: Fetched<'a>, id: NodeId) -> CompMut<'a, C> {
        let Inner::Column {
            slots,
            col: Borrowed::Exclusive(erased),
        } = fetched.inner
        else {
            unreachable!("a `&mut` element is always fetched exclusively from a column")
        };
        let column = erased
            .as_any_mut()
            .downcast_mut::<Column<C>>()
            .expect("a column holds its registered type");
        column
            .get_mut(slots, id)
            .expect("fetch checked the id is live")
    }
}

impl<R: Resource> sealed::Sealed for Res<R> {}
impl<R: Resource> Element for Res<R> {
    const MUTABLE: bool = false;
    type Out<'a> = &'a R;

    fn place(data: &Data<'_>) -> Place {
        Place::Resource(data.resources.type_of::<R>())
    }

    fn view<'a>(fetched: Fetched<'a>) -> &'a R {
        let Inner::Resource(entry) = fetched.inner else {
            unreachable!("a resource element is fetched from an entry")
        };
        &entry
            .shared()
            .downcast_ref::<Entry<R>>()
            .expect("an entry holds its type")
            .value
    }

    type One<'a> = &'a R;

    fn one<'a>(fetched: Fetched<'a>, _id: NodeId) -> &'a R {
        Self::view(fetched)
    }
}

impl<R: Resource> sealed::Sealed for ResMut<R> {}
impl<R: Resource> Element for ResMut<R> {
    const MUTABLE: bool = true;
    type Out<'a> = ResourceMut<'a, R>;

    fn place(data: &Data<'_>) -> Place {
        Place::Resource(data.resources.type_of::<R>())
    }

    fn view<'a>(fetched: Fetched<'a>) -> ResourceMut<'a, R> {
        let Inner::Resource(Borrowed::Exclusive(erased)) = fetched.inner else {
            unreachable!("a `ResMut` element is always fetched exclusively from an entry")
        };
        ResourceMut::new(
            erased
                .downcast_mut::<Entry<R>>()
                .expect("an entry holds its type"),
        )
    }

    type One<'a> = ResourceMut<'a, R>;

    fn one<'a>(fetched: Fetched<'a>, _id: NodeId) -> ResourceMut<'a, R> {
        Self::view(fetched)
    }
}

impl<C: Component> Query for &C {
    type Out<'a> = Comps<'a, C>;
    type One<'a> = &'a C;

    fn fetch<'a>(data: Data<'a>) -> Comps<'a, C> {
        <(Self,) as Query>::fetch(data).0
    }

    fn fetch_one<'a>(data: Data<'a>, id: NodeId) -> &'a C {
        <(Self,) as Query>::fetch_one(data, id).0
    }
}

impl<C: Component> Query for &mut C {
    type Out<'a> = CompsMut<'a, C>;
    type One<'a> = CompMut<'a, C>;

    fn fetch<'a>(data: Data<'a>) -> CompsMut<'a, C> {
        <(Self,) as Query>::fetch(data).0
    }

    fn fetch_one<'a>(data: Data<'a>, id: NodeId) -> CompMut<'a, C> {
        <(Self,) as Query>::fetch_one(data, id).0
    }
}

impl<R: Resource> Query for Res<R> {
    type Out<'a> = &'a R;
    type One<'a> = &'a R;

    fn fetch<'a>(data: Data<'a>) -> &'a R {
        <(Self,) as Query>::fetch(data).0
    }

    fn fetch_one<'a>(data: Data<'a>, id: NodeId) -> &'a R {
        <(Self,) as Query>::fetch_one(data, id).0
    }
}

impl<R: Resource> Query for ResMut<R> {
    type Out<'a> = ResourceMut<'a, R>;
    type One<'a> = ResourceMut<'a, R>;

    fn fetch<'a>(data: Data<'a>) -> ResourceMut<'a, R> {
        <(Self,) as Query>::fetch(data).0
    }

    fn fetch_one<'a>(data: Data<'a>, id: NodeId) -> ResourceMut<'a, R> {
        <(Self,) as Query>::fetch_one(data, id).0
    }
}

/// Borrow `N` places at once, shared or exclusive as `want` says, with
/// no aliasing: a place wanted exclusively appears once; a place wanted
/// shared may appear any number of times.
///
/// Columns: sort the column elements by column and split the store
/// slice front to back, so every borrow is a disjoint sub-slice.
/// Resources: one pass over the map, handing each entry to the one
/// exclusive element that wants it, or sharing it among the elements
/// that do. A few `TypeId` compares per resource; no unsafe either way.
///
/// # Panics
///
/// If one place appears twice and at least one of them is exclusive.
fn fetch_places<'a, const N: usize>(
    slots: &'a Slots,
    stores: &'a mut [Box<dyn AnyColumn>],
    resources: &'a mut Resources,
    want: [(Place, bool); N],
) -> [Fetched<'a>; N] {
    for i in 0..N {
        for j in (i + 1)..N {
            assert!(
                want[i].0 != want[j].0 || !(want[i].1 || want[j].1),
                "a query or fetch names the same place twice, at least once mutably"
            );
        }
    }

    let mut out: [Option<Fetched<'a>>; N] = [const { None }; N];

    // Column elements first, by column; resource elements after them.
    let mut order: [usize; N] = std::array::from_fn(|i| i);
    order.sort_unstable_by_key(|&i| match want[i].0 {
        Place::Column(column) => (0, column),
        Place::Resource(_) => (1, 0),
    });
    let columns = order
        .iter()
        .take_while(|&&i| matches!(want[i].0, Place::Column(_)))
        .count();

    let mut rest: &'a mut [Box<dyn AnyColumn>] = stores;
    let mut consumed = 0usize; // columns already split off the front
    let mut k = 0;
    while k < columns {
        let Place::Column(column) = want[order[k]].0 else {
            unreachable!("column elements sort first")
        };
        let (_, tail) = std::mem::take(&mut rest).split_at_mut(column as usize - consumed);
        let (this, tail) = tail
            .split_first_mut()
            .expect("a registered column is in range");
        rest = tail;
        consumed = column as usize + 1;

        // Elements on this column are contiguous in `order`.
        let mut end = k + 1;
        while end < columns && want[order[end]].0 == Place::Column(column) {
            end += 1;
        }

        if want[order[k]].1 {
            debug_assert_eq!(end, k + 1, "an exclusive column has one element");
            out[order[k]] = Some(Fetched {
                inner: Inner::Column {
                    slots,
                    col: Borrowed::Exclusive(&mut **this),
                },
            });
        } else {
            let shared: &'a dyn AnyColumn = &**this;
            for e in k..end {
                out[order[e]] = Some(Fetched {
                    inner: Inner::Column {
                        slots,
                        col: Borrowed::Shared(shared),
                    },
                });
            }
        }
        k = end;
    }

    if want.iter().any(|(p, _)| matches!(p, Place::Resource(_))) {
        for (type_id, entry) in resources.entries_mut() {
            let place = Place::Resource(*type_id);
            let erased: &'a mut dyn Any = &mut **entry;
            match (0..N).find(|&i| want[i].0 == place && want[i].1) {
                Some(i) => {
                    out[i] = Some(Fetched {
                        inner: Inner::Resource(Borrowed::Exclusive(erased)),
                    });
                }
                None => {
                    let shared: &'a dyn Any = &*erased;
                    for i in (0..N).filter(|&i| want[i].0 == place) {
                        out[i] = Some(Fetched {
                            inner: Inner::Resource(Borrowed::Shared(shared)),
                        });
                    }
                }
            }
        }
    }

    out.map(|fetched| fetched.expect("every element was fetched"))
}

/// `Query` for a tuple of `Element`s: fetch every place in one go, then
/// build each view.
macro_rules! tuple_query {
    ($($T:ident),+) => {
        impl<$($T: Element),+> sealed::Sealed for ($($T,)+) {}

        impl<$($T: Element),+> Query for ($($T,)+) {
            type Out<'a> = ($($T::Out<'a>,)+);
            type One<'a> = ($($T::One<'a>,)+);

            #[allow(non_snake_case)]
            fn fetch<'a>(data: Data<'a>) -> Self::Out<'a> {
                let want = [$(($T::place(&data), $T::MUTABLE)),+];
                let Data { slots, components, resources } = data;
                let [$($T,)+] = fetch_places(slots, components.stores_mut(), resources, want);
                ($($T::view($T),)+)
            }

            #[allow(non_snake_case)]
            fn fetch_one<'a>(data: Data<'a>, id: NodeId) -> Self::One<'a> {
                assert!(data.slots.is_live(id), "fetch of a stale id: {id:?}");
                let want = [$(($T::place(&data), $T::MUTABLE)),+];
                let Data { slots, components, resources } = data;
                let [$($T,)+] = fetch_places(slots, components.stores_mut(), resources, want);
                ($($T::one($T, id),)+)
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
