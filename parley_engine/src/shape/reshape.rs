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

use core::{mem, ops::Range};

use alloc::{string::String, vec::Vec};

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
    /// [`ReshapedText`]; use [`ReshapedText::run_slice`] to access them. Its other indices index
    /// the source [`ShapedText`].
    pub fn shaped_run(&self) -> &ShapedRun {
        &self.run
    }

    /// The index of [`ShapedRun`] whose glyphs were reshaped.
    ///
    /// This indexes into the original [`ShapedText::runs`].
    #[inline(always)]
    pub fn source_run(&self) -> u32 {
        self.source_run
    }
}

/// Storage for reshaped runs.
///
/// If you're implementing a line breaker that revises lines already pushed to this storage, you can
/// use [`Self::mark`] and [`Self::truncate`] to trim.
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
    /// This drops everything reshaped since the mark was taken. Other marks created after it must
    /// not be used anymore.
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

    /// Append copies of `other`'s runs, returning the index of the first.
    #[expect(clippy::cast_possible_truncation, reason = "Deferred")]
    fn extend_from(&mut self, other: &Self) -> u32 {
        let first_run = self.runs.len() as u32;
        let clusters_start = self.shaped_clusters.len() as u32;
        let glyphs_start = self.glyphs.len();

        self.runs.extend(other.runs.iter().map(|reshaped_run| {
            let mut reshaped_run = reshaped_run.clone();
            let run = &mut reshaped_run.run;
            run.shaped_clusters_range = run.shaped_clusters_range.start + clusters_start
                ..run.shaped_clusters_range.end + clusters_start;
            run.glyphs_range =
                run.glyphs_range.start + glyphs_start..run.glyphs_range.end + glyphs_start;
            reshaped_run
        }));
        self.shaped_clusters
            .extend(other.shaped_clusters.iter().map(|&cluster| {
                let mut cluster = cluster;
                if !cluster.has_inline_glyph() {
                    cluster.glyph_offset += glyphs_start as u32;
                }
                cluster
            }));
        self.glyphs.extend_from_slice(&other.glyphs);
        first_run
    }
}

/// A reshaped line start measured by [`Reshaper::measure_start`].
#[derive(Clone, Copy, Debug)]
pub struct LineStart<'a> {
    line_start_char: u32,
    start_edit: Option<Edit<'a>>,
    /// The reshaped fragment starting the line, if the start needed reshaping.
    start_fragment: Option<Fragment>,
    /// The start of the range of shaped clusters this reshaped line takes from the paragraph-wide
    /// shaped text.
    source_clusters_start: u32,
    /// The change in advance between the reshaped line's start and the original glyphs.
    ///
    /// Source clusters' advances are counted on the line that contains its first character.
    advance_delta: f32,
}

impl LineStart<'_> {
    /// How much reshaping the line's start changed the line's advance.
    ///
    /// Add this to the line's source advance, counted from the line's start, to measure where the
    /// line ends. Note that, once fully reshaped, the line may again measure differently. See
    /// [`PendingLine::advance_delta`].
    pub fn advance_delta(&self) -> f32 {
        self.advance_delta
    }
}

/// A reshaped line end measured by [`Reshaper::measure_end`].
#[derive(Clone, Copy, Debug)]
pub struct LineEnd<'a> {
    line_end_char: u32,
    end_edit: Option<Edit<'a>>,
    /// The reshaped fragment ending the line, if the end needed reshaping.
    end_fragment: Option<Fragment>,
    /// The end of the range of shaped clusters this reshaped line takes from the paragraph-wide
    /// shaped text.
    source_clusters_end: u32,
    /// The change in advance between the reshaped line's end and the original glyphs.
    ///
    /// Source clusters' advances are counted on the line that contains its first character.
    advance_delta: f32,
}

impl LineEnd<'_> {
    /// How much reshaping the line's end changed the line's advance.
    ///
    /// Add this and [`LineStart::advance_delta`] to the line's source advance to measure a line
    /// ending here. Note that, once fully reshaped, the line may again measure differently. See
    /// [`PendingLine::advance_delta`].
    pub fn advance_delta(&self) -> f32 {
        self.advance_delta
    }
}

/// A line from a [`LineStart`] to a [`LineEnd`].
#[derive(Clone, Copy, Debug)]
pub struct PendingLine<'a> {
    start: LineStart<'a>,
    end: LineEnd<'a>,
}

