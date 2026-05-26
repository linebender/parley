// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::fmt::Debug;

use attributed_text::{AttributeSegmentsWorkspace, TextRange, TextStorage};

use crate::{StyleId, StyledText};

/// A resolved styled segment.
///
/// Segments are non-empty, contiguous byte ranges. Their style is the effective
/// style identifier after resolving overlapping applied style spans. The last
/// writer is the most recently applied span that is active for the segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StyledSegment {
    range: TextRange,
    style: StyleId,
}

impl StyledSegment {
    /// Returns the byte range covered by this segment.
    #[must_use]
    #[inline]
    pub const fn range(self) -> TextRange {
        self.range
    }

    /// Returns the resolved compact style identifiers.
    #[must_use]
    #[inline]
    pub const fn style(self) -> StyleId {
        self.style
    }
}

/// Reusable allocation workspace for styled segment resolution.
///
/// Reuse this value across layout or painting passes to amortize allocation in
/// the underlying attributed-text segmentation step.
#[derive(Clone, Debug, Default)]
pub struct StyledSegmentsWorkspace {
    attributes: AttributeSegmentsWorkspace,
}

impl StyledSegmentsWorkspace {
    /// Creates an empty styled-segment workspace.
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Iterates over resolved styled segments.
    pub fn segments<'a, T, L, P>(
        &'a mut self,
        text: &'a StyledText<T, L, P>,
    ) -> impl Iterator<Item = StyledSegment> + 'a
    where
        T: Debug + TextStorage,
    {
        let base_style = text.base_style();
        let mut inner = self.attributes.segments(text.attributed());

        core::iter::from_fn(move || {
            let segment = inner.next_segment()?;
            let range = segment.range();
            // Active spans are in application order, so the last one wins.
            let style = segment
                .active_spans()
                .iter()
                .next_back()
                .map_or(base_style, |(_range, style)| *style);
            Some(StyledSegment { range, style })
        })
    }
}

#[cfg(test)]
mod tests {
    use alloc::sync::Arc;
    use alloc::vec;
    use alloc::vec::Vec;

    use super::StyledSegmentsWorkspace;
    use crate::{StyleSetBuilder, StyledText};

    #[test]
    fn resolves_last_applied_style() {
        let mut builder = StyleSetBuilder::<&'static str, &'static str>::new();
        let base = builder.intern_style("base", "black");
        let bold_red = builder.intern_style("bold", "red");
        let italic_blue = builder.intern_style("italic", "blue");
        let styles = Arc::new(builder.finish());

        let mut text = StyledText::new("abcd", styles, base);
        text.apply_style_bytes(0..3, bold_red)
            .expect("valid style range");
        text.apply_style_bytes(1..2, italic_blue)
            .expect("valid style range");

        let mut workspace = StyledSegmentsWorkspace::new();
        let segments = workspace
            .segments(&text)
            .map(|segment| (segment.range().as_range(), segment.style()))
            .collect::<Vec<_>>();

        assert_eq!(
            segments,
            vec![
                (0..1, bold_red),
                (1..2, italic_blue),
                (2..3, bold_red),
                (3..4, base),
            ]
        );
    }
}
