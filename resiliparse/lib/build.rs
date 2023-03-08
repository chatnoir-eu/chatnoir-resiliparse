extern crate bindgen;

use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=third_party/lexbor.h");
    bindgen::Builder::default()
        .header("third_party/lexbor.h")
        .allowlist_function("(lexbor|lxb)_.*")
        .allowlist_type("(LEXBOR|lexbor|lxb)_.*")
        .allowlist_var("(LEXBOR|LXB)_.*")
        .default_enum_style(bindgen::EnumVariation::Rust {non_exhaustive: false})
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .generate()
        .expect("Error generating Lexbor binding")
        .write_to_file(Path::new("third_party/lexbor.rs"))
        .unwrap();
}
