// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Itemization splitting an [`Analysis`] into [`Item`]s. See [`Analysis::itemize_with`].

use core::ops::Range;

use icu_properties::props::VerticalOrientation;
use parlance::{Language, Script};
use parley_data::Properties;

use crate::analysis::CharInfo;
use crate::analyzer::Analysis;
use crate::common::{Boundary, RunOrientation, WritingMode};

/// The Latin script is the fallback for runs that contain no script.
const LATIN: Script = Script::from_bytes(*b"Latn");

/// `U+FFFC` OBJECT REPLACEMENT CHARACTER: the in-text marker for an inline object.
///
/// Inline boxes ([`ItemKind::InlineBox`]) are represented in the text with this character. It is
/// bidi-neutral (`ON`) and a line-break opportunity on both sides (`CB`).
const OBJECT_REPLACEMENT: char = '\u{FFFC}';

impl Analysis {
    /// Itemizes the analyzed text.
    ///
    /// See the documentation of [`Self::itemize_with`] for more information about itemization. Use
    /// that method to additionally split the runs on your own conditions (like style changes that
    /// affect shaping).
    ///
    /// `text` must match what was passed to [`Analyzer::analyze`](crate::Analyzer::analyze).
    pub fn items<'a>(
        &'a self,
        text: &'a str,
        options: &ItemizeOptions<'a>,
    ) -> impl Iterator<Item = Item> + 'a {
        self.itemize_with(text, options, |_, _| false)
    }

    /// Itemizes the analyzed text with an additional break predicate.
    ///
    /// The yielded [`Item`]s are maximally contiguous runs of the text in logical order of
    /// constant bidi level, script, language, and [orientation](RunOrientation)), additionally
    /// splitting any run when the `split_before` predicate returns `true`.
    ///
    /// A `U+FFFC` OBJECT REPLACEMENT CHARACTER also breaks the run on both sides and forms a
    /// one-character [`ItemKind::InlineBox`] item. Every other item is [`ItemKind::Text`].
    ///
    /// This breask runs on the same conditions as [`Self::items`] plus wherever `split_before`
    /// returns `true`. Use it to split on conditions like style changes that affect shaping.
    ///
    /// `text` must match what was passed to [`Analyzer::analyze`](crate::Analyzer::analyze).
    pub fn itemize_with<'a, F>(
        &'a self,
        text: &'a str,
        options: &ItemizeOptions<'a>,
        split_before: F,
    ) -> Itemizer<'a, F>
    where
        F: FnMut(usize, usize) -> bool,
    {
        Itemizer::new(
            text,
            self.char_infos(),
            self.bidi_levels(),
            options.language_overrides,
            options.writing_mode.uniform_orientation(),
            options.writing_mode.suppresses_bidi(),
            split_before,
        )
    }
}

/// An [`Item`] represents a run of shapeable text or a single inline box.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum ItemKind {
    /// A run of text to shape with a font (the usual case).
    #[default]
    Text,
    /// A single inline box, marked in the source by `U+FFFC` OBJECT REPLACEMENT CHARACTER.
    ///
    /// [`level`](Item::level) and [`orientation`](Item::orientation) are relevant: the box
    /// reorders and stacks like the text around it.
    InlineBox,
}

/// A maximal run of text with constant script, bidi level, language and
/// [orientation](RunOrientation), or a single inline box.
///
/// Items tile the source text contiguously and in logical order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    /// Source text range of the item in byte offsets.
    pub text_range: Range<usize>,
    /// Character range of the item: similar to [`Self::text_range`] but counted in `char`s.
    pub char_range: Range<usize>,
    /// What this item represents (text to shape or an inline box).
    pub kind: ItemKind,
    /// The segmentation boundary immediately before this item, i.e., the [`CharInfo::boundary`] of
    /// its first character.
    pub boundary: Boundary,
    /// The Unicode script of the run.
    pub script: Script,
    /// The resolved language, if determined.
    pub language: Option<Language>,
    /// The bidi embedding level (its parity gives the direction).
    pub level: u8,
    /// The run's orientation.
    pub orientation: RunOrientation,
}

/// Options for [`Analysis::itemize_with`].
#[derive(Clone, Copy, Debug)]
pub struct ItemizeOptions<'a> {
    /// Per-range explicit language.
    ///
    /// Ranges must be sorted and non-overlapping. Gaps infer the language from the script.
    pub language_overrides: &'a [(Range<usize>, Language)],
    /// The paragraph's writing mode.
    pub writing_mode: WritingMode,
}

