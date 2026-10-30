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

/// Install vcpkg dependencies or return `None` if `RESILIPARSE_USE_SYSTEM_LIBS` is set.
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
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let install_root = env::var_os("RESILIPARSE_VCPKG_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("vcpkg_installed"));
    println!("cargo:rerun-if-env-changed=RESILIPARSE_VCPKG_PATH");
    println!("cargo:rerun-if-env-changed=VCPKG_DEFAULT_TRIPLET");
    for path in ["vcpkg.json", "vcpkg-configuration.json", "vcpkg"] {
        println!("cargo:rerun-if-changed={}", manifest_dir.join(path).display());
    }
    let triplet_root = install_root.join(&triplet);

    // Cross-builds may create the host triplet directory for build tools alone.
    // Let vcpkg check the installed packages instead of relying on that directory.
    let out = Command::new("vcpkg")
        .current_dir(&manifest_dir)
        .args(["install", "--triplet", &triplet, "--x-install-root"])
        .arg(&install_root)
        .env("CMAKE_POLICY_VERSION_MINIMUM", "3.5")
        .output()
        .expect("Failed to run vcpkg.");
    assert!(
        out.status.success(),
        "Failed to install vcpkg dependencies:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
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

/// Generate native bindings and linker instructions.
///
/// docs.rs cannot download dependencies, so we need to ship a set of pre-compiled bindings for that.
/// If the `DOCS_RS` variable is set, the function will copy the pre-generated bindings instead of
/// generating new ones from the headers.
fn generate_native_bindings(out_dir: &std::path::Path) {
    println!("cargo:rerun-if-env-changed=DOCS_RS");
    println!("cargo:rerun-if-changed=vcpkg.json");
    println!("cargo:rerun-if-env-changed=RESILIPARSE_USE_SYSTEM_LIBS");
    println!("cargo:rerun-if-env-changed=RESILIPARSE_STATIC_LIBS");

    let manifest: serde_json::Value = serde_json::from_str(include_str!("vcpkg.json")).expect("Invalid vcpkg manifest");
    let is_docs_rs = env::var_os("DOCS_RS").is_some();
    let vcpkg_dir = if is_docs_rs { None } else { get_vcpkg_dir() };
    // vcpkg deps are always linked statically
    let link_static = vcpkg_dir.is_some()
        || env::var("RESILIPARSE_STATIC_LIBS").is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"));

    for dependency in manifest["dependencies"].as_array().unwrap() {
        let Some(settings) = dependency.get("$bindgen") else {
            continue;
        };
        let dependency_name = dependency["name"].as_str().unwrap();
        let output_name = out_dir.join(format!("{dependency_name}.rs"));

        // If we're on docs.rs, use existing bindings instead of regenerating them.
        if is_docs_rs {
            let snapshot = format!("src/third_party/docs/{dependency_name}.rs");
            println!("cargo:rerun-if-changed={snapshot}");
            std::fs::copy(snapshot, output_name).expect("Failed to copy docs.rs bindings");
            continue;
        }

        if link_static {
            println!("cargo:rustc-link-lib=static={dependency_name}");
        } else {
            println!("cargo:rustc-link-lib={dependency_name}");
        }
        let header = format!("src/third_party/{dependency_name}.h");
        println!("cargo:rerun-if-changed={header}");
        let setting = |key: &str| settings[key].as_str().unwrap();
        bindgen::Builder::default()
            .header(header)
            .clang_args(vcpkg_dir.iter().map(|dir| format!("-I{}/include", dir.display())))
            .allowlist_function(setting("functions"))
            .allowlist_type(setting("types"))
            .allowlist_var(setting("variables"))
            .default_enum_style(bindgen::EnumVariation::ModuleConsts)
            .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
            .generate()
            .unwrap_or_else(|error| panic!("Error generating {dependency_name} binding: {error}"))
            .write_to_file(output_name)
            .unwrap();
    }
    if let Some(vcpkg_dir) = vcpkg_dir {
        println!("cargo:rustc-link-search=native={}/lib", vcpkg_dir.display());
    }
    if link_static && !is_docs_rs {
        match env::var("CARGO_CFG_TARGET_OS").as_deref() {
            Ok("linux") => println!("cargo:rustc-link-lib=dylib=stdc++"),
            Ok("macos") => println!("cargo:rustc-link-lib=dylib=c++"),
            _ => {}
        }
    }
}

fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    generate_native_bindings(&out_dir);
    generate_language_unicode(&out_dir);
}
