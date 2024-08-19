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

mod coll;
mod exception;
mod node;
mod tree;

use pyo3::prelude::*;


macro_rules! init_mod_path {
    ($name:literal, $m:ident) => {
        // https://github.com/PyO3/pyo3/issues/759#issuecomment-2282197848
        Python::with_gil(|py| {
            py.import_bound("sys")?
                .getattr("modules")?
                .set_item(concat!("resiliparse.parse._html_rs.", $name), $m)
        })
    };
}


#[pymodule]
pub mod _html_rs {
    use super::*;

    #[pymodule_export]
    pub use crate::tree::HTMLTree;

    #[pymodule]
    pub mod node {
        use super::*;

        #[pymodule_init]
        fn __init__(m: &Bound<'_, PyModule>) -> PyResult<()> {
            init_mod_path!("node", m)
        }

        #[pymodule_export]
        pub use crate::node::NodeType;

        #[pymodule_export]
        pub use crate::node::Node;

        #[pymodule_export]
        pub use crate::node::ElementNode;

        #[pymodule_export]
        pub use crate::node::AttrNode;

        #[pymodule_export]
        pub use crate::node::TextNode;

        #[pymodule_export]
        pub use crate::node::CdataSectionNode;

        #[pymodule_export]
        pub use crate::node::ProcessingInstructionNode;

        #[pymodule_export]
        pub use crate::node::CommentNode;

        #[pymodule_export]
        pub use crate::node::DocumentNode;

        #[pymodule_export]
        pub use crate::node::DocumentTypeNode;

        #[pymodule_export]
        pub use crate::node::DocumentFragmentNode;
    }

    #[pymodule]
    pub mod coll {
        use super::*;

        #[pymodule_init]
        fn __init__(m: &Bound<'_, PyModule>) -> PyResult<()> {
            init_mod_path!("coll", m)
        }

        #[pymodule_export]
        pub use crate::coll::NodeList;

        #[pymodule_export]
        pub use crate::coll::ElementNodeList;

        // #[pymodule_export]
        // pub use crate::coll::NamedNodeMap;
        //
        // #[pymodule_export]
        // pub use crate::coll::DOMTokenList;
    }

    #[pymodule]
    pub mod exception {
        use super::*;

        #[pymodule_init]
        fn __init__(m: &Bound<'_, PyModule>) -> PyResult<()> {
            init_mod_path!("exception", m)
        }

        #[pymodule_export]
        pub use crate::exception::DOMException;

        #[pymodule_export]
        pub use crate::exception::HTMLParserException;

        #[pymodule_export]
        pub use crate::exception::CSSParserException;
    }
}