impl Default for ItemizeOptions<'_> {
    fn default() -> Self {
        Self {
            language_overrides: &[],
            writing_mode: WritingMode::Horizontal,
        }
    }
}

/// Itemizer over the analyzed character stream, created by [`Analysis::itemize_with`].
pub struct Itemizer<'a, F> {
    text: &'a str,
    infos: &'a [CharInfo],
    levels: &'a [u8],
    language_overrides: &'a [(Range<usize>, Language)],
    /// Resolved orientation for every mode but `Vertical(Mixed)`; `None` there, where orientation
    /// is resolved per character via UTR #50.
    uniform_orientation: Option<RunOrientation>,
    /// When set every character is treated as left-to-right: bidi levels are forced to 0 so runs
    /// neither split on level nor reorder, and the line reads in logical order. See
    /// [`WritingMode::suppresses_bidi`].
    force_ltr: bool,
    split_before: F,

    /// Remaining characters.
    chars: core::str::CharIndices<'a>,
    /// Index of the next character `chars` will yield.
    char_index: usize,

    // State of the run currently being accumulated.
    run_start_byte: usize,
    run_start_char: usize,
    run_script: Script,
    run_level: u8,
    run_language: Option<Language>,
    run_orientation: RunOrientation,
    run_boundary: Boundary,
    run_kind: ItemKind,
    /// Whether the previous character was an inline box (forces a break before the following
    /// character).
    prev_is_box: bool,
    /// Set once the final run has been emitted.
    done: bool,
}

impl<F> core::fmt::Debug for Itemizer<'_, F> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Itemizer").finish_non_exhaustive()
    }
}

impl<'a, F: FnMut(usize, usize) -> bool> Itemizer<'a, F> {
    fn new(
        text: &'a str,
        infos: &'a [CharInfo],
        levels: &'a [u8],
        language_overrides: &'a [(Range<usize>, Language)],
        uniform_orientation: Option<RunOrientation>,
        force_ltr: bool,
        split_before: F,
    ) -> Self {
        let mut chars = text.char_indices();
        // Seed the run from the first character.
        let seed = match (chars.next(), infos.first()) {
            (Some((_, first_char)), Some(first_info)) => {
                // Carry the first *real* script in the paragraph; characters without one adopt it.
                let script = infos
                    .iter()
                    .map(|info| info.script())
                    .find(|&script| is_real_script(script))
                    .unwrap_or(LATIN);
                let orientation =
                    uniform_orientation.unwrap_or_else(|| mixed_orientation(first_char));
                Some((
                    script,
                    orientation,
                    first_info.boundary(),
                    if first_char == OBJECT_REPLACEMENT {
                        ItemKind::InlineBox
                    } else {
                        ItemKind::Text
                    },
                    first_char == OBJECT_REPLACEMENT,
                ))
            }
            _ => None,
        };
        let (run_script, run_orientation, run_boundary, run_kind, prev_is_box, done) = match seed {
            Some((script, orientation, boundary, kind, is_box)) => {
                (script, orientation, boundary, kind, is_box, false)
            }
            None => (
                LATIN,
                RunOrientation::default(),
                Boundary::None,
                ItemKind::Text,
                false,
                true,
            ),
        };

        Self {
            text,
            infos,
            levels,
            language_overrides,
            uniform_orientation,
            force_ltr,
            split_before,
            chars,
            // The seed character was char 0, so iteration starts at 1.
            char_index: 1,
            run_start_byte: 0,
            run_start_char: 0,
            run_script,
            run_level: if force_ltr {
                0
            } else {
                levels.first().copied().unwrap_or(0)
            },
            run_language: language_at(language_overrides, 0),
            run_orientation,
            run_boundary,
            run_kind,
            prev_is_box,
            done,
        }
    }

    /// Emits the run accumulated so far, ending just before the character at
    /// `(end_byte, end_char)`.
    fn finish_run(&self, end_byte: usize, end_char: usize) -> Item {
        Item {
            text_range: self.run_start_byte..end_byte,
            char_range: self.run_start_char..end_char,
            kind: self.run_kind,
            boundary: self.run_boundary,
            script: self.run_script,
            language: self.run_language,
            level: self.run_level,
            orientation: self.run_orientation,
        }
    }
}

