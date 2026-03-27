// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Resolution of dynamic properties within a context.

pub(crate) mod range;
pub(crate) mod tree;

pub(crate) use range::RangedStyleBuilder;

use core::ops::Range;
use parley_core::{Brush, ResolvedProperty, ResolvedStyle};

/// Style with an associated range.
#[derive(Debug, Clone)]
pub(crate) struct RangedStyle<B: Brush> {
    pub(crate) style: ResolvedStyle<B>,
    pub(crate) range: Range<usize>,
}

#[derive(Clone)]
struct RangedProperty<B: Brush> {
    property: ResolvedProperty<B>,
    range: Range<usize>,
}
