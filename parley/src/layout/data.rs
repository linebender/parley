// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::inline_box::InlineBox;
use crate::layout::{ContentWidths, Glyph, LineMetrics, RunMetrics, Style};
use crate::style::Brush;
use crate::{FontData, IndentOptions, InlineBoxKind, LineHeight, OverflowWrap, TextWrapMode};
use core::ops::Range;

use alloc::vec::Vec;

use crate::analysis::{Boundary, CharInfo};
use parley_core::Whitespace;

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct ClusterData {
    pub(crate) info: ClusterInfo,
    /// Cluster flags (see impl methods for details).
    pub(crate) flags: u16,
    /// Style index for this cluster.
    pub(crate) style_index: u16,
    /// Number of glyphs in this cluster (0xFF = single glyph stored inline)
    pub(crate) glyph_len: u8,
    /// Number of text bytes in this cluster
    pub(crate) text_len: u8,
    /// If `glyph_len == 0xFF`, then `glyph_offset` is a glyph identifier,
    /// otherwise, it's an offset into the glyph array with the base
    /// taken from the owning run.
    pub(crate) glyph_offset: u32,
    /// Offset into the text for this cluster
    pub(crate) text_offset: u16,
    /// Advance width for this cluster
    pub(crate) advance: f32,
}

impl ClusterData {
    pub(crate) const LIGATURE_START: u16 = 1;
    pub(crate) const LIGATURE_COMPONENT: u16 = 2;

    #[inline(always)]
    pub(crate) fn is_ligature_start(self) -> bool {
        self.flags & Self::LIGATURE_START != 0
    }

    #[inline(always)]
    pub(crate) fn is_ligature_component(self) -> bool {
        self.flags & Self::LIGATURE_COMPONENT != 0
    }

