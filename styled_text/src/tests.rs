// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::block::{Block, BlockKind};
use crate::document::StyledDocument;
use crate::text::StyledText;
use crate::{
    ComputedInlineStyle, ComputedParagraphStyle, FontSize, InlineResolveContext, InlineStyle,
    ResolveStyleExt, Specified,
};
use alloc::vec::Vec;
use core::ops::Range;
use text_primitives::BaseDirection;

/// Reference implementation of inline run resolution.
///
/// This intentionally uses the simplest (and slowest) algorithm: for each boundary segment, scan
/// all spans that overlap it, merge their declarations in authoring order, then resolve once.
///
/// The production implementation in `styled_text` uses a sweep-line that maintains an active span
/// set and avoids scanning all spans per segment. This helper exists to assert that the fast path
/// preserves identical semantics.
fn reference_resolved_inline_runs(
    text: &StyledText<&str, InlineStyle>,
) -> Vec<(Range<usize>, ComputedInlineStyle)> {
    let len = text.attributed.len();
    let mut boundaries = Vec::new();
    boundaries.push(0);
    boundaries.push(len);
    for (range, _) in text.attributed.attributes_iter() {
        boundaries.push(range.start);
        boundaries.push(range.end);
    }
    boundaries.sort_unstable();
    boundaries.dedup();

    let ctx = InlineResolveContext::new(&text.base_inline, &text.initial_inline, &text.root_inline);

    let mut out = Vec::new();
    for pair in boundaries.windows(2) {
        let start = pair[0];
        let end = pair[1];
        if start == end {
            continue;
        }

        let mut merged = InlineStyle::new();
        for (range, attr) in text.attributed.attributes_iter() {
            if range.start < end && range.end > start {
                for declaration in attr.declarations() {
                    merged.push_declaration(declaration.clone());
                }
            }
        }
        let computed = merged.resolve(ctx);
        out.push((start..end, computed));
    }
    out
}

/// Coalesces adjacent runs with equal computed styles.
///
/// This mirrors the behavior of `resolved_inline_runs_coalesced()` but operates on the `(Range,
/// ComputedInlineStyle)` tuples produced by `reference_resolved_inline_runs`.
fn coalesce_runs(
    runs: &[(Range<usize>, ComputedInlineStyle)],
) -> Vec<(Range<usize>, ComputedInlineStyle)> {
    let mut out: Vec<(Range<usize>, ComputedInlineStyle)> = Vec::new();
    for (range, style) in runs {
        match out.last_mut() {
            Some((last_range, last_style))
                if last_range.end == range.start && last_style == style =>
            {
                last_range.end = range.end;
            }
            _ => out.push((range.clone(), style.clone())),
        }
    }
    out
}

#[test]
fn produces_split_runs() {
    let base_inline = ComputedInlineStyle::default();
    let base_paragraph = ComputedParagraphStyle::default();
    let mut text = StyledText::new("Hello world!", base_inline, base_paragraph);
    text.apply_span(
        text.range(0..5).unwrap(),
        InlineStyle::new().font_size(Specified::Value(FontSize::Em(2.0))),
    );
    let runs: Vec<_> = text.resolved_inline_runs().collect();
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].range, 0..5);
    assert_eq!(runs[1].range, 5..12);
}

#[test]
fn set_text_clears_spans_and_paragraph_declarations() {
    let base_inline = ComputedInlineStyle::default();
    let base_paragraph = ComputedParagraphStyle::default();
    let mut text = StyledText::new("Hello", base_inline, base_paragraph);

    text.apply_span(
        text.range(0..5).unwrap(),
        InlineStyle::new().font_size(Specified::Value(FontSize::Px(20.0))),
    );
    text.set_paragraph_style(
        crate::ParagraphStyle::new().base_direction(Specified::Value(BaseDirection::Ltr)),
    );
    assert_eq!(text.attributed.attributes_len(), 1);
    assert_eq!(text.paragraph_style().declarations().len(), 1);

    text.set_text("World");
    assert_eq!(text.attributed.attributes_len(), 0);
    assert_eq!(text.paragraph_style().declarations().len(), 0);
    assert_eq!(text.attributed.text(), &"World");
}

