// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Misc helpers.

#[cfg(feature = "libm")]
#[allow(unused_imports)]
use core_maths::CoreFloat;

pub(crate) fn nearly_eq(x: f32, y: f32) -> bool {
    (x - y).abs() < f32::EPSILON
}

pub(crate) fn nearly_zero(x: f32) -> bool {
    nearly_eq(x, 0.)
}
