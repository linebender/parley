// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

impl<'a, B: Brush> Cluster<'a, B> {
    /// Returns the range of text that defines the cluster.
    pub fn text_range(&self) -> Range<usize> {
        self.data.text_range(self.run.data)
    }

    /// Returns the advance of the cluster.
    pub fn advance(&self) -> f32 {
        self.data.advance
    }

    /// Returns true if this is a right-to-left cluster.
    pub fn is_rtl(&self) -> bool {
        self.run.is_rtl()
    }

    /// Returns `true` if the cluster is the beginning of a ligature.
    pub fn is_ligature_start(&self) -> bool {
        self.data.is_ligature_start()
    }

    /// Returns `true` if the cluster is a ligature continuation.
    pub fn is_ligature_continuation(&self) -> bool {
        self.data.is_ligature_component()
    }

    /// Returns `true` if the cluster is a word boundary.
    pub fn is_word_boundary(&self) -> bool {
        self.data.info.is_boundary()
    }

    /// Returns `true` if the cluster is a soft line break.
    pub fn is_soft_line_break(&self) -> bool {
        self.data.info.boundary() == Boundary::Line
    }

    /// Returns `true` if the cluster is a hard line break.
    pub fn is_hard_line_break(&self) -> bool {
        self.data.info.boundary() == Boundary::Mandatory
    }

    /// Returns `true` if the cluster is a space or no-break space.
    pub fn is_space_or_nbsp(&self) -> bool {
        self.data.info.whitespace().is_space_or_nbsp()
    }

    /// Returns an iterator over the glyphs in the cluster.
    pub fn glyphs(&self) -> impl Iterator<Item = Glyph> + 'a + Clone {
        if self.data.glyph_len == 0xFF {
            GlyphIter::Single(Some(Glyph {
                id: self.data.glyph_offset,
                style_index: self.data.style_index,
                x: 0.,
                y: 0.,
                advance: self.data.advance,
            }))
        } else {
            let start = self.run.data.glyph_start + self.data.glyph_offset as usize;
            GlyphIter::Slice(
                self.run.layout.glyphs[start..start + self.data.glyph_len as usize].iter(),
            )
        }
    }

    pub(crate) fn info(&self) -> ClusterInfo {
        self.data.info
    }
}

/// Determines how a cursor attaches to a cluster.
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub enum Affinity {
    /// Left side for LTR clusters and right side for RTL clusters.
    #[default]
    Downstream = 0,
    /// Right side for LTR clusters and left side for RTL clusters.
    Upstream = 1,
}

impl Affinity {
    pub(crate) fn new(is_rtl: bool, is_leading: bool) -> Self {
        match (is_rtl, is_leading) {
            // trailing edge of RTL and leading edge of LTR
            (true, false) | (false, true) => Affinity::Downstream,
            // leading edge of RTL and trailing edge of LTR
            (true, true) | (false, false) => Affinity::Upstream,
        }
    }

    pub fn invert(&self) -> Self {
        match self {
            Self::Downstream => Self::Upstream,
            Self::Upstream => Self::Downstream,
        }
    }

    /// Returns true if the cursor should be placed on the leading edge.
    pub fn is_visually_leading(&self, is_rtl: bool) -> bool {
        match (*self, is_rtl) {
            (Self::Upstream, true) | (Self::Downstream, false) => true,
            (Self::Upstream, false) | (Self::Downstream, true) => false,
        }
    }

    /// Returns true if the cursor should be placed on the trailing edge.
    pub fn is_visually_trailing(&self, is_rtl: bool) -> bool {
        !self.is_visually_leading(is_rtl)
    }
}

/// Index based path to a cluster.
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub struct ClusterPath {
    line_index: u32,
    run_index: u32,
    logical_index: u32,
}

impl ClusterPath {
    /// Returns the path to the cluster for the given layout and byte index.
    pub fn from_byte_index<B: Brush>(layout: &Layout<B>, byte_index: usize) -> Self {
        let mut path = Self::default();
        if let Some((line_index, line)) = layout.line_for_byte_index(byte_index) {
            path.line_index = line_index as u32;
            for (run_index, run) in line.runs().enumerate() {
                path.run_index = run_index as u32;
                if !run.text_range().contains(&byte_index) {
                    continue;
                }
                for (cluster_index, cluster) in run.clusters().enumerate() {
                    path.logical_index = cluster_index as u32;
                    if cluster.text_range().contains(&byte_index) {
                        return path;
                    }
                }
            }
        }
        path
    }

