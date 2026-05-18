// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! See `./main.rs`.

use icu_properties::{
    CodePointMapData, CodePointSetData,
    props::{
        BidiClass, Emoji, EmojiComponent, EmojiModifier, EmojiModifierBase, EmojiPresentation,
        ExtendedPictographic, GeneralCategory, GraphemeClusterBreak, LineBreak, RegionalIndicator,
        Script, VariationSelector,
    },
};
use parley_data::{Properties, emoji::EmojiProperties};
use std::{collections::BTreeMap, fmt::Write as _};
use std::{
    io::{BufWriter, Write},
    ops::Range,
};

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
    // Dense characters table for 0..=0x10FFFF
    let mut characters = Vec::with_capacity(0x110000);
    let mut emojis = Vec::new();

    for cp in 0_u32..=0x10FFFF {
        let is_emoji = CodePointSetData::new::<Emoji>().contains32(cp);
        let is_extended_pictographic =
            CodePointSetData::new::<ExtendedPictographic>().contains32(cp);
        let is_emoji_component = CodePointSetData::new::<EmojiComponent>().contains32(cp);
        let is_regional_indicator = CodePointSetData::new::<RegionalIndicator>().contains32(cp);

        let v = Properties::new(
            CodePointMapData::<Script>::new().get32(cp),
            CodePointMapData::<GeneralCategory>::new().get32(cp),
            CodePointMapData::<GraphemeClusterBreak>::new().get32(cp),
            CodePointMapData::<BidiClass>::new().get32(cp),
            is_emoji || is_extended_pictographic,
            CodePointSetData::new::<VariationSelector>().contains32(cp),
            is_regional_indicator,
            // See: https://github.com/unicode-org/icu4x/blob/ee5399a77a6b94efb5d4b60678bb458c5eedb25d/components/segmenter/src/line.rs#L338-L351
            matches!(
                CodePointMapData::<LineBreak>::new().get32(cp),
                LineBreak::MandatoryBreak
                    | LineBreak::CarriageReturn
                    | LineBreak::LineFeed
                    | LineBreak::NextLine
            ),
        );
        characters.push(u32::from(v) as i64);

        // See: https://unicode.org/reports/tr51/#Emoji_Characters
        if is_emoji || is_extended_pictographic || is_emoji_component {
            let emoji_properties = EmojiProperties::new(
                is_emoji,
                is_extended_pictographic,
                is_emoji_component,
                CodePointSetData::new::<EmojiPresentation>().contains32(cp),
                CodePointSetData::new::<EmojiModifier>().contains32(cp),
                CodePointSetData::new::<EmojiModifierBase>().contains32(cp),
                is_regional_indicator,
            );
            emojis.push((cp, u32::from(emoji_properties)));
        }
    }

    let (info, best) = packtab::pack_table(&characters, Some(0), config.compression);

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

    let code_extra = generate_emojis(&emojis);

    writeln!(&mut file).unwrap();
    write!(&mut file, "{code_extra}").unwrap();
}

fn generate_emojis(emojis: &[(u32, u32)]) -> String {
    let mut emoji_map = BTreeMap::<u32, Vec<u32>>::new();

    for (c, b) in emojis.iter().copied() {
        emoji_map
            .entry(b)
            .and_modify(|e| e.push(c))
            .or_insert_with(|| vec![c]);
    }

    let emoji_count = emoji_map.len();
    let mut emoji_bits = Vec::with_capacity(emoji_count);
    let mut code_emoji_matches = String::new();

    for (b, mut a) in emoji_map {
        a.sort();

        let mut v = Vec::<Range<u32>>::new();

        for c in a {
            if let Some(last) = v.last_mut() {
                if c - last.end == 1 {
                    last.end = c;
                    continue;
                }
            }
            v.push(c..c);
        }

        let i = emoji_bits.len();

        let mut s = String::new();

        for r in v {
            let start = r.start;
            let end = r.end;
            let is_single = end == start;

            if is_single {
                s.push_str(&format!("{start:#X}"));
            } else {
                s.push_str(&format!("{start:#X}..={end:#X}"));
            }
            s.push('|');
        }

        s.pop();
        s.push_str(&format!(" => {i},"));

        emoji_bits.push(b);
        code_emoji_matches.push_str(&s);
    }

    let mut code_extra = String::new();

    code_extra.push_str(&format!(
        "
static EMOJI_COMPOSITE_U8: [u8; {emoji_count}] = {emoji_bits:#?};

#[allow(missing_docs, reason = \"generated code\")]
#[inline]
pub const fn emoji_composite_get(cp: u32) -> u32 {{
    let idx = match cp {{
        {code_emoji_matches}
        _ => return 0,
    }};

    EMOJI_COMPOSITE_U8[idx as usize] as u32
}}
"
    ));

    code_extra
}