#[test]
fn overlap_is_ordered() {
    let base_inline = ComputedInlineStyle::default();
    let base_paragraph = ComputedParagraphStyle::default();
    let mut text = StyledText::new("abc", base_inline.clone(), base_paragraph);
    text.apply_span(
        text.range(0..3).unwrap(),
        InlineStyle::new().font_size(Specified::Value(FontSize::Em(2.0))),
    );
    text.apply_span(
        text.range(1..2).unwrap(),
        InlineStyle::new().font_size(Specified::Value(FontSize::Px(10.0))),
    );

    let runs: Vec<_> = text.resolved_inline_runs().collect();
    assert_eq!(runs.len(), 3);
    assert_eq!(
        runs[0].style.font_size_px(),
        base_inline.font_size_px() * 2.0
    );
    assert_eq!(runs[1].style.font_size_px(), 10.0);
    assert_eq!(
        runs[2].style.font_size_px(),
        base_inline.font_size_px() * 2.0
    );
}

#[test]
fn dependent_properties_resolve_against_final_computed_style() {
    let base_inline = ComputedInlineStyle::default();
    let base_paragraph = ComputedParagraphStyle::default();
    let mut text = StyledText::new("abc", base_inline, base_paragraph);

    // Apply letter-spacing first, then change font-size. The `em` spacing should resolve against
    // the final computed font size for the run.
    text.apply_span(
        text.range(0..3).unwrap(),
        InlineStyle::new().letter_spacing(Specified::Value(crate::Spacing::Em(0.5))),
    );
    text.apply_span(
        text.range(0..3).unwrap(),
        InlineStyle::new().font_size(Specified::Value(FontSize::Px(20.0))),
    );

    let runs: Vec<_> = text.resolved_inline_runs().collect();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].style.font_size_px(), 20.0);
    assert_eq!(runs[0].style.letter_spacing_px(), 10.0);
}

#[test]
fn coalesces_adjacent_equal_runs() {
    let base_inline = ComputedInlineStyle::default();
    let base_paragraph = ComputedParagraphStyle::default();
    let mut text = StyledText::new("ab", base_inline, base_paragraph);

    let style = InlineStyle::new().font_size(Specified::Value(FontSize::Px(20.0)));
    text.apply_span(text.range(0..1).unwrap(), style.clone());
    text.apply_span(text.range(1..2).unwrap(), style);

    let runs: Vec<_> = text.resolved_inline_runs_coalesced().collect();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].range, 0..2);
    assert_eq!(runs[0].style.font_size_px(), 20.0);
}

#[test]
fn inherit_can_reset_a_property_within_an_overlap() {
    let base_inline = ComputedInlineStyle::default();
    let base_paragraph = ComputedParagraphStyle::default();
    let mut text = StyledText::new("abc", base_inline.clone(), base_paragraph);

    text.apply_span(
        text.range(0..3).unwrap(),
        InlineStyle::new().font_size(Specified::Value(FontSize::Px(20.0))),
    );
    text.apply_span(
        text.range(1..2).unwrap(),
        InlineStyle::new().font_size(Specified::Inherit),
    );

    let runs: Vec<_> = text.resolved_inline_runs().collect();
    assert_eq!(runs.len(), 3);
    assert_eq!(runs[0].style.font_size_px(), 20.0);
    assert_eq!(runs[1].style.font_size_px(), base_inline.font_size_px());
    assert_eq!(runs[2].style.font_size_px(), 20.0);
}

#[test]
fn initial_uses_block_initial_style_not_base_style() {
    let base_inline = ComputedInlineStyle::default().with_font_size_px(20.0);
    let initial_inline = ComputedInlineStyle::default().with_font_size_px(10.0);
    let base_paragraph = ComputedParagraphStyle::default();
    let initial_paragraph = ComputedParagraphStyle::default();

    let mut text = StyledText::new_with_initial(
        "abc",
        base_inline.clone(),
        initial_inline.clone(),
        base_paragraph,
        initial_paragraph,
    );

    text.apply_span(
        text.range(0..3).unwrap(),
        InlineStyle::new().font_size(Specified::Initial),
    );
    let runs: Vec<_> = text.resolved_inline_runs().collect();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].style.font_size_px(), 10.0);
}

