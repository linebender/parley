// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Shared layout config, building, and output path for vello_cpu_render and vello_hybrid_render.

use std::collections::HashMap;
use std::ops::Range;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use parley::fontique::Blob;
use parley::{
    Alignment, AlignmentOptions, FontContext, FontFamily, FontWeight, GenericFamily, InlineBox,
    Layout, LayoutContext, LineHeight, StyleProperty,
};
use peniko::Color;

pub const DEFAULT_TEXT: &str = "Some text here. Let's make it a bit longer so that line wrapping kicks in. Bitmap emoji 😊 and COLR emoji 🎉.\nAnd also some اللغة العربية arabic text.\nThis is underline and strikethrough text.";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorBrush {
    pub color: Color,
}

impl Default for ColorBrush {
    fn default() -> Self {
        Self {
            color: Color::BLACK,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ExampleConfig {
    pub text: String,
    pub display_scale: f32,
    pub quantize: bool,
    pub max_advance: Option<f32>,
    pub foreground_color: Color,
    pub background_color: Color,
    pub padding: u32,
}

impl Default for ExampleConfig {
    fn default() -> Self {
        Self {
            text: DEFAULT_TEXT.to_owned(),
            display_scale: 1.0,
            quantize: true,
            max_advance: Some(200.0),
            foreground_color: Color::BLACK,
            background_color: Color::WHITE,
            padding: 20,
        }
    }
}

/// Build the example layout with default config and COLR font path.
/// Returns `(layout, padded_width, padded_height, config)`.
pub fn prepare_example_layout() -> (Layout<ColorBrush>, u16, u16, ExampleConfig) {
    let config = ExampleConfig::default();
    let mut font_cx = FontContext::new();
    let mut layout_cx = LayoutContext::new();
    let (layout, padded_width, padded_height) = build_example_layout(
        &mut font_cx,
        &mut layout_cx,
        &config,
        &default_colr_font_path(),
    );
    (layout, padded_width, padded_height, config)
}

/// Default path to the COLR emoji font (relative to workspace).
pub fn default_colr_font_path() -> PathBuf {
    PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../parley_dev/assets/fonts/noto_color_emoji/NotoColorEmoji-Subset.ttf"
    ))
}

/// Load COLR emoji font from `colr_font_path` into `font_cx`, then build the
/// example layout. Returns the layout and padded dimensions.
pub fn build_example_layout(
    font_cx: &mut FontContext,
    layout_cx: &mut LayoutContext<ColorBrush>,
    config: &ExampleConfig,
    colr_font_path: &Path,
) -> (Layout<ColorBrush>, u16, u16) {
    let colr_font_data = std::fs::read(colr_font_path).expect("Failed to load COLR font");
    font_cx
        .collection
        .register_fonts(Blob::new(Arc::new(colr_font_data)), None);

    let (underline_range, strikethrough_range, party_emoji_range) = style_ranges(&config.text);

    let mut builder =
        layout_cx.ranged_builder(font_cx, &config.text, config.display_scale, config.quantize);

    let foreground_brush = ColorBrush {
        color: config.foreground_color,
    };
    builder.push_default(StyleProperty::Brush(foreground_brush));
    builder.push_default(GenericFamily::SystemUi);
    builder.push_default(LineHeight::FontSizeRelative(1.3));
    builder.push_default(StyleProperty::FontSize(16.0));

    let bold = FontWeight::new(600.0);
    builder.push(StyleProperty::FontWeight(bold), 0..4);
    builder.push(StyleProperty::Underline(true), underline_range);
    builder.push(StyleProperty::Strikethrough(true), strikethrough_range);
    builder.push(FontFamily::named("Noto Color Emoji"), party_emoji_range);
    builder.push_inline_box(InlineBox {
        id: 0,
        index: 40,
        width: 50.0,
        height: 50.0,
    });

    let mut layout = builder.build(&config.text);
    layout.break_all_lines(config.max_advance);
    layout.align(
        config.max_advance,
        Alignment::Start,
        AlignmentOptions::default(),
    );

    let width = layout.width().ceil() as u16;
    let height = layout.height().ceil() as u16;
    let padded_width = width + (config.padding * 2) as u16;
    let padded_height = height + (config.padding * 2) as u16;

    (layout, padded_width, padded_height)
}

/// Underline, strikethrough, party-emoji ranges (byte ranges into the config text).
pub fn style_ranges(text: &str) -> (Range<usize>, Range<usize>, Range<usize>) {
    let underline_range = {
        let (start, matched) = text.match_indices("underline").next().unwrap();
        start..start + matched.len()
    };
    let strikethrough_range = {
        let (start, matched) = text.match_indices("strikethrough").next().unwrap();
        start..start + matched.len()
    };
    let party_emoji_range = {
        let (start, matched) = text.match_indices("🎉").next().unwrap();
        start..start + matched.len()
    };
    (underline_range, strikethrough_range, party_emoji_range)
}

/// Directory for example output: `examples/_output`. Call with `file!()` from the
/// example binary so we pop the right number of path components.
pub fn output_dir(crate_file: &str) -> PathBuf {
    let mut path = PathBuf::from(crate_file);
    if let Ok(canon) = std::fs::canonicalize(&path) {
        path = canon;
    }
    for _ in 0..3 {
        path.pop();
    }
    path.push("_output");
    let _ = std::fs::create_dir(&path);
    path
}

/// Frame timing statistics tracker with automatic timing support.
pub struct FrameStats {
    // Per-label timing data (label -> vec of durations)
    timings: HashMap<String, Vec<Duration>>,
    // Active timers (label -> start instant)
    active_timers: HashMap<String, Instant>,
    // Accumulators for labels in accumulation mode (label -> accumulated duration)
    accumulators: HashMap<String, Duration>,
}

impl Default for FrameStats {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameStats {
    pub fn new() -> Self {
        Self {
            timings: HashMap::new(),
            active_timers: HashMap::new(),
            accumulators: HashMap::new(),
        }
    }

