// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! See `./main.rs`.

use icu_codepointtrie_builder::{CodePointTrieBuilder, CodePointTrieBuilderData};
use icu_collections::codepointtrie::TrieType;
use icu_properties::props::{GeneralCategory, GraphemeClusterBreak, Script};
use icu_properties::{
    CodePointMapData, CodePointSetData,
    props::{
        BidiClass, Emoji, ExtendedPictographic, LineBreak, RegionalIndicator, VariationSelector,
    },
};
use parley_data::Properties;
use std::io::{BufWriter, Write};

const COPYRIGHT_HEADER: &str =
    "// Copyright 2025 the Parley Authors\n// SPDX-License-Identifier: Apache-2.0 OR MIT\n";

/// Exports ICU data provider as Rust code into the `out` directory.
pub fn generate(out: std::path::PathBuf) {
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

        let mut file = BufWriter::new(std::fs::File::create(out.join("mod.rs")).unwrap());

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
}
