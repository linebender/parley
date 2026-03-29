// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `SplitString` tests.

use crate::test_name;
use crate::util::TestEnv;
use parley::editing::TextIndexEncoding;

#[test]
fn split_string_to_utf8_range_handles_hidden_compose_gap() {
    let mut env = TestEnv::new(test_name!(), None);
    let mut editor = env.editor("a🙂b");

    {
        let mut drv = env.driver(&mut editor);
        assert!(drv.set_document_selection(1..1));
        assert!(drv.update_composition("XYZ", None, Some(0..0)));
    }

    let text = editor.text();
    assert_eq!(text, "a🙂b");
    assert_eq!(
        text.to_utf8_range(1, 5, TextIndexEncoding::Utf8Bytes),
        Some(1..5)
    );
    assert_eq!(
        text.to_utf8_range(1, 3, TextIndexEncoding::Utf16CodeUnits),
        Some(1..5)
    );
    assert_eq!(
        text.to_utf8_range(1, 2, TextIndexEncoding::UnicodeCodePoints),
        Some(1..5)
    );
}

#[test]
fn split_string_to_utf8_range_rejects_reversed_and_out_of_bounds_offsets() {
    let mut env = TestEnv::new(test_name!(), None);
    let mut editor = env.editor("a🙂b");

    {
        let mut drv = env.driver(&mut editor);
        assert!(drv.set_document_selection(1..1));
        assert!(drv.update_composition("XYZ", None, Some(0..0)));
    }

    let text = editor.text();
    assert_eq!(text.to_utf8_range(5, 1, TextIndexEncoding::Utf8Bytes), None);
    assert_eq!(text.to_utf8_range(0, 7, TextIndexEncoding::Utf8Bytes), None);
    assert_eq!(
        text.to_utf8_range(0, 5, TextIndexEncoding::Utf16CodeUnits),
        None
    );
    assert_eq!(
        text.to_utf8_range(0, 5, TextIndexEncoding::UnicodeCodePoints),
        None
    );
}

#[test]
fn split_string_to_utf8_range_rejects_invalid_utf8_boundaries_across_segments() {
    let mut env = TestEnv::new(test_name!(), None);
    let mut editor = env.editor("🙂🙂");

    {
        let mut drv = env.driver(&mut editor);
        assert!(drv.set_document_selection(4..4));
        assert!(drv.update_composition("XYZ", None, Some(0..0)));
    }

    let text = editor.text();
    assert_eq!(text, "🙂🙂");
    assert_eq!(text.to_utf8_range(1, 4, TextIndexEncoding::Utf8Bytes), None);
    assert_eq!(text.to_utf8_range(4, 5, TextIndexEncoding::Utf8Bytes), None);
}

#[test]
fn split_string_to_utf8_range_handles_multibyte_utf16_range_across_segments() {
    let mut env = TestEnv::new(test_name!(), None);
    let mut editor = env.editor("é🙂界");

    {
        let mut drv = env.driver(&mut editor);
        assert!(drv.set_document_selection(6..6));
        assert!(drv.update_composition("XYZ", None, Some(0..0)));
    }

    let text = editor.text();
    assert_eq!(text, "é🙂界");
    assert_eq!(
        text.to_utf8_range(1, 4, TextIndexEncoding::Utf16CodeUnits),
        Some(2..9)
    );
}
