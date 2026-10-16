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
#[derive(Clone, Debug, PartialEq)]
pub struct ReshapedRun {
    run: ShapedRun,
    source_run: u32,
}

impl ReshapedRun {
    /// The reshaped run.
    ///
    /// Its [`ShapedRun::shaped_clusters_range`] and [`ShapedRun::glyphs_range`] index into
    /// [`ReshapedText`]; use [`ReshapedText::run_slice`] to access them. Its other indices  index
    /// the source [`ShapedText`].
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
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReshapedText {
    runs: Vec<ReshapedRun>,
    shaped_clusters: Vec<ShapedCluster>,
    glyphs: Vec<Glyph>,
}

impl ReshapedText {
    /// Create a reusable [`ReshapedText`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear the result while retaining capacity.
    #[inline]
    pub fn clear(&mut self) {
        self.runs.clear();
        self.shaped_clusters.clear();
        self.glyphs.clear();
    }

    /// Mark the current state, which you can truncate back to using [`Self::truncate`].
    #[inline(always)]
    #[expect(clippy::cast_possible_truncation, reason = "Deferred")]
    pub fn mark(&self) -> ReshapedTextMark {
        ReshapedTextMark {
            runs_len: self.runs.len() as u32,
            shaped_clusters_len: self.shaped_clusters.len() as u32,
            glyphs_len: self.glyphs.len() as u32,
        }
    }

    /// Truncate back to mark created by [`Self::mark`].
    ///
    /// This drops everything reshaped since the mark was taken. Other marks, [`ReshapedLineStart`]s
    /// and [`ReshapedLine`]s created after it must not be used anymore.
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

impl ReshapedLineStart<'_> {
    /// How much reshaping the line's start changed the line's advance.
    ///
    /// Add this to the line's source advance, counted from the line's start, to measure where the
    /// line ends. Note that, once fully reshaped, the line may again measure differently. See
    /// [`ReshapedLine::advance_delta`].
    pub fn advance_delta(&self) -> f32 {
        self.advance_delta
    }
}

/// A reshaped line.
///
/// In logical order, the line consists of [`Self::start_run`], shaped clusters taken from the
/// source [`ShapedText`] in [`Self::source_shaped_clusters_range`], and [`Self::end_run`].
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

impl ReshapedLine {
    /// The reshaped run the line starts with, as an index into [`ReshapedText::run`].
    ///
    /// `None` if the line's start was not reshaped.
    #[inline(always)]
    pub fn start_run(&self) -> Option<u32> {
        self.reshaped_start_run
    }

    /// The shaped clusters this line keeps from the source [`ShapedText`], as a range into
    /// [`ShapedText::shaped_clusters`].
    #[inline(always)]
    pub fn source_shaped_clusters_range(&self) -> Range<u32> {
        self.source_clusters.0..self.source_clusters.1
    }

    /// The reshaped run ending this line, as an index into [`ReshapedText::run`].
    ///
    /// `None` if the line ends with source clusters or with [`Self::start_run`].
    #[inline(always)]
    pub fn end_run(&self) -> Option<u32> {
        self.reshaped_end_run
    }

    /// How much reshaping changed the line's advance.
    ///
    /// The reshaped line's advance is the sum of this delta and the advance of the line's source
    /// shaped clusters (including those shaped clusters replaced by this reshaped line).
    #[inline(always)]
    pub fn advance_delta(&self) -> f32 {
        self.advance_delta
    }
}

/// How to determine the boundary at which to splice a reshaped line edge to the line's source
/// glyphs.
///
/// Reshaping a line edge reshapes a fragment of the original line. The glyphs beyond the reshaped
/// fragment are the glyphs from the original [`ShapedText`] (or the reshaped glyphs from the other
/// end of the line). This splice mode trades speed for exactness in where that boundary may be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpliceMode {
    /// Splice at the nearest cluster that is marked [`ShapedCluster::is_safe_to_break_before`].
    ///
    /// The reshaped fragments tend to be short, and the result almost always matches
    /// [`SpliceMode::Exact`]. However, this flag only describes the source text; a change within
    /// the reshaped clusters could technically miss a reshape past the seam. For example, Fira Code
    /// gives the hyphen in "down-town" a lowercase form because of the "t" after it. A line ending
    /// in "down-" should keep that form because of the "n" before it, but `Fast` reshapes only the
    /// hyphen, which then takes its default form.
    Fast,

    /// Splice at the nearest cluster that is marked [`ShapedCluster::is_safe_to_concat_before`] in
    /// both the source and the reshaped fragment.
    ///
    /// This extends the reshaped fragment until such a boundary is found. Note this repeatedly
    /// reshapes that fragment.
    ///
    /// The result will always be identical to reshaping the whole line, but for some fonts, like
    /// those that kern many characters, fragments may be much longer than necessary and this
    /// process may be relatively slow.
    ///
    /// Requires text shaped with `produce_safe_to_concat_flags`.
    Exact,
}

