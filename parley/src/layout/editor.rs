// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::{cmp::PartialEq, default::Default, fmt::Debug, iter::IntoIterator};

use crate::{
    layout::{
        cursor::{Cursor, Selection, VisualMode},
        Affinity, Alignment, Layout, Line,
    },
    style::{Brush, StyleProperty},
    FontContext, LayoutContext, Rect,
};
use alloc::{sync::Arc, vec::Vec};

#[derive(Copy, Clone, Debug)]
pub enum ActiveText<'a> {
    /// The selection is empty and the cursor is a caret; this is the text of the cluster it is on.
    FocusedCluster(Affinity, &'a str),
    /// The selection contains this text.
    Selection(&'a str),
}

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
    selection: Selection,
    cursor_mode: VisualMode,
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
            selection: Default::default(),
            cursor_mode: Default::default(),
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

/// Operations on a `PlainEditor` for `PlainEditor::transact`.
#[non_exhaustive]
pub enum PlainEditorOp<T>
where
    T: Brush + Clone + Debug + PartialEq + Default,
{
    /// Replace the whole text buffer.
    SetText(Arc<str>),
    /// Set the width of the layout.
    SetWidth(Option<f32>),
    /// Set the scale for the layout.
    SetScale(f32),
    /// Set the default style for the layout.
    SetDefaultStyle(Arc<[StyleProperty<'static, T>]>),
    /// Insert at cursor, or replace selection.
    InsertOrReplaceSelection(Arc<str>),
    /// Delete the selection.
    DeleteSelection,
    /// Delete the selection or the next cluster (typical ‘delete’ behavior).
    Delete,
    /// Delete the selection or up to the next word boundary (typical ‘ctrl + delete’ behavior).
    DeleteWord,
    /// Delete the selection or the previous cluster (typical ‘backspace’ behavior).
    Backdelete,
    /// Delete the selection or back to the previous word boundary (typical ‘ctrl + backspace’ behavior).
    BackdeleteWord,
    /// Move the cursor to the cluster boundary nearest this point in the layout.
    MoveToPoint(f32, f32),
    /// Move the cursor to a byte index.
    ///
    /// No-op if index is not a char boundary.
    MoveToByte(usize),
    /// Move the cursor to the start of the buffer.
    MoveToTextStart,
    /// Move the cursor to the start of the physical line.
    MoveToLineStart,
    /// Move the cursor to the end of the buffer.
    MoveToTextEnd,
    /// Move the cursor to the end of the physical line.
    MoveToLineEnd,
    /// Move up to the closest physical cluster boundary on the previous line, preserving the horizontal position for repeated movements.
    MoveUp,
    /// Move down to the closest physical cluster boundary on the next line, preserving the horizontal position for repeated movements.
    MoveDown,
    /// Move to the next cluster left in visual order.
    MoveLeft,
    /// Move to the next cluster right in visual order.
    MoveRight,
    /// Move to the next word boundary left.
    MoveWordLeft,
    /// Move to the next word boundary right.
    MoveWordRight,
    /// Select the whole buffer.
    SelectAll,
    /// Collapse selection into caret.
    CollapseSelection,
    /// Move the selection focus point to the start of the buffer.
    SelectToTextStart,
    /// Move the selection focus point to the start of the physical line.
    SelectToLineStart,
    /// Move the selection focus point to the end of the buffer.
    SelectToTextEnd,
    /// Move the selection focus point to the end of the physical line.
    SelectToLineEnd,
    /// Move the selection focus point up to the nearest cluster boundary on the previous line, preserving the horizontal position for repeated movements.
    SelectUp,
    /// Move the selection focus point down to the nearest cluster boundary on the next line, preserving the horizontal position for repeated movements.
    SelectDown,
    /// Move the selection focus point to the next cluster left in visual order.
    SelectLeft,
    /// Move the selection focus point to the next cluster right in visual order.
    SelectRight,
    /// Move the selection focus point to the next word boundary left.
    SelectWordLeft,
    /// Move the selection focus point to the next word boundary right.
    SelectWordRight,
    /// Select the word at the point.
    SelectWordAtPoint(f32, f32),
    /// Select the physical line at the point.
    SelectLineAtPoint(f32, f32),
    /// Move the selection focus point to the cluster boundary closest to point.
    ExtendSelectionToPoint(f32, f32),
    /// Move the selection focus point to a byte index.
    ///
    /// No-op if index is not a char boundary.
    ExtendSelectionToByte(usize),
    /// Select a range of byte indices
    ///
    /// No-op if either index is not a char boundary.
    SelectByteRange(usize, usize),
}

impl<T> PlainEditor<T>
where
    T: Brush + Clone + Debug + PartialEq + Default,
{
    /// Run a series of `PlainEditorOp`s, updating the layout if necessary.
    pub fn transact(
        &mut self,
        font_cx: &mut FontContext,
        layout_cx: &mut LayoutContext<T>,
        t: impl IntoIterator<Item = PlainEditorOp<T>>,
    ) {
        for op in t.into_iter() {
            use PlainEditorOp::*;
            match op {
                SetText(is) => {
                    self.buffer.clear();
                    self.buffer.push_str(&is);
                    self.layout_dirty = true;
                }
                SetWidth(width) => {
                    self.width = width;
                    self.layout_dirty = true;
                }
                SetScale(scale) => {
                    self.scale = scale;
                    self.layout_dirty = true;
                }
                SetDefaultStyle(style) => {
                    self.default_style = style.clone();
                    self.layout_dirty = true;
                }
                DeleteSelection => {
                    self.replace_selection(font_cx, layout_cx, "");
                }
                Delete => {
                    if self.selection.is_collapsed() {
                        let range = self.selection.focus().text_range();
                        if !range.is_empty() {
                            let start = range.start;
                            self.buffer.replace_range(range, "");
                            self.update_layout(font_cx, layout_cx);
                            self.set_selection(self.cursor_at(start).into());
                        }
                    } else {
                        self.replace_selection(font_cx, layout_cx, "");
                    }
                }
                DeleteWord => {
                    let start = self.selection.focus().text_range().start;
                    if self.selection.is_collapsed() {
                        let end = self.cursor_at(start).next_word(&self.layout).index();

                        self.buffer.replace_range(start..end, "");
                        self.update_layout(font_cx, layout_cx);
                        self.set_selection(self.cursor_at(start).into());
                    } else {
                        self.replace_selection(font_cx, layout_cx, "");
                    }
                }
                Backdelete => {
                    let end = self.selection.focus().text_range().start;
                    if self.selection.is_collapsed() {
                        if let Some(start) = self
                            .selection
                            .focus()
                            .cluster_path()
                            .cluster(&self.layout)
                            .map(|x| {
                                if self.selection.focus().affinity() == Affinity::Upstream {
                                    Some(x)
                                } else {
                                    x.previous_logical()
                                }
                            })
                            .and_then(|c| c.map(|x| x.text_range().start))
                        {
                            self.buffer.replace_range(start..end, "");
                            self.update_layout(font_cx, layout_cx);
                            self.set_selection(self.cursor_at(start).into());
                        }
                    } else {
                        self.replace_selection(font_cx, layout_cx, "");
                    }
                }
                BackdeleteWord => {
                    let end = self.selection.focus().text_range().start;
                    if self.selection.is_collapsed() {
                        let start = self
                            .selection
                            .focus()
                            .previous_word(&self.layout)
                            .text_range()
                            .start;

                        self.buffer.replace_range(start..end, "");
                        self.update_layout(font_cx, layout_cx);
                        self.set_selection(self.cursor_at(start).into());
                    } else {
                        self.replace_selection(font_cx, layout_cx, "");
                    }
                }
                InsertOrReplaceSelection(s) => {
                    self.replace_selection(font_cx, layout_cx, &s);
                }
                MoveToPoint(x, y) => {
                    self.refresh_layout(font_cx, layout_cx);
                    self.set_selection(Selection::from_point(&self.layout, x, y));
                }
                MoveToByte(index) => {
                    if self.buffer.is_char_boundary(index) {
                        self.refresh_layout(font_cx, layout_cx);
                        self.set_selection(self.cursor_at(index).into());
                    }
                }
                MoveToTextStart => {
                    self.set_selection(self.selection.move_lines(&self.layout, isize::MIN, false));
                }
                MoveToLineStart => {
                    self.set_selection(self.selection.line_start(&self.layout, false));
                }
                MoveToTextEnd => {
                    self.set_selection(self.selection.move_lines(&self.layout, isize::MAX, false));
                }
                MoveToLineEnd => {
                    self.set_selection(self.selection.line_end(&self.layout, false));
                }
                MoveUp => {
                    self.set_selection(self.selection.previous_line(&self.layout, false));
                }
                MoveDown => {
                    self.set_selection(self.selection.next_line(&self.layout, false));
                }
                MoveLeft => {
                    self.set_selection(self.selection.previous_visual(
                        &self.layout,
                        self.cursor_mode,
                        false,
                    ));
                }
                MoveRight => {
                    self.set_selection(self.selection.next_visual(
                        &self.layout,
                        self.cursor_mode,
                        false,
                    ));
                }
                MoveWordLeft => {
                    self.set_selection(self.selection.previous_word(&self.layout, false));
                }
                MoveWordRight => {
                    self.set_selection(self.selection.next_word(&self.layout, false));
                }
                SelectAll => {
                    self.set_selection(
                        Selection::from_index(&self.layout, 0usize, Affinity::default())
                            .move_lines(&self.layout, isize::MAX, true),
                    );
                }
                CollapseSelection => {
                    self.set_selection(self.selection.collapse());
                }
                SelectToTextStart => {
                    self.set_selection(self.selection.move_lines(&self.layout, isize::MIN, true));
                }
                SelectToLineStart => {
                    self.set_selection(self.selection.line_start(&self.layout, true));
                }
                SelectToTextEnd => {
                    self.set_selection(self.selection.move_lines(&self.layout, isize::MAX, true));
                }
                SelectToLineEnd => {
                    self.set_selection(self.selection.line_end(&self.layout, true));
                }
                SelectUp => {
                    self.set_selection(self.selection.previous_line(&self.layout, true));
                }
                SelectDown => {
                    self.set_selection(self.selection.next_line(&self.layout, true));
                }
                SelectLeft => {
                    self.set_selection(self.selection.previous_visual(
                        &self.layout,
                        self.cursor_mode,
                        true,
                    ));
                }
                SelectRight => {
                    self.set_selection(self.selection.next_visual(
                        &self.layout,
                        self.cursor_mode,
                        true,
                    ));
                }
                SelectWordLeft => {
                    self.set_selection(self.selection.previous_word(&self.layout, true));
                }
                SelectWordRight => {
                    self.set_selection(self.selection.next_word(&self.layout, true));
                }
                SelectWordAtPoint(x, y) => {
                    self.refresh_layout(font_cx, layout_cx);
                    self.set_selection(Selection::word_from_point(&self.layout, x, y));
                }
                SelectLineAtPoint(x, y) => {
                    self.refresh_layout(font_cx, layout_cx);
                    let focus = *Selection::from_point(&self.layout, x, y)
                        .line_start(&self.layout, true)
                        .focus();
                    self.set_selection(Selection::from(focus).line_end(&self.layout, true));
                }
                ExtendSelectionToPoint(x, y) => {
                    self.refresh_layout(font_cx, layout_cx);
                    // FIXME: This is usually the wrong way to handle selection extension for mouse moves, but not a regression.
                    self.set_selection(self.selection.extend_to_point(&self.layout, x, y));
                }
                ExtendSelectionToByte(index) => {
                    if self.buffer.is_char_boundary(index) {
                        self.refresh_layout(font_cx, layout_cx);
                        self.set_selection(
                            self.selection.maybe_extend(self.cursor_at(index), true),
                        );
                    }
                }
                SelectByteRange(start, end) => {
                    if self.buffer.is_char_boundary(end) && self.buffer.is_char_boundary(end) {
                        self.refresh_layout(font_cx, layout_cx);
                        self.set_selection(
                            Selection::from(self.cursor_at(start))
                                .maybe_extend(self.cursor_at(end), true),
                        );
                    }
                }
            }
        }
        self.refresh_layout(font_cx, layout_cx);
    }

    /// Make a cursor at a given byte index
    fn cursor_at(&self, index: usize) -> Cursor {
        // FIXME: `Selection` should make this easier
        if index >= self.buffer.len() {
            Cursor::from_index(
                &self.layout,
                self.buffer.len().saturating_sub(1),
                Affinity::Upstream,
            )
        } else {
            Cursor::from_index(&self.layout, index, Affinity::Downstream)
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
        self.set_selection(self.cursor_at(start.saturating_add(s.len())).into());
    }

    /// Update the selection, and nudge the `Generation` if something other than `h_pos` changed.
    fn set_selection(&mut self, new_sel: Selection) {
        if new_sel.focus() != self.selection.focus() || new_sel.anchor() != self.selection.anchor()
        {
            self.generation.nudge();
        }

        self.selection = new_sel;
    }

    /// Get either the contents of the current selection, or the text of the cluster at the caret.
    pub fn active_text(&self) -> ActiveText {
        if self.selection.is_collapsed() {
            let range = self
                .selection
                .focus()
                .cluster_path()
                .cluster(&self.layout)
                .map(|c| c.text_range())
                .unwrap_or_default();
            ActiveText::FocusedCluster(self.selection.focus().affinity(), &self.buffer[range])
        } else {
            ActiveText::Selection(&self.buffer[self.selection.text_range()])
        }
    }

    /// Get rectangles representing the selected portions of text.
    pub fn selection_geometry(&self) -> Vec<Rect> {
        self.selection.geometry(&self.layout)
    }

    /// Get a rectangle representing the current caret cursor position.
    pub fn selection_strong_geometry(&self, size: f32) -> Option<Rect> {
        self.selection.focus().strong_geometry(&self.layout, size)
    }

    pub fn selection_weak_geometry(&self, size: f32) -> Option<Rect> {
        self.selection.focus().weak_geometry(&self.layout, size)
    }

    /// Get the lines from the `Layout`.
    pub fn lines(&self) -> impl Iterator<Item = Line<T>> + '_ + Clone {
        self.layout.lines()
    }

    /// Get a copy of the text content of the buffer.
    pub fn text(&self) -> Arc<str> {
        self.buffer.clone().into()
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
}
