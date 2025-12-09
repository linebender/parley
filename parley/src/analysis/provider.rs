// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(
    unsafe_code,
    reason = "ICU4X uses fast bytearray loading in its baked data sources."
)]
#![allow(elided_lifetimes_in_paths)]
#![allow(unreachable_pub)]
#![allow(clippy::unseparated_literal_suffix)]

pub use unicode_data::generated::*;

/// This macro requires the following crates:
/// * `icu_collections`
/// * `icu_normalizer`
/// * `icu_properties`
/// * `icu_provider`
/// * `icu_segmenter`
/// * `zerovec`
macro_rules! impl_data_provider {
    ($ provider : ty) => {
        make_provider!($provider);
        impl_normalizer_nfd_tables_v1!($provider);
        impl_normalizer_nfd_supplement_v1!($provider);
        impl_segmenter_break_grapheme_cluster_v1!($provider);
        impl_segmenter_break_line_v1!($provider);
        impl_normalizer_nfc_v1!($provider);
        impl_segmenter_lstm_auto_v1!($provider);
        impl_property_name_short_script_v1!($provider);
        impl_normalizer_nfd_data_v1!($provider);
        impl_segmenter_break_word_v1!($provider);
        impl_property_enum_bidi_mirroring_glyph_v1!($provider);
        impl_segmenter_break_word_override_v1!($provider);
    };
}

pub struct BakedProvider;
impl_data_provider!(BakedProvider);

pub(crate) static PROVIDER: BakedProvider = BakedProvider;
