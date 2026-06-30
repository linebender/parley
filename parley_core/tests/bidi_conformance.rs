// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! UAX#9 conformance tests against the Unicode Consortium's authoritative test data.
//!
//! Runs `BidiResolver` against `BidiTest.txt` and `BidiCharacterTest.txt`,
//! both pinned to Unicode 16.0.0 (matching `icu_properties 2.x`'s bundled data).

use std::path::Path;

use icu_properties::{
    CodePointMapData,
    props::{BidiClass, BidiMirroringGlyph},
};
use parley_core::bidi::{BidiLevel, BidiResolver};

/// The Unicode version these test files must be pinned to.
const EXPECTED_UCD_VERSION: &str = "16.0.0";

// ---------------------------------------------------------------------------
// Helpers: abbreviation -> BidiClass
// ---------------------------------------------------------------------------

fn abbrev_to_bidi_class(abbrev: &str) -> BidiClass {
    match abbrev {
        "L" => BidiClass::LeftToRight,
        "R" => BidiClass::RightToLeft,
        "AL" => BidiClass::ArabicLetter,
        "EN" => BidiClass::EuropeanNumber,
        "ES" => BidiClass::EuropeanSeparator,
        "ET" => BidiClass::EuropeanTerminator,
        "CS" => BidiClass::CommonSeparator,
        "NSM" => BidiClass::NonspacingMark,
        "B" => BidiClass::ParagraphSeparator,
        "S" => BidiClass::SegmentSeparator,
        "WS" => BidiClass::WhiteSpace,
        "ON" => BidiClass::OtherNeutral,
        "LRE" => BidiClass::LeftToRightEmbedding,
        "RLE" => BidiClass::RightToLeftEmbedding,
        "LRO" => BidiClass::LeftToRightOverride,
        "RLO" => BidiClass::RightToLeftOverride,
        "PDF" => BidiClass::PopDirectionalFormat,
        "LRI" => BidiClass::LeftToRightIsolate,
        "RLI" => BidiClass::RightToLeftIsolate,
        "FSI" => BidiClass::FirstStrongIsolate,
        "PDI" => BidiClass::PopDirectionalIsolate,
        "BN" => BidiClass::BoundaryNeutral,
        "AN" => BidiClass::ArabicNumber,
        other => panic!("unknown BidiClass abbreviation: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Parse the version from the first line header comment.
// Expected form: "# BidiTest-16.0.0.txt" or "# BidiCharacterTest-16.0.0.txt"
// ---------------------------------------------------------------------------

fn parse_header_version(first_line: &str) -> Option<&str> {
    // Strip leading "# " and trailing ".txt", then find the version after the last '-'
    let stripped = first_line.trim().strip_prefix('#')?.trim();
    // e.g. "BidiTest-16.0.0.txt" or "BidiCharacterTest-16.0.0.txt"
    let without_ext = stripped.strip_suffix(".txt")?;
    // Find last '-' which separates name from version
    let pos = without_ext.rfind('-')?;
    Some(&without_ext[pos + 1..])
}

// ---------------------------------------------------------------------------
// Test: BidiCharacterTest.txt
// ---------------------------------------------------------------------------

#[test]
fn bidi_character_test_txt() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/BidiCharacterTest.txt");
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));

    let bidi_map = CodePointMapData::<BidiClass>::new();
    let bracket_map = CodePointMapData::<BidiMirroringGlyph>::new();

    let mut resolver = BidiResolver::new();
    let mut failures: Vec<String> = Vec::new();
    let mut case_count = 0u64;

    let mut lines = raw.lines().enumerate();

    // Check header version on the first line.
    let (_, first_line) = lines.next().expect("file is not empty");
    let version = parse_header_version(first_line)
        .unwrap_or_else(|| panic!("could not parse version from header: {first_line:?}"));
    assert_eq!(
        version, EXPECTED_UCD_VERSION,
        "BidiCharacterTest.txt header version {version:?} != expected {EXPECTED_UCD_VERSION:?}. \
         Bump EXPECTED_UCD_VERSION or re-vendor the data files."
    );

    for (line_no, line) in lines {
        let line_no = line_no + 1; // 1-based for error messages (we consumed line 0 above)
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Fields: code_points ; para_dir ; expected_level ; expected_levels ; expected_order
        let fields: Vec<&str> = line.splitn(5, ';').collect();
        if fields.len() < 4 {
            panic!("line {line_no}: expected at least 4 semicolon-separated fields, got: {line:?}");
        }

        // Field 0: space-separated hex code points
        let chars: Vec<char> = fields[0]
            .split_whitespace()
            .map(|hex| {
                let cp = u32::from_str_radix(hex, 16)
                    .unwrap_or_else(|_| panic!("line {line_no}: invalid hex {hex:?}"));
                char::from_u32(cp)
                    .unwrap_or_else(|| panic!("line {line_no}: invalid code point U+{cp:04X}"))
            })
            .collect();

        // Field 1: paragraph direction
        let para_dir_field = fields[1].trim();
        let para_dir: Option<BidiLevel> = match para_dir_field {
            "0" => Some(0),
            "1" => Some(1),
            "2" => None,
            other => panic!("line {line_no}: unknown paragraph direction {other:?}"),
        };

        // Field 2: expected resolved paragraph level
        let expected_base_level: BidiLevel = fields[2]
            .trim()
            .parse()
            .unwrap_or_else(|_| panic!("line {line_no}: invalid base level: {:?}", fields[2]));

        // Field 3: space-separated expected levels, 'x' for X9-removed
        let expected_levels: Vec<Option<BidiLevel>> =
            fields[3]
                .split_whitespace()
                .map(|tok| {
                    if tok == "x" {
                        None
                    } else {
                        Some(tok.parse::<BidiLevel>().unwrap_or_else(|_| {
                            panic!("line {line_no}: invalid level token {tok:?}")
                        }))
                    }
                })
                .collect();

        assert_eq!(
            chars.len(),
            expected_levels.len(),
            "line {line_no}: chars.len()={} != expected_levels.len()={}",
            chars.len(),
            expected_levels.len()
        );

        // Build resolver input
        let input: Vec<(char, (BidiClass, BidiMirroringGlyph))> = chars
            .iter()
            .map(|&ch| (ch, (bidi_map.get(ch), bracket_map.get(ch))))
            .collect();

        resolver.resolve(input.into_iter(), para_dir);
        case_count += 1;

        let actual_levels = resolver.levels();

        // Check base level
        if resolver.base_level() != expected_base_level {
            let msg = format!(
                "line {line_no}: base level mismatch: expected={expected_base_level}, \
                 actual={}, input={:?}",
                resolver.base_level(),
                fields[0].trim()
            );
            failures.push(msg);
            continue;
        }

        // Check levels at non-x positions
        for (i, (exp, &act)) in expected_levels.iter().zip(actual_levels.iter()).enumerate() {
            let Some(exp) = *exp else { continue };
            if exp != act {
                let msg = format!(
                    "line {line_no}: level mismatch at char {i}: expected={exp}, actual={act}, \
                     input={:?}, all_expected={:?}, all_actual={:?}",
                    fields[0].trim(),
                    fields[3].trim(),
                    actual_levels
                );
                failures.push(msg);
                break;
            }
        }
    }

    println!(
        "bidi_character_test_txt: ran {case_count} cases, {} failures",
        failures.len()
    );

    if !failures.is_empty() {
        let mut msg = format!(
            "BidiCharacterTest.txt: {} failure(s) out of {case_count} cases\n",
            failures.len()
        );
        for f in failures.iter().take(20) {
            msg.push_str("  ");
            msg.push_str(f);
            msg.push('\n');
        }
        if failures.len() > 20 {
            msg.push_str(&format!("  ... and {} more\n", failures.len() - 20));
        }
        panic!("{msg}");
    }
}

