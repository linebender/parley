// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::fontique::Collection;

#[cfg(feature = "std")]
use crate::fontique::SourceCache;

#[derive(Default)]
pub struct FontContext {
    pub collection: Collection,
    #[cfg(feature = "std")]
    pub source_cache: SourceCache,
}
