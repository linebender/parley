// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Misc helpers.

#[cfg(all(feature = "libm", not(test)))]
use core_maths::*;

pub fn nearly_eq(x: f32, y: f32) -> bool {
    (x - y).abs() < f32::EPSILON
}

pub fn nearly_zero(x: f32) -> bool {
    nearly_eq(x, 0.)
}
