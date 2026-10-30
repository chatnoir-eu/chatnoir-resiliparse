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

//! Parse HTML into a document tree and work with its nodes.
//!
//! [`tree`] provides the parser and owned document handle. [`dom`] exposes node
//! types, DOM operations, collections, and traversal, while [`selector`] supports
//! matching elements with CSS selectors.

pub mod dom;
pub mod selector;
pub mod tree;

mod lexbor;
mod serialize;

// ===========================================================
// Tests
// ===========================================================

#[cfg(test)]
#[path = "mod_test.rs"]
mod mod_test;
