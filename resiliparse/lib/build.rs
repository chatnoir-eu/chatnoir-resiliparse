// Copyright 2023 Janek Bevendorff
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::env;
use std::path::PathBuf;

extern crate bindgen;

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
        .write_to_file(PathBuf::from("third_party").join("lexbor.rs"))
        .unwrap();
}
