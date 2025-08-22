//! # Parley Bench
//!
//! This crate provides a benchmark for the Parley library.

use std::sync::{Mutex, MutexGuard, OnceLock};

use parley::{FontContext, LayoutContext};

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
    let font_cx = FONT_CX.get_or_init(|| Mutex::new(FontContext::new()));
    let layout_cx = LAYOUT_CX.get_or_init(|| Mutex::new(LayoutContext::new()));
    (font_cx.lock().unwrap(), layout_cx.lock().unwrap())
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
    let arabic = include_str!("../samples/arabic.txt");
    let latin = include_str!("../samples/latin.txt");
    let japanese = include_str!("../samples/japanese.txt");

    SAMPLES.get_or_init(|| {
        vec![
            Sample {
                name: "arabic",
                modification: "all",
                text: arabic,
            },
            //Sample {
            //    name: "latin",
            //    modification: "all",
            //    text: latin,
            //},
            //Sample {
            //    name: "japanese",
            //    modification: "all",
            //    text: japanese,
            //},
            //Sample {
            //    name: "arabic",
            //    modification: "1 paragraph",
            //    text: arabic.lines().next().unwrap(),
            //},
            //Sample {
            //    name: "latin",
            //    modification: "1 paragraph",
            //    text: latin.lines().next().unwrap(),
            //},
            //Sample {
            //    name: "japanese",
            //    modification: "1 paragraph",
            //    text: japanese.lines().next().unwrap(),
            //},
        ]
    })
}
