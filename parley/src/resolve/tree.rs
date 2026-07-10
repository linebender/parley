// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Hierarchical tree based style application.
use alloc::{string::String, vec::Vec};

use crate::style::WhiteSpaceCollapse;

use super::{Brush, ResolvedProperty, ResolvedStyle, StyleRun};

#[derive(Debug, Clone)]
struct StyleTreeNode<B: Brush> {
    parent: Option<usize>,
    style: ResolvedStyle<B>,
    style_id: Option<u16>,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ItemKind {
    None,
    InlineBox,
    TextRun,
}

/// Builder for constructing a tree of styles
#[derive(Clone)]
pub(crate) struct TreeStyleBuilder<B: Brush> {
    tree: Vec<StyleTreeNode<B>>,
    style_table: Vec<ResolvedStyle<B>>,
    style_runs: Vec<StyleRun>,
    white_space_collapse: WhiteSpaceCollapse,
    text: String,
    uncommitted_text: String,
    current_span: usize,
    is_span_first: bool,
    last_item_kind: ItemKind,
}

impl<B: Brush> TreeStyleBuilder<B> {
    fn current_style(&self) -> ResolvedStyle<B> {
        self.tree[self.current_span].style.clone()
    }
}

impl<B: Brush> Default for TreeStyleBuilder<B> {
    fn default() -> Self {
        Self {
            tree: Vec::new(),
            style_table: Vec::new(),
            style_runs: Vec::new(),
            white_space_collapse: WhiteSpaceCollapse::Preserve,
            text: String::new(),
            uncommitted_text: String::new(),
            current_span: usize::MAX,
            is_span_first: false,
            last_item_kind: ItemKind::None,
        }
    }
}

impl<B: Brush> TreeStyleBuilder<B> {
    /// Prepares the builder for accepting a tree of styles and text.
    ///
    /// The provided `root_style` is the default style applied to all text unless overridden.
    pub(crate) fn begin(&mut self, root_style: ResolvedStyle<B>) {
        self.tree.clear();
        self.style_table.clear();
        self.style_runs.clear();
        self.white_space_collapse = WhiteSpaceCollapse::Preserve;
        self.text.clear();
        self.uncommitted_text.clear();

        self.tree.push(StyleTreeNode {
            parent: None,
            style: root_style,
            style_id: None,
        });
        self.current_span = 0;
        self.is_span_first = true;
    }

    pub(crate) fn set_white_space_mode(&mut self, white_space_collapse: WhiteSpaceCollapse) {
        self.white_space_collapse = white_space_collapse;
    }

    pub(crate) fn set_is_span_first(&mut self, is_span_first: bool) {
        self.is_span_first = is_span_first;
    }

    pub(crate) fn set_last_item_kind(&mut self, item_kind: ItemKind) {
        self.last_item_kind = item_kind;
    }

    pub(crate) fn push_uncommitted_text(&mut self, is_span_last: bool) {
        if let WhiteSpaceCollapse::Collapse = self.white_space_collapse {
            let trim_start = self.is_span_first
                || (self.last_item_kind == ItemKind::TextRun
                    && self
                        .text
                        .chars()
                        .last()
                        .is_some_and(|c| c.is_ascii_whitespace()));

            collapse_whitespace_in_place(&mut self.uncommitted_text, trim_start, is_span_last);
        }

        // Nothing to do if there is no uncommitted text.
        if self.uncommitted_text.is_empty() {
            return;
        }

        let range = self.text.len()..(self.text.len() + self.uncommitted_text.len());
        let style_index = self.resolve_current_style_id();
        self.style_runs.push(StyleRun { style_index, range });
        self.text.push_str(&self.uncommitted_text);
        self.uncommitted_text.clear();
        self.is_span_first = false;
        self.last_item_kind = ItemKind::TextRun;
    }

    fn resolve_current_style_id(&mut self) -> u16 {
        if let Some(style_id) = self.tree[self.current_span].style_id {
            return style_id;
        }
        let style_id = self.style_table.len() as u16;
        self.style_table.push(self.current_style());
        self.tree[self.current_span].style_id = Some(style_id);
        style_id
    }

    pub(crate) fn current_text_len(&self) -> usize {
        self.text.len()
    }

    pub(crate) fn push_style_span(&mut self, style: ResolvedStyle<B>) {
        self.push_uncommitted_text(false);

        self.tree.push(StyleTreeNode {
            parent: Some(self.current_span),
            style,
            style_id: None,
        });
        self.current_span = self.tree.len() - 1;
        self.is_span_first = true;
    }

    pub(crate) fn push_style_modification_span(
        &mut self,
        properties: impl Iterator<Item = ResolvedProperty<B>>,
    ) {
        let mut style = self.current_style();
        for prop in properties {
            style.apply(prop.clone());
        }
        self.push_style_span(style);
    }

    pub(crate) fn pop_style_span(&mut self) {
        self.push_uncommitted_text(true);

        self.current_span = self.tree[self.current_span]
            .parent
            .expect("Popped root style");
    }

    /// Pushes a property that covers the specified range of text.
    pub(crate) fn push_text(&mut self, text: &str) {
        if !text.is_empty() {
            self.uncommitted_text.push_str(text);
        }
    }

