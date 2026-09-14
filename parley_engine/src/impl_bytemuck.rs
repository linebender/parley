// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Optional `bytemuck` trait impls.

#![allow(
    unsafe_code,
    reason = "The `bytemuck` marker traits are `unsafe` and require `unsafe impl`."
)]

use crate::NormalizedCoord;
use bytemuck::{Pod, TransparentWrapper, Zeroable};

// Safety: The struct is `repr(transparent)`, wrapping an `i16`. All bit patterns are valid.
unsafe impl Pod for NormalizedCoord {}

// Safety: The struct is `repr(transparent)`, wrapping an `i16`.
unsafe impl Zeroable for NormalizedCoord {}

// Safety: The struct is `repr(transparent)`, wrapping an `i16`, and has no other fields.
unsafe impl TransparentWrapper<i16> for NormalizedCoord {}

#[cfg(test)]
mod tests {
    use bytemuck::{TransparentWrapper, Zeroable};

    use super::NormalizedCoord;

    #[test]
    fn zeroable() {
        assert_eq!(NormalizedCoord::zeroed(), NormalizedCoord::from_bits(0));
    }

    #[test]
    fn cast_slice() {
        let coords = [
            NormalizedCoord::from_bits(-16384),
            NormalizedCoord::from_bits(0),
            NormalizedCoord::from_bits(8192),
        ];
        let bits: &[i16] = bytemuck::cast_slice(&coords);
        assert_eq!(bits, [-16384, 0, 8192]);
        let back: &[NormalizedCoord] = bytemuck::cast_slice(bits);
        assert_eq!(back, coords);
    }

    #[test]
    fn transparent_wrapper() {
        let coords = [NormalizedCoord::from_bits(1), NormalizedCoord::from_bits(2)];
        assert_eq!(NormalizedCoord::peel_slice(&coords), [1, 2]);
        assert_eq!(
            NormalizedCoord::wrap_slice(&[3, 4]),
            [NormalizedCoord::from_bits(3), NormalizedCoord::from_bits(4)]
        );
    }

    /// Tests that [`NormalizedCoord`] is two bytes.
    ///
    /// That may catch its representation changing, in which case the implementations here
    /// definitely need revisiting.
    const _: () = {
        if size_of::<NormalizedCoord>() != 2 {
            panic!("`NormalizedCoord` is not two bytes");
        }
    };
}
