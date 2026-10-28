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

fn get_vcpkg_dir() -> Option<PathBuf> {
    if env::var("RESILIPARSE_USE_SYSTEM_LIBS").is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true")) {
        return None;
    }

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
        .env("CMAKE_POLICY_VERSION_MINIMUM", "3.5")
        .output()
        .expect("Failed to run vcpkg.");
    if !out.status.success() {
        panic!("Failed to install vcpkg dependencies:\n{}", String::from_utf8_lossy(&out.stdout));
    }
    Some(PathBuf::from(install_root).join(triplet))
}

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap_or_else(|_| ".".to_string()));
    let vcpkg_dir = get_vcpkg_dir();

    println!("cargo:rerun-if-env-changed=RESILIPARSE_USE_SYSTEM_LIBS");
    println!("cargo:rustc-link-lib=lexbor");
    if let Some(vcpkg_dir) = &vcpkg_dir {
        println!("cargo:rustc-link-search=native={}/lib", vcpkg_dir.display());
        if matches!(env::var("CARGO_CFG_TARGET_OS").as_deref(), Ok("macos" | "linux")) {
            println!("cargo:rustc-link-arg=-Wl,-rpath,{}/lib", vcpkg_dir.display());
        }
    }
    println!("cargo:rerun-if-changed=src/third_party/lexbor.h");

    let mut bindgen = bindgen::Builder::default().header("src/third_party/lexbor.h");
    if let Some(vcpkg_dir) = &vcpkg_dir {
        bindgen = bindgen.clang_arg(format!("-I{}/include", vcpkg_dir.display()));
    }

    bindgen
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
