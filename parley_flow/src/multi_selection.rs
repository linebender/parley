// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Multi-surface, discontiguous selection types.
//!
//! These types represent selections that can span multiple [`TextBlock`](crate::TextBlock)s.
//! Typical flow:
//! - Use [`crate::hit_test`] to obtain a [`Caret`] from a global point.
//! - Create a collapsed set with [`SelectionSet::collapsed`], or add [`SelectionSegment`]s for
//!   ranges you want selected.
//! - Render with [`crate::selection_geometry`] and extract text with
//!   [`crate::copy_text`].

use alloc::vec::Vec;
use core::cmp::Ordering;
use core::fmt::Debug;
use core::ops::Range;

use parley::editing::Cursor;
use parley::layout::Affinity;

/// Policy for text boundaries inserted between adjacent selections from different surfaces
/// when serializing with [`crate::copy_text`].
///
/// Use this when a single, uniform separator makes sense for your selection (e.g. with
/// [`crate::flow::TextFlow::from_vertical_stack`]). For per-block separators, build your
/// [`crate::flow::TextFlow`] with per-item `join` policies.
///
/// Guidance on choosing a policy:
/// - `Space` for inline flows (e.g., multiple labels in a row) where spaces are expected.
/// - `Newline` for block/paragraph boundaries so pasted text preserves line breaks.
/// - `None` if your selected ranges already include desired spacing or punctuation.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BoundaryPolicy {
    /// No separator between surfaces.
    None,
    /// Insert a single ASCII space between surfaces.
    Space,
    /// Insert a single newline (U+000A) between surfaces.
    Newline,
}

/// A caret positioned on a particular surface.
///
/// Obtained from [`crate::hit_test`] or navigation utilities. Store it as the active caret
/// in a [`SelectionSet`] (e.g., via [`SelectionSet::collapsed`]) and update it as the user moves
/// the cursor. The `h_pos` field preserves horizontal position during vertical movement.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Caret<Id: Copy + Ord + Eq + Debug> {
    /// Surface identifier.
    pub surface: Id,
    /// Local cursor within the surface’s layout.
    pub cursor: Cursor,
    /// Sticky horizontal position for vertical movement.
    pub h_pos: Option<f32>,
}

/// One selected range on a single surface.
///
/// Construct segments when you already know the byte range you want selected relative to a
/// specific surface’s text. For contiguous stores, see
/// [`TextBlock::text_slice`](crate::TextBlock::text_slice); for non‑contiguous
/// stores, see [`TextBlock::read_text`](crate::TextBlock::read_text).
/// Add segments to a [`SelectionSet`] with [`SelectionSet::add_segment`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectionSegment<Id: Copy + Ord + Eq + Debug> {
    /// Surface identifier this segment applies to.
    pub surface: Id,
    /// Local byte range in the surface’s source text.
    pub range: Range<usize>,
    /// Affinity on the anchor end.
    pub anchor_affinity: Affinity,
    /// Affinity on the focus end.
    pub focus_affinity: Affinity,
}

impl<Id: Copy + Ord + Eq + Debug> SelectionSegment<Id> {
    /// Create a new segment.
    pub fn new(surface: Id, range: Range<usize>) -> Self {
        Self {
            surface,
            range,
            anchor_affinity: Affinity::Downstream,
            focus_affinity: Affinity::Upstream,
        }
    }
}

/// An ordered, normalized set of selection segments with a single active caret.
///
/// Use [`SelectionSet::collapsed`] to start from a caret, then add segments via
/// [`SelectionSet::add_segment`]. Pass the set to rendering/extraction helpers like
/// [`crate::selection_geometry`] and [`crate::copy_text`].
#[derive(Clone, Debug, PartialEq)]
pub struct SelectionSet<Id: Copy + Ord + Eq + Debug> {
    /// Sorted, non-overlapping segments.
    pub segments: Vec<SelectionSegment<Id>>,
    /// The active caret for navigation and editing.
    pub active: Option<Caret<Id>>,
}

impl<Id: Copy + Ord + Eq + Debug> Default for SelectionSet<Id> {
    fn default() -> Self {
        Self {
            segments: Vec::new(),
            active: None,
        }
    }
}

impl<Id: Copy + Ord + Eq + Debug> SelectionSet<Id> {
    /// Create a set from a single caret (collapsed selection).
    pub fn collapsed(caret: Caret<Id>) -> Self {
        Self {
            segments: Vec::new(),
            active: Some(caret),
        }
    }

    /// Add a segment and normalize the set (sort and merge overlaps in the same surface).
    pub fn add_segment(&mut self, mut seg: SelectionSegment<Id>) {
        if seg.range.start > seg.range.end {
            core::mem::swap(&mut seg.range.start, &mut seg.range.end);
        }
        self.segments.push(seg);
        self.normalize();
    }

    /// Sort segments by (surface, start) and coalesce overlaps/adjacencies within the same surface.
    pub fn normalize(&mut self) {
        self.segments
            .sort_by(|a, b| match a.surface.cmp(&b.surface) {
                Ordering::Equal => a.range.start.cmp(&b.range.start),
                other => other,
            });
        let mut out: Vec<SelectionSegment<Id>> = Vec::with_capacity(self.segments.len());
        for seg in self.segments.drain(..) {
            if let Some(last) = out.last_mut() {
                if last.surface == seg.surface && last.range.end >= seg.range.start {
                    // merge
                    last.range.end = last.range.end.max(seg.range.end);
                    // keep existing affinities from the earlier segment
                    continue;
                }
            }
            out.push(seg);
        }
        self.segments = out;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_merges_overlaps_in_same_surface() {
        let mut set: SelectionSet<u32> = SelectionSet::default();
        set.add_segment(SelectionSegment::new(7, 0..5));
        set.add_segment(SelectionSegment::new(7, 3..10));
        assert_eq!(set.segments.len(), 1);
        assert_eq!(set.segments[0].surface, 7);
        assert_eq!(set.segments[0].range.start, 0);
        assert_eq!(set.segments[0].range.end, 10);
    }

    #[test]
    fn normalize_orders_by_surface_then_start_and_merges_adjacencies() {
        let mut set: SelectionSet<u32> = SelectionSet::default();
        set.add_segment(SelectionSegment::new(2, 5..8));
        set.add_segment(SelectionSegment::new(1, 1..2));
        set.add_segment(SelectionSegment::new(1, 0..1));
        assert_eq!(set.segments.len(), 2);
        assert_eq!(set.segments[0].surface, 1);
        assert_eq!(set.segments[0].range.start, 0);
        assert_eq!(set.segments[0].range.end, 2);
        assert_eq!(set.segments[1].surface, 2);
        assert_eq!(set.segments[1].range.start, 5);
        assert_eq!(set.segments[1].range.end, 8);
    }
}
