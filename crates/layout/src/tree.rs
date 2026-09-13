//! `LayoutTree`: the view taffy walks for one root. Holds the tree for
//! reading and the four column views, and resolves taffy's node id, which
//! is our slot, back to a live `NodeId` through a map built for the pass.
//!
//! Taffy 0.10.1: the traits are `src/tree/traits.rs` (`TraversePartialTree`
//! 148, `LayoutPartialTree` 174, `CacheTree` 206, `RoundTree` 221,
//! `LayoutFlexboxContainer` 240, `LayoutBlockContainer` 288); the compute
//! entry points are `src/compute/mod.rs` (`compute_cached_layout` 174,
//! `round_layout` 219, `compute_hidden_layout` 278), `compute/leaf.rs:15`,
//! `compute/flexbox.rs:166` and `compute/block.rs:244`.

use app::{Comps, CompsMut, NodeId, Tree};
use geometry::{Insets, Point, Rect};
use taffy::geometry::Size as TSize;
use taffy::tree::Layout as TLayout;
use taffy::{
    CacheTree, LayoutBlockContainer, LayoutFlexboxContainer, LayoutInput, LayoutOutput,
    LayoutPartialTree, NodeId as TaffyId, RoundTree, RunMode, TraversePartialTree, TraverseTree,
    compute_block_layout, compute_cached_layout, compute_flexbox_layout, compute_hidden_layout,
    compute_leaf_layout,
};

use crate::style::{Display, LayoutStyle};
use crate::{Constraints, Layout, Measure, Scratch};

/// Taffy's id for a node: its slot.
pub(crate) fn taffy_id(id: NodeId) -> TaffyId {
    TaffyId::from(id.slot())
}

pub(crate) struct LayoutTree<'a> {
    tree: Tree<'a>,
    /// The root of this pass; its box lands at the origin.
    root: NodeId,
    /// The live id at each slot under `root`, so a slot taffy names
    /// resolves without reaching into the arena.
    slots: Vec<Option<NodeId>>,
    styles: Comps<'a, LayoutStyle>,
    measures: Comps<'a, Measure>,
    layouts: CompsMut<'a, Layout>,
    scratch: CompsMut<'a, Scratch>,
}

impl<'a> LayoutTree<'a> {
    pub(crate) fn new(
        tree: Tree<'a>,
        root: NodeId,
        styles: Comps<'a, LayoutStyle>,
        measures: Comps<'a, Measure>,
        layouts: CompsMut<'a, Layout>,
        scratch: CompsMut<'a, Scratch>,
    ) -> Self {
        let mut slots: Vec<Option<NodeId>> = Vec::new();
        for id in std::iter::once(root).chain(tree.descendants(root)) {
            let slot = id.slot() as usize;
            if slots.len() <= slot {
                slots.resize(slot + 1, None);
            }
            slots[slot] = Some(id);
        }
        Self {
            tree,
            root,
            slots,
            styles,
            measures,
            layouts,
            scratch,
        }
    }

    fn id(&self, node: TaffyId) -> NodeId {
        self.slots[usize::from(node)].expect("taffy only names slots the view gave it")
    }

    fn children(&self, node: TaffyId) -> &'a [NodeId] {
        self.tree.children(self.id(node)).unwrap_or(&[])
    }

    fn style(&self, node: TaffyId) -> &LayoutStyle {
        self.styles
            .get(self.id(node))
            .expect("every live node has a LayoutStyle")
    }
}

/// The children of a node as taffy ids, without allocating.
pub(crate) struct Children<'b>(std::slice::Iter<'b, NodeId>);

impl Iterator for Children<'_> {
    type Item = TaffyId;
    fn next(&mut self) -> Option<TaffyId> {
        self.0.next().map(|&id| taffy_id(id))
    }
}

impl TraversePartialTree for LayoutTree<'_> {
    type ChildIter<'b>
        = Children<'b>
    where
        Self: 'b;

    fn child_ids(&self, parent: TaffyId) -> Self::ChildIter<'_> {
        Children(self.children(parent).iter())
    }

    fn child_count(&self, parent: TaffyId) -> usize {
        self.children(parent).len()
    }

    fn get_child_id(&self, parent: TaffyId, index: usize) -> TaffyId {
        taffy_id(self.children(parent)[index])
    }
}

impl TraverseTree for LayoutTree<'_> {}

