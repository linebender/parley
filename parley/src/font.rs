// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use fontique::Collection;

#[cfg(feature = "std")]
use fontique::SourceCache;

/// A font database/cache (wrapper around a the [fontique] crate)
///
/// This type is designed to be a global resource with only one per-application (or per-thread)
#[derive(Default)]
pub struct FontContext {
    pub collection: Collection,
    #[cfg(feature = "std")]
    pub source_cache: SourceCache,
}

impl FontContext {
    pub fn new() -> Self {
        Default::default()
    }
}
