// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Font-dependent shaping: turning runs into positioned glyphs.
//!
//! Given a run of constant script, direction, font, and style (see
//! [`crate::Analysis::itemize_with`]), [`ShapeContext::shape_run`] produces glyphs, advances and
//! the cluster<->text mapping, resulting in [`ShapedText`].
//!
//! To reshape, e.g. breaking into or joining lines, call [`ShapeContext::apply_break`] or
//! [`ShapeContext::apply_concat`], which reshapes the regions on each side if necessary.

mod build;
mod cache;
mod lru_cache;
mod select;

use alloc::vec::Vec;
use core::ops::Range;

use fontique::{Attributes, Blob, Charmap, FallbackKey, Query, QueryFont, QueryStatus, Synthesis};
use harfrust::{
    BufferFlags, Direction, ShapeOptions, ShapePlan, ShaperData, ShaperInstance, UnicodeBuffer,
};
use icu_segmenter::GraphemeClusterSegmenter;
use linebender_resource_handle::FontData;
use parlance::{FontFeature, FontVariation, Language, Script};

use crate::analysis::{AnalysisDataSources, CharInfo};
use crate::analyzer::Analysis;
use crate::common::{NormalizedCoord, RunMetrics, RunOrientation};
use crate::convert::script_to_harfrust;
use crate::shaped_text::{ClusterData, Glyph, RunData, RunKind, ShapedText};
use crate::util::reuse_vec;

use cache::{ShapeDataKey, ShapeInstanceId, ShapeInstanceKey, ShapePlanId, ShapePlanKey};
use lru_cache::LruCache;

/// Maximum number of distinct entries kept in each shaping cache (parsed font tables,
/// variable-font instances and shape plans).
const SHAPE_CACHE_SIZE: usize = 16;

/// Variable-font axis configuration for a font run, in either of `harfrust`'s two forms: either
/// user-space variations (with costly normalization, so the resulting instance is cached) or
/// pre-normalized coordinates (used directly).
enum InstanceSource<'a> {
    Variations {
        synthesis: &'a Synthesis,
        variations: &'a [FontVariation],
    },
    Coords(&'a [NormalizedCoord]),
}

/// Reusable shaping resources.
///
/// Create one and keep it alive across paragraphs: expensive parts of shaper setup are cached.
pub struct ShapeContext {
    shape_data_cache: LruCache<ShapeDataKey, ShaperData>,
    shape_instance_cache: LruCache<ShapeInstanceId, ShaperInstance>,
    shape_plan_cache: LruCache<ShapePlanId, ShapePlan>,
    clusters: Vec<ClusterData>,
    glyphs: Vec<Glyph>,
    coords: Vec<NormalizedCoord>,

    /// Scratch buffers reused across shaping calls.
    unicode_buffer: Option<UnicodeBuffer>,
    features: Vec<harfrust::Feature>,
    char_cluster: select::CharCluster,
    font_runs: Vec<(Range<usize>, QueryFont)>,
    font_candidates: Vec<QueryFont>,
    charmaps: Vec<Option<Charmap<'static>>>,
    char_offsets: Vec<(usize, char)>,
}

impl Default for ShapeContext {
    fn default() -> Self {
        Self {
            shape_data_cache: LruCache::new(SHAPE_CACHE_SIZE),
            shape_instance_cache: LruCache::new(SHAPE_CACHE_SIZE),
            shape_plan_cache: LruCache::new(SHAPE_CACHE_SIZE),
            clusters: Vec::new(),
            glyphs: Vec::new(),
            coords: Vec::new(),

            unicode_buffer: Some(UnicodeBuffer::new()),
            features: Vec::new(),
            char_cluster: select::CharCluster::default(),
            font_runs: Vec::new(),
            font_candidates: Vec::new(),
            charmaps: Vec::new(),
            char_offsets: Vec::new(),
        }
    }
}

impl core::fmt::Debug for ShapeContext {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ShapeContext").finish_non_exhaustive()
    }
}

