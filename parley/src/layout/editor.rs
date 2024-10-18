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

#[derive(Clone)]
struct ComposeState {
    selection: Option<(usize, usize)>,
    text: Arc<str>,
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
    compose: Option<ComposeState>,
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
            scale: Default::default(),
            compose: Default::default(),
        }
    }
}

/// Operations on a `PlainEditor` for `PlainEditor::transact`
#[derive(Debug)]
#[non_exhaustive]
pub enum PlainEditorOp<T>
where
    T: Brush + Clone + Debug + PartialEq + Default,
{
    /// Replace the whole text buffer
    SetText(Arc<str>),
    /// Set the width of the layout
    SetWidth(Option<f32>),
    /// Set the scale for the layout
    SetScale(f32),
    /// Set the default style for the layout
    SetDefaultStyle(Arc<[StyleProperty<'static, T>]>),
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
    /// Commit the composing state and finish composing
    CommitCompose,
    /// Configure the composing region
    SetCompose(Arc<str>, Option<(usize, usize)>),
}

impl<T> PlainEditor<T>
where
    T: Brush + Clone + Debug + PartialEq + Default,
{
    /// Run a series of `PlainEditorOp`s, updating the layout if necessary
    pub fn transact(
        &mut self,
        font_cx: &mut FontContext,
        layout_cx: &mut LayoutContext<T>,
        t: impl IntoIterator<Item = PlainEditorOp<T>>,
    ) {
        let mut layout_after = false;

        for op in t.into_iter() {
            use PlainEditorOp::*;

            // only allow some operations during composing
            if self.compose.is_some()
                && !matches!(
                    op,
                    CommitCompose
                        | SetCompose(..)
                        | SetWidth(..)
                        | SetScale(..)
                        | SetDefaultStyle(..)
                        | MoveToPoint(..),
                )
            {
                continue;
            }

            match op {
                CommitCompose => {
                    if let Some(ComposeState { text, .. }) = self.compose.clone() {
                        let new_insert = self.selection.insertion_index() + text.len();
                        self.selection = if new_insert == self.buffer.len() {
                            Selection::from_index(
                                &self.layout,
                                new_insert.saturating_sub(1),
                                Affinity::Upstream,
                            )
                        } else {
                            Selection::from_index(&self.layout, new_insert, Affinity::Downstream)
                        };
                    }
                    self.compose = None;
                    layout_after = true;
                }
                SetCompose(s, c) => {
                    let new_compose = ComposeState {
                        text: s.clone(),
                        selection: c,
                    };
                    if let Some(ComposeState { text: oldtext, .. }) = self.compose.clone() {
                        let start = self.selection.insertion_index();
                        let end = start + oldtext.len();
                        self.buffer.replace_range(start..end, &s);
                    } else {
                        self.replace_selection(font_cx, layout_cx, "");
                        let start = self.selection.insertion_index();
                        self.buffer.insert_str(start, &s);
                    }
                    self.compose = Some(new_compose);
                    self.update_layout(font_cx, layout_cx);
                }
                SetText(is) => {
                    self.buffer.clear();
                    self.buffer.push_str(&is);
                    layout_after = true;
                }
                SetWidth(width) => {
                    self.width = width;
                    layout_after = true;
                }
                SetScale(scale) => {
                    self.scale = scale;
                    layout_after = true;
                }
                SetDefaultStyle(style) => {
                    self.default_style = style.clone();
                    layout_after = true;
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
                DeleteWord => {
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
                InsertOrReplaceSelection(s) => {
                    self.replace_selection(font_cx, layout_cx, &s);
                }
                MoveToPoint(x, y) => {
                    // FIXME: breaks when insertion point is inside or after composing region
                    let new_cur = Selection::from_point(&self.layout, x, y);
                    if let Some(ComposeState { text, .. }) = self.compose.clone() {
                        let start = self.selection.insertion_index();
                        self.buffer.replace_range(start..(start + text.len()), "");
                        self.buffer.insert_str(new_cur.insertion_index(), &text);
                    }
                    self.selection = new_cur;
                    layout_after = true;
                }
                MoveToTextStart => {
                    self.selection = self.selection.move_lines(&self.layout, isize::MIN, false);
                }
                MoveToLineStart => {
                    self.selection = self.selection.line_start(&self.layout, false);
                }
                MoveToTextEnd => {
                    self.selection = self.selection.move_lines(&self.layout, isize::MAX, false);
                }
                MoveToLineEnd => {
                    self.selection = self.selection.line_end(&self.layout, false);
                }
                MoveUp => {
                    self.selection = self.selection.previous_line(&self.layout, false);
                }
                MoveDown => {
                    self.selection = self.selection.next_line(&self.layout, false);
                }
                MoveLeft => {
                    self.selection =
                        self.selection
                            .previous_visual(&self.layout, self.cursor_mode, false);
                }
                MoveRight => {
                    self.selection =
                        self.selection
                            .next_visual(&self.layout, self.cursor_mode, false);
                }
                MoveWordLeft => {
                    self.selection = self.selection.previous_word(&self.layout, false);
                }
                MoveWordRight => {
                    self.selection = self.selection.next_word(&self.layout, false);
                }
                SelectAll => {
                    self.selection =
                        Selection::from_index(&self.layout, 0usize, Affinity::default())
                            .move_lines(&self.layout, isize::MAX, true);
                }
                CollapseSelection => {
                    self.selection = self.selection.collapse();
                }
                SelectToTextStart => {
                    self.selection = self.selection.move_lines(&self.layout, isize::MIN, true);
                }
                SelectToLineStart => {
                    self.selection = self.selection.line_start(&self.layout, true);
                }
                SelectToTextEnd => {
                    self.selection = self.selection.move_lines(&self.layout, isize::MAX, true);
                }
                SelectToLineEnd => {
                    self.selection = self.selection.line_end(&self.layout, true);
                }
                SelectUp => {
                    self.selection = self.selection.previous_line(&self.layout, true);
                }
                SelectDown => {
                    self.selection = self.selection.next_line(&self.layout, true);
                }
                SelectLeft => {
                    self.selection =
                        self.selection
                            .previous_visual(&self.layout, self.cursor_mode, true);
                }
                SelectRight => {
                    self.selection =
                        self.selection
                            .next_visual(&self.layout, self.cursor_mode, true);
                }
                SelectWordLeft => {
                    self.selection = self.selection.previous_word(&self.layout, true);
                }
                SelectWordRight => {
                    self.selection = self.selection.next_word(&self.layout, true);
                }
                SelectWordAtPoint(x, y) => {
                    self.selection = Selection::word_from_point(&self.layout, x, y);
                }
                SelectLineAtPoint(x, y) => {
                    let focus = *Selection::from_point(&self.layout, x, y)
                        .line_start(&self.layout, true)
                        .focus();
                    self.selection = Selection::from(focus).line_end(&self.layout, true);
                }
                ExtendSelectionToPoint(x, y) => {
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

    fn sel_at_index(&self, index: usize) -> Selection {
        if index == self.buffer.len() {
            Selection::from_index(&self.layout, index.saturating_sub(1), Affinity::Upstream)
        } else {
            Selection::from_index(&self.layout, index, Affinity::Downstream)
        }
    }

    fn sel_between_indices(&self, from: usize, to: usize) -> Selection {
        self.sel_at_index(from)
            .maybe_extend(*self.sel_at_index(to).focus(), true)
    }

    fn selection_or_preedit_selection(&self) -> Selection {
        if let Some(ComposeState { selection, .. }) = self.compose {
            if let Some((s, e)) = selection {
                self.sel_at_index(self.selection.insertion_index() + s)
                    .maybe_extend(
                        *self
                            .sel_at_index(self.selection.insertion_index() + e)
                            .focus(),
                        true,
                    )
            } else {
                Selection::default()
            }
        } else {
            self.selection
        }
    }

    pub fn preedit_area(&self) -> Option<Rect> {
        // FIXME: Busted
        self.compose
            .clone()
            .and_then(|ComposeState { selection, text }| {
                if selection.map(|(s, e)| s == e).unwrap_or(false) {
                    println!("yes a caret");
                    // otherwise the whole preedit area is the whole composing text region
                    let geom = self
                        .sel_between_indices(
                            self.selection.insertion_index(),
                            self.selection.insertion_index() + text.len(),
                        )
                        .geometry(&self.layout);
                    if geom.is_empty() {
                        None
                    } else {
                        let mut r = Rect::new(0f64, 0f64, f64::INFINITY, f64::INFINITY);
                        for rect in geom {
                            r.x0 = r.x0.min(rect.x0);
                            r.y0 = r.y0.min(rect.y0);
                            r.x1 = r.x1.min(rect.x1);
                            r.y1 = r.y1.min(rect.y1);
                        }
                        Some(r)
                    }
                } else {
                    println!("not a caret");
                    // compose selection is not a caret, so preedit area is the selection
                    self.selection_or_preedit_selection()
                        .focus()
                        .strong_geometry(&self.layout, 1.0)
                }
            })
    }

    pub fn preedit_underline_geometry(&self, size: f32) -> Vec<Rect> {
        self.compose
            .clone()
            .map(|ComposeState { text, .. }| {
                self.sel_at_index(self.selection.insertion_index())
                    .maybe_extend(
                        *self
                            .sel_at_index(self.selection.insertion_index() + text.len())
                            .focus(),
                        true,
                    )
                    .geometry(&self.layout)
                    .iter()
                    .map(|r| Rect::new(r.x0, r.y1 - size as f64, r.x1, r.y1))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get rectangles representing the selected portions of text
    pub fn selection_geometry(&self) -> Vec<Rect> {
        self.selection_or_preedit_selection().geometry(&self.layout)
    }

    /// Get a rectangle representing the current caret cursor position
    pub fn selection_strong_geometry(&self, size: f32) -> Option<Rect> {
        let sops = self.selection_or_preedit_selection();
        if self.compose.is_some() && !sops.is_collapsed() {
            return None;
        }
        sops.focus().strong_geometry(&self.layout, size)
    }

    pub fn selection_weak_geometry(&self, size: f32) -> Option<Rect> {
        let sops = self.selection_or_preedit_selection();
        if self.compose.is_some() && !sops.is_collapsed() {
            return None;
        }
        sops.focus().weak_geometry(&self.layout, size)
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
            builder.push_default(prop.to_owned());
        }
        builder.build_into(&mut self.layout, &self.buffer);
        self.layout.break_all_lines(self.width);
        self.layout.align(self.width, Alignment::Start);
        self.selection = self.selection.refresh(&self.layout);
    }
}
