use std::collections::VecDeque;

use crate::component::{Component, Components};
use crate::message::{Removed, Spawned};
use crate::nodes::{Node, Nodes};
use crate::query::{Columns, CompMut, Query};
use crate::slots::Slots;
use crate::system::{System, Systems};
use crate::tree::Tree;
use crate::widgets::{Root, Widgets};
use crate::{Build, Bundle, Handle, NodeId, Signal, Widget};

/// A queued unit of work: a signal or an event with its dispatch baked
/// in, so the queue needs no knowledge of the concrete type.
pub(crate) type Job = Box<dyn FnOnce(&mut App)>;

/// The runtime: a tree of nodes, each backed by a widget stored in a
/// per-type column.
///
/// Four arenas share one slot index. `slots` is the only validator;
/// `nodes`, `widgets`, and `components` trust the index they are given. The tree is never
/// empty: [`App::new`] creates the root, which is its own parent and
/// cannot be removed.
///
/// Messages are queued, never run inline: [`App::signal`] and `App::emit`
/// push jobs, [`App::flush`] runs them.
pub struct App {
    slots: Slots,
    nodes: Nodes,
    widgets: Widgets,
    components: Components,
    systems: Systems,
    /// Events wait here. Drained before every signal.
    events: VecDeque<Job>,
    /// Signals wait here. One runs per round of `flush`.
    signals: VecDeque<Job>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        let mut app = App {
            slots: Slots::new(),
            nodes: Nodes::new(),
            widgets: Widgets::new(),
            components: Components::new(),
            systems: Systems::new(),
            events: VecDeque::new(),
            signals: VecDeque::new(),
        };
        let column = app.widgets.column::<Root>(0);
        let (index, generation) = app.slots.alloc(column);
        let len = app.slots.len();
        app.widgets.grow(len);
        let id = NodeId::new(generation, column, index);
        debug_assert_eq!(id, NodeId::ROOT);
        app.nodes.grow(len);
        app.nodes.set(index, Node::new(id));
        app.widgets
            .store_mut::<Root>(column)
            .expect("root column allocated above")
            .set(index, Some(Root));
        app
    }

    /// Always live; the only node whose parent is itself.
    pub fn root(&self) -> NodeId {
        NodeId::ROOT
    }

    // ── tree: write ──────────────────────────────────────────────────────

    /// [`App::spawn_with`] with no initial component values.
    ///
    /// # Panics
    ///
    /// If `parent` is not live. See [`App::spawn_with`].
    pub fn spawn<B: Build>(&mut self, parent: impl Into<NodeId>, builder: B) -> Handle<B::Widget> {
        self.spawn_with(parent, builder, ())
    }

    /// Build `builder`'s widget, and whatever subtree its `build` attaches,
    /// as the last child of `parent`, with the component values in
    /// `bundle` written to the new node first.
    ///
    /// Order: the node is linked into the tree, then the bundle is written
    /// (each value flags the node like any other write), then `build` runs
    /// with a handle to the node and can read those values through
    /// [`Spawner::component`], then the widget is stored. So `children(parent)`
    /// already lists the node during its build, and a lookup of the node's
    /// own widget during its build is `None`.
    ///
    /// `bundle` is `()` or a tuple of one to six components; see [`Bundle`].
    ///
    /// # Panics
    ///
    /// If `parent` is not live, or a type in `bundle` is not registered.
    pub fn spawn_with<B: Build, K: Bundle>(
        &mut self,
        parent: impl Into<NodeId>,
        builder: B,
        bundle: K,
    ) -> Handle<B::Widget> {
        let parent = parent.into();
        assert!(
            self.slots.is_live(parent),
            "spawn under a stale parent: {parent:?}"
        );
        let widget_type = self.widgets.column::<B::Widget>(self.slots.len());
        let (index, generation) = self.slots.alloc(widget_type);
        let len = self.slots.len();
        self.widgets.grow(len);
        self.nodes.grow(len);
        self.components.grow(len);

        let id = NodeId::new(generation, widget_type, index);
        self.nodes.set(index, Node::new(parent));
        self.node_mut(parent).children.push(id);

        bundle.insert(self, id);

        let handle = Handle::new(id);
        let widget = B::Widget::build(
            builder,
            handle,
            &mut Spawner {
                app: self,
                me: handle,
            },
        );
        debug_assert!(self.slots.is_live(id), "build removed its own node");
        self.widgets
            .store_mut::<B::Widget>(widget_type)
            .expect("column allocated above")
            .set(index, Some(widget));
        self.signal(Spawned { id, parent });
        handle
    }

    /// Remove `id` and every node under it. Every id in the subtree is
    /// stale afterwards and its slots go back to the free list.
    ///
    /// Returns `false`, changing nothing, if `id` is already stale:
    /// removing a subtree twice is legitimate once queued work exists.
    ///
    /// # Panics
    ///
    /// If `id` is the root.
    pub fn remove(&mut self, id: impl Into<NodeId>) -> bool {
        let id = id.into();
        if !self.slots.is_live(id) {
            return false;
        }
        assert!(id != NodeId::ROOT, "the root node cannot be removed");

        let parent = self.node(id).parent;
        self.signal(Removed { id, parent });
        let siblings = &mut self.node_mut(parent).children;
        let position = siblings
            .iter()
            .position(|c| *c == id)
            .expect("live node is in its parent's children");
        siblings.remove(position);

        // Parent before children: free a node, then push its children. Every
        // descendant is reached before we return, and each is freed after
        // its parent so nothing is left pointing into the tree.
        let mut pending = vec![id];
        while let Some(current) = pending.pop() {
            self.slots.free(current.index());
            self.widgets.free(current.widget_type(), current.index());
            self.components.free(current.index());
            let node = self.nodes.take(current.index());
            pending.extend(node.children);
        }
        true
    }

    // ── tree: read ───────────────────────────────────────────────────────

    /// The tree as a read-only view. Every read below is this view's
    /// method; take the view itself to hold the tree across calls or to
    /// pair it with a column view through [`App::split`].
    pub fn tree(&self) -> Tree<'_> {
        Tree::new(&self.slots, &self.nodes)
    }

    /// Whether `id` names a node that exists right now.
    pub fn is_live(&self, id: impl Into<NodeId>) -> bool {
        self.tree().is_live(id)
    }

    /// `None` if `id` is stale. The root's parent is the root.
    pub fn parent(&self, id: impl Into<NodeId>) -> Option<NodeId> {
        self.tree().parent(id)
    }

    /// In sibling order. `None` if `id` is stale.
    pub fn children(&self, id: impl Into<NodeId>) -> Option<&[NodeId]> {
        self.tree().children(id)
    }

    /// The parent chain from `id` up to and including the root, nearest
    /// first, excluding `id`. Empty for the root and for a stale id.
    pub fn ancestors(&self, id: impl Into<NodeId>) -> impl Iterator<Item = NodeId> + '_ {
        self.tree().ancestors(id)
    }

    /// The subtree under `id` in pre-order (a node before its children,
    /// children in sibling order), excluding `id`. Empty for a leaf and
    /// for a stale id. Borrows the app, so the tree cannot change while
    /// the iterator is alive.
    pub fn descendants(&self, id: impl Into<NodeId>) -> impl Iterator<Item = NodeId> + '_ {
        self.tree().descendants(id)
    }

    // ── widgets ──────────────────────────────────────────────────────────

    /// `None` if `id` is stale, holds a widget of another type, or is the
    /// node whose `build` is currently running.
    pub fn widget<W: Widget>(&self, id: impl Into<NodeId>) -> Option<&W> {
        let id = id.into();
        if !self.slots.is_live(id) {
            return None;
        }
        self.widgets
            .store::<W>(id.widget_type())?
            .get(id.index())
            .as_ref()
    }

    /// `None` under the same conditions as [`App::widget`].
    pub fn widget_mut<W: Widget>(&mut self, id: impl Into<NodeId>) -> Option<&mut W> {
        let id = id.into();
        if !self.slots.is_live(id) {
            return None;
        }
        self.widgets
            .store_mut::<W>(id.widget_type())?
            .get_mut(id.index())
            .as_mut()
    }

    /// Every live widget of type `W` with its id, in slot order. Empty if
    /// no `W` was ever spawned. Slot order is not tree order.
    pub fn widgets<W: Widget>(&mut self) -> impl Iterator<Item = (NodeId, &mut W)> {
        let slots = &self.slots;
        let widgets = &mut self.widgets;
        let column = widgets.column_of::<W>();
        let store = column.and_then(|c| widgets.store_mut::<W>(c).map(|s| (c, s)));
        store.into_iter().flat_map(move |(widget_type, store)| {
            store.iter_mut().filter_map(move |(index, slot)| {
                let w = slot.as_mut()?;
                let id = NodeId::new(slots.generation(index), widget_type, index);
                debug_assert!(slots.is_live(id), "a widget in a vacant slot");
                Some((id, w))
            })
        })
    }

    // ── components ───────────────────────────────────────────────────────

    /// Allocate a column for `C`. Every live node, the root included,
    /// holds `C::default()` from here on, and so does every node spawned
    /// later.
    ///
    /// # Panics
    ///
    /// If `C` is already registered.
    pub fn register_component<C: Component>(&mut self) {
        self.components.register::<C>(self.slots.len());
    }

    /// One node's `C`. `None` if `id` is stale.
    ///
    /// Pays the column lookup on every call; a pass over many nodes takes
    /// a view through [`App::components`] once instead.
    ///
    /// # Panics
    ///
    /// If `C` is not registered.
    pub fn component<C: Component>(&self, id: impl Into<NodeId>) -> Option<&C> {
        self.components.column::<C>().get(&self.slots, id.into())
    }

    /// A write guard for one node's `C`. `None` if `id` is stale. The
    /// guard's first `DerefMut` records the node for [`App::take_changed`].
    ///
    /// # Panics
    ///
    /// If `C` is not registered.
    pub fn component_mut<C: Component>(&mut self, id: impl Into<NodeId>) -> Option<CompMut<'_, C>> {
        let slots = &self.slots;
        self.components.column_mut::<C>().get_mut(slots, id.into())
    }

    /// Every node whose `C` was written since the last drain, in
    /// first-write order, live nodes only. Clears the record as it goes;
    /// dropping the iterator early still clears. O(changed).
    ///
    /// This is the hand-off to the events slice, which will turn each id
    /// into an `OnChanged<C>` dispatch. Nothing fires here.
    ///
    /// # Panics
    ///
    /// If `C` is not registered.
    pub fn take_changed<C: Component>(&mut self) -> impl Iterator<Item = NodeId> + '_ {
        let slots = &self.slots;
        self.components.column_mut::<C>().take_changed(slots)
    }

    /// Fetch one or more columns as views: `&C` for a
    /// [`Comps`](crate::Comps), `&mut C` for a
    /// [`CompsMut`](crate::CompsMut), or a tuple of up to six of those.
    ///
    /// ```
    /// # use app::prelude::*;
    /// # #[derive(Default)]
    /// # struct Style;
    /// # impl Component for Style {}
    /// # #[derive(Default)]
    /// # struct Rect;
    /// # impl Component for Rect {}
    /// # let mut app = App::new();
    /// # app.register_component::<Style>();
    /// # app.register_component::<Rect>();
    /// let (style, mut rect) = app.components::<(&Style, &mut Rect)>();
    /// # let _ = (&style, &mut rect);
    /// ```
    ///
    /// Takes `&mut self` even for a read-only query, since one signature
    /// covers both, so the views hold the app exclusively while they live.
    /// Cost per column: one hash and one downcast. Nothing per node.
    ///
    /// # Panics
    ///
    /// If a named type is not registered, or one type appears twice with
    /// at least one `&mut`. `(&A, &A)` is fine.
    pub fn components<Q: Query>(&mut self) -> Q::Out<'_> {
        Q::fetch(Columns::new(&self.slots, &mut self.components))
    }

    /// The tree read-only and the columns mutably, at the same time. For a
    /// pass that walks the tree while writing a column, such as layout
    /// reading `children` and writing a rect.
    pub fn split(&mut self) -> (Tree<'_>, Columns<'_>) {
        (
            Tree::new(&self.slots, &self.nodes),
            Columns::new(&self.slots, &mut self.components),
        )
    }

    // ── systems and signals ──────────────────────────────────────────────

    /// Run `system` for every `S` that is flushed from now on, after the
    /// systems registered for `S` before it. Never removed.
    pub fn system<S: Signal>(&mut self, system: System<S>) -> &mut Self {
        self.systems.add(system);
        self
    }

    /// Queue `signal` for the systems registered for `S`. Nothing runs
    /// until [`App::flush`]. If no system is registered for `S` right
    /// now the signal is dropped here, so unlistened traffic costs one
    /// hash; a system registered between the send and the flush still
    /// runs, because the job looks the run up when it runs.
    pub fn signal<S: Signal>(&mut self, signal: S) {
        if !self.systems.has::<S>() {
            return;
        }
        self.signals
            .push_back(Box::new(move |app: &mut App| run_signal(app, &signal)));
    }

    /// Run everything queued: every event, then one signal, then every
    /// event again, until both queues are empty. Work a job queues runs
    /// in the same call. A no-op on empty queues.
    pub fn flush(&mut self) {
        loop {
            while let Some(job) = self.events.pop_front() {
                job(self);
            }
            match self.signals.pop_front() {
                Some(job) => job(self),
                None => break,
            }
        }
    }

    // ── internals ────────────────────────────────────────────────────────

    /// The node record of an id the caller has already validated.
    fn node(&self, id: NodeId) -> &Node {
        self.nodes.get(id.index())
    }

    fn node_mut(&mut self, id: NodeId) -> &mut Node {
        self.nodes.get_mut(id.index())
    }
}

