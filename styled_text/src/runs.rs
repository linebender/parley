// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec;
use alloc::vec::Vec;
use core::fmt::Debug;
use core::ops::Range;

use attributed_text::TextStorage;
use text_style::{InlineDeclaration, InlineStyle};
use text_style_resolve::{
    ComputedInlineStyle, InlineResolveContext, ResolveStyleError, ResolveStyleExt,
};

use crate::text::StyledText;
use crate::traits::HasInlineStyle;

#[derive(Clone, Debug)]
struct Span<'a> {
    declarations: &'a [InlineDeclaration],
}

const INLINE_DECL_KEY_COUNT: usize = 14;

fn inline_decl_key_index(decl: &InlineDeclaration) -> Option<usize> {
    match decl {
        InlineDeclaration::FontStack(_) => Some(0),
        InlineDeclaration::FontSize(_) => Some(1),
        InlineDeclaration::FontStyle(_) => Some(2),
        InlineDeclaration::FontWeight(_) => Some(3),
        InlineDeclaration::FontWidth(_) => Some(4),
        InlineDeclaration::FontVariations(_) => Some(5),
        InlineDeclaration::FontFeatures(_) => Some(6),
        InlineDeclaration::Locale(_) => Some(7),
        InlineDeclaration::Underline(_) => Some(8),
        InlineDeclaration::Strikethrough(_) => Some(9),
        InlineDeclaration::LineHeight(_) => Some(10),
        InlineDeclaration::WordSpacing(_) => Some(11),
        InlineDeclaration::LetterSpacing(_) => Some(12),
        InlineDeclaration::BidiControl(_) => Some(13),
        // `InlineDeclaration` is `#[non_exhaustive]`, so new variants can be added in `text_style`
        // without breaking `styled_text`. Returning `None` triggers a slower fallback path that
        // preserves correct "last writer wins" semantics for unknown properties.
        _ => None,
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
///
/// Each item is a `Result` because inline resolution can fail if any spans include declarations
/// that require parsing (for example OpenType settings supplied as a raw CSS-like string).
#[derive(Clone, Debug)]
pub struct ResolvedInlineRuns<'a, T: Debug + TextStorage, A: Debug + HasInlineStyle> {
    pub(crate) styled: &'a StyledText<T, A>,
    pub(crate) boundaries: Vec<usize>,
    start_events: Vec<Vec<usize>>,
    end_events: Vec<Vec<usize>>,
    spans: Vec<Span<'a>>,
    active: Vec<usize>,
    scratch: InlineStyle,
    pub(crate) index: usize,
}

impl<'a, T: Debug + TextStorage, A: Debug + HasInlineStyle> ResolvedInlineRuns<'a, T, A> {
    pub(crate) fn new(styled: &'a StyledText<T, A>) -> Self {
        let len = styled.attributed.len();
        let mut boundaries = Vec::new();
        boundaries.push(0);
        boundaries.push(len);
        for (range, _) in styled.attributed.attributes_iter() {
            boundaries.push(range.start);
            boundaries.push(range.end);
        }
        boundaries.sort_unstable();
        boundaries.dedup();

        let mut start_events = vec![Vec::new(); boundaries.len()];
        let mut end_events = vec![Vec::new(); boundaries.len()];
        let mut spans = Vec::new();

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

            let id = spans.len();
            spans.push(Span {
                declarations: attr.inline_style().declarations(),
            });
            start_events[start_boundary].push(id);
            end_events[end_boundary].push(id);
        }

        Self {
            styled,
            boundaries,
            start_events,
            end_events,
            spans,
            active: Vec::new(),
            scratch: InlineStyle::new(),
            index: 0,
        }
    }

    fn update_active_for_boundary(&mut self, boundary_index: usize) {
        for &id in &self.end_events[boundary_index] {
            if let Ok(ix) = self.active.binary_search(&id) {
                self.active.remove(ix);
            }
        }
        for &id in &self.start_events[boundary_index] {
            match self.active.binary_search(&id) {
                Ok(_) => {}
                Err(ix) => self.active.insert(ix, id),
            }
        }
    }

    fn compute_style_for_current_segment(
        &mut self,
    ) -> Result<ComputedInlineStyle, ResolveStyleError> {
        self.scratch.clear();

        let mut has_unknown = false;
        'outer: for &span_id in self.active.iter().rev() {
            let span = &self.spans[span_id];
            for decl in span.declarations.iter() {
                if inline_decl_key_index(decl).is_none() {
                    has_unknown = true;
                    break 'outer;
                }
            }
        }

        if has_unknown {
            // Preserve semantics for forward-compatible `InlineDeclaration` variants: merge all
            // active declarations in authoring order.
            for &span_id in &self.active {
                for decl in self.spans[span_id].declarations {
                    self.scratch.push_declaration(decl.clone());
                }
            }
        } else {
            let mut picked: [Option<InlineDeclaration>; INLINE_DECL_KEY_COUNT] =
                core::array::from_fn(|_| None);
            let mut remaining = INLINE_DECL_KEY_COUNT;

            for &span_id in self.active.iter().rev() {
                let span = &self.spans[span_id];
                for decl in span.declarations.iter().rev() {
                    let Some(idx) = inline_decl_key_index(decl) else {
                        continue;
                    };
                    if picked[idx].is_some() {
                        continue;
                    }
                    picked[idx] = Some(decl.clone());
                    remaining -= 1;
                    if remaining == 0 {
                        break;
                    }
                }
                if remaining == 0 {
                    break;
                }
            }

            for decl in picked.into_iter().flatten() {
                self.scratch.push_declaration(decl);
            }
        }

        self.scratch.resolve(InlineResolveContext::new(
            &self.styled.base_inline,
            &self.styled.initial_inline,
            &self.styled.root_inline,
        ))
    }
}

