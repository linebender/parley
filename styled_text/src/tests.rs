// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::block::{Block, BlockKind};
use crate::document::StyledDocument;
use crate::text::StyledText;
use alloc::vec::Vec;
use text_style::{ComputedInlineStyle, ComputedParagraphStyle, FontSize, InlineStyle, Specified};

#[test]
fn produces_split_runs() {
    let base_inline = ComputedInlineStyle::default();
    let base_paragraph = ComputedParagraphStyle::default();
    let mut text = StyledText::new("Hello world!", base_inline, base_paragraph);
    text.apply_span(
        0..5,
        InlineStyle::new().font_size(Specified::Value(FontSize::Em(2.0))),
    )
    .unwrap();
    let runs: Vec<_> = text.resolved_inline_runs().map(Result::unwrap).collect();
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].range, 0..5);
    assert_eq!(runs[1].range, 5..12);
}

#[test]
fn overlap_is_ordered() {
    let base_inline = ComputedInlineStyle::default();
    let base_paragraph = ComputedParagraphStyle::default();
    let mut text = StyledText::new("abc", base_inline.clone(), base_paragraph);
    text.apply_span(
        0..3,
        InlineStyle::new().font_size(Specified::Value(FontSize::Em(2.0))),
    )
    .unwrap();
    text.apply_span(
        1..2,
        InlineStyle::new().font_size(Specified::Value(FontSize::Px(10.0))),
    )
    .unwrap();

    let runs: Vec<_> = text.resolved_inline_runs().map(Result::unwrap).collect();
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
        0..3,
        InlineStyle::new().letter_spacing(Specified::Value(text_style::Spacing::Em(0.5))),
    )
    .unwrap();
    text.apply_span(
        0..3,
        InlineStyle::new().font_size(Specified::Value(FontSize::Px(20.0))),
    )
    .unwrap();

    let runs: Vec<_> = text.resolved_inline_runs().map(Result::unwrap).collect();
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
    text.apply_span(0..1, style.clone()).unwrap();
    text.apply_span(1..2, style).unwrap();

    let runs: Vec<_> = text
        .resolved_inline_runs_coalesced()
        .map(Result::unwrap)
        .collect();
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
        0..3,
        InlineStyle::new().font_size(Specified::Value(FontSize::Px(20.0))),
    )
    .unwrap();
    text.apply_span(1..2, InlineStyle::new().font_size(Specified::Inherit))
        .unwrap();

    let runs: Vec<_> = text.resolved_inline_runs().map(Result::unwrap).collect();
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

    text.apply_span(0..3, InlineStyle::new().font_size(Specified::Initial))
        .unwrap();
    let runs: Vec<_> = text.resolved_inline_runs().map(Result::unwrap).collect();
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
        0..3,
        InlineStyle::new().font_size(Specified::Value(FontSize::Rem(2.0))),
    )
    .unwrap();
    let runs: Vec<_> = text.resolved_inline_runs().map(Result::unwrap).collect();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].style.font_size_px(), 20.0);
}

#[test]
fn invalid_settings_surface_as_errors_in_run_iteration() {
    let base_inline = ComputedInlineStyle::default();
    let base_paragraph = ComputedParagraphStyle::default();
    let mut text = StyledText::new("abc", base_inline, base_paragraph);
    text.apply_span(
        0..3,
        InlineStyle::new()
            .font_variations(Specified::Value(text_style::Settings::source("wght 1"))),
    )
    .unwrap();

    let first = text.resolved_inline_runs().next().unwrap();
    assert!(first.is_err());
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
        0..3,
        InlineStyle::new().font_size(Specified::Value(FontSize::Rem(2.0))),
    )
    .unwrap();

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
        .map(Result::unwrap)
        .collect();
    assert_eq!(runs[0].style.font_size_px(), 20.0);
}