    #[inline(always)]
    pub(crate) fn text_range(self, run: &RunData) -> Range<usize> {
        let start = run.text_range.start + self.text_offset as usize;
        start..start + self.text_len as usize
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct ClusterInfo {
    boundary: Boundary,
    source_char: char,
}

impl ClusterInfo {
    pub(crate) fn new(boundary: Boundary, source_char: char) -> Self {
        Self {
            boundary,
            source_char,
        }
    }

    // Returns the boundary type of the cluster.
    pub(crate) fn boundary(self) -> Boundary {
        self.boundary
    }

    // Returns the whitespace type of the cluster.
    pub(crate) fn whitespace(self) -> Whitespace {
        to_whitespace(self.source_char)
    }

    /// Returns if the cluster is a line boundary.
    pub(crate) fn is_boundary(self) -> bool {
        self.boundary != Boundary::None
    }

    /// Returns if the cluster is an emoji.
    pub(crate) fn is_emoji(self) -> bool {
        // TODO: Defer to ICU4X properties (see: https://docs.rs/icu/latest/icu/properties/props/struct.Emoji.html).
        matches!(self.source_char as u32, 0x1F600..=0x1F64F | 0x1F300..=0x1F5FF | 0x1F680..=0x1F6FF | 0x2600..=0x26FF | 0x2700..=0x27BF)
    }

    /// Returns if the cluster is any whitespace.
    pub(crate) fn is_whitespace(self) -> bool {
        self.source_char.is_whitespace()
    }

    /// Returns the cluster's original character.
    pub(crate) fn source_char(self) -> char {
        self.source_char
    }
}

const fn to_whitespace(c: char) -> Whitespace {
    const LINE_SEPARATOR: char = '\u{2028}';
    const PARAGRAPH_SEPARATOR: char = '\u{2029}';

    match c {
        ' ' => Whitespace::Space,
        '\t' => Whitespace::Tab,
        '\n' | '\r' | LINE_SEPARATOR | PARAGRAPH_SEPARATOR => Whitespace::Newline,
        '\u{00A0}' => Whitespace::NoBreakSpace,
        _ => Whitespace::None,
    }
}

/// `HarfRust`-based run data
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RunData {
    /// Index of the font for the run.
    pub(crate) font_index: usize,
    /// Font size.
    pub(crate) font_size: f32,
    /// Font attributes, needed for accessibility.
    pub(crate) font_attrs: fontique::Attributes,
    /// Synthesis for rendering (contains variation settings)
    pub(crate) synthesis: fontique::Synthesis,
    /// Range of normalized coordinates in the layout data.
    pub(crate) coords_range: Range<usize>,
    /// Range of the source text.
    pub(crate) text_range: Range<usize>,
    /// Bidi level for the run.
    pub(crate) bidi_level: u8,
    /// Range of clusters.
    pub(crate) cluster_range: Range<usize>,
    /// Base for glyph indices.
    pub(crate) glyph_start: usize,
    /// Metrics for the run.
    pub(crate) metrics: RunMetrics,
    /// Total advance of the run.
    pub(crate) advance: f32,
}

#[derive(Copy, Clone, Default, PartialEq, Debug)]
pub enum BreakReason {
    #[default]
    None,
    Regular,
    Explicit,
    Emergency,
}

#[derive(Clone, Default, Debug, PartialEq)]
pub(crate) struct LineData {
    /// Range of the source text.
    pub(crate) text_range: Range<usize>,
    /// Range of line items.
    pub(crate) item_range: Range<usize>,
    /// Metrics for the line.
    pub(crate) metrics: LineMetrics,
    /// The cause of the line break.
    pub(crate) break_reason: BreakReason,
    /// Maximum advance for the line.
    pub(crate) max_advance: f32,
    /// Number of justified clusters on the line.
    pub(crate) num_spaces: usize,
    /// Text indent applied to this line.
    pub(crate) indent: f32,
}

impl LineData {
    pub(crate) fn size(&self) -> f32 {
        self.metrics.ascent + self.metrics.descent + self.metrics.leading
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LineItemData {
    /// Whether the item is a run or an inline box
    pub(crate) kind: LayoutItemKind,
    /// The index of the run or inline box in the runs or `inline_boxes` vec
    pub(crate) index: usize,
    /// Bidi level for the item (used for reordering)
    pub(crate) bidi_level: u8,
    /// Advance (size in direction of text flow) for the run.
    pub(crate) advance: f32,

    // Fields that only apply to text runs (Ignored for boxes)
    // TODO: factor this out?
    /// True if the run is composed entirely of whitespace.
    pub(crate) is_whitespace: bool,
    /// True if the run ends in whitespace.
    pub(crate) has_trailing_whitespace: bool,
    /// Range of the source text.
    pub(crate) text_range: Range<usize>,
    /// Range of clusters.
    pub(crate) cluster_range: Range<usize>,
}

impl LineItemData {
    pub(crate) fn is_text_run(&self) -> bool {
        self.kind == LayoutItemKind::TextRun
    }

    #[inline(always)]
    pub(crate) fn is_rtl(&self) -> bool {
        self.bidi_level & 1 != 0
    }

    /// If the item is a text run
    ///   - Determine if it consists entirely of whitespace (`is_whitespace` property)
    ///   - Determine if it has trailing whitespace (`has_trailing_whitespace` property)
    pub(crate) fn compute_whitespace_properties<B: Brush>(&mut self, layout_data: &LayoutData<B>) {
        // Skip items which are not text runs
        if self.kind != LayoutItemKind::TextRun {
            return;
        }

        self.is_whitespace = true;
        if self.is_rtl() {
            // RTL runs check for "trailing" whitespace at the front.
            for cluster in layout_data.clusters[self.cluster_range.clone()].iter() {
                if cluster.info.is_whitespace() {
                    self.has_trailing_whitespace = true;
                } else {
                    self.is_whitespace = false;
                    break;
                }
            }
        } else {
            for cluster in layout_data.clusters[self.cluster_range.clone()]
                .iter()
                .rev()
            {
                if cluster.info.is_whitespace() {
                    self.has_trailing_whitespace = true;
                } else {
                    self.is_whitespace = false;
                    break;
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LayoutItemKind {
    TextRun,
    InlineBox,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LayoutItem {
    /// Whether the item is a run or an inline box
    pub(crate) kind: LayoutItemKind,
    /// The index of the run or inline box in the runs or `inline_boxes` vec
    pub(crate) index: usize,
    /// Bidi level for the item (used for reordering)
    pub(crate) bidi_level: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LayoutData<B: Brush> {
    // General settings (directly from the "builder")
    /// The display scale factor
    pub(crate) scale: f32,
    /// Whether metrics should be quantized to pixel boundaries
    pub(crate) quantize: bool,
    /// The `BiDi` base level
    pub(crate) base_level: u8,
    /// The length of the text in the layout
    pub(crate) text_len: usize,

    // Output of style resolution (input to line breaking)
    pub(crate) styles: Vec<Style<B>>,
    pub(crate) inline_boxes: Vec<InlineBox>,

    // Output of shaping (input to line breaking)
    pub(crate) fonts: Vec<FontData>,
    pub(crate) coords: Vec<i16>,
    pub(crate) runs: Vec<RunData>,
    pub(crate) items: Vec<LayoutItem>,
    pub(crate) clusters: Vec<ClusterData>,
    pub(crate) glyphs: Vec<Glyph>,

    // Output of line breaking
    /// The lines in the
    pub(crate) lines: Vec<LineData>,
    /// Items within each line
    pub(crate) line_items: Vec<LineItemData>,
    /// The width constraint that was used to line break the layout
    pub(crate) layout_max_advance: f32,
    /// The computed width of the layout excluding trailing whitespace
    pub(crate) width: f32,
    /// The computed width of the layout including trailing whitespace
    pub(crate) full_width: f32,
    /// The computed height of the layout
    pub(crate) height: f32,

    // Output of alignment
    #[cfg(feature = "accesskit")]
    /// Directly store the alignment if accessibility is enabled so we can
    /// set the corresponding AccessKit property.
    pub(crate) alignment: Option<super::Alignment>,
    /// Whether the layout is aligned with [`crate::Alignment::Justify`].
    pub(crate) is_aligned_justified: bool,
    /// The text-indent amount in layout units.
    pub(crate) indent_amount: f32,
    /// Options controlling text-indent behavior (each-line, hanging).
    pub(crate) indent_options: IndentOptions,
}

impl<B: Brush> Default for LayoutData<B> {
    fn default() -> Self {
        Self {
            scale: 1.,
            quantize: true,
            base_level: 0,
            text_len: 0,
            width: 0.,
            full_width: 0.,
            height: 0.,
            fonts: Vec::new(),
            coords: Vec::new(),
            styles: Vec::new(),
            inline_boxes: Vec::new(),
            runs: Vec::new(),
            items: Vec::new(),
            clusters: Vec::new(),
            glyphs: Vec::new(),
            lines: Vec::new(),
            line_items: Vec::new(),
            #[cfg(feature = "accesskit")]
            alignment: None,
            is_aligned_justified: false,
            layout_max_advance: 0.0,
            indent_amount: 0.0,
            indent_options: IndentOptions::default(),
        }
    }
}

impl<B: Brush> LayoutData<B> {
    pub(crate) fn clear(&mut self) {
        self.scale = 1.;
        self.quantize = true;
        self.base_level = 0;
        self.text_len = 0;
        self.width = 0.;
        self.full_width = 0.;
        self.height = 0.;
        self.fonts.clear();
        self.coords.clear();
        self.styles.clear();
        self.inline_boxes.clear();
        self.runs.clear();
        self.items.clear();
        self.clusters.clear();
        self.glyphs.clear();
        self.lines.clear();
        self.line_items.clear();
    }

    /// Push an inline box to the list of items
    pub(crate) fn push_inline_box(&mut self, index: usize) {
        // Give the box the same bidi level as the preceding text run
        // (or else default to 0 if there is not yet a text run)
        let bidi_level = self.runs.last().map(|r| r.bidi_level).unwrap_or(0);

        self.items.push(LayoutItem {
            kind: LayoutItemKind::InlineBox,
            index,
            bidi_level,
        });
    }
    /// Translates one shaped run from [`parley_core`] into this layout's run, cluster and glyph
    /// storage.
    ///
    /// Each [`parley_core::Run`] is a sequence of clusters with a single font, orientation, etc.;
    /// call this function once per run in logical order. `parley` layers three pieces of data on
    /// top:
    /// - the style index per cluster and glyph;
    /// - the line height, from the run's representative style applied to the run's own font metrics; and
    /// - the source char of each cluster.
    pub(crate) fn push_shaped_run(
        &mut self,
        run: &parley_core::Run<'_>,
        text: &str,
        infos: &[(CharInfo, u16)],
        char_start: usize,
        line_height_style_index: u16,
    ) {
        if run.is_empty() {
            return;
        }
        let bidi_level = run.bidi_level();
        let font_size = run.font_size();

        // Intern the run's font.
        let font = run.font().expect("a text run has a font");
        let font_index = self
            .fonts
            .iter()
            .position(|f| f == font)
            .unwrap_or_else(|| {
                self.fonts.push(font.clone());
                self.fonts.len() - 1
            });

        // Carry the run's variation coordinates over.
        let coords_start = self.coords.len();
        self.coords.extend_from_slice(run.normalized_coords());
        let coords_end = self.coords.len();

        // Copy the metrics and layer line height on top. Line height uses the representative
        // style's policy but the run's own metrics.
        let metrics = run.metrics();
        let line_height = match self.styles[line_height_style_index as usize].line_height {
            LineHeight::Absolute(value) => value,
            LineHeight::FontSizeRelative(value) => value * font_size,
            LineHeight::MetricsRelative(value) => {
                (metrics.ascent + metrics.descent + metrics.leading) * value
            }
        };
        let metrics = RunMetrics {
            ascent: metrics.ascent,
            descent: metrics.descent,
            leading: metrics.leading,
            underline_offset: metrics.underline_offset,
            underline_size: metrics.underline_size,
            strikethrough_offset: metrics.strikethrough_offset,
            strikethrough_size: metrics.strikethrough_size,
            line_height,
            x_height: metrics.x_height,
            cap_height: metrics.cap_height,
        };

        let run_text_start = run.text_range().start;
        let glyph_start = self.glyphs.len();
        let cluster_start = self.clusters.len();

        // Clusters tile the run's source characters one-to-one in logical order (a ligature is a
        // start cluster plus one zero-glyph component per extra character), so the per-character
        // style index advances by one cluster at a time.
        for (char_index, cluster) in (char_start..).zip(run.clusters()) {
            let style_index = infos[char_index].1;

            let cluster_text = cluster.text_range();
            let source_char = text[cluster_text.clone()].chars().next().unwrap_or(' ');
            let text_offset = (cluster_text.start - run_text_start) as u16;
            let text_len = (cluster_text.end - cluster_text.start) as u8;

            let mut flags = 0_u16;
            if cluster.is_ligature_start() {
                flags |= ClusterData::LIGATURE_START;
            }
            if cluster.is_ligature_continuation() {
                flags |= ClusterData::LIGATURE_COMPONENT;
            }

            // A regular cluster (everything but ligatures and newlines) with a single glyph is
            // stored inline; everything else goes to the glyph array, and zero-glyph clusters
            // (newlines, ligature components) carry only an advance.
            let is_regular = flags == 0 && cluster.whitespace() != Whitespace::Newline;
            let mut glyphs = cluster.glyphs();
            let glyph_count = glyphs.len();
            let (glyph_len, glyph_offset) = if glyph_count == 0 {
                (0_u8, 0_u32)
            } else if is_regular && glyph_count == 1 {
                let glyph = glyphs.next().unwrap();
                if glyph.x == 0.0 && glyph.y == 0.0 {
                    (0xFF_u8, glyph.id)
                } else {
                    let offset = (self.glyphs.len() - glyph_start) as u32;
                    self.glyphs.push(Glyph {
                        id: glyph.id,
                        style_index,
                        x: glyph.x,
                        y: glyph.y,
                        advance: glyph.advance,
                    });
                    (1_u8, offset)
                }
            } else {
                let offset = (self.glyphs.len() - glyph_start) as u32;
                for glyph in glyphs {
                    self.glyphs.push(Glyph {
                        id: glyph.id,
                        style_index,
                        x: glyph.x,
                        y: glyph.y,
                        advance: glyph.advance,
                    });
                }
                (glyph_count as u8, offset)
            };

            self.clusters.push(ClusterData {
                info: ClusterInfo::new(cluster.boundary(), source_char),
                flags,
                style_index,
                glyph_len,
                text_len,
                glyph_offset,
                text_offset,
                advance: cluster.advance(),
            });
        }

        self.runs.push(RunData {
            font_index,
            font_size,
            font_attrs: run.font_attrs(),
            synthesis: run.synthesis(),
            coords_range: coords_start..coords_end,
            text_range: run.text_range(),
            bidi_level,
            cluster_range: cluster_start..self.clusters.len(),
            glyph_start,
            metrics,
            advance: run.advance(),
        });
        self.items.push(LayoutItem {
            kind: LayoutItemKind::TextRun,
            index: self.runs.len() - 1,
            bidi_level,
        });
    }

    // TODO: this method does not handle mixed direction text at all.
    pub(crate) fn calculate_content_widths(&self) -> ContentWidths {
        fn whitespace_advance(cluster: Option<&ClusterData>) -> f32 {
            cluster
                .filter(|cluster| cluster.info.whitespace().is_space_or_nbsp())
                .map_or(0.0, |cluster| cluster.advance)
        }

        let mut min_width = 0.0_f32;
        let mut max_width = 0.0_f32;

        let mut running_min_width = 0.0;
        let mut running_max_width = 0.0;
        let mut text_wrap_mode = TextWrapMode::Wrap;
        let mut prev_cluster: Option<&ClusterData> = None;
        let is_rtl = self.base_level & 1 == 1;
        for item in &self.items {
            match item.kind {
                LayoutItemKind::TextRun => {
                    let run = &self.runs[item.index];
                    let clusters = &self.clusters[run.cluster_range.clone()];
                    if is_rtl {
                        prev_cluster = clusters.first();
                    }
                    for cluster in clusters {
                        let boundary = cluster.info.boundary();
                        let style = &self.styles[cluster.style_index as usize];
                        let prev_text_wrap_mode = text_wrap_mode;
                        text_wrap_mode = style.text_wrap_mode;
                        if boundary == Boundary::Mandatory
                            || (prev_text_wrap_mode == TextWrapMode::Wrap
                                && (boundary == Boundary::Line
                                    || style.overflow_wrap == OverflowWrap::Anywhere))
                        {
                            let trailing_whitespace = whitespace_advance(prev_cluster);
                            min_width = min_width.max(running_min_width - trailing_whitespace);
                            running_min_width = 0.0;
                            if boundary == Boundary::Mandatory {
                                max_width = max_width.max(running_max_width - trailing_whitespace);
                                running_max_width = 0.0;
                            }
                        }
                        running_min_width += cluster.advance;
                        running_max_width += cluster.advance;
                        if !is_rtl {
                            prev_cluster = Some(cluster);
                        }
                    }
                    let trailing_whitespace = whitespace_advance(prev_cluster);
                    min_width = min_width.max(running_min_width - trailing_whitespace);
                }
                LayoutItemKind::InlineBox => {
                    let ibox = &self.inline_boxes[item.index];
                    if ibox.kind == InlineBoxKind::InFlow {
                        running_max_width += ibox.width;
                        if text_wrap_mode == TextWrapMode::Wrap {
                            let trailing_whitespace = whitespace_advance(prev_cluster);
                            min_width = min_width.max(running_min_width - trailing_whitespace);
                            min_width = min_width.max(ibox.width);
                            running_min_width = 0.0;
                        } else {
                            running_min_width += ibox.width;
                        }
                    }
                    prev_cluster = None;
                }
            }
            let trailing_whitespace = whitespace_advance(prev_cluster);
            max_width = max_width.max(running_max_width - trailing_whitespace);
        }

        let trailing_whitespace = whitespace_advance(prev_cluster);
        min_width = min_width.max(running_min_width - trailing_whitespace);

        ContentWidths {
            min: min_width,
            max: max_width,
        }
    }
}