impl ShapeContext {
    /// Creates a new shaping context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Shapes one run, appending the result to `out`.
    ///
    /// May append more than one [`Run`](crate::Run), because fonts may not be able to map a
    /// character, in which case the run gets split and shaped against various fonts in `query`.
    pub fn shape_run(
        &mut self,
        input: &ShapeInput<'_>,
        query: &mut Query<'_>,
        out: &mut ShapedText,
    ) {
        let run_text = &input.text[input.text_range.clone()];
        if run_text.is_empty() {
            return;
        }
        let char_infos = input.analysis.char_infos();
        let run_char_infos = &char_infos[input.char_range.clone()];

        // The caller owns the family list; we add the script/locale fallbacks and the requested
        // attributes (so faux bold/oblique synthesis is computed).
        query.set_attributes(input.attributes);
        query.set_fallbacks(FallbackKey::new(input.script, input.language.as_ref()));

        // Split the run into maximal single-font runs by selecting a font per grapheme cluster.
        // These font runs tile the run text contiguously. (The `core::mem::take` is a scratch
        // buffer borrow-checker dance.)
        let mut font_runs = core::mem::take(&mut self.font_runs);
        self.select_font_runs(run_text, run_char_infos, query, &mut font_runs);

        let params = FontRunParams {
            font_size: input.font_size,
            script: input.script,
            language: input.language,
            level: input.level,
            orientation: input.orientation,
            features: input.features,
            letter_spacing: input.letter_spacing,
            word_spacing: input.word_spacing,
        };

        let mut char_cursor = 0;
        for (range, font) in &font_runs {
            let font_run_text = &run_text[range.clone()];
            let font_run_char_count = font_run_text.chars().count();
            let font_run_char_infos =
                &run_char_infos[char_cursor..char_cursor + font_run_char_count];
            char_cursor += font_run_char_count;

            let Some((advance, metrics)) = self.shape_font_run(
                &font.blob,
                font.index,
                InstanceSource::Variations {
                    synthesis: &font.synthesis,
                    variations: input.variations,
                },
                &params,
                font_run_text,
                font_run_char_infos,
            ) else {
                continue;
            };
            if self.clusters.is_empty() {
                continue;
            }

            let font_index = out.intern_font(FontData::new(font.blob.clone(), font.index));
            let text_start = input.text_range.start + range.start;
            let run = RunData {
                font_index,
                font_size: input.font_size,
                synthesis: font.synthesis,
                font_attrs: input.attributes,
                coords_range: 0..0,
                features_range: 0..0,
                text_range: text_start..text_start + font_run_text.len(),
                cluster_range: 0..0,
                glyph_start: 0,
                script: input.script,
                language: input.language,
                bidi_level: input.level,
                orientation: input.orientation,
                metrics,
                advance,
                letter_spacing: input.letter_spacing,
                word_spacing: input.word_spacing,
                kind: RunKind::Text,
                inline_box_id: 0,
            };
            out.append_run(
                run,
                &self.clusters,
                &self.glyphs,
                &self.coords,
                input.features,
            );
        }

        // Put the font-run scratch back to reuse its allocation.
        self.font_runs = font_runs;
    }

