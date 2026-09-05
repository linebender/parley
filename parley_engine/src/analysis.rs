// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Text analysis.
//!
//! Analysis is performed prior to shaping and is independent of fonts, turning a `&str` into
//! [`Analysis`].

use alloc::vec::Vec;
use icu_normalizer::properties::{
    CanonicalComposition, CanonicalCompositionBorrowed, CanonicalDecomposition,
    CanonicalDecompositionBorrowed,
};
use icu_properties::props::{BidiMirroringGlyph, GeneralCategory, GraphemeClusterBreak, Script};
use icu_properties::{
    CodePointMapData, CodePointMapDataBorrowed, PropertyNamesShort, PropertyNamesShortBorrowed,
};
use icu_segmenter::{GraphemeClusterSegmenter, GraphemeClusterSegmenterBorrowed};
use parlance::{BaseDirection, BidiLevel};
use parley_data::Properties;

use crate::bidi;
use crate::{AnalysisOptions, Analyzer};

/// The result of [`Analyzer::analyze`].
#[derive(Debug, Default)]
pub struct Analysis {
    /// Info for each character.
    pub(crate) info: Vec<CharInfo>,

    /// Bidi level for each character, parallel to `info`.
    ///
    /// Empty if the text is all LTR.
    pub(crate) levels: Vec<BidiLevel>,

    /// The base bidi level of the paragraph of text.
    pub(crate) paragraph_level: BidiLevel,
}

impl Analysis {
    /// Create a reusable [`Analysis`].
    ///
    /// Pass this to [`Analyzer::analyze`].
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear the result while retaining capacity.
    pub(crate) fn clear(&mut self) {
        self.info.clear();
        self.levels.clear();
        self.paragraph_level = BidiLevel::new(0);
    }

    /// The per-character info in source order.
    #[inline(always)]
    pub fn char_info(&self) -> &[CharInfo] {
        &self.info
    }

    /// The bidi level for each character, parallel to [`Self::char_info`].
    ///
    /// Empty when the whole paragraph is left-to-right.
    #[inline(always)]
    pub fn bidi_levels(&self) -> &[BidiLevel] {
        &self.levels
    }

    /// The base bidi level of the paragraph of text.
    #[inline(always)]
    pub fn paragraph_level(&self) -> BidiLevel {
        self.paragraph_level
    }
}

// TODO: Make `pub(crate)` once `parley_engine` owns shaping.
#[doc(hidden)]
#[expect(missing_debug_implementations, reason = "Will become private")]
pub struct AnalysisDataSources;

impl AnalysisDataSources {
    #[expect(clippy::new_without_default, reason = "Will become private")]
    pub fn new() -> Self {
        Self
    }

    #[inline(always)]
    pub fn properties(&self, c: char) -> Properties {
        Properties::get(c)
    }

    #[inline(always)]
    pub fn grapheme_segmenter(&self) -> GraphemeClusterSegmenterBorrowed<'_> {
        const { GraphemeClusterSegmenter::new() }
    }

    #[inline(always)]
    pub fn composing_normalizer(&self) -> CanonicalCompositionBorrowed<'_> {
        const { CanonicalComposition::new() }
    }

    #[inline(always)]
    pub fn decomposing_normalizer(&self) -> CanonicalDecompositionBorrowed<'_> {
        const { CanonicalDecomposition::new() }
    }

    #[inline(always)]
    pub fn script_short_name(&self) -> PropertyNamesShortBorrowed<'static, Script> {
        PropertyNamesShort::new()
    }

    #[inline(always)]
    fn brackets(&self) -> CodePointMapDataBorrowed<'_, BidiMirroringGlyph> {
        const { CodePointMapData::new() }
    }
}

/// Per-character analysis info.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct CharInfo {
    /// The line/word breaking boundary classification of this character.
    pub boundary: Boundary,
    /// The Unicode script this character belongs to.
    pub script: Script,
    /// The grapheme cluster boundary property of this character.
    pub grapheme_cluster_break: GraphemeClusterBreak,
    /// The impact this character has on directionality.
    pub bidi_class: icu_properties::props::BidiClass,
    /// Whether or not the character is a bracket, plus mirror data if so.
    pub bracket: BidiMirroringGlyph,

    flags: u8,
}

impl CharInfo {
    const VARIATION_SELECTOR_SHIFT: u8 = 0;
    const REGION_INDICATOR_SHIFT: u8 = 1;
    const CONTROL_SHIFT: u8 = 2;
    const EMOJI_OR_PICTOGRAPH_SHIFT: u8 = 3;
    const CONTRIBUTES_TO_SHAPING_SHIFT: u8 = 4;
    const FORCE_NORMALIZE_SHIFT: u8 = 5;
    const GRAPHEME_START_SHIFT: u8 = 6;

