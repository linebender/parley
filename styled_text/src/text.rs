// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::sync::Arc;
use core::fmt::Debug;
use core::ops::Range;

use attributed_text::{AttributedText, Error, TextChunk, TextRange, TextStorage};

use crate::{StyleId, StyleSet};

/// Text plus compact style spans.
///
/// The text storage can be contiguous (`&str`, `String`, `Arc<str>`) or chunked.
/// Style payloads live in a shared [`StyleSet`], while each applied span stores
/// only compact identifiers.
#[derive(Debug)]
pub struct StyledText<T: Debug + TextStorage, L, P> {
    attributed: AttributedText<T, StyleId>,
    styles: Arc<StyleSet<L, P>>,
    base_style: StyleId,
}

impl<T: Debug + TextStorage, L, P> StyledText<T, L, P> {
    pub(crate) fn from_attributed_parts(
        attributed: AttributedText<T, StyleId>,
        styles: Arc<StyleSet<L, P>>,
        base_style: StyleId,
    ) -> Self {
        Self {
            attributed,
            styles,
            base_style,
        }
    }

    /// Creates styled text from text storage, a shared style set, and a base style.
    ///
    /// The base style identifier must come from `styles`.
    #[must_use]
    pub fn new(text: T, styles: Arc<StyleSet<L, P>>, base_style: StyleId) -> Self {
        debug_assert!(
            styles.get_style(base_style).is_some(),
            "base style id must be interned in styles"
        );
        Self::from_attributed_parts(AttributedText::new(text), styles, base_style)
    }

    /// Borrow the underlying attributed text.
    #[must_use]
    #[inline]
    pub const fn attributed(&self) -> &AttributedText<T, StyleId> {
        &self.attributed
    }

    /// Borrow the underlying text storage.
    #[must_use]
    #[inline]
    pub fn text(&self) -> &T {
        self.attributed.text()
    }

    /// Replaces the underlying text and clears applied style spans.
    #[inline]
    pub fn set_text(&mut self, text: T) {
        self.attributed.set_text(text);
    }

    /// Borrow the shared style set.
    #[must_use]
    #[inline]
    pub fn style_set(&self) -> &StyleSet<L, P> {
        &self.styles
    }

    /// Borrow the shared style set handle.
    #[must_use]
    #[inline]
    pub fn style_set_arc(&self) -> &Arc<StyleSet<L, P>> {
        &self.styles
    }

    /// Returns the base style used when no active span is present.
    #[must_use]
    #[inline]
    pub const fn base_style(&self) -> StyleId {
        self.base_style
    }

    /// Updates the base style used by segment resolution.
    ///
    /// The style identifier must come from this text's [`StyleSet`].
    #[inline]
    pub fn set_base_style(&mut self, base_style: StyleId) {
        self.debug_assert_style_is_in_set(base_style);
        self.base_style = base_style;
    }

    /// Returns the text length in bytes.
    #[must_use]
    #[inline]
    pub fn len(&self) -> usize {
        self.attributed.len()
    }

    /// Returns `true` if the underlying text is empty.
    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.attributed.is_empty()
    }

    /// Borrow the underlying text as `&str` when the storage is contiguous.
    #[must_use]
    #[inline]
    pub fn as_str(&self) -> Option<&str> {
        self.attributed.as_str()
    }

    /// Iterates over borrowed text chunks covering `range`.
    pub fn chunks(&self, range: TextRange) -> impl Iterator<Item = TextChunk<'_>> {
        self.attributed.chunks(range)
    }

    /// Validates a byte range against the underlying text storage.
    #[inline]
    pub fn validate_range(&self, range: Range<usize>) -> Result<TextRange, Error> {
        self.text().validate_range(range)
    }

    /// Applies compact style identifiers to a validated range.
    ///
    /// The style identifier must come from this text's [`StyleSet`].
    #[inline]
    pub fn apply_style(&mut self, range: TextRange, style: StyleId) {
        self.debug_assert_style_is_in_set(style);
        self.attributed.apply_attribute(range, style);
    }

    /// Applies compact style identifiers to a byte range after validation.
    ///
    /// The style identifier must come from this text's [`StyleSet`].
    #[inline]
    pub fn apply_style_bytes(&mut self, range: Range<usize>, style: StyleId) -> Result<(), Error> {
        let range = self.validate_range(range)?;
        self.apply_style(range, style);
        Ok(())
    }

    /// Removes all applied style spans.
    #[inline]
    pub fn clear_styles(&mut self) {
        self.attributed.clear_attributes();
    }

    /// Returns the number of applied style spans.
    #[must_use]
    #[inline]
    pub fn style_spans_len(&self) -> usize {
        self.attributed.attributes_len()
    }

    /// Iterates over all applied style spans in application order.
    #[inline]
    pub fn style_spans(&self) -> impl ExactSizeIterator<Item = (TextRange, &StyleId)> {
        self.attributed.attributes_iter()
    }

    fn debug_assert_style_is_in_set(&self, style: StyleId) {
        debug_assert!(
            self.styles.get_style(style).is_some(),
            "style id must be interned in this text's style set"
        );
    }
}

#[cfg(test)]
mod tests {
    use alloc::sync::Arc;

    use crate::{StyleSetBuilder, StyledText};

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "style id must be interned in this text's style set")]
    fn apply_style_rejects_out_of_set_style_in_debug() {
        let mut first = StyleSetBuilder::<u8, ()>::new();
        let base = first.intern_style(0, ());
        let styles = Arc::new(first.finish());

        let mut second = StyleSetBuilder::<u8, ()>::new();
        second.intern_style(0, ());
        let foreign = second.intern_style(1, ());

        let mut text = StyledText::new("abc", styles, base);
        let range = text.validate_range(0..1).expect("valid range");
        text.apply_style(range, foreign);
    }
}