    /// Reshapes a sub-range of an already-shaped run as an isolated fragment.
    ///
    /// The shaper sees only the fragment, so cursive joins and ligatures are severed at its outer
    /// boundaries and shape freely within.
    ///
    /// `text_range` must lie within a single run and begin and end on cluster boundaries. You can
    /// use the ranges from [`ShapedText::unsafe_break_region`] or
    /// [`ShapedText::unsafe_concat_region`].
    pub fn reshape_fragment(
        &mut self,
        text: &str,
        analysis: &Analysis,
        shaped: &mut ShapedText,
        text_range: Range<usize>,
        query: &mut Query<'_>,
    ) {
        // Re-shaping reuses the run's own font, so no fallback query is needed.
        let _ = query;
        let Some(target) = shaped.reshape_locate(text_range.clone()) else {
            return;
        };

        let fragment_text = &text[text_range.clone()];
        let fragment_char_start = text[..text_range.start].chars().count();
        let fragment_char_count = fragment_text.chars().count();
        let fragment_char_infos =
            &analysis.char_infos()[fragment_char_start..fragment_char_start + fragment_char_count];

        // Read the run's shaping parameters in place rather than snapshotting them:
        // `shape_font_run` only writes into `self`, so this immutable borrow of `shaped`
        // ends before the `splice_fragment` below takes it mutably.
        let Some(run) = shaped.run(target.run_index) else {
            return;
        };
        let Some(font) = run.font() else {
            // Inline-box runs carry no font and are never reshaped.
            return;
        };
        let params = FontRunParams {
            font_size: run.font_size(),
            script: run.script(),
            language: run.language(),
            level: run.bidi_level(),
            orientation: run.orientation(),
            features: run.features(),
            letter_spacing: run.letter_spacing(),
            word_spacing: run.word_spacing(),
        };
        let result = self.shape_font_run(
            &font.data,
            font.index,
            InstanceSource::Coords(run.normalized_coords()),
            &params,
            fragment_text,
            fragment_char_infos,
        );
        if result.is_none() {
            return;
        }
        shaped.splice_fragment(&target, &self.clusters, &self.glyphs);
    }

    /// Commits a line break at byte offset `pos`, reshaping the bounded region on each side so
    /// cursive joins are severed.
    ///
    /// Undo this with [`Self::apply_concat`].
    ///
    /// This queries [`ShapedText::unsafe_break_region`] and reshapes each side as necessary. It is
    /// a no-op when `pos` is a break-safe boundary. If the break severs a cursive join or splits a
    /// ligature (e.g. a hyphenation falling inside a Latin `fi` ligature), both sides are reshaped.
    pub fn apply_break(
        &mut self,
        text: &str,
        analysis: &Analysis,
        shaped: &mut ShapedText,
        pos: usize,
        query: &mut Query<'_>,
    ) {
        let ranges = shaped.unsafe_break_region(pos);
        if !ranges.tail.is_empty() {
            self.reshape_fragment(text, analysis, shaped, ranges.tail, query);
        }
        if !ranges.head.is_empty() {
            self.reshape_fragment(text, analysis, shaped, ranges.head, query);
        }
    }

    /// Merges two adjacent shaped fragments meeting at byte offset `pos`, reshaping the bounded
    /// region as one fragment so that cursive joins and ligatures are formed across the seam.
    ///
    /// This is the reverse of [`Self::apply_break`].
    ///
    /// This queries [`ShapedText::unsafe_concat_region`] and reshapes each side as necessary. It
    /// is a no-op when the boundary at `pos` is already safe-to-concat.
    pub fn apply_concat(
        &mut self,
        text: &str,
        analysis: &Analysis,
        shaped: &mut ShapedText,
        pos: usize,
        query: &mut Query<'_>,
    ) {
        let region = shaped.unsafe_concat_region(pos);
        if !region.is_empty() {
            self.reshape_fragment(text, analysis, shaped, region, query);
        }
    }

