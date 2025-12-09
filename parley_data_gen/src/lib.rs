// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! See `./main.rs`.

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
use icu_provider_export::prelude::*;
use icu_provider_source::SourceDataProvider;
use std::io::{BufWriter, Write};
use parley_data::Properties;

const COPYRIGHT_HEADER: &str =
    "// Copyright 2025 the Parley Authors\n// SPDX-License-Identifier: Apache-2.0 OR MIT\n";

/// Exports ICU data provider as Rust code into the `out` directory.
pub fn generate(out: std::path::PathBuf) {
    let icu4x_source_provider = SourceDataProvider::new();

    // Generate ICU4X data
    {
        // The directory for ICU baked data is `out/icu4x_data`.
        let icu4x_data_dir = out.clone().join("icu4x_data");
        std::fs::create_dir_all(&icu4x_data_dir).unwrap();

        ExportDriver::new(
            [DataLocaleFamily::single(DataLocale::default())],
            DeduplicationStrategy::None.into(),
            LocaleFallbacker::new_without_data(),
        )
        .with_markers([
            icu_properties::provider::PropertyEnumBidiMirroringGlyphV1::INFO,
            icu_properties::provider::PropertyNameShortScriptV1::INFO,
            icu_segmenter::provider::SegmenterBreakGraphemeClusterV1::INFO,
            icu_segmenter::provider::SegmenterBreakWordOverrideV1::INFO,
            icu_segmenter::provider::SegmenterLstmAutoV1::INFO,
            icu_segmenter::provider::SegmenterBreakWordV1::INFO,
            icu_segmenter::provider::SegmenterBreakLineV1::INFO,
            icu_normalizer::provider::NormalizerNfcV1::INFO,
            icu_normalizer::provider::NormalizerNfdDataV1::INFO,
            icu_normalizer::provider::NormalizerNfdSupplementV1::INFO,
            icu_normalizer::provider::NormalizerNfdTablesV1::INFO,
        ])
        .with_segmenter_models([])
        .export(
            &icu4x_source_provider,
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
            COPYRIGHT_HEADER.to_string()
                + "\n#![allow(clippy::allow_attributes_without_reason)]\n"
                + &std::fs::read_to_string(icu4x_data_dir.clone().join("mod.rs")).unwrap(),
        )
        .unwrap();
    }

    // Generate `CompositeProps` data
    {
        // Dense values table for 0..=0x10FFFF
        let mut values = Vec::<u32>::with_capacity(0x110000);
        for cp in 0_u32..=0x10FFFF {
            let v = Properties::new(
                CodePointMapData::<Script>::new().get32(cp),
                CodePointMapData::<GeneralCategory>::new().get32(cp),
                CodePointMapData::<GraphemeClusterBreak>::new().get32(cp),
                CodePointMapData::<BidiClass>::new().get32(cp),
                CodePointSetData::new::<Emoji>().contains32(cp)
                    || CodePointSetData::new::<ExtendedPictographic>().contains32(cp),
                CodePointSetData::new::<VariationSelector>().contains32(cp),
                CodePointSetData::new::<RegionalIndicator>().contains32(cp),
                // See: https://github.com/unicode-org/icu4x/blob/ee5399a77a6b94efb5d4b60678bb458c5eedb25d/components/segmenter/src/line.rs#L338-L351
                matches!(
                    CodePointMapData::<LineBreak>::new().get32(cp),
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

        let composite_dir = out.join("composite");
        if !composite_dir.exists() {
            std::fs::create_dir(&composite_dir).unwrap();
        }
        let mut file = BufWriter::new(std::fs::File::create(composite_dir.join("mod.rs")).unwrap());

        writeln!(&mut file, "{COPYRIGHT_HEADER}").unwrap();
        writeln!(&mut file, "/// Backing data for the `CompositeProps`").unwrap();
        writeln!(
            &mut file,
            "/// Expected size: {}B",
            size_of_val(&trie) + databake::BakeSize::borrows_size(&trie)
        )
        .unwrap();
        writeln!(&mut file, "#[rustfmt::skip]").unwrap();
        writeln!(&mut file, "#[allow(unsafe_code, unused_unsafe, clippy::unseparated_literal_suffix, reason = \"databake behaviour\")]").unwrap();
        writeln!(
            &mut file,
            "pub const COMPOSITE: icu_collections::codepointtrie::CodePointTrie<'static, u32> = {};",
            databake::Bake::bake(&trie, &databake::CrateEnv::default())
        )
        .unwrap();
    }

    let mut file = BufWriter::new(std::fs::File::create(out.join("mod.rs")).unwrap());

    writeln!(&mut file, "{COPYRIGHT_HEADER}").unwrap();
    writeln!(&mut file, "mod composite;").unwrap();
    writeln!(&mut file, "mod icu4x_data;").unwrap();
    writeln!(&mut file, "pub use composite::*;").unwrap();
    writeln!(&mut file, "pub use icu4x_data::*;").unwrap();
}
