// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Tests for [`parley_core`]'s analysis API.

use parlance::BaseDirection;
use parley_core::{Analysis, AnalysisOptions, Analyzer, Boundary};

#[test]
fn trivial_inputs_produce_trivial_output() {
    let mut analyzer = Analyzer::new();
    let mut analysis = Analysis::new();

    // Empty text: every output buffer is empty and the base level is 0.
    analyzer.analyze("", &AnalysisOptions::default(), &mut analysis);
    assert!(analysis.char_infos().is_empty());
    assert!(analysis.bidi_levels().is_empty());
    assert_eq!(analysis.paragraph_level(), 0);

    // All-LTR text: bidi levels stay empty and the base level is still 0.
    let text = "The quick brown fox";
    analyzer.analyze(text, &AnalysisOptions::default(), &mut analysis);
    assert_eq!(analysis.char_infos().len(), text.chars().count());
    assert!(analysis.bidi_levels().is_empty());
    assert_eq!(analysis.paragraph_level(), 0);
}

#[test]
fn mandatory_break_after_newline() {
    let mut analyzer = Analyzer::new();
    let mut analysis = Analysis::new();
    let text = "a\nb";
    analyzer.analyze(text, &AnalysisOptions::default(), &mut analysis);
    let infos = analysis.char_infos();
    // The newline is a control character...
    assert!(infos[1].is_control());
    assert!(!infos[1].contributes_to_shaping());
    // ...and the boundary *before* the following character is mandatory.
    assert_eq!(infos[2].boundary(), Boundary::Mandatory);
}

#[test]
fn forced_rtl_base_direction() {
    let mut analyzer = Analyzer::new();
    let mut analysis = Analysis::new();
    let text = "hello مرحبا world";
    let options = AnalysisOptions {
        base_direction: BaseDirection::Rtl,
        ..Default::default()
    };
    analyzer.analyze(text, &options, &mut analysis);
    // A forced RTL base level resolves even when the first strong char is LTR.
    assert_eq!(analysis.paragraph_level(), 1);
    assert_eq!(analysis.bidi_levels().len(), text.chars().count());
}