    /// Splits `run_text` into maximal runs of clusters that select the same font, appending into
    /// `font_runs`.
    fn select_font_runs(
        &mut self,
        run_text: &str,
        run_char_infos: &[CharInfo],
        query: &mut Query<'_>,
        font_runs: &mut Vec<(Range<usize>, QueryFont)>,
    ) {
        font_runs.clear();
        let analysis_data_sources = AnalysisDataSources::new();

        // We lazily parse the `Charmap`s used in this run and then cache them (otherwise we
        // reparse them for every single grapheme cluster in this run, which is wasteful). Because
        // of the lifetime on `Charmap` that means we need to collect the fonts used in this run
        // beforehand. This is probably fine, because the fonts will usually be in fontique's
        // cache, but if there are large amounts of uncached fonts, and we don't end up actually
        // using them, then we unnecessarily load them here.
        //
        // TODO: This probably needs revisiting such that we don't need to load the fonts
        // beforehand, and ideally the cached `Charmap`s also survive over a single call to
        // `select_font_runs`.
        self.font_candidates.clear();
        query.matches_with(|font| {
            self.font_candidates.push(font.clone());
            QueryStatus::Continue
        });
        let mut charmaps: Vec<Option<Charmap<'_>>> = reuse_vec(core::mem::take(&mut self.charmaps));
        charmaps.extend(core::iter::repeat_with(|| None).take(self.font_candidates.len()));

        let mut char_cursor = 0;
        let mut boundaries = GraphemeClusterSegmenter::new().segment_str(run_text);
        let mut grapheme_start = boundaries.next().unwrap_or(0);
        let mut font_run_start = grapheme_start;
        let mut current_font: Option<QueryFont> = None;

        for grapheme_end in boundaries {
            let grapheme = &run_text[grapheme_start..grapheme_end];
            self.char_cluster
                .fill(grapheme, &mut char_cursor, run_char_infos);
            if let Some(index) = select::select_font(
                &self.font_candidates,
                &mut charmaps,
                &mut self.char_cluster,
                &analysis_data_sources,
            ) {
                let font = &self.font_candidates[index];
                match &current_font {
                    // No font yet: this is the first; it absorbs any leading
                    // graphemes that found no font at all.
                    None => current_font = Some(font.clone()),
                    Some(current) if select::same_font(font, current) => {}
                    Some(_) => {
                        font_runs.push((
                            font_run_start..grapheme_start,
                            current_font.replace(font.clone()).unwrap(),
                        ));
                        font_run_start = grapheme_start;
                    }
                }
            }
            // A grapheme with no font is absorbed into the current font run.
            grapheme_start = grapheme_end;
        }
        self.charmaps = reuse_vec(charmaps);

        if let Some(font) = current_font {
            font_runs.push((font_run_start..run_text.len(), font));
        }
    }

