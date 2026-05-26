// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ops::Range;

use attributed_text::{AttributeSegmentsWorkspace, AttributedText, Error, TextRange, TextStorage};

use crate::{StyleId, StyleSetBuilder, StyledText};

/// A partial style patch that can be applied to full layout and paint styles.
///
/// This trait is intentionally generic. Toolkits can define their own patch
/// type with whatever fields make sense, then merge those patches into their
/// own full style payloads during [`StyledTextBuilder::finish`].
pub trait StylePatch<L, P> {
    /// Applies this patch to the current full layout and paint styles.
    fn apply_to(&self, layout: &mut L, paint: &mut P);
}

impl<L, P> StylePatch<L, P> for () {
    #[inline]
    fn apply_to(&self, _layout: &mut L, _paint: &mut P) {}
}

/// Builder for constructing compact styled text from text and style patches.
///
/// This is the higher-level construction API. Callers append text and record
/// partial style patches against byte ranges. At [`finish`](Self::finish), the
/// builder composes overlapping patches in the order they were applied,
/// interns the resulting full layout and paint payloads, then returns
/// [`StyledText`].
///
/// The resolved output remains the low-level representation: text plus compact
/// [`StyleId`] spans. Individual style fields are a construction detail of the
/// caller's patch type; the stored spans refer to complete styles.
#[derive(Debug)]
pub struct StyledTextBuilder<L, P, Patch = ()> {
    text: String,
    patches: Vec<(TextRange, Patch)>,
    base_layout: L,
    base_paint: P,
}

impl<L, P, Patch> StyledTextBuilder<L, P, Patch> {
    /// Creates an empty builder with base layout and paint styles.
    #[must_use]
    pub fn new(base_layout: L, base_paint: P) -> Self {
        Self {
            text: String::new(),
            patches: Vec::new(),
            base_layout,
            base_paint,
        }
    }

    /// Creates an empty builder with retained capacity for text and patches.
    #[must_use]
    pub fn with_capacity(
        base_layout: L,
        base_paint: P,
        text_capacity: usize,
        patch_capacity: usize,
    ) -> Self {
        Self {
            text: String::with_capacity(text_capacity),
            patches: Vec::with_capacity(patch_capacity),
            base_layout,
            base_paint,
        }
    }

    /// Reserves capacity for additional text bytes and style patches.
    ///
    /// This is useful when the text length is known but the patch type should be
    /// inferred from later [`push_with`](Self::push_with) or [`apply`](Self::apply)
    /// calls.
    pub fn reserve(&mut self, additional_text: usize, additional_patches: usize) {
        self.text.reserve(additional_text);
        self.patches.reserve(additional_patches);
    }

    /// Creates a builder from existing text.
    #[must_use]
    pub fn from_text(text: impl Into<String>, base_layout: L, base_paint: P) -> Self {
        Self {
            text: text.into(),
            patches: Vec::new(),
            base_layout,
            base_paint,
        }
    }

    /// Returns the current text.
    #[must_use]
    #[inline]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the current text length in bytes.
    #[must_use]
    #[inline]
    pub fn len(&self) -> usize {
        self.text.len()
    }

    /// Returns `true` if the current text is empty.
    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Returns the number of applied style patches.
    #[must_use]
    #[inline]
    pub fn patch_len(&self) -> usize {
        self.patches.len()
    }

    /// Validates a byte range against the current text.
    #[inline]
    pub fn validate_range(&self, range: Range<usize>) -> Result<TextRange, Error> {
        self.text.validate_range(range)
    }

    /// Appends unstyled text and returns its range.
    ///
    /// "Unstyled" means no additional patch is applied; the range still inherits
    /// the base style and any later overlapping patches.
    pub fn push(&mut self, text: &str) -> TextRange {
        let start = self.text.len();
        self.text.push_str(text);
        TextRange::new_unchecked(start, self.text.len())
    }

    /// Applies a patch to a validated range.
    pub fn apply(&mut self, range: TextRange, patch: Patch) {
        if !range.is_empty() {
            self.patches.push((range, patch));
        }
    }

    /// Applies a patch to a byte range after validation.
    pub fn apply_bytes(&mut self, range: Range<usize>, patch: Patch) -> Result<(), Error> {
        let range = self.validate_range(range)?;
        self.apply(range, patch);
        Ok(())
    }

    /// Appends text, applies a patch to that appended range, and returns the range.
    pub fn push_with(&mut self, text: &str, patch: Patch) -> TextRange {
        let range = self.push(text);
        self.apply(range, patch);
        range
    }
}

