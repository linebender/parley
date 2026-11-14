// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::FontData;
use crate::layout::cluster::Cluster;
use crate::layout::data::{LineItemData, RunData};
use crate::layout::layout::Layout;
use crate::layout::spacing::{Gaps, RunGaps};
use crate::style::Brush;

use core::ops::Range;
use fontique::Synthesis;
use parley_engine::{
    Atom, Atoms, FontMetrics, Glyph, Graphemes, NormalizedCoord, ShapedClusterGlyphs, ShapedRun,
    ShapedSlice,
};

/// Sequence of clusters with a single font and style.
pub struct Run<'a, B: Brush> {
    pub(crate) layout: &'a Layout<B>,
    /// The index of the line this run is part of.
    pub(crate) line_index: u32,
    /// The index of the run within the line it is part of.
    pub(crate) index: u32,
    /// The index of the shaped run within [`parley_engine::ShapedText`].
    pub(crate) shaped_text_run_index: u32,
    pub(crate) shaped: &'a ShapedRun,
    pub(crate) data: &'a RunData,
    pub(crate) line_data: Option<&'a LineItemData>,
}

// `Run` is `Copy` and `Clone` regardless of `B`.
impl<B: Brush> Copy for Run<'_, B> {}
impl<B: Brush> Clone for Run<'_, B> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, B: Brush> Run<'a, B> {
    #[expect(clippy::cast_possible_truncation, reason = "deferred")]
    pub(crate) fn new(
        layout: &'a Layout<B>,
        line_index: u32,
        index: u32,
        run_index: usize,
        line_data: Option<&'a LineItemData>,
    ) -> Self {
        Self {
            layout,
            line_index,
            index,
            shaped_text_run_index: run_index as u32,
            shaped: &layout.data.shaped_text.runs()[run_index],
            data: &layout.data.runs[run_index],
            line_data,
        }
    }