impl<F: FnMut(usize, usize) -> bool> Iterator for Itemizer<'_, F> {
    type Item = Item;

    fn next(&mut self) -> Option<Item> {
        if self.done {
            return None;
        }

        while let Some((byte_index, ch)) = self.chars.next() {
            let char_index = self.char_index;
            self.char_index += 1;

            let is_box = ch == OBJECT_REPLACEMENT;
            let mut script = self.infos[char_index].script();
            if !is_real_script(script) {
                script = self.run_script;
            }
            let level = if self.force_ltr {
                0
            } else {
                self.levels.get(char_index).copied().unwrap_or(0)
            };
            let language = language_at(self.language_overrides, byte_index);
            let orientation = self
                .uniform_orientation
                .unwrap_or_else(|| mixed_orientation(ch));

            if is_box
                || self.prev_is_box
                || level != self.run_level
                || script != self.run_script
                || language != self.run_language
                || orientation != self.run_orientation
                || (self.split_before)(char_index, byte_index)
            {
                let item = self.finish_run(byte_index, char_index);
                self.run_start_byte = byte_index;
                self.run_start_char = char_index;
                self.run_script = script;
                self.run_level = level;
                self.run_language = language;
                self.run_orientation = orientation;
                self.run_boundary = self.infos[char_index].boundary();
                self.run_kind = if ch == OBJECT_REPLACEMENT {
                    ItemKind::InlineBox
                } else {
                    ItemKind::Text
                };
                self.prev_is_box = is_box;
                return Some(item);
            }
            self.prev_is_box = is_box;
        }

        // No more characters, emit the final run.
        self.done = true;
        Some(self.finish_run(self.text.len(), self.char_index))
    }
}

/// A script is "real" (and therefore can break an itemization run) when it is not common,
/// inherited or unknown. Otherwise it adopts the current run's script.
fn is_real_script(script: Script) -> bool {
    script != Script::COMMON && script != Script::INHERITED && script != Script::UNKNOWN
}

/// Resolves a character's [`RunOrientation`] under [`crate::TextOrientation::Mixed`] from its
/// UTR #50 `Vertical_Orientation`.
fn mixed_orientation(ch: char) -> RunOrientation {
    let vo = Properties::get(ch).vertical_orientation();
    if vo == VerticalOrientation::Upright || vo == VerticalOrientation::TransformedUpright {
        RunOrientation::Upright
    } else {
        RunOrientation::Sideways
    }
}