    /// Computes style table + style runs and returns the final text buffer.
    pub(crate) fn finish(
        &mut self,
        style_table: &mut Vec<ResolvedStyle<B>>,
        style_runs: &mut Vec<StyleRun>,
    ) -> String {
        while self.tree[self.current_span].parent.is_some() {
            self.pop_style_span();
        }

        self.push_uncommitted_text(true);

        style_table.clear();
        style_runs.clear();
        style_table.extend_from_slice(&self.style_table);
        style_runs.extend_from_slice(&self.style_runs);

        core::mem::take(&mut self.text)
    }
}

/// Collapse whitespace in-place, reusing the `uncommitted_text` buffer.
fn collapse_whitespace_in_place(text: &mut String, trim_start: bool, trim_end: bool) {
    // Convert the input string into a Vec<u8> so that we can run raw bytes manipuation on it
    let mut bytes = core::mem::take(text).into_bytes();

    // Keep track of how many bytes have been written, and whether the last seen character was whitespace.
    let mut bytes_written = 0;
    let mut prev_whitespace = trim_start;

    for read in 0..bytes.len() {
        let byte = bytes[read];
        let is_whitespace = byte.is_ascii_whitespace();

        // If byte IS NOT ascii whitespace, then copy it to the new place in the String
        // (accounting for the offset generated by already collapsed sequences of whitespace)
        if !is_whitespace {
            bytes[bytes_written] = byte;
            bytes_written += 1;
        }
        // If the bytes IS ascii whitespace and the previous byte was NOT.
        // Then output a space character in the next position of the output string.
        else if !prev_whitespace {
            bytes[bytes_written] = b' ';
            bytes_written += 1;
        }
        // If the bytes IS ascii whitespace and the previous byte was also ascii whitespace
        // Then ignore this byte (collapse it)
        else {
            // Do nothing
        }

        prev_whitespace = is_whitespace;
    }

    // If `trim_end` is true, and the last chracter is a space, then trim that character from the output
    // Any run of whitespace has been collapsed to a single space.
    if trim_end && bytes_written > 0 && bytes[bytes_written - 1] == b' ' {
        bytes_written -= 1;
    }

    // Truncate the Vec to the size of the collapsed text
    bytes.truncate(bytes_written);

    // Convert Vec<u8> buffer back to a String
    //
    // SAFETY
    //
    // The original input is a String (and can therefore be assumed to be valid UTF-8), and
    // collapsing ASCII whitespace (above, in this function) preserves UTF-8.
    //
    // All ASCII whitespace bytes are single-byte and never occur within a
    // multi-byte UTF-8 sequence, so dropping them or replacing them with a
    // space leaves the buffer valid UTF-8.
    #[allow(unsafe_code)]
    {
        *text = unsafe { String::from_utf8_unchecked(bytes) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    use core::ops::Range;

    #[test]
    fn reuses_style_id_when_returning_to_parent_span() {
        let mut builder = TreeStyleBuilder::<u32>::default();
        builder.begin(ResolvedStyle::default());
        builder.push_text("A");
        builder.push_style_modification_span([ResolvedProperty::FontSize(20.)].into_iter());
        builder.push_text("B");
        builder.pop_style_span();
        builder.push_text("C");

        let mut style_table = Vec::new();
        let mut style_runs = Vec::new();
        let text = builder.finish(&mut style_table, &mut style_runs);

        assert_eq!(text, "ABC");
        assert_eq!(style_table.len(), 2);
        assert_eq!(style_runs.len(), 3);
        assert_eq!(style_runs[0].style_index, 0);
        assert_eq!(style_runs[1].style_index, 1);
        assert_eq!(style_runs[2].style_index, 0);
        assert_eq!(style_runs[0].range, Range { start: 0, end: 1 });
        assert_eq!(style_runs[1].range, Range { start: 1, end: 2 });
        assert_eq!(style_runs[2].range, Range { start: 2, end: 3 });
    }

    #[test]
    fn reuses_root_style_id_across_multiple_pop_return_cycles() {
        let mut builder = TreeStyleBuilder::<u32>::default();
        builder.begin(ResolvedStyle::default());
        builder.push_text("A");
        builder.push_style_modification_span([ResolvedProperty::FontSize(20.)].into_iter());
        builder.push_text("B");
        builder.pop_style_span();
        builder.push_text("C");
        builder.push_style_modification_span([ResolvedProperty::LetterSpacing(1.)].into_iter());
        builder.push_text("D");
        builder.pop_style_span();
        builder.push_text("E");

        let mut style_table = Vec::new();
        let mut style_runs = Vec::new();
        let text = builder.finish(&mut style_table, &mut style_runs);

        assert_eq!(text, "ABCDE");
        assert_eq!(style_table.len(), 3);
        assert_eq!(style_runs.len(), 5);
        assert_eq!(style_runs[0].style_index, 0);
        assert_eq!(style_runs[1].style_index, 1);
        assert_eq!(style_runs[2].style_index, 0);
        assert_eq!(style_runs[3].style_index, 2);
        assert_eq!(style_runs[4].style_index, 0);
    }

    #[test]
    fn reuses_parent_and_root_style_ids_after_nested_pop() {
        let mut builder = TreeStyleBuilder::<u32>::default();
        builder.begin(ResolvedStyle::default());
        builder.push_text("R");
        builder.push_style_modification_span([ResolvedProperty::FontSize(20.)].into_iter());
        builder.push_text("A");
        builder.push_style_modification_span([ResolvedProperty::LetterSpacing(1.)].into_iter());
        builder.push_text("B");
        builder.pop_style_span();
        builder.push_text("C");
        builder.pop_style_span();
        builder.push_text("D");

        let mut style_table = Vec::new();
        let mut style_runs = Vec::new();
        let text = builder.finish(&mut style_table, &mut style_runs);

        assert_eq!(text, "RABCD");
        assert_eq!(style_table.len(), 3);
        assert_eq!(style_runs.len(), 5);
        assert_eq!(style_runs[0].style_index, 0);
        assert_eq!(style_runs[1].style_index, 1);
        assert_eq!(style_runs[2].style_index, 2);
        assert_eq!(style_runs[3].style_index, 1);
        assert_eq!(style_runs[4].style_index, 0);
    }
}
