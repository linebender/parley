// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;
use core::fmt;

/// Identifier for an interned layout-affecting style payload.
///
/// Layout styles are intended for data that can affect shaping or line layout,
/// such as font size, font family, spacing, or wrapping policy.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LayoutStyleId(u32);

impl LayoutStyleId {
    fn from_index(index: usize) -> Self {
        let index = u32::try_from(index).expect("too many interned layout styles");
        Self(index)
    }

    /// Returns this identifier as a zero-based table index.
    #[must_use]
    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

impl fmt::Debug for LayoutStyleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("LayoutStyleId").field(&self.0).finish()
    }
}

/// Identifier for an interned paint-only style payload.
///
/// Paint styles are intended for data that should not invalidate shaping, such
/// as color or renderer-specific paint metadata.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PaintStyleId(u32);

impl PaintStyleId {
    fn from_index(index: usize) -> Self {
        let index = u32::try_from(index).expect("too many interned paint styles");
        Self(index)
    }

    /// Returns this identifier as a zero-based table index.
    #[must_use]
    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

impl fmt::Debug for PaintStyleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("PaintStyleId").field(&self.0).finish()
    }
}

/// Identifier for an interned full style.
///
/// This is the compact style identity stored on text spans and resolved
/// segments. Internally, each full style points at separately interned layout
/// and paint payloads.
///
/// An id is an index into one specific [`StyleSet`] and carries no provenance.
/// Using it with a different set resolves to an unrelated style or, in
/// [`StyleSet::segment_style`], panics. Only use an id with the set it came from.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StyleId(u32);

impl StyleId {
    fn from_index(index: usize) -> Self {
        let index = u32::try_from(index).expect("too many interned styles");
        Self(index)
    }

    /// Returns this identifier as a zero-based table index.
    #[must_use]
    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

impl fmt::Debug for StyleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("StyleId").field(&self.0).finish()
    }
}

/// Interned component identifiers for a full style.
///
/// This record is exposed for diagnostics, invalidation decisions, and adapters
/// that need to know whether two full styles share the same layout or paint
/// payload. Text spans should store [`StyleId`], not this component record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StyleRecord {
    layout: LayoutStyleId,
    paint: PaintStyleId,
}

impl StyleRecord {
    /// Returns the interned layout payload identifier.
    #[must_use]
    #[inline]
    pub const fn layout_id(self) -> LayoutStyleId {
        self.layout
    }

    /// Returns the interned paint payload identifier.
    #[must_use]
    #[inline]
    pub const fn paint_id(self) -> PaintStyleId {
        self.paint
    }
}

/// Interned style payloads used by [`StyledText`](crate::StyledText).
///
/// Text spans use one compact [`StyleId`]. This set keeps full style identity
/// stable while still deduplicating layout and paint payloads separately.
#[derive(Clone, Debug, Default)]
pub struct StyleSet<L, P> {
    styles: Vec<StyleRecord>,
    layout: Vec<L>,
    paint: Vec<P>,
}

impl<L, P> StyleSet<L, P> {
    /// Creates an empty style set.
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self {
            styles: Vec::new(),
            layout: Vec::new(),
            paint: Vec::new(),
        }
    }

    /// Returns the interned full-style record for `id`.
    #[must_use]
    #[inline]
    pub fn get_style(&self, id: StyleId) -> Option<StyleRecord> {
        self.styles.get(id.index()).copied()
    }

    /// Returns the interned layout payload for `id`.
    #[must_use]
    #[inline]
    pub fn get_layout(&self, id: LayoutStyleId) -> Option<&L> {
        self.layout.get(id.index())
    }

    /// Returns the interned paint payload for `id`.
    #[must_use]
    #[inline]
    pub fn get_paint(&self, id: PaintStyleId) -> Option<&P> {
        self.paint.get(id.index())
    }

    /// Returns the number of interned full styles.
    #[must_use]
    #[inline]
    pub fn style_len(&self) -> usize {
        self.styles.len()
    }

    /// Returns the number of interned layout payloads.
    #[must_use]
    #[inline]
    pub fn layout_len(&self) -> usize {
        self.layout.len()
    }

    /// Returns the number of interned paint payloads.
    #[must_use]
    #[inline]
    pub fn paint_len(&self) -> usize {
        self.paint.len()
    }

    /// Returns `true` if there are no interned full styles.
    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.styles.is_empty()
    }

    /// Iterates over all full-style identifiers in table order.
    pub fn style_ids(&self) -> impl ExactSizeIterator<Item = StyleId> + Clone {
        (0..self.styles.len()).map(StyleId::from_index)
    }

    /// Resolves a compact style identifier into borrowed payloads from this set,
    /// returning `None` if the id is not interned here.
    ///
    /// Use this for ids that may not belong to this set; see
    /// [`segment_style`](Self::segment_style) for the panicking convenience.
    #[must_use]
    pub fn try_segment_style(&self, id: StyleId) -> Option<SegmentStyle<'_, L, P>> {
        let record = self.get_style(id)?;
        Some(SegmentStyle {
            id,
            record,
            layout: self.get_layout(record.layout_id())?,
            paint: self.get_paint(record.paint_id())?,
        })
    }

    /// Resolves a compact style identifier into borrowed payloads from this set.
    ///
    /// # Panics
    ///
    /// Panics if `id` (or one of its component ids) is not interned in this set,
    /// which happens when an id from a different [`StyleSet`] is used here. Use
    /// [`try_segment_style`](Self::try_segment_style) for ids that may not belong
    /// to this set.
    #[must_use]
    pub fn segment_style(&self, id: StyleId) -> SegmentStyle<'_, L, P> {
        self.try_segment_style(id)
            .expect("style id must be interned in this style set")
    }
}

