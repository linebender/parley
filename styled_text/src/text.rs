// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::fmt::Debug;
use core::ops::Range;

use crate::{
    ComputedInlineStyle, ComputedParagraphStyle, ParagraphResolveContext, ParagraphStyle,
    ResolveStyleExt,
};
use attributed_text::{AttributedText, Error, TextRange, TextStorage};

use crate::runs::{CoalescedInlineRuns, ResolvedInlineRuns};
use crate::traits::HasInlineStyle;

/// A single layout block worth of styled text.
///
/// This is designed to map 1:1 to a single Parley `Layout` ("one paragraph per layout").
#[derive(Debug)]
pub struct StyledText<T: Debug + TextStorage, A: Debug> {
    pub(crate) attributed: AttributedText<T, A>,
    pub(crate) paragraph_style: ParagraphStyle,
    pub(crate) base_inline: ComputedInlineStyle,
    pub(crate) initial_inline: ComputedInlineStyle,
    pub(crate) root_inline: ComputedInlineStyle,
    pub(crate) base_paragraph: ComputedParagraphStyle,
    pub(crate) initial_paragraph: ComputedParagraphStyle,
    pub(crate) root_paragraph: ComputedParagraphStyle,
}

impl<T: Debug + TextStorage, A: Debug> StyledText<T, A> {
    /// Creates a new `StyledText` with base styles.
    ///
    /// The base styles also serve as the "initial" values for [`Specified::Initial`](crate::Specified::Initial).
    #[inline]
    pub fn new(
        text: T,
        base_inline: ComputedInlineStyle,
        base_paragraph: ComputedParagraphStyle,
    ) -> Self {
        Self::new_with_initial(
            text,
            base_inline.clone(),
            base_inline,
            base_paragraph.clone(),
            base_paragraph,
        )
    }

    /// Creates a new `StyledText` with separate base and initial styles.
    #[inline]
    pub fn new_with_initial(
        text: T,
        base_inline: ComputedInlineStyle,
        initial_inline: ComputedInlineStyle,
        base_paragraph: ComputedParagraphStyle,
        initial_paragraph: ComputedParagraphStyle,
    ) -> Self {
        Self {
            attributed: AttributedText::new(text),
            paragraph_style: ParagraphStyle::new(),
            base_inline,
            root_inline: initial_inline.clone(),
            initial_inline,
            base_paragraph,
            root_paragraph: initial_paragraph.clone(),
            initial_paragraph,
        }
    }

    /// Returns the underlying text as `&str` when the storage is contiguous.
    #[inline]
    pub fn as_str(&self) -> &str
    where
        T: AsRef<str>,
    {
        self.attributed.as_str()
    }

    /// Returns the base computed inline style used for run resolution.
    #[inline]
    pub fn base_inline_style(&self) -> &ComputedInlineStyle {
        &self.base_inline
    }

    /// Returns the base computed paragraph style used for paragraph resolution.
    #[inline]
    pub fn base_paragraph_style(&self) -> &ComputedParagraphStyle {
        &self.base_paragraph
    }

    /// Sets the paragraph style declarations for this block.
    pub fn set_paragraph_style(&mut self, style: ParagraphStyle) {
        self.paragraph_style = style;
    }

    /// Returns the paragraph style declarations for this block.
    #[inline]
    pub fn paragraph_style(&self) -> &ParagraphStyle {
        &self.paragraph_style
    }

    /// Returns the computed paragraph style for this block.
    pub fn computed_paragraph_style(&self) -> ComputedParagraphStyle {
        self.paragraph_style.resolve(ParagraphResolveContext::new(
            &self.base_paragraph,
            &self.initial_paragraph,
            &self.root_paragraph,
        ))
    }

    /// Sets the root computed styles used for root-relative units such as `rem`.
    pub fn set_root_styles(
        &mut self,
        root_inline: ComputedInlineStyle,
        root_paragraph: ComputedParagraphStyle,
    ) {
        self.root_inline = root_inline;
        self.root_paragraph = root_paragraph;
    }

    /// Returns the root inline style used for root-relative units such as `rem`.
    #[inline]
    pub fn root_inline_style(&self) -> &ComputedInlineStyle {
        &self.root_inline
    }

    /// Returns the root paragraph style used for root-relative properties.
    #[inline]
    pub fn root_paragraph_style(&self) -> &ComputedParagraphStyle {
        &self.root_paragraph
    }

    /// Applies a span attribute to a validated [`TextRange`].
    #[inline]
    pub fn apply_span(&mut self, range: TextRange, attribute: A) {
        self.attributed.apply_attribute(range, attribute);
    }

    /// Clears all applied span attributes, retaining allocated storage.
    #[inline]
    pub fn clear_spans(&mut self) {
        self.attributed.clear_attributes();
    }

    /// Clears the paragraph style declarations, retaining allocated storage.
    #[inline]
    pub fn clear_paragraph_style(&mut self) {
        self.paragraph_style.clear();
    }

    /// Replaces the underlying text and clears span attributes and paragraph declarations.
    ///
    /// This retains allocated storage for spans and declarations so the same `StyledText` value
    /// can be reused across rebuilds.
    #[inline]
    pub fn set_text(&mut self, text: T) {
        self.attributed.set_text(text);
        self.paragraph_style.clear();
    }

    /// Applies a span attribute to the specified byte `range`.
    ///
    /// This validates the range (bounds + UTF-8 codepoint boundaries) before applying it.
    #[inline]
    pub fn apply_span_bytes(&mut self, range: Range<usize>, attribute: A) -> Result<(), Error> {
        self.attributed.apply_attribute_bytes(range, attribute)
    }

    /// Validates a byte `range` against this text and returns a [`TextRange`].
    #[inline]
    pub fn range(&self, range: Range<usize>) -> Result<TextRange, Error> {
        TextRange::new(self.attributed.text(), range)
    }
}

impl<T: Debug + TextStorage, A: Debug + HasInlineStyle> StyledText<T, A> {
    /// Returns an iterator over resolved inline style runs.
    ///
    /// Overlapping spans are applied in the order they were added (last writer wins).
    #[inline]
    pub fn resolved_inline_runs(&self) -> ResolvedInlineRuns<'_, T, A> {
        ResolvedInlineRuns::new(self)
    }

    /// Returns an iterator over resolved inline style runs, coalescing adjacent runs with the same
    /// computed style.
    #[inline]
    pub fn resolved_inline_runs_coalesced(&self) -> CoalescedInlineRuns<'_, T, A> {
        CoalescedInlineRuns::new(self)
    }
}
