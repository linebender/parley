// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! # `xtask`
//!
//! xtask with helper utilities for snapshot testing

use clap::Parser;
use kompari::xtask_cli::{XtaskActions, XtaskArgs};
use kompari::Error;
use std::path::Path;
use std::process::Command;

struct XtaskActionsImpl();

impl XtaskActions for XtaskActionsImpl {
    fn generate_all_tests(&self) -> kompari::Result<()> {
        let cargo = std::env::var("CARGO").unwrap();
        Command::new(&cargo)
            .arg("test")
            .env("PARLEY_TEST", "generate-all")
            .status()?;
        Ok(())
    }
}

fn main() -> Result<(), Error> {
    let test_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("parley")
        .join("tests");
    let current_path = test_dir.join("current");
    let snapshot_path = test_dir.join("snapshots");
    XtaskArgs::parse().run(&current_path, &snapshot_path, XtaskActionsImpl())?;
    Ok(())
}
