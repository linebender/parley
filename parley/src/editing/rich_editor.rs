// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A text editor supporting multiple styles applied to ranges ("spans") of the text.

use alloc::{string::String, vec::Vec};
use core::{cmp::PartialEq, default::Default, fmt::Debug, num::NonZeroUsize, ops::Range};

use attributed_text::{AttributedText, TextRange};

use crate::editing::{Cursor, Generation, Selection, SplitString};
use crate::layout::{Affinity, Alignment, AlignmentOptions, Layout};
use crate::style::Brush;
use crate::{BoundingBox, FontContext, LayoutContext, StyleSet};

#[cfg(feature = "accesskit")]
use crate::layout::LayoutAccessibility;
#[cfg(feature = "accesskit")]
use accesskit::{Node, NodeId, TreeUpdate};

/// A style applied to a range of text, owned for the lifetime of a [`RichEditor`].
type StyleProperty<Brush> = crate::StyleProperty<'static, Brush>;

/// Compute the new range for a style span after `old_range` in the text is replaced by
/// `new_len` bytes of new content.
///
/// A span which ends exactly where the edit starts absorbs the new content, so that typing
/// at the end of a styled run continues in that style. Returns `None` if the edit fully
/// consumes the span.
fn shift_span_range(
    span: Range<usize>,
    old_range: Range<usize>,
    new_len: usize,
) -> Option<Range<usize>> {
    let old_len = old_range.len();
    if new_len == old_len {
        return Some(span);
    }
    // Translate a position which lies at or after `old_range.end` by the change in length.
    let shift = |index: usize| -> usize {
        if new_len >= old_len {
            index + (new_len - old_len)
        } else {
            index - (old_len - new_len)
        }
    };

    // Entirely before the edit: unaffected.
    if span.end < old_range.start {
        return Some(span);
    }
    // Touches the edit's start: absorb the new content, continuing this span's style.
    if span.end == old_range.start {
        return Some(span.start..span.end + new_len);
    }
    // Entirely after the edit: shift both ends.
    if span.start >= old_range.end {
        return Some(shift(span.start)..shift(span.end));
    }

    // The span overlaps the edited range.
    let new_start = if span.start <= old_range.start {
        span.start
    } else {
        // The span started inside the edited range; it now begins right after the replacement.
        old_range.start + new_len
    };
    let new_end = if span.end > old_range.end {
        // The span extends past the edit; shift its end like a span entirely after it would.
        shift(span.end)
    } else {
        // The span's end was inside the edited range; clip it to just before the edit.
        old_range.start
    };
    (new_start < new_end).then_some(new_start..new_end)
}

/// A text editor supporting multiple styles applied to ranges of the text.
///
/// This is the rich-text counterpart to [`PlainEditor`](crate::editing::PlainEditor): instead of
/// a single [`StyleSet`] applied to the whole buffer, styles can additionally be applied to
/// arbitrary byte ranges via [`RichEditor::set_style`].
///
/// Internally, this is a wrapper around an [`AttributedText`] buffer and its corresponding
/// [`Layout`], which is kept up-to-date as needed.
#[derive(Clone, Debug)]
pub struct RichEditor<T>
where
    T: Brush + Clone + Debug + PartialEq + Default,
{
    layout: Layout<T>,
    spans: AttributedText<String, StyleProperty<T>>,
    default_style: StyleSet<T>,
    #[cfg(feature = "accesskit")]
    layout_access: LayoutAccessibility,
    selection: Selection,
    /// Byte offsets of IME composing preedit text in the text buffer.
    /// `None` if the IME is not currently composing.
    compose: Option<Range<usize>>,
    /// Whether the cursor should be shown. The IME can request to hide the cursor.
    show_cursor: bool,
    width: Option<f32>,
    font_size: f32,
    scale: f32,
    quantize: bool,
    layout_dirty: bool,
    alignment: Alignment,
    generation: Generation,
}

impl<T> RichEditor<T>
where
    T: Brush,
{
    /// Create a new editor, with default font size `font_size`.
    pub fn new(font_size: f32) -> Self {
        Self {
            default_style: StyleSet::new(font_size),
            spans: AttributedText::new(String::new()),
            layout: Layout::default(),
            #[cfg(feature = "accesskit")]
            layout_access: LayoutAccessibility::default(),
            selection: Selection::default(),
            compose: None,
            show_cursor: true,
            width: None,
            font_size,
            scale: 1.0,
            quantize: true,
            layout_dirty: true,
            alignment: Alignment::Start,
            generation: Generation(1),
        }
    }
}

/// A short-lived wrapper around [`RichEditor`].
///
/// This can perform operations which require the editor's layout to
/// be up-to-date by refreshing it as necessary.
pub struct RichEditorDriver<'a, T>
where
    T: Brush + Clone + Debug + PartialEq + Default,
{
    pub editor: &'a mut RichEditor<T>,
    pub font_cx: &'a mut FontContext,
    pub layout_cx: &'a mut LayoutContext<T>,
}

