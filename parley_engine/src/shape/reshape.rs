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
    Glyph, ShapedRun, ShapedSlice, ShapedText, Shaper,
    itemize::TextRange,
    shape::{ShapedCluster, shaped_text::push_glyph_buffer},
};

/// A mark that [`ReshapedText`] can be trimmed to.
#[derive(Clone, Copy, Debug)]
pub struct ReshapedTextMark {
    runs_len: u32,
    shaped_clusters_len: u32,
    glyphs_len: u32,
}

/// One run of reshaped text, belonging to a [`ReshapedText`].
#[derive(Clone, Debug)]
pub struct ReshapedRun {
    run: ShapedRun,
    source_run: u32,
}

impl ReshapedRun {
    /// The reshaped run.
    pub fn shaped_run(&self) -> &ShapedRun {
        &self.run
    }

    /// The index of [`ShapedRun`] whose glyphs were reshaped.
    ///
    /// This indexes into the original [`ShapedText::runs`] .
    #[inline(always)]
    pub fn source_run(&self) -> u32 {
        self.source_run
    }
}

/// Storage for reshapes runs.
///
/// While reshaping lines, reshapes fragments are appended to this storage. If you're implementing a
/// line breaker that tries multiple candidates, you can use [`Self::mark`] and [`Self::truncate`]
/// to trim some abandoned candidates.
#[derive(Debug)]
pub struct ReshapedText {
    runs: Vec<ReshapedRun>,
    shaped_clusters: Vec<ShapedCluster>,
    glyphs: Vec<Glyph>,
}

impl ReshapedText {
    /// Create a reusable [`ReshapedText`].
    pub fn new() -> Self {
        Self {
            runs: Vec::new(),
            shaped_clusters: Vec::new(),
            glyphs: Vec::new(),
        }
    }

    /// Clear the result while retaining capacity.
    #[inline]
    pub fn clear(&mut self) {
        self.runs.clear();
        self.shaped_clusters.clear();
        self.glyphs.clear();
    }

    /// Create a mark within this reshaped that you can truncate back to later using [`Self::truncate`].
    ///
    /// A mark N must not be used anymore if it was created after another mark M, and you truncated
    /// back to mark M.
    #[inline(always)]
    pub fn mark(&self) -> ReshapedTextMark {
        ReshapedTextMark {
            runs_len: self.runs.len() as u32,
            shaped_clusters_len: self.shaped_clusters.len() as u32,
            glyphs_len: self.glyphs.len() as u32,
        }
    }

    /// Trim back to a mark created by [`Self::mark`].
    #[inline]
    pub fn truncate(&mut self, mark: ReshapedTextMark) {
        self.runs.truncate(mark.runs_len as usize);
        self.shaped_clusters
            .truncate(mark.shaped_clusters_len as usize);
        self.glyphs.truncate(mark.glyphs_len as usize);
    }

    /// Get the reshaped run at the given index.
    #[inline(always)]
    pub fn run(&self, reshaped_run_idx: u32) -> &ReshapedRun {
        &self.runs[reshaped_run_idx as usize]
    }

