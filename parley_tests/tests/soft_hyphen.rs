// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Soft hyphen (U+00AD) tests.
//!
//! A soft hyphen is an in-word line breaking opportunity that is invisible unless the break is
//! taken, in which case the line ends with a hyphen.

use crate::test_name;
use crate::util::{ColorBrush, TestEnv};
use parley::{Alignment, AlignmentOptions, BreakReason, Glyph, Layout, PositionedLayoutItem};

/// `hyphenation` with a soft hyphen after `hyphe`, followed by a second word so that there is a
/// regular break opportunity to fall back to.
const TEXT: &str = "hyphe\u{AD}nation word";

fn build(env: &mut TestEnv, text: &str, max_advance: Option<f32>) -> Layout<ColorBrush> {
    let builder = env.ranged_builder(text);
    let mut layout = builder.build(text);
    layout.break_all_lines(max_advance);
    layout.align(Alignment::Start, AlignmentOptions::default());
    layout
}

fn line_glyphs(layout: &Layout<ColorBrush>, line: usize) -> Vec<Glyph> {
    layout
        .get(line)
        .unwrap()
        .items()
        .filter_map(|item| match item {
            PositionedLayoutItem::GlyphRun(glyph_run) => Some(glyph_run),
            PositionedLayoutItem::InlineBox(_) => None,
        })
        .flat_map(|glyph_run| glyph_run.positioned_glyphs().collect::<Vec<_>>())
        .collect()
}

/// The glyph the test font renders for U+2010 HYPHEN, which is what a soft hyphen break draws.
fn hyphen(env: &mut TestEnv) -> Glyph {
    let layout = build(env, "\u{2010}", None);
    line_glyphs(&layout, 0)[0]
}

/// The advance of `text` laid out on a single line.
fn width(env: &mut TestEnv, text: &str) -> f32 {
    build(env, text, None).width()
}

#[track_caller]
fn assert_close(a: f32, b: f32, what: &str) {
    assert!((a - b).abs() < 1e-3, "{what}: {a} != {b}");
}

#[test]
fn soft_hyphen_break_renders_hyphen() {
    let mut env = TestEnv::new(test_name!(), None);

    let hyphen = hyphen(&mut env);
    let layout = build(&mut env, TEXT, Some(60.0));

    assert_eq!(layout.len(), 3, "expected `hyphe-`, `nation`, `word`");
    let line = layout.get(0).unwrap();
    assert_eq!(
        &TEXT[line.text_range()],
        "hyphe\u{AD}",
        "the hyphen is not part of the text"
    );
    assert_eq!(line.break_reason(), BreakReason::Regular);

    let glyphs = line_glyphs(&layout, 0);
    let last = *glyphs.last().expect("glyphs on the first line");
    assert_eq!(
        last.id, hyphen.id,
        "expected the line to end with the hyphen glyph"
    );
    assert_close(last.advance, hyphen.advance, "hyphen advance");
    assert_close(
        last.y,
        line.metrics().baseline,
        "the hyphen sits on the baseline",
    );

    env.check_layout_snapshot(&layout);
}

#[test]
fn soft_hyphen_advance_is_part_of_the_line() {
    let mut env = TestEnv::new(test_name!(), None);

    let hyphen = hyphen(&mut env);
    let hyphe_width = width(&mut env, "hyphe");
    let layout = build(&mut env, TEXT, Some(60.0));
    let line = layout.get(0).unwrap();

    assert_close(
        line.metrics().advance,
        hyphe_width + hyphen.advance,
        "the line advance includes the hyphen",
    );
    let run_advance: f32 = line.runs().map(|run| run.advance()).sum();
    assert_close(run_advance, line.metrics().advance, "run advances");
    let cluster_advance: f32 = line
        .runs()
        .flat_map(|run| {
            run.clusters()
                .map(|cluster| cluster.advance())
                .collect::<Vec<_>>()
        })
        .sum();
    assert_close(cluster_advance, line.metrics().advance, "cluster advances");

    // A soft hyphen that isn't at the end of a line stays invisible and costs nothing.
    let unbroken = build(&mut env, TEXT, None);
    assert_eq!(unbroken.len(), 1);
    let plain_width = width(&mut env, "hyphenation word");
    assert_close(
        unbroken.width(),
        plain_width,
        "a soft hyphen within a line has no width",
    );
    assert!(
        line_glyphs(&unbroken, 0)
            .iter()
            .all(|glyph| glyph.id != hyphen.id),
        "no hyphen glyph on an unbroken line"
    );
}

#[test]
fn soft_hyphen_break_requires_the_hyphen_to_fit() {
    let mut env = TestEnv::new(test_name!(), None);

    let hyphen = hyphen(&mut env);
    let hyphe_width = width(&mut env, "hyphe");

    // Room for `hyphe` but not for `hyphe-`: the opportunity isn't one, and the line breaks at
    // the space instead, as it would without the soft hyphen.
    let layout = build(&mut env, TEXT, Some(hyphe_width + hyphen.advance * 0.5));
    let line = layout.get(0).unwrap();
    assert_eq!(&TEXT[line.text_range()], "hyphe\u{AD}nation ");
    assert!(
        line_glyphs(&layout, 0)
            .iter()
            .all(|glyph| glyph.id != hyphen.id)
    );

    // Just enough room for `hyphe-`: the opportunity is taken.
    let layout = build(&mut env, TEXT, Some(hyphe_width + hyphen.advance + 0.01));
    let line = layout.get(0).unwrap();
    assert_eq!(&TEXT[line.text_range()], "hyphe\u{AD}");
    assert_eq!(
        line_glyphs(&layout, 0).last().unwrap().id,
        hyphen.id,
        "expected the line to end with the hyphen glyph"
    );
}

#[test]
fn soft_hyphen_break_in_rtl_text() {
    let mut env = TestEnv::new(test_name!(), None);

    let hyphen = hyphen(&mut env);
    // Arabic "hello world" with a soft hyphen inside the first word, and a third word to give
    // the breaker a regular opportunity as well.
    let text = "\u{645}\u{631}\u{62d}\u{628}\u{627}\u{ad}\u{628}\u{627}\u{644}\u{639}\u{627}\u{644}\u{645} \u{643}\u{644}\u{645}\u{629}";

    let layout = build(&mut env, text, Some(50.0));
    assert!(layout.is_rtl());
    let line = layout.get(0).unwrap();
    assert_eq!(
        &text[line.text_range()],
        "\u{645}\u{631}\u{62d}\u{628}\u{627}\u{ad}"
    );

    // The hyphen sits at the line's end edge, which for right-to-left text is its left edge, so
    // it is yielded first in visual order.
    let glyphs = line_glyphs(&layout, 0);
    let first = glyphs[0];
    assert_eq!(first.id, hyphen.id, "expected the hyphen glyph first");
    assert!(
        glyphs[1..].iter().all(|glyph| glyph.x > first.x),
        "expected the hyphen to be leftmost"
    );

    let run_advance: f32 = line.runs().map(|run| run.advance()).sum();
    assert_close(run_advance, line.metrics().advance, "run advances");
}
