//! Defines baked ICU4X Unicode data providers.
//!
//! This narrows data compiled from all Unicode data sets, to only that which we use.

use icu_provider_export::baked_exporter::*;
use icu_provider_export::prelude::*;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let mod_directory = PathBuf::from(std::env::var_os("OUT_DIR").unwrap())
        .join("baked_data");

    let source = icu_provider_source::SourceDataProvider::new();

    ExportDriver::new(
        [
            DataLocaleFamily::single(locale!("en").into()),
        ],
        DeduplicationStrategy::Maximal.into(),
        LocaleFallbacker::new_without_data(),
    )
        .with_markers([
            // Properties - Map data
            icu_properties::provider::PropertyEnumScriptV1::INFO,
            icu_properties::provider::PropertyEnumGeneralCategoryV1::INFO,
            icu_properties::provider::PropertyEnumBidiClassV1::INFO,
            icu_properties::provider::PropertyEnumLineBreakV1::INFO,
            icu_properties::provider::PropertyEnumGraphemeClusterBreakV1::INFO,

            // Script - short name
            icu_properties::provider::PropertyNameShortScriptV1::INFO,

            // Properties - Set data
            icu_properties::provider::PropertyBinaryVariationSelectorV1::INFO,
            icu_properties::provider::PropertyBinaryBasicEmojiV1::INFO,
            icu_properties::provider::PropertyBinaryEmojiV1::INFO,
            icu_properties::provider::PropertyBinaryExtendedPictographicV1::INFO,
            icu_properties::provider::PropertyBinaryRegionalIndicatorV1::INFO,

            // Segmenters
            icu_segmenter::provider::SegmenterBreakGraphemeClusterV1::INFO,
            icu_segmenter::provider::SegmenterBreakWordOverrideV1::INFO,
            icu_segmenter::provider::SegmenterDictionaryAutoV1::INFO,
            icu_segmenter::provider::SegmenterLstmAutoV1::INFO,
            icu_segmenter::provider::SegmenterBreakWordV1::INFO,
            icu_segmenter::provider::SegmenterBreakLineV1::INFO,

            // Normalisation
            icu_normalizer::provider::NormalizerNfcV1::INFO,
            icu_normalizer::provider::NormalizerNfdDataV1::INFO,
            icu_normalizer::provider::NormalizerNfdTablesV1::INFO,
        ])
        .export(
            &source,
            BakedExporter::new(mod_directory.clone(), {
                let mut options = Options::default();
                options.overwrite = true;
                options.use_separate_crates = true;
                options
            })
                .unwrap(),
        )
        .expect("Datagen should be successful");
}