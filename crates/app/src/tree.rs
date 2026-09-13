use crate::NodeId;
use crate::nodes::{Node, Nodes};
use crate::slots::Slots;

/// A read-only view of the tree. Every tree read on [`App`](crate::App)
/// delegates here; the view exists so [`App::split`](crate::App::split)
/// can lend the tree next to a mutable column view.
///
/// `Copy`. `children` hands out a slice for the view's `'a`, not for the
/// call, so it outlives the view value itself. The two iterator methods take
/// the view by value, which is free because it is Copy, so their iterators
/// borrow only the tree for `'a`.
#[derive(Clone, Copy)]
pub struct Tree<'a> {
    slots: &'a Slots,
    nodes: &'a Nodes,
}

impl<'a> Tree<'a> {
    pub(crate) fn new(slots: &'a Slots, nodes: &'a Nodes) -> Self {
        Self { slots, nodes }
    }

    /// Always live; the only node whose parent is itself.
    pub fn root(&self) -> NodeId {
        NodeId::ROOT
    }

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
    pub fn children(&self, id: impl Into<NodeId>) -> Option<&'a [NodeId]> {
        let id = id.into();
        self.slots
            .is_live(id)
            .then(|| self.node(id).children.as_slice())
    }

    /// The parent chain from `id` up to and including the root, nearest
    /// first, excluding `id`. Empty for the root and for a stale id.
    pub fn ancestors(self, id: impl Into<NodeId>) -> impl Iterator<Item = NodeId> + 'a {
        let tree = self;
        let id = id.into();
        let mut current = tree.slots.is_live(id).then_some(id);
        std::iter::from_fn(move || {
            let node = current?;
            if node == NodeId::ROOT {
                current = None;
                return None;
            }
            let parent = tree.node(node).parent;
            current = Some(parent);
            Some(parent)
        })
    }

    /// The subtree under `id` in pre-order (a node before its children,
    /// children in sibling order), excluding `id`. Empty for a leaf and
    /// for a stale id. Borrows the app, so the tree cannot change while
    /// the iterator is alive.
    pub fn descendants(self, id: impl Into<NodeId>) -> impl Iterator<Item = NodeId> + 'a {
        let tree = self;
        let id = id.into();
        let mut stack: Vec<NodeId> = Vec::new();
        if tree.slots.is_live(id) {
            stack.extend(tree.node(id).children.iter().rev());
        }
        std::iter::from_fn(move || {
            let next = stack.pop()?;
            stack.extend(tree.node(next).children.iter().rev());
            Some(next)
        })
    }

    /// The node record of an id the caller has already validated.
    fn node(&self, id: NodeId) -> &'a Node {
        self.nodes.get(id.slot())
    }
}
