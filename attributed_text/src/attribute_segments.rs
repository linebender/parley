// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Attribute-based segmentation for [`AttributedText`].
//!
//! Given an [`AttributedText`] with overlapping attribute spans, this module produces
//! non-overlapping, contiguous segments and provides a view of spans active over each segment.

use alloc::vec::Vec;
use core::fmt::Debug;
use core::ops::Range;

use crate::AttributedText;
use crate::TextStorage;

fn build_segment_state<T: Debug + TextStorage, Attr: Debug>(
    attributed: &AttributedText<T, Attr>,
    workspace: &mut AttributeSegmentsWorkspace,
) {
    let len = attributed.len();
    debug_assert!(
        len <= u32::MAX as usize,
        "attributed_text currently supports texts up to u32::MAX bytes (got {len})"
    );
    let len_u32 = u32::try_from(len).expect("validated by debug_assert above");
    let attr_count = attributed.attributes_len();
    debug_assert!(
        attr_count <= u32::MAX as usize,
        "attributed_text currently supports up to u32::MAX span attributes (got {attr_count})"
    );

    workspace.boundaries.clear();
    workspace
        .boundaries
        .reserve(2 + attr_count.saturating_mul(2));
    workspace.boundaries.push(0);
    workspace.boundaries.push(len_u32);
    for (range, _) in attributed.attributes_iter() {
        let start_u32 = u32::try_from(range.start).expect("range start should fit in u32");
        let end_u32 = u32::try_from(range.end).expect("range end should fit in u32");
        workspace.boundaries.push(start_u32);
        workspace.boundaries.push(end_u32);
    }
    workspace.boundaries.sort_unstable();
    workspace.boundaries.dedup();

    let boundary_count = workspace.boundaries.len();

    workspace.start_counts.clear();
    workspace.start_counts.resize(boundary_count, 0);
    workspace.end_counts.clear();
    workspace.end_counts.resize(boundary_count, 0);
    workspace.span_build.clear();
    workspace.span_build.reserve(attr_count);

    for (attr_index, (range, _)) in attributed.attributes_iter().enumerate() {
        if range.start == range.end {
            continue;
        }
        let start_u32 = u32::try_from(range.start).expect("range start should fit in u32");
        let end_u32 = u32::try_from(range.end).expect("range end should fit in u32");
        let start_boundary = workspace
            .boundaries
            .binary_search(&start_u32)
            .expect("attribute boundary start should be in boundary list");
        let end_boundary = workspace
            .boundaries
            .binary_search(&end_u32)
            .expect("attribute boundary end should be in boundary list");
        if start_boundary == end_boundary {
            continue;
        }

        workspace.span_build.push((
            u32::try_from(attr_index).expect("attribute index overflow"),
            u32::try_from(start_boundary).expect("start boundary index overflow"),
            u32::try_from(end_boundary).expect("end boundary index overflow"),
        ));
        workspace.start_counts[start_boundary] += 1;
        workspace.end_counts[end_boundary] += 1;
    }

    workspace.start_offsets.clear();
    workspace.start_offsets.resize(boundary_count + 1, 0);
    workspace.end_offsets.clear();
    workspace.end_offsets.resize(boundary_count + 1, 0);

    {
        let mut cursor = 0_usize;
        for i in 0..boundary_count {
            cursor += workspace.start_counts[i] as usize;
            workspace.start_offsets[i + 1] =
                u32::try_from(cursor).expect("start event cursor overflow");
        }
    }
    {
        let mut cursor = 0_usize;
        for i in 0..boundary_count {
            cursor += workspace.end_counts[i] as usize;
            workspace.end_offsets[i + 1] =
                u32::try_from(cursor).expect("end event cursor overflow");
        }
    }

    workspace.start_events.clear();
    workspace
        .start_events
        .resize(workspace.start_offsets[boundary_count] as usize, 0);
    workspace.end_events.clear();
    workspace
        .end_events
        .resize(workspace.end_offsets[boundary_count] as usize, 0);

    // Reuse counts as per-boundary write cursors.
    workspace.start_counts.fill(0);
    workspace.end_counts.fill(0);
    for &(attr_index, start_boundary, end_boundary) in &workspace.span_build {
        let start_boundary = start_boundary as usize;
        let end_boundary = end_boundary as usize;

        let start_ix = workspace.start_offsets[start_boundary] as usize
            + workspace.start_counts[start_boundary] as usize;
        workspace.start_events[start_ix] = attr_index;
        workspace.start_counts[start_boundary] += 1;

        let end_ix = workspace.end_offsets[end_boundary] as usize
            + workspace.end_counts[end_boundary] as usize;
        workspace.end_events[end_ix] = attr_index;
        workspace.end_counts[end_boundary] += 1;
    }

    workspace.active.clear();
    if workspace.active.capacity() < workspace.span_build.len() {
        workspace
            .active
            .reserve(workspace.span_build.len() - workspace.active.capacity());
    }
}

