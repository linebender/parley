// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec;
use alloc::vec::Vec;
use core::fmt::Debug;
use core::ops::Range;

use crate::resolve::resolve_inline_declarations;
use crate::{ComputedInlineStyle, InlineDeclaration, InlineResolveContext};
use attributed_text::TextStorage;

use crate::text::StyledText;
use crate::traits::HasInlineStyle;

#[derive(Clone, Debug)]
struct Span<'a> {
    declarations: &'a [InlineDeclaration],
}

const INLINE_DECL_KEY_COUNT: usize = 14;

#[inline]
fn inline_decl_key_index(decl: &InlineDeclaration) -> usize {
    match decl {
        InlineDeclaration::FontFamily(_) => 0,
        InlineDeclaration::FontSize(_) => 1,
        InlineDeclaration::FontStyle(_) => 2,
        InlineDeclaration::FontWeight(_) => 3,
        InlineDeclaration::FontWidth(_) => 4,
        InlineDeclaration::FontVariations(_) => 5,
        InlineDeclaration::FontFeatures(_) => 6,
        InlineDeclaration::Locale(_) => 7,
        InlineDeclaration::Underline(_) => 8,
        InlineDeclaration::Strikethrough(_) => 9,
        InlineDeclaration::LineHeight(_) => 10,
        InlineDeclaration::WordSpacing(_) => 11,
        InlineDeclaration::LetterSpacing(_) => 12,
        InlineDeclaration::BidiControl(_) => 13,
    }
}

/// A resolved inline style run for a contiguous text range.
#[derive(Clone, Debug, PartialEq)]
pub struct InlineStyleRun {
    /// The byte range in the underlying text.
    pub range: Range<usize>,
    /// The computed inline style for this range.
    pub style: ComputedInlineStyle,
}

/// An iterator over resolved inline style runs.
#[derive(Clone, Debug)]
pub struct ResolvedInlineRuns<'a, T: Debug + TextStorage, A: Debug + HasInlineStyle> {
    pub(crate) styled: &'a StyledText<T, A>,
    pub(crate) boundaries: Vec<usize>,
    start_offsets: Vec<usize>,
    start_events: Vec<usize>,
    end_offsets: Vec<usize>,
    end_events: Vec<usize>,
    spans: Vec<Span<'a>>,
    active: Vec<usize>,
    pub(crate) index: usize,
}

impl<'a, T: Debug + TextStorage, A: Debug + HasInlineStyle> ResolvedInlineRuns<'a, T, A> {
    pub(crate) fn new(styled: &'a StyledText<T, A>) -> Self {
        let len = styled.attributed.len();
        let attr_count = styled.attributed.attributes_len();
        // Each attribute can contribute up to two boundaries (start/end), plus the implicit 0/len.
        let mut boundaries = Vec::with_capacity(2 + attr_count.saturating_mul(2));
        boundaries.push(0);
        boundaries.push(len);
        for (range, _) in styled.attributed.attributes_iter() {
            boundaries.push(range.start);
            boundaries.push(range.end);
        }
        boundaries.sort_unstable();
        boundaries.dedup();

        let boundary_count = boundaries.len();

        let mut spans = Vec::with_capacity(attr_count);
        let mut span_boundaries = Vec::with_capacity(attr_count);

        // We build start/end event lists keyed by boundary index. Instead of a `Vec<Vec<usize>>`
        // (which would allocate an inner `Vec` for each boundary), we use a
        // CSR (Compressed Sparse Row)-style layout:
        // a single flat event buffer plus an offsets array giving the slice for each boundary.
        //
        // This represents "many small lists" without lots of tiny heap allocations.
        //
        // We store each span's (start_boundary, end_boundary) indices to avoid allocating
        // separate `span_starts`/`span_ends` arrays.
        let mut start_counts = vec![0_usize; boundary_count];
        let mut end_counts = vec![0_usize; boundary_count];
        for (range, attr) in styled.attributed.attributes_iter() {
            if range.start == range.end {
                continue;
            }
            let start_boundary = boundaries
                .binary_search(&range.start)
                .expect("attribute boundary start should be in boundary list");
            let end_boundary = boundaries
                .binary_search(&range.end)
                .expect("attribute boundary end should be in boundary list");
            if start_boundary == end_boundary {
                continue;
            }

            spans.push(Span {
                declarations: attr.inline_style().declarations(),
            });
            span_boundaries.push((start_boundary, end_boundary));
            start_counts[start_boundary] += 1;
            end_counts[end_boundary] += 1;
        }

        let mut start_offsets = vec![0_usize; boundary_count + 1];
        let mut end_offsets = vec![0_usize; boundary_count + 1];
        for i in 0..boundary_count {
            start_offsets[i + 1] = start_offsets[i] + start_counts[i];
            end_offsets[i + 1] = end_offsets[i] + end_counts[i];
        }

        let mut start_events = vec![0_usize; start_offsets[boundary_count]];
        let mut end_events = vec![0_usize; end_offsets[boundary_count]];

        // Reuse `*_counts` as per-boundary write cursors to fill the CSR event buffers without
        // allocating additional `*_next` arrays.
        start_counts.fill(0);
        end_counts.fill(0);
        for (id, (start_boundary, end_boundary)) in span_boundaries.iter().copied().enumerate() {
            let start_ix = start_offsets[start_boundary] + start_counts[start_boundary];
            start_events[start_ix] = id;
            start_counts[start_boundary] += 1;

            let end_ix = end_offsets[end_boundary] + end_counts[end_boundary];
            end_events[end_ix] = id;
            end_counts[end_boundary] += 1;
        }

        let span_len = spans.len();
        Self {
            styled,
            boundaries,
            start_offsets,
            start_events,
            end_offsets,
            end_events,
            spans,
            // In the worst case, all spans could overlap a single boundary segment.
            active: Vec::with_capacity(span_len),
            index: 0,
        }
    }

