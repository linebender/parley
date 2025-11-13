// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Explicit flow of text blocks (containers), inspired by TextKit's container array.

use alloc::vec::Vec;
use parley::BoundingBox;

use crate::TextBlock;
use crate::multi_selection::BoundaryPolicy;
use parley::style::Brush;

/// A single container in the text flow.
#[derive(Clone, Copy, PartialEq)]
pub struct FlowItem<Id: Copy + Ord + Eq> {
    /// Identifier of the text block this container references.
    pub id: Id,
    /// Container bounds in global coordinates.
    pub rect: BoundingBox,
    /// Separator policy to apply after this container when concatenating text.
    pub join: BoundaryPolicy,
}

impl<Id: Copy + Ord + Eq> FlowItem<Id> {
    /// Create a new flow item with the given block `id`, global container `rect`, and `join`
    /// policy to apply when concatenating text after this item.
    pub fn new(id: Id, rect: BoundingBox, join: BoundaryPolicy) -> Self {
        Self { id, rect, join }
    }
}

impl<Id: Copy + Ord + Eq + core::fmt::Debug> core::fmt::Debug for FlowItem<Id> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("FlowItem")
            .field("id", &self.id)
            .field("join", &self.join)
            .finish_non_exhaustive()
    }
}

/// An ordered list of containers defining hit-testing, navigation order, and join behavior.
#[derive(Clone, Debug, Default)]
pub struct TextFlow<Id: Copy + Ord + Eq> {
    items: Vec<FlowItem<Id>>,
}

impl<Id: Copy + Ord + Eq> TextFlow<Id> {
    /// Create a flow from an explicit `items` vector.
    pub fn new(items: Vec<FlowItem<Id>>) -> Self {
        Self { items }
    }

    /// Read-only access to the flow items.
    pub fn items(&self) -> &[FlowItem<Id>] {
        &self.items
    }

    /// Returns the index of the flow item with the given `id`.
    pub fn index_of(&self, id: Id) -> Option<usize> {
        self.items.iter().position(|it| it.id == id)
    }

    /// Returns the previous block `id` in flow order.
    pub fn prev_id(&self, id: Id) -> Option<Id> {
        let ix = self.index_of(id)?;
        ix.checked_sub(1).map(|i| self.items[i].id)
    }

    /// Returns the next block `id` in flow order.
    pub fn next_id(&self, id: Id) -> Option<Id> {
        let ix = self.index_of(id)?;
        (ix + 1 < self.items.len()).then(|| self.items[ix + 1].id)
    }

    /// Returns the block `id` whose rect contains the point, if any.
    pub fn hit_test(&self, x: f32, y: f32) -> Option<Id> {
        let x = x as f64;
        let y = y as f64;
        self.items
            .iter()
            .find(|it| {
                let r = it.rect;
                x >= r.x0 && x < r.x1 && y >= r.y0 && y < r.y1
            })
            .map(|it| it.id)
    }

    /// Returns the join policy to use after the item identified by `id`.
    pub fn join_after(&self, id: Id) -> BoundaryPolicy {
        if let Some(ix) = self.index_of(id) {
            self.items[ix].join
        } else {
            BoundaryPolicy::Space
        }
    }

    /// Convenience: build a vertical-flow from a list of blocks.
    ///
    /// Each block contributes one [`FlowItem`] with a rect that stacks blocks top-to-bottom
    /// starting at y=0 with no gaps. The height and width are taken from each block’s layout.
    /// The `join` policy is applied to all items.
    pub fn from_vertical_stack<B: Brush, S>(blocks: &[S], join: BoundaryPolicy) -> Self
    where
        S: TextBlock<B, Id = Id>,
    {
        let mut items = Vec::with_capacity(blocks.len());
        let mut y0 = 0.0_f64;
        for b in blocks {
            let layout = b.layout();
            let rect =
                BoundingBox::new(0.0, y0, layout.width() as f64, y0 + layout.height() as f64);
            items.push(FlowItem::new(b.id(), rect, join));
            y0 += layout.height() as f64;
        }
        Self::new(items)
    }
}
