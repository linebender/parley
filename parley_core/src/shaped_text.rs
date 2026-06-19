// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The shaped text representation.
//!
//! This sits between shaping and everything built on top of it (line breaking, justification,
//! rendering).
//!
//! Reuse a single [`ShapedText`] across paragraphs via [`ShapedText::clear`] to avoid reallocating.
//!
//! Runs are stored in logical (source) order together with their bidi level. See
//! [`crate::reorder_visual`] if you need visual order.

use alloc::vec::Vec;
use core::ops::Range;

use fontique::{Attributes, Synthesis};
use linebender_resource_handle::FontData;
use parlance::{FontFeature, Language, Script};

use crate::common::{Boundary, NormalizedCoord, RunMetrics, RunOrientation, Whitespace};
use crate::itemize::Item;

/// Whether a [`Run`] holds shaped text or an inline box.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum RunKind {
    /// A run of shaped glyphs sharing one font.
    #[default]
    Text,
    /// An inline box: it occupies space and takes part in line breaking and bidi reordering, but
    /// has no font and emits no glyphs. See [`Run::inline_box`].
    InlineBox,
}

/// The geometry of an inline box. This is set by the caller.
///
/// [`Self::advance`] is reserved along the run's main (inline) axis and
/// [`Self::ascent`]/[`Self::descent`] across it (so the box contributes to line height). The
/// surrounding text is positioned around it.
///
/// Supply one for each [`crate::ItemKind::InlineBox`] item via [`ShapedText::push_inline_box`].
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct InlineBox {
    /// Byte offset of the box's `U+FFFC OBJECT REPLACEMENT CHARACTER` marker in the source text.
    pub offset: usize,
    /// An opaque identifier set and read by the caller.
    pub id: u64,
    /// The advance along the run's main (inline) axis: the space the box reserves between the
    /// surrounding text.
    pub advance: f32,
    /// The extent above the baseline on the cross axis, contributing to line ascent.
    pub ascent: f32,
    /// The extent below the baseline on the cross axis, contributing to line descent.
    pub descent: f32,
}

/// A positioned glyph.
///
/// `advance` is along the run's main (inline) axis; `x`/`y` are offsets from the pen position.
///
/// For horizontal text the pen advances by `advance` in `x`; for vertical text it advances in `y`.
#[derive(Copy, Clone, Default, Debug, PartialEq)]
pub struct Glyph {
    /// The glyph identifier within the run's font.
    pub id: u32,
    /// Horizontal offset from the pen position.
    pub x: f32,
    /// Vertical offset from the pen position.
    pub y: f32,
    /// Advance along the run's main axis.
    pub advance: f32,
}

/// Per-run storage.
///
/// Ranges index into the parent [`ShapedText`]'s arrays.
#[derive(Clone, Debug)]
pub(crate) struct RunData {
    /// Index of the font for the run.
    pub(crate) font_index: usize,
    /// Font size.
    pub(crate) font_size: f32,
    /// Font attributes, needed for accessibility.
    pub(crate) font_attrs: Attributes,
    /// Synthesis for rendering (contains variation settings)
    pub(crate) synthesis: Synthesis,
    /// Range of normalized coordinates in the layout data.
    pub(crate) coords_range: Range<usize>,
    /// Range of OpenType features applied to this run, indexing into the [`ShapedText`]'s
    /// `features` array.
    pub(crate) features_range: Range<usize>,
    /// Range of the source text.
    pub(crate) text_range: Range<usize>,
    /// Range of clusters.
    pub(crate) cluster_range: Range<usize>,
    /// Base into the glyph array; cluster `glyph_offset`s are relative to this.
    pub(crate) glyph_start: usize,
    pub(crate) script: Script,
    pub(crate) language: Option<Language>,
    /// Bidi level for the run.
    pub(crate) bidi_level: u8,
    pub(crate) orientation: RunOrientation,
    /// Metrics for the run.
    pub(crate) metrics: RunMetrics,
    /// Total advance of the run.
    pub(crate) advance: f32,
    /// Whether this run is shaped text or an inline box. A box run has no font
    /// (its `font_index` is unused) and a single zero-glyph cluster.
    pub(crate) kind: RunKind,
    /// The caller's inline-box id; only meaningful when `kind` is
    /// [`RunKind::InlineBox`].
    pub(crate) inline_box_id: u64,
    /// Word spacing baked into the cluster advances; see also [`Self::letter_spacing`].
    pub(crate) word_spacing: f32,
    /// Letter spacing baked into the cluster advances.
    ///
    /// This is retained so reshaping a fragment ([`ShapedText::reshape_locate`]) can re-apply it.
    pub(crate) letter_spacing: f32,
}

