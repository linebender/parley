// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::borrow::Cow;

pub use fontique::{FontStyle, FontWeight, FontWidth, GenericFamily};
pub use text_primitives::{FontFamily, FontFamilyName, FontFeature, FontVariation, Tag};

/// Font variation settings (OpenType axis values).
///
/// Parley requires typed settings; if you have CSS-like strings, parse them up-front with
/// [`FontVariation::parse_css_list`] and then pass the resulting slice to Parley.
///
/// ```
/// # use parley::{FontVariation, FontVariations, StyleProperty};
/// #
/// let variations_vec: Vec<_> = FontVariation::parse_css_list(r#""wght" 700, "wdth" 125.5"#)
///     .collect::<Result<_, _>>()
///     .unwrap();
///
/// let property: StyleProperty<'_, ()> = variations_vec.as_slice().into();
/// ```
#[derive(Clone, PartialEq, Debug)]
pub struct FontVariations<'a>(Cow<'a, [FontVariation]>);

impl<'a> FontVariations<'a> {
    /// Creates an empty list of font variations.
    #[inline]
    pub const fn empty() -> Self {
        Self(Cow::Borrowed(&[]))
    }

    /// Returns the settings as a slice.
    #[inline]
    pub fn as_slice(&self) -> &[FontVariation] {
        self.0.as_ref()
    }
}

impl<'a> From<&'a [FontVariation]> for FontVariations<'a> {
    fn from(value: &'a [FontVariation]) -> Self {
        Self(Cow::Borrowed(value))
    }
}

impl<'a, const N: usize> From<&'a [FontVariation; N]> for FontVariations<'a> {
    fn from(value: &'a [FontVariation; N]) -> Self {
        Self(Cow::Borrowed(&value[..]))
    }
}

impl AsRef<[FontVariation]> for FontVariations<'_> {
    #[inline]
    fn as_ref(&self) -> &[FontVariation] {
        self.0.as_ref()
    }
}

/// Font feature settings (OpenType feature values).
///
/// Parley requires typed settings; if you have CSS-like strings, parse them up-front with
/// [`FontFeature::parse_css_list`] and then pass the resulting slice to Parley.
///
/// ```
/// # use parley::{FontFeature, FontFeatures, StyleProperty};
/// #
/// let features_vec: Vec<_> = FontFeature::parse_css_list(r#""liga" on, "kern" off, "salt" 3"#)
///     .collect::<Result<_, _>>()
///     .unwrap();
///
/// let property: StyleProperty<'_, ()> = features_vec.as_slice().into();
/// ```
#[derive(Clone, PartialEq, Debug)]
pub struct FontFeatures<'a>(Cow<'a, [FontFeature]>);

impl<'a> FontFeatures<'a> {
    /// Creates an empty list of font features.
    #[inline]
    pub const fn empty() -> Self {
        Self(Cow::Borrowed(&[]))
    }

    /// Returns the settings as a slice.
    #[inline]
    pub fn as_slice(&self) -> &[FontFeature] {
        self.0.as_ref()
    }
}

impl<'a> From<&'a [FontFeature]> for FontFeatures<'a> {
    fn from(value: &'a [FontFeature]) -> Self {
        Self(Cow::Borrowed(value))
    }
}

impl<'a, const N: usize> From<&'a [FontFeature; N]> for FontFeatures<'a> {
    fn from(value: &'a [FontFeature; N]) -> Self {
        Self(Cow::Borrowed(&value[..]))
    }
}

impl AsRef<[FontFeature]> for FontFeatures<'_> {
    #[inline]
    fn as_ref(&self) -> &[FontFeature] {
        self.0.as_ref()
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc;

    use alloc::vec::Vec;

    use super::{FontFeature, FontFeatures, FontVariation, FontVariations};
    use crate::StyleProperty;

    #[test]
    fn opentype_settings_from_parse_css_list() {
        let features_vec: Vec<_> =
            FontFeature::parse_css_list(r#""liga" on, "kern" off, "salt" 3"#)
                .collect::<Result<_, _>>()
                .unwrap();
        let variations_vec: Vec<_> = FontVariation::parse_css_list(r#""wght" 700, "wdth" 125.5"#)
            .collect::<Result<_, _>>()
            .unwrap();

        let features = FontFeatures::from(features_vec.as_slice());
        let variations = FontVariations::from(variations_vec.as_slice());

        let features_prop: StyleProperty<'_, ()> = features.clone().into();
        let variations_prop: StyleProperty<'_, ()> = variations.clone().into();

        assert_eq!(features.as_ref(), features_vec.as_slice());
        assert_eq!(variations.as_ref(), variations_vec.as_slice());

        match features_prop {
            StyleProperty::FontFeatures(list) => {
                assert_eq!(list.as_ref(), features_vec.as_slice());
            }
            _ => panic!("expected StyleProperty::FontFeatures"),
        }
        match variations_prop {
            StyleProperty::FontVariations(list) => {
                assert_eq!(list.as_ref(), variations_vec.as_slice());
            }
            _ => panic!("expected StyleProperty::FontVariations"),
        }
    }
}
