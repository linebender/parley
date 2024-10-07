// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::{cmp::PartialEq, default::Default, fmt::Debug, iter::IntoIterator};

use crate::{
    layout::{
        cursor::{Selection, VisualMode},
        Affinity, Alignment, Layout, Line,
    },
    style::{Brush, StyleProperty},
    FontContext, LayoutContext, Rect,
};
use alloc::{sync::Arc, vec::Vec};

#[derive(Copy, Clone, Debug)]
pub enum ActiveText<'a> {
    /// The selection is empty and the cursor is a caret; this is the text of the cluster it is on
    FocusedCluster(Affinity, &'a str),
    /// The selection contains this text
    Selection(&'a str),
}

/// Basic plain text editor with a single default style.
pub struct PlainEditor<'a, T>
where
    T: Brush + Clone + Debug + PartialEq + Default,
{
    default_style: Arc<[StyleProperty<'a, T>]>,
    buffer: String,
    layout: Layout<T>,
    selection: Selection,
    cursor_mode: VisualMode,
    width: f32,
    scale: f32,
}

// TODO: When MSRV >= 1.80 we can remove this. Default was not implemented for Arc<[T]> where T: !Default until 1.80
impl<'a, T> Default for PlainEditor<'a, T>
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
            scale: Default::default(),
        }
    }
}