impl SpliceMode {
    fn is_safe_to_splice_before(self, cluster: ShapedCluster) -> bool {
        if !cluster.is_grapheme_start() {
            // For now, for simplicity, we want to ensure reshaped pieces are aligned to graphemes,
            // so that they're also an `Atom` boundary.
            false
        } else {
            match self {
                Self::Fast => cluster.is_safe_to_break_before(),
                Self::Exact => cluster.is_safe_to_concat_before(),
            }
        }
    }
}

impl Shaper {
    /// Start reshaping a line at `line_start_char` with the given `edit`.
    ///
    /// `text` must be the text that `shaped` was produced from, and `line_start_char` must be a
    /// grapheme boundary.
    ///
    /// Pass the result to [`Self::reshape_line_end`], with the same `text`, `shaped` and
    /// `reshaped`, to finish reshaping the line.
    ///
    /// # Panics
    ///
    /// Panics if `splice` is [`SpliceMode::Exact`] but `shaped` lacks
    /// [`ShapedText::has_safe_to_concat_flags`]; or if `edit` replaces characters past the end of
    /// the [`ShapedRun`] at the line's edge.
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
            splice == SpliceMode::Fast || shaped.has_safe_to_concat_flags(),
            "`SpliceMode::Exact` requires text shaped with `produce_safe_to_concat_flags`"
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

        let run_start_cluster = run.shaped_clusters_range.start as usize;
        let run_end_cluster = run.shaped_clusters_range.end as usize;
        let run_clusters = &shaped.shaped_clusters()[run_start_cluster..run_end_cluster];
        let first_char_idx_of_cluster = |i: usize| {
            if i == run_end_cluster {
                run.characters_range.end
            } else {
                shaped.shaped_clusters()[i].chars_range().start
            }
        };
        let cluster_is_seam = |i: usize, splice: SpliceMode| {
            // Note that separate runs are shaped separately, so a run's edge is a seam.
            i == run_start_cluster
                || i == run_end_cluster
                || splice.is_safe_to_splice_before(shaped.shaped_clusters()[i])
        };

        let mut seam_cluster = run_start_cluster
            + match edit {
                None => run_clusters
                    .partition_point(|cluster| cluster.chars_range().start < line_start_char),
                // The edit may kern or join with the neighboring unchanged character, so at the
                // very least that neighbor is included in the reshaped fragment.
                Some(edit) => run_clusters.partition_point(|cluster| {
                    cluster.chars_range().start <= line_start_char + edit.replaced_chars()
                }),
            };

        // With no `edit`, no reshaping is necessary when at a cluster that is safe to break before.
        if edit.is_none()
            && first_char_idx_of_cluster(seam_cluster) == line_start_char
            && cluster_is_seam(seam_cluster, SpliceMode::Fast)
        {
            return reshaped_line_start;
        }

        while !cluster_is_seam(seam_cluster, splice) {
            seam_cluster += 1;
        }

        let reshaped_run_idx = loop {
            if splice == SpliceMode::Fast || seam_cluster == run_end_cluster {
                break self.reshape(
                    text,
                    shaped,
                    run_index as u32,
                    line_start_char..first_char_idx_of_cluster(seam_cluster),
                    edit,
                    None,
                    reshaped,
                );
            }

            let mut next_seam_cluster = seam_cluster + 1;
            while !cluster_is_seam(next_seam_cluster, splice) {
                next_seam_cluster += 1;
            }
            let mark = reshaped.mark();
            let reshaped_run_idx = self.reshape(
                text,
                shaped,
                run_index as u32,
                line_start_char..first_char_idx_of_cluster(next_seam_cluster),
                edit,
                None,
                reshaped,
            );
            // The fragment has no concat flag at its end, but if the fragment is safe to concat at
            // a seam before its end, we know the fragment and the source glyphs will match from
            // that point on (we could in principle trim the fragment back to that point).
            let fragment_clusters = reshaped
                .run_slice(shaped, reshaped_run_idx)
                .shaped_clusters();
            let is_safe = next_seam_cluster == run_end_cluster
                || fragment_clusters
                    .binary_search_by_key(&first_char_idx_of_cluster(seam_cluster), |c| {
                        c.chars_range().start
                    })
                    .is_ok_and(|i| fragment_clusters[i].is_safe_to_concat_before());
            seam_cluster = next_seam_cluster;
            if is_safe {
                break reshaped_run_idx;
            }
            reshaped.truncate(mark);
        };

