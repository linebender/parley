// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Reshaping of lines.
//!
//! A shaped piece of text broken into two parts sometimes requires the shaped glyphs to change. For
//! example, some fonts may shape the Latin character sequence "ffi" into a ligature, and breaking
//! somewhere in that ligature requires reshaping. Furthermore, you may want to insert hyphens or
//! perform replacements near the edge: e.g., the Dutch word "skiën" once broken could be written as
//! "ski-en".
//!
//! This module provides utilities to that end.

use core::ops::Range;

use alloc::vec::Vec;

use crate::{
    FontInstance, Glyph, NormalizedCoord, ShapedRun, ShapedSlice, ShapedText, Shaper,
    itemize::TextRange,
    shape::{ShapedCluster, shaped_text::push_glyph_buffer},
};

use super::Character;

// #[derive(Clone, Debug, PartialEq, Eq)]
// pub struct ReshapedLine {
//     /// Reshaped runs at the start of the line, as a range into [`ReshapedArena::runs`].
//     pub start: Range<u32>,
//     /// Reshaped runs in the middle of the line, as a range into [`ShapedText::shaped_clusters`].
//     pub middle: Range<u32>,
//     /// Reshaped runs at the start of the line, as a range into [`ReshapedArena::runs`].
//     pub end: Range<u32>,
// }

/// A mark that [`ReshapedText`] can be trimmed to.
#[derive(Clone, Copy, Debug)]
pub struct ReshapedTextMark {
    runs_len: u32,
    shaped_clusters_len: u32,
    glyphs_len: u32,
}

#[derive(Clone, Debug)]
pub struct ReshapedRun {
    run: ShapedRun,
    source_run: u32,
}

/// Storage for reshapes runs.
///
/// While reshaping lines, reshapes fragments are appended to this storage. If you're implementing a
/// line breaker that tries multiple candidates, you can use [`Self::mark`] and [`Self::truncate`]
/// to trim some abandoned candidates.
pub struct ReshapedText {
    runs: Vec<ReshapedRun>,
    shaped_clusters: Vec<ShapedCluster>,
    glyphs: Vec<Glyph>,
}

impl ReshapedText {
    pub fn new() -> Self {
        Self {
            runs: Vec::new(),
            shaped_clusters: Vec::new(),
            glyphs: Vec::new(),
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.runs.clear();
        self.shaped_clusters.clear();
        self.glyphs.clear();
    }

    #[inline(always)]
    pub fn mark(&self) -> ReshapedTextMark {
        ReshapedTextMark {
            runs_len: self.runs.len() as u32,
            shaped_clusters_len: self.shaped_clusters.len() as u32,
            glyphs_len: self.glyphs.len() as u32,
        }
    }

    #[inline]
    pub fn truncate(&mut self, mark: ReshapedTextMark) {
        self.runs.truncate(mark.runs_len as usize);
        self.shaped_clusters
            .truncate(mark.shaped_clusters_len as usize);
        self.glyphs.truncate(mark.glyphs_len as usize);
    }

    #[inline(always)]
    pub fn run(&self, reshaped_run_idx: u32) -> &ReshapedRun {
        &self.runs[reshaped_run_idx as usize]
    }

    #[inline(always)]
    pub fn run_slice<'a>(
        &'a self,
        source: &'a ShapedText,
        reshaped_run_idx: u32,
    ) -> ShapedSlice<'a> {
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ReshapedLineStart<'a> {
    line_start_char: u32,
    start_edit: Option<Edit<'a>>,
    reshaped_start_run: Option<u32>,
    /// The start of the range of shaped clusters this reshaped line takes from the paragraph-wide
    /// shaped text.
    source_clusters_start: u32,
    /// The change in advance between the reshaped line's start and the original glyphs.
    ///
    /// Source clusters' advances are counted on the line that contains its first character.
    advance_delta: f32,
}

/// When the start and end reshape windows overlapped, the whole line was reshaped as a single
/// piece. In that case, `reshaped_start_run` contains the reshaped run.
#[derive(Clone, Copy, Debug)]
pub struct ReshapedLine {
    /// The index into [`ReshapedText::runs`] that starts this line, if the start needed reshaping.
    reshaped_start_run: Option<u32>,
    /// The range of shaped clusters this reshaped line takes from the paragraph-wide shaped text.
    source_clusters: (u32, u32),
    /// The index into [`ReshapedText::runs`] that ends this line, if the end needed reshaping.
    reshaped_end_run: Option<u32>,
    /// The change in advance between the reshaped line and the original glyphs.
    ///
    /// Source clusters' advances are counted on the line that contains its first character.
    advance_delta: f32,
}

pub enum SpliceMode {
    Approximate,
    Correct,
}