    /// Returns the path to the cluster and the clicked side for the given layout
    /// and point.
    pub fn from_point<B: Brush>(layout: &Layout<B>, x: f32, y: f32) -> (Self, Affinity) {
        let mut path = Self::default();
        if let Some((line_index, line)) = layout.line_for_offset(y) {
            path.line_index = line_index as u32;
            let mut offset = 0.0;
            let last_run_index = line.len().saturating_sub(1);
            for (run_index, run) in line.runs().enumerate() {
                let is_last_run = run_index == last_run_index;
                let run_advance = run.advance();
                path.run_index = run_index as u32;
                path.logical_index = 0;
                if x > offset + run_advance && !is_last_run {
                    offset += run_advance;
                    continue;
                }
                let last_cluster_index = run.cluster_range().len().saturating_sub(1);
                for (visual_index, cluster) in run.visual_clusters().enumerate() {
                    let is_last_cluster = is_last_run && visual_index == last_cluster_index;
                    path.logical_index =
                        run.visual_to_logical(visual_index).unwrap_or_default() as u32;
                    let cluster_advance = cluster.advance();
                    let edge = offset;
                    offset += cluster_advance;
                    if x > offset && !is_last_cluster {
                        continue;
                    }
                    let affinity =
                        Affinity::new(cluster.is_rtl(), x <= edge + cluster_advance * 0.5);
                    return (path, affinity);
                }
            }
        }
        (path, Affinity::default())
    }

    /// Returns the index of the line containing this cluster.
    pub fn line_index(&self) -> usize {
        self.line_index as usize
    }

    /// Returns the index of the run (within the owning line) containing this
    /// cluster.
    pub fn run_index(&self) -> usize {
        self.run_index as usize
    }

    /// Returns the logical index of the cluster within the owning run.
    pub fn logical_index(&self) -> usize {
        self.logical_index as usize
    }

