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

//! Generated native bindings used by the encoding detector and HTML parser.
//!
//! The build script generates these bindings from Lexbor and uchardet headers. Public Rust
//! APIs wrap the native types and functions exposed here.

#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(dead_code)]

/// Bindings to Lexbor's HTML parsing, DOM, and selector APIs.
pub(crate) mod lexbor {
    include!(concat!(env!("OUT_DIR"), "/lexbor.rs"));
}

/// Bindings to uchardet's incremental character encoding detector.
pub(crate) mod uchardet {
    include!(concat!(env!("OUT_DIR"), "/uchardet.rs"));
}
