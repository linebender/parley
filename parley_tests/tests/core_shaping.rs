// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Snapshot tests for `parley_core`'s analysis + shaping pipeline.
//!
//! Each test runs the two-stage pipeline (analyze, then shape every item) and
//! renders the resulting [`ShapedText`] — glyphs plus analysis-annotation
//! overlays — to a snapshot. See `util::core_render` for what the overlays show.

use fontique::{Attributes, Collection, CollectionOptions, SourceCache, SourceCacheOptions};
use parlance::{FontFeature, FontVariation, Tag};
use parley_core::{
    Analysis, AnalysisOptions, Analyzer, ItemizeOptions, ShapeContext, ShapeInput, ShapedText,
    TextOrientation, WritingMode,
};

use crate::test_name;
use crate::util::TestEnv;
use crate::util::env::load_fonts;

/// Em size the samples are shaped at.
const FONT_SIZE: f32 = 32.0;

/// A [`TestEnv`] for the shaping snapshots. The annotation overlays are drawn
/// with faint alpha, whose anti-aliased edges drift by ±1 per channel through
/// the PNG round-trip (see [`TestEnv`] docs), so allow a small tolerance — the
/// same value the rasterization-sensitive `draw`/`line_break` tests use.
fn shaping_env(test_name: &str) -> TestEnv {
    let mut env = TestEnv::new(test_name, None);
    env.set_tolerance(5.0);
    env
}

/// A single shaping request. Build one with struct-update syntax over
/// [`Default`] and call [`Shape::run`] to get the shaped output.
struct Shape<'a> {
    text: &'a str,
    /// Primary families, in priority order; per-grapheme fallback picks the
    /// first that covers each cluster.
    families: &'a [&'a str],
    font_size: f32,
    letter_spacing: f32,
    word_spacing: f32,
    features: &'a [FontFeature],
    variations: &'a [FontVariation],
    /// The paragraph's writing mode. Itemization resolves this to a per-run
    /// orientation (and, for `Vertical(Mixed)`, per character via UTR #50).
    writing_mode: WritingMode,
}

impl Default for Shape<'_> {
    fn default() -> Self {
        Self {
            text: "",
            families: &["Roboto"],
            font_size: FONT_SIZE,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            features: &[],
            variations: &[],
            writing_mode: WritingMode::Horizontal,
        }
    }
}

impl Shape<'_> {
    /// Analyzes and shapes the request against the bundled test fonts (no system
    /// fonts, so the result is deterministic).
    fn run(&self) -> ShapedText {
        let mut collection = Collection::new(CollectionOptions {
            system_fonts: false,
            shared: false,
        });
        load_fonts(&mut collection, parley_dev::font_dirs()).unwrap();
        let mut source = SourceCache::new(SourceCacheOptions::default());

        let mut analysis = Analysis::new();
        Analyzer::new().analyze(self.text, &AnalysisOptions::default(), &mut analysis);

        let mut scx = ShapeContext::new();
        let mut shaped = ShapedText::new();
        let mut query = collection.query(&mut source);
        query.set_families(self.families.iter().copied());

        let itemize_options = ItemizeOptions {
            writing_mode: self.writing_mode,
            ..Default::default()
        };
        for item in analysis.items(self.text, &itemize_options) {
            scx.shape_run(
                &ShapeInput {
                    text: self.text,
                    analysis: &analysis,
                    text_range: item.text_range.clone(),
                    char_range: item.char_range.clone(),
                    script: item.script,
                    language: item.language,
                    level: item.level,
                    orientation: item.orientation,
                    attributes: Attributes::default(),
                    font_size: self.font_size,
                    features: self.features,
                    variations: self.variations,
                    letter_spacing: self.letter_spacing,
                    word_spacing: self.word_spacing,
                },
                &mut query,
                &mut shaped,
            );
        }
        shaped
    }
}

#[test]
fn shaping_latin() {
    let mut env = shaping_env(test_name!());
    let shaped = Shape {
        text: "Hello, world!",
        ..Default::default()
    }
    .run();
    env.check_shaped_snapshot(&shaped);
}

#[test]
fn shaping_ligatures() {
    let mut env = shaping_env(test_name!());
    let features = [
        FontFeature::new(Tag::new(b"liga"), 1),
        FontFeature::new(Tag::new(b"dlig"), 1),
    ];
    let shaped = Shape {
        text: "fi fl ffi office",
        features: &features,
        ..Default::default()
    }
    .run();
    env.check_shaped_snapshot(&shaped);
}

#[test]
fn shaping_arabic_cursive() {
    let mut env = shaping_env(test_name!());
    // A cursively joined word; interior clusters are unsafe to break.
    let shaped = Shape {
        text: "تجربة",
        families: &["Noto Kufi Arabic"],
        ..Default::default()
    }
    .run();
    env.check_shaped_snapshot(&shaped);
}

#[test]
fn shaping_mixed_bidi() {
    let mut env = shaping_env(test_name!());
    // LTR paragraph (first-strong is Latin) with a right-to-left island, so the
    // runs reorder under UAX #9 L2.
    let shaped = Shape {
        text: "Hi مرحبا!",
        families: &["Roboto", "Noto Kufi Arabic"],
        ..Default::default()
    }
    .run();
    env.check_shaped_snapshot(&shaped);
}

