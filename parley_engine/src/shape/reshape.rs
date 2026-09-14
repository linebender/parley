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
    shape::ShapedCluster,
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

    /// Shape a character range of one run with edits.
    pub fn reshape(
        &mut self,
        text: &str,
        shaped: &ShapedText,
        line: Range<u32>,
        reshaped: &mut ReshapedText,
    ) {
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
    #[inline(always)]
    pub fn replaced_chars(self) -> u32 {
        self.replaced_chars
    }

    #[inline(always)]
    pub fn inserted(&self) -> &'a str {
        self.inserted
    }
}