/// Reusable allocation workspace for attribute segmentation.
///
/// Reusing a workspace amortizes setup allocations when processing many pieces of text.
#[derive(Clone, Debug, Default)]
pub struct AttributeSegmentsWorkspace {
    boundaries: Vec<u32>,
    start_counts: Vec<u32>,
    end_counts: Vec<u32>,
    start_offsets: Vec<u32>,
    start_events: Vec<u32>,
    end_offsets: Vec<u32>,
    end_events: Vec<u32>,
    span_build: Vec<(u32, u32, u32)>,
    active: Vec<u32>,
}

impl AttributeSegmentsWorkspace {
    /// Create an empty workspace.
    pub fn new() -> Self {
        Self::default()
    }

    /// Build an iterator using this workspace's retained allocations.
    pub fn segments<'w, 'a, T: Debug + TextStorage, Attr: Debug>(
        &'w mut self,
        attributed: &'a AttributedText<T, Attr>,
    ) -> AttributeSegments<'w, 'a, T, Attr> {
        build_segment_state(attributed, self);

        AttributeSegments {
            attributed,
            workspace: self,
            index: 0,
        }
    }
}

/// Iterator over contiguous attribute segments produced from an [`AttributedText`].
///
/// Each yielded item is a non-empty, non-overlapping byte range. The active spans for the
/// yielded range are exposed through [`AttributeSegments::active_spans`].
///
/// # Examples
///
/// ```
/// use attributed_text::{AttributeSegmentsWorkspace, AttributedText, TextRange};
///
/// #[derive(Debug, PartialEq, Eq)]
/// enum Color {
///     Red,
///     Blue,
/// }
///
/// let mut text = AttributedText::new("hello");
/// text.apply_attribute(TextRange::new(text.text(), 0..2).unwrap(), Color::Red);
/// text.apply_attribute(TextRange::new(text.text(), 1..5).unwrap(), Color::Blue);
///
/// let mut workspace = AttributeSegmentsWorkspace::new();
/// let mut segments = workspace.segments(&text);
///
/// assert_eq!(segments.next(), Some(0..1));
/// let colors: Vec<_> = segments
///     .active_spans()
///     .iter()
///     .map(|(_, c)| c)
///     .collect();
/// assert_eq!(colors, vec![&Color::Red]);
///
/// assert_eq!(segments.next(), Some(1..2));
/// let colors: Vec<_> = segments
///     .active_spans()
///     .iter()
///     .map(|(_, c)| c)
///     .collect();
/// assert_eq!(colors, vec![&Color::Red, &Color::Blue]);
///
/// let active = segments.active_spans();
/// let mut count = 0;
/// for (_range, _attr) in &active {
///     count += 1;
/// }
/// assert_eq!(count, 2);
/// ```
///
/// # Implementation notes
///
/// Indices are stored as `u32` to reduce memory footprint on 64-bit platforms. This caps
/// supported text length and event counts at `u32::MAX`. Inputs that exceed this bound panic.
///
/// Zero-length attribute ranges are excluded from the active span set, but their boundaries are
/// still included in segmentation, so they can split output ranges.
#[derive(Debug)]
pub struct AttributeSegments<'w, 'a, T: Debug + TextStorage, Attr: Debug> {
    attributed: &'a AttributedText<T, Attr>,
    workspace: &'w mut AttributeSegmentsWorkspace,
    index: usize,
}

