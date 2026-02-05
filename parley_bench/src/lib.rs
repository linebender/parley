// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! # Parley Bench
//!
//! This crate provides benchmarks for the Parley library.

use std::{
    borrow::Cow,
    cell::RefCell,
    path::PathBuf,
    sync::{Arc, OnceLock},
};

use parley::{
    FontContext, FontFamilyName, LayoutContext,
    fontique::{Blob, Collection, CollectionOptions, SourceCache},
};

pub mod benches;
pub mod fontique_benches;

/// A color brush.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ColorBrush {}

std::thread_local! {
    static FONT_CX_TL: RefCell<FontContext> = RefCell::new(create_font_context());
    static LAYOUT_CX_TL: RefCell<LayoutContext<ColorBrush>> = RefCell::new(LayoutContext::new());
}

/// Runs the provided closure with mutable access to the thread-local font and layout contexts.
pub fn with_contexts<R>(
    f: impl FnOnce(&mut FontContext, &mut LayoutContext<ColorBrush>) -> R,
) -> R {
    FONT_CX_TL.with(|font_cell| {
        LAYOUT_CX_TL.with(|layout_cell| {
            let mut font_cx = font_cell.borrow_mut();
            let mut layout_cx = layout_cell.borrow_mut();
            f(&mut font_cx, &mut layout_cx)
        })
    })
}

pub(crate) fn create_font_context() -> FontContext {
    let mut collection = Collection::new(CollectionOptions {
        shared: false,
        system_fonts: false,
    });
    load_fonts(&mut collection, parley_dev::font_dirs()).unwrap();
    for font in FONT_FAMILY_LIST {
        if let FontFamilyName::Named(font_name) = font {
            collection
                .family_id(font_name)
                .unwrap_or_else(|| panic!("{font_name} font not found"));
        }
    }
    FontContext {
        collection,
        source_cache: SourceCache::default(),
    }
}

pub(crate) const FONT_FAMILY_LIST: &[FontFamilyName<'_>] = &[
    FontFamilyName::Named(Cow::Borrowed("Roboto")),
    FontFamilyName::Named(Cow::Borrowed("Noto Kufi Arabic")),
];

pub(crate) fn load_fonts(
    collection: &mut Collection,
    font_dirs: impl Iterator<Item = PathBuf>,
) -> std::io::Result<()> {
    for dir in font_dirs {
        let paths = std::fs::read_dir(dir)?;
        for entry in paths {
            let entry = entry?;
            if !entry.metadata()?.is_file() {
                continue;
            }
            let path = entry.path();
            if path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_none_or(|ext| !["ttf", "otf", "ttc", "otc"].contains(&ext))
            {
                continue;
            }
            let font_data = std::fs::read(&path)?;
            collection.register_fonts(Blob::new(Arc::new(font_data)), None);
        }
    }
    Ok(())
}

/// A sample to be used for benchmarking.
#[derive(Debug)]
pub struct Sample {
    /// The name of the sample.
    pub name: &'static str,
    /// The modification of the sample.
    pub modification: &'static str,
    /// The text of the sample.
    pub text: String,
}

static SAMPLES: OnceLock<Vec<Sample>> = OnceLock::new();

/// Returns a list of samples to be used for benchmarking.
pub fn get_samples() -> &'static [Sample] {
    let samples = parley_dev::TextSamples::new();

    SAMPLES.get_or_init(|| {
        vec![
            Sample {
                name: samples.arabic.name,
                modification: "20 characters",
                text: samples.arabic.text.chars().take(20).collect(),
            },
            Sample {
                name: samples.latin.name,
                modification: "20 characters",
                text: samples.latin.text.chars().take(20).collect(),
            },
            Sample {
                name: samples.japanese.name,
                modification: "20 characters",
                text: samples.japanese.text.chars().take(20).collect(),
            },
            Sample {
                name: samples.arabic.name,
                modification: "1 paragraph",
                text: samples.arabic.text.lines().next().unwrap().to_string(),
            },
            Sample {
                name: samples.latin.name,
                modification: "1 paragraph",
                text: samples.latin.text.lines().next().unwrap().to_string(),
            },
            Sample {
                name: samples.japanese.name,
                modification: "1 paragraph",
                text: samples.japanese.text.lines().next().unwrap().to_string(),
            },
            Sample {
                name: samples.arabic.name,
                modification: "4 paragraph",
                text: samples
                    .arabic
                    .text
                    .lines()
                    .take(4)
                    .collect::<Vec<_>>()
                    .join("\n"),
            },
            Sample {
                name: samples.latin.name,
                modification: "4 paragraph",
                text: samples
                    .latin
                    .text
                    .lines()
                    .take(4)
                    .collect::<Vec<_>>()
                    .join("\n"),
            },
            Sample {
                name: samples.japanese.name,
                modification: "4 paragraph",
                text: samples
                    .japanese
                    .text
                    .lines()
                    .take(4)
                    .collect::<Vec<_>>()
                    .join("\n"),
            },
        ]
    })
}