/// Run `signal` through every system for `S`, by index, so a system
/// registered during the pass runs in it.
fn run_signal<S: Signal>(app: &mut App, signal: &S) {
    let mut i = 0;
    while let Some(system) = app.systems.get::<S>(i) {
        system(app, signal);
        i += 1;
    }
}

/// What a [`Widget::build`] gets to touch while it runs: the node being
/// built, attaching children under it, single-node component access
/// meant for `me` and the nodes it spawned, and attaching handlers on
/// its behalf. Typed by the widget being built so a handler it registers
/// knows what `me` is. Deliberately narrow: no whole-column views, no
/// drain, no split. A later slice adds resource access here.
pub struct Spawner<'a, W: Widget> {
    app: &'a mut App,
    me: Handle<W>,
}

impl<W: Widget> Spawner<'_, W> {
    /// The node being built: the `me` the build was given.
    pub fn me(&self) -> Handle<W> {
        self.me
    }

    /// Build `builder` as the last child of `parent`, which is the `me`
    /// the build was given or a handle returned by an earlier `spawn` in
    /// the same build. Either way it is live. See [`App::spawn`].
    pub fn spawn<B: Build>(&mut self, parent: impl Into<NodeId>, builder: B) -> Handle<B::Widget> {
        self.app.spawn(parent, builder)
    }

    /// [`Spawner::spawn`] with initial component values. See
    /// [`App::spawn_with`].
    pub fn spawn_with<B: Build, K: Bundle>(
        &mut self,
        parent: impl Into<NodeId>,
        builder: B,
        bundle: K,
    ) -> Handle<B::Widget> {
        self.app.spawn_with(parent, builder, bundle)
    }

    /// See [`App::component`].
    pub fn component<C: Component>(&self, id: impl Into<NodeId>) -> Option<&C> {
        self.app.component(id)
    }

    /// See [`App::component_mut`]. A write here flags like any other
    /// write; the `Default` a spawn starts from does not.
    pub fn component_mut<C: Component>(&mut self, id: impl Into<NodeId>) -> Option<CompMut<'_, C>> {
        self.app.component_mut(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_is_column_zero_slot_zero() {
        let app = App::new();
        assert_eq!(app.root(), NodeId::new(0, 0, 0));
        assert_eq!(app.widgets.column_of::<Root>(), Some(0));
        assert!(app.widgets.store::<Root>(0).unwrap().get(0).is_some());
    }

    struct Leaf;
    impl Build for Leaf {
        type Widget = Leaf;
    }
    impl Widget for Leaf {
        type Builder = Leaf;
        fn build(b: Leaf, _: Handle<Leaf>, _: &mut Spawner<'_, Self>) -> Leaf {
            b
        }
    }

    #[test]
    fn a_reused_slot_keeps_its_index_and_bumps_the_generation() {
        let mut app = App::new();
        let old = app.spawn(app.root(), Leaf).id();
        assert!(app.remove(old));
        let new = app.spawn(app.root(), Leaf).id();
        assert_eq!(new.index(), old.index());
        assert_eq!(new.widget_type(), old.widget_type());
        assert_eq!(new.generation(), old.generation() + 1);
        assert!(app.is_live(new));
        assert!(!app.is_live(old));
    }

    #[test]
    fn removed_slots_are_back_to_default_in_every_arena() {
        let mut app = App::new();
        let id = app.spawn(app.root(), Leaf).id();
        app.remove(id);
        let node = app.nodes.get(id.index());
        assert_eq!(node.parent, NodeId::ROOT);
        assert!(node.children.is_empty());
        let column = app.widgets.column_of::<Leaf>().unwrap();
        assert!(
            app.widgets
                .store::<Leaf>(column)
                .unwrap()
                .get(id.index())
                .is_none()
        );
    }
}
