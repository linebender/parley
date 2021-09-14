//! Misc helpers.

pub fn nearly_eq(x: f32, y: f32) -> bool {
    (x - y).abs() < f32::EPSILON
}

pub fn nearly_zero(x: f32) -> bool {
    nearly_eq(x, 0.)
}
