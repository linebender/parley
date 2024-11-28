// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::{cmp::PartialEq, default::Default, fmt::Debug};

#[cfg(feature = "accesskit")]
use crate::layout::LayoutAccessibility;
use crate::{
    layout::{
        cursor::{Cursor, Selection},
        Affinity, Alignment, Layout, Line,
    },
    style::{Brush, StyleProperty},
    FontContext, LayoutContext, Rect,
};
#[cfg(feature = "accesskit")]
use accesskit::{Node, NodeId, TreeUpdate};
use alloc::{borrow::ToOwned, string::String, sync::Arc, vec::Vec};

/// Opaque representation of a generation.
///
/// Obtained from [`PlainEditor::generation`].
// Overflow handling: the generations are only compared,
// so wrapping is fine. This could only fail if exactly
// `u32::MAX` generations happen between drawing
// operations. This is implausible and so can be ignored.
#[derive(PartialEq, Eq, Default, Clone, Copy)]
pub struct Generation(u32);

impl Generation {
    /// Make it not what it currently is.
    pub(crate) fn nudge(&mut self) {
        self.0 = self.0.wrapping_add(1);
    }
}

/// Basic plain text editor with a single default style.
#[derive(Clone)]
pub struct PlainEditor<T>
where
    T: Brush + Clone + Debug + PartialEq + Default,
{
    default_style: Arc<[StyleProperty<'static, T>]>,
    buffer: String,
    layout: Layout<T>,
    #[cfg(feature = "accesskit")]
    layout_access: LayoutAccessibility,
    selection: Selection,
    width: Option<f32>,
    scale: f32,
    // Simple tracking of when the layout needs to be updated
    // before it can be used for `Selection` calculations or
    // for drawing.
    // Not all operations on `PlainEditor` need to operate on a
    // clean layout, and not all operations trigger a layout.
    layout_dirty: bool,
    generation: Generation,
}

// TODO: When MSRV >= 1.80 we can remove this. Default was not implemented for Arc<[T]> where T: !Default until 1.80
impl<T> Default for PlainEditor<T>
where
    T: Brush + Clone + Debug + PartialEq + Default,
{
    fn default() -> Self {
        Self {
            default_style: Arc::new([]),
            buffer: Default::default(),
            layout: Default::default(),
            #[cfg(feature = "accesskit")]
            layout_access: Default::default(),
            selection: Default::default(),
            width: Default::default(),
            scale: 1.0,
            layout_dirty: Default::default(),
            // We don't use the `default` value to start with, as our consumers
            // will choose to use that as their initial value, but will probably need
            // to redraw if they haven't already.
            generation: Generation(1),
        }
    }
}

/// The argument passed to the callback of [`PlainEditor::transact`],
/// on which the caller performs operations.
pub struct PlainEditorTxn<'a, T>
where
    T: Brush + Clone + Debug + PartialEq + Default,
{
    editor: &'a mut PlainEditor<T>,
    font_cx: &'a mut FontContext,
    layout_cx: &'a mut LayoutContext<T>,
}

impl<T> PlainEditorTxn<'_, T>
where
    T: Brush + Clone + Debug + PartialEq + Default,
{
    /// Replace the whole text buffer.
    pub fn set_text(&mut self, is: &str) {
        self.editor.buffer.clear();
        self.editor.buffer.push_str(is);
        self.editor.layout_dirty = true;
    }

    /// Set the width of the layout.
    pub fn set_width(&mut self, width: Option<f32>) {
        self.editor.width = width;
        self.editor.layout_dirty = true;
    }

    /// Set the scale for the layout.
    pub fn set_scale(&mut self, scale: f32) {
        self.editor.scale = scale;
        self.editor.layout_dirty = true;
    }

    /// Set the default style for the layout.
    pub fn set_default_style(&mut self, style: Arc<[StyleProperty<'static, T>]>) {
        self.editor.default_style = style;
        self.editor.layout_dirty = true;
    }

    /// Insert at cursor, or replace selection.
    pub fn insert_or_replace_selection(&mut self, s: &str) {
        self.editor
            .replace_selection(self.font_cx, self.layout_cx, s);
    }

    /// Delete the selection.
    pub fn delete_selection(&mut self) {
        self.insert_or_replace_selection("");
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
                self.editor.buffer.replace_range(range, "");
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
            if self.editor.text().get(start..end).is_some() {
                self.editor.buffer.replace_range(start..end, "");
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
                .clone()
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
                        .text()
                        .get(..end)
                        .and_then(|str| str.char_indices().next_back())
                    else {
                        return;
                    };
                    start
                };
                self.editor.buffer.replace_range(start..end, "");
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
            if self.editor.text().get(start..end).is_some() {
                self.editor.buffer.replace_range(start..end, "");
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
        if self.editor.buffer.is_char_boundary(index) {
            self.refresh_layout();
            self.editor
                .set_selection(self.editor.cursor_at(index).into());
        }
    }

    /// Move the cursor to the start of the buffer.
    pub fn move_to_text_start(&mut self) {
        self.editor.set_selection(self.editor.selection.move_lines(
            &self.editor.layout,
            isize::MIN,
            false,
        ));
    }

    /// Move the cursor to the start of the physical line.
    pub fn move_to_line_start(&mut self) {
        self.editor
            .set_selection(self.editor.selection.line_start(&self.editor.layout, false));
    }

    /// Move the cursor to the end of the buffer.
    pub fn move_to_text_end(&mut self) {
        self.editor.set_selection(self.editor.selection.move_lines(
            &self.editor.layout,
            isize::MAX,
            false,
        ));
    }

    /// Move the cursor to the end of the physical line.
    pub fn move_to_line_end(&mut self) {
        self.editor
            .set_selection(self.editor.selection.line_end(&self.editor.layout, false));
    }

    /// Move up to the closest physical cluster boundary on the previous line, preserving the horizontal position for repeated movements.
    pub fn move_up(&mut self) {
        self.editor.set_selection(
            self.editor
                .selection
                .previous_line(&self.editor.layout, false),
        );
    }

    /// Move down to the closest physical cluster boundary on the next line, preserving the horizontal position for repeated movements.
    pub fn move_down(&mut self) {
        self.editor
            .set_selection(self.editor.selection.next_line(&self.editor.layout, false));
    }

    /// Move to the next cluster left in visual order.
    pub fn move_left(&mut self) {
        self.editor.set_selection(
            self.editor
                .selection
                .previous_visual(&self.editor.layout, false),
        );
    }

    /// Move to the next cluster right in visual order.
    pub fn move_right(&mut self) {
        self.editor.set_selection(
            self.editor
                .selection
                .next_visual(&self.editor.layout, false),
        );
    }

    /// Move to the next word boundary left.
    pub fn move_word_left(&mut self) {
        self.editor.set_selection(
            self.editor
                .selection
                .previous_visual_word(&self.editor.layout, false),
        );
    }

    /// Move to the next word boundary right.
    pub fn move_word_right(&mut self) {
        self.editor.set_selection(
            self.editor
                .selection
                .next_visual_word(&self.editor.layout, false),
        );
    }

    /// Select the whole buffer.
    pub fn select_all(&mut self) {
        self.editor.set_selection(
            Selection::from_byte_index(&self.editor.layout, 0usize, Affinity::default())
                .move_lines(&self.editor.layout, isize::MAX, true),
        );
    }

    /// Collapse selection into caret.
    pub fn collapse_selection(&mut self) {
        self.editor.set_selection(self.editor.selection.collapse());
    }

    /// Move the selection focus point to the start of the buffer.
    pub fn select_to_text_start(&mut self) {
        self.editor.set_selection(self.editor.selection.move_lines(
            &self.editor.layout,
            isize::MIN,
            true,
        ));
    }

    /// Move the selection focus point to the start of the physical line.
    pub fn select_to_line_start(&mut self) {
        self.editor
            .set_selection(self.editor.selection.line_start(&self.editor.layout, true));
    }

    /// Move the selection focus point to the end of the buffer.
    pub fn select_to_text_end(&mut self) {
        self.editor.set_selection(self.editor.selection.move_lines(
            &self.editor.layout,
            isize::MAX,
            true,
        ));
    }

    /// Move the selection focus point to the end of the physical line.
    pub fn select_to_line_end(&mut self) {
        self.editor
            .set_selection(self.editor.selection.line_end(&self.editor.layout, true));
    }

    /// Move the selection focus point up to the nearest cluster boundary on the previous line, preserving the horizontal position for repeated movements.
    pub fn select_up(&mut self) {
        self.editor.set_selection(
            self.editor
                .selection
                .previous_line(&self.editor.layout, true),
        );
    }

    /// Move the selection focus point down to the nearest cluster boundary on the next line, preserving the horizontal position for repeated movements.
    pub fn select_down(&mut self) {
        self.editor
            .set_selection(self.editor.selection.next_line(&self.editor.layout, true));
    }

    /// Move the selection focus point to the next cluster left in visual order.
    pub fn select_left(&mut self) {
        self.editor.set_selection(
            self.editor
                .selection
                .previous_visual(&self.editor.layout, true),
        );
    }

    /// Move the selection focus point to the next cluster right in visual order.
    pub fn select_right(&mut self) {
        self.editor
            .set_selection(self.editor.selection.next_visual(&self.editor.layout, true));
    }

    /// Move the selection focus point to the next word boundary left.
    pub fn select_word_left(&mut self) {
        self.editor.set_selection(
            self.editor
                .selection
                .previous_visual_word(&self.editor.layout, true),
        );
    }

    /// Move the selection focus point to the next word boundary right.
    pub fn select_word_right(&mut self) {
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
    pub fn select_line_at_point(&mut self, x: f32, y: f32) {
        self.refresh_layout();
        let line = Selection::line_from_point(&self.editor.layout, x, y);
        self.editor.set_selection(line);
    }

    /// Move the selection focus point to the cluster boundary closest to point.
    pub fn extend_selection_to_point(&mut self, x: f32, y: f32) {
        self.refresh_layout();
        // FIXME: This is usually the wrong way to handle selection extension for mouse moves, but not a regression.
        self.editor.set_selection(
            self.editor
                .selection
                .extend_to_point(&self.editor.layout, x, y),
        );
    }

    /// Move the selection focus point to a byte index.
    ///
    /// No-op if index is not a char boundary.
    pub fn extend_selection_to_byte(&mut self, index: usize) {
        if self.editor.buffer.is_char_boundary(index) {
            self.refresh_layout();
            self.editor.set_selection(
                self.editor
                    .selection
                    .maybe_extend(self.editor.cursor_at(index), true),
            );
        }
    }

    /// Select a range of byte indices
    ///
    /// No-op if either index is not a char boundary.
    pub fn select_byte_range(&mut self, start: usize, end: usize) {
        if self.editor.buffer.is_char_boundary(start) && self.editor.buffer.is_char_boundary(end) {
            self.refresh_layout();
            self.editor.set_selection(
                Selection::from(self.editor.cursor_at(start))
                    .maybe_extend(self.editor.cursor_at(end), true),
            );
        }
    }

    #[cfg(feature = "accesskit")]
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

    fn update_layout(&mut self) {
        self.editor.update_layout(self.font_cx, self.layout_cx);
    }

    fn refresh_layout(&mut self) {
        self.editor.refresh_layout(self.font_cx, self.layout_cx);
    }
}

impl<T> PlainEditor<T>
where
    T: Brush + Clone + Debug + PartialEq + Default,
{
    /// Run a series of [`PlainEditorTxn`] methods, updating the layout
    /// if necessary.
    pub fn transact(
        &mut self,
        font_cx: &mut FontContext,
        layout_cx: &mut LayoutContext<T>,
        callback: impl FnOnce(&mut PlainEditorTxn<'_, T>),
    ) {
        let mut txn = PlainEditorTxn {
            editor: self,
            font_cx,
            layout_cx,
        };
        callback(&mut txn);
        txn.refresh_layout();
    }

    /// Make a cursor at a given byte index
    fn cursor_at(&self, index: usize) -> Cursor {
        // FIXME: `Selection` should make this easier
        if index >= self.buffer.len() {
            Cursor::from_byte_index(&self.layout, self.buffer.len(), Affinity::Upstream)
        } else {
            Cursor::from_byte_index(&self.layout, index, Affinity::Downstream)
        }
    }

    fn replace_selection(
        &mut self,
        font_cx: &mut FontContext,
        layout_cx: &mut LayoutContext<T>,
        s: &str,
    ) {
        let range = self.selection.text_range();
        let start = range.start;
        if self.selection.is_collapsed() {
            self.buffer.insert_str(start, s);
        } else {
            self.buffer.replace_range(range, s);
        }

        self.update_layout(font_cx, layout_cx);
        let new_index = start.saturating_add(s.len());
        let affinity = if s.ends_with("\n") {
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
        // Keeping this commented debug code in for now because it's quite
        // useful when diagnosing selection problems:
        //----------------------------------------------------------------------
        // #[cfg(feature = "std")]
        // {
        //     let focus = new_sel.focus();
        //     let cluster = focus.logical_clusters(&self.layout);
        //     let dbg = (
        //         cluster[0].as_ref().map(|c| &self.buffer[c.text_range()]),
        //         focus.index(),
        //         focus.affinity(),
        //         cluster[1].as_ref().map(|c| &self.buffer[c.text_range()]),
        //     );
        //     print!("{dbg:?}");
        //     let cluster = focus.visual_clusters(&self.layout);
        //     let dbg = (
        //         cluster[0].as_ref().map(|c| &self.buffer[c.text_range()]),
        //         cluster[0]
        //             .as_ref()
        //             .map(|c| if c.is_word_boundary() { " W" } else { "" })
        //             .unwrap_or_default(),
        //         focus.index(),
        //         focus.affinity(),
        //         cluster[1].as_ref().map(|c| &self.buffer[c.text_range()]),
        //         cluster[1]
        //             .as_ref()
        //             .map(|c| if c.is_word_boundary() { " W" } else { "" })
        //             .unwrap_or_default(),
        //     );
        //     println!(" | visual: {dbg:?}");
        // }
        self.selection = new_sel;
    }

    /// If the current selection is not collapsed, returns the text content of
    /// that selection.
    pub fn selected_text(&self) -> Option<&str> {
        if !self.selection.is_collapsed() {
            self.text().get(self.selection.text_range())
        } else {
            None
        }
    }

    /// Get rectangles representing the selected portions of text.
    pub fn selection_geometry(&self) -> Vec<Rect> {
        self.selection.geometry(&self.layout)
    }

    /// Get a rectangle representing the current caret cursor position.
    pub fn cursor_geometry(&self, size: f32) -> Option<Rect> {
        Some(self.selection.focus().geometry(&self.layout, size))
    }

    /// Returns the underlying `Layout`.
    pub fn layout(&self) -> &Layout<T> {
        &self.layout
    }

    /// Get the lines from the `Layout`.
    pub fn lines(&self) -> impl Iterator<Item = Line<T>> + '_ + Clone {
        self.layout.lines()
    }

    /// Borrow the text content of the buffer.
    pub fn text(&self) -> &str {
        &self.buffer
    }

    /// Get the current `Generation` of the layout, to decide whether to draw.
    ///
    /// You should store the generation the editor was at when you last drew it, and then redraw
    /// when the generation is different (`Generation` is [`PartialEq`], so supports the equality `==` operation).
    pub fn generation(&self) -> Generation {
        self.generation
    }

    /// Update the layout if it is dirty.
    fn refresh_layout(&mut self, font_cx: &mut FontContext, layout_cx: &mut LayoutContext<T>) {
        if self.layout_dirty {
            self.update_layout(font_cx, layout_cx);
        }
    }

    /// Update the layout.
    fn update_layout(&mut self, font_cx: &mut FontContext, layout_cx: &mut LayoutContext<T>) {
        let mut builder = layout_cx.ranged_builder(font_cx, &self.buffer, self.scale);
        for prop in self.default_style.iter() {
            builder.push_default(prop.to_owned());
        }
        builder.build_into(&mut self.layout, &self.buffer);
        self.layout.break_all_lines(self.width);
        self.layout.align(self.width, Alignment::Start);
        self.selection = self.selection.refresh(&self.layout);
        self.layout_dirty = false;
        self.generation.nudge();
    }

    #[cfg(feature = "accesskit")]
    pub fn accessibility(
        &mut self,
        update: &mut TreeUpdate,
        node: &mut Node,
        next_node_id: impl FnMut() -> NodeId,
        x_offset: f64,
        y_offset: f64,
    ) {
        self.layout_access.build_nodes(
            &self.buffer,
            &self.layout,
            update,
            node,
            next_node_id,
            x_offset,
            y_offset,
        );
        if let Some(selection) = self
            .selection
            .to_access_selection(&self.layout, &self.layout_access)
        {
            node.set_text_selection(selection);
        }
        node.add_action(accesskit::Action::SetTextSelection);
    }
}
