// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Misc helpers.

use alloc::vec::Vec;

/// Used to reinterpret the lifetimes of a vector.
// For how this works, see:
// https://davidlattimore.github.io/posts/2025/09/02/rustforge-wild-performance-tricks.html
pub(crate) fn reuse_vec<T, U>(mut v: Vec<T>) -> Vec<U> {
    const {
        assert!(size_of::<T>() == size_of::<U>());
        assert!(align_of::<T>() == align_of::<U>());
    }
    v.clear();
    v.into_iter().map(|_x| unreachable!()).collect()
}

pub(crate) fn nearly_eq(x: f32, y: f32) -> bool {
    (x - y).abs() < f32::EPSILON
}

pub(crate) fn nearly_zero(x: f32) -> bool {
    nearly_eq(x, 0.)
}