    /// Shapes `font_run_text` with one font.
    ///
    /// Fills [`Self::clusters`], [`Self::glyphs`] and [`Self::coords`], returning the font run
    /// advance and run metrics. Returns `None` if the font cannot be read.
    fn shape_font_run(
        &mut self,
        blob: &Blob<u8>,
        index: u32,
        instance_source: InstanceSource<'_>,
        params: &FontRunParams<'_>,
        font_run_text: &str,
        font_run_char_infos: &[CharInfo],
    ) -> Option<(f32, RunMetrics)> {
        let font_ref = harfrust::FontRef::from_index(blob.as_ref(), index).ok()?;
        let blob_id = blob.id();

        // Resolve the variable-font instance. With variations this runs the `avar` normalization
        // (costly!), so it is cached by font + synthesis + variations. The coords form is cheaper
        // because of the already-normalized coords.
        let coords_instance;
        let instance: &ShaperInstance = match instance_source {
            InstanceSource::Variations {
                synthesis,
                variations,
            } => self.shape_instance_cache.entry(
                ShapeInstanceKey::new(blob_id, index, synthesis, variations),
                || {
                    ShaperInstance::from_variations(
                        &font_ref,
                        synthesis
                            .variation_settings()
                            .iter()
                            .map(|(tag, value)| harfrust::Variation {
                                tag: *tag,
                                value: *value,
                            })
                            .chain(variations.iter().map(|variation| harfrust::Variation {
                                tag: harfrust::Tag::new(&variation.tag.to_bytes()),
                                value: variation.value,
                            })),
                    )
                },
            ),
            InstanceSource::Coords(coords) => {
                coords_instance = ShaperInstance::from_coords(
                    &font_ref,
                    coords
                        .iter()
                        .map(|&c| harfrust::NormalizedCoord::from_bits(c)),
                );
                &coords_instance
            }
        };

        self.features.clear();
        for feature in params.features {
            self.features.push(harfrust::Feature::new(
                harfrust::Tag::new(&feature.tag.to_bytes()),
                u32::from(feature.value),
                ..,
            ));
        }

        // `Upright` runs shape along the vertical axis: `harfrust` applies the font's vertical
        // typesetting features (`vert`/`vrt2`/`vkrn`). `Horizontal` and `Sideways` both shape
        // horizontally (`Sideways` is rotated afterwards), so their direction is based on bidi.
        let direction = if params.orientation.is_vertical_shaping() {
            Direction::TopToBottom
        } else if params.level & 1 != 0 {
            Direction::RightToLeft
        } else {
            Direction::LeftToRight
        };

        let script = script_to_harfrust(params.script);
        let language = params
            .language
            .and_then(|lang| lang.language().parse::<harfrust::Language>().ok());

        let mut buffer = self.unicode_buffer.take().unwrap_or_default();
        buffer.clear();
        buffer.reserve(font_run_text.len());
        // The cluster value is the font-run-relative character index, so the glyph builder can
        // index `font_run_char_infos` and count ligature components.
        for (char_index, ch) in font_run_text.chars().enumerate() {
            buffer.add(ch, char_index as u32);
        }
        buffer.set_direction(direction);
        buffer.set_script(script);
        if let Some(language) = &language {
            buffer.set_language(language.clone());
        }
        // Ask for the unsafe-to-concat and tatweel flags, which are not produced by default.
        buffer.set_flags(
            BufferFlags::PRODUCE_UNSAFE_TO_CONCAT | BufferFlags::PRODUCE_SAFE_TO_INSERT_TATWEEL,
        );

        let data = self
            .shape_data_cache
            .entry(ShapeDataKey::new(blob_id, index), || {
                ShaperData::new(&font_ref)
            });
        let shaper = data.shaper(&font_ref).instance(Some(instance)).build();

        // Retain the resolved normalized coords for the run, and key the shape plan on them.
        self.coords.clear();
        self.coords
            .extend(shaper.coords().iter().map(|coord| coord.to_bits()));

        // Compiling the plan is costly; cache it so runs sharing a font, direction, script,
        // language, feature and variation set reuse one plan.
        let plan = self.shape_plan_cache.entry(
            ShapePlanKey::new(
                blob_id,
                index,
                &self.coords,
                direction,
                script,
                language.as_ref(),
                &self.features,
            ),
            || {
                ShapePlan::new(
                    &shaper,
                    direction,
                    Some(script),
                    language.as_ref(),
                    &self.features,
                )
            },
        );
        let shaper = data.shaper(&font_ref).instance(Some(instance)).build();
        let glyph_buffer = shaper.shape(
            buffer,
            ShapeOptions::new()
                .features(&self.features)
                .plan(Some(plan))
                .point_size(Some(params.font_size)),
        );

        let units_per_em = shaper.units_per_em() as f32;
        let scale = if units_per_em == 0.0 {
            0.0
        } else {
            params.font_size / units_per_em
        };
        let metrics = compute_metrics(blob.as_ref(), index, params.font_size, shaper.coords());

        self.char_offsets.clear();
        self.char_offsets.extend(font_run_text.char_indices());
        self.clusters.clear();
        self.glyphs.clear();
        let mut advance = build::build_clusters(
            &glyph_buffer,
            direction == Direction::RightToLeft,
            direction == Direction::TopToBottom,
            scale,
            &self.char_offsets,
            font_run_char_infos,
            &mut self.clusters,
            &mut self.glyphs,
        );
        advance += build::apply_spacing(
            &mut self.clusters,
            &mut self.glyphs,
            params.letter_spacing,
            params.word_spacing,
        );

        self.unicode_buffer = Some(glyph_buffer.clear());
        Some((advance, metrics))
    }
}

