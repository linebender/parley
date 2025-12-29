// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;
use core::fmt::Debug;
use core::ops::Range;

use attributed_text::TextStorage;
use text_style::InlineResolveContext;
use text_style::{ComputedInlineStyle, InlineStyle, ResolveStyleError};

use crate::text::StyledText;
use crate::traits::HasInlineStyle;

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
        Self {
            styled,
            boundaries,
            index: 0,
        }
    }

    pub(crate) fn compute_style(
        &self,
        start: usize,
        end: usize,
    ) -> Result<ComputedInlineStyle, ResolveStyleError> {
        let mut merged = InlineStyle::new();
        for (range, attr) in self.styled.attributed.attributes_iter() {
            if range.start < end && range.end > start {
                for declaration in attr.inline_style().declarations() {
                    merged.push_declaration(declaration.clone());
                }
            }
        }
        merged.resolve(InlineResolveContext::new(
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
            let start = self.boundaries[self.index];
            let end = self.boundaries[self.index + 1];
            self.index += 1;
            if start == end {
                continue;
            }

            return Some(self.compute_style(start, end).map(|style| InlineStyleRun {
                range: start..end,
                style,
            }));
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
}

impl<'a, T: Debug + TextStorage, A: Debug + HasInlineStyle> CoalescedInlineRuns<'a, T, A> {
    pub(crate) fn new(styled: &'a StyledText<T, A>) -> Self {
        Self {
            inner: ResolvedInlineRuns::new(styled),
        }
    }
}

impl<T: Debug + TextStorage, A: Debug + HasInlineStyle> Iterator for CoalescedInlineRuns<'_, T, A> {
    type Item = Result<InlineStyleRun, ResolveStyleError>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.inner.index + 1 < self.inner.boundaries.len() {
            let start = self.inner.boundaries[self.inner.index];
            let mut end = self.inner.boundaries[self.inner.index + 1];
            self.inner.index += 1;
            if start == end {
                continue;
            }

            let mut style = match self.inner.compute_style(start, end) {
                Ok(style) => style,
                Err(err) => {
                    // Propagate the error and terminate the iterator.
                    self.inner.index = self.inner.boundaries.len();
                    return Some(Err(err));
                }
            };

            while self.inner.index + 1 < self.inner.boundaries.len() {
                let next_start = self.inner.boundaries[self.inner.index];
                let next_end = self.inner.boundaries[self.inner.index + 1];
                if next_start == next_end {
                    self.inner.index += 1;
                    continue;
                }
                debug_assert_eq!(
                    next_start, end,
                    "run boundaries should be contiguous after dedup/sort"
                );
                let next_style = match self.inner.compute_style(next_start, next_end) {
                    Ok(s) => s,
                    Err(err) => {
                        self.inner.index = self.inner.boundaries.len();
                        return Some(Err(err));
                    }
                };
                if next_style != style {
                    break;
                }
                style = next_style;
                end = next_end;
                self.inner.index += 1;
            }

            return Some(Ok(InlineStyleRun {
                range: start..end,
                style,
            }));
        }
        None
    }
}