/// Per-cluster storage: one per grapheme cluster.
///
/// If `glyph_len == INLINE_GLYPH`, then `glyph_offset` is a glyph identifier, otherwise, it's the
/// relative offset into the glyph array with the base taken from the owning run.
#[derive(Copy, Clone, Debug)]
pub(crate) struct ClusterData {
    pub(crate) advance: f32,
    /// Run-relative offset into the glyph array, or the glyph id itself when
    /// `glyph_len == INLINE_GLYPH`.
    pub(crate) glyph_offset: u32,
    /// Byte offset of this cluster from the start of the run's `text_range`.
    pub(crate) text_offset: u16,
    /// Number of source bytes in this cluster.
    pub(crate) text_len: u8,
    /// Number of glyphs, or [`INLINE_GLYPH`] for the single-glyph fast path.
    pub(crate) glyph_len: u8,
    pub(crate) flags: u16,
    pub(crate) boundary: Boundary,
    pub(crate) whitespace: Whitespace,
}

/// Sentinel `glyph_len` marking the single-inline-glyph fast path.
pub(crate) const INLINE_GLYPH: u8 = 0xFF;

impl ClusterData {
    /// This cluster begins a ligature (its glyphs span several clusters).
    pub(crate) const LIGATURE_START: u16 = 1 << 0;
    /// This cluster is a non-initial component of a ligature.
    pub(crate) const LIGATURE_COMPONENT: u16 = 1 << 1;
    /// Breaking the text *before* this cluster requires reshaping both sides (`HarfBuzz`
    /// `UNSAFE_TO_BREAK`). See [`ShapedText::unsafe_break_region`].
    pub(crate) const UNSAFE_TO_BREAK: u16 = 1 << 2;
    /// Concatenating a separately-shaped run before this cluster may change the result (`HarfBuzz`
    /// `UNSAFE_TO_CONCAT`): summed advances across here are not guaranteed exact.
    pub(crate) const UNSAFE_TO_CONCAT: u16 = 1 << 3;
    /// A tatweel (kashida) may be inserted before this cluster for justification (`HarfBuzz`
    /// `SAFE_TO_INSERT_TATWEEL`).
    pub(crate) const SAFE_TO_INSERT_TATWEEL: u16 = 1 << 4;

    #[inline]
    fn has(self, flag: u16) -> bool {
        self.flags & flag != 0
    }
}

/// The shaped result of one paragraph (or any contiguous unit of text).
///
/// Shape a paragraph using [`ShapeContext::shape_run`](crate::ShapeContext::shape_run), read the
/// result using [`Self::runs`], and reuse it with [`clear`](Self::clear).
#[derive(Clone, Debug, Default)]
pub struct ShapedText {
    runs: Vec<RunData>,
    /// Cluster and glyph arrays for all runs. The shaper writes new runs straight onto their ends
    /// (see [`Self::finish_run`]), so they are `pub(crate)` for the `shape` module to append into.
    pub(crate) clusters: Vec<ClusterData>,
    pub(crate) glyphs: Vec<Glyph>,
    coords: Vec<NormalizedCoord>,
    /// Per-run OpenType features; each run indexes a contiguous slice via
    /// [`RunData::features_range`].
    ///
    /// Retained so reshaping a fragment ([`ShapedText::reshape_locate`]) re-applies the same
    /// features as the original run.
    features: Vec<FontFeature>,
    /// Interned font handles; runs reference these by index so a font blob shared across runs is
    /// stored once.
    fonts: Vec<FontData>,
}

impl ShapedText {
    /// Creates an empty `ShapedText`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Clears while retaining capacity, so the allocation can be reused for the next paragraph.
    pub fn clear(&mut self) {
        self.runs.clear();
        self.clusters.clear();
        self.glyphs.clear();
        self.coords.clear();
        self.features.clear();
        self.fonts.clear();
    }

    /// Returns the number of runs, in logical order.
    #[inline]
    pub fn len(&self) -> usize {
        self.runs.len()
    }