/// The shaping parameters `shape_font_run` consumes.
///
/// Lifted out of the larger [`ShapeInput`] so the reshape path can pass an equivalent set without
/// fabricating `analysis`/`text_range`/etc.
struct FontRunParams<'a> {
    font_size: f32,
    script: Script,
    language: Option<Language>,
    level: u8,
    orientation: RunOrientation,
    features: &'a [FontFeature],
    letter_spacing: f32,
    word_spacing: f32,
}

/// Computes a run's vertical metrics and decoration geometry, scaled to `font_size` with the given
/// normalized coordinates.
fn compute_metrics(
    blob: &[u8],
    index: u32,
    font_size: f32,
    coords: &[harfrust::NormalizedCoord],
) -> RunMetrics {
    let Ok(font_ref) = skrifa::FontRef::from_index(blob, index) else {
        return RunMetrics::default();
    };
    let metrics =
        skrifa::metrics::Metrics::new(&font_ref, skrifa::prelude::Size::new(font_size), coords);
    let units_per_em = metrics.units_per_em as f32;
    let (underline_offset, underline_size) = match metrics.underline {
        Some(underline) => (underline.offset, underline.thickness),
        None => {
            let default = units_per_em / 18.0;
            (default, default)
        }
    };
    let (strikethrough_offset, strikethrough_size) = match metrics.strikeout {
        Some(strikeout) => (strikeout.offset, strikeout.thickness),
        None => (metrics.ascent / 2.0, units_per_em / 18.0),
    };
    RunMetrics {
        ascent: metrics.ascent,
        descent: -metrics.descent,
        leading: metrics.leading,
        underline_offset,
        underline_size,
        strikethrough_offset,
        strikethrough_size,
        x_height: metrics.x_height,
        cap_height: metrics.cap_height,
    }
}

