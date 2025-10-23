//! Doco

use std::fs::File;

use icu::locale::locale;
use icu::properties::{CodePointMapData, props::{Script, GeneralCategory, GraphemeClusterBreak, BidiClass, LineBreak}};
use icu::collections::codepointtrie::TrieType;
use databake::Bake;
use icu_codepointtrie_builder::{CodePointTrieBuilder, CodePointTrieBuilderData};
use icu_provider::prelude::*;
use icu_provider_adapters::fork::ForkByMarkerProvider;
use icu_provider_export::blob_exporter::BlobExporter;
use icu_provider_export::prelude::*;
use icu_provider_source::SourceDataProvider;

use composite_props_marker::{CompositePropsV1, CompositePropsV1Data};

fn pack(script: Script, gc: GeneralCategory, gcb: GraphemeClusterBreak, bidi: BidiClass, lb: LineBreak) -> u32 {
    const SCRIPT_BITS: u32 = 10;
    const GC_BITS: u32 = 5;
    const GCB_BITS: u32 = 5;
    const BIDI_BITS: u32 = 5;
    const LB_BITS: u32 = 6;

    const SCRIPT_SHIFT: u32 = 0;
    const GC_SHIFT: u32 = SCRIPT_SHIFT + SCRIPT_BITS;
    const GCB_SHIFT: u32 = GC_SHIFT + GC_BITS;
    const BIDI_SHIFT: u32 = GCB_SHIFT + GCB_BITS;
    const LB_SHIFT: u32 = BIDI_SHIFT + BIDI_BITS;

    let s = script.to_icu4c_value() as u32;
    let gc = gc as u32;
    let gcb = gcb.to_icu4c_value() as u32;
    let bidi = bidi.to_icu4c_value() as u32;
    let lb = lb.to_icu4c_value() as u32;

    (s << SCRIPT_SHIFT)
        | (gc << GC_SHIFT)
        | (gcb << GCB_SHIFT)
        | (bidi << BIDI_SHIFT)
        | (lb << LB_SHIFT)
}

struct CompositePropsProvider {
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
        let lb_source = CodePointMapData::<LineBreak>::try_new_unstable(&self.source)?;

        // Load the individual properties from the source provider
        let script = script_source.as_borrowed();
        let gc = gc_source.as_borrowed();
        let gcb = gcb_source.as_borrowed();
        let bidi = bidi_source.as_borrowed();
        let lb = lb_source.as_borrowed();

        // Dense values table for 0..=0x10FFFF
        let mut values = Vec::<u32>::with_capacity(0x110000);
        for cp in 0u32..=0x10FFFF {
            let v = pack(
                script.get32(cp),
                gc.get32(cp),
                gcb.get32(cp),
                bidi.get32(cp),
                lb.get32(cp),
            );
            values.push(v);
        }

        // Build trie; `wasm` feature avoids external ICU4C at build-time
        let trie = CodePointTrieBuilder {
            data: CodePointTrieBuilderData::ValuesByCodePoint(&values),
            default_value: 0, // not observed; we filled all entries
            error_value: 0,
            trie_type: TrieType::Small,
        }.build();

        Ok(DataResponse {
            metadata: Default::default(),
            payload: DataPayload::from_owned(CompositePropsV1Data {
                trie: icu_properties::provider::PropertyCodePointMap::CodePointTrie(trie),
            }),
        })
    }
}

impl IterableDataProvider<CompositePropsV1> for CompositePropsProvider {
    fn iter_ids(&self) -> Result<std::collections::BTreeSet<DataIdentifierCow>, DataError> {
        let mut set = std::collections::BTreeSet::new();
        set.insert(DataIdentifierCow::from_locale(DataLocale::default()));
        Ok(set)
    }
}

// Let the exporter discover our custom marker:
extern crate alloc;
icu_provider::export::make_exportable_provider!(CompositePropsProvider, [CompositePropsV1,]);

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("baked_data");

    let icu4x_source_provider = SourceDataProvider::new();
    let custom_source_provider = CompositePropsProvider::new(icu4x_source_provider.clone());

    ExportDriver::new(
        // Your project is singleton-only; this family is ignored for singletons
        [DataLocaleFamily::single(locale!("en").into())],
        DeduplicationStrategy::Maximal.into(),
        LocaleFallbacker::new_without_data(),
    )
    .with_markers([
        // Your existing markers...
        icu_properties::provider::PropertyEnumScriptV1::INFO,
        icu_properties::provider::PropertyEnumGeneralCategoryV1::INFO,
        icu_properties::provider::PropertyEnumBidiClassV1::INFO,
        icu_properties::provider::PropertyEnumLineBreakV1::INFO,
        icu_properties::provider::PropertyEnumGraphemeClusterBreakV1::INFO,
        icu_properties::provider::PropertyBinaryVariationSelectorV1::INFO,
        icu_properties::provider::PropertyBinaryBasicEmojiV1::INFO,
        icu_properties::provider::PropertyBinaryEmojiV1::INFO,
        icu_properties::provider::PropertyBinaryExtendedPictographicV1::INFO,
        icu_properties::provider::PropertyBinaryRegionalIndicatorV1::INFO,
        icu_segmenter::provider::SegmenterBreakGraphemeClusterV1::INFO,
        icu_segmenter::provider::SegmenterBreakWordOverrideV1::INFO,
        icu_segmenter::provider::SegmenterDictionaryAutoV1::INFO,
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
        }).unwrap(),
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
            BlobExporter::new_with_sink(Box::new(File::create(&blob_path).unwrap())),
        )
        .expect("Composite blob export should succeed");
    
        // Generate a small Rust file to embed the blob bytes
        std::fs::write(
            out.join("composite_blob.rs"),
            "pub const COMPOSITE_BLOB: &[u8] = include_bytes!(concat!(env!(\"OUT_DIR\"), \"/composite.postcard\"));"
        ).unwrap();
}