// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use parley::style::LineHeight;
use styled_text::{FontFamily, FontFamilyName, GenericFamily, Tag};

use crate::convert::to_parley_line_height;

#[test]
fn converts_font_family_to_parley_font_family() {
    static NAMES: [FontFamilyName; 2] = [
        FontFamilyName::named("Inter"),
        FontFamilyName::Generic(GenericFamily::SansSerif),
    ];
    let family = FontFamily::from(&NAMES[..]);
    let parley = family;
    assert!(matches!(
        parley,
        parley::FontFamily::List(list)
        if matches!(&list[0], parley::FontFamilyName::Named(name) if name.as_ref() == "Inter")
    ));
}

#[test]
fn converts_tags_by_bytes() {
    let tag = Tag::new(b"wght");
    let parley = tag;
    assert_eq!(parley.to_bytes(), *b"wght");
}

#[test]
fn converts_line_height_variants() {
    assert_eq!(
        to_parley_line_height(styled_text::ComputedLineHeight::MetricsRelative(1.0)),
        LineHeight::MetricsRelative(1.0)
    );
    assert_eq!(
        to_parley_line_height(styled_text::ComputedLineHeight::FontSizeRelative(1.5)),
        LineHeight::FontSizeRelative(1.5)
    );
    assert_eq!(
        to_parley_line_height(styled_text::ComputedLineHeight::Px(12.0)),
        LineHeight::Absolute(12.0)
    );
}
