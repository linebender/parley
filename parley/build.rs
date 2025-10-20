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
        [DataLocaleFamily::FULL],
        DeduplicationStrategy::Maximal.into(),
        LocaleFallbacker::new_without_data(),
    )
        .with_markers([
            // Properties - Map data
            icu::properties::provider::PropertyEnumScriptV1::INFO,
            icu::properties::provider::PropertyEnumGeneralCategoryV1::INFO,
            icu::properties::provider::PropertyEnumBidiClassV1::INFO,
            icu::properties::provider::PropertyEnumLineBreakV1::INFO,
            icu::properties::provider::PropertyEnumGraphemeClusterBreakV1::INFO,

            // Properties - Set data
            icu::properties::provider::PropertyBinaryVariationSelectorV1::INFO,
            icu::properties::provider::PropertyBinaryBasicEmojiV1::INFO,
            icu::properties::provider::PropertyBinaryEmojiV1::INFO,
            icu::properties::provider::PropertyBinaryExtendedPictographicV1::INFO,
            icu::properties::provider::PropertyBinaryRegionalIndicatorV1::INFO,

            // Segmenters
            icu::segmenter::provider::SegmenterBreakGraphemeClusterV1::INFO,
            icu::segmenter::provider::SegmenterBreakWordOverrideV1::INFO,
            icu::segmenter::provider::SegmenterDictionaryAutoV1::INFO,
            icu::segmenter::provider::SegmenterLstmAutoV1::INFO,
            icu::segmenter::provider::SegmenterBreakWordV1::INFO,
            icu::segmenter::provider::SegmenterBreakLineV1::INFO,
        ])
        .export(
            &source,
            BakedExporter::new(mod_directory.clone(), {
                let mut options = Options::default();
                options.overwrite = true;
                options
            })
                .unwrap(),
        )
        .expect("Datagen should be successful");
}