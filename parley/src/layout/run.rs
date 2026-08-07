// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::layout::cluster::{Cluster, ClusterPath};
use crate::layout::data::{LineItemData, RunData, count_graphemes};
use crate::layout::layout::Layout;
use crate::layout::spacing::LineSpacing;
use crate::style::Brush;

use core::ops::Range;
use fontique::Synthesis;
use parley_engine::{
    Atom, Atoms, FontInstance, FontMetrics, Glyph, Graphemes, NormalizedCoord, ShapedRun,
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
    pub fn font(&self) -> &FontInstance {
        self.layout
            .data
            .shaped_text
            .fonts()
            .get(self.shaped.font_index)
            .unwrap()
    }

    /// Returns the font size for the run.
    pub fn font_size(&self) -> f32 {
        self.shaped.font_size
    }

    /// Returns the font attributes for the run.
    pub fn font_attrs(&self) -> &fontique::Attributes {
        &self.data.font_attrs
    }

    /// Returns the synthesis suggestions for the font associated with the run.
    pub fn synthesis(&self) -> Synthesis {
        self.data.synthesis
    }

    /// Returns the normalized variation coordinates for the font associated
    /// with the run.
    pub fn normalized_coords(&self) -> &[NormalizedCoord] {
        self.layout
            .data
            .shaped_text
            .normalized_coords()
            .get(self.shaped.normalized_coords_range.clone())
            .unwrap_or(&[])
    }

    /// Returns metrics for the run.
    pub fn font_metrics(&self) -> &FontMetrics {
        &self.shaped.font_metrics
    }

    /// This run's line height.
    pub fn line_height(&self) -> f32 {
        self.data.line_height
    }

    #[inline]
    pub(crate) fn line_spacing(&self) -> LineSpacing {
        let spacing = LineSpacing::new(self.data.spacing);
        if self.line_data.is_some() {
            // If `line_data` is `Some`, this run is scoped to a line. Add its justification.
            spacing
                .with_justification(self.layout.data.lines[self.line_index as usize].justification)
        } else {
            spacing
        }
    }

    /// Returns the advance for the run.
    ///
    /// This includes the additional advance inserted between the run's atoms.
    pub fn advance(&self) -> f32 {
        let spacing = self.line_spacing();
        spacing.slice_advance(self.line_slice())
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
    pub fn cluster_range(&self) -> Range<usize> {
        self.line_data
            .map(|d| d.grapheme_range.clone())
            .unwrap_or_else(|| 0..count_graphemes(self.full_slice()))
    }

    /// Returns the number of clusters in the run.
    pub fn len(&self) -> usize {
        self.cluster_range().len()
    }

    /// Returns `true` if the run is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the cluster at the specified index.
    ///
    /// Note this walks the run's clusters, so the cost is `O(index)`.
    pub fn get(&self, index: usize) -> Option<Cluster<'a, B>> {
        Clusters::new(*self, false).nth(index)
    }

    /// Returns an iterator over the clusters in logical order.
    pub fn clusters(&self) -> impl Iterator<Item = Cluster<'a, B>> + Clone + use<'a, B> {
        Clusters::new(*self, false)
    }

    /// Returns the visual cluster index for the specified logical cluster index.
    pub fn logical_to_visual(&self, logical_index: usize) -> Option<usize> {
        let num_clusters = self.len();
        if logical_index >= num_clusters {
            return None;
        }

        let visual_index = if self.is_rtl() {
            num_clusters - 1 - logical_index
        } else {
            logical_index
        };

        Some(visual_index)
    }

    /// Returns the logical cluster index for the specified visual cluster index.
    pub fn visual_to_logical(&self, visual_index: usize) -> Option<usize> {
        let num_clusters = self.len();
        if visual_index >= num_clusters {
            return None;
        }

        let logical_index = if self.is_rtl() {
            num_clusters - 1 - visual_index
        } else {
            visual_index
        };

        Some(logical_index)
    }

    /// Returns an iterator over the clusters in visual order.
    pub fn visual_clusters(&self) -> impl Iterator<Item = Cluster<'a, B>> + Clone + use<'a, B> {
        Clusters::new(*self, self.is_rtl())
    }

    /// An iterator over the glyphs in `clusters` in visual left-to-right order.
    ///
    /// This includes additional spacing from [`LineSpacing`].
    pub(crate) fn glyphs_in(
        self,
        clusters: Range<u32>,
    ) -> impl Iterator<Item = Glyph> + Clone + use<'a, B> {
        let spacing = self.line_spacing();

        return spacing
            .is_zero()
            .then(|| glyphs_without_spacing(self, clusters.clone()))
            .into_iter()
            .flatten()
            .chain(
                (!spacing.is_zero())
                    .then(|| glyphs_with_spacing(self, clusters, spacing))
                    .into_iter()
                    .flatten(),
            );

        fn glyphs_without_spacing<'a, B: Brush>(
            run: Run<'a, B>,
            clusters: Range<u32>,
        ) -> impl Iterator<Item = Glyph> + Clone + use<'a, B> {
            let slice = run
                .layout
                .data
                .shaped_text
                .run_slice(run.shaped_text_run_index);

            (!run.is_rtl())
                .then(|| clusters.clone())
                .into_iter()
                .flatten()
                // we chain because `.rev()` wraps the iterator in `Rev` - a different type than the LTR
                // iterator.
                .chain((run.is_rtl()).then(|| clusters.rev()).into_iter().flatten())
                .flat_map(move |cluster_idx| slice.shaped_cluster_glyphs(cluster_idx))
        }

        fn glyphs_with_spacing<'a, B: Brush>(
            run: Run<'a, B>,
            clusters: Range<u32>,
            spacing: LineSpacing,
        ) -> impl Iterator<Item = Glyph> + Clone + use<'a, B> {
            let slice = run
                .layout
                .data
                .shaped_text
                .run_slice(run.shaped_text_run_index)
                .narrow(clusters);

            let is_rtl = run.is_rtl();
            (!is_rtl)
                .then(|| slice.atoms_start())
                .into_iter()
                .flatten()
                // we chain because `.rev()` wraps the iterator in `Rev` - a different type than the LTR
                // iterator.
                .chain(
                    is_rtl
                        .then(|| slice.atoms_end().rev())
                        .into_iter()
                        .flatten(),
                )
                .flat_map(move |atom| {
                    let gaps = spacing.gaps(&atom);
                    let glyph_count: usize = atom
                        .shaped_clusters()
                        .iter()
                        .map(|cluster| usize::from(cluster.glyph_len()))
                        .sum();

                    let atom_clusters = atom.shaped_clusters_range();
                    (!is_rtl)
                        .then(|| atom_clusters.clone())
                        .into_iter()
                        .flatten()
                        .chain(is_rtl.then(|| atom_clusters.rev()).into_iter().flatten())
                        .flat_map(move |cluster_idx| slice.shaped_cluster_glyphs(cluster_idx))
                        .enumerate()
                        .map(move |(glyph_idx, mut glyph)| {
                            if glyph_idx == 0 {
                                glyph.x += gaps.before;
                                glyph.advance += gaps.before;
                            }
                            if glyph_idx + 1 == glyph_count {
                                glyph.advance += gaps.after;
                            }
                            glyph
                        })
                })
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
    /// In forward iteration, the logical index of the cluster yielded next; in reverse
    /// iteration, one past that index.
    logical_index: usize,
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
            logical_index: if rev { run.len() } else { 0 },
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
            logical_index: self.logical_index,
            rev: self.rev,
        }
    }
}

impl<'a, B: Brush> Iterator for Clusters<'a, B> {
    type Item = Cluster<'a, B>;

    #[expect(clippy::cast_possible_truncation, reason = "deferred")]
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
        let logical_index = if self.rev {
            self.logical_index -= 1;
            self.logical_index
        } else {
            let index = self.logical_index;
            self.logical_index += 1;
            index
        };
        Some(Cluster {
            path: ClusterPath::new(self.run.line_index, self.run.index, logical_index as u32),
            run: self.run,
            atom,
            grapheme,
        })
    }
}
