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
use parley::{FontContext, Layout, LayoutContext, TextStyle};
use styled_text::StyledText;

mod convert;

#[cfg(test)]
mod tests;

use crate::convert::to_parley_line_height;

/// Builds a Parley [`Layout`] from a [`StyledText`].
///
/// This uses Parley’s [`StyleRunBuilder`](parley::StyleRunBuilder) and passes fully specified
/// styles for each computed inline run. The returned `Layout` has been shaped, but line breaking
/// and alignment are left to the caller.
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
    let paragraph = styled.computed_paragraph_style();
    let mut builder = layout_cx.style_run_builder(font_cx, text, scale, quantize);

    for run in styled.resolved_inline_runs_coalesced() {
        let style: TextStyle<'static, '_, B> = TextStyle {
            font_family: run.style.font_family().clone(),
            font_size: run.style.font_size_px(),
            font_width: run.style.font_width(),
            font_style: run.style.font_style(),
            font_weight: run.style.font_weight(),
            font_variations: parley::FontVariations::from(run.style.font_variations()),
            font_features: parley::FontFeatures::from(run.style.font_features()),
            locale: run.style.locale().copied(),
            brush: default_brush.clone(),
            has_underline: run.style.underline(),
            underline_offset: None,
            underline_size: None,
            underline_brush: None,
            has_strikethrough: run.style.strikethrough(),
            strikethrough_offset: None,
            strikethrough_size: None,
            strikethrough_brush: None,
            line_height: to_parley_line_height(run.style.line_height()),
            word_spacing: run.style.word_spacing_px(),
            letter_spacing: run.style.letter_spacing_px(),
            word_break: paragraph.word_break(),
            overflow_wrap: paragraph.overflow_wrap(),
            text_wrap_mode: paragraph.text_wrap_mode(),
        };
        builder.push_style_run(style, run.range);
    }

    builder.build(text)
}
