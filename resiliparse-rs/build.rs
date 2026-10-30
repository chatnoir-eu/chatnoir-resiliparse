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
use std::process::Command;

/// Detect vcpkg directory or return `None` if `RESILIPARSE_USE_SYSTEM_LIBS` is set.
fn get_vcpkg_dir() -> Option<PathBuf> {
    if env::var("RESILIPARSE_USE_SYSTEM_LIBS").is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true")) {
        return None;
    }

    let arch = env::var("CARGO_CFG_TARGET_ARCH")
        .unwrap()
        .replace("x86_64", "x64")
        .replace("aarch64", "arm64");
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap().replace("macos", "osx");
    let target_triplet = format!("{arch}-{os}");
    let triplet = if env::var("TARGET") == env::var("HOST") {
        env::var("VCPKG_DEFAULT_TRIPLET").unwrap_or(target_triplet)
    } else {
        target_triplet
    };
    let workspace_root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("..");
    let install_root = env::var_os("RESILIPARSE_VCPKG_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace_root.join("vcpkg_installed"));
    println!("cargo:rerun-if-env-changed=RESILIPARSE_VCPKG_PATH");
    let triplet_root = install_root.join(&triplet);
    if triplet_root.is_dir() {
        return Some(triplet_root);
    }

    let out = Command::new("vcpkg")
        .current_dir(&workspace_root)
        .args(["install", "--triplet", &triplet, "--x-install-root"])
        .arg(&install_root)
        .env("CMAKE_POLICY_VERSION_MINIMUM", "3.5")
        .output()
        .expect("Failed to run vcpkg.");
    assert!(
        out.status.success(),
        "Failed to install vcpkg dependencies:\n{}",
        String::from_utf8_lossy(&out.stdout)
    );
    Some(triplet_root)
}

/// Fast language detection: Compile the toolchain's Unicode properties into deduplicated bitmap pages.
/// This keeps the classification rules in Rust without per-character searches through the standard
/// library's Unicode ranges or a runtime cache.
fn generate_language_unicode(out_dir: &std::path::Path) {
    use std::collections::BTreeMap;
    let mut indices = Vec::new();
    let mut pages = Vec::new();
    let mut seen = BTreeMap::new();
    for start in (0..0x110000u32).step_by(256) {
        let mut bits = [0u64; 4];
        for offset in 0..256u32 {
            if char::from_u32(start + offset).is_some_and(char::is_alphabetic) {
                bits[(offset / 64) as usize] |= 1u64 << (offset % 64);
            }
        }
        let index = *seen.entry(bits).or_insert_with(|| {
            let index = pages.len() as u16;
            pages.push(bits);
            index
        });
        indices.push(index);
    }
    let source = format!(
        "pub(super) static ALPHABETIC_PAGES: [u16; {}] = {:?};\n\
         pub(super) static ALPHABETIC_BITS: [[u64; 4]; {}] = {:?};\n",
        indices.len(),
        indices,
        pages.len(),
        pages,
    );
    std::fs::write(out_dir.join("lang_unicode.rs"), source).expect("Failed to generate Unicode tables");
}

fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let vcpkg_dir = get_vcpkg_dir();

    println!("cargo:rerun-if-env-changed=RESILIPARSE_USE_SYSTEM_LIBS");
    if let Some(vcpkg_dir) = &vcpkg_dir {
        println!("cargo:rustc-link-lib=static=lexbor_static");
        println!("cargo:rustc-link-lib=static=uchardet");
        println!("cargo:rustc-link-search=native={}/lib", vcpkg_dir.display());
        match env::var("CARGO_CFG_TARGET_OS").as_deref() {
            Ok("linux") => println!("cargo:rustc-link-lib=dylib=stdc++"),
            Ok("macos") => println!("cargo:rustc-link-lib=dylib=c++"),
            _ => {}
        }
    } else {
        println!("cargo:rustc-link-lib=lexbor");
        println!("cargo:rustc-link-lib=uchardet");
    }
    for (name, functions, types, variables) in [
        ("lexbor", "(lexbor|lxb)_.*", "(LEXBOR|lexbor|lxb)_.*", "(LEXBOR|LXB)_.*"),
        ("uchardet", "uchardet_.*", "uchardet.*", "UCHARDET_.*"),
    ] {
        let header = format!("src/third_party/{name}.h");
        println!("cargo:rerun-if-changed={header}");
        bindgen::Builder::default()
            .header(header)
            .clang_args(vcpkg_dir.iter().map(|dir| format!("-I{}/include", dir.display())))
            .allowlist_function(functions)
            .allowlist_type(types)
            .allowlist_var(variables)
            .default_enum_style(bindgen::EnumVariation::ModuleConsts)
            .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
            .generate()
            .unwrap_or_else(|error| panic!("Error generating {name} binding: {error}"))
            .write_to_file(out_dir.join(format!("{name}.rs")))
            .unwrap();
    }

    generate_language_unicode(&out_dir);
}