    /// Returns the line for this path and the specified layout.
    pub fn line<'a, B: Brush>(&self, layout: &'a Layout<B>) -> Option<Line<'a, B>> {
        layout.get(self.line_index())
    }

    /// Returns the run for this path and the specified layout.
    pub fn run<'a, B: Brush>(&self, layout: &'a Layout<B>) -> Option<Run<'a, B>> {
        self.line(layout)?.run(self.run_index())
    }

    /// Returns the cluster for this path and the specified layout.
    pub fn cluster<'a, B: Brush>(&self, layout: &'a Layout<B>) -> Option<Cluster<'a, B>> {
        self.run(layout)?.get(self.logical_index())
    }

    /// Returns true if this cluster is at the beginning of a line.
    pub fn is_start_of_line<B: Brush>(&self, layout: &Layout<B>) -> bool {
        self.run_index == 0
            && self
                .run(layout)
                .and_then(|run| run.logical_to_visual(self.logical_index()))
                == Some(0)
    }

    /// Returns true if this cluster is at the end of a line.
    pub fn is_end_of_line<B: Brush>(&self, layout: &Layout<B>) -> bool {
        self.line(layout).map(|line| line.len().saturating_sub(1)) == Some(self.run_index())
            && self
                .run(layout)
                .map(|run| {
                    run.logical_to_visual(self.logical_index())
                        == Some(run.cluster_range().len().saturating_sub(1))
                })
                .unwrap_or_default()
    }

    /// If this cluster, combined with the given affinity, sits on a
    /// directional boundary, returns the cluster that represents the alternate
    /// insertion position.
    ///
    /// For example, if this cluster is a left-to-right cluster, then this
    /// will return the cluster that represents the position where a
    /// right-to-left character would be inserted, and vice versa.
    pub fn bidi_link_cluster<'a, B: Brush>(
        &self,
        layout: &'a Layout<B>,
        affinity: Affinity,
    ) -> Option<(Self, Cluster<'a, B>)> {
        let run = self.run(layout)?;
        let run_end = run.len().checked_sub(1)?;
        let visual_index = run.logical_to_visual(self.logical_index())?;
        let cluster = self.cluster(layout)?;
        let is_rtl = cluster.is_rtl();
        let is_leading = affinity.is_visually_leading(is_rtl);
        let at_start = visual_index == 0 && is_leading;
        let at_end = visual_index == run_end && !is_leading;
        let other_path = if (at_start && !is_rtl) || (at_end && is_rtl) {
            let line = self.line(layout)?;
            let prev_run_index = self.run_index().checked_sub(1)?;
            let prev_run = line.run(prev_run_index)?;
            ClusterPath {
                line_index: self.line_index,
                run_index: prev_run_index as u32,
                logical_index: prev_run.len().checked_sub(1)? as u32,
            }
        } else if (at_end && !is_rtl) || (at_start && is_rtl) {
            ClusterPath {
                line_index: self.line_index,
                run_index: self.run_index() as u32 + 1,
                logical_index: 0,
            }
        } else {
            return None;
        };
        let other_cluster = other_path.cluster(layout)?;
        if other_cluster.is_rtl() == is_rtl {
            return None;
        }
        Some((other_path, other_path.cluster(layout)?))
    }

    /// Returns the path of the cluster that follows this one in visual order.
    pub fn next_visual<B: Brush>(&self, layout: &Layout<B>) -> Option<Self> {
        let line = self.line(layout)?;
        let run = line.run(self.run_index())?;
        let visual_index = run.logical_to_visual(self.logical_index())?;
        if let Some(cluster_index) = run.visual_to_logical(visual_index + 1) {
            // Easy mode: next visual cluster is in the same run
            Some(Self {
                line_index: self.line_index,
                run_index: self.run_index,
                logical_index: cluster_index as u32,
            })
        } else {
            // We just want to find the first line/run following this one that
            // contains any cluster.
            let mut run_index = self.run_index() + 1;
            for line_index in self.line_index()..layout.len() {
                let line = layout.get(line_index)?;
                for run_index in run_index..line.len() {
                    if let Some(run) = line.run(run_index) {
                        if !run.cluster_range().is_empty() {
                            return Some(Self {
                                line_index: line_index as u32,
                                run_index: run_index as u32,
                                logical_index: run.visual_to_logical(0)? as u32,
                            });
                        }
                    }
                }
                // Restart at first run on next line
                run_index = 0;
            }
            None
        }
    }

    pub fn next_visual_cluster<'a, B: Brush>(
        &self,
        layout: &'a Layout<B>,
    ) -> Option<(Self, Cluster<'a, B>)> {
        self.next_visual(layout)
            .and_then(|path| Some((path, path.cluster(layout)?)))
    }

    /// Returns the path of the cluster that follows this one in logical order.
    pub fn next_logical<B: Brush>(&self, layout: &Layout<B>) -> Option<Self> {
        let line = self.line(layout)?;
        let run = line.run(self.run_index())?;
        if self.logical_index() + 1 < run.cluster_range().len() {
            // Easy mode: next cluster is in the same run
            Some(Self {
                line_index: self.line_index,
                run_index: self.run_index,
                logical_index: self.logical_index + 1,
            })
        } else {
            // We just want to find the first line/run following this one that
            // contains any cluster.
            let mut run_index = self.run_index() + 1;
            for line_index in self.line_index()..layout.len() {
                let line = layout.get(line_index)?;
                for run_index in run_index..line.len() {
                    if let Some(run) = line.run(run_index) {
                        if !run.cluster_range().is_empty() {
                            return Some(Self {
                                line_index: line_index as u32,
                                run_index: run_index as u32,
                                logical_index: 0,
                            });
                        }
                    }
                }
                // Restart at first run on next line
                run_index = 0;
            }
            None
        }
    }

    pub fn next_logical_cluster<'a, B: Brush>(
        &self,
        layout: &'a Layout<B>,
    ) -> Option<(Self, Cluster<'a, B>)> {
        self.next_logical(layout)
            .and_then(|path| Some((path, path.cluster(layout)?)))
    }

    /// Returns the path of the cluster that precedes this one in visual order.
    pub fn previous_visual<B: Brush>(&self, layout: &Layout<B>) -> Option<Self> {
        let line = self.line(layout)?;
        let run = line.run(self.run_index())?;
        let visual_index = run.logical_to_visual(self.logical_index())?;
        if let Some(cluster_index) = visual_index
            .checked_sub(1)
            .and_then(|visual_index| run.visual_to_logical(visual_index))
        {
            // Easy mode: previous visual cluster is in the same run
            Some(Self {
                line_index: self.line_index,
                run_index: self.run_index,
                logical_index: cluster_index as u32,
            })
        } else {
            // We just want to find the first line/run preceding this one that
            // contains any cluster.
            let mut run_index = Some(self.run_index());
            for line_index in (0..=self.line_index()).rev() {
                let line = layout.get(line_index)?;
                let first_run = run_index.unwrap_or(line.len());
                for run_index in (0..first_run).rev() {
                    if let Some(run) = line.run(run_index) {
                        let range = run.cluster_range();
                        if !range.is_empty() {
                            return Some(Self {
                                line_index: line_index as u32,
                                run_index: run_index as u32,
                                logical_index: run.visual_to_logical(range.len() - 1)? as u32,
                            });
                        }
                    }
                }
                // Restart at last run
                run_index = None;
            }
            None
        }
    }

    pub fn previous_visual_cluster<'a, B: Brush>(
        &self,
        layout: &'a Layout<B>,
    ) -> Option<(Self, Cluster<'a, B>)> {
        self.previous_visual(layout)
            .and_then(|path| Some((path, path.cluster(layout)?)))
    }

    /// Returns the path of the cluster that precedes this one in logical
    /// order.
    pub fn previous_logical<B: Brush>(&self, layout: &Layout<B>) -> Option<Self> {
        if self.logical_index > 0 {
            // Easy mode: previous cluster is in the same run
            Some(Self {
                line_index: self.line_index,
                run_index: self.run_index,
                logical_index: self.logical_index - 1,
            })
        } else {
            // We just want to find the first line/run preceding this one that
            // contains any cluster.
            let mut run_index = Some(self.run_index());
            for line_index in (0..=self.line_index()).rev() {
                let line = layout.get(line_index)?;
                let first_run = run_index.unwrap_or(line.len());
                for run_index in (0..first_run).rev() {
                    if let Some(run) = line.run(run_index) {
                        let range = run.cluster_range();
                        if !range.is_empty() {
                            return Some(Self {
                                line_index: line_index as u32,
                                run_index: run_index as u32,
                                logical_index: (range.len() - 1) as u32,
                            });
                        }
                    }
                }
                // Restart at last run
                run_index = None;
            }
            None
        }
    }

    pub fn previous_logical_cluster<'a, B: Brush>(
        &self,
        layout: &'a Layout<B>,
    ) -> Option<(Self, Cluster<'a, B>)> {
        self.previous_logical(layout)
            .and_then(|path| Some((path, path.cluster(layout)?)))
    }

    pub fn next_word<B: Brush>(&self, layout: &Layout<B>) -> Option<Self> {
        let line_start = self.line_index();
        let mut run_start = self.run_index();
        let mut cluster_start = self.logical_index() + 1;
        for line_index in line_start..layout.len() {
            let line = layout.get(line_index)?;
            for run_index in run_start..line.len() {
                let run = line.run(run_index)?;
                for cluster_index in cluster_start..run.len() {
                    let cluster = run.get(cluster_index)?;
                    if cluster.is_word_boundary() {
                        return Some(Self {
                            line_index: line_index as u32,
                            run_index: run_index as u32,
                            logical_index: cluster_index as u32,
                        });
                    }
                }
                cluster_start = 0;
            }
            run_start = 0;
        }
        None
    }

    pub fn next_word_cluster<'a, B: Brush>(
        &self,
        layout: &'a Layout<B>,
    ) -> Option<(Self, Cluster<'a, B>)> {
        self.next_word(layout)
            .and_then(|p| Some((p, p.cluster(layout)?)))
    }

    pub fn previous_word<B: Brush>(&self, layout: &Layout<B>) -> Option<Self> {
        let line_start = self.line_index();
        let mut run_start = Some(self.run_index() + 1);
        let mut cluster_start = Some(self.logical_index());
        for line_index in (0..=line_start).rev() {
            let line = layout.get(line_index)?;
            let run_start = run_start.take().unwrap_or(line.len());
            for run_index in (0..run_start).rev() {
                let run = line.run(run_index)?;
                let cluster_start = cluster_start.take().unwrap_or(run.len());
                for cluster_index in (0..cluster_start).rev() {
                    let cluster = run.get(cluster_index)?;
                    if cluster.is_word_boundary() {
                        return Some(Self {
                            line_index: line_index as u32,
                            run_index: run_index as u32,
                            logical_index: cluster_index as u32,
                        });
                    }
                }
            }
        }
        None
    }

    pub fn previous_word_cluster<'a, B: Brush>(
        &self,
        layout: &'a Layout<B>,
    ) -> Option<(Self, Cluster<'a, B>)> {
        self.previous_word(layout)
            .and_then(|p| Some((p, p.cluster(layout)?)))
    }

    /// Returns the visual offset of this cluster along direction of text flow.
    ///
    /// This cost of this function is roughly linear in the number of clusters
    /// on the containing line.
    pub fn visual_offset<B: Brush>(&self, layout: &Layout<B>) -> Option<f32> {
        let line = self.line(layout)?;
        let mut offset = 0.0;
        for run_index in 0..=self.run_index() {
            let run = line.run(run_index)?;
            if run_index != self.run_index() {
                offset += run.advance();
            } else {
                let visual_index = run.logical_to_visual(self.logical_index())?;
                for cluster in run.visual_clusters().take(visual_index) {
                    offset += cluster.advance();
                }
            }
        }
        Some(offset)
    }
}

#[derive(Clone)]
enum GlyphIter<'a> {
    Single(Option<Glyph>),
    Slice(core::slice::Iter<'a, Glyph>),
}

impl<'a> Iterator for GlyphIter<'a> {
    type Item = Glyph;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Single(glyph) => glyph.take(),
            Self::Slice(iter) => {
                let glyph = *iter.next()?;
                Some(glyph)
            }
        }
    }
}