    /// Returns `true` if there are no runs.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.runs.is_empty()
    }

    /// Returns the run at `index` in logical order, if any.
    #[inline]
    pub fn run(&self, index: usize) -> Option<Run<'_>> {
        let data = self.runs.get(index)?;
        Some(Run {
            shaped: self,
            data,
            index,
        })
    }

    /// Iterates the runs in logical (source) order.
    #[inline]
    pub fn runs(&self) -> impl Iterator<Item = Run<'_>> + '_ {
        (0..self.runs.len()).map(|index| Run {
            shaped: self,
            data: &self.runs[index],
            index,
        })
    }

    /// Appends an inline box as its own run, in logical (source) order.
    ///
    /// Call this for each [`Item`] of kind [`ItemKind::InlineBox`](crate::ItemKind::InlineBox),
    /// interleaved with [`ShapeContext::shape_run`](crate::ShapeContext::shape_run) for text items
    /// so that runs stay in source order. `geometry` is the caller-measured size.
    ///
    /// The box becomes a [`Run`] of [`RunKind::InlineBox`] holding a single zero-glyph [`Cluster`]
    /// with the correct advance that is break-safe on both sides.
    pub fn push_inline_box(&mut self, item: &Item, geometry: InlineBox) {
        let cluster_start = self.clusters.len();
        let glyph_start = self.glyphs.len();
        self.clusters.push(ClusterData {
            advance: geometry.advance,
            glyph_offset: 0,
            text_offset: 0,
            text_len: (item.text_range.end - item.text_range.start) as u8,
            glyph_len: 0,
            flags: 0,
            boundary: item.boundary,
            whitespace: Whitespace::None,
        });
        self.runs.push(RunData {
            font_index: 0,
            font_size: 0.0,
            synthesis: Synthesis::default(),
            font_attrs: Attributes::default(),
            coords_range: 0..0,
            features_range: 0..0,
            text_range: item.text_range.clone(),
            cluster_range: cluster_start..self.clusters.len(),
            glyph_start,
            script: item.script,
            language: item.language,
            bidi_level: item.level,
            orientation: item.orientation,
            metrics: RunMetrics {
                ascent: geometry.ascent,
                descent: geometry.descent,
                ..RunMetrics::default()
            },
            advance: geometry.advance,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            kind: RunKind::InlineBox,
            inline_box_id: geometry.id,
        });
    }

    /// Computes the minimal text ranges that must be reshaped when a line break is committed at
    /// byte offset `pos`.
    ///
    /// Cursive scripts (e.g., Arabic) join glyphs; severing that join changes the glyphs and
    /// advances on both sides. The same is true of ligatures (e.g., Latin `fi`): a break between a
    /// ligature's source characters must decompose the single glyph back into standalone ones. This
    /// method expands outward from `pos` to the nearest break-safe cluster boundaries, returning
    /// the two fragments to re-shape in isolation.
    ///
    /// Both ranges are empty when `pos` is already a safe boundary, which will be a common case
    /// (e.g., Latin breaks at spaces, CJK), for which no reshaping is needed. `pos` must be a
    /// cluster boundary (a committed break); otherwise empty ranges are returned.
    ///
    /// Use the results with [`reshape_fragment`](crate::ShapeContext::reshape_fragment). See also
    /// [`ShapeContext::apply_break`](crate::ShapeContext::apply_break), which calls this for you.
    pub fn unsafe_break_region(&self, pos: usize) -> ReshapeRanges {
        let empty = ReshapeRanges {
            tail: pos..pos,
            head: pos..pos,
        };
        // Locate the run whose logical text contains `pos`, and the cluster that starts at `pos`.
        // Runs and clusters are both in logical order.
        let Some((run_index, ci)) = self.locate_cluster(pos) else {
            return empty;
        };
        let run = &self.runs[run_index];
        let clusters = &self.clusters[run.cluster_range.clone()];
        let local = ci - run.cluster_range.start;

        // A safe break before `pos` needs no reshaping. The first cluster of a run is always safe
        // (it sits on an itemization boundary).
        if local == 0 || !clusters[local].has(ClusterData::UNSAFE_TO_BREAK) {
            return empty;
        }

        let start_of =
            |local_idx: usize| run.text_range.start + clusters[local_idx].text_offset as usize;

        // Expand left to the nearest break-safe cluster boundary (or run start).
        let mut lo = local;
        while lo > 0 && clusters[lo].has(ClusterData::UNSAFE_TO_BREAK) {
            lo -= 1;
        }

        // Expand right to the nearest break-safe cluster boundary (or run end).
        let mut hi = local + 1;
        while hi < clusters.len() && clusters[hi].has(ClusterData::UNSAFE_TO_BREAK) {
            hi += 1;
        }
        let head_end = if hi < clusters.len() {
            start_of(hi)
        } else {
            run.text_range.end
        };

        ReshapeRanges {
            tail: start_of(lo)..pos,
            head: pos..head_end,
        }
    }

    /// Computes the minimal text range to reshape when merging two fragments at `pos`.
    ///
    /// Empty when no reshape is needed.
    ///
    /// Use the result with [`reshape_fragment`](crate::ShapeContext::reshape_fragment). See also
    /// [`ShapeContext::apply_concat`](crate::ShapeContext::apply_concat), which calls this for
    /// you.
    pub fn unsafe_concat_region(&self, pos: usize) -> Range<usize> {
        let empty = pos..pos;
        let Some((run_index, ci)) = self.locate_cluster(pos) else {
            return empty;
        };
        let run = &self.runs[run_index];
        let clusters = &self.clusters[run.cluster_range.clone()];
        let local = ci - run.cluster_range.start;

        // A break at a run boundary needed no reshape on commit, so it needs none to undo. And if
        // joining wouldn't change anything, there's nothing to do.
        if local == 0 || !clusters[local].has(ClusterData::UNSAFE_TO_CONCAT) {
            return empty;
        }

        let start_of =
            |local_idx: usize| run.text_range.start + clusters[local_idx].text_offset as usize;

        // Scan outward through clusters that carry UNSAFE_TO_CONCAT, i.e., those whose shape
        // depends on what's before them. We stop on either side at a cluster whose shape is
        // independent of left context (no UNSAFE_TO_CONCAT); that cluster is a safe edge, as its
        // shape won't change however we reshape inside the range.
        let mut lo = local - 1;
        while lo > 0 && clusters[lo].has(ClusterData::UNSAFE_TO_CONCAT) {
            lo -= 1;
        }

        let mut hi = local + 1;
        while hi < clusters.len() && clusters[hi].has(ClusterData::UNSAFE_TO_CONCAT) {
            hi += 1;
        }
        let end = if hi < clusters.len() {
            start_of(hi)
        } else {
            run.text_range.end
        };

        start_of(lo)..end
    }

    /// Finds `(run_index, global_cluster_index)` for the cluster that starts at byte offset `pos`,
    /// or `None` if `pos` is not a cluster boundary.
    fn locate_cluster(&self, pos: usize) -> Option<(usize, usize)> {
        // Runs are in logical order, so their text ranges are ascending and non-overlapping:
        // search for the last run starting at or before `pos`.
        let run_index = self
            .runs
            .partition_point(|r| r.text_range.start <= pos)
            .checked_sub(1)?;
        let run = &self.runs[run_index];
        if pos >= run.text_range.end {
            return None;
        }
        let clusters = &self.clusters[run.cluster_range.clone()];
        let offset = (pos - run.text_range.start) as u16;
        let local = clusters
            .binary_search_by_key(&offset, |c| c.text_offset)
            .ok()?;
        Some((run_index, run.cluster_range.start + local))
    }

    /// Interns a font handle, returning its index. Identical blobs are stored once.
    pub(crate) fn intern_font(&mut self, font: FontData) -> usize {
        if let Some(index) = self.fonts.iter().position(|f| f == &font) {
            index
        } else {
            self.fonts.push(font);
            self.fonts.len() - 1
        }
    }

    /// Finalizes a run.
    ///
    /// The clusters and glyphs must already have been appended onto [`Self::clusters`] and
    /// [`Self::glyphs`] (starting at `cluster_start`/`glyph_start`).
    ///
    /// This appends `coords` and `features`, and fills in the run's `coords_range`,
    /// `features_range`, `glyph_start` and `cluster_range` accordingly.
    pub(crate) fn finish_run(
        &mut self,
        mut run: RunData,
        cluster_start: usize,
        glyph_start: usize,
        coords: &[NormalizedCoord],
        features: &[FontFeature],
    ) {
        let coords_start = self.coords.len();
        self.coords.extend_from_slice(coords);
        run.coords_range = coords_start..self.coords.len();
        let features_start = self.features.len();
        self.features.extend_from_slice(features);
        run.features_range = features_start..self.features.len();
        run.glyph_start = glyph_start;
        run.cluster_range = cluster_start..self.clusters.len();
        self.runs.push(run);
    }

    /// Locates the run, clusters and glyphs covering `text_range` (which must lie
    /// within a single run and start/end on cluster boundaries), so the caller can
    /// re-shape the range in isolation and splice the result back with
    /// [`splice_fragment`](Self::splice_fragment). The run's shaping parameters
    /// are read directly from the run at reshape time.
    pub(crate) fn reshape_locate(&self, text_range: Range<usize>) -> Option<ReshapeTarget> {
        if text_range.is_empty() {
            return None;
        }
        let run_index = self.runs.iter().position(|r| {
            text_range.start >= r.text_range.start && text_range.end <= r.text_range.end
        })?;
        let run = &self.runs[run_index];
        let clusters = &self.clusters[run.cluster_range.clone()];

        let lo_off = (text_range.start - run.text_range.start) as u16;
        let hi_off = (text_range.end - run.text_range.start) as u16;
        let c0_local = clusters
            .binary_search_by_key(&lo_off, |c| c.text_offset)
            .ok()?;
        let c1_local = if text_range.end == run.text_range.end {
            clusters.len()
        } else {
            clusters
                .binary_search_by_key(&hi_off, |c| c.text_offset)
                .ok()?
        };
        if c0_local > c1_local {
            return None;
        }

        // Inline-glyph clusters occupy no slot in the glyph array, so the glyph
        // range is a prefix-sum of the array glyph counts.
        let array_len = |c: &ClusterData| {
            if c.glyph_len == INLINE_GLYPH {
                0
            } else {
                c.glyph_len as usize
            }
        };
        let g0_local: usize = clusters[..c0_local].iter().map(array_len).sum();
        let seg_glyphs: usize = clusters[c0_local..c1_local].iter().map(array_len).sum();
        let g0 = run.glyph_start + g0_local;

        Some(ReshapeTarget {
            run_index,
            cluster_range: run.cluster_range.start + c0_local..run.cluster_range.start + c1_local,
            glyph_range: g0..g0 + seg_glyphs,
            text_offset_base: lo_off,
        })
    }

    /// Replaces the clusters/glyphs identified by `target` with freshly shaped
    /// ones, fixing up the affected run's range/advance and shifting every later
    /// run's cluster and glyph bases.
    ///
    /// `new_clusters` carry fragment-relative `text_offset`s and fragment-local
    /// (0-based) array `glyph_offset`s; this method rebases both. `new_glyphs` are
    /// in the same order the clusters reference.
    pub(crate) fn splice_fragment(
        &mut self,
        target: &ReshapeTarget,
        new_clusters: &[ClusterData],
        new_glyphs: &[Glyph],
    ) {
        let run = &self.runs[target.run_index];
        let local_g0 = (target.glyph_range.start - run.glyph_start) as u32;
        let old_run_cluster_end = run.cluster_range.end;
        let c0 = target.cluster_range.start;

        let delta_g = new_glyphs.len() as isize - target.glyph_range.len() as isize;
        let delta_c = new_clusters.len() as isize - target.cluster_range.len() as isize;

        // Rebase the new clusters into run-relative coordinates.
        let rebased = new_clusters.iter().map(|cluster| {
            let mut cluster = *cluster;
            cluster.text_offset += target.text_offset_base;
            if cluster.glyph_len != INLINE_GLYPH {
                cluster.glyph_offset += local_g0;
            }
            cluster
        });

        self.glyphs
            .splice(target.glyph_range.clone(), new_glyphs.iter().copied());
        self.clusters.splice(target.cluster_range.clone(), rebased);

        // Clusters of the same run that follow the new fragment shift their array
        // glyph offsets by the change in glyph count.
        let tail_start = c0 + new_clusters.len();
        let tail_end = (old_run_cluster_end as isize + delta_c) as usize;
        if delta_g != 0 {
            for cluster in &mut self.clusters[tail_start..tail_end] {
                if cluster.glyph_len != INLINE_GLYPH {
                    cluster.glyph_offset = (cluster.glyph_offset as isize + delta_g) as u32;
                }
            }
        }

        let run = &mut self.runs[target.run_index];
        run.cluster_range.end = tail_end;
        let advance = self.clusters[self.runs[target.run_index].cluster_range.clone()]
            .iter()
            .map(|c| c.advance)
            .sum();
        self.runs[target.run_index].advance = advance;

        // Every later run shifts by the cluster/glyph deltas.
        for run in &mut self.runs[target.run_index + 1..] {
            run.cluster_range.start = (run.cluster_range.start as isize + delta_c) as usize;
            run.cluster_range.end = (run.cluster_range.end as isize + delta_c) as usize;
            run.glyph_start = (run.glyph_start as isize + delta_g) as usize;
        }
    }
}

