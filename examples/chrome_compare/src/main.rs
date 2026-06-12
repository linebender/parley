// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Compares Parley's layout widths against Chrome's for the same text and font.
//!
//! For each test case, the text is laid out with Parley using the repository's
//! `Roboto-Regular.ttf`, and the resulting advance width is compared against the
//! widths reported by a Chrome page which embeds the exact same font bytes as a
//! data URL, measured three ways:
//!
//! - `canvas.measureText(text).width`: raw shaper output, unquantized. This
//!   should agree with Parley to well below a pixel; larger differences point
//!   at a shaping or metrics divergence worth investigating.
//! - A hidden `<span>`'s `getBoundingClientRect().width`: DOM layout, which
//!   Chrome quantizes to 1/64px (its internal `LayoutUnit`).
//! - The same span's `offsetWidth`: DOM layout rounded to whole CSS pixels.
//!   This is how web applications often measure text, so it is reported for
//!   reference, but it is rounded by definition and not held to the threshold.
//!
//! Requires a Chrome or Chromium binary. The binary is located automatically on
//! common install paths, or can be given explicitly via the `CHROME` environment
//! variable. Run with:
//!
//! ```sh
//! cargo run -p chrome_compare
//! ```
//!
//! An optional `--threshold <px>` argument sets the maximum allowed width
//! difference (default 1.0); the process exits non-zero if any case exceeds it.
//! `--kerning <on|off>` (default on) toggles kerning consistently on both
//! sides: Parley gets a `"kern" 0` font feature, and the Chrome measurements
//! use the matching `font-kerning` value. `off` mirrors applications that
//! measure DOM text with `font-kerning: none`.

use std::fmt::Write as _;
use std::io::{BufRead as _, BufReader, Read as _};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;

use parley::fontique::{Blob, Collection, CollectionOptions, SourceCache};
use parley::{FontContext, FontFamily, FontFeatures, LayoutContext, StyleProperty};

/// Family name of the font under test, as it appears in the font's name table
/// (Parley side) and in the generated `@font-face` rule (Chrome side).
const FONT_FAMILY: &str = "Roboto";

/// Test strings. All characters should be covered by Roboto, since the two
/// sides have different fallback behavior for missing glyphs (Parley is given
/// only this font, while Chrome falls back to system fonts).
const TEXTS: &[(&str, &str)] = &[
    ("hello", "Hello, world!"),
    ("pangram", "The quick brown fox jumps over the lazy dog."),
    ("kerning", "AV. To Ta Te Yo P. F. WAVE Vo LT y."),
    ("ligatures", "office waffle final fjord affluent"),
    ("numbers", "0123456789 (50%) [#42] {x * y} != >= +-"),
    (
        "long",
        "It is a truth universally acknowledged, that a single man in possession \
         of a good fortune, must be in want of a wife. However little known the \
         feelings or views of such a man may be on his first entering a neighbourhood.",
    ),
];

/// Font sizes in CSS pixels, including fractional sizes to exercise scaling.
const SIZES: &[f32] = &[12.0, 16.0, 17.5, 24.0, 32.5];

#[derive(Debug)]
struct TestCase {
    name: &'static str,
    text: &'static str,
    font_size: f32,
}

fn test_cases() -> Vec<TestCase> {
    let mut cases = Vec::new();
    for &(name, text) in TEXTS {
        for &font_size in SIZES {
            cases.push(TestCase {
                name,
                text,
                font_size,
            });
        }
    }
    cases
}

