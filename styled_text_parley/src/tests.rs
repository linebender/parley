// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::sync::Arc;

use parley::style::{FontFamily as ParleyFontFamily, FontStack as ParleyFontStack, LineHeight};
use text_style::{FontFamily, FontStack, GenericFamily, Tag};

use crate::convert::{to_parley_font_stack, to_parley_line_height, to_parley_tag};

#[test]
fn converts_font_stack_to_parley_list() {
    let mut stack = FontStack::new();
    stack.push(FontFamily::named(Arc::from("Inter")));
    stack.push(FontFamily::Generic(GenericFamily::SansSerif));

    let parley = to_parley_font_stack(&stack);
    let ParleyFontStack::List(list) = parley else {
        panic!("expected list font stack");
    };
    assert_eq!(list.len(), 2);
    assert!(matches!(
        &list[0],
        ParleyFontFamily::Named(name) if name.as_ref() == "Inter"
    ));
}

#[test]
fn converts_tags_by_bytes() {
    let tag = Tag::from_bytes(*b"wght");
    let parley = to_parley_tag(tag);
    assert_eq!(parley.into_bytes(), *b"wght");
}

#[test]
fn converts_line_height_variants() {
    assert_eq!(
        to_parley_line_height(text_style_resolve::ComputedLineHeight::MetricsRelative(1.0)),
        LineHeight::MetricsRelative(1.0)
    );
    assert_eq!(
        to_parley_line_height(text_style_resolve::ComputedLineHeight::FontSizeRelative(
            1.5
        )),
        LineHeight::FontSizeRelative(1.5)
    );
    assert_eq!(
        to_parley_line_height(text_style_resolve::ComputedLineHeight::Px(12.0)),
        LineHeight::Absolute(12.0)
    );
}
