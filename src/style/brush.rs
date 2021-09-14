/// Trait for types that represent the color of glyphs or decorations.
pub trait Brush: Clone + PartialEq + Default + core::fmt::Debug {}

/// Empty brush.
impl Brush for () {}

/// Brush for a 4-byte color value.
impl Brush for [u8; 4] {}

/// Brush for a 3-byte color value.
impl Brush for [u8; 3] {}
