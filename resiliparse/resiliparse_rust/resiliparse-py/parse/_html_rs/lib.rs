// Copyright 2024 Janek Bevendorff
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

mod node;
mod coll;
mod tree;

use pyo3::create_exception;
use pyo3::prelude::*;
use pyo3::exceptions::*;

create_exception!(_html_rs, DOMException, PyException);


#[pymodule]
pub mod _html_rs {
    #[pymodule_export]
    pub use super::DOMException;

    #[pymodule_export]
    pub use super::coll::DOMCollection;

    #[pymodule_export]
    pub use super::coll::DOMElementClassList;

    #[pymodule_export]
    pub use super::node::NodeType;

    #[pymodule_export]
    pub use super::node::DOMNode;

    #[pymodule_export]
    pub use super::tree::HTMLTree;

    #[pymodule_export]
    pub use super::tree::HTMLParserException;
}
