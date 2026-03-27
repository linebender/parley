// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Misc helpers.

pub(crate) fn nearly_eq(x: f32, y: f32) -> bool {
    (x - y).abs() < f32::EPSILON
}