/// Returns the explicit language covering byte `pos`, if any override does.
fn language_at(overrides: &[(Range<usize>, Language)], pos: usize) -> Option<Language> {
    overrides
        .iter()
        .find(|(range, _)| range.contains(&pos))
        .map(|(_, language)| *language)
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use super::*;
    use crate::analyzer::{AnalysisOptions, Analyzer};
    use crate::common::{Direction, TextOrientation};

    fn arab() -> Script {
        Script::parse("Arab").unwrap()
    }

    fn latn() -> Script {
        Script::parse("Latn").unwrap()
    }

    #[test]
    fn horizontal_itemization() {
        let mut analyzer = Analyzer::new();
        let mut analysis = Analysis::new();
        // "hello"/"مرحبا"/"world"
        let text = "hello مرحبا world";
        analyzer.analyze(text, &AnalysisOptions::default(), &mut analysis);

        // First-strong is Latin, so the paragraph is LTR overall, and the run mix gives us both
        // LTR (even) and RTL (odd) bidi levels.
        assert_eq!(analysis.paragraph_level(), 0);
        let levels = analysis.bidi_levels();
        assert!(levels.iter().any(|&l| l % 2 == 0));
        assert!(levels.iter().any(|&l| l % 2 == 1));

        // A language override on the first three bytes ("hel") plus a `split_before` predicate
        // that fires once inside the trailing Latin word.
        let english = Language::parse("en").unwrap();
        let language_overrides = [(0..3, english)];
        let options = ItemizeOptions {
            language_overrides: &language_overrides,
            ..Default::default()
        };
        let items: Vec<Item> = analysis
            .itemize_with(text, &options, |_, byte_index| byte_index == 19)
            .collect();

        // The Arabic word is its own right-to-left run.
        let arabic = items
            .iter()
            .find(|item| item.script == arab())
            .expect("an Arabic run");
        assert_eq!(arabic.text_range, 6..16);
        assert!(Direction::from_bidi_level(arabic.level).is_rtl());

        // The language override carves a leading English item out of the Latin run.
        assert_eq!(items[0].text_range, 0..3);
        assert_eq!(items[0].language, Some(english));
        assert!(items[1..].iter().all(|item| item.language.is_none()));

        // The `split_before` predicate splits the trailing Latin word at byte 19. Both halves keep
        // the same script and bidi level.
        let split = items
            .iter()
            .position(|item| item.text_range.start == 19)
            .expect("a predicate-induced split");
        assert_eq!(items[split].script, latn());
        assert_eq!(items[split - 1].script, items[split].script);
        assert_eq!(items[split - 1].level, items[split].level);

        // Items tile the text contiguously in both byte and char space, and each item's char range
        // matches the chars in its byte range across the multibyte Arabic.
        assert_eq!(items.first().unwrap().text_range.start, 0);
        assert_eq!(items.last().unwrap().text_range.end, text.len());
        assert_eq!(items.first().unwrap().char_range.start, 0);
        assert_eq!(items.last().unwrap().char_range.end, text.chars().count());
        for pair in items.windows(2) {
            assert_eq!(pair[0].text_range.end, pair[1].text_range.start);
            assert_eq!(pair[0].char_range.end, pair[1].char_range.start);
        }
        for item in &items {
            let chars_in_bytes = text[item.text_range.clone()].chars().count();
            assert_eq!(item.char_range.len(), chars_in_bytes);
        }
    }

    #[test]
    fn vertical_orientation() {
        let mut analyzer = Analyzer::new();
        let mut analysis = Analysis::new();
        // Digits "1994" are sideways under UTR #50; "年に至っては" is upright. Script also varies
        // across the upright tail (Han vs Hiragana), which splits it into several runs.
        let text = "1994年に至っては";
        analyzer.analyze(text, &AnalysisOptions::default(), &mut analysis);

        let mixed = ItemizeOptions {
            writing_mode: WritingMode::Vertical(TextOrientation::Mixed),
            ..Default::default()
        };
        let items: Vec<_> = analysis.items(text, &mixed).collect();
        // The orientation boundary falls right after the four ASCII digit bytes.
        assert_eq!(items[0].text_range, 0..4);
        assert_eq!(items[0].orientation, RunOrientation::Sideways);
        let tail = &items[1..];
        assert!(
            tail.iter()
                .all(|item| item.orientation == RunOrientation::Upright),
            "the CJK tail stays upright regardless of its script splits",
        );
        assert_eq!(tail.first().unwrap().text_range.start, 4);
        assert_eq!(tail.last().unwrap().text_range.end, text.len());

        // A non-`Mixed` vertical mode forces a single orientation for the whole paragraph: the
        // digits no longer carve out their own sideways run.
        let upright = ItemizeOptions {
            writing_mode: WritingMode::Vertical(TextOrientation::Upright),
            ..Default::default()
        };
        let items: Vec<_> = analysis.items(text, &upright).collect();
        assert!(
            items
                .iter()
                .all(|item| item.orientation == RunOrientation::Upright),
            "Upright forces every run upright",
        );
        assert!(
            !items.iter().any(|item| item.text_range == (0..4)),
            "digits no longer carve out their own run",
        );
    }

    #[test]
    fn inline_boxes() {
        let mut analyzer = Analyzer::new();
        let mut analysis = Analysis::new();
        // "ab", two adjacent boxes (each 3 bytes), then "cd".
        let text = "ab\u{FFFC}\u{FFFC}cd";
        analyzer.analyze(text, &AnalysisOptions::default(), &mut analysis);

        let items: Vec<_> = analysis.items(text, &ItemizeOptions::default()).collect();
        // text / box / box / text; adjacent boxes don't merge.
        assert_eq!(items.len(), 4);

        assert_eq!(items[0].kind, ItemKind::Text);
        assert_eq!(items[0].text_range, 0..2);

        // Two back-to-back box items, each three bytes. U+FFFC is line-break class CB, so a break
        // is permitted before each box.
        for item in &items[1..=2] {
            assert_eq!(item.kind, ItemKind::InlineBox);
            assert_eq!(item.text_range.len(), 3);
            assert!(item.boundary.is_line_break());
        }

        // Trailing text run; CB also permits a break after the previous box.
        assert_eq!(items[3].kind, ItemKind::Text);
        assert_eq!(items[3].text_range.end, text.len());
        assert!(items[3].boundary.is_line_break());

        // Items still tile the text contiguously and in order.
        for pair in items.windows(2) {
            assert_eq!(pair[0].text_range.end, pair[1].text_range.start);
        }
    }
}