impl LayoutPartialTree for LayoutTree<'_> {
    type CoreContainerStyle<'b>
        = &'b LayoutStyle
    where
        Self: 'b;
    type CustomIdent = String;

    fn get_core_container_style(&self, node: TaffyId) -> &LayoutStyle {
        self.style(node)
    }

    fn set_unrounded_layout(&mut self, node: TaffyId, layout: &TLayout) {
        let id = self.id(node);
        if let Some(mut scratch) = self.scratch.get_mut(id) {
            scratch.unrounded = *layout;
        }
    }

    fn compute_child_layout(&mut self, node: TaffyId, inputs: LayoutInput) -> LayoutOutput {
        if inputs.run_mode == RunMode::PerformHiddenLayout {
            return compute_hidden_layout(self, node);
        }
        compute_cached_layout(self, node, inputs, |tree, node, inputs| {
            let has_children = !tree.children(node).is_empty();
            match (tree.style(node).display, has_children) {
                (Display::Hidden, _) => compute_hidden_layout(tree, node),
                (Display::Block, true) => compute_block_layout(tree, node, inputs, None),
                (Display::Flex, true) => compute_flexbox_layout(tree, node, inputs),
                (_, false) => {
                    let id = tree.id(node);
                    let style = tree.style(node);
                    let measure = tree
                        .measures
                        .get(id)
                        .expect("every live node has a Measure");
                    compute_leaf_layout(
                        inputs,
                        style,
                        |_, _| 0.0,
                        |known, available| {
                            let size = measure.measure(Constraints::from_taffy(known, available));
                            TSize {
                                width: size.width,
                                height: size.height,
                            }
                        },
                    )
                }
            }
        })
    }
}

impl LayoutFlexboxContainer for LayoutTree<'_> {
    type FlexboxContainerStyle<'b>
        = &'b LayoutStyle
    where
        Self: 'b;
    type FlexboxItemStyle<'b>
        = &'b LayoutStyle
    where
        Self: 'b;

    fn get_flexbox_container_style(&self, node: TaffyId) -> &LayoutStyle {
        self.style(node)
    }
    fn get_flexbox_child_style(&self, child: TaffyId) -> &LayoutStyle {
        self.style(child)
    }
}

impl LayoutBlockContainer for LayoutTree<'_> {
    type BlockContainerStyle<'b>
        = &'b LayoutStyle
    where
        Self: 'b;
    type BlockItemStyle<'b>
        = &'b LayoutStyle
    where
        Self: 'b;

    fn get_block_container_style(&self, node: TaffyId) -> &LayoutStyle {
        self.style(node)
    }
    fn get_block_child_style(&self, child: TaffyId) -> &LayoutStyle {
        self.style(child)
    }
}

impl CacheTree for LayoutTree<'_> {
    fn cache_get(&self, node: TaffyId, input: &LayoutInput) -> Option<LayoutOutput> {
        self.scratch.get(self.id(node))?.cache.get(input)
    }
    fn cache_store(&mut self, node: TaffyId, input: &LayoutInput, output: LayoutOutput) {
        let id = self.id(node);
        if let Some(mut scratch) = self.scratch.get_mut(id) {
            scratch.cache.store(input, output);
        }
    }
    fn cache_clear(&mut self, node: TaffyId) {
        let id = self.id(node);
        if let Some(mut scratch) = self.scratch.get_mut(id) {
            scratch.cache.clear();
        }
    }
}

impl RoundTree for LayoutTree<'_> {
    fn get_unrounded_layout(&self, node: TaffyId) -> TLayout {
        self.scratch
            .get(self.id(node))
            .expect("every live node has a Scratch")
            .unrounded
    }

    /// Taffy's rounding walk (`round_layout`, `compute/mod.rs:219`) is
    /// preorder: a node's final layout is set before its children are
    /// visited, with `location` relative to the parent. So the parent's
    /// absolute box is already in the `Layout` column, and one write per
    /// node makes the child absolute too.
    fn set_final_layout(&mut self, node: TaffyId, layout: &TLayout) {
        let id = self.id(node);
        let origin = if id == self.root {
            Point::ZERO
        } else {
            let parent = self.tree.parent(id).expect("a node in a pass is live");
            self.layouts
                .get(parent)
                .expect("the parent's Layout is written before its children's")
                .rect
                .origin
        };
        let value = Layout {
            rect: Rect::new(
                origin.x + layout.location.x,
                origin.y + layout.location.y,
                layout.size.width,
                layout.size.height,
            ),
            padding: Insets::new(
                layout.padding.top,
                layout.padding.right,
                layout.padding.bottom,
                layout.padding.left,
            ),
            border: Insets::new(
                layout.border.top,
                layout.border.right,
                layout.border.bottom,
                layout.border.left,
            ),
        };
        if let Some(mut slot) = self.layouts.get_mut(id) {
            slot.set_if_neq(value);
        }
    }
}