impl<L, P, Patch> StyledTextBuilder<L, P, Patch>
where
    L: Clone + PartialEq,
    P: Clone + PartialEq,
    Patch: StylePatch<L, P>,
{
    /// Finishes the builder and returns compact resolved styled text.
    ///
    /// Overlapping patches are applied in the order they were applied to the
    /// builder over a clone of the base layout and paint styles for each
    /// resolved non-base segment.
    #[must_use]
    pub fn finish(self) -> StyledText<String, L, P> {
        let Self {
            text,
            patches,
            base_layout,
            base_paint,
        } = self;

        let mut style_builder = StyleSetBuilder::with_capacity(patches.len().saturating_add(1));
        let base_style = style_builder.intern_style(base_layout.clone(), base_paint.clone());

        let resolved_text = if patches.is_empty() {
            AttributedText::new(text)
        } else {
            let mut resolved_spans = Vec::new();
            let mut workspace = AttributeSegmentsWorkspace::new();
            let mut pending: Option<(TextRange, StyleId)> = None;

            workspace.for_each_span_segment_unchecked(text.len(), &patches, |range, active| {
                if active.is_empty() {
                    if let Some(pending) = pending.take() {
                        resolved_spans.push(pending);
                    }
                    return;
                }

                let mut layout = base_layout.clone();
                let mut paint = base_paint.clone();
                for &patch_index in active {
                    patches[patch_index as usize]
                        .1
                        .apply_to(&mut layout, &mut paint);
                }

                let style = style_builder.intern_style(layout, paint);
                if style == base_style {
                    if let Some(pending) = pending.take() {
                        resolved_spans.push(pending);
                    }
                    return;
                }

                match pending.take() {
                    Some((pending_range, pending_style))
                        if pending_style == style && pending_range.end() == range.start() =>
                    {
                        pending = Some((
                            TextRange::new_unchecked(pending_range.start(), range.end()),
                            style,
                        ));
                    }
                    Some(pending_span) => {
                        resolved_spans.push(pending_span);
                        pending = Some((range, style));
                    }
                    None => {
                        pending = Some((range, style));
                    }
                }
            });

            if let Some(pending) = pending {
                resolved_spans.push(pending);
            }

            AttributedText::from_attributes_unchecked(text, resolved_spans)
        };

        StyledText::from_attributed_parts(
            resolved_text,
            Arc::new(style_builder.finish()),
            base_style,
        )
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;
    use alloc::vec::Vec;
    use core::sync::atomic::{AtomicUsize, Ordering};

    use crate::{StylePatch, StyledSegmentsWorkspace, StyledTextBuilder};

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Layout {
        size: u8,
        underline: bool,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Paint(u8);

    #[derive(Clone, Copy, Default)]
    struct Patch {
        size: Option<u8>,
        underline: Option<bool>,
        paint: Option<u8>,
    }

    impl StylePatch<Layout, Paint> for Patch {
        fn apply_to(&self, layout: &mut Layout, paint: &mut Paint) {
            if let Some(size) = self.size {
                layout.size = size;
            }
            if let Some(underline) = self.underline {
                layout.underline = underline;
            }
            if let Some(color) = self.paint {
                paint.0 = color;
            }
        }
    }

    static LAYOUT_CLONES: AtomicUsize = AtomicUsize::new(0);

    #[derive(Debug, PartialEq, Eq)]
    struct CountedLayout(u8);

    impl Clone for CountedLayout {
        fn clone(&self) -> Self {
            LAYOUT_CLONES.fetch_add(1, Ordering::Relaxed);
            Self(self.0)
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct CountedPaint(u8);

    #[derive(Clone, Copy)]
    struct CountedPatch(u8);

    impl StylePatch<CountedLayout, CountedPaint> for CountedPatch {
        fn apply_to(&self, layout: &mut CountedLayout, _paint: &mut CountedPaint) {
            layout.0 = self.0;
        }
    }

    #[test]
    fn builder_merges_overlapping_patches_by_property() {
        let base_layout = Layout {
            size: 16,
            underline: false,
        };
        let mut builder = StyledTextBuilder::new(base_layout, Paint(1));
        let all = builder.push("abcd");
        builder.apply(
            all,
            Patch {
                size: Some(12),
                ..Patch::default()
            },
        );
        builder
            .apply_bytes(
                1..3,
                Patch {
                    underline: Some(true),
                    paint: Some(2),
                    ..Patch::default()
                },
            )
            .expect("valid range");

        let styled = builder.finish();
        let mut workspace = StyledSegmentsWorkspace::new();
        let segments = workspace
            .segments(&styled)
            .map(|segment| {
                let style = styled.style_set().segment_style(segment.style());
                (segment.range().as_range(), *style.layout(), *style.paint())
            })
            .collect::<Vec<_>>();

        assert_eq!(
            segments,
            vec![
                (
                    0..1,
                    Layout {
                        size: 12,
                        underline: false
                    },
                    Paint(1)
                ),
                (
                    1..3,
                    Layout {
                        size: 12,
                        underline: true
                    },
                    Paint(2)
                ),
                (
                    3..4,
                    Layout {
                        size: 12,
                        underline: false
                    },
                    Paint(1)
                ),
            ]
        );
    }

    #[test]
    fn builder_skips_base_style_clone_for_unpatched_segments() {
        let mut builder = StyledTextBuilder::new(CountedLayout(1), CountedPaint(1));
        builder.push("abcdef");
        builder
            .apply_bytes(2..4, CountedPatch(2))
            .expect("valid range");

        LAYOUT_CLONES.store(0, Ordering::Relaxed);
        let styled = builder.finish();

        assert_eq!(LAYOUT_CLONES.load(Ordering::Relaxed), 2);
        assert_eq!(styled.style_set().style_len(), 2);
    }
}
