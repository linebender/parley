use super::layout::{Alignment, Decoration, Glyph, LineMetrics, RunMetrics, Style};
use super::util::*;
use crate::font::Font;
use core::ops::Range;
use swash::shape::Shaper;
use swash::text::cluster::{Boundary, ClusterInfo};
use swash::Synthesis;

#[derive(Copy, Clone)]
pub struct ClusterData {
    pub info: ClusterInfo,
    pub flags: u16,
    pub style_index: u16,
    pub glyph_len: u8,
    pub text_len: u8,
    /// If glyph_len == 0xFF, then glyph_offset is a glyph identifier,
    /// otherwise, it's an offset into the glyph array with the base
    /// taken from the owning run.
    pub glyph_offset: u16,
    pub text_offset: u16,
    pub advance: f32,
}

impl ClusterData {
    pub const LIGATURE_START: u16 = 1;
    pub const LIGATURE_COMPONENT: u16 = 2;

    pub fn is_ligature_start(&self) -> bool {
        self.flags & Self::LIGATURE_START != 0
    }

    pub fn is_ligature_component(&self) -> bool {
        self.flags & Self::LIGATURE_COMPONENT != 0
    }

    pub fn text_range(&self, run: &RunData) -> Range<usize> {
        let start = run.text_range.start + self.text_offset as usize;
        start..start + self.text_len as usize
    }
}

#[derive(Clone)]
pub struct RunData {
    /// Index of the font for the run.
    pub font_index: usize,
    /// Synthesis information for the font.
    pub synthesis: Synthesis,
    /// Range of normalized coordinates in the layout data.
    pub coords_range: Range<usize>,
    /// Range of the source text.
    pub text_range: Range<usize>,
    /// Bidi level for the run.
    pub bidi_level: u8,
    /// True if the run ends with a newline.
    pub ends_with_newline: bool,
    /// Range of clusters.
    pub cluster_range: Range<usize>,
    /// Base for glyph indices.
    pub glyph_start: usize,
    /// Metrics for the run.
    pub metrics: RunMetrics,
    /// Total advance of the run.
    pub advance: f32,
}

#[derive(Clone, Default)]
pub struct LineData {
    /// Range of the source text.
    pub text_range: Range<usize>,
    /// Range of line runs.
    pub run_range: Range<usize>,
    /// Metrics for the line.
    pub metrics: LineMetrics,
    /// Alignment.
    pub alignment: Alignment,
    /// True if the line ends with an explicit break.
    pub explicit_break: bool,
    /// Maximum advance for the line.
    pub max_advance: f32,
}

impl LineData {
    pub fn size(&self) -> f32 {
        self.metrics.ascent + self.metrics.descent + self.metrics.leading
    }
}

#[derive(Clone, Default)]
pub struct LineRunData {
    /// Index of the original run.
    pub run_index: usize,
    /// Bidi level for the run.
    pub bidi_level: u8,
    /// True if the run is composed entirely of whitespace.
    pub is_whitespace: bool,
    /// True if the run ends in whitespace.
    pub has_trailing_whitespace: bool,
    /// Range of the source text.
    pub text_range: Range<usize>,
    /// Range of clusters.
    pub cluster_range: Range<usize>,
}

#[derive(Clone)]
pub struct StyleData<B> {
    pub brush: B,
    pub underline: Option<Decoration<B>>,
    pub strikethrough: Option<Decoration<B>>,
}

#[derive(Clone)]
pub struct LayoutData<B> {
    pub has_bidi: bool,
    pub base_level: u8,
    pub text_len: usize,
    pub fonts: Vec<Font>,
    pub coords: Vec<i16>,
    pub styles: Vec<Style<B>>,
    pub runs: Vec<RunData>,
    pub clusters: Vec<ClusterData>,
    pub glyphs: Vec<Glyph>,
    pub lines: Vec<LineData>,
    pub line_runs: Vec<LineRunData>,
}

impl<B> Default for LayoutData<B> {
    fn default() -> Self {
        Self {
            has_bidi: false,
            base_level: 0,
            text_len: 0,
            fonts: Vec::new(),
            coords: Vec::new(),
            styles: Vec::new(),
            runs: Vec::new(),
            clusters: Vec::new(),
            glyphs: Vec::new(),
            lines: Vec::new(),
            line_runs: Vec::new(),
        }
    }
}

impl<B> LayoutData<B> {
    pub fn clear(&mut self) {
        self.has_bidi = false;
        self.base_level = 0;
        self.text_len = 0;
        self.fonts.clear();
        self.coords.clear();
        self.styles.clear();
        self.runs.clear();
        self.clusters.clear();
        self.glyphs.clear();
        self.lines.clear();
        self.line_runs.clear();
    }

