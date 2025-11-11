// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Unicode data as required by the Parley crate for efficient text analysis.

use alloc::{boxed::Box, vec::Vec};
use icu_codepointtrie_builder::{CodePointTrieBuilder, CodePointTrieBuilderData};
use icu_collections::codepointtrie::TrieType;
use icu_locale::LocaleFallbacker;
use icu_properties::props::{GeneralCategory, GraphemeClusterBreak, Script};
use icu_properties::{
    CodePointMapData, CodePointSetData,
    props::{
        BidiClass, Emoji, ExtendedPictographic, LineBreak, RegionalIndicator, VariationSelector,
    },
};
use icu_provider::prelude::icu_locale_core::locale;
use icu_provider::prelude::*;
use icu_provider_adapters::fork::ForkByMarkerProvider;
use icu_provider_export::{
    DataLocaleFamily, DeduplicationStrategy, ExportDriver, blob_exporter::BlobExporter,
};
use icu_provider_source::SourceDataProvider;

use unicode_data::*;

extern crate alloc;

/// Exports ICU data provider as Rust code into the `out` directory.
pub fn generate(out: std::path::PathBuf) {
    let icu4x_source_provider = SourceDataProvider::new();

    // Generate ICU4X data
    {
        // The directory for ICU baked data is `out/icu4x_data`.
        let icu4x_data_dir = out.clone().join("icu4x_data");
        std::fs::create_dir_all(&icu4x_data_dir).unwrap();

        ExportDriver::new(
            [DataLocaleFamily::single(locale!("en").into())],
            DeduplicationStrategy::Maximal.into(),
            LocaleFallbacker::new_without_data(),
        )
        .with_markers([
            icu_properties::provider::PropertyNameShortScriptV1::INFO,
            icu_segmenter::provider::SegmenterBreakGraphemeClusterV1::INFO,
            icu_segmenter::provider::SegmenterBreakWordOverrideV1::INFO,
            icu_segmenter::provider::SegmenterLstmAutoV1::INFO,
            icu_segmenter::provider::SegmenterBreakWordV1::INFO,
            icu_segmenter::provider::SegmenterBreakLineV1::INFO,
            icu_normalizer::provider::NormalizerNfcV1::INFO,
            icu_normalizer::provider::NormalizerNfdDataV1::INFO,
            icu_normalizer::provider::NormalizerNfdTablesV1::INFO,
        ])
        .export(
            &icu4x_source_provider.clone(),
            icu_provider_export::baked_exporter::BakedExporter::new(icu4x_data_dir.clone(), {
                let mut o = icu_provider_export::baked_exporter::Options::default();
                o.overwrite = true;
                o.use_separate_crates = true;
                o.pretty = true;
                o
            })
            .unwrap(),
        )
        .expect("Datagen should be successful");
        std::fs::write(
            icu4x_data_dir.clone().join("mod.rs"),
            "#![allow(clippy::allow_attributes_without_reason)]\n".to_string()
                + &std::fs::read_to_string(icu4x_data_dir.clone().join("mod.rs")).unwrap(),
        )
        .unwrap();
    }

    // Generate `CompositePropsV1` data
    {
        let custom_source_provider = CompositePropsProvider::new(icu4x_source_provider.clone());

        // The directory for the composite data is `out/composite`.
        let custom_data_dir = out.clone().join("composite");
        std::fs::create_dir_all(&custom_data_dir).unwrap();
        ExportDriver::new(
            [DataLocaleFamily::single(locale!("en").into())],
            DeduplicationStrategy::None.into(),
            LocaleFallbacker::new_without_data(),
        )
        .with_markers([CompositePropsV1::INFO])
        .export(
            &ForkByMarkerProvider::new(icu4x_source_provider.clone(), custom_source_provider),
            BlobExporter::new_with_sink(Box::new(
                std::fs::File::create(custom_data_dir.clone().join("composite.postcard")).unwrap(),
            )),
        )
        .expect("Composite blob export should succeed");

        // Generate a small Rust file to embed the blob bytes
        std::fs::write(
        custom_data_dir.clone().join("mod.rs"),
        "/// Backing data for the `CompositePropsV1` data provider.\npub const COMPOSITE_BLOB: &[u8] = include_bytes!(\"./composite.postcard\");\n",
    )
    .unwrap();

        // Write a small mod.rs file in `out` that re-exports the ICU baked data and the composite data.
        std::fs::write(
            out.clone().join("mod.rs"),
            "mod composite;\nmod icu4x_data;\npub use composite::*;\npub use icu4x_data::*;\n",
        )
        .unwrap();
    }
}