// ---------------------------------------------------------------------------
// Test: BidiTest.txt
// ---------------------------------------------------------------------------

#[test]
fn bidi_test_txt() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/BidiTest.txt");
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));

    let bracket_map = CodePointMapData::<BidiMirroringGlyph>::new();
    // 'A' (U+0041) has no bracket pairing; use its BidiMirroringGlyph as the
    // neutral glyph to pair with every synthesized element in BidiTest.txt
    // (which explicitly has no bracket cases).
    let neutral_glyph = bracket_map.get('A');

    let mut resolver = BidiResolver::new();
    let mut failures: Vec<String> = Vec::new();
    let mut case_count = 0u64;

    let mut current_levels: Vec<Option<u8>> = Vec::new();
    // current_reorder is ignored (stretch goal)

    let mut lines = raw.lines().enumerate();

    // Check header version on the first line.
    let (_, first_line) = lines.next().expect("file is not empty");
    let version = parse_header_version(first_line)
        .unwrap_or_else(|| panic!("could not parse version from header: {first_line:?}"));
    assert_eq!(
        version, EXPECTED_UCD_VERSION,
        "BidiTest.txt header version {version:?} != expected {EXPECTED_UCD_VERSION:?}. \
         Bump EXPECTED_UCD_VERSION or re-vendor the data files."
    );

    for (line_no, line) in lines {
        let line_no = line_no + 1;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(rest) = line.strip_prefix('@') {
            if let Some(levels_str) = rest.strip_prefix("Levels:") {
                current_levels = levels_str
                    .split_whitespace()
                    .map(|tok| {
                        if tok == "x" {
                            None
                        } else {
                            Some(tok.parse::<u8>().unwrap_or_else(|_| {
                                panic!("line {line_no}: invalid level token {tok:?}")
                            }))
                        }
                    })
                    .collect();
            }
            // @Reorder: and any other @ lines are ignored (stretch goal / forward compat)
            continue;
        }

        // Data line: "<space-separated abbrevs> ; <hex bitset>"
        let Some((input_part, bitset_part)) = line.split_once(';') else {
            panic!("line {line_no}: data line has no ';': {line:?}");
        };

        let classes: Vec<BidiClass> = input_part
            .split_whitespace()
            .map(|abbrev| abbrev_to_bidi_class(abbrev))
            .collect();

        if classes.is_empty() {
            continue;
        }

        let bitset = u8::from_str_radix(bitset_part.trim(), 16)
            .unwrap_or_else(|_| panic!("line {line_no}: invalid bitset {:?}", bitset_part.trim()));

        assert_eq!(
            classes.len(),
            current_levels.len(),
            "line {line_no}: classes.len()={} != current_levels.len()={}: \
             check that @Levels was seen before data",
            classes.len(),
            current_levels.len()
        );

        // Build the synthesized input chars: ('A', (class, neutral_glyph))
        let input: Vec<(char, (BidiClass, BidiMirroringGlyph))> = classes
            .iter()
            .map(|&class| ('A', (class, neutral_glyph)))
            .collect();

        // Run once per set bit in the bitset.
        // Bit 1 => auto (None), bit 2 => LTR (Some(0)), bit 4 => RTL (Some(1))
        for (bit, para_dir) in [(1u8, None), (2u8, Some(0u8)), (4u8, Some(1u8))] {
            if bitset & bit == 0 {
                continue;
            }

            resolver.resolve(input.iter().copied(), para_dir);
            case_count += 1;

            let actual_levels = resolver.levels();

            // Compare levels at non-x positions
            for (i, (exp, &act)) in current_levels.iter().zip(actual_levels.iter()).enumerate() {
                let Some(exp) = *exp else { continue };
                if exp != act {
                    let dir_label = match para_dir {
                        None => "auto",
                        Some(0) => "LTR",
                        Some(1) => "RTL",
                        _ => "?",
                    };
                    failures.push(format!(
                        "line {line_no} ({dir_label}): level mismatch at char {i}: \
                         expected={exp}, actual={act}, \
                         input={input_part:?}, \
                         expected_levels={current_levels:?}, \
                         actual_levels={actual_levels:?}"
                    ));
                    break;
                }
            }
        }
    }

    println!(
        "bidi_test_txt: ran {case_count} cases, {} failures",
        failures.len()
    );

    if !failures.is_empty() {
        let mut msg = format!(
            "BidiTest.txt: {} failure(s) out of {case_count} cases\n",
            failures.len()
        );
        for f in failures.iter().take(20) {
            msg.push_str("  ");
            msg.push_str(f);
            msg.push('\n');
        }
        if failures.len() > 20 {
            msg.push_str(&format!("  ... and {} more\n", failures.len() - 20));
        }
        panic!("{msg}");
    }
}
