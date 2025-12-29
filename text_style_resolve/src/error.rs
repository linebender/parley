// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::ParseSettingsError;

/// Errors that can occur while resolving a style.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ResolveStyleError {
    /// Failed to parse font variation settings.
    FontVariations(ParseSettingsError),
    /// Failed to parse font feature settings.
    FontFeatures(ParseSettingsError),
}

impl core::fmt::Display for ResolveStyleError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::FontVariations(err) => write!(f, "invalid font variation settings: {err}"),
            Self::FontFeatures(err) => write!(f, "invalid font feature settings: {err}"),
        }
    }
}

impl core::error::Error for ResolveStyleError {}
