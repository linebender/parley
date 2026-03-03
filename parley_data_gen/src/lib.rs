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

/// Build the dense composite properties array for all Unicode code points.
fn build_composite_values() -> Vec<u32> {
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
    values
}

/// Exports ICU data as a CodePointTrie (databake) into the `out` directory.
pub fn generate(out: std::path::PathBuf) {
    let values = build_composite_values();

    let trie = CodePointTrieBuilder {
        data: CodePointTrieBuilderData::ValuesByCodePoint(&values),
        default_value: 0,
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

/// PackTab generation configuration.
#[derive(Debug)]
pub struct PacktabConfig {
    /// Compression level (1.0 = balanced, 5.0 = smaller, 9.0 = smallest).
    pub compression: f64,
    /// Whether to use unsafe array access in generated code.
    pub unsafe_access: bool,
}

/// Exports ICU data as PackTab lookup tables + generated Rust code into the `out` directory.
pub fn generate_packtab(out: std::path::PathBuf, config: &PacktabConfig) {
    let values = build_composite_values();
    let scalar_data: Vec<i64> = values.iter().map(|&v| v as i64).collect();

    let (info, best) = packtab::pack_table(&scalar_data, Some(0), config.compression);

    let namespace = "composite_packtab";
    let mut code = packtab::generate(
        &info,
        best,
        namespace,
        packtab::codegen::Language::Rust {
            unsafe_access: config.unsafe_access,
        },
    );

    if !code.ends_with('\n') {
        code.push('\n');
    }
    code.push('\n');
    code.push_str(&format!(
        "#[inline]\npub fn composite_get(cp: u32) -> u32 {{\n    {namespace}_get(cp as usize)\n}}\n"
    ));

    let mut file = BufWriter::new(std::fs::File::create(out.join("mod.rs")).unwrap());
    writeln!(&mut file, "{COPYRIGHT_HEADER}").unwrap();
    writeln!(
        &mut file,
        "//! Backing data for composite properties (PackTab, compression={}, unsafe={})",
        config.compression, config.unsafe_access
    )
    .unwrap();
    writeln!(&mut file).unwrap();
    writeln!(&mut file, "#![allow(").unwrap();
    for lint in [
        "unsafe_code",
        "trivial_numeric_casts",
        "missing_docs",
        "clippy::allow_attributes_without_reason",
        "clippy::unseparated_literal_suffix",
        "clippy::double_parens",
        "clippy::unnecessary_cast",
    ] {
        writeln!(&mut file, "    {lint},").unwrap();
    }
    writeln!(&mut file, "    reason = \"packtab generated code\"").unwrap();
    writeln!(&mut file, ")]").unwrap();
    writeln!(&mut file).unwrap();
    write!(&mut file, "{code}").unwrap();
}