impl PendingLine<'_> {
    /// How much reshaping changed the line's advance.
    ///
    /// The reshaped line's advance is the sum of this delta and the advance of the line's source
    /// shaped clusters (including those shaped clusters replaced by this reshaped line).
    #[inline(always)]
    pub fn advance_delta(&self) -> f32 {
        self.start.advance_delta + self.end.advance_delta
    }
}

/// A reshaped line stored in a [`ReshapedText`].
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
enum SpliceMode {
    /// Splice at the nearest cluster that is marked [`ShapedCluster::is_safe_to_break_before`].
    Fast,

    /// Splice at the nearest cluster that is marked [`ShapedCluster::is_safe_to_concat_before`] in
    /// both the source and the reshaped fragment.
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

/// The source characters a reshaped fragment of a line covers.
#[derive(Clone, Copy, Debug)]
struct Fragment {
    /// The source run the characters are in, as an index into [`ShapedText::runs`].
    run: u32,
    /// The range of characters covered, including those replaced by an [`Edit`].
    chars: (u32, u32),
}

#[derive(Debug, Default)]
pub(crate) struct FragmentCache {
    /// Slots 0 and 1 hold fragments starting lines, slots 2 and 3 fragments ending lines, including
    /// whole lines.
    ///
    /// These are stored as separate collections to have a relatively good chance of a cache hit
    /// under common patterns of line breakers. Note in particular that a line breaker, after
    /// measuring the line's new start, may need to measure multiple candidates for the line's end.
    /// We can't simply us a single [`ReshapedText`] as a a cache, because there'd be no way to
    /// evict older entries.
    slots: [CachedFragment; 4],
    /// For each of those pairs, which of its two slots was used last.
    recent: [usize; 2],
}

impl FragmentCache {
    const STARTS: usize = 0;
    const ENDS: usize = 1;

    /// The slot holding the fragment shaped from `chars` with `edits`, if any.
    fn find(&self, chars: (u32, u32), edits: [Option<Edit<'_>>; 2]) -> Option<usize> {
        self.slots.iter().position(|slot| {
            slot.chars == Some(chars)
                && (slot.edits.iter().zip(edits)).all(|((replaced, inserted), edit)| {
                    *replaced == edit.map(Edit::replaced_chars)
                        && inserted == edit.map_or("", |edit| edit.inserted)
                })
        })
    }
}

#[derive(Debug, Default)]
struct CachedFragment {
    /// The characters `fragment` was shaped from, or `None` for an empty slot.
    chars: Option<(u32, u32)>,
    /// The replaced character count and inserted text of the edits `fragment` was shaped with.
    edits: [(Option<u32>, String); 2],
    /// The fragment, as the only run.
    fragment: ReshapedText,
}

/// Reshape lines of a [`ShapedText`].
///
/// Create this using [`Shaper::reshaper`].
///
/// Measure the effect of breaking at a position, including [`Edit`]s, [`Self::measure_start`] and
/// [`Self::measure_end`]. Create lines using [`Self::line`]. Optionally make the line you keep
/// exact with [`Self::make_exact`].
///
/// Store resulting lines using [`Self::push_line`].
///
/// # Exactness
///
/// A line created by [`Self::line`] splices reshaped fragments to the source glyphs at the nearest
/// cluster that is marked [`ShapedCluster::is_safe_to_break_before`]. These reshaped fragments tend
/// to be short, and the result almost always matches [`Self::make_exact`]. However, this method of
/// splicing is based only on the source text, and a change within the reshaped clusters could
/// technically miss a reshape past the seam. For example, Fira Code gives the hyphen in "down-town"
/// a lowercase form because of the "t" after it. A line ending in "down-" should keep that form
/// because of the "n" before it, but measuring reshapes only the hyphen, which then takes its
/// default form.
#[derive(Debug)]
pub struct Reshaper<'a> {
    shaper: &'a mut Shaper,
    text: &'a str,
    shaped: &'a ShapedText,
    cache: FragmentCache,
}

impl Drop for Reshaper<'_> {
    fn drop(&mut self) {
        self.shaper.fragment_cache = mem::take(&mut self.cache);
    }
}