/// Locates the clusters and glyphs that a reshaped fragment replaces within a run.
///
/// Produced by [`ShapedText::reshape_locate`] and consumed by [`ShapedText::splice_fragment`].
pub(crate) struct ReshapeTarget {
    pub(crate) run_index: usize,
    /// Global cluster indices to replace.
    pub(crate) cluster_range: Range<usize>,
    /// Global glyph indices to replace.
    pub(crate) glyph_range: Range<usize>,
    /// Byte offset of the fragment start relative to the run's `text_range`.
    pub(crate) text_offset_base: u16,
}

/// The fragments that must be re-shaped when committing a line break, as computed by
/// [`ShapedText::unsafe_break_region`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReshapeRanges {
    /// Text range ending at the break, on the line being closed. Empty if the break is already
    /// safe.
    pub tail: Range<usize>,
    /// Text range starting at the break, on the next line. Empty if the break is already safe.
    pub head: Range<usize>,
}

impl ReshapeRanges {
    /// Returns `true` if no reshaping is required (the break is break-safe).
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.tail.is_empty() && self.head.is_empty()
    }
}

/// A sequence of clusters sharing one font, size, script, direction and writing mode.
#[derive(Clone, Copy)]
pub struct Run<'a> {
    shaped: &'a ShapedText,
    data: &'a RunData,
    index: usize,
}