fn main() {
    let config = parse_args();

    let font_path = roboto_path();
    let font_data = std::fs::read(&font_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", font_path.display()));

    let cases = test_cases();
    let parley_widths = parley_widths(&cases, font_data.clone(), config.kerning);
    let chrome_widths = chrome_widths(&cases, &font_data, config.kerning);
    let threshold = config.threshold;

    println!("kerning: {}\n", if config.kerning { "on" } else { "off" });
    println!(
        "{:<12} {:>6} {:>14} {:>14} {:>12} {:>14} {:>12} {:>10}",
        "case",
        "size",
        "parley (px)",
        "canvas (px)",
        "Δcanvas",
        "DOM rect (px)",
        "Δrect",
        "offsetW"
    );
    let mut max_diff = 0.0_f64;
    let mut failures = 0_usize;
    for (i, case) in cases.iter().enumerate() {
        let parley = f64::from(parley_widths[i]);
        let chrome = chrome_widths[i];
        let canvas_diff = parley - chrome.canvas;
        let rect_diff = parley - chrome.rect;
        // `offsetWidth` is rounded to whole pixels by definition, so it is
        // reported but not held to the threshold.
        let diff = canvas_diff.abs().max(rect_diff.abs());
        max_diff = max_diff.max(diff);
        let exceeded = diff > threshold;
        failures += usize::from(exceeded);
        println!(
            "{:<12} {:>6} {:>14.6} {:>14.6} {:>+12.6} {:>14.6} {:>+12.6} {:>10}{}",
            case.name,
            case.font_size,
            parley,
            chrome.canvas,
            canvas_diff,
            chrome.rect,
            rect_diff,
            chrome.offset,
            if exceeded {
                "  <-- EXCEEDS THRESHOLD"
            } else {
                ""
            }
        );
    }
    println!("\nmax |diff| (canvas & DOM rect): {max_diff:.6}px (threshold: {threshold}px)");

    if failures > 0 {
        eprintln!("{failures} case(s) exceeded the width difference threshold");
        std::process::exit(1);
    }
}

#[derive(Debug)]
struct Config {
    /// Maximum allowed width difference in pixels before the run fails.
    threshold: f64,
    /// Whether kerning is applied, consistently on both sides. Some
    /// applications measure DOM text with `font-kerning: none`; running with
    /// `--kerning off` mirrors that setup.
    kerning: bool,
}

fn parse_args() -> Config {
    let mut config = Config {
        threshold: 1.0,
        kerning: true,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = |what: &str| {
            args.next()
                .unwrap_or_else(|| panic!("{what} requires a value"))
        };
        match arg.as_str() {
            "--threshold" => {
                let value = value("--threshold");
                config.threshold = value
                    .parse()
                    .unwrap_or_else(|e| panic!("invalid --threshold value {value:?}: {e}"));
            }
            "--kerning" => {
                config.kerning = match value("--kerning").as_str() {
                    "on" => true,
                    "off" => false,
                    other => panic!("invalid --kerning value {other:?}; expected on or off"),
                };
            }
            other => panic!(
                "unrecognized argument {other:?}; supported: --threshold <px>, --kerning <on|off>"
            ),
        }
    }
    config
}

fn roboto_path() -> PathBuf {
    parley_dev::font_dirs()
        .find(|dir| dir.ends_with("roboto_fonts"))
        .expect("parley_dev should provide a roboto_fonts directory")
        .join("Roboto-Regular.ttf")
}

/// Lay out each test case with Parley and return its full advance width.
fn parley_widths(cases: &[TestCase], font_data: Vec<u8>, kerning: bool) -> Vec<f32> {
    // Register only the font under test, with system fonts disabled, so any
    // unexpected fallback shows up as a missing glyph rather than a silently
    // different width.
    let mut collection = Collection::new(CollectionOptions {
        shared: false,
        system_fonts: false,
    });
    collection.register_fonts(Blob::new(Arc::new(font_data)), None);
    let mut font_cx = FontContext {
        collection,
        source_cache: SourceCache::default(),
    };
    let mut layout_cx: LayoutContext<()> = LayoutContext::new();

    cases
        .iter()
        .map(|case| {
            // `quantize: false` keeps subpixel positions, matching the
            // unquantized advances reported by Chrome's `measureText`.
            let mut builder = layout_cx.ranged_builder(&mut font_cx, case.text, 1.0, false);
            builder.push_default(FontFamily::named(FONT_FAMILY));
            builder.push_default(StyleProperty::FontSize(case.font_size));
            if !kerning {
                builder.push_default(FontFeatures::from(r#""kern" 0"#));
            }
            let mut layout = builder.build(case.text);
            // No max advance: each case is a single unwrapped line, like the
            // single shaped run that `measureText` measures.
            layout.break_all_lines(None);
            // `full_width` includes trailing whitespace, as `measureText` does.
            layout.full_width()
        })
        .collect()
}

/// Widths reported by Chrome for one test case, via the different measurement
/// APIs a web application might use.
#[derive(Clone, Copy, Debug)]
struct ChromeWidths {
    /// `canvas.measureText(text).width`: raw shaper output, not quantized.
    canvas: f64,
    /// `span.getBoundingClientRect().width`: DOM layout, quantized to Chrome's
    /// internal 1/64px `LayoutUnit`.
    rect: f64,
    /// `span.offsetWidth`: DOM layout rounded to whole CSS pixels.
    offset: f64,
}

/// Measure each test case in headless Chrome and return the widths, indexed
/// like `cases`.
fn chrome_widths(cases: &[TestCase], font_data: &[u8], kerning: bool) -> Vec<ChromeWidths> {
    let chrome = find_chrome().unwrap_or_else(|| {
        panic!(
            "could not find a Chrome/Chromium binary; set the CHROME environment \
             variable to the path of one"
        )
    });

    let work_dir = std::env::temp_dir().join("parley_chrome_compare");
    std::fs::create_dir_all(&work_dir).expect("failed to create temp dir");
    let html_path = work_dir.join("measure.html");
    std::fs::write(&html_path, measurement_page(cases, font_data, kerning))
        .expect("failed to write measurement page");
    // Per-process profile directory: killing Chrome (below) leaves a stale
    // `SingletonLock` behind, which would abort the next run if reused.
    let profile_dir = work_dir.join(format!("profile-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&profile_dir);

    let mut child = Command::new(&chrome)
        .arg("--headless=new")
        .arg("--disable-gpu")
        .arg("--no-first-run")
        .arg("--force-device-scale-factor=1")
        .arg(format!("--user-data-dir={}", profile_dir.display()))
        .arg("--dump-dom")
        .arg(format!("file://{}", html_path.display()))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("failed to run {}: {e}", chrome.display()));

    // Drain stderr on a separate thread so Chrome can't block on a full pipe.
    let stderr = child.stderr.take().expect("child stderr should be piped");
    let stderr_thread = std::thread::spawn(move || {
        let mut buf = String::new();
        let _ = BufReader::new(stderr).read_to_string(&mut buf);
        buf
    });

    // Managed Chrome installs can linger for tens of seconds after dumping the
    // DOM (policy fetches, updater wake-ups, ...), so rather than waiting for
    // the process to exit, parse the dump as it streams and kill Chrome once
    // every case has been measured.
    let stdout = child.stdout.take().expect("child stdout should be piped");
    let mut widths: Vec<Option<ChromeWidths>> = vec![None; cases.len()];
    let mut found = 0;
    let mut dom = String::new();
    for line in BufReader::new(stdout).lines() {
        let line = line.expect("failed to read Chrome's stdout");
        // In the serialized DOM the first and last result lines share a line
        // with the `<pre>` open/close tags, so search rather than prefix-match
        // and trim any trailing tag from the final field.
        if let Some(pos) = line.find("RESULT\t") {
            let rest = line[pos + "RESULT\t".len()..].split('<').next().unwrap();
            let mut fields = rest.split('\t');
            let index: usize = fields
                .next()
                .expect("RESULT line from Chrome is missing the case index")
                .parse()
                .expect("malformed case index from Chrome");
            let mut next_field = |what: &str| -> f64 {
                fields
                    .next()
                    .unwrap_or_else(|| panic!("RESULT line from Chrome is missing {what}"))
                    .parse()
                    .unwrap_or_else(|e| panic!("malformed {what} from Chrome: {e}"))
            };
            let measured = ChromeWidths {
                canvas: next_field("canvas width"),
                rect: next_field("rect width"),
                offset: next_field("offset width"),
            };
            if widths[index].replace(measured).is_none() {
                found += 1;
            }
            if found == cases.len() {
                break;
            }
        }
        dom.push_str(&line);
        dom.push('\n');
    }
    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&profile_dir);

    if found != cases.len() {
        eprintln!("--- Chrome stderr ---\n{}", stderr_thread.join().unwrap());
        eprintln!("--- Dumped DOM ---\n{dom}");
        eprintln!("--- Measurement page kept at {} ---", html_path.display());
        panic!("Chrome did not report a width for every test case");
    }
    widths.into_iter().map(|w| w.unwrap()).collect()
}

fn find_chrome() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("CHROME") {
        return Some(PathBuf::from(path));
    }
    [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
        "/usr/bin/google-chrome",
        "/usr/bin/google-chrome-stable",
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
    ]
    .iter()
    .map(PathBuf::from)
    .find(|p| p.exists())
}

/// Build a self-contained HTML page which loads the font under test from a
/// data URL, measures every case with `canvas.measureText`, and writes one
/// `RESULT\t<index>\t<width>` line per case into a `<pre>` for `--dump-dom`.
fn measurement_page(cases: &[TestCase], font_data: &[u8], kerning: bool) -> String {
    // Chrome treats canvas's default `fontKerning` of "auto" as kerning OFF,
    // while DOM text treats "auto" as kerning ON, so both are always set
    // explicitly to keep every measurement path consistent.
    let font_kerning = if kerning { "normal" } else { "none" };
    let mut case_array = String::new();
    for (i, case) in cases.iter().enumerate() {
        let _ = writeln!(
            case_array,
            "    [{i}, {}, {}],",
            case.font_size,
            js_string_literal(case.text)
        );
    }

    // The script runs synchronously during page load (a `FontFace` constructed
    // from an `ArrayBuffer` is parsed immediately), so the results are already
    // in the DOM by the time `--dump-dom` snapshots it.
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
</head>
<body>
<pre id="out">measurement script did not run</pre>
<script>
  const cases = [
{case_array}  ];
  const fontBytes = Uint8Array.from(atob("{font_base64}"), (c) => c.charCodeAt(0));
  const face = new FontFace("{FONT_FAMILY}", fontBytes.buffer);
  let out = "";
  if (face.status !== "loaded") {{
    out = "ERROR: font failed to parse synchronously (status: " + face.status + ")";
  }} else {{
    document.fonts.add(face);
    const ctx = document.createElement("canvas").getContext("2d");
    // Hidden span for DOM-layout measurement, the way applications often
    // measure text (a measuring element read back via offsetWidth or
    // getBoundingClientRect). Unlike measureText, the DOM path goes through
    // Chrome's layout tree, which quantizes to 1/64px LayoutUnits.
    const span = document.createElement("span");
    span.style.position = "absolute";
    span.style.visibility = "hidden";
    span.style.whiteSpace = "pre";
    document.body.appendChild(span);
    for (const [index, size, text] of cases) {{
      ctx.font = size + 'px "{FONT_FAMILY}"';
      ctx.fontKerning = "{font_kerning}";
      const canvasWidth = ctx.measureText(text).width;
      // The font shorthand resets font-kerning, so it must be re-set after.
      span.style.font = size + 'px "{FONT_FAMILY}"';
      span.style.fontKerning = "{font_kerning}";
      span.textContent = text;
      const rectWidth = span.getBoundingClientRect().width;
      const offsetWidth = span.offsetWidth;
      out += "RESULT\t" + index + "\t" + canvasWidth.toFixed(6) +
        "\t" + rectWidth.toFixed(6) + "\t" + offsetWidth + "\n";
    }}
  }}
  document.getElementById("out").textContent = out;
</script>
</body>
</html>
"#,
        font_base64 = base64_encode(font_data),
    )
}

/// Encode `s` as a JavaScript string literal, escaping `<` so the result is
/// also safe inside an inline `<script>` element.
fn js_string_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '<' => out.push_str("\\u003c"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        out.push(ALPHABET[((n >> 18) & 63) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}