impl<T> RichEditorDriver<'_, T>
where
    T: Brush + Clone + Debug + PartialEq + Default,
{
    // --- MARK: Forced relayout ---
    /// Insert at cursor, or replace selection.
    pub fn insert_or_replace_selection(&mut self, s: &str) {
        self.editor
            .replace_selection(self.font_cx, self.layout_cx, s);
    }

    /// Delete the selection.
    pub fn delete_selection(&mut self) {
        self.insert_or_replace_selection("");
    }

    /// Delete the specified numbers of bytes before the selection.
    /// The selection is moved to the left by that number of bytes
    /// but otherwise unchanged.
    ///
    /// The deleted range is clamped to the start of the buffer.
    /// No-op if the start of the range is not a char boundary.
    pub fn delete_bytes_before_selection(&mut self, len: NonZeroUsize) {
        let old_selection = self.editor.selection;
        let selection_range = old_selection.text_range();
        let range = selection_range.start.saturating_sub(len.get())..selection_range.start;
        if range.is_empty() || !self.editor.spans.text().is_char_boundary(range.start) {
            return;
        }
        self.editor.replace_text(range.clone(), "");
        self.update_layout();
        let old_anchor = old_selection.anchor();
        let old_focus = old_selection.focus();
        // When doing the equivalent of a backspace on a collapsed selection,
        // always use downstream affinity, as `backdelete` does.
        let (anchor_affinity, focus_affinity) = if old_selection.is_collapsed() {
            (Affinity::Downstream, Affinity::Downstream)
        } else {
            (old_anchor.affinity(), old_focus.affinity())
        };
        self.editor.set_selection(Selection::new(
            Cursor::from_byte_index(
                &self.editor.layout,
                old_anchor.index() - range.len(),
                anchor_affinity,
            ),
            Cursor::from_byte_index(
                &self.editor.layout,
                old_focus.index() - range.len(),
                focus_affinity,
            ),
        ));
    }

    /// Delete the specified numbers of bytes after the selection.
    /// The selection is unchanged.
    ///
    /// The deleted range is clamped to the end of the buffer.
    /// No-op if the end of the range is not a char boundary.
    pub fn delete_bytes_after_selection(&mut self, len: NonZeroUsize) {
        let selection_range = self.editor.selection.text_range();
        let range = selection_range.end
            ..selection_range
                .end
                .saturating_add(len.get())
                .min(self.editor.spans.len());
        if range.is_empty() || !self.editor.spans.text().is_char_boundary(range.end) {
            return;
        }
        self.editor.replace_text(range, "");
        self.update_layout();
    }

    /// Delete the selection or the next cluster (typical ‘delete’ behavior).
    pub fn delete(&mut self) {
        if self.editor.selection.is_collapsed() {
            // Upstream cluster range
            if let Some(range) = self
                .editor
                .selection
                .focus()
                .logical_clusters(&self.editor.layout)[1]
                .as_ref()
                .map(|cluster| cluster.text_range())
                .and_then(|range| (!range.is_empty()).then_some(range))
            {
                self.editor.replace_text(range, "");
                self.update_layout();
            }
        } else {
            self.delete_selection();
        }
    }

    /// Delete the selection or up to the next word boundary (typical ‘ctrl + delete’ behavior).
    pub fn delete_word(&mut self) {
        if self.editor.selection.is_collapsed() {
            let focus = self.editor.selection.focus();
            let start = focus.index();
            let end = focus.next_logical_word(&self.editor.layout).index();
            if self.editor.spans.text().get(start..end).is_some() {
                self.editor.replace_text(start..end, "");
                self.update_layout();
                self.editor.set_selection(
                    Cursor::from_byte_index(&self.editor.layout, start, Affinity::Downstream)
                        .into(),
                );
            }
        } else {
            self.delete_selection();
        }
    }

    /// Delete the selection or the previous cluster (typical ‘backspace’ behavior).
    pub fn backdelete(&mut self) {
        if self.editor.selection.is_collapsed() {
            // Upstream cluster
            if let Some(cluster) = self
                .editor
                .selection
                .focus()
                .logical_clusters(&self.editor.layout)[0]
            {
                let range = cluster.text_range();
                let end = range.end;
                let start = if cluster.is_hard_line_break() || cluster.is_emoji() {
                    // For newline sequences and emoji, delete the previous cluster
                    range.start
                } else {
                    // Otherwise, delete the previous character
                    let Some((start, _)) = self
                        .editor
                        .spans
                        .text()
                        .get(..end)
                        .and_then(|str| str.char_indices().next_back())
                    else {
                        return;
                    };
                    start
                };
                self.editor.replace_text(start..end, "");
                self.update_layout();
                self.editor.set_selection(
                    Cursor::from_byte_index(&self.editor.layout, start, Affinity::Downstream)
                        .into(),
                );
            }
        } else {
            self.delete_selection();
        }
    }

    /// Delete the selection or back to the previous word boundary (typical ‘ctrl + backspace’ behavior).
    pub fn backdelete_word(&mut self) {
        if self.editor.selection.is_collapsed() {
            let focus = self.editor.selection.focus();
            let end = focus.index();
            let start = focus.previous_logical_word(&self.editor.layout).index();
            if self.editor.spans.text().get(start..end).is_some() {
                self.editor.replace_text(start..end, "");
                self.update_layout();
                self.editor.set_selection(
                    Cursor::from_byte_index(&self.editor.layout, start, Affinity::Downstream)
                        .into(),
                );
            }
        } else {
            self.delete_selection();
        }
    }

    // --- MARK: IME ---
    /// Set the IME preedit composing text.
    ///
    /// This starts composing. Composing is reset by calling [`clear_compose`](Self::clear_compose).
    /// Alternatively, the preedit text can be committed by calling [`finish_compose`](Self::finish_compose).
    ///
    /// The selection and preedit region can be manipulated independently while composing
    /// is active.
    ///
    /// The preedit text replaces the current selection if this call starts composing.
    ///
    /// The selection is updated based on `cursor`, which contains the byte offsets relative to the
    /// start of the preedit text. If `cursor` is `None`, the selection and caret are hidden.
    pub fn set_compose(&mut self, text: &str, cursor: Option<(usize, usize)>) {
        debug_assert!(!text.is_empty());
        debug_assert!(cursor.map(|cursor| cursor.1 <= text.len()).unwrap_or(true));

        let start = if let Some(preedit_range) = self.editor.compose.clone() {
            self.editor.replace_text(preedit_range.clone(), text);
            preedit_range.start
        } else {
            let range = self.editor.selection.text_range();
            self.editor.replace_text(range.clone(), text);
            range.start
        };
        self.editor.compose = Some(start..start + text.len());
        self.editor.show_cursor = cursor.is_some();
        self.update_layout();

        // Select the location indicated by the IME. If `cursor` is none, collapse the selection to
        // a caret at the start of the preedit text. As `self.editor.show_cursor` is `false`, it
        // won't show up.
        let cursor = cursor.unwrap_or((0, 0));
        self.editor.set_selection(Selection::new(
            self.editor.cursor_at(start + cursor.0),
            self.editor.cursor_at(start + cursor.1),
        ));
    }

    /// Set the preedit range to a range of byte indices.
    /// This leaves the selection and cursor unchanged.
    ///
    /// No-op if either index is not a char boundary.
    pub fn set_compose_byte_range(&mut self, start: usize, end: usize) {
        let text = self.editor.spans.text();
        if text.is_char_boundary(start) && text.is_char_boundary(end) {
            self.editor.compose = Some(start..end);
            self.update_layout();
        }
    }

    /// Stop IME composing.
    ///
    /// This removes the IME preedit text, shows the cursor if it was hidden,
    /// and moves the cursor to the start of the former preedit region.
    pub fn clear_compose(&mut self) {
        if let Some(preedit_range) = self.editor.compose.take() {
            self.editor.replace_text(preedit_range.clone(), "");
            self.editor.show_cursor = true;
            self.update_layout();

            self.editor
                .set_selection(self.editor.cursor_at(preedit_range.start).into());
        }
    }

    /// Commit the IME preedit text, if any.
    ///
    /// This doesn't change the selection, but shows the cursor if
    /// it was hidden.
    pub fn finish_compose(&mut self) {
        if self.editor.compose.take().is_some() {
            self.editor.show_cursor = true;
            self.update_layout();
        }
    }

    // --- MARK: Cursor Movement ---
    /// Move the cursor to the cluster boundary nearest this point in the layout.
    pub fn move_to_point(&mut self, x: f32, y: f32) {
        self.refresh_layout();
        self.editor
            .set_selection(Selection::from_point(&self.editor.layout, x, y));
    }

    /// Move the cursor to a byte index.
    ///
    /// No-op if index is not a char boundary.
    pub fn move_to_byte(&mut self, index: usize) {
        if self.editor.spans.text().is_char_boundary(index) {
            self.refresh_layout();
            self.editor
                .set_selection(self.editor.cursor_at(index).into());
        }
    }

    /// Move the cursor to the start of the buffer.
    pub fn move_to_text_start(&mut self) {
        self.refresh_layout();
        self.editor.set_selection(self.editor.selection.move_lines(
            &self.editor.layout,
            isize::MIN,
            false,
        ));
    }

    /// Move the cursor to just after the previous hard line break (such as `\n`).
    pub fn move_to_hard_line_start(&mut self) {
        self.refresh_layout();
        self.editor.set_selection(
            self.editor
                .selection
                .hard_line_start(&self.editor.layout, false),
        );
    }

    /// Move the cursor to the start of the physical line.
    pub fn move_to_line_start(&mut self) {
        self.refresh_layout();
        self.editor
            .set_selection(self.editor.selection.line_start(&self.editor.layout, false));
    }

    /// Move the cursor to the end of the buffer.
    pub fn move_to_text_end(&mut self) {
        self.refresh_layout();
        self.editor.set_selection(self.editor.selection.move_lines(
            &self.editor.layout,
            isize::MAX,
            false,
        ));
    }

    /// Move the cursor to just before the next hard line break (such as `\n`).
    pub fn move_to_hard_line_end(&mut self) {
        self.refresh_layout();
        self.editor.set_selection(
            self.editor
                .selection
                .hard_line_end(&self.editor.layout, false),
        );
    }

    /// Move the cursor to the end of the physical line.
    pub fn move_to_line_end(&mut self) {
        self.refresh_layout();
        self.editor
            .set_selection(self.editor.selection.line_end(&self.editor.layout, false));
    }

    /// Move up to the closest physical cluster boundary on the previous line, preserving the horizontal position for repeated movements.
    pub fn move_up(&mut self) {
        self.refresh_layout();
        self.editor.set_selection(
            self.editor
                .selection
                .previous_line(&self.editor.layout, false),
        );
    }

    /// Move down to the closest physical cluster boundary on the next line, preserving the horizontal position for repeated movements.
    pub fn move_down(&mut self) {
        self.refresh_layout();
        self.editor
            .set_selection(self.editor.selection.next_line(&self.editor.layout, false));
    }

    /// Move to the next cluster left in visual order.
    pub fn move_left(&mut self) {
        self.refresh_layout();
        self.editor.set_selection(
            self.editor
                .selection
                .previous_visual(&self.editor.layout, false),
        );
    }

    /// Move to the next cluster right in visual order.
    pub fn move_right(&mut self) {
        self.refresh_layout();
        self.editor.set_selection(
            self.editor
                .selection
                .next_visual(&self.editor.layout, false),
        );
    }

    /// Move to the next word boundary left.
    pub fn move_word_left(&mut self) {
        self.refresh_layout();
        self.editor.set_selection(
            self.editor
                .selection
                .previous_visual_word(&self.editor.layout, false),
        );
    }

    /// Move to the next word boundary right.
    pub fn move_word_right(&mut self) {
        self.refresh_layout();
        self.editor.set_selection(
            self.editor
                .selection
                .next_visual_word(&self.editor.layout, false),
        );
    }

    /// Select the whole buffer.
    pub fn select_all(&mut self) {
        self.refresh_layout();
        self.editor.set_selection(
            Selection::from_byte_index(&self.editor.layout, 0_usize, Affinity::default())
                .move_lines(&self.editor.layout, isize::MAX, true),
        );
    }

    /// Collapse selection into caret.
    pub fn collapse_selection(&mut self) {
        self.editor.set_selection(self.editor.selection.collapse());
    }

    /// Move the selection focus point to the start of the buffer.
    pub fn select_to_text_start(&mut self) {
        self.refresh_layout();
        self.editor.set_selection(self.editor.selection.move_lines(
            &self.editor.layout,
            isize::MIN,
            true,
        ));
    }

    /// Move the selection focus point to just after the previous hard line break (such as `\n`).
    pub fn select_to_hard_line_start(&mut self) {
        self.refresh_layout();
        self.editor.set_selection(
            self.editor
                .selection
                .hard_line_start(&self.editor.layout, true),
        );
    }

    /// Move the selection focus point to the start of the physical line.
    pub fn select_to_line_start(&mut self) {
        self.refresh_layout();
        self.editor
            .set_selection(self.editor.selection.line_start(&self.editor.layout, true));
    }

    /// Move the selection focus point to the end of the buffer.
    pub fn select_to_text_end(&mut self) {
        self.refresh_layout();
        self.editor.set_selection(self.editor.selection.move_lines(
            &self.editor.layout,
            isize::MAX,
            true,
        ));
    }

    /// Move the selection focus point to just before the next hard line break (such as `\n`).
    pub fn select_to_hard_line_end(&mut self) {
        self.refresh_layout();
        self.editor.set_selection(
            self.editor
                .selection
                .hard_line_end(&self.editor.layout, true),
        );
    }

    /// Move the selection focus point to the end of the physical line.
    pub fn select_to_line_end(&mut self) {
        self.refresh_layout();
        self.editor
            .set_selection(self.editor.selection.line_end(&self.editor.layout, true));
    }

    /// Move the selection focus point up to the nearest cluster boundary on the previous line, preserving the horizontal position for repeated movements.
    pub fn select_up(&mut self) {
        self.refresh_layout();
        self.editor.set_selection(
            self.editor
                .selection
                .previous_line(&self.editor.layout, true),
        );
    }

    /// Move the selection focus point down to the nearest cluster boundary on the next line, preserving the horizontal position for repeated movements.
    pub fn select_down(&mut self) {
        self.refresh_layout();
        self.editor
            .set_selection(self.editor.selection.next_line(&self.editor.layout, true));
    }

    /// Move the selection focus point to the next cluster left in visual order.
    pub fn select_left(&mut self) {
        self.refresh_layout();
        self.editor.set_selection(
            self.editor
                .selection
                .previous_visual(&self.editor.layout, true),
        );
    }

    /// Move the selection focus point to the next cluster right in visual order.
    pub fn select_right(&mut self) {
        self.refresh_layout();
        self.editor
            .set_selection(self.editor.selection.next_visual(&self.editor.layout, true));
    }

    /// Move the selection focus point to the next word boundary left.
    pub fn select_word_left(&mut self) {
        self.refresh_layout();
        self.editor.set_selection(
            self.editor
                .selection
                .previous_visual_word(&self.editor.layout, true),
        );
    }

    /// Move the selection focus point to the next word boundary right.
    pub fn select_word_right(&mut self) {
        self.refresh_layout();
        self.editor.set_selection(
            self.editor
                .selection
                .next_visual_word(&self.editor.layout, true),
        );
    }

    /// Select the word at the point.
    pub fn select_word_at_point(&mut self, x: f32, y: f32) {
        self.refresh_layout();
        self.editor
            .set_selection(Selection::word_from_point(&self.editor.layout, x, y));
    }

    /// Select the physical line at the point.
    ///
    /// Note that this metehod determines line breaks for any reason, including due to word wrapping.
    /// To select the text between explicit newlines, use [`select_hard_line_at_point`](Self::select_hard_line_at_point).
    /// In most text editing cases, this is the preferred behaviour.
    pub fn select_line_at_point(&mut self, x: f32, y: f32) {
        self.refresh_layout();
        let line = Selection::line_from_point(&self.editor.layout, x, y);
        self.editor.set_selection(line);
    }

    /// Select the "logical" line at the point.
    ///
    /// The logical line is defined by line break characters, such as `\n`, rather than due to soft-wrapping.
    pub fn select_hard_line_at_point(&mut self, x: f32, y: f32) {
        self.refresh_layout();
        let hard_line = Selection::hard_line_from_point(&self.editor.layout, x, y);
        self.editor.set_selection(hard_line);
    }

    /// Move the selection focus point to the cluster boundary closest to point.
    ///
    /// If the initial selection was created from a word or line, then the new
    /// selection will be extended at the same granularity.
    pub fn extend_selection_to_point(&mut self, x: f32, y: f32) {
        self.refresh_layout();
        // FIXME: This is usually the wrong way to handle selection extension for mouse moves, but not a regression.
        self.editor.set_selection(
            self.editor
                .selection
                .extend_to_point(&self.editor.layout, x, y),
        );
    }

    /// Move the selection focus point to the cluster boundary closest to point.
    pub fn shift_click_extension(&mut self, x: f32, y: f32) {
        self.refresh_layout();
        self.editor
            .set_selection(
                self.editor
                    .selection
                    .shift_click_extension(&self.editor.layout, x, y),
            );
    }

    /// Move the selection focus point to a byte index.
    ///
    /// No-op if index is not a char boundary.
    pub fn extend_selection_to_byte(&mut self, index: usize) {
        if self.editor.spans.text().is_char_boundary(index) {
            self.refresh_layout();
            self.editor
                .set_selection(self.editor.selection.extend(self.editor.cursor_at(index)));
        }
    }

    /// Select a range of byte indices.
    ///
    /// No-op if either index is not a char boundary.
    pub fn select_byte_range(&mut self, start: usize, end: usize) {
        let text = self.editor.spans.text();
        if text.is_char_boundary(start) && text.is_char_boundary(end) {
            self.refresh_layout();
            self.editor.set_selection(Selection::new(
                self.editor.cursor_at(start),
                self.editor.cursor_at(end),
            ));
        }
    }

    #[cfg(feature = "accesskit")]
    /// Select inside the editor based on the selection provided by accesskit.
    pub fn select_from_accesskit(&mut self, selection: &accesskit::TextSelection) {
        self.refresh_layout();
        if let Some(selection) = Selection::from_access_selection(
            selection,
            &self.editor.layout,
            &self.editor.layout_access,
        ) {
            self.editor.set_selection(selection);
        }
    }

    // --- MARK: Rendering ---
    #[cfg(feature = "accesskit")]
    /// Perform an accessibility update.
    pub fn accessibility(
        &mut self,
        update: &mut TreeUpdate,
        node: &mut Node,
        next_node_id: impl FnMut() -> NodeId,
        x_offset: f64,
        y_offset: f64,
        set_brush_properties: impl Fn(&mut Node, &crate::Style<T>),
    ) -> Option<()> {
        self.refresh_layout();
        self.editor.accessibility_unchecked(
            update,
            node,
            next_node_id,
            x_offset,
            y_offset,
            set_brush_properties,
        );
        Some(())
    }

    /// Get the up-to-date layout for this driver.
    pub fn layout(&mut self) -> &Layout<T> {
        self.editor.layout(self.font_cx, self.layout_cx)
    }

    // --- MARK: Internal helpers---
    /// Update the layout if needed.
    pub fn refresh_layout(&mut self) {
        self.editor.refresh_layout(self.font_cx, self.layout_cx);
    }

    /// Update the layout unconditionally.
    fn update_layout(&mut self) {
        self.editor.update_layout(self.font_cx, self.layout_cx);
    }
}

