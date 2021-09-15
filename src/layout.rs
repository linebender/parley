//! Layout types.

use super::data::*;
use super::font::Font;
use super::style::Brush;
use core::ops::Range;
use swash::text::cluster::{Boundary, ClusterInfo};
use swash::{GlyphId, NormalizedCoord, Synthesis};

pub use super::line::BreakLines;

/// Alignment of a layout.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Alignment {
    Start,
    Middle,
    End,
}

impl Default for Alignment {
    fn default() -> Self {
        Self::Start
    }
}

/// Text layout.
#[derive(Clone)]
pub struct Layout<B: Brush> {
    pub(crate) data: LayoutData<B>,
}

impl<B: Brush> Layout<B> {
    /// Creates an empty layout.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the style collection for the layout.
    pub fn styles(&self) -> &[Style<B>] {
        &self.data.styles
    }

    /// Returns the number of lines in the layout.
    pub fn len(&self) -> usize {
        self.data.lines.len()
    }

    /// Returns true if the layout is empty.
    pub fn is_empty(&self) -> bool {
        self.data.lines.is_empty()
    }

    /// Returns the line at the specified index.
    pub fn get(&self, index: usize) -> Option<Line<B>> {
        Some(Line {
            layout: &self.data,
            data: self.data.lines.get(index)?,
        })
    }

    /// Returns an iterator over the lines in the layout.
    pub fn lines(&self) -> impl Iterator<Item = Line<B>> + '_ + Clone {
        self.data.lines.iter().map(move |data| Line {
            layout: &self.data,
            data,
        })
    }

    /// Returns line breaker to compute lines for the layout.
    pub fn break_lines(&mut self) -> BreakLines<B> {
        BreakLines::new(&mut self.data)
    }

    /// Breaks all lines with the specified maximum advance and alignment.
    pub fn break_all_lines(&mut self, max_advance: Option<f32>, alignment: Alignment) {
        self.break_lines()
            .break_remaining(max_advance.unwrap_or(f32::MAX), alignment)
    }

    /// Returns the information about the layout at the specified point.
    pub fn hit_test_point(&self, mut x: f32, y: f32) -> HitTestResult {
        let mut result = HitTestResult::default();
        result.is_inside = x >= 0. && y >= 0.;
        let last_line = self.data.lines.len().saturating_sub(1);
        for (line_index, line) in self.lines().enumerate() {
            let line_metrics = line.metrics();
            if y <= line_metrics.baseline || line_index == last_line {
                if y > line_metrics.baseline + line_metrics.leading * 0.5 {
                    result.is_inside = false;
                    x = f32::MAX;
                } else if y < 0. {
                    x = 0.;
                }
                result.baseline = line_metrics.baseline;
                result.line_index = line_index;
                let mut last_edge = line_metrics.offset;
                for (run_index, run) in line.runs().enumerate() {
                    result.run_index = run_index;
                    let cluster_range = run.data().cluster_range.clone();
                    for (cluster_index, cluster) in run.visual_clusters().enumerate() {
                        result.text_range = cluster.text_range();
                        if run.is_rtl() {
                            result.cluster_index = cluster_range.end - cluster_index - 1;
                        } else {
                            result.cluster_index = cluster_index;
                        }
                        let advance = cluster.advance();
                        if x >= last_edge {
                            let far_edge = last_edge + advance;
                            if x < far_edge {
                                result.is_leading = false;
                                let middle = (last_edge + far_edge) * 0.5;
                                if x <= middle {
                                    result.is_leading = true;
                                    result.offset = last_edge;
                                } else {
                                    result.is_leading = false;
                                    result.offset = far_edge;
                                }
                                return result;
                            }
                            last_edge = far_edge;
                        } else {
                            result.is_inside = false;
                            result.is_leading = true;
                            result.offset = line_metrics.offset;
                            return result;
                        }
                    }
                }
                break;
            }
        }
        result
    }

    /// Returns information about the layout at the specified text position.
    pub fn hit_test_position(&self, mut position: usize) -> HitTestResult {
        let mut result = HitTestResult::default();
        result.is_leading = true;
        result.is_inside = true;
        if position >= self.data.text_len {
            result.is_inside = false;
            result.is_leading = false;
            position = self.data.text_len.saturating_sub(1);
        }
        let last_line = self.data.lines.len().saturating_sub(1);
        for (line_index, line) in self.lines().enumerate() {
            let line_metrics = line.metrics();
            result.baseline = line_metrics.baseline;
            result.line_index = line_index;
            if !line.text_range().contains(&position) && line_index != last_line {
                continue;
            }
            let mut last_edge = line_metrics.offset;
            result.offset = last_edge;
            for (run_index, run) in line.runs().enumerate() {
                result.run_index = run_index;
                if !run.text_range().contains(&position) {
                    continue;
                }
                let cluster_range = run.data().cluster_range.clone();
                for (cluster_index, cluster) in run.visual_clusters().enumerate() {
                    result.text_range = cluster.text_range();
                    result.offset = last_edge;
                    if run.is_rtl() {
                        result.cluster_index = cluster_range.end - cluster_index - 1;
                    } else {
                        result.cluster_index = cluster_index;
                    }
                    let advance = cluster.advance();
                    if result.text_range.contains(&position) {
                        if !result.is_inside {
                            result.offset += advance;
                        }
                        return result;
                    }
                    last_edge += advance;
                }
            }
            result.offset = last_edge;
            break;
        }
        result.is_leading = false;
        result.is_inside = false;
        result
    }

    /// Returns an iterator over the runs in the layout.
    pub fn runs(&self) -> impl Iterator<Item = Run<B>> + '_ + Clone {
        self.data.runs.iter().map(move |data| Run {
            layout: &self.data,
            data,
            line_data: None,
        })
    }
}

