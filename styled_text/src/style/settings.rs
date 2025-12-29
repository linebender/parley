// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::sync::Arc;

pub use text_primitives::{FontFeature, FontVariation, Tag};

/// Font variation settings (OpenType axis values).
///
/// This is a typed wrapper over a list of [`FontVariation`] values.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct FontVariations(Arc<[FontVariation]>);

impl FontVariations {
    /// Creates settings from a parsed list.
    #[inline]
    pub fn list(list: impl Into<Arc<[FontVariation]>>) -> Self {
        Self(list.into())
    }

    /// Returns the backing shared slice.
    #[inline]
    pub const fn as_arc_slice(&self) -> &Arc<[FontVariation]> {
        &self.0
    }
}

/// Font feature settings (OpenType feature values).
///
/// This is a typed wrapper over a list of [`FontFeature`] values.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct FontFeatures(Arc<[FontFeature]>);

impl FontFeatures {
    /// Creates settings from a parsed list.
    #[inline]
    pub fn list(list: impl Into<Arc<[FontFeature]>>) -> Self {
        Self(list.into())
    }

    /// Returns the backing shared slice.
    #[inline]
    pub const fn as_arc_slice(&self) -> &Arc<[FontFeature]> {
        &self.0
    }
}