        let line_start_cluster = reshaped_line_start.source_clusters_start as usize;
        let source_advance: f32 = shaped.shaped_clusters()[line_start_cluster..seam_cluster]
            .iter()
            .map(|c| c.advance)
            .sum();
        reshaped_line_start.reshaped_start_run = Some(reshaped_run_idx);
        reshaped_line_start.source_clusters_start = seam_cluster as u32;
        reshaped_line_start.advance_delta =
            reshaped.runs[reshaped_run_idx as usize].run.advance - source_advance;
        reshaped_line_start
    }

    /// Finish reshaping the line started by `line_start`, ending it at `line_end_char` with the
    /// given `edit`.
    ///
    /// `line_end_char` must be a grapheme boundary after the line's start.
    ///
    /// # Panics
    ///
    /// Panics if `splice` is [`SpliceMode::Exact`] but `shaped` lacks
    /// [`ShapedText::has_safe_to_concat_flags`], if `edit` replaces characters before the start of
    /// the [`ShapedRun`] at the line's edge, or if the start and end edits together replace more
    /// characters than the line has.
    #[expect(clippy::cast_possible_truncation, reason = "Deferred")]
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
            splice == SpliceMode::Fast || shaped.has_safe_to_concat_flags(),
            "`SpliceMode::Exact` requires text shaped with `produce_safe_to_concat_flags`"
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

        let run_start_cluster = run.shaped_clusters_range.start as usize;
        let run_end_cluster = run.shaped_clusters_range.end as usize;
        let run_clusters = &shaped.shaped_clusters()[run_start_cluster..run_end_cluster];
        let first_char_idx_of_cluster = |i: usize| {
            if i == run_end_cluster {
                run.characters_range.end
            } else {
                shaped.shaped_clusters()[i].chars_range().start
            }
        };
        let cluster_is_seam = |i: usize, splice: SpliceMode| {
            // Note that separate runs are shaped separately, so a run's edge is a seam.
            i == run_start_cluster
                || i == run_end_cluster
                || splice.is_safe_to_splice_before(shaped.shaped_clusters()[i])
        };

        // The reshaped end can't start before the reshaped start ends, which is where the source
        // clusters start.
        let min_seam_cluster = line_start.source_clusters_start as usize;

        let mut seam_cluster = run_start_cluster
            + match edit {
                None if line_end_char == run.characters_range.end => run_clusters.len(),
                None => {
                    run_clusters
                        .partition_point(|cluster| cluster.chars_range().start <= line_end_char)
                        - 1
                }
                // The edit may kern or join with the neighboring unchanged character, so at the
                // very least that neighbor is included in the reshaped fragment.
                Some(edit) => run_clusters
                    .partition_point(|cluster| {
                        cluster.chars_range().start < line_end_char - edit.replaced_chars()
                    })
                    .saturating_sub(1),
            };

        // With no `edit`, the line end needs no reshaping when at a cluster that is safe to break
        // before, unless the reshaped start reaches past it.
        if edit.is_none()
            && first_char_idx_of_cluster(seam_cluster) == line_end_char
            && seam_cluster >= min_seam_cluster
            && cluster_is_seam(seam_cluster, SpliceMode::Fast)
        {
            return reshaped_line;
        }

        while !cluster_is_seam(seam_cluster, splice) {
            seam_cluster -= 1;
        }

        let reshaped_run_idx = loop {
            if seam_cluster < min_seam_cluster {
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
                let line_start_cluster = shaped
                    .shaped_clusters()
                    .partition_point(|cluster| cluster.chars_range().start < line_start_char);
                let source_advance: f32 = shaped.shaped_clusters()
                    [line_start_cluster..source_clusters_end as usize]
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
            }

            let mark = reshaped.mark();
            let reshaped_run_idx = self.reshape(
                text,
                shaped,
                run_index as u32,
                first_char_idx_of_cluster(seam_cluster)..line_end_char,
                None,
                edit,
                reshaped,
            );

            let first_cluster = reshaped.runs[reshaped_run_idx as usize]
                .run
                .shaped_clusters_range
                .start;
            if splice == SpliceMode::Fast
                || seam_cluster == run_start_cluster
                || first_char_idx_of_cluster(seam_cluster) == line_start_char
                || reshaped.shaped_clusters[first_cluster as usize].is_safe_to_concat_before()
            {
                break reshaped_run_idx;
            }
            reshaped.truncate(mark);
            seam_cluster -= 1;
            while !cluster_is_seam(seam_cluster, splice) {
                seam_cluster -= 1;
            }
        };

        let source_advance: f32 = shaped.shaped_clusters()
            [seam_cluster..source_clusters_end as usize]
            .iter()
            .map(|c| c.advance)
            .sum();
        reshaped_line.source_clusters.1 = seam_cluster as u32;
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
    #[expect(clippy::cast_possible_truncation, reason = "Deferred")]
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
    /// The replaced characters must lie in the [`ShapedRun`] at that edge of the line.
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

    /// The number of characters replaced by this [`Edit`].
    #[inline(always)]
    pub fn replaced_chars(self) -> u32 {
        self.replaced_chars
    }

    /// The characters inserted by this [`Edit`].
    #[inline(always)]
    pub fn inserted(&self) -> &'a str {
        self.inserted
    }
}
