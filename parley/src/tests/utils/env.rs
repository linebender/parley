// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::tests::utils::renderer::render_layout;
use crate::{
    FontContext, FontFamily, FontStack, Layout, LayoutContext, RangedBuilder, StyleProperty,
};
use fontique::{Collection, CollectionOptions};
use peniko::Color;
use std::path::{Path, PathBuf};
use tiny_skia::Pixmap;

// Creates a new instance of TestEnv and put current function name in constructor
#[macro_export]
macro_rules! testenv {
    () => {{
        // Get name of the current function
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let name = type_name_of(f);
        let name = &name[..name.len() - 3];
        let name = &name[name.rfind(':').map(|x| x + 1).unwrap_or(0)..];

        // Create test env
        $crate::tests::utils::TestEnv::new(name)
    }};
}

fn current_imgs_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("current")
}

fn snapshot_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("snapshots")
}

fn font_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("assets")
        .join("roboto_fonts")
}

const DEFAULT_FONT_NAME: &str = "Roboto";

pub(crate) struct TestEnv {
    test_name: String,
    check_counter: u32,
    font_cx: FontContext,
    layout_cx: LayoutContext<Color>,
    foreground_color: Color,
    background_color: Color,
    tolerance: f32,
    errors: Vec<(PathBuf, String)>,
}

fn is_accept_mode() -> bool {
    std::env::var("PARLEY_TEST")
        .map(|x| x.to_ascii_lowercase() == "accept")
        .unwrap_or(false)
}

pub(crate) fn load_fonts_dir(collection: &mut Collection, path: &Path) -> std::io::Result<()> {
    let paths = std::fs::read_dir(path)?;
    for entry in paths {
        let entry = entry?;
        if !entry.metadata()?.is_file() {
            continue;
        }
        let path = entry.path();
        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| !["ttf", "otf", "ttc", "otc"].contains(&ext))
            .unwrap_or(true)
        {
            continue;
        }
        let font_data = std::fs::read(&path)?;
        collection.register_fonts(font_data);
    }
    Ok(())
}

impl TestEnv {
    pub(crate) fn new(test_name: &str) -> Self {
        let file_prefix = format!("{}-", test_name);
        let entries = std::fs::read_dir(current_imgs_dir()).unwrap();
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.starts_with(&file_prefix) && name.ends_with(".png"))
                .unwrap_or(false)
            {
                std::fs::remove_file(&path).unwrap();
            }
        }

        let mut collection = Collection::new(CollectionOptions {
            shared: false,
            system_fonts: false,
        });
        load_fonts_dir(&mut collection, &font_dir()).unwrap();
        collection
            .family_id(DEFAULT_FONT_NAME)
            .unwrap_or_else(|| panic!("{} font not found", DEFAULT_FONT_NAME));
        TestEnv {
            test_name: test_name.to_string(),
            check_counter: 0,
            font_cx: FontContext {
                collection,
                source_cache: Default::default(),
            },
            tolerance: 0.0,
            layout_cx: LayoutContext::new(),
            foreground_color: Color::rgb8(0, 0, 0),
            background_color: Color::rgb8(255, 255, 255),
            errors: Vec::new(),
        }
    }

    pub(crate) fn builder<'a>(&'a mut self, text: &'a str) -> RangedBuilder<'a, Color> {
        let mut builder = self.layout_cx.ranged_builder(&mut self.font_cx, text, 1.0);
        builder.push_default(StyleProperty::Brush(self.foreground_color));
        builder.push_default(StyleProperty::FontStack(FontStack::Single(
            FontFamily::Named(DEFAULT_FONT_NAME.into()),
        )));
        builder
    }

    fn image_name(&mut self) -> String {
        let name = format!("{}-{}.png", self.test_name, self.check_counter);
        self.check_counter += 1;
        name
    }

    fn check_images(&self, current_img: &Pixmap, snapshot_path: &Path) -> Result<(), String> {
        if !snapshot_path.is_file() {
            return Err(format!("Cannot find snapshot {}", snapshot_path.display()));
        }
        let snapshot_img = Pixmap::load_png(snapshot_path)
            .map_err(|_| format!("Loading snapshot {} failed", snapshot_path.display()))?;
        if snapshot_img.width() != current_img.width()
            || snapshot_img.height() != current_img.height()
        {
            return Err(format!(
                "Snapshot has different size: snapshot {}x{}; generated image: {}x{}",
                snapshot_img.width(),
                snapshot_img.height(),
                current_img.width(),
                current_img.height()
            ));
        }

        let mut n_different_pixels = 0;
        let mut color_cumulative_difference = 0.0;
        for (pixel1, pixel2) in snapshot_img.pixels().iter().zip(current_img.pixels()) {
            if pixel1 != pixel2 {
                n_different_pixels += 1;
            }
            let diff_r = (pixel1.red() as f32 - pixel2.red() as f32).abs();
            let diff_g = (pixel1.green() as f32 - pixel2.green() as f32).abs();
            let diff_b = (pixel1.blue() as f32 - pixel2.blue() as f32).abs();
            color_cumulative_difference += diff_r.max(diff_g).max(diff_b);
        }
        if color_cumulative_difference > self.tolerance {
            return Err(format!(
                "Testing image differs in {n_different_pixels} pixels (color difference = {color_cumulative_difference})",
            ));
        }
        Ok(())
    }

    pub(crate) fn check(&mut self, layout: &Layout<Color>) {
        let current_img = render_layout(layout, self.background_color, self.foreground_color);
        let image_name = self.image_name();

        let snapshot_path = snapshot_dir().join(&image_name);
        let comparison_path = current_imgs_dir().join(&image_name);

        if let Err(e) = self.check_images(&current_img, &snapshot_path) {
            if is_accept_mode() {
                current_img.save_png(&snapshot_path).unwrap();
            } else {
                current_img.save_png(&comparison_path).unwrap();
                self.errors.push((comparison_path, e));
            }
        }
    }
}

impl Drop for TestEnv {
    // Dropping of TestEnv cause panic (if there is not already one)
    // We do not panic immediately when error is detected because we want to
    // generate all images in the test and do visual confirmation of the whole
    // set and not stop at the first error.
    fn drop(&mut self) {
        if !self.errors.is_empty() && !std::thread::panicking() {
            use std::fmt::Write;
            let mut panic_msg = String::new();
            for (path, msg) in &self.errors {
                write!(
                    &mut panic_msg,
                    "{}\nImage written into: {}\n",
                    msg,
                    path.display()
                )
                .unwrap();
            }
            panic!("{}", &panic_msg);
        }
    }
}