    #[allow(
        dead_code,
        reason = "To be used in more complete emoji checking, in select_font"
    )]
    const VARIATION_SELECTOR_MASK: u8 = 1 << Self::VARIATION_SELECTOR_SHIFT;
    #[allow(
        dead_code,
        reason = "To be used in more complete emoji checking, in select_font"
    )]
    const REGION_INDICATOR_MASK: u8 = 1 << Self::REGION_INDICATOR_SHIFT;
    const CONTROL_MASK: u8 = 1 << Self::CONTROL_SHIFT;
    const EMOJI_OR_PICTOGRAPH_MASK: u8 = 1 << Self::EMOJI_OR_PICTOGRAPH_SHIFT;
    const CONTRIBUTES_TO_SHAPING_MASK: u8 = 1 << Self::CONTRIBUTES_TO_SHAPING_SHIFT;
    const FORCE_NORMALIZE_MASK: u8 = 1 << Self::FORCE_NORMALIZE_SHIFT;
    const GRAPHEME_START_MASK: u8 = 1 << Self::GRAPHEME_START_SHIFT;

    fn new(
        boundary: Boundary,
        script: Script,
        grapheme_cluster_break: GraphemeClusterBreak,
        bidi_class: icu_properties::props::BidiClass,
        bracket: BidiMirroringGlyph,
        is_variation_selector: bool,
        is_region_indicator: bool,
        is_control: bool,
        is_emoji_or_pictograph: bool,
        contributes_to_shaping: bool,
        force_normalize: bool,
        is_grapheme_start: bool,
    ) -> Self {
        Self {
            boundary,
            script,
            grapheme_cluster_break,
            bidi_class,
            bracket,
            flags: (is_variation_selector as u8) << Self::VARIATION_SELECTOR_SHIFT
                | (is_region_indicator as u8) << Self::REGION_INDICATOR_SHIFT
                | (is_control as u8) << Self::CONTROL_SHIFT
                | (is_emoji_or_pictograph as u8) << Self::EMOJI_OR_PICTOGRAPH_SHIFT
                | (contributes_to_shaping as u8) << Self::CONTRIBUTES_TO_SHAPING_SHIFT
                | (force_normalize as u8) << Self::FORCE_NORMALIZE_SHIFT
                | (is_grapheme_start as u8) << Self::GRAPHEME_START_SHIFT,
        }
    }

    /// Whether this character is a variation selector.
    #[inline(always)]
    pub fn is_variation_selector(self) -> bool {
        self.flags & Self::VARIATION_SELECTOR_MASK != 0
    }

    /// Whether this character is a regional indicator symbol.
    #[inline(always)]
    pub fn is_region_indicator(self) -> bool {
        self.flags & Self::REGION_INDICATOR_MASK != 0
    }

    /// Whether this character is a control character.
    #[inline(always)]
    pub fn is_control(self) -> bool {
        self.flags & Self::CONTROL_MASK != 0
    }

    /// Whether this character is an emoji or pictograph.
    #[inline(always)]
    pub fn is_emoji_or_pictograph(self) -> bool {
        self.flags & Self::EMOJI_OR_PICTOGRAPH_MASK != 0
    }

    /// Whether this character contributes glyphs to shaping (`false` for control characters and
    /// most format characters).
    #[inline(always)]
    pub fn contributes_to_shaping(self) -> bool {
        self.flags & Self::CONTRIBUTES_TO_SHAPING_MASK != 0
    }

    /// Whether this character should be normalized before glyph mapping during shaping.
    #[inline(always)]
    pub fn force_normalize(self) -> bool {
        self.flags & Self::FORCE_NORMALIZE_MASK != 0
    }

    /// Whether this character begins a grapheme cluster ([UAX #29 § 3][graphemes]).
    ///
    /// [graphemes]: https://www.unicode.org/reports/tr29/#Grapheme_Cluster_Boundaries
    #[inline(always)]
    pub fn is_grapheme_start(self) -> bool {
        self.flags & Self::GRAPHEME_START_MASK != 0
    }
}

/// Boundary type of a character or cluster.
#[derive(Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Boundary {
    /// Not a boundary.
    None = 0,
    /// Potential line break.
    Line = 1,
    /// Mandatory line break.
    Mandatory = 2,
}

