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
use std::env::consts;
use std::path::PathBuf;
use std::process::Command;

extern crate bindgen;

fn get_vcpkg_dir() -> PathBuf {
    let arch = consts::ARCH.replace("x86_64", "x64").replace("aarch64", "arm64");
    let os = consts::OS.replace("macos", "osx");
    let triplet = env::var("VCPKG_DEFAULT_TRIPLET").unwrap_or_else(|_| format!("{}-{}", arch, os));
    let install_root = format!("{}/vcpkg_installed", env::var("OUT_DIR").unwrap_or_else(|_| ".".to_string()));
    let out = Command::new("vcpkg")
        .args([
            "install",
            "--triplet",
            &triplet,
            "--x-install-root",
            install_root.as_str(),
        ])
        .output()
        .expect("Failed to run vcpkg.");
    if !out.status.success() {
        panic!("Failed to install vcpkg dependencies:\n{}", String::from_utf8_lossy(&out.stdout));
    }
    PathBuf::from(install_root).join(triplet)
}

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap_or_else(|_| ".".to_string()));
    let vcpkg_dir = get_vcpkg_dir();

    println!("cargo:rustc-link-search=native={}/lib", vcpkg_dir.to_str().unwrap());
    println!("cargo:rustc-link-lib=lexbor");
    println!("cargo:rerun-if-changed=src/third_party/lexbor.h");

    bindgen::Builder::default()
        .header("src/third_party/lexbor.h")
        .clang_arg(format!("-I{}/include", vcpkg_dir.to_str().unwrap()))
        .allowlist_function("(lexbor|lxb)_.*")
        .allowlist_type("(LEXBOR|lexbor|lxb)_.*")
        .allowlist_var("(LEXBOR|LXB)_.*")
        .default_enum_style(bindgen::EnumVariation::ModuleConsts)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Error generating Lexbor binding")
        .write_to_file(PathBuf::from(out_dir).join("lexbor.rs"))
        .unwrap();
}
