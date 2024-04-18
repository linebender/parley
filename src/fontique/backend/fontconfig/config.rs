// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Extremely naive fontconfig xml parser to extract the data we need.

use roxmltree::Node;
use std::path::Path;

pub trait ParserSink {
    fn include_path(&mut self, path: &Path);
    fn cache_path(&mut self, path: &Path);
    fn alias(&mut self, family: &str, prefer: &[&str]);
    fn lang_map(&mut self, lang: &str, from_family: Option<&str>, family: &str);
}

pub fn parse_config(path: &Path, sink: &mut impl ParserSink) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let Ok(doc) = roxmltree::Document::parse(&text) else {
        return;
    };
    let root = doc.root_element();
    if root.tag_name().name() != "fontconfig" {
        return;
    }
    let mut prefer = vec![];
    'outer: for child in root.children() {
        match child.tag_name().name() {
            "alias" => {
                let mut family = None;
                for child in child.children() {
                    match child.tag_name().name() {
                        "family" => {
                            family = child.text();
                            if !family.map(is_alias_family).unwrap_or(false) {
                                continue 'outer;
                            }
                        }
                        "prefer" => {
                            prefer.clear();
                            prefer.extend(child.children().filter_map(|family| {
                                match family.tag_name().name() {
                                    "family" => family.text(),
                                    _ => None,
                                }
                            }));
                        }
                        _ => {}
                    }
                }
                match family {
                    Some(family) if !prefer.is_empty() => {
                        sink.alias(family, &prefer);
                    }
                    _ => {}
                }
            }
            "cachedir" => {
                if let Some(path) = resolve_dir(child, path) {
                    sink.cache_path(&path);
                }
            }
            "include" => {
                if let Some(path) = resolve_dir(child, path) {
                    let _ = include_config(&path, sink);
                }
            }
            "match" => {
                // We only care about pattern matches
                if !matches!(child.attribute("target"), Some("pattern") | None) {
                    continue;
                }
                let mut test_lang = None;
                let mut test_family = None;
                let mut edit_family = None;
                for child in child.children() {
                    match child.tag_name().name() {
                        "test" => {
                            if !matches!(
                                child.attribute("compare"),
                                Some("eq") | Some("contains") | None
                            ) {
                                continue 'outer;
                            }
                            match child.attribute("name") {
                                Some("lang") => {
                                    test_lang =
                                        child.first_element_child().and_then(|inner| inner.text());
                                }
                                Some("family") => {
                                    test_family =
                                        child.first_element_child().and_then(|inner| inner.text());
                                    if !test_family.map(is_match_family).unwrap_or(true) {
                                        continue 'outer;
                                    }
                                }
                                _ => continue 'outer,
                            }
                        }
                        "edit" => {
                            if child.attribute("name") == Some("family") {
                                edit_family =
                                    child.first_element_child().and_then(|inner| inner.text());
                            }
                        }
                        "" => {}
                        _ => continue 'outer,
                    }
                }
                if let (Some(lang), Some(family)) = (test_lang, edit_family) {
                    sink.lang_map(lang, test_family, family);
                }
            }
            _ => {}
        }
    }
}

/// Families we care about for aliases.
const ALIAS_FAMILIES: &[&str] = &[
    "cursive",
    "emoji",
    "fantasy",
    "math",
    "monospace",
    "sans-serif",
    "serif",
    "system-ui",
];

fn is_alias_family(family: &str) -> bool {
    ALIAS_FAMILIES.binary_search(&family).is_ok()
}

/// Families we care about for lang matches.
const MATCH_FAMILIES: &[&str] = &["monospace", "sans-serif", "serif"];

fn is_match_family(family: &str) -> bool {
    MATCH_FAMILIES.binary_search(&family).is_ok()
}

fn include_config(path: &Path, sink: &mut impl ParserSink) -> std::io::Result<()> {
    let meta = std::fs::metadata(path)?;
    let ty = meta.file_type();
    // fs::metadata follow symlink so ty is never symlink
    if ty.is_file() {
        parse_config(path, sink);
    } else if ty.is_dir() {
        let dir = std::fs::read_dir(path)?;
        let mut config_paths = dir
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let ty = entry.file_type().ok()?;

                if ty.is_file() || ty.is_symlink() {
                    Some(entry.path())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        config_paths.sort_unstable();
        for config_path in &config_paths {
            sink.include_path(config_path);
            parse_config(config_path, sink);
        }
    }
    Ok(())
}

fn resolve_dir(
    node: Node,
    config_file_path: impl AsRef<std::path::Path>,
) -> Option<std::path::PathBuf> {
    let dir_path = node.text()?;
    let (xdg_env, xdg_fallback) = match node.tag_name().name() {
        "include" => ("XDG_CONFIG_HOME", "~/.config"),
        "cachedir" => ("XDG_CACHE_HOME", "~/.cache"),
        _ => return None,
    };
    let path = match node.attribute("prefix") {
        Some("xdg") => {
            std::path::PathBuf::from(std::env::var(xdg_env).unwrap_or_else(|_| xdg_fallback.into()))
                .join(dir_path)
        }
        _ => {
            if dir_path.starts_with('/') {
                dir_path.into()
            } else {
                match config_file_path.as_ref().parent() {
                    Some(parent) => parent.join(dir_path),
                    None => std::path::Path::new(".").join(dir_path),
                }
            }
        }
    };
    Some(if let Ok(stripped_path) = path.strip_prefix("~") {
        let home = config_home().unwrap_or("/".to_string());
        std::path::Path::new(&home).join(stripped_path)
    } else {
        path
    })
}

/// Get the location to user home directory.
///
/// This implementation follows `FcConfigHome` function of freedesktop.org's
/// Fontconfig library.
#[allow(unused_mut, clippy::let_and_return)]
fn config_home() -> Result<String, std::env::VarError> {
    let mut home = std::env::var("HOME");
    #[cfg(target_os = "windows")]
    {
        home = home.or_else(|_| std::env::var("USERPROFILE"));
    }
    home
}