pub(crate) fn analyze_text(
    analyzer: &mut Analyzer,
    text: &str,
    options: &AnalysisOptions<'_>,
    analysis: &mut Analysis,
) {
    assert!(
        options
            .line_break_opportunities
            .windows(2)
            .all(|pair| pair[0] <= pair[1]),
        "line break opportunities must be sorted"
    );
    assert!(
        options
            .line_break_opportunities
            .iter()
            .all(|&offset| text.is_char_boundary(offset)),
        "line break opportunities must be UTF-8 character boundaries"
    );

    if text.is_empty() {
        analyzer
            .bidi
            .resolve(core::iter::empty(), options.base_direction);
        analysis.paragraph_level = analyzer.bidi.base_level();
        return;
    }

    let data_sources = AnalysisDataSources::new();
    let mut gb_iter = data_sources
        .grapheme_segmenter()
        .segment_str(text)
        .peekable();

    // Merge caller-provided line opportunities with grapheme boundaries.
    let mut lb_iter = options.line_break_opportunities.iter().peekable();
    let boundary_iter = text.char_indices().map(|(byte_pos, ch)| {
        // advance any stale grapheme boundary positions
        while let Some(&g) = gb_iter.peek() {
            if g < byte_pos {
                _ = gb_iter.next();
            } else {
                break;
            }
        }
        let mut is_grapheme_start = false;
        if let Some(&g) = gb_iter.peek()
            && g == byte_pos
        {
            is_grapheme_start = true;
            _ = gb_iter.next();
        }
        let is_line = lb_iter.peek().is_some_and(|&&offset| offset == byte_pos);
        while lb_iter.peek().is_some_and(|&&offset| offset == byte_pos) {
            _ = lb_iter.next();
        }
        assert!(
            !is_line || is_grapheme_start,
            "line break opportunities must be grapheme-cluster boundaries"
        );

        let boundary = if is_line {
            Boundary::Line
        } else {
            Boundary::None
        };

        (boundary, is_grapheme_start, ch)
    });

    let properties = |c| data_sources.properties(c);

    let mut needs_bidi_resolution = false;

    analysis.info.reserve(text.len());
    boundary_iter
        // Shift line break data forward one, as line boundaries corresponding with line-breaking
        // characters (like '\n') exist at an index position one higher than the respective
        // character's index, but we need our iterators to align, and the rest are simply
        // character-indexed.
        .fold(
            false,
            |is_mandatory_linebreak, (boundary, is_grapheme_start, ch)| {
                let properties = properties(ch);
                let script = properties.script();
                let grapheme_cluster_break = properties.grapheme_cluster_break();
                let bidi_class = properties.bidi_class();
                let general_category = properties.general_category();
                let is_emoji_or_pictograph = properties.is_emoji_or_pictograph();
                let is_variation_selector = properties.is_variation_selector();
                let is_region_indicator = properties.is_region_indicator();
                let next_mandatory_linebreak = properties.is_mandatory_linebreak();

                let boundary = if is_mandatory_linebreak {
                    Boundary::Mandatory
                } else {
                    boundary
                };

                let force_normalize = {
                    // "Extend" break chars should be normalized first, with two exceptions
                    if matches!(grapheme_cluster_break, GraphemeClusterBreak::Extend) &&
                    ch as u32 != 0x200C && // Is not a Zero Width Non-Joiner &&
                    !is_variation_selector
                    {
                        true
                    } else {
                        // All spacing mark break chars should be normalized first.
                        matches!(grapheme_cluster_break, GraphemeClusterBreak::SpacingMark)
                    }
                };

                needs_bidi_resolution |= bidi::needs_bidi_resolution(bidi_class);
                // TODO: maybe extend Properties to u64 to fit BidiMirroringGlyph
                let bracket = data_sources.brackets().get(ch);

                analysis.info.push(CharInfo::new(
                    boundary,
                    script,
                    grapheme_cluster_break,
                    bidi_class,
                    bracket,
                    is_variation_selector,
                    is_region_indicator,
                    general_category == GeneralCategory::Control,
                    is_emoji_or_pictograph,
                    contributes_to_shaping(general_category, script),
                    force_normalize,
                    is_grapheme_start,
                ));

                next_mandatory_linebreak
            },
        );

    if needs_bidi_resolution || options.base_direction == BaseDirection::Rtl {
        analyzer.bidi.resolve(
            text.chars().zip(
                analysis
                    .info
                    .iter()
                    .map(|info| (info.bidi_class, info.bracket)),
            ),
            options.base_direction,
        );
        core::mem::swap(&mut analysis.levels, &mut analyzer.bidi.levels);
        analysis.paragraph_level = analyzer.bidi.base_level();
    }
}

/// All characters contribute to shaping except:
/// - Control characters
/// - Format characters, unless they use the "Inherited" script
#[inline(always)]
pub(crate) fn contributes_to_shaping(general_category: GeneralCategory, script: Script) -> bool {
    if matches!(
        general_category,
        GeneralCategory::Control
            | GeneralCategory::LineSeparator
            | GeneralCategory::ParagraphSeparator
    ) {
        return false;
    }

    !(general_category == GeneralCategory::Format && script != Script::Inherited)
}
