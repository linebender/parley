// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// The paragraph's base direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum BaseDirection {
    /// Choose direction automatically (commonly "first-strong").
    #[default]
    Auto,
    /// Left-to-right.
    Ltr,
    /// Right-to-left.
    Rtl,
}

/// Control over word breaking, named for the CSS property.
///
/// See: <https://www.w3.org/TR/css-text-3/#word-break-property>
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum WordBreak {
    /// Customary rules.
    #[default]
    Normal,
    /// Breaking is allowed within "words".
    BreakAll,
    /// Breaking is forbidden within "words".
    KeepAll,
}

/// Control over "emergency" line-breaking.
///
/// See: <https://www.w3.org/TR/css-text-3/#overflow-wrap-property>
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum OverflowWrap {
    /// Only break at opportunities specified by word-breaking rules.
    #[default]
    Normal,
    /// Words may be broken at an arbitrary point if needed.
    Anywhere,
    /// Like `Anywhere`, but treated differently for min-content sizing in some engines.
    BreakWord,
}

/// Control over non-"emergency" line-breaking.
///
/// See: <https://www.w3.org/TR/css-text-4/#text-wrap-mode>
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum TextWrapMode {
    /// Wrap as needed to prevent overflow.
    #[default]
    Wrap,
    /// Do not wrap at soft-wrap opportunities.
    NoWrap,
}