impl<T: Debug + TextStorage, A: Debug + HasInlineStyle> Iterator for ResolvedInlineRuns<'_, T, A> {
    type Item = Result<InlineStyleRun, ResolveStyleError>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.index + 1 < self.boundaries.len() {
            self.update_active_for_boundary(self.index);
            let start = self.boundaries[self.index];
            let end = self.boundaries[self.index + 1];
            self.index += 1;
            if start == end {
                continue;
            }

            return Some(
                self.compute_style_for_current_segment()
                    .map(|style| InlineStyleRun {
                        range: start..end,
                        style,
                    }),
            );
        }
        None
    }
}

/// An iterator over coalesced resolved inline style runs.
///
/// Coalescing stops early if an error is encountered; the error is returned as the final item.
#[derive(Clone, Debug)]
pub struct CoalescedInlineRuns<'a, T: Debug + TextStorage, A: Debug + HasInlineStyle> {
    pub(crate) inner: ResolvedInlineRuns<'a, T, A>,
    pending: Option<InlineStyleRun>,
    terminated: bool,
}

impl<'a, T: Debug + TextStorage, A: Debug + HasInlineStyle> CoalescedInlineRuns<'a, T, A> {
    pub(crate) fn new(styled: &'a StyledText<T, A>) -> Self {
        Self {
            inner: ResolvedInlineRuns::new(styled),
            pending: None,
            terminated: false,
        }
    }
}

impl<T: Debug + TextStorage, A: Debug + HasInlineStyle> Iterator for CoalescedInlineRuns<'_, T, A> {
    type Item = Result<InlineStyleRun, ResolveStyleError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.terminated {
            return None;
        }

        let mut run = match self.pending.take().map(Ok).or_else(|| self.inner.next())? {
            Ok(run) => run,
            Err(err) => {
                self.terminated = true;
                return Some(Err(err));
            }
        };

        loop {
            match self.inner.next() {
                None => break,
                Some(Err(err)) => {
                    self.terminated = true;
                    return Some(Err(err));
                }
                Some(Ok(next_run)) => {
                    if next_run.range.start == run.range.end && next_run.style == run.style {
                        run.range.end = next_run.range.end;
                        continue;
                    }
                    self.pending = Some(next_run);
                    break;
                }
            }
        }

        Some(Ok(run))
    }
}