impl<'a> Reshaper<'a> {
    /// Measure the effect of starting a line at `line_start_char` with the given `edit`.
    ///
    /// `line_start_char` must be a grapheme boundary.
    ///
    /// # Panics
    ///
    /// Panics if `edit` replaces characters past the end of the [`ShapedRun`] at the line's edge.
    #[inline]
    pub fn measure_start<'e>(
        &mut self,
        line_start_char: u32,
        edit: Option<Edit<'e>>,
    ) -> LineStart<'e> {
        self.reshape_line_start(line_start_char, edit, SpliceMode::Fast, None)
            .expect("an unbounded line start can't overlap a line end")
    }

    /// Measure the effect of ending a line at `line_end_char` with the given `edit`.
    ///
    /// `line_end_char` must be a grapheme boundary.
    ///
    /// # Panics
    ///
    /// Panics if `edit` replaces characters before the start of the [`ShapedRun`] at the line's
    /// edge.
    #[inline]
    pub fn measure_end<'e>(&mut self, line_end_char: u32, edit: Option<Edit<'e>>) -> LineEnd<'e> {
        self.reshape_line_end(line_end_char, edit, SpliceMode::Fast, None)
            .expect("an unbounded line end can't overlap a line start")
    }

    /// Create the line from `start` to `end`.
    ///
    /// `end` must not be before `start`. If the line is so short that its reshaped start and end
    /// overlap, the whole line is reshaped as one piece. An empty line drops the edits, as it has
    /// no characters for inserted text to attach to.
    ///
    /// See the note about exactness on [`Self`].
    ///
    /// # Panics
    ///
    /// Panics if the edits of a line together replace more characters than the line has (unless the
    /// line is empty, in which case the edits are dropped).
    #[inline]
    #[expect(clippy::cast_possible_truncation, reason = "Deferred")]
    pub fn line<'e>(&mut self, start: &LineStart<'e>, end: &LineEnd<'e>) -> PendingLine<'e> {
        if end.line_end_char == start.line_start_char {
            let source_clusters = self
                .shaped
                .shaped_clusters()
                .partition_point(|cluster| cluster.chars_range().start < end.line_end_char)
                as u32;
            return PendingLine {
                start: LineStart {
                    start_fragment: None,
                    source_clusters_start: source_clusters,
                    advance_delta: 0.,
                    ..*start
                },
                end: LineEnd {
                    end_fragment: None,
                    source_clusters_end: source_clusters,
                    advance_delta: 0.,
                    ..*end
                },
            };
        }
        if end.source_clusters_end < start.source_clusters_start {
            // The reshaped start and end would overlap, so reshape the whole line as one piece.
            return self.reshape_whole_line(*start, *end);
        }
        PendingLine {
            start: *start,
            end: *end,
        }
    }

    /// Make `line` identical to reshaping the whole line.
    ///
    /// This splices the line's reshaped fragments to the source glyphs at the nearest cluster that
    /// is marked [`ShapedCluster::is_safe_to_concat_before`] in both the source and the reshaped
    /// fragment, extending the fragments until such a boundary is found, or reshaping the whole line
    /// if they would overlap. Note this repeatedly reshapes the fragments.
    ///
    /// The result will always be identical to reshaping the whole line, but for some fonts, like
    /// those that kern many characters, fragments may be much longer than necessary and this
    /// process may be relatively slow. Call this once, for the line you keep, rather than for every
    /// candidate line. The line's [advance](PendingLine::advance_delta) may change.
    ///
    /// # Panics
    ///
    /// Panics if the [`ShapedText`] lacks [`ShapedText::has_safe_to_concat_flags`].
    #[must_use]
    pub fn make_exact<'e>(&mut self, mut line: PendingLine<'e>) -> PendingLine<'e> {
        assert!(
            self.shaped.has_safe_to_concat_flags(),
            "`make_exact` requires text shaped with `produce_safe_to_concat_flags`"
        );

        let PendingLine { start, end } = line;
        if end.end_fragment.is_none() && start.source_clusters_start == end.source_clusters_end {
            // The line is empty, or reshaped as one piece.
            return line;
        }

        if end.end_fragment.is_some() {
            match self.reshape_line_end(
                end.line_end_char,
                end.end_edit,
                SpliceMode::Exact,
                Some(&start),
            ) {
                Some(end) => line.end = end,
                // The reshaped start and end would overlap, so reshape the whole line as one piece.
                None => return self.reshape_whole_line(start, end),
            }
        }
        if start.start_fragment.is_some() {
            match self.reshape_line_start(
                start.line_start_char,
                start.start_edit,
                SpliceMode::Exact,
                Some(&line.end),
            ) {
                Some(start) => line.start = start,
                // The reshaped start and end would overlap, so reshape the whole line as one piece.
                None => return self.reshape_whole_line(start, end),
            }
        }
        line
    }

    /// Store `line` in `reshaped`.
    ///
    /// Note that [`Reshaper`] caches the fragments it reshaped most recently (separately for the
    /// starts and ends), and copies `line`'s fragments from there if they're still in the cache.
    /// Otherwise, the line is reshaped again.
    pub fn push_line(
        &mut self,
        line: &PendingLine<'_>,
        reshaped: &mut ReshapedText,
    ) -> ReshapedLine {
        ReshapedLine {
            reshaped_start_run: (line.start.start_fragment)
                .map(|fragment| self.push_fragment(line, fragment, reshaped)),
            source_clusters: (
                line.start.source_clusters_start,
                line.end.source_clusters_end,
            ),
            reshaped_end_run: (line.end.end_fragment)
                .map(|fragment| self.push_fragment(line, fragment, reshaped)),
            advance_delta: line.advance_delta(),
        }
    }

    /// Reshape the start of a line at `line_start_char` with the given `edit`.
    ///
    /// With a `line_end`, returns `None` if the reshaped start and end would overlap.
    #[expect(clippy::cast_possible_truncation, reason = "Deferred")]
    fn reshape_line_start<'e>(
        &mut self,
        line_start_char: u32,
        edit: Option<Edit<'e>>,
        splice: SpliceMode,
        line_end: Option<&LineEnd<'_>>,
    ) -> Option<LineStart<'e>> {
        let shaped = self.shaped;
        let mut line_start = LineStart {
            line_start_char,
            start_edit: edit,
            start_fragment: None,
            source_clusters_start: shaped
                .shaped_clusters()
                .partition_point(|cluster| cluster.chars_range().start < line_start_char)
                as u32,
            advance_delta: 0.,
        };

        if line_start_char as usize == shaped.characters().len() {
            // Be graceful about a caller trying to shape the empty line at the end of text.
            return Some(line_start);
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

        // The reshaped start can't end after the reshaped end starts, which is where the source
        // clusters end.
        let max_seam_cluster =
            line_end.map_or(usize::MAX, |line_end| line_end.source_clusters_end as usize);

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
            return Some(line_start);
        }

        while !cluster_is_seam(seam_cluster, splice) {
            seam_cluster += 1;
        }

        let advance = loop {
            if splice == SpliceMode::Fast || seam_cluster == run_end_cluster {
                if seam_cluster > max_seam_cluster {
                    return None;
                }
                let fragment = Fragment {
                    run: run_index as u32,
                    chars: (line_start_char, first_char_idx_of_cluster(seam_cluster)),
                };
                line_start.start_fragment = Some(fragment);
                break self.shape(FragmentCache::STARTS, fragment, [edit, None]).1;
            }

            let mut next_seam_cluster = seam_cluster + 1;
            while !cluster_is_seam(next_seam_cluster, splice) {
                next_seam_cluster += 1;
            }
            if next_seam_cluster > max_seam_cluster {
                return None;
            }
            let fragment = Fragment {
                run: run_index as u32,
                chars: (
                    line_start_char,
                    first_char_idx_of_cluster(next_seam_cluster),
                ),
            };
            let (slot, advance) = self.shape(FragmentCache::STARTS, fragment, [edit, None]);
            // The fragment has no concat flag at its end, but if the fragment is safe to concat at
            // a seam before its end, we know the fragment and the source glyphs will match from
            // that point on (we could in principle trim the fragment back to that point).
            let fragment_clusters = self.cache.slots[slot]
                .fragment
                .run_slice(shaped, 0)
                .shaped_clusters();
            let is_safe = next_seam_cluster == run_end_cluster
                // A fragment up to an unreshaped line end is the whole line.
                || line_end.is_some_and(|line_end| {
                    line_end.end_fragment.is_none() && next_seam_cluster == max_seam_cluster
                })
                || fragment_clusters
                    .binary_search_by_key(&first_char_idx_of_cluster(seam_cluster), |c| {
                        c.chars_range().start
                    })
                    .is_ok_and(|i| fragment_clusters[i].is_safe_to_concat_before());
            seam_cluster = next_seam_cluster;
            if is_safe {
                line_start.start_fragment = Some(fragment);
                break advance;
            }
            // Let the next attempt take this slot.
            self.cache.recent[slot / 2] = 1 - slot % 2;
        };

        let line_start_cluster = line_start.source_clusters_start as usize;
        let source_advance: f32 = shaped.shaped_clusters()[line_start_cluster..seam_cluster]
            .iter()
            .map(|c| c.advance)
            .sum();
        line_start.source_clusters_start = seam_cluster as u32;
        line_start.advance_delta = advance - source_advance;
        Some(line_start)
    }

    /// Reshape the end of a line at `line_end_char` with the given `edit`.
    ///
    /// With a `line_start`, returns `None` if the reshaped start and end would overlap.
    #[expect(clippy::cast_possible_truncation, reason = "Deferred")]
    fn reshape_line_end<'e>(
        &mut self,
        line_end_char: u32,
        edit: Option<Edit<'e>>,
        splice: SpliceMode,
        line_start: Option<&LineStart<'_>>,
    ) -> Option<LineEnd<'e>> {
        let shaped = self.shaped;
        let source_clusters_end = shaped
            .shaped_clusters()
            .partition_point(|cluster| cluster.chars_range().start < line_end_char)
            as u32;
        let mut line_end = LineEnd {
            line_end_char,
            end_edit: edit,
            end_fragment: None,
            source_clusters_end,
            advance_delta: 0.,
        };

        if line_end_char == 0 {
            // Be graceful about a caller trying to shape the empty line at the start of text.
            return Some(line_end);
        }

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
        let min_seam_cluster =
            line_start.map_or(0, |line_start| line_start.source_clusters_start as usize);

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
                        cluster.chars_range().start
                            < line_end_char.saturating_sub(edit.replaced_chars())
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
            return Some(line_end);
        }

        while !cluster_is_seam(seam_cluster, splice) {
            seam_cluster -= 1;
        }

        let advance = loop {
            if seam_cluster < min_seam_cluster {
                // The reshaped start and end would overlap.
                return None;
            }

            let fragment = Fragment {
                run: run_index as u32,
                chars: (first_char_idx_of_cluster(seam_cluster), line_end_char),
            };
            let (slot, advance) = self.shape(FragmentCache::ENDS, fragment, [None, edit]);

            let reshaped = &self.cache.slots[slot].fragment;
            let first_cluster = reshaped.runs[0].run.shaped_clusters_range.start;
            if splice == SpliceMode::Fast
                || seam_cluster == run_start_cluster
                || line_start.is_some_and(|line_start| {
                    first_char_idx_of_cluster(seam_cluster) == line_start.line_start_char
                })
                || reshaped.shaped_clusters[first_cluster as usize].is_safe_to_concat_before()
            {
                line_end.end_fragment = Some(fragment);
                break advance;
            }
            // Let the next attempt take this slot.
            self.cache.recent[slot / 2] = 1 - slot % 2;
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
        line_end.source_clusters_end = seam_cluster as u32;
        line_end.advance_delta = advance - source_advance;
        Some(line_end)
    }

    /// Reshape the whole line from `start` to `end` as one piece.
    #[expect(clippy::cast_possible_truncation, reason = "Deferred")]
    fn reshape_whole_line<'e>(
        &mut self,
        start: LineStart<'e>,
        end: LineEnd<'e>,
    ) -> PendingLine<'e> {
        let shaped = self.shaped;
        let line_start_cluster = shaped
            .shaped_clusters()
            .partition_point(|cluster| cluster.chars_range().start < start.line_start_char);
        let source_clusters_end = shaped
            .shaped_clusters()
            .partition_point(|cluster| cluster.chars_range().start < end.line_end_char)
            as u32;
        let fragment = Fragment {
            run: shaped
                .runs()
                .partition_point(|run| run.characters_range.end <= start.line_start_char)
                as u32,
            chars: (start.line_start_char, end.line_end_char),
        };
        let (_, advance) = self.shape(
            FragmentCache::ENDS,
            fragment,
            [start.start_edit, end.end_edit],
        );
        let source_advance: f32 = shaped.shaped_clusters()
            [line_start_cluster..source_clusters_end as usize]
            .iter()
            .map(|c| c.advance)
            .sum();
        PendingLine {
            start: LineStart {
                start_fragment: Some(fragment),
                source_clusters_start: source_clusters_end,
                advance_delta: advance - source_advance,
                ..start
            },
            end: LineEnd {
                end_fragment: None,
                source_clusters_end,
                advance_delta: 0.,
                ..end
            },
        }
    }

    /// Copy `fragment` of `line` to `reshaped` from the cache if present; if not present, it's
    /// reshaped again.
    fn push_fragment(
        &mut self,
        line: &PendingLine<'_>,
        fragment: Fragment,
        reshaped: &mut ReshapedText,
    ) -> u32 {
        // A fragment is shaped with the edits of the line's edges it touches.
        let edits = [
            (line.start.start_edit).filter(|_| fragment.chars.0 == line.start.line_start_char),
            (line.end.end_edit).filter(|_| fragment.chars.1 == line.end.line_end_char),
        ];
        match self.cache.find(fragment.chars, edits) {
            Some(slot) => {
                let cached = &self.cache.slots[slot].fragment;
                debug_assert_eq!(cached.runs.len(), 1, "a cache slot holds one fragment");
                reshaped.extend_from(cached)
            }
            None => self.shaper.reshape(
                self.text,
                self.shaped,
                fragment.run,
                fragment.chars.0..fragment.chars.1,
                edits[0],
                edits[1],
                reshaped,
            ),
        }
    }

    /// Reshape `fragment` with `edits` into one of the cache slots of `pair`, unless the cache
    /// already holds it.
    ///
    /// Returns the slot and the fragment's advance.
    fn shape(
        &mut self,
        pair: usize,
        fragment: Fragment,
        edits: [Option<Edit<'_>>; 2],
    ) -> (usize, f32) {
        let cache = &mut self.cache;
        let slot = match cache.find(fragment.chars, edits) {
            Some(slot) => slot,
            None => {
                // Take the slot of the pair that wasn't used last.
                let slot = 2 * pair + 1 - cache.recent[pair];
                let cached = &mut cache.slots[slot];
                cached.fragment.clear();
                self.shaper.reshape(
                    self.text,
                    self.shaped,
                    fragment.run,
                    fragment.chars.0..fragment.chars.1,
                    edits[0],
                    edits[1],
                    &mut cached.fragment,
                );
                cached.chars = Some(fragment.chars);
                for ((replaced, inserted), edit) in cached.edits.iter_mut().zip(edits) {
                    *replaced = edit.map(Edit::replaced_chars);
                    inserted.clear();
                    inserted.push_str(edit.map_or("", |edit| edit.inserted));
                }
                slot
            }
        };
        cache.recent[slot / 2] = slot % 2;
        (slot, cache.slots[slot].fragment.runs[0].run.advance)
    }
}