    /// Get a [`ShapedSlice`] of the reshaped run at the given index.
    #[inline(always)]
    pub fn run_slice<'a>(
        &'a self,
        source: &'a ShapedText,
        reshaped_run_idx: u32,
    ) -> ShapedSlice<'a> {
        let run = &self.runs[reshaped_run_idx as usize].run;
        ShapedSlice {
            characters: source.characters(),
            shaped_clusters: &self.shaped_clusters,
            glyphs: &self.glyphs,
            clusters: (
                run.shaped_clusters_range.start,
                run.shaped_clusters_range.end,
            ),
        }
    }

    /// Cut the last reshaped run short at the character `end`, if the shaped cluster is safe to concat
    /// there.
    ///
    /// Returns whether it was cut.
    #[expect(clippy::cast_possible_truncation, reason = "Deferred")]
    fn cut_at_concat_safe(&mut self, source: &ShapedText, end: u32) -> bool {
        let run = &mut self.runs.last_mut().unwrap().run;
        let clusters = &self.shaped_clusters[run.shaped_clusters_range.start as usize..];
        let kept = clusters.partition_point(|c| c.chars_range().start < end);
        if !clusters
            .get(kept)
            .is_some_and(|c| c.chars_range().start == end && c.is_safe_to_concat_before())
        {
            return false;
        }

        let glyphs_end = clusters[kept..]
            .iter()
            .find(|c| !c.has_inline_glyph())
            .map_or(run.glyphs_range.end, |c| c.glyph_offset as usize);
        run.advance = clusters[..kept].iter().map(|c| c.advance).sum();
        run.range.byte_range.end = source.characters()[end as usize].text_byte_start as usize;
        run.range.char_range.end = end as usize;
        run.characters_range.end = end;
        run.shaped_clusters_range.end -= (clusters.len() - kept) as u32;
        run.glyphs_range.end = glyphs_end;
        self.shaped_clusters
            .truncate(run.shaped_clusters_range.end as usize);
        self.glyphs.truncate(glyphs_end);
        true
    }
}

/// A marker for a line in the process of being reshaped. Produced by
/// [`Shaper::reshape_line_start`], consumed by [`Shaper::reshape_line_end`].
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpliceMode {
    Approximate,
    Correct,
}

impl SpliceMode {
    fn is_safe_to_splice_before(self, cluster: ShapedCluster) -> bool {
        if !cluster.is_grapheme_start() {
            // For now, for simplicity, we want to ensure reshaped pieces are aligned to graphemes,
            // so that they're also an `Atom` boundary.
            false
        } else {
            match self {
                Self::Approximate => cluster.is_safe_to_break_before(),
                Self::Correct => cluster.is_safe_to_concat_before(),
            }
        }
    }
}

