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

use pyo3::prelude::*;

pub mod coll;
pub mod exception;
pub mod node;
pub mod tree;

#[pymodule(name = "html", module = "resiliparse.parse")]
pub mod _html {
    use super::*;

    #[pymodule_init]
    pub fn __init__(m: &Bound<'_, PyModule>) -> PyResult<()> {
        let py = m.py();
        crate::set_package_path(m, "parse/html")?;

        // Load submodules.
        // Don't add with add_submodule(), otherwise they end up in __all__.
        let tree = pyo3::wrap_pymodule!(_tree)(py);
        let tree = tree.bind(py);
        let node = pyo3::wrap_pymodule!(_node)(py);
        let node = node.bind(py);
        let coll = pyo3::wrap_pymodule!(_coll)(py);
        let coll = coll.bind(py);
        let exception = pyo3::wrap_pymodule!(_exception)(py);
        let exception = exception.bind(py);

        // Register submodules to make them importable.
        let sys = PyModule::import(py, "sys")?;
        let modules = sys.getattr("modules")?;
        modules.set_item("resiliparse.parse.html.tree", tree)?;
        modules.set_item("resiliparse.parse.html.node", node)?;
        modules.set_item("resiliparse.parse.html.coll", coll)?;
        modules.set_item("resiliparse.parse.html.exception", exception)?;
        m.setattr("tree", tree)?;
        m.setattr("node", node)?;
        m.setattr("coll", coll)?;
        m.setattr("exception", exception)?;

        // Re-export the public API from the package submodules.
        for name in ["HTMLTree"] {
            m.add(name, tree.getattr(name)?)?;
        }
        for name in [
            "NodeType",
            "Node",
            "DOMNode",
            "ElementNode",
            "AttrNode",
            "TextNode",
            "CDATASectionNode",
            "ProcessingInstructionNode",
            "CommentNode",
            "DocumentNode",
            "DocumentTypeNode",
            "DocumentFragmentNode",
            "NotationNode",
            "DOMContext",
            "traverse_dom",
            "ELEMENT",
            "ATTRIBUTE",
            "TEXT",
            "CDATA_SECTION",
            "ENTITY_REFERENCE",
            "ENTITY",
            "PROCESSING_INSTRUCTION",
            "COMMENT",
            "DOCUMENT",
            "DOCUMENT_TYPE",
            "DOCUMENT_FRAGMENT",
            "NOTATION",
        ] {
            m.add(name, node.getattr(name)?)?;
        }
        for name in [
            "NodeList",
            "ElementNodeList",
            "DOMCollection",
            "HTMLCollection",
            "NamedNodeMap",
            "DOMTokenList",
            "DOMElementClassList",
        ] {
            m.add(name, coll.getattr(name)?)?;
        }
        for name in ["DOMException", "HTMLParserException", "CSSParserException"] {
            m.add(name, exception.getattr(name)?)?;
        }

        Ok(())
    }
}

#[pymodule(name = "tree", module = "resiliparse.parse.html")]
pub mod _tree {
    use super::*;

    #[pymodule_export]
    pub use self::tree::HTMLTreePy;
}

#[pymodule(name = "node", module = "resiliparse.parse.html")]
pub mod _node {
    use super::*;

    #[pymodule_init]
    pub fn __init__(m: &Bound<'_, PyModule>) -> PyResult<()> {
        // Export constants
        m.setattr("_node_cache", m.py().import("weakref")?.getattr("WeakValueDictionary")?.call0()?)?;
        let node_type = m.getattr("NodeType")?;
        for name in [
            "ELEMENT",
            "ATTRIBUTE",
            "TEXT",
            "CDATA_SECTION",
            "ENTITY_REFERENCE",
            "ENTITY",
            "PROCESSING_INSTRUCTION",
            "COMMENT",
            "DOCUMENT",
            "DOCUMENT_TYPE",
            "DOCUMENT_FRAGMENT",
            "NOTATION",
        ] {
            m.add(name, node_type.getattr(name)?)?;
        }

        // Register legacy alias names.
        m.add("DOMNode", m.getattr("Node")?)?;

        Ok(())
    }

    // Collections.
    #[pymodule_export]
    pub use self::coll::DOMTokenListPy;

    #[pymodule_export]
    pub use self::coll::ElementNodeListPy;

    #[pymodule_export]
    pub use self::coll::NamedNodeMapPy;

    #[pymodule_export]
    pub use self::coll::NodeListPy;

    // Node types.
    #[pymodule_export]
    pub use self::node::NodeTypePy;

    #[pymodule_export]
    pub use self::node::AttrNodePy;

    #[pymodule_export]
    pub use self::node::CDATASectionNodePy;

    #[pymodule_export]
    pub use self::node::CommentNodePy;

    #[pymodule_export]
    pub use self::node::DocumentFragmentNodePy;

    #[pymodule_export]
    pub use self::node::DocumentNodePy;

    #[pymodule_export]
    pub use self::node::DocumentTypeNodePy;

    #[pymodule_export]
    pub use self::node::DOMContextPy;

    #[pymodule_export]
    pub use self::node::ElementNodePy;

    #[pymodule_export]
    pub use self::node::NodePy;

    #[pymodule_export]
    pub use self::node::NotationNodePy;

    #[pymodule_export]
    pub use self::node::ProcessingInstructionNodePy;

    #[pymodule_export]
    pub use self::node::TextNodePy;

    // Helpers.
    #[pymodule_export]
    pub use self::node::traverse_dom;
}

#[pymodule(name = "coll", module = "resiliparse.parse.html")]
pub mod _coll {
    use super::*;

    #[pymodule_init]
    fn __init__(m: &Bound<'_, PyModule>) -> PyResult<()> {
        // Register legacy alias names.
        let elements = m.getattr("ElementNodeList")?;
        m.add("DOMCollection", &elements)?;
        m.add("HTMLCollection", &elements)?;

        let tokens = m.getattr("DOMTokenList")?;
        m.add("DOMElementClassList", &tokens)?;
        Ok(())
    }

    #[pymodule_export]
    pub use self::coll::DOMTokenListPy;

    #[pymodule_export]
    pub use self::coll::ElementNodeListPy;

    #[pymodule_export]
    pub use self::coll::NamedNodeMapPy;

    #[pymodule_export]
    pub use self::coll::NodeListPy;
}

#[pymodule(name = "exception", module = "resiliparse.parse.html")]
mod _exception {
    use super::*;

    #[pymodule_export]
    pub use self::exception::CSSParserExceptionPy;

    #[pymodule_export]
    pub use self::exception::DOMExceptionPy;

    #[pymodule_export]
    pub use self::exception::HTMLParserExceptionPy;
}