/// Operations on a `PlainEditor` for `PlainEditor::transact`
#[non_exhaustive]
pub enum PlainEditorOp<'a, T>
where
    T: Brush + Clone + Debug + PartialEq + Default,
{
    /// Replace the whole text buffer
    SetText(Arc<str>),
    /// Set the width of the layout
    SetWidth(f32),
    /// Set the scale for the layout
    SetScale(f32),
    /// Set the default style for the layout
    SetDefaultStyle(Arc<[StyleProperty<'a, T>]>),
    /// Insert at cursor, or replace selection
    InsertOrReplaceSelection(Arc<str>),
    /// Delete the selection
    DeleteSelection,
    /// Delete the selection or the next cluster (typical ‘delete’ behavior)
    Delete,
    /// Delete the selection or up to the next word boundary (typical ‘ctrl + delete’ behavior)
    DeleteWord,
    /// Delete the selection or the previous cluster (typical ‘backspace’ behavior)
    Backdelete,
    /// Delete the selection or back to the previous word boundary (typical ‘ctrl + backspace’ behavior)
    BackdeleteWord,
    /// Move the cursor to the cluster boundary nearest this point in the layout
    MoveToPoint(f32, f32),
    /// Move the cursor to the start of the buffer
    MoveToTextStart,
    /// Move the cursor to the start of the physical line
    MoveToLineStart,
    /// Move the cursor to the end of the buffer
    MoveToTextEnd,
    /// Move the cursor to the end of the physical line
    MoveToLineEnd,
    /// Move up to the closest physical cluster boundary on the previous line, preserving the horizontal position for repeated movements
    MoveUp,
    /// Move down to the closest physical cluster boundary on the next line, preserving the horizontal position for repeated movements
    MoveDown,
    /// Move to the next cluster left in visual order
    MoveLeft,
    /// Move to the next cluster right in visual order
    MoveRight,
    /// Move to the next word boundary left
    MoveWordLeft,
    /// Move to the next word boundary right
    MoveWordRight,
    /// Select the whole buffer
    SelectAll,
    /// Collapse selection into caret
    CollapseSelection,
    /// Move the selection focus point to the start of the buffer
    SelectToTextStart,
    /// Move the selection focus point to the start of the physical line
    SelectToLineStart,
    /// Move the selection focus point to the end of the buffer
    SelectToTextEnd,
    /// Move the selection focus point to the end of the physical line
    SelectToLineEnd,
    /// Move the selection focus point up to the nearest cluster boundary on the previous line, preserving the horizontal position for repeated movements
    SelectUp,
    /// Move the selection focus point down to the nearest cluster boundary on the next line, preserving the horizontal position for repeated movements
    SelectDown,
    /// Move the selection focus point to the next cluster left in visual order
    SelectLeft,
    /// Move the selection focus point to the next cluster right in visual order
    SelectRight,
    /// Move the selection focus point to the next word boundary left
    SelectWordLeft,
    /// Move the selection focus point to the next word boundary right
    SelectWordRight,
    /// Select the word at the point
    SelectWordAtPoint(f32, f32),
    /// Select the physical line at the point
    SelectLineAtPoint(f32, f32),
    /// Move the selection focus point to the cluster boundary closest to point
    ExtendSelectionToPoint(f32, f32),
}

impl<'a, T> PlainEditor<'a, T>
where
    T: Brush + Clone + Debug + PartialEq + Default,
{
    /// Run a series of `PlainEditorOp`s, updating the layout if necessary
    pub fn transact(
        &mut self,
        font_cx: &mut FontContext,
        layout_cx: &mut LayoutContext<T>,
        t: impl IntoIterator<Item = PlainEditorOp<'a, T>>,
    ) {
        let mut layout_after = false;

        for op in t.into_iter() {
            match op {
                PlainEditorOp::SetText(is) => {
                    self.buffer.clear();
                    self.buffer.push_str(&is);
                    layout_after = true;
                }
                PlainEditorOp::SetWidth(width) => {
                    self.width = width;
                    layout_after = true;
                }
                PlainEditorOp::SetScale(scale) => {
                    self.scale = scale;
                    layout_after = true;
                }
                PlainEditorOp::SetDefaultStyle(style) => {
                    self.default_style = style.clone();
                    layout_after = true;
                }
                PlainEditorOp::DeleteSelection => {
                    self.replace_selection(font_cx, layout_cx, "");
                }
                PlainEditorOp::Delete => {
                    if self.selection.is_collapsed() {
                        let range = self.selection.focus().text_range();
                        if !range.is_empty() {
                            let start = range.start;
                            self.buffer.replace_range(range, "");
                            self.update_layout(font_cx, layout_cx);
                            self.selection = if start == self.buffer.len() {
                                Selection::from_index(
                                    &self.layout,
                                    start.saturating_sub(1),
                                    Affinity::Upstream,
                                )
                            } else {
                                Selection::from_index(
                                    &self.layout,
                                    start.min(self.buffer.len()),
                                    Affinity::Downstream,
                                )
                            };
                        }
                    } else {
                        self.replace_selection(font_cx, layout_cx, "");
                    }
                }
                PlainEditorOp::DeleteWord => {
                    let start = self.selection.insertion_index();
                    if self.selection.is_collapsed() {
                        let end = self
                            .selection
                            .focus()
                            .next_word(&self.layout)
                            .text_range()
                            .end;

                        self.buffer.replace_range(start..end, "");
                        self.update_layout(font_cx, layout_cx);
                        let (start, affinity) = if start > 0 {
                            (start - 1, Affinity::Upstream)
                        } else {
                            (start, Affinity::Downstream)
                        };
                        self.selection = Selection::from_index(&self.layout, start, affinity);
                    } else {
                        self.replace_selection(font_cx, layout_cx, "");
                    }
                }
                PlainEditorOp::Backdelete => {
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
                            let (start, affinity) = if start > 0 {
                                (start - 1, Affinity::Upstream)
                            } else {
                                (start, Affinity::Downstream)
                            };
                            self.selection = Selection::from_index(&self.layout, start, affinity);
                        }
                    } else {
                        self.replace_selection(font_cx, layout_cx, "");
                    }
                }
                PlainEditorOp::BackdeleteWord => {
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
                        let (start, affinity) = if start > 0 {
                            (start - 1, Affinity::Upstream)
                        } else {
                            (start, Affinity::Downstream)
                        };
                        self.selection = Selection::from_index(&self.layout, start, affinity);
                    } else {
                        self.replace_selection(font_cx, layout_cx, "");
                    }
                }
                PlainEditorOp::InsertOrReplaceSelection(s) => {
                    self.replace_selection(font_cx, layout_cx, &s);
                }
                PlainEditorOp::MoveToPoint(x, y) => {
                    self.selection = Selection::from_point(&self.layout, x, y);
                }
                PlainEditorOp::MoveToTextStart => {
                    self.selection = self.selection.move_lines(&self.layout, isize::MIN, false);
                }
                PlainEditorOp::MoveToLineStart => {
                    self.selection = self.selection.line_start(&self.layout, false);
                }
                PlainEditorOp::MoveToTextEnd => {
                    self.selection = self.selection.move_lines(&self.layout, isize::MAX, false);
                }
                PlainEditorOp::MoveToLineEnd => {
                    self.selection = self.selection.line_end(&self.layout, false);
                }
                PlainEditorOp::MoveUp => {
                    self.selection = self.selection.previous_line(&self.layout, false);
                }
                PlainEditorOp::MoveDown => {
                    self.selection = self.selection.next_line(&self.layout, false);
                }
                PlainEditorOp::MoveLeft => {
                    self.selection =
                        self.selection
                            .previous_visual(&self.layout, self.cursor_mode, false);
                }
                PlainEditorOp::MoveRight => {
                    self.selection =
                        self.selection
                            .next_visual(&self.layout, self.cursor_mode, false);
                }
                PlainEditorOp::MoveWordLeft => {
                    self.selection = self.selection.previous_word(&self.layout, false);
                }
                PlainEditorOp::MoveWordRight => {
                    self.selection = self.selection.next_word(&self.layout, false);
                }
                PlainEditorOp::SelectAll => {
                    self.selection =
                        Selection::from_index(&self.layout, 0usize, Affinity::default())
                            .move_lines(&self.layout, isize::MAX, true);
                }
                PlainEditorOp::CollapseSelection => {
                    self.selection = self.selection.collapse();
                }
                PlainEditorOp::SelectToTextStart => {
                    self.selection = self.selection.move_lines(&self.layout, isize::MIN, true);
                }
                PlainEditorOp::SelectToLineStart => {
                    self.selection = self.selection.line_start(&self.layout, true);
                }
                PlainEditorOp::SelectToTextEnd => {
                    self.selection = self.selection.move_lines(&self.layout, isize::MAX, true);
                }
                PlainEditorOp::SelectToLineEnd => {
                    self.selection = self.selection.line_end(&self.layout, true);
                }
                PlainEditorOp::SelectUp => {
                    self.selection = self.selection.previous_line(&self.layout, true);
                }
                PlainEditorOp::SelectDown => {
                    self.selection = self.selection.next_line(&self.layout, true);
                }
                PlainEditorOp::SelectLeft => {
                    self.selection =
                        self.selection
                            .previous_visual(&self.layout, self.cursor_mode, true);
                }
                PlainEditorOp::SelectRight => {
                    self.selection =
                        self.selection
                            .next_visual(&self.layout, self.cursor_mode, true);
                }
                PlainEditorOp::SelectWordLeft => {
                    self.selection = self.selection.previous_word(&self.layout, true);
                }
                PlainEditorOp::SelectWordRight => {
                    self.selection = self.selection.next_word(&self.layout, true);
                }
                PlainEditorOp::SelectWordAtPoint(x, y) => {
                    self.selection = Selection::word_from_point(&self.layout, x, y);
                }
                PlainEditorOp::SelectLineAtPoint(x, y) => {
                    let focus = *Selection::from_point(&self.layout, x, y)
                        .line_start(&self.layout, true)
                        .focus();
                    self.selection = Selection::from(focus).line_end(&self.layout, true);
                }
                PlainEditorOp::ExtendSelectionToPoint(x, y) => {
                    // FIXME: This is usually the wrong way to handle selection extension for mouse moves, but not a regression.
                    self.selection = self.selection.extend_to_point(&self.layout, x, y);
                }
            }
        }

        if layout_after {
            self.update_layout(font_cx, layout_cx);
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
        let new_start = start.saturating_add(s.len());
        self.selection = if new_start == self.buffer.len() {
            Selection::from_index(
                &self.layout,
                new_start.saturating_sub(1),
                Affinity::Upstream,
            )
        } else {
            Selection::from_index(
                &self.layout,
                new_start.min(self.buffer.len()),
                Affinity::Downstream,
            )
        };
    }

    /// Get either the contents of the current selection, or the text of the cluster at the caret
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

    /// Get rectangles representing the selected portions of text
    pub fn selection_geometry(&self) -> Vec<Rect> {
        self.selection.geometry(&self.layout)
    }

    /// Get a rectangle representing the current caret cursor position
    pub fn selection_strong_geometry(&self, size: f32) -> Option<Rect> {
        self.selection.focus().strong_geometry(&self.layout, size)
    }

    pub fn selection_weak_geometry(&self, size: f32) -> Option<Rect> {
        self.selection.focus().weak_geometry(&self.layout, size)
    }

    /// Get the lines from the `Layout`
    pub fn lines(&self) -> impl Iterator<Item = Line<T>> + '_ + Clone {
        self.layout.lines()
    }

    /// Get a copy of the text content of the buffer
    pub fn text(&self) -> Arc<str> {
        self.buffer.clone().into()
    }

    /// Update the layout
    fn update_layout(&mut self, font_cx: &mut FontContext, layout_cx: &mut LayoutContext<T>) {
        let mut builder = layout_cx.ranged_builder(font_cx, &self.buffer, self.scale);
        for prop in self.default_style.iter() {
            builder.push_default(prop);
        }
        builder.build_into(&mut self.layout, &self.buffer);
        self.layout.break_all_lines(Some(self.width));
        self.layout.align(Some(self.width), Alignment::Start);
        self.selection = self.selection.refresh(&self.layout);
    }
}
