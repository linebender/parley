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
        // The white space collapsing is performed in place within `self.uncommitted_text` (all
        // modes only ever shrink the text), whose allocation is also retained across commits.
        match self.white_space_collapse {
            // Text is kept verbatim. `BreakSpaces` behaves identically to `Preserve` at this
            // stage; its extra soft-wrap opportunities and non-hanging white space are handled
            // during line breaking.
            WhiteSpaceCollapse::Preserve | WhiteSpaceCollapse::BreakSpaces => {}
            WhiteSpaceCollapse::Collapse | WhiteSpaceCollapse::PreserveBreaks => {
                let preserve_breaks = matches!(
                    self.white_space_collapse,
                    WhiteSpaceCollapse::PreserveBreaks
                );
                let trim_start = self.is_span_first
                    || (self.last_item_kind == ItemKind::TextRun
                        && self
                            .text
                            .chars()
                            .last()
                            .is_some_and(|c| c.is_ascii_whitespace()));

                collapse_white_space(
                    &mut self.uncommitted_text,
                    preserve_breaks,
                    trim_start,
                    is_span_last,
                );
            }
            // Preserve all white space, but convert tabs and segment breaks to spaces.
            WhiteSpaceCollapse::PreserveSpaces => {
                convert_white_space_to_spaces(&mut self.uncommitted_text);
            }
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
        let white_space_collapse = self.white_space_collapse;
        // The cached style id can be reused as long as the active white-space-collapse mode
        // matches the one it was resolved under (the mode is not part of the style tree, so it
        // can change independently of the current span's style).
        if let Some(style_id) = self.tree[self.current_span].style_id
            && self.style_table[style_id as usize].white_space_collapse == white_space_collapse
        {
            return style_id;
        }
        let style_id = self.style_table.len() as u16;
        let mut style = self.current_style();
        style.white_space_collapse = white_space_collapse;
        self.style_table.push(style);
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

/// Collapses sequences of white space into a single space, in place, reusing `text`'s allocation.
///
/// If `trim_start`/`trim_end` are `true`, white space at the respective edge of the text is
/// removed entirely instead of being collapsed.
///
/// When `preserve_breaks` is `true`, segment breaks (newlines) within a white space sequence are
/// preserved (with any surrounding spaces and tabs removed), matching the CSS
/// `white-space-collapse: preserve-breaks` behavior. Otherwise every white space sequence collapses
/// to a single space (`white-space-collapse: collapse`).
///
/// A CRLF (`"\r\n"`) is treated as a single segment break.
fn collapse_white_space(
    text: &mut String,
    preserve_breaks: bool,
    trim_start: bool,
    trim_end: bool,
) {
    // Compute the byte range that remains after trimming (`str::trim_start`/`str::trim_end` trim
    // Unicode white space).
    let mut trimmed = text.as_str();
    if trim_start {
        trimmed = trimmed.trim_start();
    }
    if trim_end {
        trimmed = trimmed.trim_end();
    }
    let start = if trim_start {
        text.len() - text.trim_start().len()
    } else {
        0
    };
    let end = start + trimmed.len();

    // Collapse the white space by compacting the buffer in place, with separate read (`read`) and
    // write (`write`) positions. This operates on the underlying bytes: the white space collapsed
    // here is ASCII, and ASCII bytes never occur within a multi-byte UTF-8 sequence, so scanning
    // the bytes for ASCII white space is sound, and all other bytes are copied verbatim. The
    // write position can never overtake the read position, as no replacement is longer than the
    // white space sequence it replaces.
    let mut bytes = core::mem::take(text).into_bytes();
    let mut write = 0;
    let mut read = start;
    while read < end {
        let byte = bytes[read];
        if !byte.is_ascii_whitespace() {
            bytes[write] = byte;
            write += 1;
            read += 1;
            continue;
        }

        // We are at the start of a white space sequence; consume the whole run, counting the
        // number of segment breaks it contains (treating CRLF as a single break).
        let mut segment_breaks = 0_usize;
        while read < end && bytes[read].is_ascii_whitespace() {
            match bytes[read] {
                b'\r' => {
                    segment_breaks += 1;
                    read += 1;
                    if read < end && bytes[read] == b'\n' {
                        read += 1;
                    }
                }
                b'\n' => {
                    segment_breaks += 1;
                    read += 1;
                }
                _ => read += 1,
            }
        }

        if preserve_breaks && segment_breaks > 0 {
            for _ in 0..segment_breaks {
                bytes[write] = b'\n';
                write += 1;
            }
        } else {
            bytes[write] = b' ';
            write += 1;
        }
    }
    bytes.truncate(write);
    *text = String::from_utf8(bytes).expect("collapsing preserves UTF-8 validity");
}

/// Preserves white space but converts tabs and segment breaks (newlines) to spaces, in place,
/// reusing `text`'s allocation, matching the CSS `white-space-collapse: preserve-spaces` behavior.
///
/// A CRLF (`"\r\n"`) is treated as a single segment break and thus becomes a single space.
fn convert_white_space_to_spaces(text: &mut String) {
    // U+2028 LINE SEPARATOR and U+2029 PARAGRAPH SEPARATOR (which are segment breaks) as UTF-8.
    const LINE_SEPARATOR: [u8; 3] = [0xE2, 0x80, 0xA8];
    const PARAGRAPH_SEPARATOR: [u8; 3] = [0xE2, 0x80, 0xA9];

    // Like `collapse_white_space`, this compacts the underlying bytes in place. All replacements
    // are ASCII bytes replacing either ASCII bytes or whole multi-byte sequences (the separators
    // above), so UTF-8 validity is preserved and the write position can never overtake the read
    // position.
    let mut bytes = core::mem::take(text).into_bytes();
    let len = bytes.len();
    let mut write = 0;
    let mut read = 0;
    while read < len {
        match bytes[read] {
            b'\t' | b'\n' => {
                bytes[write] = b' ';
                write += 1;
                read += 1;
            }
            b'\r' => {
                read += 1;
                if read < len && bytes[read] == b'\n' {
                    read += 1;
                }
                bytes[write] = b' ';
                write += 1;
            }
            0xE2 if bytes[read..].starts_with(&LINE_SEPARATOR)
                || bytes[read..].starts_with(&PARAGRAPH_SEPARATOR) =>
            {
                bytes[write] = b' ';
                write += 1;
                read += LINE_SEPARATOR.len();
            }
            byte => {
                bytes[write] = byte;
                write += 1;
                read += 1;
            }
        }
    }
    bytes.truncate(write);
    *text = String::from_utf8(bytes).expect("conversion preserves UTF-8 validity");
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

    /// Helper that runs a single span of text through the tree builder with the given
    /// white-space-collapse mode and returns the resulting text buffer.
    fn collapsed_text(mode: WhiteSpaceCollapse, text: &str) -> String {
        let mut builder = TreeStyleBuilder::<u32>::default();
        builder.begin(ResolvedStyle::default());
        builder.set_white_space_mode(mode);
        builder.push_text(text);
        let mut style_table = Vec::new();
        let mut style_runs = Vec::new();
        builder.finish(&mut style_table, &mut style_runs)
    }

    #[test]
    fn white_space_collapse_modes() {
        use WhiteSpaceCollapse::*;

        let input = "a  b \t c\n\n d\r\ne  ";

        assert_eq!(collapsed_text(Preserve, input), "a  b \t c\n\n d\r\ne  ");
        assert_eq!(collapsed_text(BreakSpaces, input), "a  b \t c\n\n d\r\ne  ");
        assert_eq!(collapsed_text(Collapse, input), "a b c d e");
        assert_eq!(collapsed_text(PreserveBreaks, input), "a b c\n\nd\ne");
        assert_eq!(collapsed_text(PreserveSpaces, input), "a  b   c   d e  ");
    }

    #[test]
    fn preserve_breaks_preserves_each_blank_line() {
        use WhiteSpaceCollapse::*;
        // Spaces around and between segment breaks are removed, but every break is kept
        // (CRLF counts as a single break).
        assert_eq!(collapsed_text(PreserveBreaks, "a \n \n b"), "a\n\nb");
        assert_eq!(collapsed_text(PreserveBreaks, "a \r\n b"), "a\nb");
        assert_eq!(collapsed_text(PreserveBreaks, "a\tb"), "a b");
    }

    #[test]
    fn preserve_spaces_converts_tabs_and_breaks() {
        use WhiteSpaceCollapse::*;
        // Tabs and segment breaks become single spaces; runs of spaces are preserved.
        assert_eq!(collapsed_text(PreserveSpaces, "a\tb"), "a b");
        assert_eq!(collapsed_text(PreserveSpaces, "a\r\nb"), "a b");
        assert_eq!(collapsed_text(PreserveSpaces, "a  b"), "a  b");
        assert_eq!(
            collapsed_text(PreserveSpaces, "a\u{2028}b\u{2029}c"),
            "a b c"
        );
    }

    #[test]
    fn white_space_collapse_handles_multibyte_content() {
        use WhiteSpaceCollapse::*;
        // The collapsing is performed on the underlying (UTF-8) bytes; multi-byte characters
        // adjacent to white space must be copied through intact.
        assert_eq!(collapsed_text(Collapse, "é  ü\t☃"), "é ü ☃");
        assert_eq!(collapsed_text(Collapse, "  éü  "), "éü");
        assert_eq!(collapsed_text(PreserveBreaks, "é \r\n ü"), "é\nü");
        assert_eq!(collapsed_text(PreserveSpaces, "é\u{2028}ü"), "é ü");
    }

    #[test]
    fn white_space_collapse_reuses_the_allocation() {
        let mut text = String::from("a  b \t c\n\n d\r\ne  ");
        let pointer = text.as_ptr();
        let capacity = text.capacity();

        collapse_white_space(&mut text, false, true, true);
        assert_eq!(text, "a b c d e");
        assert_eq!(text.as_ptr(), pointer);
        assert_eq!(text.capacity(), capacity);

        let mut text = String::from("a\tb\r\nc\u{2028}d");
        let pointer = text.as_ptr();
        let capacity = text.capacity();

        convert_white_space_to_spaces(&mut text);
        assert_eq!(text, "a b c d");
        assert_eq!(text.as_ptr(), pointer);
        assert_eq!(text.capacity(), capacity);
    }

    #[test]
    fn white_space_mode_recorded_in_style() {
        use WhiteSpaceCollapse::*;
        let mut builder = TreeStyleBuilder::<u32>::default();
        builder.begin(ResolvedStyle::default());
        builder.set_white_space_mode(BreakSpaces);
        builder.push_text("a");
        // A span boundary commits "a" under the active mode; the following span uses a different
        // mode, which must be recorded in a distinct style entry.
        builder.push_style_modification_span(core::iter::empty());
        builder.set_white_space_mode(Collapse);
        builder.push_text("b");
        builder.pop_style_span();

        let mut style_table = Vec::new();
        let mut style_runs = Vec::new();
        let text = builder.finish(&mut style_table, &mut style_runs);

        assert_eq!(text, "ab");
        assert_eq!(style_runs.len(), 2);
        assert_eq!(
            style_table[style_runs[0].style_index as usize].white_space_collapse,
            BreakSpaces
        );
        assert_eq!(
            style_table[style_runs[1].style_index as usize].white_space_collapse,
            Collapse
        );
    }
}
