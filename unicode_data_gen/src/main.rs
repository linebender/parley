//! Command-line interface for generating Parley Unicode data.

fn main() {
    use std::{env, ffi::OsString, path::PathBuf, process};

    let mut args = env::args_os();
    let exe = args
        .next()
        .unwrap_or_else(|| OsString::from("unicode_data_gen"));

    let Some(out_arg) = args.next() else {
        eprintln!("Usage: {} <output-dir>", exe.to_string_lossy());
        process::exit(1);
    };

    let out_path = PathBuf::from(out_arg);

    if let Err(err) = std::fs::create_dir_all(&out_path) {
        eprintln!(
            "Failed to create output directory '{}': {}",
            out_path.display(),
            err
        );
        process::exit(1);
    }

    unicode_data_gen::generate(out_path);
}
