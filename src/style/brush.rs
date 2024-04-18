// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// Trait for types that represent the color of glyphs or decorations.
pub trait Brush: Clone + PartialEq + Default + core::fmt::Debug {}

/// Empty brush.
impl Brush for () {}

/// Brush for a 4-byte color value.
impl Brush for [u8; 4] {}

/// Brush for a 3-byte color value.
impl Brush for [u8; 3] {}

impl Brush for peniko::Brush {}