impl<'w, 'a, T: Debug + TextStorage, Attr: Debug> AttributeSegments<'w, 'a, T, Attr> {
    fn update_active_for_boundary(&mut self, boundary_index: usize) {
        let end_range = self.workspace.end_offsets[boundary_index] as usize
            ..self.workspace.end_offsets[boundary_index + 1] as usize;
        for &id in &self.workspace.end_events[end_range] {
            if let Ok(ix) = self.workspace.active.binary_search(&id) {
                self.workspace.active.remove(ix);
            }
        }

        let start_range = self.workspace.start_offsets[boundary_index] as usize
            ..self.workspace.start_offsets[boundary_index + 1] as usize;
        for &id in &self.workspace.start_events[start_range] {
            match self.workspace.active.binary_search(&id) {
                Ok(_) => {}
                Err(ix) => self.workspace.active.insert(ix, id),
            }
        }
    }

    /// Returns the spans active for the most recently yielded segment.
    ///
    /// Before the first successful [`Iterator::next`] call, this returns an empty view.
    /// After exhaustion (`next()` returns `None`), this also returns an empty view.
    pub fn active_spans(&self) -> ActiveSpans<'_, 'a, T, Attr> {
        ActiveSpans {
            active_ids: &self.workspace.active,
            attributed: self.attributed,
        }
    }
}

impl<T: Debug + TextStorage, Attr: Debug> Iterator for AttributeSegments<'_, '_, T, Attr> {
    type Item = Range<usize>;

    fn size_hint(&self) -> (usize, Option<usize>) {
        // Remaining segments are remaining adjacent boundary pairs: [i, i + 1).
        let remaining = self
            .workspace
            .boundaries
            .len()
            .saturating_sub(self.index + 1);
        (remaining, Some(remaining))
    }

    fn next(&mut self) -> Option<Self::Item> {
        if self.index + 1 < self.workspace.boundaries.len() {
            self.update_active_for_boundary(self.index);
            let start = self.workspace.boundaries[self.index] as usize;
            let end = self.workspace.boundaries[self.index + 1] as usize;
            self.index += 1;
            debug_assert!(start < end, "boundaries are sorted + deduped");

            return Some(start..end);
        }
        self.workspace.active.clear();
        None
    }
}

impl<T: Debug + TextStorage, Attr: Debug> ExactSizeIterator for AttributeSegments<'_, '_, T, Attr> {
    fn len(&self) -> usize {
        // Remaining segments are remaining adjacent boundary pairs: [i, i + 1).
        self.workspace
            .boundaries
            .len()
            .saturating_sub(self.index + 1)
    }
}

/// A view of the attribute spans active over a particular segment.
///
/// Provides iteration in both application order (ascending span id) and reverse
/// application order (descending span id — useful for last-writer-wins resolution).
#[derive(Clone, Debug)]
pub struct ActiveSpans<'s, 'a, T: Debug + TextStorage, Attr: Debug> {
    active_ids: &'s [u32],
    attributed: &'a AttributedText<T, Attr>,
}

/// Iterator over active spans in application order.
///
/// Obtain this by calling [`ActiveSpans::iter`] or by iterating `&ActiveSpans`
/// via [`IntoIterator`].
#[derive(Clone, Debug)]
pub struct ActiveSpansIter<'s, 'a, T: Debug + TextStorage, Attr: Debug> {
    ids: core::slice::Iter<'s, u32>,
    attributed: &'a AttributedText<T, Attr>,
}

