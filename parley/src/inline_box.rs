// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::VerticalAlign;

/// A box to be laid out inline with text
///
/// Such boxes take up space in the layout and flow in line with text. They correspond to
/// [atomic inlines] in CSS, and take part in line breaking and selection.
///
/// See also [`FloatedBox`].
///
/// [atomic inlines]: https://www.w3.org/TR/css-display-3/#atomic-inline
#[derive(PartialEq, Debug, Clone)]
pub struct InlineBox {
    /// User-specified identifier for the box, which can be used by the user to determine which box in
    /// parley's output corresponds to which box in its input.
    pub id: u64,
    /// The byte offset into the underlying text string at which the box is placed.
    ///
    /// The box must be represented by a U+FFFC OBJECT REPLACEMENT CHARACTER in the text, and this
    /// must be the offset of that character. Each such character holds at most one box.
    ///
    /// Any U+FFFC character in the text without a corresponding box, is treated as ordinary text
    /// (some fonts may render .notdef, others may have a glyph for it).
    ///
    /// When using the [`TreeBuilder`](crate::TreeBuilder), this offset is set by the builder.
    pub index: usize,
    /// The width of the box in pixels
    pub width: f32,
    /// The height of the box in pixels
    pub height: f32,
    /// The baseline of the box in pixels, relative to the top of the box
    ///
    /// If `None`, the baseline is the box's bottom edge.
    pub baseline: Option<f32>,
    /// Vertical alignment of the box within its line, relative to the enclosing style span.
    pub vertical_align: VerticalAlign,
}

/// A box outside of the inline flow, which the caller positions during line breaking.
///
/// When line breaking encounters the box, it yields control flow with
/// [`YieldData::FloatedBox`](crate::YieldData::FloatedBox), and the caller is responsible for
/// laying out the box, e.g. adjusting the line geometry through
/// [`BreakerState`](crate::BreakerState). These correspond to floated boxes in CSS (`float: ...`).
///
/// See also [`InlineBox`]. To position out-of-flow boxes that don't affect layout at all, you can
/// use e.g. [`Layout::primary_caret`](crate::Layout::primary_caret).
#[derive(PartialEq, Debug, Clone)]
pub struct FloatedBox {
    /// User-specified identifier for the box, which can be used by the user to determine which box in
    /// parley's output corresponds to which box in its input.
    pub id: u64,
    /// The byte offset into the underlying text string at which the box is placed.
    ///
    /// Floated boxes can be placed at any character boundary.
    ///
    /// When using the [`TreeBuilder`](crate::TreeBuilder), this offset is set by the builder.
    pub index: usize,
}

/// An [`InlineBox`] together with the style index of the span that contains it.
#[derive(PartialEq, Debug, Clone)]
pub(crate) struct LayoutInlineBox {
    pub(crate) inline_box: InlineBox,
    /// Style index of the span containing the box, against which the box's `vertical_align`
    /// is resolved. The box itself has no style of its own.
    pub(crate) parent_style_index: u16,
    /// Offset of the box's baseline above the baseline of the independent aligned subtree it
    /// belongs to (that of the parent style), resolved when the layout is built and when the box
    /// is edited (see [`crate::Layout::edit_inline_box`]). Unused for
    /// `vertical-align: top | bottom` boxes.
    pub(crate) baseline_offset: f32,
    /// The shaped cluster of the box's inline object. See
    /// [`parley_engine::AnalysisOptions::inline_objects`].
    pub(crate) inline_object_cluster: Option<u32>,
}

impl LayoutInlineBox {
    pub(crate) fn new(inline_box: InlineBox, parent_style_index: u16) -> Self {
        Self {
            inline_box,
            parent_style_index,
            baseline_offset: 0.,
            inline_object_cluster: None,
        }
    }
}

/// The index into `inline_boxes` of the inline object whose U+FFFC starts at `text_byte`.
///
/// `inline_boxes` must be sorted by index, and there must be such an inline object.
pub(crate) fn inline_object_box_index(inline_boxes: &[LayoutInlineBox], text_byte: usize) -> usize {
    let index = inline_boxes.partition_point(|b| b.inline_box.index < text_byte);
    debug_assert_eq!(
        inline_boxes[index].inline_box.index, text_byte,
        "an inline object must have an `InlineBox`"
    );
    index
}
