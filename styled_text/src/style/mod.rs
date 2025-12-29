// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! CSS-inspired text style vocabulary.
//!
//! This module defines:
//! - A closed set of inline and paragraph style properties (the vocabulary)
//! - CSS-like reset semantics via [`Specified`]
//!
//! It is intentionally independent of any shaping/layout engine.
//!
//! Specified→computed resolution lives in [`resolve`](crate::resolve).

mod declarations;
mod paragraph;
mod settings;
mod specified;
mod values;

pub use declarations::{InlineDeclaration, InlineStyle, ParagraphDeclaration};
pub use paragraph::{BaseDirection, OverflowWrap, ParagraphStyle, TextWrapMode, WordBreak};
pub use settings::{FontFeature, FontFeatures, FontVariation, FontVariations, Tag};
pub use specified::Specified;
pub use text_primitives::{BidiControl, BidiDirection, BidiOverride};
pub use text_primitives::{FontWeight, FontWidth};
pub use text_primitives::{GenericFamily, ParseFontFamilyError, ParseFontFamilyErrorKind};
pub use text_primitives::{Language, ParseLanguageError};
pub use text_primitives::{ParseSettingsError, ParseSettingsErrorKind};
pub use values::{FontSize, FontStyle, LineHeight, Spacing};

/// Owned CSS `font-family` property value.
///
/// This is the owned form of [`text_primitives::FontFamily`]. The `'static` lifetime indicates
/// that the value does not borrow from an external string slice.
pub type FontFamily = text_primitives::FontFamily<'static>;

/// Owned font family name or generic family.
///
/// This is the owned form of [`text_primitives::FontFamilyName`]. The `'static` lifetime
/// indicates that the value does not borrow from an external string slice.
pub type FontFamilyName = text_primitives::FontFamilyName<'static>;
