// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Optional `bytemuck` trait impls.

#![allow(
    unsafe_code,
    reason = "The `bytemuck` marker traits are `unsafe` and require `unsafe impl`."
)]

use crate::GenericFamily;
use bytemuck::{Contiguous, NoUninit, Zeroable, checked::CheckedBitPattern};

// Safety: The enum is `repr(u8)` and has only fieldless variants.
unsafe impl NoUninit for GenericFamily {}

// Safety: The enum is `repr(u8)` and `0` is a valid value.
unsafe impl Zeroable for GenericFamily {}

// Safety: The enum is `repr(u8)`.
unsafe impl CheckedBitPattern for GenericFamily {
    type Bits = u8;

    fn is_valid_bit_pattern(bits: &u8) -> bool {
        // Don't need to compare against MIN_VALUE as this is u8 and 0 is the MIN_VALUE.
        *bits <= Self::MAX_VALUE
    }
}

// Safety: The enum is `repr(u8)`. All values are `u8` and fall within
// the min and max values.
unsafe impl Contiguous for GenericFamily {
    type Int = u8;
    const MIN_VALUE: u8 = Self::Serif as u8;
    #[allow(
        clippy::use_self,
        reason = "Using `Self::MAX_VALUE` here would refer to `Contiguous::MAX_VALUE` (self-reference)."
    )]
    const MAX_VALUE: u8 = GenericFamily::MAX_VALUE;
}

#[cfg(test)]
mod tests {
    use super::GenericFamily;
    use bytemuck::{Contiguous, Zeroable, checked::try_from_bytes};
    use core::ptr;

    #[test]
    fn checked_bit_pattern() {
        let valid = bytemuck::bytes_of(&2_u8);
        let invalid = bytemuck::bytes_of(&200_u8);

        assert_eq!(
            Ok(&GenericFamily::Monospace),
            try_from_bytes::<GenericFamily>(valid)
        );

        assert!(try_from_bytes::<GenericFamily>(invalid).is_err());
    }

    #[test]
    fn contiguous() {
        let hd1 = GenericFamily::SansSerif;
        let hd2 = GenericFamily::from_integer(hd1.into_integer());
        assert_eq!(Some(hd1), hd2);

        assert_eq!(None, GenericFamily::from_integer(255));
    }

    #[test]
    fn zeroable() {
        let hd = GenericFamily::zeroed();
        assert_eq!(hd, GenericFamily::Serif);
    }

    /// Tests that the [`Contiguous`] impl for [`GenericFamily`] is not trivially incorrect.
    const _: () = {
        let mut value = 0;
        while value <= GenericFamily::MAX_VALUE {
            // Safety: In a const context, therefore if this makes an invalid GenericFamily, that will be detected.
            let it: GenericFamily = unsafe { ptr::read((&raw const value).cast()) };
            // Evaluate the enum value to ensure it actually has a valid tag.
            if it as u8 != value {
                unreachable!();
            }
            value += 1;
        }
    };
}

#[cfg(doctest)]
/// Doctests aren't collected under `cfg(test)`; we can use `cfg(doctest)` instead.
mod doctests {
    /// Validates that any new variants in `GenericFamily` has led to a change in the `Contiguous`
    /// impl.
    ///
    /// ```compile_fail,E0080
    /// use bytemuck::Contiguous;
    /// use text_primitives::GenericFamily;
    /// const {
    ///     let value = GenericFamily::MAX_VALUE + 1;
    ///     // Safety: In a const context, therefore if this makes an invalid GenericFamily, that will be detected.
    ///     // (Indeed, we rely upon that)
    ///     let it: GenericFamily = unsafe { core::ptr::read((&raw const value).cast()) };
    ///     // Evaluate the enum value to ensure it actually has an invalid tag.
    ///     if it as u8 != value {
    ///         unreachable!();
    ///     }
    /// }
    /// ```
    const _GENERIC_FAMILY: () = {};
}
