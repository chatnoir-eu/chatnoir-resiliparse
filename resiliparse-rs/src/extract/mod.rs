// Copyright 2026 Janek Bevendorff
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

//! Extract readable text and main content from parsed HTML documents.
//!
//! [`html2text::extract_plain_text`] converts an HTML tree into text without modifying it.
//! [`html2text::ExtractOptions`] controls formatting, link targets, alternative text, and
//! heuristic boilerplate removal; [`html2text::Formatting`] selects plain text or reduced HTML output.

mod filters;

pub mod html2text;