/// A data provider that provides composite properties for a given character.
#[derive(Debug)]
pub struct CompositePropsProvider {
    source: SourceDataProvider,
}

impl CompositePropsProvider {
    fn new(source: SourceDataProvider) -> Self {
        Self { source }
    }
}

impl DataProvider<CompositePropsV1> for CompositePropsProvider {
    fn load(&self, _req: DataRequest<'_>) -> Result<DataResponse<CompositePropsV1>, DataError> {
        let script_source = CodePointMapData::<Script>::try_new_unstable(&self.source)?;
        let gc_source = CodePointMapData::<GeneralCategory>::try_new_unstable(&self.source)?;
        let gcb_source = CodePointMapData::<GraphemeClusterBreak>::try_new_unstable(&self.source)?;
        let bidi_source = CodePointMapData::<BidiClass>::try_new_unstable(&self.source)?;
        let emoji_source = CodePointSetData::try_new_unstable::<Emoji>(&self.source).unwrap();
        let extended_pictographic_source =
            CodePointSetData::try_new_unstable::<ExtendedPictographic>(&self.source).unwrap();
        let variation_selector_source =
            CodePointSetData::try_new_unstable::<VariationSelector>(&self.source).unwrap();
        let regional_indicator_source =
            CodePointSetData::try_new_unstable::<RegionalIndicator>(&self.source).unwrap();
        let linebreak_source =
            CodePointMapData::<LineBreak>::try_new_unstable(&self.source).unwrap();

        // Load the individual properties from the source provider
        let script = script_source.as_borrowed();
        let gc = gc_source.as_borrowed();
        let gcb = gcb_source.as_borrowed();
        let bidi = bidi_source.as_borrowed();
        let emoji = emoji_source.as_borrowed();
        let variation_selector = variation_selector_source.as_borrowed();
        let regional_indicator = regional_indicator_source.as_borrowed();
        let extended_pictographic = extended_pictographic_source.as_borrowed();
        let linebreak = linebreak_source.as_borrowed();

        // Dense values table for 0..=0x10FFFF
        let mut values = Vec::<u32>::with_capacity(0x110000);
        for cp in 0_u32..=0x10FFFF {
            let v = Properties::new(
                script.get32(cp),
                gc.get32(cp),
                gcb.get32(cp),
                bidi.get32(cp),
                emoji.contains32(cp) || extended_pictographic.contains32(cp),
                variation_selector.contains32(cp),
                regional_indicator.contains32(cp),
                // See: https://github.com/unicode-org/icu4x/blob/ee5399a77a6b94efb5d4b60678bb458c5eedb25d/components/segmenter/src/line.rs#L338-L351
                matches!(
                    linebreak.get32(cp),
                    LineBreak::MandatoryBreak
                        | LineBreak::CarriageReturn
                        | LineBreak::LineFeed
                        | LineBreak::NextLine
                ),
            );
            values.push(v.into());
        }

        let trie = CodePointTrieBuilder {
            data: CodePointTrieBuilderData::ValuesByCodePoint(&values),
            default_value: 0, // not observed; we filled all entries
            error_value: 0,
            trie_type: TrieType::Small,
        }
        .build();

        Ok(DataResponse {
            metadata: DataResponseMetadata::default(),
            payload: DataPayload::from_owned(CompositePropsV1Data::new(trie)),
        })
    }
}

impl IterableDataProvider<CompositePropsV1> for CompositePropsProvider {
    fn iter_ids(&self) -> Result<alloc::collections::BTreeSet<DataIdentifierCow<'_>>, DataError> {
        let mut set = alloc::collections::BTreeSet::new();
        set.insert(DataIdentifierCow::from_locale(DataLocale::default()));
        Ok(set)
    }
}

icu_provider::export::make_exportable_provider!(CompositePropsProvider, [CompositePropsV1,]);
