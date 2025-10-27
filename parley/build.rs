//! Builds ICU4X data providers for Parley (via `unicode_data`).

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("baked_data");

    unicode_data::build::bake(out);
}