impl<B: Brush> Default for Layout<B> {
    fn default() -> Self {
        Self {
            data: Default::default(),
        }
    }
}

/// Sequence of clusters with a single font and style.
#[derive(Copy, Clone)]
pub struct Run<'a, B: Brush> {
    layout: &'a LayoutData<B>,
    data: &'a RunData,
    line_data: Option<&'a LineRunData>,
}

impl<'a, B: Brush> Run<'a, B> {
    pub(crate) fn new(
        layout: &'a LayoutData<B>,
        data: &'a RunData,
        line_data: Option<&'a LineRunData>,
    ) -> Self {
        Self {
            layout,
            data,
            line_data,
        }
    }

    /// Returns the font for the run.
    pub fn font(&self) -> &Font {
        self.layout.fonts.get(self.data.font_index).unwrap()
    }

    /// Returns the font size for the run.
    pub fn font_size(&self) -> f32 {
        self.data.font_size
    }

    /// Returns the synthesis suggestions for the font associated with the run.
    pub fn synthesis(&self) -> Synthesis {
        self.data.synthesis
    }

    /// Returns the normalized variation coordinates for the font associated
    /// with the run.
    pub fn normalized_coords(&self) -> &[NormalizedCoord] {
        self.layout
            .coords
            .get(self.data.coords_range.clone())
            .unwrap_or(&[])
    }

    /// Returns metrics for the run.
    pub fn metrics(&self) -> &RunMetrics {
        &self.data.metrics
    }

    /// Returns the original text range for the run.
    pub fn text_range(&self) -> Range<usize> {
        self.line_data
            .map(|d| &d.text_range)
            .unwrap_or(&self.data.text_range)
            .clone()
    }

    /// Returns true if the run has right-to-left directionality.
    pub fn is_rtl(&self) -> bool {
        self.data.bidi_level & 1 != 0
    }

    /// Returns the number of clusters in the run.
    pub fn len(&self) -> usize {
        self.line_data
            .map(|d| &d.cluster_range)
            .unwrap_or(&self.data.cluster_range)
            .len()
    }

    /// Returns true if the run is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the cluster at the specified index.
    pub fn get(&'a self, index: usize) -> Option<Cluster<'a, B>> {
        let range = self
            .line_data
            .map(|d| &d.cluster_range)
            .unwrap_or(&self.data.cluster_range);
        let index = range.start + index;
        Some(Cluster {
            run: self,
            data: self.layout.clusters.get(index)?,
        })
    }

    /// Returns an iterator over the clusters in logical order.
    pub fn clusters(&'a self) -> impl Iterator<Item = Cluster<'a, B>> + 'a + Clone {
        let range = self
            .line_data
            .map(|d| &d.cluster_range)
            .unwrap_or(&self.data.cluster_range)
            .clone();
        Clusters {
            run: self,
            range,
            rev: false,
        }
    }

    /// Returns an iterator over the clusters in visual order.
    pub fn visual_clusters(&'a self) -> impl Iterator<Item = Cluster<'a, B>> + 'a + Clone {
        let range = self
            .line_data
            .map(|d| &d.cluster_range)
            .unwrap_or(&self.data.cluster_range)
            .clone();
        Clusters {
            run: self,
            range,
            rev: self.is_rtl(),
        }
    }

    pub(crate) fn data(&self) -> &'a RunData {
        self.data
    }
}

struct Clusters<'a, B: Brush> {
    run: &'a Run<'a, B>,
    range: Range<usize>,
    rev: bool,
}

impl<'a, B: Brush> Clone for Clusters<'a, B> {
    fn clone(&self) -> Self {
        Self {
            run: self.run,
            range: self.range.clone(),
            rev: self.rev,
        }
    }
}