impl<'a> Run<'a> {
    /// Returns this run's index within the [`ShapedText`], in logical order.
    #[inline]
    pub fn index(&self) -> usize {
        self.index
    }

    /// Returns the source text range covered by this run.
    #[inline]
    pub fn text_range(&self) -> Range<usize> {
        self.data.text_range.clone()
    }

    /// Returns whether this run holds shaped text or an inline box.
    #[inline]
    pub fn kind(&self) -> RunKind {
        self.data.kind
    }

    /// Returns the box geometry for an inline-box run, or `None` for a text run.
    ///
    /// `Some` exactly when [`Self::kind`] is [`RunKind::InlineBox`]. The returned [`InlineBox`]
    /// reflects this run's text offset, advance and metrics.
    #[inline]
    pub fn inline_box(&self) -> Option<InlineBox> {
        match self.data.kind {
            RunKind::Text => None,
            RunKind::InlineBox => Some(InlineBox {
                offset: self.data.text_range.start,
                id: self.data.inline_box_id,
                advance: self.data.advance,
                ascent: self.data.metrics.ascent,
                descent: self.data.metrics.descent,
            }),
        }
    }

    /// Returns the font used to shape this run, or `None` for an inline-box run (which has no
    /// font).
    #[inline]
    pub fn font(&self) -> Option<&'a FontData> {
        match self.data.kind {
            RunKind::Text => Some(&self.shaped.fonts[self.data.font_index]),
            RunKind::InlineBox => None,
        }
    }

    /// Returns the font size in pixels per em.
    #[inline]
    pub fn font_size(&self) -> f32 {
        self.data.font_size
    }

    /// Returns the requested font attributes (weight, width, style).
    ///
    /// Useful for accessibility and for detecting synthesized styling alongside
    /// [`Self::synthesis`].
    #[inline]
    pub fn font_attrs(&self) -> Attributes {
        self.data.font_attrs
    }

    /// Returns the synthesis applied to the font (e.g. faux bold/oblique).
    #[inline]
    pub fn synthesis(&self) -> Synthesis {
        self.data.synthesis
    }

    /// Returns the normalized variation coordinates for this run's font instance.
    #[inline]
    pub fn normalized_coords(&self) -> &'a [NormalizedCoord] {
        &self.shaped.coords[self.data.coords_range.clone()]
    }

    /// Returns the Unicode script of this run.
    #[inline]
    pub fn script(&self) -> Script {
        self.data.script
    }

    /// Returns the resolved language of this run, if one was determined.
    #[inline]
    pub fn language(&self) -> Option<Language> {
        self.data.language
    }

    /// Returns the bidi embedding level.
    ///
    /// Reorder runs by this per line (UAX #9 L2). See also [`Self::is_rtl`].
    #[inline]
    pub fn bidi_level(&self) -> u8 {
        self.data.bidi_level
    }

    /// Returns `true` if the run has right-to-left directionality.
    #[inline]
    pub fn is_rtl(&self) -> bool {
        self.data.bidi_level & 1 != 0
    }

    /// Returns the resolved orientation of this run.
    #[inline]
    pub fn orientation(&self) -> RunOrientation {
        self.data.orientation
    }

    /// Returns the run's vertical metrics and decoration geometry.
    #[inline]
    pub fn metrics(&self) -> &'a RunMetrics {
        &self.data.metrics
    }

    /// Returns the total advance of the run along its main axis.
    #[inline]
    pub fn advance(&self) -> f32 {
        self.data.advance
    }

    /// Returns the number of clusters in the run.
    #[inline]
    pub fn len(&self) -> usize {
        self.data.cluster_range.len()
    }

    /// Returns `true` if the run has no clusters.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.cluster_range.is_empty()
    }

    /// Returns the cluster at `index` within this run.
    #[inline]
    pub fn get(&self, index: usize) -> Option<Cluster<'a>> {
        let global = self.data.cluster_range.start + index;
        if global >= self.data.cluster_range.end {
            return None;
        }
        Some(Cluster {
            run: *self,
            data: &self.shaped.clusters[global],
        })
    }

    /// Iterates this run's clusters in logical order.
    #[inline]
    pub fn clusters(&self) -> impl Iterator<Item = Cluster<'a>> + 'a {
        let run = *self;
        self.shaped.clusters[self.data.cluster_range.clone()]
            .iter()
            .map(move |data| Cluster { run, data })
    }

    /// Iterates every glyph in the run, in logical order, flattening clusters (including inline
    /// single-glyph clusters).
    #[inline]
    pub fn glyphs(&self) -> impl Iterator<Item = Glyph> + 'a {
        self.clusters().flat_map(|cluster| cluster.glyphs())
    }

    pub(crate) fn letter_spacing(&self) -> f32 {
        self.data.letter_spacing
    }

    pub(crate) fn word_spacing(&self) -> f32 {
        self.data.word_spacing
    }

    /// The OpenType features applied when this run was shaped.
    ///
    /// Retained so [`ShapeContext::reshape_fragment`](crate::ShapeContext::reshape_fragment) can
    /// re-apply the same feature set.
    pub(crate) fn features(&self) -> &'a [FontFeature] {
        &self.shaped.features[self.data.features_range.clone()]
    }
}