    fn update_active_for_boundary(&mut self, boundary_index: usize) {
        let end_range = self.end_offsets[boundary_index]..self.end_offsets[boundary_index + 1];
        for &id in &self.end_events[end_range] {
            if let Ok(ix) = self.active.binary_search(&id) {
                self.active.remove(ix);
            }
        }
        let start_range =
            self.start_offsets[boundary_index]..self.start_offsets[boundary_index + 1];
        for &id in &self.start_events[start_range] {
            match self.active.binary_search(&id) {
                Ok(_) => {}
                Err(ix) => self.active.insert(ix, id),
            }
        }
    }

    fn compute_style_for_current_segment(&mut self) -> ComputedInlineStyle {
        let mut picked: [Option<&InlineDeclaration>; INLINE_DECL_KEY_COUNT] =
            core::array::from_fn(|_| None);
        let mut remaining = INLINE_DECL_KEY_COUNT;

        for &span_id in self.active.iter().rev() {
            let span = &self.spans[span_id];
            for decl in span.declarations.iter().rev() {
                let idx = inline_decl_key_index(decl);
                if picked[idx].is_some() {
                    continue;
                }
                picked[idx] = Some(decl);
                remaining -= 1;
                if remaining == 0 {
                    break;
                }
            }
            if remaining == 0 {
                break;
            }
        }

        resolve_inline_declarations(
            picked.into_iter().flatten(),
            InlineResolveContext::new(
                &self.styled.base_inline,
                &self.styled.initial_inline,
                &self.styled.root_inline,
            ),
        )
    }
}

impl<T: Debug + TextStorage, A: Debug + HasInlineStyle> Iterator for ResolvedInlineRuns<'_, T, A> {
    type Item = InlineStyleRun;

    fn next(&mut self) -> Option<Self::Item> {
        while self.index + 1 < self.boundaries.len() {
            self.update_active_for_boundary(self.index);
            let start = self.boundaries[self.index];
            let end = self.boundaries[self.index + 1];
            self.index += 1;
            if start == end {
                continue;
            }

            return Some(InlineStyleRun {
                range: start..end,
                style: self.compute_style_for_current_segment(),
            });
        }
        None
    }
}

/// An iterator over coalesced resolved inline style runs.
#[derive(Clone, Debug)]
pub struct CoalescedInlineRuns<'a, T: Debug + TextStorage, A: Debug + HasInlineStyle> {
    pub(crate) inner: ResolvedInlineRuns<'a, T, A>,
    pending: Option<InlineStyleRun>,
}

impl<'a, T: Debug + TextStorage, A: Debug + HasInlineStyle> CoalescedInlineRuns<'a, T, A> {
    pub(crate) fn new(styled: &'a StyledText<T, A>) -> Self {
        Self {
            inner: ResolvedInlineRuns::new(styled),
            pending: None,
        }
    }
}

impl<T: Debug + TextStorage, A: Debug + HasInlineStyle> Iterator for CoalescedInlineRuns<'_, T, A> {
    type Item = InlineStyleRun;

    fn next(&mut self) -> Option<Self::Item> {
        let mut run = self.pending.take().or_else(|| self.inner.next())?;

        loop {
            match self.inner.next() {
                None => break,
                Some(next_run) => {
                    if next_run.range.start == run.range.end && next_run.style == run.style {
                        run.range.end = next_run.range.end;
                        continue;
                    }
                    self.pending = Some(next_run);
                    break;
                }
            }
        }

        Some(run)
    }
}