    /// Borrow the shaped content of this run as a slice.
    ///
    /// Note this covers the whole shaped run even when this [`Run`] is scoped to a line. Also see
    /// [`Self::line_slice`].
    pub(crate) fn full_slice(&self) -> ShapedSlice<'a> {
        self.layout
            .data
            .shaped_text
            .run_slice(self.shaped_text_run_index)
    }

    /// Borrow the shaped content of this run as a slice, narrowed to this [`Run`]'s line.
    pub(crate) fn line_slice(&self) -> ShapedSlice<'a> {
        let slice = self.full_slice();
        match self.line_data {
            Some(line_data) => slice.narrow(line_data.shaped_cluster_range.clone()),
            None => slice,
        }
    }

    /// Returns the index of the run within the line.
    pub fn index(&self) -> usize {
        self.index as usize
    }

    /// Returns the font for the run.
    pub fn font(&self) -> &'a FontData {
        &self
            .layout
            .data
            .shaped_text
            .fonts()
            .get(self.shaped.font_index)
            .unwrap()
            .font
    }

    /// Returns the font size for the run.
    pub fn font_size(&self) -> f32 {
        self.shaped.font_size
    }

    /// Returns the font attributes for the run.
    pub fn font_attrs(&self) -> &'a fontique::Attributes {
        &self.data.font_attrs
    }

    /// Returns the synthesis suggestions for the font associated with the run.
    pub fn synthesis(&self) -> Synthesis {
        self.data.synthesis
    }

    /// Returns the normalized variation coordinates for the font associated
    /// with the run.
    pub fn normalized_coords(&self) -> &'a [NormalizedCoord] {
        self.layout
            .data
            .shaped_text
            .normalized_coords()
            .get(self.shaped.normalized_coords_range.clone())
            .unwrap_or(&[])
    }

    /// Returns metrics for the run.
    pub fn font_metrics(&self) -> &'a FontMetrics {
        &self.shaped.font_metrics
    }

    /// Spacing for this run's shaped clusters on its line, or `None` for unspaced text.
    pub(crate) fn gaps(&self) -> Option<RunGaps<'a>> {
        // A Run with no line_data only happens during line breaking, which doesn't call this method.
        // TODO: Maybe use a different data type than run in that case?
        let item = self.line_data.unwrap();
        let start = item.gaps_start? as usize;
        Some(RunGaps::new(
            &self.layout.data.cluster_gaps[start..start + item.shaped_cluster_range.len()],
            item.shaped_cluster_range.start,
        ))
    }

    /// Returns the advance for the run.
    ///
    /// This includes the additional advances inserted for word spacing, letter spacing, and, if the
    /// layout has been [aligned](Layout::set_alignment) with [`Justify`](crate::Alignment::Justify), the
    /// justification.
    pub fn advance(&self) -> f32 {
        let slice = self.line_slice();
        match self.gaps() {
            Some(gaps) => gaps
                .for_cluster_range(slice.shaped_clusters_range())
                .iter()
                .zip(slice.shaped_clusters())
                .map(|(entry, cluster)| cluster.advance + entry.total())
                .sum(),
            None => slice
                .shaped_clusters()
                .iter()
                .map(|cluster| cluster.advance)
                .sum(),
        }
    }

    /// Returns the original text range for the run.
    pub fn text_range(&self) -> Range<usize> {
        self.line_data
            .map(|d| &d.text_range)
            .unwrap_or(&self.shaped.range.byte_range)
            .clone()
    }

    /// Returns `true` if the run has right-to-left directionality.
    pub fn is_rtl(&self) -> bool {
        self.shaped.bidi_level.is_rtl()
    }

    /// Returns the cluster range for the run.
    ///
    /// The indices are grapheme cluster indices, relative to the shaped run this [`Run`] belongs
    /// to: for a run scoped to a line, this is the sub-range of the shaped run's clusters that fall
    /// on that line; otherwise it covers all of the shaped run's clusters.
    ///
    /// Note this counts the shaped run's clusters, so the cost is linear in the shaped run's
    /// length.
    pub fn cluster_range(&self) -> Range<usize> {
        let start = match self.line_data {
            Some(line_data) => count_graphemes(self.full_slice().narrow(
                self.shaped.shaped_clusters_range.start..line_data.shaped_cluster_range.start,
            )),
            None => 0,
        };
        start..start + self.len()
    }

    /// Returns the number of clusters in the run.
    ///
    /// Note this counts the run's clusters, so the cost is linear in the run's length. To check
    /// whether the run has any clusters, use [`Self::is_empty`].
    pub fn len(&self) -> usize {
        count_graphemes(self.line_slice())
    }

    /// Returns `true` if the run is empty.
    pub fn is_empty(&self) -> bool {
        self.line_slice().shaped_clusters_range().is_empty()
    }

    /// Returns the cluster at the specified logical index.
    ///
    /// Note this walks the run's clusters, so the cost is `O(index)`.
    pub fn get(&self, index: usize) -> Option<Cluster<'a, B>> {
        Clusters::new(*self, false).nth(index)
    }

    /// Returns an iterator over the clusters in logical order.
    pub fn clusters(&self) -> impl Iterator<Item = Cluster<'a, B>> + Clone + use<'a, B> {
        Clusters::new(*self, false)
    }

    /// Returns an iterator over the clusters in visual order.
    pub fn visual_clusters(&self) -> impl Iterator<Item = Cluster<'a, B>> + Clone + use<'a, B> {
        Clusters::new(*self, self.is_rtl())
    }

    /// Returns an iterator over the clusters in reverse visual order.
    pub(crate) fn visual_clusters_rev(
        &self,
    ) -> impl Iterator<Item = Cluster<'a, B>> + Clone + use<'a, B> {
        Clusters::new(*self, !self.is_rtl())
    }

    /// The cluster containing the character at `char_index`.
    ///
    /// Returns `None` if `char_index` is not within this run.
    pub(crate) fn cluster_containing_char(&self, char_index: u32) -> Option<Cluster<'a, B>> {
        let atom = self.line_slice().atom_at_char(char_index)?;
        let grapheme = atom
            .graphemes_start()
            .find(|grapheme| grapheme.char_range().contains(&char_index))?;
        Some(Cluster {
            run: *self,
            atom,
            grapheme,
        })
    }

    /// The cluster containing the source text byte at `text_byte`.
    ///
    /// Returns `None` if `text_byte` is not within this run.
    pub(crate) fn cluster_at_text_byte(&self, text_byte: usize) -> Option<Cluster<'a, B>> {
        let slice = self.line_slice();
        let atom = slice.atom_at_text_byte(u32::try_from(text_byte).ok()?)?;
        let grapheme = atom
            .graphemes_start()
            .find(|grapheme| slice.text_byte_range(grapheme.char_range()).end > text_byte)?;
        Some(Cluster {
            run: *self,
            atom,
            grapheme,
        })
    }

    /// An iterator over the glyphs in `shaped_clusters` in visual left-to-right order.
    ///
    /// This includes additional spacing from the stored [`Gaps`].
    ///
    /// `shaped_clusters` must be a subrange of this [`Run`]'s shaped clusters on its line
    /// (indexing into [`ShapedText::shaped_clusters`](parley_engine::ShapedText::shaped_clusters)).
    pub(crate) fn glyphs_in(self, shaped_clusters: Range<u32>) -> Glyphs<'a> {
        let gaps = self
            .gaps()
            .map(|run_gaps| run_gaps.for_cluster_range(shaped_clusters.clone()));
        Glyphs::new(
            self.full_slice().narrow(shaped_clusters),
            self.is_rtl(),
            gaps,
        )
    }
}