impl core::fmt::Debug for Run<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Run")
            .field("text_range", &self.text_range())
            .field("script", &self.script())
            .field("bidi_level", &self.bidi_level())
            .field("advance", &self.advance())
            .field("clusters", &self.len())
            .finish_non_exhaustive()
    }
}

/// A cluster: mapping between a span of source text and a span of glyphs.
#[derive(Clone, Copy)]
pub struct Cluster<'a> {
    run: Run<'a>,
    data: &'a ClusterData,
}

impl<'a> Cluster<'a> {
    /// Returns the source text range of this cluster.
    pub fn text_range(&self) -> Range<usize> {
        let start = self.run.data.text_range.start + self.data.text_offset as usize;
        start..start + self.data.text_len as usize
    }

    /// Returns the advance of this cluster along the run's main axis.
    #[inline]
    pub fn advance(&self) -> f32 {
        self.data.advance
    }

    /// Returns the segmentation boundary immediately *before* this cluster.
    #[inline]
    pub fn boundary(&self) -> Boundary {
        self.data.boundary
    }

    /// Returns the cluster's whitespace classification.
    #[inline]
    pub fn whitespace(&self) -> Whitespace {
        self.data.whitespace
    }

    /// Iterates the glyphs of this cluster (one synthesized glyph for the inline fast path).
    pub fn glyphs(&self) -> ClusterGlyphs<'a> {
        if self.data.glyph_len == INLINE_GLYPH {
            ClusterGlyphs(ClusterGlyphsInner::Single(Some(Glyph {
                id: self.data.glyph_offset,
                advance: self.data.advance,
                x: 0.0,
                y: 0.0,
            })))
        } else {
            let base = self.run.data.glyph_start + self.data.glyph_offset as usize;
            let glyphs = &self.run.shaped.glyphs[base..base + self.data.glyph_len as usize];
            ClusterGlyphs(ClusterGlyphsInner::Slice(glyphs.iter()))
        }
    }

    /// Returns the number of glyphs in this cluster.
    #[inline]
    pub fn glyph_len(&self) -> usize {
        if self.data.glyph_len == INLINE_GLYPH {
            1
        } else {
            self.data.glyph_len as usize
        }
    }

    /// Returns `true` if this cluster begins a ligature.
    #[inline]
    pub fn is_ligature_start(&self) -> bool {
        self.data.has(ClusterData::LIGATURE_START)
    }

    /// Returns `true` if the cluster is a ligature continuation.
    #[inline]
    pub fn is_ligature_continuation(&self) -> bool {
        self.data.has(ClusterData::LIGATURE_COMPONENT)
    }

    /// Returns `true` if breaking the line *before* this cluster requires reshaping both sides
    /// (`HarfBuzz` `UNSAFE_TO_BREAK`). Use this when *committing* a break; see
    /// [`ShapedText::unsafe_break_region`].
    #[inline]
    pub fn unsafe_to_break(&self) -> bool {
        self.data.has(ClusterData::UNSAFE_TO_BREAK)
    }

    /// Returns `true` if concatenating a separately-shaped piece *before* this cluster may change
    /// the shape (`HarfBuzz` `UNSAFE_TO_CONCAT`). Use this when *measuring* candidate lines from
    /// pre-shaped advances, and when deciding whether merging two adjacent fragments at this
    /// cluster's leading boundary needs reshaping; see [`ShapedText::unsafe_concat_region`].
    #[inline]
    pub fn unsafe_to_concat(&self) -> bool {
        self.data.has(ClusterData::UNSAFE_TO_CONCAT)
    }

    /// Returns `true` if a tatweel (kashida) may be inserted before this cluster
    /// for Arabic-style justification (`HarfBuzz` `SAFE_TO_INSERT_TATWEEL`).
    #[inline]
    pub fn safe_to_insert_tatweel(&self) -> bool {
        self.data.has(ClusterData::SAFE_TO_INSERT_TATWEEL)
    }
}