impl<T> RichEditor<T>
where
    T: Brush + Clone + Debug + PartialEq + Default,
{
    /// Run a series of [`RichEditorDriver`] methods.
    ///
    /// This type is only used to simplify methods which require both
    /// the editor and the provided contexts.
    pub fn driver<'drv>(
        &'drv mut self,
        font_cx: &'drv mut FontContext,
        layout_cx: &'drv mut LayoutContext<T>,
    ) -> RichEditorDriver<'drv, T> {
        RichEditorDriver {
            editor: self,
            font_cx,
            layout_cx,
        }
    }

    /// Apply `property` to the given byte `range` of text.
    ///
    /// Spans may overlap, including other spans of the same property: conflicts are resolved
    /// when the layout is next rebuilt, with the most-recently-applied span winning for the
    /// bytes it covers.
    ///
    /// No-op if `range` is not a valid byte range for the current text.
    pub fn set_style(&mut self, property: StyleProperty<T>, range: Range<usize>) {
        if let Ok(range) = TextRange::new(self.spans.text(), range) {
            self.spans.apply_attribute(range, property);
            self.layout_dirty = true;
        }
    }

    /// Iterate over the styles applied at a given byte `index`, in application order.
    pub fn style_at(&self, index: usize) -> impl Iterator<Item = (Range<usize>, &StyleProperty<T>)> {
        self.spans
            .attributes_at(index)
            .map(|(range, prop)| (range.as_range(), prop))
    }

    /// Borrow the current selection. The indices returned by functions
    /// such as [`Selection::text_range`] refer to the raw text buffer,
    /// including the IME preedit region, which can be accessed via
    /// [`RichEditor::raw_text`].
    pub fn raw_selection(&self) -> &Selection {
        &self.selection
    }

    /// Borrow the current IME preedit range, if any. These indices refer
    /// to the raw text buffer, which can be accessed via [`RichEditor::raw_text`].
    pub fn raw_compose(&self) -> &Option<Range<usize>> {
        &self.compose
    }

    /// If the current selection is not collapsed, returns the text content of
    /// that selection.
    pub fn selected_text(&self) -> Option<&str> {
        if self.is_composing() {
            return None;
        }
        if !self.selection.is_collapsed() {
            self.spans.text().get(self.selection.text_range())
        } else {
            None
        }
    }

    /// Get rectangles, and their corresponding line indices, representing the selected portions of
    /// text.
    pub fn selection_geometry(&self) -> Vec<(BoundingBox, usize)> {
        // We do not check `self.show_cursor` here, as the IME handling code collapses the
        // selection to a caret in that case.
        self.selection.geometry(&self.layout)
    }

    /// Invoke a callback with each rectangle representing the selected portions of text, and the
    /// indices of the lines to which they belong.
    pub fn selection_geometry_with(&self, f: impl FnMut(BoundingBox, usize)) {
        // We do not check `self.show_cursor` here, as the IME handling code collapses the
        // selection to a caret in that case.
        self.selection.geometry_with(&self.layout, f);
    }

    /// Get a rectangle representing the current caret cursor position.
    ///
    /// There is not always a caret. For example, the IME may have indicated the caret should be
    /// hidden.
    pub fn cursor_geometry(&self, size: f32) -> Option<BoundingBox> {
        self.show_cursor
            .then(|| self.selection.focus().geometry(&self.layout, size))
    }

    /// Get a rectangle bounding the text the user is currently editing.
    ///
    /// This is useful for suggesting an exclusion area to the platform for, e.g., IME candidate
    /// box placement. This bounds the area of the preedit text if present, otherwise it bounds the
    /// selection on the focused line.
    pub fn ime_cursor_area(&self) -> BoundingBox {
        let (area, focus) = if let Some(preedit_range) = &self.compose {
            let selection = Selection::new(
                self.cursor_at(preedit_range.start),
                self.cursor_at(preedit_range.end),
            );

            // Bound the entire preedit text.
            let mut area = None;
            selection.geometry_with(&self.layout, |rect, _| {
                let area = area.get_or_insert(rect);
                *area = area.union(rect);
            });

            (
                area.unwrap_or_else(|| selection.focus().geometry(&self.layout, 0.)),
                selection.focus(),
            )
        } else {
            // Bound the selected parts of the focused line only.
            let focus = self.selection.focus().geometry(&self.layout, 0.);
            let mut area = focus;
            self.selection.geometry_with(&self.layout, |rect, _| {
                if rect.y0 == focus.y0 {
                    area = area.union(rect);
                }
            });

            (area, self.selection.focus())
        };

        // Ensure some context is captured even for tiny or collapsed selections by including a
        // region surrounding the selection. Doing this unconditionally, the IME candidate box
        // usually does not need to jump around when composing starts or the preedit is added to.
        let [upstream, downstream] = focus.logical_clusters(&self.layout);
        let font_size = downstream
            .or(upstream)
            .map(|cluster| cluster.run().font_size())
            .unwrap_or(self.font_size * self.scale);
        // Using 0.6 as an estimate of the average advance
        let inflate = 3. * 0.6 * font_size as f64;
        let editor_width = self.width.map(f64::from).unwrap_or(f64::INFINITY);
        BoundingBox {
            x0: (area.x0 - inflate).max(0.),
            x1: (area.x1 + inflate).min(editor_width),
            y0: area.y0,
            y1: area.y1,
        }
    }

    /// Borrow the text content of the buffer.
    ///
    /// The return value is a `SplitString` because it
    /// excludes the IME preedit region.
    pub fn text(&self) -> SplitString<'_> {
        let text = self.spans.text().as_str();
        if let Some(preedit_range) = &self.compose {
            SplitString::new([&text[..preedit_range.start], &text[preedit_range.end..]])
        } else {
            SplitString::new([text, ""])
        }
    }

    /// Borrow the text content of the buffer, including the IME preedit
    /// region if any.
    ///
    /// Application authors should generally prefer [`text`](Self::text). That method excludes the
    /// IME preedit contents, which are not meaningful for applications to access; the
    /// in-progress IME content is not itself what the user intends to write.
    pub fn raw_text(&self) -> &str {
        self.spans.text().as_str()
    }

    /// Get the current `Generation` of the layout, to decide whether to draw.
    ///
    /// You should store the generation the editor was at when you last drew it, and then redraw
    /// when the generation is different (`Generation` is [`PartialEq`], so supports the equality `==` operation).
    pub fn generation(&self) -> Generation {
        self.generation
    }

    /// Replace the whole text buffer.
    ///
    /// This clears all styles applied via [`set_style`](Self::set_style).
    pub fn set_text(&mut self, is: &str) {
        self.spans.set_text(String::from(is));
        self.layout_dirty = true;
        self.compose = None;
    }

    /// Set the width of the layout.
    pub fn set_width(&mut self, width: Option<f32>) {
        self.width = width;
        self.layout_dirty = true;
    }

    /// Set the alignment of the layout.
    pub fn set_alignment(&mut self, alignment: Alignment) {
        self.alignment = alignment;
        self.layout_dirty = true;
    }

    /// Set the scale for the layout.
    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale;
        self.layout_dirty = true;
    }

    /// Get the current scale for the layout.
    pub fn get_scale(&self) -> f32 {
        self.scale
    }

    pub fn get_font_size(&self) -> f32 {
        self.font_size
    }

    /// Set whether to quantize the layout coordinates.
    ///
    /// See [`PlainEditor::set_quantize`](crate::editing::PlainEditor::set_quantize) for details.
    pub fn set_quantize(&mut self, quantize: bool) {
        self.quantize = quantize;
        self.layout_dirty = true;
    }

    /// Modify the default styles provided for this editor.
    ///
    /// These apply to any text not otherwise styled via [`set_style`](Self::set_style).
    pub fn edit_styles(&mut self) -> &mut StyleSet<T> {
        self.layout_dirty = true;
        &mut self.default_style
    }

    /// Get the current default styles for this editor.
    pub fn get_styles(&self) -> &StyleSet<T> {
        &self.default_style
    }

    /// Whether the editor is currently in IME composing mode.
    pub fn is_composing(&self) -> bool {
        self.compose.is_some()
    }

    /// Get the full read-only details from the layout, which will be updated if necessary.
    ///
    /// If the required contexts are not available, then [`refresh_layout`](Self::refresh_layout) can
    /// be called in a scope when they are available, and [`try_layout`](Self::try_layout) can
    /// be used instead.
    pub fn layout(
        &mut self,
        font_cx: &mut FontContext,
        layout_cx: &mut LayoutContext<T>,
    ) -> &Layout<T> {
        self.refresh_layout(font_cx, layout_cx);
        &self.layout
    }

    // --- MARK: Raw APIs ---
    /// Get the full read-only details from the layout, if valid.
    ///
    /// Returns `None` if the layout is not up-to-date.
    /// You can call [`refresh_layout`](Self::refresh_layout) before using this method,
    /// to ensure that the layout is up-to-date.
    ///
    /// The [`layout`](Self::layout) method should generally be preferred.
    pub fn try_layout(&self) -> Option<&Layout<T>> {
        if self.layout_dirty {
            None
        } else {
            Some(&self.layout)
        }
    }

    #[cfg(feature = "accesskit")]
    #[inline]
    /// Perform an accessibility update if the layout is valid.
    ///
    /// Returns `None` if the layout is not up-to-date.
    /// You can call [`refresh_layout`](Self::refresh_layout) before using this method,
    /// to ensure that the layout is up-to-date.
    /// The [`accessibility`](RichEditorDriver::accessibility) method on the driver type
    /// should be preferred if the contexts are available, which will do this automatically.
    pub fn try_accessibility(
        &mut self,
        update: &mut TreeUpdate,
        node: &mut Node,
        next_node_id: impl FnMut() -> NodeId,
        x_offset: f64,
        y_offset: f64,
        set_brush_properties: impl Fn(&mut Node, &crate::Style<T>),
    ) -> Option<()> {
        if self.layout_dirty {
            return None;
        }
        self.accessibility_unchecked(
            update,
            node,
            next_node_id,
            x_offset,
            y_offset,
            set_brush_properties,
        );
        Some(())
    }

    /// Update the layout if it is dirty.
    ///
    /// This should only be used alongside [`try_layout`](Self::try_layout)
    /// or [`try_accessibility`](Self::try_accessibility), if those will be
    /// called in a scope where the contexts are not available.
    pub fn refresh_layout(&mut self, font_cx: &mut FontContext, layout_cx: &mut LayoutContext<T>) {
        if self.layout_dirty {
            self.update_layout(font_cx, layout_cx);
        }
    }

    // --- MARK: Internal Helpers ---
    /// Make a cursor at a given byte index.
    fn cursor_at(&self, index: usize) -> Cursor {
        if index >= self.spans.len() {
            Cursor::from_byte_index(&self.layout, self.spans.len(), Affinity::Upstream)
        } else {
            Cursor::from_byte_index(&self.layout, index, Affinity::Downstream)
        }
    }

    /// Replace `old_range` of the text buffer with `s`, keeping style spans and the IME
    /// compose range consistent with the edit.
    fn replace_text(&mut self, old_range: Range<usize>, s: &str) {
        let new_len = s.len();
        let mut text = self.spans.text().clone();
        if old_range.is_empty() {
            text.insert_str(old_range.start, s);
        } else {
            text.replace_range(old_range.clone(), s);
        }
        let shifted: Vec<_> = self
            .spans
            .attributes_iter()
            .filter_map(|(range, attr)| {
                shift_span_range(range.as_range(), old_range.clone(), new_len)
                    .map(|range| (range, attr.clone()))
            })
            .collect();
        self.spans.set_text(text);
        for (range, attr) in shifted {
            self.spans
                .apply_attribute(TextRange::new_unchecked(range.start, range.end), attr);
        }
        self.compose = self
            .compose
            .take()
            .and_then(|range| shift_span_range(range, old_range.clone(), new_len));
    }

    fn replace_selection(
        &mut self,
        font_cx: &mut FontContext,
        layout_cx: &mut LayoutContext<T>,
        s: &str,
    ) {
        let range = self.selection.text_range();
        let start = range.start;
        self.replace_text(range, s);

        self.update_layout(font_cx, layout_cx);
        let new_index = start.saturating_add(s.len());
        let affinity = if s.ends_with(['\n', '\r', '\u{2028}', '\u{2029}']) {
            Affinity::Downstream
        } else {
            Affinity::Upstream
        };
        self.set_selection(Cursor::from_byte_index(&self.layout, new_index, affinity).into());
    }

    /// Update the selection, and nudge the `Generation` if something other than `h_pos` changed.
    fn set_selection(&mut self, new_sel: Selection) {
        if new_sel.focus() != self.selection.focus() || new_sel.anchor() != self.selection.anchor()
        {
            self.generation.nudge();
        }
        self.selection = new_sel;
    }

    /// Update the layout.
    fn update_layout(&mut self, font_cx: &mut FontContext, layout_cx: &mut LayoutContext<T>) {
        let mut builder =
            layout_cx.ranged_builder(font_cx, self.spans.text(), self.scale, self.quantize);
        for prop in self.default_style.inner().values() {
            builder.push_default(prop.clone());
        }
        for (range, prop) in self.spans.attributes_iter() {
            builder.push(prop.clone(), range.as_range());
        }
        if let Some(preedit_range) = &self.compose {
            builder.push(crate::StyleProperty::Underline(true), preedit_range.clone());
        }
        self.layout = builder.build(self.spans.text());
        self.layout.break_all_lines(self.width);
        self.layout
            .align(self.alignment, AlignmentOptions::default());
        self.selection = self.selection.refresh(&self.layout);
        self.layout_dirty = false;
        self.generation.nudge();
    }

    #[cfg(feature = "accesskit")]
    /// Perform an accessibility update, assuming that the layout is valid.
    ///
    /// The wrapper [`accessibility`](RichEditorDriver::accessibility) on the driver type should
    /// be preferred.
    ///
    /// You should always call [`refresh_layout`](Self::refresh_layout) before using this method,
    /// with no other modifying method calls in between.
    fn accessibility_unchecked(
        &mut self,
        update: &mut TreeUpdate,
        node: &mut Node,
        next_node_id: impl FnMut() -> NodeId,
        x_offset: f64,
        y_offset: f64,
        set_brush_properties: impl Fn(&mut Node, &crate::Style<T>),
    ) {
        self.layout_access.build_nodes(
            self.spans.text(),
            &self.layout,
            update,
            node,
            next_node_id,
            x_offset,
            y_offset,
            set_brush_properties,
        );
        if self.show_cursor {
            if let Some(selection) = self
                .selection
                .to_access_selection(&self.layout, &self.layout_access)
            {
                node.set_text_selection(selection);
            }
        } else {
            node.clear_text_selection();
        }
        node.add_action(accesskit::Action::SetTextSelection);
    }
}