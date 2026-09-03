// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Tests for the cross-layout font cache owned by [`FontContext`].

use std::sync::Arc;

use fontique::{Collection, CollectionOptions, SourceCache};
use peniko::Blob;

use super::test_builders::{FONT_FAMILY_LIST, create_font_context, load_fonts};
use super::utils::ColorBrush;
use crate::{FontContext, FontFamily, Layout, LayoutContext, TextStyle};

fn build(fcx: &mut FontContext, text: &str) -> Layout<ColorBrush> {
    let mut lcx: LayoutContext<ColorBrush> = LayoutContext::new();
    let root = TextStyle {
        font_family: FontFamily::from(FONT_FAMILY_LIST),
        font_size: 20.,
        ..TextStyle::default()
    };
    let mut builder = lcx.tree_builder(fcx, 1., false, &root);
    builder.push_text(text);
    builder.push_style_span(TextStyle {
        font_size: 12.,
        ..root.clone()
    });
    builder.push_text("small");
    builder.pop_style_span();
    let (layout, _) = builder.build();
    layout
}

fn some_font_blob() -> Blob<u8> {
    let path = parley_dev::font_dirs()
        .flat_map(|dir| std::fs::read_dir(dir).unwrap())
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|ext| ext == "ttf"))
        .expect("at least one test font");
    Blob::new(Arc::new(std::fs::read(path).unwrap()))
}

#[test]
fn cache_is_populated_and_reused_across_layouts() {
    let mut fcx = create_font_context();
    assert_eq!(fcx.cache_len(), (0, 0));

    let first = build(&mut fcx, "hello");
    let (primary, metrics) = fcx.cache_len();
    assert!(primary >= 1, "primary font cache should be populated");
    assert!(metrics >= 2, "one metrics entry per (font, size)");

    // A second layout with the same styles must not add entries.
    let second = build(&mut fcx, "world");
    assert_eq!(fcx.cache_len(), (primary, metrics));
    assert_eq!(
        first.data.style_metrics.len(),
        second.data.style_metrics.len()
    );
    for (a, b) in first
        .data
        .style_metrics
        .iter()
        .zip(second.data.style_metrics.iter())
    {
        assert_eq!(a.ascent, b.ascent);
        assert_eq!(a.descent, b.descent);
    }
}

#[test]
fn unknown_family_caches_no_primary_font() {
    let mut fcx = create_font_context();
    let mut lcx: LayoutContext<ColorBrush> = LayoutContext::new();
    let root = TextStyle {
        font_family: FontFamily::named("No Such Family"),
        font_size: 20.,
        ..TextStyle::default()
    };
    let mut builder = lcx.tree_builder(&mut fcx, 1., false, &root);
    builder.push_text("hello");
    let _ = builder.build();
    // The (negative) query result is cached, but no metrics are since there is no primary font.
    assert_eq!(fcx.cache_len(), (1, 0));
}

#[test]
fn cache_is_cleared_when_collection_changes() {
    let mut fcx = create_font_context();
    build(&mut fcx, "hello");
    assert_ne!(fcx.cache_len(), (0, 0));

    // Mutating the collection bumps its generation; the cache is dropped the
    // next time it is synchronised with the collection.
    fcx.collection.register_fonts(some_font_blob(), None);
    let _ = fcx.query_and_cache();
    assert_eq!(fcx.cache_len(), (0, 0));

    // Rebuilding repopulates the cache.
    build(&mut fcx, "hello");
    let populated = fcx.cache_len();
    assert_ne!(populated, (0, 0));

    // Fallback changes also invalidate.
    fcx.collection
        .set_fallbacks(fontique::Script::from_bytes(*b"Latn"), core::iter::empty());
    let _ = fcx.query_and_cache();
    assert_eq!(fcx.cache_len(), (0, 0));

    build(&mut fcx, "hello");
    assert_eq!(fcx.cache_len(), populated);

    // Explicit clearing.
    fcx.clear_cache();
    assert_eq!(fcx.cache_len(), (0, 0));
}

#[test]
fn collection_generation_bumps_on_mutation() {
    let mut collection = Collection::new(CollectionOptions {
        shared: false,
        system_fonts: false,
    });
    let g0 = collection.generation();
    assert_eq!(collection.generation(), g0, "reading does not bump");

    load_fonts(&mut collection, parley_dev::font_dirs()).unwrap();
    let g1 = collection.generation();
    assert_ne!(g0, g1);

    collection.set_fallbacks(fontique::Script::from_bytes(*b"Latn"), core::iter::empty());
    let g2 = collection.generation();
    assert_ne!(g1, g2);

    let fcx = FontContext::from_parts(collection, SourceCache::default());
    assert_eq!(fcx.cache_len(), (0, 0));
}