impl core::fmt::Debug for Cluster<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Cluster")
            .field("text_range", &self.text_range())
            .field("advance", &self.advance())
            .field("boundary", &self.boundary())
            .field("glyph_len", &self.glyph_len())
            .finish_non_exhaustive()
    }
}

/// Iterator over the glyphs of a [`Cluster`], returned by [`Cluster::glyphs`].
#[derive(Clone, Debug)]
pub struct ClusterGlyphs<'a>(ClusterGlyphsInner<'a>);

#[derive(Clone, Debug)]
enum ClusterGlyphsInner<'a> {
    Single(Option<Glyph>),
    Slice(core::slice::Iter<'a, Glyph>),
}

impl Iterator for ClusterGlyphs<'_> {
    type Item = Glyph;

    fn next(&mut self) -> Option<Glyph> {
        match &mut self.0 {
            ClusterGlyphsInner::Single(glyph) => glyph.take(),
            ClusterGlyphsInner::Slice(iter) => iter.next().copied(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match &self.0 {
            ClusterGlyphsInner::Single(glyph) => {
                let n = usize::from(glyph.is_some());
                (n, Some(n))
            }
            ClusterGlyphsInner::Slice(iter) => iter.size_hint(),
        }
    }
}

impl ExactSizeIterator for ClusterGlyphs<'_> {}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    /// Builds a single-run `ShapedText` from `(advance, text_len)` pairs, one per cluster, each
    /// with a single inline glyph. `unsafe_breaks` lists cluster indices flagged unsafe-to-break.
    fn single_run(clusters: &[(f32, u8)], unsafe_breaks: &[usize]) -> ShapedText {
        let mut st = ShapedText::new();
        let glyph_start = st.glyphs.len();
        let cluster_start = st.clusters.len();
        let mut advance = 0.0;
        let mut text_offset: u16 = 0;
        for (i, &(adv, text_len)) in clusters.iter().enumerate() {
            let mut flags = 0;
            if unsafe_breaks.contains(&i) {
                flags |= ClusterData::UNSAFE_TO_BREAK;
            }
            st.clusters.push(ClusterData {
                advance: adv,
                glyph_offset: 100 + i as u32, // inline glyph id
                text_offset,
                text_len,
                glyph_len: INLINE_GLYPH,
                flags,
                boundary: Boundary::None,
                whitespace: Whitespace::None,
            });
            advance += adv;
            text_offset += text_len as u16;
        }
        st.runs.push(RunData {
            font_index: 0,
            font_size: 16.0,
            synthesis: Synthesis::default(),
            font_attrs: Attributes::default(),
            coords_range: 0..0,
            features_range: 0..0,
            text_range: 0..text_offset as usize,
            cluster_range: cluster_start..st.clusters.len(),
            glyph_start,
            script: Script::from_bytes(*b"Latn"),
            language: None,
            bidi_level: 0,
            orientation: RunOrientation::Horizontal,
            metrics: RunMetrics::default(),
            advance,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            kind: RunKind::Text,
            inline_box_id: 0,
        });
        st
    }

    #[test]
    fn run_and_cluster_access() {
        let st = single_run(&[(10.0, 1), (12.0, 1), (8.0, 1)], &[]);
        let run = st.run(0).unwrap();
        assert_eq!(run.len(), 3);
        assert_eq!(run.advance(), 30.0);
        assert!(run.get(3).is_none());

        let c0 = run.get(0).unwrap();
        assert_eq!(c0.text_range(), 0..1);
        assert_eq!(c0.advance(), 10.0);

        // Inline glyph fast path: one synthesized glyph carrying the cluster advance.
        let c1 = run.get(1).unwrap();
        assert_eq!(c1.glyph_len(), 1);
        let glyphs: Vec<_> = c1.glyphs().collect();
        assert_eq!(glyphs.len(), 1);
        assert_eq!(glyphs[0].id, 101);
        assert_eq!(glyphs[0].advance, 12.0);
        assert_eq!((glyphs[0].x, glyphs[0].y), (0.0, 0.0));

        // run.glyphs() flattens inline-glyph clusters that have no entry in the array.
        assert_eq!(run.glyphs().count(), 3);
    }

    #[test]
    fn unsafe_break_region() {
        // Nothing unsafe: every position is a safe break.
        let st = single_run(&[(5.0, 1); 6], &[]);
        for pos in 0..=6 {
            assert!(st.unsafe_break_region(pos).is_empty(), "pos {pos} safe");
        }

        // A single unsafe cluster expands to its immediate safe neighbors, and unrelated positions
        // stay safe.
        let st = single_run(&[(5.0, 1); 6], &[3]);
        let r = st.unsafe_break_region(3);
        assert_eq!(r.tail, 2..3);
        assert_eq!(r.head, 3..4);
        assert!(st.unsafe_break_region(1).is_empty());

        // Contiguous unsafe clusters expand outward until reaching safe boundaries.
        let st = single_run(&[(5.0, 1); 6], &[2, 3, 4]);
        let r = st.unsafe_break_region(3);
        assert_eq!(r.tail, 1..3);
        assert_eq!(r.head, 3..5);

        // The first cluster of a run sits on an itemization boundary and is always safe; expansion
        // clamps to the run start.
        let st = single_run(&[(5.0, 1); 4], &[0, 1]);
        assert!(st.unsafe_break_region(0).is_empty());
        let r = st.unsafe_break_region(1);
        assert_eq!(r.tail, 0..1);
        assert_eq!(r.head, 1..2);

        // A position that's not a cluster boundary can't be a committed break.
        let st = single_run(&[(9.0, 2)], &[0]);
        assert!(st.unsafe_break_region(1).is_empty());
    }

    #[test]
    fn push_inline_box_makes_a_box_run() {
        let mut st = ShapedText::new();
        let item = Item {
            text_range: 4..7,
            char_range: 4..5,
            kind: crate::ItemKind::InlineBox,
            boundary: Boundary::Line,
            script: Script::from_bytes(*b"Latn"),
            language: None,
            level: 0,
            orientation: RunOrientation::Horizontal,
        };
        let geometry = InlineBox {
            offset: 4,
            id: 42,
            advance: 20.0,
            ascent: 18.0,
            descent: 2.0,
        };
        st.push_inline_box(&item, geometry);

        assert_eq!(st.len(), 1);
        let run = st.run(0).unwrap();
        assert_eq!(run.kind(), RunKind::InlineBox);
        assert_eq!(run.text_range(), 4..7);
        assert_eq!(run.advance(), 20.0);
        assert_eq!(run.bidi_level(), 0);
        // The box contributes its cross-axis extent to line height via the metrics.
        assert_eq!(run.metrics().ascent, 18.0);
        assert_eq!(run.metrics().descent, 2.0);
        // A box has no font; the geometry round-trips through `inline_box`.
        assert!(run.font().is_none());
        assert_eq!(run.inline_box(), Some(geometry));

        // Exactly one zero-glyph cluster, carrying the advance and leading boundary so a
        // cluster-level line breaker handles the box with no special case.
        assert_eq!(run.len(), 1);
        let cluster = run.get(0).unwrap();
        assert_eq!(cluster.text_range(), 4..7);
        assert_eq!(cluster.advance(), 20.0);
        assert_eq!(cluster.boundary(), Boundary::Line);
        assert_eq!(cluster.glyph_len(), 0);
        assert_eq!(cluster.glyphs().count(), 0);
        assert_eq!(run.glyphs().count(), 0);
    }
}