/// A run ready to shape.
///
/// `text` and `analysis` describe the whole paragraph; `text_range` selects the run within them.
/// The remaining fields are the resolved style for this run.
#[derive(Clone, Debug)]
pub struct ShapeInput<'a> {
    /// The full paragraph text.
    pub text: &'a str,
    /// The analysis of the full paragraph text.
    pub analysis: &'a Analysis,
    /// The byte range within `text`/`analysis` to shape.
    pub text_range: Range<usize>,
    /// The char range corresponding to `text_range`, i.e., indices into [`Analysis::char_infos`],
    /// counted in `char`s.
    pub char_range: Range<usize>,
    /// The run's script.
    pub script: Script,
    /// The run's language, if known.
    pub language: Option<Language>,
    /// The bidi embedding level (its parity gives the inline direction).
    pub level: u8,
    /// The run's orientation, from the paragraph's [`WritingMode`](crate::WritingMode).
    pub orientation: RunOrientation,
    /// The requested font attributes.
    pub attributes: Attributes,
    /// Font size in pixels per em.
    pub font_size: f32,
    /// OpenType features to apply.
    pub features: &'a [FontFeature],
    /// Variation axis settings to apply.
    pub variations: &'a [FontVariation],
    /// Extra spacing added to each cluster advance.
    pub letter_spacing: f32,
    /// Extra spacing added to each space cluster.
    pub word_spacing: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Analysis, AnalysisOptions, Analyzer, ItemizeOptions, ShapedText};
    use alloc::sync::Arc;
    use alloc::vec::Vec;
    use fontique::{
        Collection, CollectionOptions, FamilyId, QueryFamily, SourceCache, SourceCacheOptions,
    };

    const ROBOTO: &[u8] =
        include_bytes!("../../../parley_dev/assets/fonts/roboto_fonts/Roboto-Regular.ttf");
    const NOTO_ARABIC: &[u8] =
        include_bytes!("../../../parley_dev/assets/fonts/noto_fonts/NotoKufiArabic-Regular.otf");

    /// A single-font collection (no system fonts) holding `font`, plus its family id.
    fn single_font(font: &'static [u8]) -> (Collection, SourceCache, FamilyId) {
        let mut collection = Collection::new(CollectionOptions {
            system_fonts: false,
            shared: false,
        });
        let registered = collection.register_fonts(Blob::new(Arc::new(font)), None);
        let family = registered[0].0;
        (
            collection,
            SourceCache::new(SourceCacheOptions::default()),
            family,
        )
    }

    fn input<'a>(
        text: &'a str,
        analysis: &'a Analysis,
        item: &crate::Item,
        letter_spacing: f32,
    ) -> ShapeInput<'a> {
        ShapeInput {
            text,
            analysis,
            text_range: item.text_range.clone(),
            char_range: item.char_range.clone(),
            script: item.script,
            language: item.language,
            level: item.level,
            orientation: item.orientation,
            attributes: Attributes::default(),
            font_size: 32.0,
            features: &[],
            variations: &[],
            letter_spacing,
            word_spacing: 0.0,
        }
    }

    /// Analyses and shapes `text` with `query`, returning the pieces the to drive subsequent
    /// `apply_break`/`apply_concat`.
    fn shape_all(
        text: &str,
        query: &mut Query<'_>,
        letter_spacing: f32,
    ) -> (Analysis, ShapeContext, ShapedText) {
        let mut analysis = Analysis::new();
        Analyzer::new().analyze(text, &AnalysisOptions::default(), &mut analysis);
        let mut scx = ShapeContext::new();
        let mut shaped = ShapedText::new();
        for item in analysis.items(text, &ItemizeOptions::default()) {
            scx.shape_run(
                &input(text, &analysis, &item, letter_spacing),
                query,
                &mut shaped,
            );
        }
        (analysis, scx, shaped)
    }

    /// Asserts that runs tile the text in order, clusters tile each run, glyph access is
    /// consistent, and each run's advance matches its clusters.
    fn check_invariants(text: &str, shaped: &ShapedText) {
        let mut next_start = 0;
        for run in shaped.runs() {
            let range = run.text_range();
            assert_eq!(range.start, next_start, "runs tile the text contiguously");
            next_start = range.end;

            let mut pos = range.start;
            let mut advance = 0.0_f32;
            let mut glyphs = 0;
            for cluster in run.clusters() {
                let cluster_range = cluster.text_range();
                assert_eq!(cluster_range.start, pos, "clusters tile the run text");
                assert!(cluster_range.end > cluster_range.start);
                assert!(text.is_char_boundary(cluster_range.start));
                assert!(text.is_char_boundary(cluster_range.end));
                pos = cluster_range.end;
                advance += cluster.advance();
                let collected: Vec<_> = cluster.glyphs().collect();
                assert_eq!(collected.len(), cluster.glyph_len());
                glyphs += collected.len();
            }
            assert_eq!(pos, range.end, "clusters cover the whole run");
            assert_eq!(
                glyphs,
                run.glyphs().count(),
                "run glyphs match its clusters"
            );
            assert!(
                (advance - run.advance()).abs() < 0.01,
                "run advance matches its clusters"
            );
        }
        if !shaped.is_empty() {
            assert_eq!(next_start, text.len(), "runs cover the whole text");
        }
    }

    /// Finds the first interior cluster boundary that is unsafe to break.
    fn first_unsafe_interior_break(shaped: &ShapedText) -> Option<usize> {
        shaped.runs().find_map(|run| {
            run.clusters()
                .enumerate()
                .find(|(i, c)| *i > 0 && c.unsafe_to_break())
                .map(|(_, c)| c.text_range().start)
        })
    }

    #[test]
    fn shapes_latin_text() {
        let text = "Hello";
        let (mut collection, mut source, family) = single_font(ROBOTO);
        let mut query = collection.query(&mut source);
        query.set_families([QueryFamily::Id(family)]);

        let (_, _, base) = shape_all(text, &mut query, 0.0);
        check_invariants(text, &base);

        assert_eq!(base.len(), 1, "one font covers the text, so one run");
        let run = base.run(0).unwrap();
        assert_eq!(run.len(), 5, "five single-byte Latin clusters");
        for (i, cluster) in run.clusters().enumerate() {
            assert_eq!(cluster.text_range(), i..i + 1);
            assert_eq!(cluster.glyph_len(), 1);
        }
        let base_advance = run.advance();

        let (_, _, spaced) = shape_all(text, &mut query, 4.0);
        let spaced_advance = spaced.run(0).unwrap().advance();
        assert!(
            (spaced_advance - base_advance - 5.0 * 4.0).abs() < 0.01,
            "letter-spacing adds to every cluster ({base_advance} -> {spaced_advance})"
        );
    }

    /// Reshape an Arabic word around an unsafe interior break (severing a cursive join), then
    /// reshape a concat at the same position. The glyphs must change after the break and
    /// return to the original after the concat.
    #[test]
    fn arabic_break_concat_roundtrip() {
        let text = "تجربة";
        let (mut collection, mut source, family) = single_font(NOTO_ARABIC);
        let mut query = collection.query(&mut source);
        query.set_families([QueryFamily::Id(family)]);
        let (analysis, mut scx, mut shaped) = shape_all(text, &mut query, 0.0);
        check_invariants(text, &shaped);

        let snapshot = |shaped: &ShapedText| -> Vec<(u32, f32, f32, f32)> {
            shaped
                .runs()
                .flat_map(|run| run.glyphs().map(|g| (g.id, g.advance, g.x, g.y)))
                .collect()
        };
        let initial = snapshot(&shaped);

        let pos = first_unsafe_interior_break(&shaped)
            .expect("cursive Arabic has an unsafe interior break");
        assert!(!shaped.unsafe_break_region(pos).is_empty());
        scx.apply_break(text, &analysis, &mut shaped, pos, &mut query);
        check_invariants(text, &shaped);
        assert_ne!(
            initial,
            snapshot(&shaped),
            "severing a cursive join must change the glyphs"
        );

        // The seam is unsafe-to-concat: prepending tail would re-form the cursive join.
        let seam = shaped
            .runs()
            .flat_map(|run| run.clusters())
            .find(|c| c.text_range().start == pos)
            .expect("the break left a cluster boundary at pos");
        assert!(seam.unsafe_to_concat());
        assert!(!shaped.unsafe_concat_region(pos).is_empty());

        scx.apply_concat(text, &analysis, &mut shaped, pos, &mut query);
        check_invariants(text, &shaped);
        assert_eq!(
            initial,
            snapshot(&shaped),
            "break + concat must restore the original shape"
        );
    }

    /// Roboto forms an `fi` ligature: two characters, one glyph. Breaking between them must
    /// decompose that glyph, so the boundary before the second component is unsafe to break.
    #[test]
    fn latin_ligature_reshapes_when_split() {
        let text = "fi";
        let (mut collection, mut source, family) = single_font(ROBOTO);
        let mut query = collection.query(&mut source);
        query.set_families([QueryFamily::Id(family)]);
        let (analysis, mut scx, mut shaped) = shape_all(text, &mut query, 0.0);
        check_invariants(text, &shaped);

        let run = shaped.run(0).unwrap();
        assert!(
            run.clusters().any(|c| c.is_ligature_start()),
            "Roboto shapes `fi` as a ligature"
        );
        let component = run
            .clusters()
            .find(|c| c.is_ligature_continuation())
            .expect("the ligature has a second component");
        assert!(
            component.unsafe_to_break(),
            "breaking before a ligature component severs the ligature"
        );
        let pos = component.text_range().start;
        let glyphs_before = run.glyphs().count();

        assert!(!shaped.unsafe_break_region(pos).is_empty());
        scx.apply_break(text, &analysis, &mut shaped, pos, &mut query);
        check_invariants(text, &shaped);

        let run = shaped.run(0).unwrap();
        assert!(
            run.glyphs().count() > glyphs_before,
            "the ligature decomposed into more glyphs"
        );
        assert!(
            !run.clusters()
                .any(|c| c.is_ligature_start() || c.is_ligature_continuation()),
            "no ligature remains once it straddles the break"
        );
    }
}