impl<'a, B: Brush> Iterator for Clusters<'a, B> {
    type Item = Cluster<'a, B>;

    fn next(&mut self) -> Option<Self::Item> {
        let index = if self.rev {
            self.range.next_back()?
        } else {
            self.range.next()?
        };
        Some(Cluster {
            run: self.run,
            data: self.run.layout.clusters.get(index)?,
        })
    }
}

/// Metrics information for a run.
#[derive(Copy, Clone, Default, Debug)]
pub struct RunMetrics {
    /// Typographic ascent.
    pub ascent: f32,
    /// Typographic descent.
    pub descent: f32,
    /// Typographic leading.
    pub leading: f32,
    /// Offset of the top of underline decoration from the baseline.
    pub underline_offset: f32,
    /// Thickness of the underline decoration.
    pub underline_size: f32,
    /// Offset of the top of underline decoration from the baseline.
    pub strikethrough_offset: f32,
    /// Thickness of the underline decoration.
    pub strikethrough_size: f32,
}

/// Atomic unit of text.
#[derive(Copy, Clone)]
pub struct Cluster<'a, B: Brush> {
    run: &'a Run<'a, B>,
    data: &'a ClusterData,
}

impl<'a, B: Brush> Cluster<'a, B> {
    /// Returns the range of text that defines the cluster.
    pub fn text_range(&self) -> Range<usize> {
        self.data.text_range(self.run.data)
    }

    /// Returns the advance of the cluster.
    pub fn advance(&self) -> f32 {
        self.data.advance
    }

    /// Returns true if the cluster is the beginning of a ligature.
    pub fn is_ligature_start(&self) -> bool {
        self.data.is_ligature_start()
    }

    /// Returns true if the cluster is a ligature continuation.
    pub fn is_ligature_continuation(&self) -> bool {
        self.data.is_ligature_component()
    }

    /// Returns true if the cluster is a word boundary.
    pub fn is_word_boundary(&self) -> bool {
        self.data.info.is_boundary()
    }

    /// Returns true if the cluster is a soft line break.
    pub fn is_soft_line_break(&self) -> bool {
        self.data.info.boundary() == Boundary::Line
    }

    /// Returns true if the cluster is a hard line break.
    pub fn is_hard_line_break(&self) -> bool {
        self.data.info.boundary() == Boundary::Mandatory
    }

    /// Returns true if the cluster is a space or no-break space.
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

/// Glyph with an offset and advance.
#[derive(Copy, Clone, Default, Debug)]
pub struct Glyph {
    pub id: GlyphId,
    pub style_index: u16,
    pub x: f32,
    pub y: f32,
    pub advance: f32,
}

impl Glyph {
    /// Returns the index into the layout style collection.
    pub fn style_index(&self) -> usize {
        self.style_index as usize
    }
}

/// Line in a text layout.
#[derive(Copy, Clone)]
pub struct Line<'a, B: Brush> {
    layout: &'a LayoutData<B>,
    data: &'a LineData,
}

impl<'a, B: Brush> Line<'a, B> {
    /// Returns the metrics for the line.
    pub fn metrics(&self) -> &LineMetrics {
        &self.data.metrics
    }

    /// Returns the range of text for the line.
    pub fn text_range(&self) -> Range<usize> {
        self.data.text_range.clone()
    }

    /// Returns the number of runs in the line.
    pub fn len(&self) -> usize {
        self.data.run_range.len()
    }

    /// Returns true if the line is empty.
    pub fn is_empty(&self) -> bool {
        self.data.run_range.is_empty()
    }

    /// Returns the run at the specified index.
    pub fn get(&self, index: usize) -> Option<Run<'a, B>> {
        let index = self.data.run_range.start + index;
        let line_data = self.layout.line_runs.get(index)?;
        Some(Run {
            layout: self.layout,
            data: self.layout.runs.get(line_data.run_index)?,
            line_data: Some(line_data),
        })
    }

    /// Returns an iterator over the runs for the line.
    pub fn runs(&self) -> impl Iterator<Item = Run<'a, B>> + 'a + Clone {
        let copy = self.clone();
        let line_runs = &copy.layout.line_runs[self.data.run_range.clone()];
        line_runs.iter().map(move |line_data| Run {
            layout: copy.layout,
            data: &copy.layout.runs[line_data.run_index],
            line_data: Some(line_data),
        })
    }

    /// Returns an iterator over the glyph runs for the line.
    pub fn glyph_runs(&self) -> impl Iterator<Item = GlyphRun<'a, B>> + 'a + Clone {
        GlyphRunIter {
            line: self.clone(),
            run_index: 0,
            glyph_start: 0,
            offset: 0.,
        }
    }
}

