use smallvec::SmallVec;

use crate::NodeId;
use crate::store::Store;

/// Tree links for one slot. Nothing else: the widget lives in its column
/// and validity lives in `Slots`.
pub(crate) struct Node {
    /// The root is its own parent.
    pub parent: NodeId,
    /// In sibling order. Five inline before the first heap allocation.
    pub children: SmallVec<[NodeId; 5]>,
}

impl Node {
    pub fn new(parent: NodeId) -> Self {
        Self {
            parent,
            children: SmallVec::new(),
        }
    }
}

/// What a vacant slot holds: no children, parented to the root. Manual so
/// `NodeId` needs no public `Default`.
impl Default for Node {
    fn default() -> Self {
        Node::new(NodeId::ROOT)
    }
}

/// One `Node` per slot. A vacant slot holds the default and is never
/// read, because `Slots` rejects the id first.
pub(crate) type Nodes = Store<Node>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_vacant_slot_is_an_empty_node_under_the_root() {
        let parent = NodeId::new(0, 1, 1);
        let mut nodes = Nodes::new();
        nodes.grow(3);
        assert_eq!(nodes.get(1).parent, NodeId::ROOT, "vacant slot");
        assert!(nodes.get(1).children.is_empty());
        nodes.set(2, Node::new(parent));
        assert_eq!(nodes.get(2).parent, parent);
        assert!(nodes.get(2).children.is_empty());
        nodes.get_mut(2).children.push(parent);
        assert_eq!(nodes.get(2).children.as_slice(), &[parent]);
        let taken = nodes.take(2);
        assert_eq!(taken.children.as_slice(), &[parent]);
        assert_eq!(nodes.get(2).parent, NodeId::ROOT, "back to the default");
    }
}
