// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! # `xtask_test`
//!
//! xtask with helper utilities for snapshot testing

use image_diff_review::{CompareConfig, Error, ImageDiff, ReportConfig};
use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

fn create_report(
    current_path: &Path,
    snapshot_path: &Path,
    output_path: &Path,
) -> Result<(), Error> {
    let mut config = CompareConfig::default();
    config.set_ignore_left_missing(true);

    let mut image_diff = ImageDiff::default();
    image_diff.compare_directories(&config, current_path, snapshot_path)?;

    let mut report_config = ReportConfig::default();
    report_config.set_left_title("Current test");
    report_config.set_right_title("Snapshot");
    image_diff.create_report(&report_config, output_path, true)?;
    Ok(())
}

fn list_image_dir(dir_path: &Path) -> Result<impl Iterator<Item = PathBuf>, std::io::Error> {
    Ok(std::fs::read_dir(dir_path)?.filter_map(|entry| {
        if let Ok(entry) = entry {
            let path = entry.path();
            if path
                .extension()
                .and_then(OsStr::to_str)
                .map(|ext| ext == "png")
                .unwrap_or(false)
            {
                Some(path)
            } else {
                None
            }
        } else {
            None
        }
    }))
}

fn clean_image_path(dir_path: &Path) -> Result<(), std::io::Error> {
    list_image_dir(dir_path)?.try_for_each(|path| std::fs::remove_file(&path))
}

fn detect_dead_snapshots(current_path: &Path, snapshot_path: &Path) -> Result<(), Error> {
    clean_image_path(current_path)?;
    let cargo = std::env::var("CARGO").unwrap();
    Command::new(&cargo)
        .arg("test")
        .env("PARLEY_TEST", "generate-all")
        .status()?;

    let current_imgs: BTreeSet<String> = list_image_dir(current_path)?
        .map(|r| r.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    let snapshot_imgs: BTreeSet<String> = list_image_dir(snapshot_path)?
        .map(|r| r.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    let dead_paths: Vec<String> = snapshot_imgs.difference(&current_imgs).cloned().collect();

    if dead_paths.is_empty() {
        println!("No dead snapshots detected");
    } else {
        println!("========== DEAD SNAPSHOTS ==========");
        for path in &dead_paths {
            println!("{}", path);
        }
    }
    clean_image_path(current_path)?;
    Ok(())
}

fn main() -> Result<(), Error> {
    let test_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("parley")
        .join("tests");
    let current_path = test_dir.join("current");
    let snapshot_path = test_dir.join("snapshots");
    match std::env::args().nth(1).as_deref() {
        Some("clean") => clean_image_path(&current_path)?,
        Some("report") => {
            let output_path = test_dir.join("report.html");
            create_report(&current_path, &snapshot_path, &output_path)?;
        }
        Some("dead-snapshots") => detect_dead_snapshots(&current_path, &snapshot_path)?,
        _ => println!("Invalid command\n\nCommands:\n- clean: Clean 'current' directory\n- report: Create report with image diffs\n- dead-snapshots: print dead snapshots"),
    }
    Ok(())
}