impl<'s, 'a, T: Debug + TextStorage, Attr: Debug> Iterator for ActiveSpansIter<'s, 'a, T, Attr> {
    type Item = (&'a Range<usize>, &'a Attr);

    fn next(&mut self) -> Option<Self::Item> {
        let &attr_index = self.ids.next()?;
        Some(
            self.attributed
                .attribute_at_idx(attr_index as usize)
                .expect("span attribute index should be valid"),
        )
    }
}

impl<T: Debug + TextStorage, Attr: Debug> ExactSizeIterator for ActiveSpansIter<'_, '_, T, Attr> {}

impl<'s, 'a, T: Debug + TextStorage, Attr: Debug> DoubleEndedIterator
    for ActiveSpansIter<'s, 'a, T, Attr>
{
    fn next_back(&mut self) -> Option<Self::Item> {
        let &attr_index = self.ids.next_back()?;
        Some(
            self.attributed
                .attribute_at_idx(attr_index as usize)
                .expect("span attribute index should be valid"),
        )
    }
}

impl<'s, 'a, T: Debug + TextStorage, Attr: Debug> ActiveSpans<'s, 'a, T, Attr> {
    /// Iterate over the active spans in application order (ascending span id).
    ///
    /// Each item is `(&Range<usize>, &Attr)`.
    pub fn iter(&self) -> ActiveSpansIter<'_, 'a, T, Attr> {
        ActiveSpansIter {
            ids: self.active_ids.iter(),
            attributed: self.attributed,
        }
    }

    /// Returns `true` if no attribute spans are active in this segment.
    pub fn is_empty(&self) -> bool {
        self.active_ids.is_empty()
    }

    /// Returns the number of active attribute spans.
    pub fn len(&self) -> usize {
        self.active_ids.len()
    }
}