/// Metrics information for a line.
#[derive(Copy, Clone, Default, Debug)]
pub struct LineMetrics {
    /// Typographic ascent.
    pub ascent: f32,
    /// Typographic descent.
    pub descent: f32,
    /// Typographic leading.
    pub leading: f32,
    /// Offset to the baseline.
    pub baseline: f32,
    /// Offset for alignment.
    pub offset: f32,
    /// Full advance of the line.
    pub advance: f32,
    /// Advance of trailing whitespace.
    pub trailing_whitespace: f32,
}

impl LineMetrics {
    /// Returns the size of the line (ascent + descent + leading).
    pub fn size(&self) -> f32 {
        self.ascent + self.descent + self.leading
    }
}

/// Style properties.
#[derive(Clone, Debug)]
pub struct Style<B: Brush> {
    /// Brush for drawing glyphs.
    pub brush: B,
    /// Underline decoration.
    pub underline: Option<Decoration<B>>,
    /// Strikethrough decoration.
    pub strikethrough: Option<Decoration<B>>,
}

/// Underline or strikethrough decoration.
#[derive(Clone, Debug)]
pub struct Decoration<B: Brush> {
    /// Brush used to draw the decoration.
    pub brush: B,
    /// Offset of the decoration from the baseline. If `None`, use the metrics
    /// of the containing run.
    pub offset: Option<f32>,
    /// Thickness of the decoration. If `None`, use the metrics of the
    /// containing run.
    pub size: Option<f32>,
}

/// Result of testing a point or text position in a layout.
#[derive(Clone, Default, Debug)]
pub struct HitTestResult {
    /// Baseline metric of the caret position.
    pub baseline: f32,
    /// Offset along the baseline of the caret position.
    pub offset: f32,
    /// Index of the containing line.
    pub line_index: usize,
    /// Index of the run within the containing line.
    pub run_index: usize,
    /// Index of the cluster within the containing run.
    pub cluster_index: usize,
    /// Source text range of the cluster.
    pub text_range: Range<usize>,
    /// True if the hit was on the leading edge of a cluster.
    pub is_leading: bool,
    /// True if the hit was inside the layout bounds.
    pub is_inside: bool,
}

/// Sequence of fully positioned glyphs with the same style.
#[derive(Clone)]
pub struct GlyphRun<'a, B: Brush> {
    run: Run<'a, B>,
    style: &'a Style<B>,
    glyph_start: usize,
    glyph_count: usize,
    offset: f32,
    baseline: f32,
}

impl<'a, B: Brush> GlyphRun<'a, B> {
    /// Returns the underlying run.
    pub fn run(&self) -> &Run<'a, B> {
        &self.run
    }

    /// Returns the associated style.
    pub fn style(&self) -> &Style<B> {
        self.style
    }

    /// Returns an iterator over fully positioned glyphs in the run.
    pub fn glyphs(&'a self) -> impl Iterator<Item = Glyph> + 'a + Clone {
        let mut offset = self.offset;
        let baseline = self.baseline;
        let glyphs = self
            .run
            .visual_clusters()
            .map(|cluster| cluster.glyphs())
            .flatten();
        glyphs
            .skip(self.glyph_start)
            .take(self.glyph_count)
            .map(move |mut g| {
                g.x += offset;
                g.y += baseline;
                offset += g.advance;
                g
            })
    }
}

#[derive(Clone)]
struct GlyphRunIter<'a, B: Brush> {
    line: Line<'a, B>,
    run_index: usize,
    glyph_start: usize,
    offset: f32,
}

impl<'a, B: Brush> Iterator for GlyphRunIter<'a, B> {
    type Item = GlyphRun<'a, B>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let run = self.line.get(self.run_index)?;
            let mut iter = run
                .visual_clusters()
                .map(|c| c.glyphs())
                .flatten()
                .skip(self.glyph_start);
            if let Some(first) = iter.next() {
                let mut advance = first.advance;
                let style_index = first.style_index();
                let mut glyph_count = 1;
                for glyph in iter.take_while(|g| g.style_index() == style_index) {
                    glyph_count += 1;
                    advance += glyph.advance;
                }
                let style = run.layout.styles.get(style_index)?;
                let glyph_start = self.glyph_start;
                self.glyph_start += glyph_count;
                let offset = self.offset;
                self.offset += advance;
                return Some(GlyphRun {
                    run,
                    style,
                    glyph_start,
                    glyph_count,
                    offset: offset + self.line.data.metrics.offset,
                    baseline: self.line.data.metrics.baseline,
                });
            }
            self.run_index += 1;
            self.glyph_start = 0;
        }
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