#[test]
fn rem_uses_root_style_when_set() {
    let base_inline = ComputedInlineStyle::default();
    let base_paragraph = ComputedParagraphStyle::default();
    let mut text = StyledText::new("abc", base_inline, base_paragraph);

    let root_inline = ComputedInlineStyle::default().with_font_size_px(10.0);
    let root_paragraph = ComputedParagraphStyle::default();
    text.set_root_styles(root_inline, root_paragraph);

    text.apply_span(
        text.range(0..3).unwrap(),
        InlineStyle::new().font_size(Specified::Value(FontSize::Rem(2.0))),
    );
    let runs: Vec<_> = text.resolved_inline_runs().collect();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].style.font_size_px(), 20.0);
}

#[test]
fn document_root_styles_apply_to_new_blocks() {
    let mut doc = StyledDocument::new_with_root(
        ComputedInlineStyle::default().with_font_size_px(10.0),
        ComputedParagraphStyle::default(),
    );

    let mut text = StyledText::new(
        "abc",
        ComputedInlineStyle::default(),
        ComputedParagraphStyle::default(),
    );
    text.apply_span(
        text.range(0..3).unwrap(),
        InlineStyle::new().font_size(Specified::Value(FontSize::Rem(2.0))),
    );

    doc.push(Block {
        kind: BlockKind::Paragraph,
        nesting: 0,
        text,
    });

    let runs: Vec<_> = doc
        .iter()
        .next()
        .unwrap()
        .text
        .resolved_inline_runs()
        .collect();
    assert_eq!(runs[0].style.font_size_px(), 20.0);
}

#[test]
fn sweep_line_matches_reference_for_many_overlaps() {
    use crate::{FontWeight, Spacing};

    struct Lcg(u64);
    impl Lcg {
        fn new(seed: u64) -> Self {
            Self(seed)
        }
        fn next_u32(&mut self) -> u32 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1);
            (self.0 >> 32) as u32
        }
        fn next_usize(&mut self, max: usize) -> usize {
            if max == 0 {
                0
            } else {
                (self.next_u32() as usize) % max
            }
        }
        fn next_f32(&mut self, min: f32, max: f32) -> f32 {
            let t = (self.next_u32() as f32) / (u32::MAX as f32);
            min + (max - min) * t
        }
        fn next_bool(&mut self) -> bool {
            (self.next_u32() & 1) == 1
        }
    }

    let base_inline = ComputedInlineStyle::default().with_font_size_px(14.0);
    let base_paragraph = ComputedParagraphStyle::default();
    let content = "0123456789abcdef0123456789abcdef";

    let mut rng = Lcg::new(0x1234_5678_9abc_def0);
    for _case in 0..200 {
        let mut text = StyledText::new(content, base_inline.clone(), base_paragraph.clone());

        let span_count = rng.next_usize(25);
        for _ in 0..span_count {
            let mut start = rng.next_usize(content.len() + 1);
            let mut end = rng.next_usize(content.len() + 1);
            if start > end {
                core::mem::swap(&mut start, &mut end);
            }
            if start == end {
                continue;
            }

            let mut style = InlineStyle::new();
            let decl_count = 1 + rng.next_usize(4);
            for _ in 0..decl_count {
                match rng.next_usize(5) {
                    0 => {
                        // Intentionally allow multiple declarations of the same property within a
                        // single span; last wins.
                        let px = rng.next_f32(8.0, 40.0);
                        style = style.font_size(Specified::Value(FontSize::Px(px)));
                    }
                    1 => {
                        let px = rng.next_f32(-2.0, 8.0);
                        style = style.letter_spacing(Specified::Value(Spacing::Px(px)));
                    }
                    2 => style = style.underline(Specified::Value(rng.next_bool())),
                    3 => {
                        let w = rng.next_f32(1.0, 1000.0);
                        style = style.font_weight(Specified::Value(FontWeight::new(w)));
                    }
                    _ => style = style.font_size(Specified::Initial),
                }
            }

            text.apply_span(text.range(start..end).unwrap(), style);
        }

        let expected = reference_resolved_inline_runs(&text);
        let actual: Vec<_> = text
            .resolved_inline_runs()
            .map(|run| (run.range, run.style))
            .collect();
        assert_eq!(actual, expected);

        let expected_coalesced = coalesce_runs(&expected);
        let actual_coalesced: Vec<_> = text
            .resolved_inline_runs_coalesced()
            .map(|run| (run.range, run.style))
            .collect();
        assert_eq!(actual_coalesced, expected_coalesced);
    }
}