    /// Start timing for a labeled section. Call `end()` with the same label to record the duration.
    pub fn start(&mut self, label: impl Into<String>) {
        let label = label.into();
        self.active_timers.insert(label, Instant::now());
    }

    /// End timing for a labeled section and record the duration.
    /// If the label is in accumulation mode, adds to the accumulator instead.
    pub fn end(&mut self, label: impl Into<String>) {
        let label = label.into();
        if let Some(start) = self.active_timers.remove(&label) {
            let duration = start.elapsed();

            // Check if this label is in accumulation mode
            if let Some(accumulator) = self.accumulators.get_mut(&label) {
                *accumulator += duration; // Add to accumulator
            } else {
                self.timings.entry(label).or_default().push(duration); // Normal recording
            }
        }
    }

    /// Enable accumulation mode for a label. All subsequent start()/end() calls
    /// with this label will sum up until finish_accumulating() is called.
    pub fn start_accumulating(&mut self, label: impl Into<String>) {
        self.accumulators.insert(label.into(), Duration::ZERO);
    }

    /// Disable accumulation mode and record the total accumulated duration.
    pub fn finish_accumulating(&mut self, label: impl Into<String>) {
        let label = label.into();
        if let Some(total) = self.accumulators.remove(&label) {
            self.timings.entry(label).or_default().push(total);
        }
    }

    pub fn print_summary(&self) {
        // Separate one-time and per-frame stats
        let mut one_time = Vec::new();
        let mut per_frame = Vec::new();

        for (label, times) in &self.timings {
            if times.len() == 1 {
                one_time.push(label.as_str());
            } else {
                per_frame.push(label.as_str());
            }
        }

        one_time.sort();
        per_frame.sort();

        if !one_time.is_empty() {
            println!("\nOne-time operations:");
            for label in one_time {
                if let Some(times) = self.timings.get(label) {
                    self.calculate_stats(label, times);
                }
            }
        }

        if !per_frame.is_empty() {
            println!("\nPer-frame statistics:");
            for label in per_frame {
                if let Some(times) = self.timings.get(label) {
                    self.calculate_stats(label, times);
                }
            }
        }
    }

    fn calculate_stats(&self, name: &str, times: &[Duration]) {
        if times.is_empty() {
            return;
        }

        let mut sorted = times.to_vec();
        sorted.sort();

        let min = sorted[0];
        let max = sorted[sorted.len() - 1];
        let median = sorted[sorted.len() / 2];
        let sum: Duration = times.iter().sum();
        let avg = sum / times.len() as u32;

        let count = times.len();
        if count == 1 {
            println!("  {}: {:?}", name, sum);
        } else {
            println!("  {} (n={}):", name, count);
            println!("    median: {:?}", median);
            println!("    avg:    {:?}", avg);
            println!("    min:    {:?}", min);
            println!("    max:    {:?}", max);
            println!("    sum:    {:?}", sum);
        }
    }
}
