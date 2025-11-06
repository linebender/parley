// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Builds ICU4X data providers for Parley (via `unicode_data`).

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../unicode_data");

    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("baked_data");

    unicode_data::build::bake(out);
}
