// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::fmt;

use crate::ParseSettingsError;

/// Errors that can occur while resolving specified styles into computed styles.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ResolveStyleError {
    /// Font variations failed to parse.
    FontVariations(ParseSettingsError),
    /// Font features failed to parse.
    FontFeatures(ParseSettingsError),
}

impl fmt::Display for ResolveStyleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FontVariations(err) => write!(f, "font variations parse error: {err}"),
            Self::FontFeatures(err) => write!(f, "font features parse error: {err}"),
        }
    }
}

impl core::error::Error for ResolveStyleError {}
