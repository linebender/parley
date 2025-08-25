//! # Parley Bench
//!
//! This crate provides benchmarks for the Parley library.

use std::{
    borrow::Cow,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, OnceLock},
};

use parley::{
    FontContext, FontFamily, FontStack, LayoutContext, RangedBuilder, StyleProperty,
    fontique::{Blob, Collection, CollectionOptions},
};

pub mod default_style;

/// A color brush.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorBrush {}

impl Default for ColorBrush {
    fn default() -> Self {
        Self {}
    }
}

// Since Tango runs benchmarks consecutively, no two benchmarks running from the same revision will have to
// wait for the mutex to be available.
static FONT_CX: OnceLock<Mutex<FontContext>> = OnceLock::new();
static LAYOUT_CX: OnceLock<Mutex<LayoutContext<ColorBrush>>> = OnceLock::new();

/// Returns a tuple of font and layout contexts.
pub fn get_contexts() -> (
    MutexGuard<'static, FontContext>,
    MutexGuard<'static, LayoutContext<ColorBrush>>,
) {
    let font_cx = FONT_CX.get_or_init(|| Mutex::new(create_font_context()));
    let layout_cx = LAYOUT_CX.get_or_init(|| Mutex::new(LayoutContext::new()));
    (font_cx.lock().unwrap(), layout_cx.lock().unwrap())
}

pub(crate) fn create_font_context() -> FontContext {
    let mut collection = Collection::new(CollectionOptions {
        shared: false,
        system_fonts: false,
    });
    load_fonts(&mut collection, parley_dev::font_dirs()).unwrap();
    for font in FONT_STACK {
        if let FontFamily::Named(font_name) = font {
            collection
                .family_id(font_name)
                .unwrap_or_else(|| panic!("{font_name} font not found"));
        }
    }
    FontContext {
        collection,
        source_cache: Default::default(),
    }
}

pub(crate) fn apply_default_style(builder: &mut RangedBuilder<'_, ColorBrush>) {
    builder.push_default(StyleProperty::Brush(ColorBrush {}));
    builder.push_default(StyleProperty::FontStack(FontStack::List(FONT_STACK.into())));
}

pub(crate) const FONT_STACK: &[FontFamily<'_>] = &[
    FontFamily::Named(Cow::Borrowed("Roboto")),
    FontFamily::Named(Cow::Borrowed("Noto Kufi Arabic")),
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
    pub text: &'static str,
}

static SAMPLES: OnceLock<Vec<Sample>> = OnceLock::new();

/// Returns a list of samples to be used for benchmarking.
pub fn get_samples() -> &'static [Sample] {
    let samples = parley_dev::TextSamples::new();

    SAMPLES.get_or_init(|| {
        vec![
            Sample {
                name: samples.arabic.name,
                modification: "all",
                text: samples.arabic.text,
            },
            Sample {
                name: samples.latin.name,
                modification: "all",
                text: samples.latin.text,
            },
            Sample {
                name: samples.japanese.name,
                modification: "all",
                text: samples.japanese.text,
            },
            Sample {
                name: samples.arabic.name,
                modification: "1 paragraph",
                text: samples.arabic.text.lines().next().unwrap(),
            },
            Sample {
                name: samples.latin.name,
                modification: "1 paragraph",
                text: samples.latin.text.lines().next().unwrap(),
            },
            Sample {
                name: samples.japanese.name,
                modification: "1 paragraph",
                text: samples.japanese.text.lines().next().unwrap(),
            },
        ]
    })
}
