use crate::nodes::{Node, Nodes};
use crate::slots::Slots;
use crate::widgets::{Root, Widgets};
use crate::{Build, Handle, NodeId, Widget};

/// The runtime: a tree of nodes, each backed by a widget stored in a
/// per-type column.
///
/// Three arenas share one slot index. `slots` is the only validator;
/// `nodes` and `widgets` trust the index they are given. The tree is never
/// empty: [`App::new`] creates the root, which is its own parent and
/// cannot be removed.
pub struct App {
    slots: Slots,
    nodes: Nodes,
    widgets: Widgets,
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
        };
        let (index, generation) = app.slots.alloc();
        let len = app.slots.len();
        let column = app.widgets.column::<Root>(len);
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

    /// Build `builder`'s widget, and whatever subtree its `build` attaches,
    /// as the last child of `parent`.
    ///
    /// The node is linked into the tree before `build` runs, so the handle
    /// `build` receives is the one returned here and `children(parent)`
    /// already lists it. Its widget is stored after `build` returns, so
    /// a lookup of the node's own widget during its build is `None`.
    ///
    /// # Panics
    ///
    /// If `parent` is not live. Spawning under a dead node is a caller
    /// bug, not a state to recover from.
    pub fn spawn<B: Build>(&mut self, parent: impl Into<NodeId>, builder: B) -> Handle<B::Widget> {
        let parent = parent.into();
        assert!(
            self.slots.is_live(parent),
            "spawn under a stale parent: {parent:?}"
        );
        let (index, generation) = self.slots.alloc();
        let len = self.slots.len();
        let widget_type = self.widgets.column::<B::Widget>(len);
        self.widgets.grow(len);
        self.nodes.grow(len);

        let id = NodeId::new(generation, widget_type, index);
        self.nodes.set(index, Node::new(parent));
        self.node_mut(parent).children.push(id);

        let handle = Handle::new(id);
        let widget = B::Widget::build(builder, handle, &mut Spawner { app: self });
        self.widgets
            .store_mut::<B::Widget>(widget_type)
            .expect("column allocated above")
            .set(index, Some(widget));
        handle
    }

    // ── tree: read ───────────────────────────────────────────────────────

    /// Whether `id` names a node that exists right now.
    pub fn is_live(&self, id: impl Into<NodeId>) -> bool {
        self.slots.is_live(id.into())
    }

    /// `None` if `id` is stale. The root's parent is the root.
    pub fn parent(&self, id: impl Into<NodeId>) -> Option<NodeId> {
        let id = id.into();
        self.slots.is_live(id).then(|| self.node(id).parent)
    }

    /// In sibling order. `None` if `id` is stale.
    pub fn children(&self, id: impl Into<NodeId>) -> Option<&[NodeId]> {
        let id = id.into();
        self.slots
            .is_live(id)
            .then(|| self.node(id).children.as_slice())
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
        let store = column.and_then(|c| widgets.store_mut::<W>(c));
        store.into_iter().flat_map(move |store| {
            let widget_type = column.expect("a store exists only with a column");
            store.iter_mut().filter_map(move |(index, slot)| {
                let w = slot.as_mut()?;
                let id = NodeId::new(slots.generation(index), widget_type, index);
                Some((id, w))
            })
        })
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

/// What a [`Widget::build`] gets to touch while it runs: attaching children
/// under the node being built. Later slices add handler registration and
/// component and resource access here.
pub struct Spawner<'a> {
    app: &'a mut App,
}

impl Spawner<'_> {
    /// Build `builder` as the last child of `parent`, which is the `me`
    /// the build was given or a handle returned by an earlier `spawn` in
    /// the same build. Either way it is live. See [`App::spawn`].
    pub fn spawn<B: Build>(&mut self, parent: impl Into<NodeId>, builder: B) -> Handle<B::Widget> {
        self.app.spawn(parent, builder)
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
}
