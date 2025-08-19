// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::tests::utils::renderer::{ColorBrush, RenderingConfig, render_layout};
use crate::{
    FontContext, FontFamily, FontStack, Layout, LayoutContext, LineHeight, PlainEditor,
    PlainEditorDriver, RangedBuilder, Rect, StyleProperty, TextStyle, TreeBuilder,
};
use fontique::{Blob, Collection, CollectionOptions};
use peniko::kurbo::Size;
use std::{
    borrow::Cow,
    path::{Path, PathBuf},
    sync::Arc,
};
use tiny_skia::{Color, Pixmap};

// Returns the current function name
#[macro_export]
macro_rules! test_name {
    () => {{
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let name = type_name_of(f);
        let name = &name[..name.len() - 3];
        let name = &name[name.rfind(':').map(|x| x + 1).unwrap_or(0)..];

        name
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

fn font_dirs() -> impl Iterator<Item = PathBuf> {
    [
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("assets")
            .join("arimo_fonts"),
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("assets")
            .join("roboto_fonts"),
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("assets")
            .join("noto_fonts"),
    ]
    .into_iter()
}

pub(crate) const FONT_STACK: &[FontFamily<'_>] = &[
    FontFamily::Named(Cow::Borrowed("Roboto")),
    FontFamily::Named(Cow::Borrowed("Noto Kufi Arabic")),
];

pub(crate) struct TestEnv {
    test_name: String,
    check_counter: u32,
    font_cx: FontContext,
    layout_cx: LayoutContext<ColorBrush>,
    text_color: Color,
    rendering_config: RenderingConfig,
    cursor_size: f32,
    tolerance: f32,
    // TODO: Add core::panic::Location for case.
    errors: Vec<(PathBuf, String)>,
    next_test_case_name: String,
}

fn is_accept_mode() -> bool {
    std::env::var("PARLEY_TEST")
        .map(|x| x.eq_ignore_ascii_case("accept"))
        .unwrap_or(false)
}

fn is_generate_all_mode() -> bool {
    std::env::var("PARLEY_TEST")
        .map(|x| x.eq_ignore_ascii_case("generate-all"))
        .unwrap_or(false)
}

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

pub(crate) fn create_font_context() -> FontContext {
    let mut collection = Collection::new(CollectionOptions {
        shared: false,
        system_fonts: false,
    });
    load_fonts(&mut collection, font_dirs()).unwrap();
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

impl TestEnv {
    pub(crate) fn new<S: Into<Option<Size>>>(test_name: &str, size: S) -> Self {
        let file_prefix = format!("{test_name}-");
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
        Self {
            test_name: test_name.to_string(),
            check_counter: 0,
            font_cx: create_font_context(),
            tolerance: 0.0,
            layout_cx: LayoutContext::new(),
            text_color: Color::BLACK,
            rendering_config: RenderingConfig {
                background_color: Color::WHITE,
                padding_color: Color::from_rgba8(166, 200, 255, 255),
                cursor_color: Color::from_rgba8(0, 0, 255, 255),
                selection_colors: [
                    Color::from_rgba8(0, 255, 0, 255),
                    Color::from_rgba8(255, 0, 0, 255),
                ],
                inline_box_color: Color::BLACK,
                size: size.into(),
            },
            cursor_size: 2.0,
            errors: Vec::new(),
            next_test_case_name: String::new(),
        }
    }

    pub(crate) fn rendering_config(&mut self) -> &mut RenderingConfig {
        &mut self.rendering_config
    }

    fn default_style(&self) -> [StyleProperty<'static, ColorBrush>; 3] {
        [
            StyleProperty::Brush(ColorBrush {
                color: self.text_color,
            }),
            StyleProperty::FontStack(FontStack::List(FONT_STACK.into())),
            // When our tests were originally generated, they used this style value, as it was the default.
            // To avoid regenerating all tests, as the default has now changed, set this explicitly.
            StyleProperty::LineHeight(LineHeight::FontSizeRelative(1.0)),
        ]
    }

    pub(crate) fn ranged_builder<'a>(&'a mut self, text: &'a str) -> RangedBuilder<'a, ColorBrush> {
        let default_style = self.default_style();
        let mut builder = self
            .layout_cx
            .ranged_builder(&mut self.font_cx, text, 1.0, true);
        for style in default_style {
            builder.push_default(style);
        }
        builder
    }

    pub(crate) fn tree_builder(&mut self) -> TreeBuilder<'_, ColorBrush> {
        let default_style = self.default_style();
        let mut builder =
            self.layout_cx
                .tree_builder(&mut self.font_cx, 1.0, true, &TextStyle::default());
        builder.push_style_modification_span(&default_style);
        builder
    }

    pub(crate) fn driver<'a>(
        &'a mut self,
        editor: &'a mut PlainEditor<ColorBrush>,
    ) -> PlainEditorDriver<'a, ColorBrush> {
        editor.driver(&mut self.font_cx, &mut self.layout_cx)
    }

    pub(crate) fn editor(&mut self, text: &str) -> PlainEditor<ColorBrush> {
        let mut editor = PlainEditor::new(16.);
        for style in self.default_style() {
            editor.edit_styles().insert(style);
        }
        editor.set_text(text);
        editor
    }

    fn image_name(&mut self, test_case_name: &str) -> String {
        if test_case_name.is_empty() {
            let name = format!("{}-{}.png", self.test_name, self.check_counter);
            self.check_counter += 1;
            name
        } else {
            assert!(
                test_case_name
                    .chars()
                    .all(|c| c == '_' || char::is_alphanumeric(c))
            );
            format!("{}-{}.png", self.test_name, test_case_name)
        }
    }

    fn check_images(&self, current_img: &Pixmap, snapshot_path: &Path) -> Result<(), String> {
        if !snapshot_path.is_file() {
            return Err(format!("Cannot find snapshot {}", snapshot_path.display()));
        }
        let snapshot_img = match Pixmap::load_png(snapshot_path) {
            Ok(snapshot_img) => snapshot_img,
            Err(d) => {
                if std::env::var("PARLEY_IGNORE_DECODING_ERRORS").is_ok() {
                    return Ok(());
                }
                return Err(format!(
                    "Loading snapshot {} failed due to decoding error {d}.\n\
                    If this file is an LFS file, install git lfs (https://git-lfs.com/) and run `git lfs pull`.\n\
                    If that fails (due to e.g. a lack of bandwidth), rerun tests with `PARLEY_IGNORE_DECODING_ERRORS=1` to skip this test.",
                    snapshot_path.display()
                ))?;
            }
        };
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

    pub(crate) fn with_name(&mut self, test_case_name: &str) -> &mut Self {
        self.next_test_case_name = test_case_name.to_string();
        self
    }

    pub(crate) fn check_editor_snapshot(&mut self, editor: &mut PlainEditor<ColorBrush>) {
        editor.refresh_layout(&mut self.font_cx, &mut self.layout_cx);
        self.render_and_check_snapshot(
            editor.try_layout().unwrap(),
            editor.cursor_geometry(self.cursor_size),
            &editor.selection_geometry(),
        );
    }

    pub(crate) fn check_layout_snapshot(&mut self, layout: &Layout<ColorBrush>) {
        self.render_and_check_snapshot(layout, None, &[]);
    }

    pub(crate) fn render_and_check_snapshot(
        &mut self,
        layout: &Layout<ColorBrush>,
        cursor_rect: Option<Rect>,
        selection_rects: &[(Rect, usize)],
    ) {
        let test_case_name = std::mem::take(&mut self.next_test_case_name);
        let current_img =
            render_layout(&self.rendering_config, layout, cursor_rect, selection_rects);
        let image_name = self.image_name(&test_case_name);

        let snapshot_path = snapshot_dir().join(&image_name);
        let comparison_path = current_imgs_dir().join(&image_name);

        if let Err(e) = self.check_images(&current_img, &snapshot_path) {
            if is_accept_mode() {
                current_img.save_png(&snapshot_path).unwrap();
            } else {
                current_img.save_png(&comparison_path).unwrap();
                self.errors.push((comparison_path, e));
            }
        } else if is_generate_all_mode() {
            current_img.save_png(&comparison_path).unwrap();
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
