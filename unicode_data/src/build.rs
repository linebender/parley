// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Exposes functionality that allows for building ICU4X data providers.

use icu_codepointtrie_builder::{CodePointTrieBuilder, CodePointTrieBuilderData};
use icu_collections::codepointtrie::TrieType;
use icu_locale::LocaleFallbacker;
use icu_properties::{
    CodePointMapData, CodePointSetData,
    props::{
        BidiClass, Emoji, ExtendedPictographic, GeneralCategory, GraphemeClusterBreak, LineBreak,
        RegionalIndicator, Script, VariationSelector,
    },
};
use icu_provider::prelude::icu_locale_core::locale;
use icu_provider::prelude::*;
use icu_provider_adapters::fork::ForkByMarkerProvider;
use icu_provider_export::{
    DataLocaleFamily, DeduplicationStrategy, ExportDriver, blob_exporter::BlobExporter,
};
use icu_provider_source::SourceDataProvider;

use crate::*;

extern crate alloc;

fn pack(
    script: Script,
    gc: GeneralCategory,
    gcb: GraphemeClusterBreak,
    bidi: BidiClass,
    is_emoji_or_pictographic: bool,
    is_variation_selector: bool,
    is_region_indicator: bool,
    is_mandatory_linebreak: bool,
) -> u32 {
    let s = script.to_icu4c_value() as u32;
    let gc = gc as u32;
    let gcb = gcb.to_icu4c_value() as u32;
    let bidi = unicode_to_unicode_bidi(bidi) as u32;

    (s << Properties::SCRIPT_SHIFT)
        | (gc << Properties::GC_SHIFT)
        | (gcb << Properties::GCB_SHIFT)
        | (bidi << Properties::BIDI_SHIFT)
        | ((is_emoji_or_pictographic as u32) << Properties::IS_EMOJI_OR_PICTOGRAPH_SHIFT)
        | ((is_variation_selector as u32) << Properties::IS_VARIATION_SELECTOR_SHIFT)
        | ((is_region_indicator as u32) << Properties::IS_REGION_INDICATOR_SHIFT)
        | ((is_mandatory_linebreak as u32) << Properties::IS_MANDATORY_LINE_BREAK_SHIFT)
}

fn unicode_to_unicode_bidi(bidi: BidiClass) -> unicode_bidi::BidiClass {
    match bidi {
        BidiClass::LeftToRight => unicode_bidi::BidiClass::L,
        BidiClass::RightToLeft => unicode_bidi::BidiClass::R,
        BidiClass::ArabicNumber => unicode_bidi::BidiClass::AL,
        BidiClass::EuropeanNumber => unicode_bidi::BidiClass::EN,
        BidiClass::EuropeanSeparator => unicode_bidi::BidiClass::ES,
        BidiClass::EuropeanTerminator => unicode_bidi::BidiClass::ET,
        BidiClass::CommonSeparator => unicode_bidi::BidiClass::CS,
        BidiClass::ParagraphSeparator => unicode_bidi::BidiClass::B,
        BidiClass::SegmentSeparator => unicode_bidi::BidiClass::S,
        BidiClass::WhiteSpace => unicode_bidi::BidiClass::WS,
        BidiClass::OtherNeutral => unicode_bidi::BidiClass::ON,
        BidiClass::LeftToRightEmbedding => unicode_bidi::BidiClass::LRE,
        BidiClass::LeftToRightOverride => unicode_bidi::BidiClass::LRO,
        BidiClass::ArabicLetter => unicode_bidi::BidiClass::AL,
        BidiClass::RightToLeftEmbedding => unicode_bidi::BidiClass::RLE,
        BidiClass::RightToLeftOverride => unicode_bidi::BidiClass::RLO,
        BidiClass::PopDirectionalFormat => unicode_bidi::BidiClass::PDF,
        BidiClass::NonspacingMark => unicode_bidi::BidiClass::NSM,
        BidiClass::BoundaryNeutral => unicode_bidi::BidiClass::BN,
        BidiClass::FirstStrongIsolate => unicode_bidi::BidiClass::FSI,
        BidiClass::LeftToRightIsolate => unicode_bidi::BidiClass::LRI,
        BidiClass::RightToLeftIsolate => unicode_bidi::BidiClass::RLI,
        BidiClass::PopDirectionalIsolate => unicode_bidi::BidiClass::PDI,
        _ => unreachable!("Invalid BidiClass: {:?}", bidi),
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
            let v = pack(
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
            values.push(v);
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
            payload: DataPayload::from_owned(CompositePropsV1Data { trie }),
        })
    }
}

impl IterableDataProvider<CompositePropsV1> for CompositePropsProvider {
    fn iter_ids(&self) -> Result<std::collections::BTreeSet<DataIdentifierCow<'_>>, DataError> {
        let mut set = std::collections::BTreeSet::new();
        set.insert(DataIdentifierCow::from_locale(DataLocale::default()));
        Ok(set)
    }
}

icu_provider::export::make_exportable_provider!(CompositePropsProvider, [CompositePropsV1,]);

/// Exports ICU data provider as Rust code into the `out` directory.
///
/// After running this function in your `build.rs`, you can access it for consumption via:
///
/// ```rs
/// include!(concat!(env!("OUT_DIR"), "/baked_data/mod.rs"));
/// include!(concat!(env!("OUT_DIR"), "/baked_data/composite_blob.rs"));
///
/// pub struct BakedProvider;
/// impl_data_provider!(BakedProvider);
///
/// pub(crate) static PROVIDER: BakedProvider = BakedProvider;
/// ```
pub fn bake(out: std::path::PathBuf) {
    let icu4x_source_provider = SourceDataProvider::new();
    let custom_source_provider = CompositePropsProvider::new(icu4x_source_provider.clone());

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
        icu_provider_export::baked_exporter::BakedExporter::new(out.clone(), {
            let mut o = icu_provider_export::baked_exporter::Options::default();
            o.overwrite = true;
            o.use_separate_crates = true;
            o
        })
        .unwrap(),
    )
    .expect("Datagen should be successful");

    // Blob export for the composite marker
    let blob_path = out.clone().join("composite.postcard");

    ExportDriver::new(
        [DataLocaleFamily::single(locale!("en").into())],
        DeduplicationStrategy::None.into(),
        LocaleFallbacker::new_without_data(),
    )
    .with_markers([CompositePropsV1::INFO])
    .export(
        &ForkByMarkerProvider::new(icu4x_source_provider.clone(), custom_source_provider),
        BlobExporter::new_with_sink(Box::new(std::fs::File::create(&blob_path).unwrap())),
    )
    .expect("Composite blob export should succeed");

    // Generate a small Rust file to embed the blob bytes
    std::fs::write(
            out.join("composite_blob.rs"),
            "pub const COMPOSITE_BLOB: &[u8] = include_bytes!(concat!(env!(\"OUT_DIR\"), \"/baked_data/composite.postcard\"));"
        ).unwrap();
}