impl Shaper {
    /// Start reshaping lines of `shaped`.
    ///
    /// `text` must be the text that `shaped` was produced from. See [`Reshaper`] for how to
    /// continue.
    pub fn reshaper<'a>(&'a mut self, text: &'a str, shaped: &'a ShapedText) -> Reshaper<'a> {
        let mut cache = mem::take(&mut self.fragment_cache);
        // Fragments of another text must not be reused.
        for slot in &mut cache.slots {
            slot.chars = None;
        }
        Reshaper {
            shaper: self,
            text,
            shaped,
            cache,
        }
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
            "the edits must lie in the run and must not overlap"
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
    /// The replaced characters must lie in the [`ShapedRun`] at that edge of the line.
    ///
    /// For an edit at the line's logical start, the replaced characters are the line's first
    /// characters. For an edit at a line's logical end, these are the line's last characters.
    /// `replaced_chars` may be zero, in which case `inserted` is only inserted. With an empty
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

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use super::{Edit, Range, ReshapedLine, ReshapedText};
    use crate::{Glyph, ShapedSlice, ShapedText, Shaper, shape::shaped_text::tests::*};

    type Cluster = (Range<u32>, f32, Vec<Glyph>);

    fn clusters(slice: ShapedSlice<'_>) -> impl Iterator<Item = Cluster> {
        slice.shaped_clusters_range().map(move |i| {
            let cluster = slice.shaped_clusters[i as usize];
            let glyphs = slice.shaped_cluster_glyphs(i).collect();
            (cluster.chars_range(), cluster.advance, glyphs)
        })
    }

    /// The clusters of `line` in logical order.
    fn line_clusters(shaped: &ShapedText, out: &ReshapedText, line: ReshapedLine) -> Vec<Cluster> {
        let range = line.source_shaped_clusters_range();
        let source = ShapedSlice {
            clusters: (range.start, range.end),
            ..shaped.run_slice(0)
        };
        let [start, end] =
            [line.start_run(), line.end_run()].map(|run| run.map(|run| out.run_slice(shaped, run)));
        let slices = start.into_iter().chain([source]).chain(end);
        slices.flat_map(clusters).collect()
    }

    /// The clusters of reshaping `chars` with the edits at its start and end, by reshaping runs as
    /// a whole.
    fn reshape_whole(
        text: &str,
        shaped: &ShapedText,
        chars: Range<u32>,
        [start_edit, end_edit]: [Option<Edit<'_>>; 2],
    ) -> Vec<Cluster> {
        let (mut shaper, mut out, mut result) =
            (Shaper::default(), ReshapedText::new(), Vec::new());
        for (i, run) in (0..).zip(shaped.runs()) {
            let a = chars.start.max(run.characters_range.start);
            let b = chars.end.min(run.characters_range.end);
            if a < b {
                let start = start_edit.filter(|_| a == chars.start);
                let end = end_edit.filter(|_| b == chars.end);
                let run = shaper.reshape(text, shaped, i, a..b, start, end, &mut out);
                result.extend(clusters(out.run_slice(shaped, run)));
            }
        }
        result
    }

    #[test]
    fn reshaped_lines() {
        // The following font selector attempts to shape 'س' with Roboto to split the Arabic text
        // into runs mid-word, so we can exercise shaping that depends on context.
        let fonts = |font| FontForChar {
            ch: 'س',
            font: font_instance(ROBOTO),
            other_font: font_instance(font),
        };
        let mut shaper = Shaper::default();
        let mut fast_misses = 0;
        for (text, font) in [("office AVA", ROBOTO), ("كتب وهب بسلاسة", NOTO_KUFI_ARABIC)]
        {
            let shaped = shape_with_font_selector(text, fonts(font), true);
            let chars = shaped.characters();
            let len = u32::try_from(chars.len()).unwrap();

            // An edit where the character is replaced by itself.
            let same = |i: u32| Edit::new(1, &text[chars[i as usize].text_byte_range()]);

            let mut reshaper = shaper.reshaper(text, &shaped);
            let mut out = ReshapedText::new();

            let boundaries = || (0..=len).filter(|&i| i == len || chars[i as usize].grapheme_start);
            for p in boundaries() {
                for q in boundaries().filter(|&q| q >= p) {
                    let source: f32 = (shaped.shaped_clusters().iter())
                        .filter(|c| (p..q).contains(&c.chars_range().start))
                        .map(|c| c.advance)
                        .sum();
                    let none = [None; 2];
                    let insert = [Some(Edit::new(0, "s")), Some(Edit::new(0, "-"))];
                    // Replacing a character by itself is no edit at all (and empty lines drop their
                    // edits).
                    let same = [Some(same(p.min(len - 1))), Some(same(q.max(1) - 1))];
                    for (edits, equivalent_edits) in [(none, none), (insert, insert), (same, none)]
                    {
                        let replaced = edits.map(|edit| edit.map_or(0, Edit::replaced_chars));
                        if p < q && replaced[0] + replaced[1] > q - p {
                            continue;
                        }
                        let whole = reshape_whole(text, &shaped, p..q, equivalent_edits);
                        let start = reshaper.measure_start(p, edits[0]);
                        let end = reshaper.measure_end(q, edits[1]);
                        let fast = reshaper.line(&start, &end);
                        for (line, exact) in [(fast, false), (reshaper.make_exact(fast), true)] {
                            let pushed = reshaper.push_line(&line, &mut out);
                            let clusters = line_clusters(&shaped, &out, pushed);
                            let advance: f32 = clusters.iter().map(|c| c.1).sum();
                            assert!((advance - source - line.advance_delta()).abs() < 1e-3);
                            assert_eq!(pushed.advance_delta(), line.advance_delta());
                            if exact {
                                assert_eq!(clusters, whole, "{text} {p}..{q} {edits:?}");
                            } else {
                                fast_misses += usize::from(clusters != whole);
                            }
                        }
                    }
                }
            }
        }
        assert!(
            fast_misses > 0,
            "We expect some misses for the 'fast' reshaping, so that we're actually exercising `make_exact`"
        );
    }
}
