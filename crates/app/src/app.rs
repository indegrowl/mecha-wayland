use crate::NodeId;
use crate::nodes::{Node, Nodes};
use crate::slots::Slots;
use crate::widgets::{Root, Widgets};

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

    // ── internals ────────────────────────────────────────────────────────

    /// The node record of an id the caller has already validated.
    fn node(&self, id: NodeId) -> &Node {
        self.nodes.get(id.index())
    }

    fn node_mut(&mut self, id: NodeId) -> &mut Node {
        self.nodes.get_mut(id.index())
    }
}

/// What a [`Widget::build`](crate::Widget::build) gets to touch while it
/// runs. Methods arrive in the next task.
pub struct Spawner<'a> {
    #[allow(dead_code)]
    app: &'a mut App,
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
