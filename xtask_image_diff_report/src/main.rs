// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! # `xtask_image_diff_report`
//!
//! xtask for creating report when snapshot testing fails

use image_diff_review::{CompareConfig, Error, ImageDiff, ReportConfig};
use std::path::Path;

fn main() -> Result<(), Error> {
    let test_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("parley")
        .join("tests");
    let current_path = test_dir.join("current");
    let snapshot_path = test_dir.join("snapshots");
    let output_path = test_dir.join("report.html");

    let mut config = CompareConfig::default();
    config.set_ignore_left_missing(true);

    let mut image_diff = ImageDiff::default();
    image_diff.compare_directories(&config, &current_path, &snapshot_path)?;

    let mut report_config = ReportConfig::default();
    report_config.set_left_title("Current test");
    report_config.set_right_title("Snapshot");
    image_diff.create_report(&report_config, &output_path, true)?;
    Ok(())
}