/// Borrowed style payloads for a resolved segment.
#[derive(Debug, PartialEq)]
pub struct SegmentStyle<'a, L, P> {
    id: StyleId,
    record: StyleRecord,
    layout: &'a L,
    paint: &'a P,
}

impl<L, P> Clone for SegmentStyle<'_, L, P> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<L, P> Copy for SegmentStyle<'_, L, P> {}

impl<'a, L, P> SegmentStyle<'a, L, P> {
    /// Returns the compact full-style identifier.
    #[must_use]
    #[inline]
    pub const fn id(self) -> StyleId {
        self.id
    }

    /// Returns the interned component identifiers for this full style.
    #[must_use]
    #[inline]
    pub const fn record(self) -> StyleRecord {
        self.record
    }

    /// Returns the resolved layout-style identifier.
    #[must_use]
    #[inline]
    pub const fn layout_id(self) -> LayoutStyleId {
        self.record.layout_id()
    }

    /// Returns the resolved paint-style identifier.
    #[must_use]
    #[inline]
    pub const fn paint_id(self) -> PaintStyleId {
        self.record.paint_id()
    }

    /// Returns the resolved layout payload.
    #[must_use]
    #[inline]
    pub const fn layout(self) -> &'a L {
        self.layout
    }

    /// Returns the resolved paint payload.
    #[must_use]
    #[inline]
    pub const fn paint(self) -> &'a P {
        self.paint
    }
}

/// Builder for [`StyleSet`] values.
///
/// Interning is currently a linear search over retained vectors. That keeps the
/// first slice dependency-free and compact; callers can reuse a builder during
/// style collection to amortize allocations.
#[derive(Clone, Debug, Default)]
pub struct StyleSetBuilder<L, P> {
    styles: Vec<StyleRecord>,
    layout: Vec<L>,
    paint: Vec<P>,
}

impl<L, P> StyleSetBuilder<L, P> {
    /// Creates an empty style-set builder.
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self {
            styles: Vec::new(),
            layout: Vec::new(),
            paint: Vec::new(),
        }
    }

    /// Creates a builder with capacity for full styles and their component payloads.
    #[must_use]
    #[inline]
    pub fn with_capacity(style_capacity: usize) -> Self {
        Self {
            styles: Vec::with_capacity(style_capacity),
            layout: Vec::with_capacity(style_capacity),
            paint: Vec::with_capacity(style_capacity),
        }
    }

    /// Removes all interned styles and payloads while keeping allocated storage.
    #[inline]
    pub fn clear(&mut self) {
        self.styles.clear();
        self.layout.clear();
        self.paint.clear();
    }

    /// Finishes the builder and returns the style set.
    #[must_use]
    #[inline]
    pub fn finish(self) -> StyleSet<L, P> {
        StyleSet {
            styles: self.styles,
            layout: self.layout,
            paint: self.paint,
        }
    }

    fn intern_style_record(&mut self, record: StyleRecord) -> StyleId {
        if let Some(index) = self.styles.iter().position(|existing| *existing == record) {
            return StyleId::from_index(index);
        }

        let index = self.styles.len();
        self.styles.push(record);
        StyleId::from_index(index)
    }
}

impl<L: PartialEq, P> StyleSetBuilder<L, P> {
    fn intern_layout(&mut self, style: L) -> LayoutStyleId {
        if let Some(index) = self.layout.iter().position(|existing| existing == &style) {
            return LayoutStyleId::from_index(index);
        }

        let index = self.layout.len();
        self.layout.push(style);
        LayoutStyleId::from_index(index)
    }
}

impl<L, P: PartialEq> StyleSetBuilder<L, P> {
    fn intern_paint(&mut self, style: P) -> PaintStyleId {
        if let Some(index) = self.paint.iter().position(|existing| existing == &style) {
            return PaintStyleId::from_index(index);
        }

        let index = self.paint.len();
        self.paint.push(style);
        PaintStyleId::from_index(index)
    }
}

impl<L: PartialEq, P: PartialEq> StyleSetBuilder<L, P> {
    /// Interns layout and paint payloads as a full style and returns its identifier.
    pub fn intern_style(&mut self, layout: L, paint: P) -> StyleId {
        let layout = self.intern_layout(layout);
        let paint = self.intern_paint(paint);
        self.intern_style_record(StyleRecord { layout, paint })
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::String;

    use super::StyleSetBuilder;

    #[test]
    fn builder_interns_equal_payloads_and_full_styles() {
        let mut builder = StyleSetBuilder::<u8, u16>::new();
        let normal_red = builder.intern_style(12, 0xff00_u16);
        let normal_red_again = builder.intern_style(12, 0xff00_u16);
        let normal_blue = builder.intern_style(12, 0x00ff_u16);

        assert_eq!(normal_red, normal_red_again);
        assert_ne!(normal_red, normal_blue);

        let styles = builder.finish();
        assert_eq!(styles.style_len(), 2);
        assert_eq!(styles.layout_len(), 1);
        assert_eq!(styles.paint_len(), 2);

        let style = styles.segment_style(normal_red);
        assert_eq!(style.layout(), &12);
        assert_eq!(style.paint(), &0xff00_u16);
    }

    #[test]
    fn segment_style_is_copy_for_non_copy_payloads() {
        let mut builder = StyleSetBuilder::<String, String>::new();
        let style_id = builder.intern_style(String::from("layout"), String::from("paint"));
        let styles = builder.finish();

        let style = styles.segment_style(style_id);
        let layout = style.layout();
        let paint = style.paint();

        assert_eq!(layout, "layout");
        assert_eq!(paint, "paint");
    }
}