impl Shaper {
    /// Start reshaping a line at `line_start_char` with the given `edit`.
    ///
    /// `text` must be the original text that `shaped` was produced from.
    ///
    /// Pass the reshaped line start to [`Self::reshape_line_end`] to finish reshaping the line.
    #[expect(clippy::cast_possible_truncation, reason = "Deferred")]
    pub fn reshape_line_start<'a>(
        &mut self,
        text: &str,
        shaped: &ShapedText,
        line_start_char: u32,
        edit: Option<Edit<'a>>,
        splice: SpliceMode,
        reshaped: &mut ReshapedText,
    ) -> ReshapedLineStart<'a> {
        assert!(
            splice == SpliceMode::Approximate || shaped.has_safe_to_concat_flags(),
            "`SpliceMode::Correct` requires text shaped with `produce_safe_to_concat_flags`"
        );

        let mut reshaped_line_start = ReshapedLineStart {
            line_start_char,
            start_edit: edit,
            reshaped_start_run: None,
            source_clusters_start: shaped
                .shaped_clusters()
                .partition_point(|cluster| cluster.chars_range().start < line_start_char)
                as u32,
            advance_delta: 0.,
        };

        if line_start_char as usize == shaped.characters().len() {
            // Be graceful about a caller trying to shape the empty line at the end of text.
            return reshaped_line_start;
        }

        let run_index = shaped
            .runs()
            .partition_point(|run| run.characters_range.end <= line_start_char);
        let run = &shaped.runs()[run_index];

        // let base = run.shaped_clusters_range.start as usize;
        let run_clusters = &shaped.shaped_clusters()
            [run.shaped_clusters_range.start as usize..run.shaped_clusters_range.end as usize];
        let first_char_idx_of_cluster = |i: usize| {
            run_clusters
                .get(i)
                .map_or(run.characters_range.end, |cluster| {
                    cluster.chars_range().start
                })
        };
        let cluster_is_seam = |i: usize| {
            i == 0 || i == run_clusters.len() || splice.is_safe_to_splice_before(run_clusters[i])
        };

        let mut seam = match edit {
            None => run_clusters
                .partition_point(|cluster| cluster.chars_range().start < line_start_char),
            Some(edit) => run_clusters.partition_point(|cluster| {
                cluster.chars_range().start <= line_start_char + edit.replaced_chars()
            }),
        };

        while !cluster_is_seam(seam) {
            seam += 1;
        }
        if first_char_idx_of_cluster(seam) == line_start_char {
            return reshaped_line_start;
        }

        let reshaped_run_idx = if splice == SpliceMode::Approximate {
            self.reshape(
                text,
                shaped,
                run_index as u32,
                line_start_char..first_char_idx_of_cluster(seam),
                edit,
                None,
                reshaped,
            )
        } else {
            loop {
                if seam == run_clusters.len() {
                    break self.reshape(
                        text,
                        shaped,
                        run_index as u32,
                        line_start_char..first_char_idx_of_cluster(seam),
                        edit,
                        None,
                        reshaped,
                    );
                }

                let mut next_seam = seam + 1;
                while !cluster_is_seam(next_seam) {
                    next_seam += 1;
                }
                let mark = reshaped.mark();
                let reshaped_run_idx = self.reshape(
                    text,
                    shaped,
                    run_index as u32,
                    line_start_char..first_char_idx_of_cluster(next_seam),
                    edit,
                    None,
                    reshaped,
                );
                if reshaped.cut_at_concat_safe(shaped, first_char_idx_of_cluster(seam)) {
                    break reshaped_run_idx;
                }
                reshaped.truncate(mark);
                seam = next_seam;
            }
        };

        let source_start = reshaped_line_start.source_clusters_start as usize;
        let source_advance: f32 = shaped.shaped_clusters()
            [source_start..run.shaped_clusters_range.start as usize + seam]
            .iter()
            .map(|c| c.advance)
            .sum();
        reshaped_line_start.reshaped_start_run = Some(reshaped_run_idx);
        reshaped_line_start.source_clusters_start =
            (run.shaped_clusters_range.start as usize + seam) as u32;
        reshaped_line_start.advance_delta =
            reshaped.runs[reshaped_run_idx as usize].run.advance - source_advance;
        reshaped_line_start
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
        reshaped: &mut ReshapedText,
    ) -> ReshapedLine {
        assert!(
            splice == SpliceMode::Approximate || shaped.has_safe_to_concat_flags(),
            "`SpliceMode::Correct` requires text shaped with `produce_safe_to_concat_flags`"
        );

        let line_start_char = line_start.line_start_char;
        let source_clusters_end = shaped
            .shaped_clusters()
            .partition_point(|cluster| cluster.chars_range().start < line_end_char)
            as u32;

        if line_end_char == line_start_char {
            return ReshapedLine {
                reshaped_start_run: None,
                source_clusters: (source_clusters_end, source_clusters_end),
                reshaped_end_run: None,
                advance_delta: 0.,
            };
        }

        let mut reshaped_line = ReshapedLine {
            reshaped_start_run: line_start.reshaped_start_run,
            source_clusters: (line_start.source_clusters_start, source_clusters_end),
            reshaped_end_run: None,
            advance_delta: line_start.advance_delta,
        };

        let run_index = shaped
            .runs()
            .partition_point(|run| run.characters_range.end < line_end_char);
        let run = &shaped.runs()[run_index];

        let run_clusters = &shaped.shaped_clusters()
            [run.shaped_clusters_range.start as usize..run.shaped_clusters_range.end as usize];
        let first_char_idx_of_cluster = |i: usize| {
            run_clusters
                .get(i)
                .map_or(run.characters_range.end, |cluster| {
                    cluster.chars_range().start
                })
        };
        let cluster_is_seam = |i: usize| {
            i == 0 || i == run_clusters.len() || splice.is_safe_to_splice_before(run_clusters[i])
        };

        // The reshaped end can't start before the reshaped start ends (which is where the source
        // characters start).
        let source_start_char =
            line_start
                .reshaped_start_run
                .map_or(line_start_char, |reshaped_run_idx| {
                    reshaped.runs[reshaped_run_idx as usize]
                        .run
                        .characters_range
                        .end
                });

        let mut seam = match edit {
            None if line_end_char == run.characters_range.end => run_clusters.len(),
            None => {
                run_clusters.partition_point(|cluster| cluster.chars_range().start <= line_end_char)
                    - 1
            }
            // Include the last unchanged character, as the edit may kern or join with it.
            Some(edit) => run_clusters
                .partition_point(|cluster| {
                    cluster.chars_range().start < line_end_char - edit.replaced_chars()
                })
                .saturating_sub(1),
        };

        while !cluster_is_seam(seam) {
            seam -= 1;
        }

        // The reshaped start may reach past the line end.
        if first_char_idx_of_cluster(seam) == line_end_char && line_end_char >= source_start_char {
            return reshaped_line;
        }

        let reshaped_run_idx = if splice == SpliceMode::Approximate {
            if first_char_idx_of_cluster(seam) < source_start_char {
                None
            } else {
                Some(self.reshape(
                    text,
                    shaped,
                    run_index as u32,
                    first_char_idx_of_cluster(seam)..line_end_char,
                    None,
                    edit,
                    reshaped,
                ))
            }
        } else {
            loop {
                if first_char_idx_of_cluster(seam) < source_start_char {
                    break None;
                }

                let mark = reshaped.mark();
                let reshaped_run_idx = self.reshape(
                    text,
                    shaped,
                    run_index as u32,
                    first_char_idx_of_cluster(seam)..line_end_char,
                    None,
                    edit,
                    reshaped,
                );

                let first_cluster = reshaped.runs[reshaped_run_idx as usize]
                    .run
                    .shaped_clusters_range
                    .start;
                if seam == 0
                    || first_char_idx_of_cluster(seam) == line_start_char
                    || reshaped.shaped_clusters[first_cluster as usize].is_safe_to_concat_before()
                {
                    break Some(reshaped_run_idx);
                }
                reshaped.truncate(mark);
                seam -= 1;
                while !cluster_is_seam(seam) {
                    seam -= 1;
                }
            }
        };

        let Some(reshaped_run_idx) = reshaped_run_idx else {
            // The reshaped start and end would overlap, so reshape the whole line as one piece.
            let reshaped_run_idx = self.reshape(
                text,
                shaped,
                run_index as u32,
                line_start_char..line_end_char,
                line_start.start_edit,
                edit,
                reshaped,
            );
            let source_start = shaped
                .shaped_clusters()
                .partition_point(|cluster| cluster.chars_range().start < line_start_char);
            let source_advance: f32 = shaped.shaped_clusters()
                [source_start..source_clusters_end as usize]
                .iter()
                .map(|c| c.advance)
                .sum();
            return ReshapedLine {
                reshaped_start_run: Some(reshaped_run_idx),
                source_clusters: (source_clusters_end, source_clusters_end),
                reshaped_end_run: None,
                advance_delta: reshaped.runs[reshaped_run_idx as usize].run.advance
                    - source_advance,
            };
        };

        let source_end = run.shaped_clusters_range.start as usize + seam;
        let source_advance: f32 = shaped.shaped_clusters()
            [source_end..source_clusters_end as usize]
            .iter()
            .map(|c| c.advance)
            .sum();
        reshaped_line.source_clusters.1 = source_end as u32;
        reshaped_line.reshaped_end_run = Some(reshaped_run_idx);
        reshaped_line.advance_delta +=
            reshaped.runs[reshaped_run_idx as usize].run.advance - source_advance;
        reshaped_line
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
        let (shaped_clusters_range, glyphs_range, advance) = push_glyph_buffer(
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
                advance,
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
