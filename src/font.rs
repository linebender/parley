// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::fontique::{Collection, SourceCache};

#[derive(Default)]
pub struct FontContext {
    pub collection: Collection,
    pub source_cache: SourceCache,
}