/// An iterator over the glyphs of a range of shaped clusters, in visual left-to-right order.
///
/// See [`Run::glyphs_in`].
#[derive(Clone)]
pub(crate) struct Glyphs<'a> {
    /// The slice of shaped text narrowed to the shaped clusters being iterated.
    slice: ShapedSlice<'a>,
    is_rtl: bool,
    /// The gaps of the shaped clusters being iterated, indexed relative to the start of
    /// [`Self::slice`]'s shaped cluster range.
    /// (Semantically, this is the same index space conversion as a [`RunGaps`] constructed
    /// with that index, but using that abstraction would grow this iterator)
    ///
    /// If `None`, the shaped clusters have no additional spacing.
    gaps: Option<&'a [Gaps]>,
    /// The shaped clusters not yet visited. Visited in logical order for LTR text and in reverse
    /// for RTL.
    remaining: Range<u32>,
    /// The not-yet-yielded glyphs of the current shaped cluster.
    glyphs: ShapedClusterGlyphs<'a>,
    /// The current shaped cluster's [`Gaps::before`], if its first glyph hasn't been yielded yet,
    /// otherwise zero.
    shift: f32,
    /// The current shaped cluster's [`Gaps::after`].
    after: f32,
}

impl<'a> Glyphs<'a> {
    fn new(slice: ShapedSlice<'a>, is_rtl: bool, gaps: Option<&'a [Gaps]>) -> Self {
        Self {
            remaining: slice.shaped_clusters_range(),
            slice,
            is_rtl,
            gaps,
            glyphs: ShapedClusterGlyphs::empty(),
            shift: 0.,
            after: 0.,
        }
    }
}

impl Iterator for Glyphs<'_> {
    type Item = Glyph;

    #[inline]
    fn next(&mut self) -> Option<Glyph> {
        loop {
            if let Some(mut glyph) = self.glyphs.next() {
                if self.gaps.is_some() {
                    glyph.x += self.shift;
                    // Also shift subsequent glyphs.
                    glyph.advance += self.shift;
                    self.shift = 0.;
                    if self.glyphs.len() == 0 {
                        glyph.advance += self.after;
                    }
                }
                return Some(glyph);
            }

            let cluster = if self.is_rtl {
                self.remaining.next_back()
            } else {
                self.remaining.next()
            }?;
            self.glyphs = self.slice.shaped_cluster_glyphs(cluster);
            if let Some(cluster_gaps) = self.gaps {
                // TODO: if `self.shift` is non-zero at this point, then there was a shaped cluster without glyphs,
                // and glyph rendering would fall out of sync with layout.
                let index = (cluster - self.slice.shaped_clusters_range().start) as usize;
                // TODO: These gaps are never made visible to the caller if
                // this shaped cluster didn't contain any glyphs.
                // This can make accumulated glyph advances (as in `GlyphRun::positioned_glyphs` disagree
                // with layout, mispositioning subsequent glyphs in the run).
                let gaps = cluster_gaps[index];
                self.shift = gaps.before;
                self.after = gaps.after;
            }
        }
    }
}

/// An iterator over a [`Run`]'s clusters.
///
/// This walks the run's graphemes. Each grapheme is one [`Cluster`].
struct Clusters<'a, B: Brush> {
    run: Run<'a, B>,
    /// Cursor over the run's (line-scoped) atoms.
    atoms: Atoms<'a>,
    /// Grapheme cursor over the run's (line-scoped) slice.
    graphemes: Graphemes<'a>,
    /// The atom containing the most recently yielded grapheme; `None` before the first grapheme.
    atom: Option<Atom<'a>>,
    /// Whether iteration is in reverse logical order.
    rev: bool,
}

impl<'a, B: Brush> Clusters<'a, B> {
    fn new(run: Run<'a, B>, rev: bool) -> Self {
        let slice = run.line_slice();
        Self {
            run,
            atoms: if rev {
                slice.atoms_end()
            } else {
                slice.atoms_start()
            },
            graphemes: if rev {
                slice.graphemes_end()
            } else {
                slice.graphemes_start()
            },
            atom: None,
            rev,
        }
    }
}

impl<B: Brush> Clone for Clusters<'_, B> {
    fn clone(&self) -> Self {
        Self {
            run: self.run,
            atoms: self.atoms,
            graphemes: self.graphemes,
            atom: self.atom,
            rev: self.rev,
        }
    }
}

impl<'a, B: Brush> Iterator for Clusters<'a, B> {
    type Item = Cluster<'a, B>;

    fn next(&mut self) -> Option<Self::Item> {
        let grapheme = if self.rev {
            self.graphemes.prev()?
        } else {
            self.graphemes.next()?
        };
        let entered_new_atom = if self.rev {
            grapheme.is_atom_end()
        } else {
            grapheme.is_atom_start()
        };
        if entered_new_atom {
            self.atom = if self.rev {
                self.atoms.prev()
            } else {
                self.atoms.next()
            };
        }
        let atom = self.atom.expect(
            "The first call to `next` should always be an atom edge, so this should always be set at this point.",
        );
        Some(Cluster {
            run: self.run,
            atom,
            grapheme,
        })
    }
}

/// The number of graphemes in `slice`, also counting a grapheme the slice starts inside of.
///
/// This effectively counts the number of [`Cluster`]s the `slice` would produce.
///
/// This is `O(n)` in the slice's characters.
fn count_graphemes(slice: ShapedSlice<'_>) -> usize {
    slice
        .characters()
        .iter()
        .filter(|character| character.grapheme_start)
        .count()
        + usize::from(slice.grapheme_continues_before())
}
