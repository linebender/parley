// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! See `./main.rs`.

use icu_properties::props::{GeneralCategory, GraphemeClusterBreak, JoiningType, Script};
use icu_properties::{
    CodePointMapData, CodePointSetData, PropertyNamesShort,
    props::{
        BidiClass, Emoji, ExtendedPictographic, LineBreak, RegionalIndicator, VariationSelector,
    },
};
use parley_data::Properties;
use std::fmt::Write as _;
use std::io::{BufWriter, Write};

const COPYRIGHT_HEADER: &str =
    "// Copyright 2025 the Parley Authors\n// SPDX-License-Identifier: Apache-2.0 OR MIT\n";

/// Generation configuration.
#[derive(Debug)]
pub struct Config {
    /// Compression level (1.0 = balanced, 5.0 = smaller, 9.0 = even smaller, 10.0 = smallest).
    pub compression: f64,
    /// Whether to use unsafe array access in generated code.
    pub unsafe_access: bool,
}

/// Exports ICU data as `PackTab` lookup tables + generated Rust code into the `out` directory.
pub fn generate(out: std::path::PathBuf, config: &Config) {
    let scripts = CodePointMapData::<Script>::new();
    let joining_types = CodePointMapData::<JoiningType>::new();
    let general_categories = CodePointMapData::<GeneralCategory>::new();
    let grapheme_cluster_breaks = CodePointMapData::<GraphemeClusterBreak>::new();
    let bidi_classes = CodePointMapData::<BidiClass>::new();
    let line_breaks = CodePointMapData::<LineBreak>::new();
    let emoji = CodePointSetData::new::<Emoji>();
    let extended_pictographic = CodePointSetData::new::<ExtendedPictographic>();
    let variation_selectors = CodePointSetData::new::<VariationSelector>();
    let regional_indicators = CodePointSetData::new::<RegionalIndicator>();

    let mut scripts_with_joining_characters = [false; 256];

    // Generate the data required for `CompositeProps`.
    let values = {
        // Dense values table for 0..=0x10FFFF
        let mut values = Vec::<u32>::with_capacity(0x110000);
        for cp in 0_u32..=0x10FFFF {
            let script = scripts.get32(cp);
            let joining_type = joining_types.get32(cp);
            if matches!(
                joining_type,
                JoiningType::DualJoining | JoiningType::LeftJoining | JoiningType::RightJoining
            ) {
                let script = usize::from(script.to_icu4c_value());
                scripts_with_joining_characters[script] = true;
            }

            let v = Properties::new(
                script,
                general_categories.get32(cp),
                grapheme_cluster_breaks.get32(cp),
                bidi_classes.get32(cp),
                joining_type,
                emoji.contains32(cp) || extended_pictographic.contains32(cp),
                variation_selectors.contains32(cp),
                regional_indicators.contains32(cp),
                // See: https://github.com/unicode-org/icu4x/blob/ee5399a77a6b94efb5d4b60678bb458c5eedb25d/components/segmenter/src/line.rs#L338-L351
                matches!(
                    line_breaks.get32(cp),
                    LineBreak::MandatoryBreak
                        | LineBreak::CarriageReturn
                        | LineBreak::LineFeed
                        | LineBreak::NextLine
                ),
            );
            values.push(v.into());
        }
        values
    };
    let script_names = PropertyNamesShort::<Script>::new();
    let mut scripts_with_joining_characters: Vec<[u8; 4]> = scripts_with_joining_characters
        .into_iter()
        .enumerate()
        .filter(|(_, has_joining_characters)| *has_joining_characters)
        .map(|(value, _)| {
            let value = u16::try_from(value).expect("script value fits in u16");
            let script = Script::from_icu4c_value(value);
            script_names
                .get(script)
                .expect("script has a short name")
                .as_bytes()
                .try_into()
                .expect("script short name has four bytes")
        })
        .collect();
    scripts_with_joining_characters.sort_unstable();
    let mut joining_script_tags = String::from("[");
    for (index, tag) in scripts_with_joining_characters.iter().enumerate() {
        if index != 0 {
            joining_script_tags.push_str(", ");
        }
        write!(
            joining_script_tags,
            "u32::from_be_bytes(*b\"{}\")",
            str::from_utf8(tag).expect("script tag is ASCII")
        )
        .unwrap();
    }
    joining_script_tags.push(']');
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
    write!(
        code,
        "#[allow(missing_docs, reason = \"packtab generated code\")]\n#[inline]\npub fn composite_get(cp: u32) -> u32 {{\n    {namespace}_get(cp as usize)\n}}\n"
    )
    .unwrap();
    writeln!(
        code,
        "\n/// Sorted ISO 15924 tags for scripts with cursively joining characters.\npub(super) const SCRIPTS_WITH_JOINING_CHARACTERS: [u32; {}] = {joining_script_tags};",
        scripts_with_joining_characters.len()
    )
    .unwrap();

    let mut file = BufWriter::new(std::fs::File::create(out.join("mod.rs")).unwrap());
    writeln!(&mut file, "{COPYRIGHT_HEADER}").unwrap();
    writeln!(
        &mut file,
        "//! Backing data for composite properties (PackTab, compression={}, unsafe={})",
        config.compression, config.unsafe_access
    )
    .unwrap();
    writeln!(&mut file).unwrap();
    write!(&mut file, "{code}").unwrap();
}
