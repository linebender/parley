// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Parley backend for [`styled_text`].
//!
//! This crate lowers `styled_text`’s resolved computed style runs into Parley builder calls,
//! producing a [`parley::Layout`].
//!
//! ## Scope
//!
//! This crate focuses on mapping `styled_text` computed styles into Parley
//! [`parley::StyleProperty`] values.
//!
//! It intentionally does not handle:
//! - paint/brush resolution (callers provide a default brush and may extend this crate later)
//! - inline bidi controls / forced base direction (not currently modeled by Parley style properties)
//! - inline boxes / attachments (use `parley::InlineBox` directly when you add an attachment layer)
//!
//! ## Example
//!
//! ```no_run
//! use parley::{FontContext, Layout, LayoutContext};
//! use styled_text::StyledText;
//! use styled_text_parley::build_layout_from_styled_text;
//! use styled_text::{ComputedInlineStyle, ComputedParagraphStyle, FontSize, InlineStyle, Specified};
//!
//! let mut font_cx = FontContext::new();
//! let mut layout_cx = LayoutContext::new();
//! let base_inline = ComputedInlineStyle::default();
//! let base_paragraph = ComputedParagraphStyle::default();
//! let mut text = StyledText::new("Hello world!", base_inline, base_paragraph);
//! text.apply_span(
//!     text.range(6..12).unwrap(),
//!     InlineStyle::new().font_size(Specified::Value(FontSize::Em(1.5))),
//! );
//!
//! let layout: Layout<()> =
//!     build_layout_from_styled_text(&mut layout_cx, &mut font_cx, &text, 1.0, true, ());
//! ```
// LINEBENDER LINT SET - lib.rs - v3
// See https://linebender.org/wiki/canonical-lints/
// These lints shouldn't apply to examples or tests.
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
// These lints shouldn't apply to examples.
#![warn(clippy::print_stdout, clippy::print_stderr)]
// Targeting e.g. 32-bit means structs containing usize can give false positives for 64-bit.
#![cfg_attr(target_pointer_width = "64", warn(clippy::trivially_copy_pass_by_ref))]
// END LINEBENDER LINT SET
#![cfg_attr(docsrs, feature(doc_cfg))]
#![no_std]

extern crate alloc;

use core::fmt::Debug;

use parley::style::Brush;
use parley::{
    FontContext, FontFeatures, FontVariations, Layout, LayoutContext, RangedBuilder, StyleProperty,
};
use styled_text::{ComputedInlineStyle, StyledText};

mod convert;

#[cfg(test)]
mod tests;

use crate::convert::to_parley_line_height;

/// Builds a Parley [`Layout`] from a [`StyledText`].
///
/// This uses Parley’s ranged builder and applies computed inline runs as explicit `StyleProperty`
/// spans. The returned `Layout` has been shaped, but line breaking and alignment are left to the
/// caller.
pub fn build_layout_from_styled_text<T, A, B>(
    layout_cx: &mut LayoutContext<B>,
    font_cx: &mut FontContext,
    styled: &StyledText<T, A>,
    scale: f32,
    quantize: bool,
    default_brush: B,
) -> Layout<B>
where
    T: Debug + attributed_text::TextStorage + AsRef<str>,
    A: Debug + styled_text::HasInlineStyle,
    B: Brush + Clone,
{
    let text = styled.as_str();
    let mut builder = layout_cx.ranged_builder(font_cx, text, scale, quantize);

    let default_inline = styled.base_inline_style();
    push_inline_defaults(&mut builder, default_inline, default_brush.clone());

    // Paragraph-level properties currently supported by Parley.
    let paragraph = styled.computed_paragraph_style();
    builder.push_default(StyleProperty::WordBreak(paragraph.word_break()));
    builder.push_default(StyleProperty::OverflowWrap(paragraph.overflow_wrap()));
    builder.push_default(StyleProperty::TextWrapMode(paragraph.text_wrap_mode()));

    for run in styled.resolved_inline_runs_coalesced() {
        push_run_diffs(&mut builder, default_inline, &run.style, run.range);
    }

    builder.build(text)
}

fn push_inline_defaults<B: Brush + Clone>(
    builder: &mut RangedBuilder<'_, B>,
    style: &ComputedInlineStyle,
    brush: B,
) {
    builder.push_default(StyleProperty::Brush(brush));
    builder.push_default(StyleProperty::FontFamily(style.font_family().clone()));
    builder.push_default(StyleProperty::FontSize(style.font_size_px()));
    builder.push_default(StyleProperty::FontWidth(style.font_width()));
    builder.push_default(StyleProperty::FontStyle(style.font_style()));
    builder.push_default(StyleProperty::FontWeight(style.font_weight()));
    builder.push_default(FontVariations::from(style.font_variations()));
    builder.push_default(FontFeatures::from(style.font_features()));
    builder.push_default(StyleProperty::Locale(style.locale().copied()));
    builder.push_default(StyleProperty::Underline(style.underline()));
    builder.push_default(StyleProperty::Strikethrough(style.strikethrough()));
    builder.push_default(StyleProperty::LineHeight(to_parley_line_height(
        style.line_height(),
    )));
    builder.push_default(StyleProperty::WordSpacing(style.word_spacing_px()));
    builder.push_default(StyleProperty::LetterSpacing(style.letter_spacing_px()));
}

fn push_run_diffs<B: Brush + Clone>(
    builder: &mut RangedBuilder<'_, B>,
    default: &ComputedInlineStyle,
    run: &ComputedInlineStyle,
    range: core::ops::Range<usize>,
) {
    macro_rules! push_if {
        ($cond:expr, $prop:expr) => {
            if $cond {
                builder.push($prop, range.clone());
            }
        };
    }

    push_if!(
        run.font_family() != default.font_family(),
        StyleProperty::FontFamily(run.font_family().clone())
    );
    push_if!(
        run.font_size_px() != default.font_size_px(),
        StyleProperty::FontSize(run.font_size_px())
    );
    push_if!(
        run.font_width() != default.font_width(),
        StyleProperty::FontWidth(run.font_width())
    );
    push_if!(
        run.font_style() != default.font_style(),
        StyleProperty::FontStyle(run.font_style())
    );
    push_if!(
        run.font_weight() != default.font_weight(),
        StyleProperty::FontWeight(run.font_weight())
    );
    push_if!(
        run.font_variations() != default.font_variations(),
        FontVariations::from(run.font_variations())
    );
    push_if!(
        run.font_features() != default.font_features(),
        FontFeatures::from(run.font_features())
    );
    push_if!(
        run.locale() != default.locale(),
        StyleProperty::Locale(run.locale().copied())
    );
    push_if!(
        run.underline() != default.underline(),
        StyleProperty::Underline(run.underline())
    );
    push_if!(
        run.strikethrough() != default.strikethrough(),
        StyleProperty::Strikethrough(run.strikethrough())
    );
    push_if!(
        run.line_height() != default.line_height(),
        StyleProperty::LineHeight(to_parley_line_height(run.line_height()))
    );
    push_if!(
        run.word_spacing_px() != default.word_spacing_px(),
        StyleProperty::WordSpacing(run.word_spacing_px())
    );
    push_if!(
        run.letter_spacing_px() != default.letter_spacing_px(),
        StyleProperty::LetterSpacing(run.letter_spacing_px())
    );
}