impl Shaper {
    pub fn reshape_line_start<'a>(
        &mut self,
        text: &str,
        shaped: &ShapedText,
        line_start_char: u32,
        edit: Option<Edit<'a>>,
        splice: SpliceMode,
        out: &mut ReshapedText,
    ) -> ReshapedLineStart<'a> {
    }

    /// `line_end_char` is the exclusive end of the line.
    pub fn reshape_line_end(
        &mut self,
        text: &str,
        shaped: &ShapedText,
        line_start: &ReshapedLineStart<'_>,
        line_end_char: u32,
        edit: Option<Edit<'_>>,
        splice: SpliceMode,
        out: &mut ReshapedText,
    ) -> ReshapedLine {
    }

    /// Shape the characters `chars` of the run `run_index` again.
    ///
    /// Returns the new run's index in [`ReshapedText::runs`].
    ///
    /// `start_edit` and `end_edit` edit the first and last characters of `chars`.
    pub(crate) fn reshape(
        &mut self,
        text: &str,
        shaped: &ShapedText,
        run_index: u32,
        chars: Range<u32>,
        start_edit: Option<Edit<'_>>,
        end_edit: Option<Edit<'_>>,
        reshaped: &mut ReshapedText,
    ) -> u32 {
        let run = &shaped.runs()[run_index as usize];
        let char_count = chars.len() as u32;

        let start_num_chars_replaced = start_edit.map_or(0, Edit::replaced_chars);
        let end_num_chars_replaced = end_edit.map_or(0, Edit::replaced_chars);
        assert!(
            start_num_chars_replaced + end_num_chars_replaced <= char_count,
            "the edits must not overlap"
        );

        let characters = shaped.characters();
        let bytes = characters[chars.start as usize].text_byte_start as usize
            ..characters[chars.end as usize - 1].text_byte_range().end;

        let mut buffer = self.unicode_buffer.take().unwrap();
        buffer.clear();

        // Fill the characters into the `HarfRust` buffer with the start and end edits applied.
        if let Some(edit) = start_edit {
            edit.inserted().chars().for_each(|ch| buffer.add(ch, 0));
        }
        for (ch, cluster_value) in text[bytes.clone()]
            .chars()
            .zip(0..)
            .take((char_count - end_num_chars_replaced) as usize)
            .skip(start_num_chars_replaced as usize)
        {
            buffer.add(ch, cluster_value);
        }
        if let Some(edit) = end_edit {
            let cluster_value = char_count - end_num_chars_replaced.max(1);
            edit.inserted
                .chars()
                .for_each(|ch| buffer.add(ch, cluster_value));
        }
        buffer.set_pre_context(&text[..bytes.start]);
        buffer.set_post_context(&text[bytes.end..]);

        let (glyph_buffer, font_unit_scale) = self.shape_like_run(shaped, run, buffer);
        let (shaped_clusters_range, glyphs_range) = push_glyph_buffer(
            &mut reshaped.shaped_clusters,
            &mut reshaped.glyphs,
            &glyph_buffer,
            font_unit_scale,
            run.bidi_level.is_rtl(),
            characters,
            chars.start as usize..chars.end as usize,
            shaped.has_safe_to_concat_flags,
        );
        self.unicode_buffer = Some(glyph_buffer.clear());

        reshaped.runs.push(ReshapedRun {
            run: ShapedRun {
                range: TextRange {
                    byte_range: bytes,
                    char_range: chars.start as usize..chars.end as usize,
                },
                characters_range: chars,
                shaped_clusters_range,
                glyphs_range,
                ..run.clone()
            },
            source_run: run_index,
        });

        reshaped.runs.len() as u32 - 1
    }
}

/// The edit made to one end of a line.
///
/// This could be, e.g., the insertion of a hyphen, but could also insert or replace characters.
///
/// - In Dutch, "skiën" written with hyphenation can become "ski-en". Note on the second line the
///   "ë" character was replaced by "e".
/// - In Norwegian, "bussjåfør" can become "buss-sjåfør". Note on the second line an "s" character
///   was inserted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edit<'a> {
    /// The number of source characters dropped and replaced by [`Self::inserted`].
    ///
    /// This number can be zero if no source text is dropped or substituted, so in particular this
    /// is also zero if characters were only inserted.
    replaced_chars: u32,

    /// The inserted text.
    ///
    /// Inserted text takes the character index of the logically-first replaced character. If there
    /// are no replaced characters, it attaches to the character at the edge. For a line spanning
    /// character indices `[p, q)` this is `p` for the start, and `q - max(replaced_chars, 1)` for
    /// the end.
    inserted: &'a str,
}

impl<'a> Edit<'a> {
    /// Replace the `replaced_chars` source characters at the line's edge by `inserted`.
    ///
    /// For an edit at the line's logical start, the replaced characters are the line's first
    /// characters. For an edit at a line's logical end, these are the line's last characters.
    /// `replace_chars` may be zero, in which case `inserted` is only inserted. With an empty
    /// `inserted`, the characters are only dropped.
    ///
    /// # Examples
    ///
    /// - "skiën" broken after "ski" ends the first line with `Edit::new(0, "-")`, and starts the
    ///   second line with `Edit::new(1, "e")`.
    /// - "bussjåfør" broken after "buss" ends the first line with `Edit::new(0, "-")`, and starts
    ///   the second line with `Edit::new(0, "s")`.
    #[inline(always)]
    pub const fn new(replaced_chars: u32, inserted: &'a str) -> Self {
        Self {
            replaced_chars,
            inserted,
        }
    }

    #[inline(always)]
    pub fn replaced_chars(self) -> u32 {
        self.replaced_chars
    }

    #[inline(always)]
    pub fn inserted(&self) -> &'a str {
        self.inserted
    }
}