impl<'active, 's, 'a, T: Debug + TextStorage, Attr: Debug> IntoIterator
    for &'active ActiveSpans<'s, 'a, T, Attr>
{
    type Item = (&'a Range<usize>, &'a Attr);
    type IntoIter = ActiveSpansIter<'active, 'a, T, Attr>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TextRange;
    use alloc::vec;
    use alloc::vec::Vec;

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Color {
        Red,
        Blue,
        Green,
    }

    #[test]
    fn empty_text_yields_nothing() {
        let at = AttributedText::<&str, Color>::new("");
        let mut workspace = AttributeSegmentsWorkspace::new();
        let mut segments = workspace.segments(&at);
        assert!(segments.next().is_none());
    }

    #[test]
    fn no_attributes_yields_single_segment() {
        let at = AttributedText::<&str, Color>::new("hello");
        let mut workspace = AttributeSegmentsWorkspace::new();
        let mut segments = workspace.segments(&at);
        assert_eq!(segments.next(), Some(0..5));
        assert!(segments.active_spans().is_empty());
        assert_eq!(segments.next(), None);
    }

    #[test]
    fn size_hint_tracks_remaining_segments() {
        let mut at = AttributedText::new("hello");
        at.apply_attribute(TextRange::new(at.text(), 1..3).unwrap(), Color::Red);
        let mut workspace = AttributeSegmentsWorkspace::new();
        let mut segments = workspace.segments(&at);

        assert_eq!(segments.size_hint(), (3, Some(3)));
        assert_eq!(segments.next(), Some(0..1));
        assert_eq!(segments.size_hint(), (2, Some(2)));
        assert_eq!(segments.next(), Some(1..3));
        assert_eq!(segments.size_hint(), (1, Some(1)));
        assert_eq!(segments.next(), Some(3..5));
        assert_eq!(segments.size_hint(), (0, Some(0)));
        assert_eq!(segments.next(), None);
    }

    #[test]
    fn single_full_span() {
        let mut at = AttributedText::new("hello");
        at.apply_attribute(TextRange::new(at.text(), 0..5).unwrap(), Color::Red);
        let mut workspace = AttributeSegmentsWorkspace::new();
        let mut segments = workspace.segments(&at);
        assert_eq!(segments.next(), Some(0..5));
        let active: Vec<_> = segments.active_spans().iter().collect();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].1, &Color::Red);
        assert_eq!(segments.next(), None);
    }

    #[test]
    fn partial_span_splits_into_segments() {
        let mut at = AttributedText::new("hello");
        at.apply_attribute(TextRange::new(at.text(), 1..3).unwrap(), Color::Red);
        let mut workspace = AttributeSegmentsWorkspace::new();
        let mut segments = workspace.segments(&at);
        assert_eq!(segments.next(), Some(0..1));
        assert!(segments.active_spans().is_empty());
        assert_eq!(segments.next(), Some(1..3));
        assert_eq!(segments.active_spans().len(), 1);
        assert_eq!(segments.next(), Some(3..5));
        assert!(segments.active_spans().is_empty());
        assert_eq!(segments.next(), None);
    }

    #[test]
    fn overlapping_spans() {
        let mut at = AttributedText::new("abcdef");
        at.apply_attribute(TextRange::new(at.text(), 1..4).unwrap(), Color::Red);
        at.apply_attribute(TextRange::new(at.text(), 2..5).unwrap(), Color::Blue);
        let mut workspace = AttributeSegmentsWorkspace::new();
        let mut segments = workspace.segments(&at);
        assert_eq!(segments.next(), Some(0..1));
        assert!(segments.active_spans().is_empty());

        assert_eq!(segments.next(), Some(1..2));
        let a: Vec<_> = segments.active_spans().iter().map(|(_, c)| c).collect();
        assert_eq!(a, vec![&Color::Red]);

        assert_eq!(segments.next(), Some(2..4));
        let a: Vec<_> = segments.active_spans().iter().map(|(_, c)| c).collect();
        assert_eq!(a, vec![&Color::Red, &Color::Blue]);

        assert_eq!(segments.next(), Some(4..5));
        let a: Vec<_> = segments.active_spans().iter().map(|(_, c)| c).collect();
        assert_eq!(a, vec![&Color::Blue]);

        assert_eq!(segments.next(), Some(5..6));
        assert!(segments.active_spans().is_empty());
        assert_eq!(segments.next(), None);
    }

    #[test]
    fn application_order_preserved() {
        let mut at = AttributedText::new("abcdef");
        at.apply_attribute(TextRange::new(at.text(), 0..6).unwrap(), Color::Red);
        at.apply_attribute(TextRange::new(at.text(), 0..6).unwrap(), Color::Blue);
        at.apply_attribute(TextRange::new(at.text(), 0..6).unwrap(), Color::Green);
        let mut workspace = AttributeSegmentsWorkspace::new();
        let mut segments = workspace.segments(&at);
        assert_eq!(segments.next(), Some(0..6));

        let forward: Vec<_> = segments.active_spans().iter().map(|(_, c)| c).collect();
        assert_eq!(forward, vec![&Color::Red, &Color::Blue, &Color::Green]);

        let reverse: Vec<_> = segments
            .active_spans()
            .iter()
            .rev()
            .map(|(_, c)| c)
            .collect();
        assert_eq!(reverse, vec![&Color::Green, &Color::Blue, &Color::Red]);
        assert_eq!(segments.next(), None);
    }

    #[test]
    fn empty_range_attribute_is_skipped() {
        let mut at = AttributedText::new("hello");
        at.apply_attribute(TextRange::new(at.text(), 2..2).unwrap(), Color::Red);
        let mut workspace = AttributeSegmentsWorkspace::new();
        let mut segments = workspace.segments(&at);
        assert_eq!(segments.next(), Some(0..2));
        assert!(segments.active_spans().is_empty());
        assert_eq!(segments.next(), Some(2..5));
        assert!(segments.active_spans().is_empty());
        assert_eq!(segments.next(), None);
    }

    #[test]
    fn adjacent_non_overlapping_spans() {
        let mut at = AttributedText::new("abcdef");
        at.apply_attribute(TextRange::new(at.text(), 0..3).unwrap(), Color::Red);
        at.apply_attribute(TextRange::new(at.text(), 3..6).unwrap(), Color::Blue);
        let mut workspace = AttributeSegmentsWorkspace::new();
        let mut segments = workspace.segments(&at);
        assert_eq!(segments.next(), Some(0..3));
        let a: Vec<_> = segments.active_spans().iter().map(|(_, c)| c).collect();
        assert_eq!(a, vec![&Color::Red]);
        assert_eq!(segments.next(), Some(3..6));
        let a: Vec<_> = segments.active_spans().iter().map(|(_, c)| c).collect();
        assert_eq!(a, vec![&Color::Blue]);
        assert_eq!(segments.next(), None);
    }

    #[test]
    fn active_spans_is_empty_after_exhaustion() {
        let mut at = AttributedText::new("abc");
        at.apply_attribute(TextRange::new(at.text(), 0..3).unwrap(), Color::Red);
        let mut workspace = AttributeSegmentsWorkspace::new();
        let mut segments = workspace.segments(&at);

        assert_eq!(segments.next(), Some(0..3));
        assert_eq!(segments.active_spans().len(), 1);
        assert_eq!(segments.next(), None);
        assert!(segments.active_spans().is_empty());
    }

    #[test]
    fn active_spans_into_iter_works_for_reference() {
        let mut at = AttributedText::new("abcd");
        at.apply_attribute(TextRange::new(at.text(), 0..4).unwrap(), Color::Red);
        at.apply_attribute(TextRange::new(at.text(), 1..3).unwrap(), Color::Blue);
        let mut workspace = AttributeSegmentsWorkspace::new();
        let mut segments = workspace.segments(&at);

        assert_eq!(segments.next(), Some(0..1));
        let first: Vec<_> = (&segments.active_spans())
            .into_iter()
            .map(|(_, c)| c)
            .collect();
        assert_eq!(first, vec![&Color::Red]);

        assert_eq!(segments.next(), Some(1..3));
        let overlap: Vec<_> = (&segments.active_spans())
            .into_iter()
            .map(|(_, c)| c)
            .collect();
        assert_eq!(overlap, vec![&Color::Red, &Color::Blue]);
    }

    #[test]
    fn workspace_reuses_for_multiple_texts() {
        let mut workspace = AttributeSegmentsWorkspace::new();

        let mut a = AttributedText::new("abc");
        a.apply_attribute(TextRange::new(a.text(), 0..1).unwrap(), Color::Red);
        {
            let mut segments = workspace.segments(&a);
            assert_eq!(segments.next(), Some(0..1));
            let first: Vec<_> = segments.active_spans().iter().map(|(_, c)| c).collect();
            assert_eq!(first, vec![&Color::Red]);
            assert_eq!(segments.next(), Some(1..3));
            assert!(segments.active_spans().is_empty());
            assert_eq!(segments.next(), None);
        }

        let mut b = AttributedText::new("wxyz");
        b.apply_attribute(TextRange::new(b.text(), 1..4).unwrap(), Color::Blue);
        {
            let mut segments = workspace.segments(&b);
            assert_eq!(segments.next(), Some(0..1));
            assert!(segments.active_spans().is_empty());
            assert_eq!(segments.next(), Some(1..4));
            let second: Vec<_> = segments.active_spans().iter().map(|(_, c)| c).collect();
            assert_eq!(second, vec![&Color::Blue]);
            assert_eq!(segments.next(), None);
        }
    }
}
