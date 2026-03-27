// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// An inline bidirectional text control.
///
/// These map conceptually to Unicode bidi isolate/override controls. They are expressed as style
/// properties rather than literal control characters.
///
/// For background on bidi behavior see UAX #9:
/// <https://www.unicode.org/reports/tr9/>
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BidiControl {
    /// No explicit control.
    #[default]
    None,
    /// Isolate this span with the given direction.
    Isolate(BidiDirection),
    /// Override directional resolution within this span.
    Override(BidiOverride),
}

/// Direction choice used by bidi controls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BidiDirection {
    /// Choose direction automatically.
    Auto,
    /// Left-to-right.
    Ltr,
    /// Right-to-left.
    Rtl,
}

/// Direction choice used by bidi overrides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BidiOverride {
    /// Force left-to-right.
    Ltr,
    /// Force right-to-left.
    Rtl,
}
