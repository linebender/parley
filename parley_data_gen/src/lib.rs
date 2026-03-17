// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! See `./main.rs`.

use icu_properties::props::{GeneralCategory, GraphemeClusterBreak, Script};
use icu_properties::{
    CodePointMapData, CodePointSetData,
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
    // Generate the data required for `CompositeProps`.
    let values = {
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
        values
    };
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