    #[allow(unused_assignments)]
    pub fn push_run(&mut self, font: Font, synthesis: Synthesis, shaper: Shaper, bidi_level: u8) {
        let font_index = self
            .fonts
            .iter()
            .position(|f| *f == font)
            .unwrap_or_else(|| {
                let index = self.fonts.len();
                self.fonts.push(font);
                index
            });
        let metrics = shaper.metrics();
        let cluster_range = self.clusters.len()..self.clusters.len();
        let coords_start = self.coords.len();
        self.coords.extend_from_slice(shaper.normalized_coords());
        let coords_end = self.coords.len();
        let mut run = RunData {
            font_index,
            synthesis,
            coords_range: coords_start..coords_end,
            text_range: 0..0,
            bidi_level,
            ends_with_newline: false,
            cluster_range,
            glyph_start: self.glyphs.len(),
            metrics: RunMetrics {
                ascent: metrics.ascent,
                descent: metrics.descent,
                leading: metrics.leading,
                underline_offset: metrics.underline_offset,
                underline_size: metrics.stroke_size,
                strikethrough_offset: metrics.strikeout_offset,
                strikethrough_size: metrics.stroke_size,
            },
            advance: 0.,
        };
        // Track these so that we can flush if they overflow a u16.
        let mut glyph_count = 0usize;
        let mut text_offset = 0;
        macro_rules! flush_run {
            () => {
                if !run.cluster_range.is_empty() {
                    self.runs.push(run.clone());
                    run.text_range = text_offset..text_offset;
                    run.cluster_range.start = run.cluster_range.end;
                    run.glyph_start = self.glyphs.len();
                    run.advance = 0.;
                    glyph_count = 0;
                }
            };
        }
        let mut first = true;
        shaper.shape_with(|cluster| {
            if cluster.info.boundary() == Boundary::Mandatory {
                // Force break runs at newlines to simplify line breaking.
                run.ends_with_newline = true;
                flush_run!();
            }
            run.ends_with_newline = false;
            const MAX_LEN: usize = u16::MAX as usize;
            let source_range = cluster.source.to_range();
            if first {
                run.text_range = source_range.start..source_range.start;
                text_offset = source_range.start;
                first = false;
            }
            let num_components = cluster.components.len() + 1;
            if glyph_count > MAX_LEN
                || (text_offset - run.text_range.start) > MAX_LEN
                || (num_components > 1
                    && (cluster.components.last().unwrap().start as usize - run.text_range.start)
                        > MAX_LEN)
            {
                flush_run!();
            }
            let text_len = source_range.len();
            let glyph_len = cluster.glyphs.len();
            let advance = cluster.advance();
            run.advance += advance;
            let mut cluster_data = ClusterData {
                info: cluster.info,
                flags: 0,
                style_index: cluster.data as _,
                glyph_len: glyph_len as u8,
                text_len: text_len as u8,
                advance,
                text_offset: (text_offset - run.text_range.start) as u16,
                glyph_offset: 0,
            };
            if num_components > 1 {
                cluster_data.flags = ClusterData::LIGATURE_START;
                cluster_data.advance /= cluster.components.len() as f32;
                cluster_data.text_len = cluster.components[0].to_range().len() as u8;
            }
            macro_rules! push_components {
                () => {
                    self.clusters.push(cluster_data);
                    if num_components > 1 {
                        cluster_data.glyph_offset = 0;
                        cluster_data.glyph_len = 0;
                        for component in &cluster.components[1..] {
                            let range = component.to_range();
                            cluster_data.flags = ClusterData::LIGATURE_COMPONENT;
                            cluster_data.text_offset = (range.start - run.text_range.start) as u16;
                            cluster_data.text_len = range.len() as u8;
                            self.clusters.push(cluster_data);
                            run.cluster_range.end += 1;
                        }
                        cluster_data.flags = 0;
                    }
                };
            }
            run.cluster_range.end += 1;
            run.text_range.end += text_len;
            text_offset += text_len;
            if glyph_len == 1 {
                let g = &cluster.glyphs[0];
                if nearly_zero(g.x) && nearly_zero(g.y) {
                    // Handle the case with a single glyph with zero'd offset.
                    cluster_data.glyph_len = 0xFF;
                    cluster_data.glyph_offset = g.id;
                    push_components!();
                    return;
                }
            } else if glyph_len == 0 {
                // Insert an empty cluster. This occurs for both invisible
                // control characters and ligature components.
                push_components!();
                return;
            }
            // Otherwise, encode all of the glyphs.
            cluster_data.glyph_offset = (self.glyphs.len() - run.glyph_start) as u16;
            self.glyphs.extend(cluster.glyphs.iter().map(|g| Glyph {
                id: g.id,
                style_index: g.data as _,
                x: g.x,
                y: g.y,
                advance: g.advance,
            }));
            glyph_count += glyph_len;
            push_components!();
        });
        flush_run!();
    }
}