#[test]
fn shaping_letter_spacing() {
    let mut env = shaping_env(test_name!());
    let shaped = Shape {
        text: "Spaced",
        letter_spacing: 8.0,
        ..Default::default()
    }
    .run();
    env.check_shaped_snapshot(&shaped);
}

#[test]
fn shaping_word_spacing() {
    let mut env = shaping_env(test_name!());
    let shaped = Shape {
        text: "a b c d",
        word_spacing: 16.0,
        ..Default::default()
    }
    .run();
    env.check_shaped_snapshot(&shaped);
}

#[test]
fn shaping_variable_weight() {
    let mut env = shaping_env(test_name!());
    // Drive the `wght` axis of the variable font directly; heavier glyphs are
    // wider, so the two snapshots differ visibly.
    for (name, wght) in [("regular", 400.0), ("bold", 900.0)] {
        let variations = [FontVariation::new(Tag::new(b"wght"), wght)];
        let shaped = Shape {
            text: "Weight",
            families: &["Arimo"],
            variations: &variations,
            ..Default::default()
        }
        .run();
        env.with_name(name).check_shaped_snapshot(&shaped);
    }
}

#[test]
fn shaping_vertical_upright() {
    let mut env = shaping_env(test_name!());
    // A CJK string in `vertical-rl` with upright orientation: every glyph is set
    // upright and the pen advances down the page (vertical metrics apply).
    let shaped = Shape {
        text: "日本語",
        families: &["Noto Sans CJK JP"],
        writing_mode: WritingMode::Vertical(TextOrientation::Upright),
        ..Default::default()
    }
    .run();
    env.check_shaped_snapshot(&shaped);
}

#[test]
fn shaping_vertical_upright_bidi() {
    let mut env = shaping_env(test_name!());
    // Upright orientation with an RTL Arabic island. Upright forces the direction to
    // left-to-right, so every run here stays upright and in logical order down the page.
    let shaped = Shape {
        // We use a different Arabic string from the "مرحبا" used elsewhere: the Noto Kufi Arabic
        // font appears to be missing vertical mark anchors, and `harfrust` then falls back to
        // shaping the dot in "ب" as if it's horizontally shaped. That means our usual string would
        // render erroneously, and we don't have a good way of dealing with it here.
        text: "日本 ABC 123 سلام 語",
        families: &["Roboto", "Noto Sans CJK JP", "Noto Kufi Arabic"],
        writing_mode: WritingMode::Vertical(TextOrientation::Upright),
        ..Default::default()
    }
    .run();
    env.check_shaped_snapshot(&shaped);
}

#[test]
fn shaping_vertical_mixed() {
    let mut env = shaping_env(test_name!());
    // Mixed orientation (UTR #50): the CJK characters are set upright while the
    // Latin run is rotated 90° clockwise (sideways), all on one vertical line.
    let shaped = Shape {
        text: "AB日本",
        families: &["Roboto", "Noto Sans CJK JP"],
        writing_mode: WritingMode::Vertical(TextOrientation::Mixed),
        ..Default::default()
    }
    .run();
    env.check_shaped_snapshot(&shaped);
}

#[test]
fn shaping_vertical_mixed_bidi() {
    let mut env = shaping_env(test_name!());
    // Japanese with an RTL Arabic island, in a vertical mixed mode. The CJK is set upright (UTR
    // #50), the Arabic is rotated sideways, and the runs reorder along the vertical line under UAX
    // #9 L2. The RTL Arabic run's clusters advancing down the page in reverse, between the two
    // upright CJK runs.
    let shaped = Shape {
        text: "日本 مرحبا 語",
        families: &["Noto Sans CJK JP", "Noto Kufi Arabic"],
        writing_mode: WritingMode::Vertical(TextOrientation::Mixed),
        ..Default::default()
    }
    .run();
    env.check_shaped_snapshot(&shaped);
}

#[test]
fn shaping_vertical_mixed_digits() {
    let mut env = shaping_env(test_name!());
    // Four scripts on one vertical mixed-orientation line.
    //
    // - The CJK ("日本語", "語") is set upright; the Latin ("ABC") and both digit groups are
    // rotated 90° clockwise (sideways).
    // - "123" follows a strong-L (the CJK), so under UAX #9 it keeps a left-to-right number that
    //   stays in place.
    // - "456" follows the Arabic letters, UAX #9 reclassifies it as an Arabic Number and
    //   it is pulled into the RTL run and reorders to sit before the Arabic word along the line,
    //   reading 4-5-6 top to bottom.
    // - The Arabic run is RTL, so its clusters advance down the page in reverse.
    let shaped = Shape {
        text: "日本語 123 مرحبا 456 ABC",
        families: &["Roboto", "Noto Sans CJK JP", "Noto Kufi Arabic"],
        writing_mode: WritingMode::Vertical(TextOrientation::Mixed),
        ..Default::default()
    }
    .run();
    env.check_shaped_snapshot(&shaped);
}

#[test]
fn shaping_vertical_sideways() {
    let mut env = shaping_env(test_name!());
    // `sideways-rl`: the whole line is shaped horizontally and rotated 90°
    // clockwise, so the Latin text reads top-to-bottom with its tops facing right.
    let shaped = Shape {
        text: "Parley",
        families: &["Roboto"],
        writing_mode: WritingMode::Vertical(TextOrientation::Sideways),
        ..Default::default()
    }
    .run();
    env.check_shaped_snapshot(&shaped);
}
