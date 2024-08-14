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

use pyo3::prelude::*;
use pyo3::types::*;

#[pymodule]
#[allow(unused_variables)]
mod _html_rs {
    use super::*;

    #[pyclass(eq, eq_int)]
    #[derive(PartialEq)]
    #[allow(non_camel_case_types)]
    enum NodeType {
        ELEMENT = 0x01,
        ATTRIBUTE = 0x02,
        TEXT = 0x03,
        CDATA_SECTION = 0x04,
        ENTITY_REFERENCE = 0x05,
        ENTITY = 0x06,
        PROCESSING_INSTRUCTION = 0x07,
        COMMENT = 0x08,
        DOCUMENT = 0x09,
        DOCUMENT_TYPE = 0x0A,
        DOCUMENT_FRAGMENT = 0x0B,
        NOTATION = 0x0C,
        LAST_ENTRY = 0x0D
    }

    #[pyclass]
    struct DOMCollection {}

    #[pymethods]
    //noinspection DuplicatedCode
    impl DOMCollection {
        #[pyo3(signature = (element_id, case_insensitive=false))]
        fn get_element_by_id(&self, element_id: &str, case_insensitive: Option<bool>) -> PyResult<DOMNode> {
            Ok(DOMNode {})
        }

        #[pyo3(signature = (attr_name, attr_value, case_insensitive=false))]
        fn get_elements_by_attr(&self, attr_name: &str, attr_value: &str, case_insensitive: Option<bool>) -> PyResult<DOMCollection> {
            Ok(DOMCollection {})
        }

        #[pyo3(signature = (class_name, case_insensitive=false))]
        fn get_elements_by_class_name(&self, class_name: &str, case_insensitive: Option<bool>) -> PyResult<DOMCollection> {
            Ok(DOMCollection {})
        }

        fn get_elements_by_tag_name(&self, tag_name: &str) -> PyResult<DOMCollection> {
            Ok(DOMCollection {})
        }

        fn query_selector(&self, selector: &str) -> PyResult<DOMNode> {
            Ok(DOMNode {})
        }

        fn query_selector_all(&self, selector: &str) -> PyResult<DOMCollection> {
            Ok(DOMCollection {})
        }

        fn matches(&self, selector: &str) -> PyResult<bool> {
            Ok(false)
        }
    }

    #[pyclass]
    struct DOMElementClassList {}

    #[pymethods]
    impl DOMElementClassList {
        #[new]
        fn __new__() -> Self {
            DOMElementClassList {}
        }

        fn add(&self, class_name: &str) -> PyResult<()> {
            Ok(())
        }

        fn remove(&mut self, class_name: &str) -> PyResult<()> {
            Ok(())
        }
    }

    #[pyclass]
    struct DOMNode {}

    #[pymethods]
    impl DOMNode {
        #[new]
        fn __new__() -> Self {
            DOMNode {}
        }

        #[getter]
        #[pyo3(name = "type")]
        fn type_(&self) -> PyResult<NodeType> {
            Ok(NodeType::ELEMENT)
        }

        #[getter]
        fn first_child(&self) -> PyResult<DOMNode> {
            Ok(DOMNode {})
        }

        #[getter]
        fn first_element_child(&self) -> PyResult<DOMNode> {
            Ok(DOMNode {})
        }

        #[getter]
        fn last_element_child(&self) -> PyResult<DOMNode> {
            Ok(DOMNode {})
        }

        #[getter]
        fn child_nodes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
            Ok(PyList::empty_bound(py))
        }

        #[getter]
        fn parent(&self) -> PyResult<DOMNode> {
            Ok(DOMNode {})
        }

        #[getter]
        fn next(&self) -> PyResult<DOMNode> {
            Ok(DOMNode {})
        }

        #[getter]
        fn prev(&self) -> PyResult<DOMNode> {
            Ok(DOMNode {})
        }

        #[getter]
        fn next_element(&self) -> PyResult<DOMNode> {
            Ok(DOMNode {})
        }

        #[getter]
        fn prev_element(&self) -> PyResult<DOMNode> {
            Ok(DOMNode {})
        }

        #[getter]
        fn tag(&self) -> PyResult<&str> {
            Ok("")
        }

        #[getter]
        fn value(&self) -> PyResult<&str> {
            Ok("")
        }

        #[getter]
        fn get_text(&self) -> PyResult<&str> {
            Ok("")
        }

        #[setter]
        fn set_text(&mut self, text: &str) -> PyResult<()> {
            Ok(())
        }

        #[getter]
        fn get_html(&self) -> PyResult<&str> {
            Ok("")
        }

        #[setter]
        fn set_html(&mut self, html: &str) -> PyResult<()> {
            Ok(())
        }

        #[getter]
        fn get_id(&self) -> PyResult<&str> {
            Ok("")
        }

        #[setter]
        fn set_id(&mut self, id: &str) -> PyResult<()> {
            Ok(())
        }

        #[getter]
        fn get_class_name(&self) -> PyResult<&str> {
            Ok("")
        }

        #[setter]
        fn set_class_name(&mut self, class_name: &str) -> PyResult<()> {
            Ok(())
        }

        #[getter]
        fn get_class_list(&self) -> PyResult<DOMElementClassList> {
            Ok(DOMElementClassList {})
        }

        #[getter]
        fn attrs<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
            Ok(PyTuple::empty_bound(py))
        }

        fn hasattr(&self, attr_name: &str) -> PyResult<bool> {
            Ok(false)
        }

        #[pyo3(signature = (attr_name, default_value=None))]
        fn getattr(&self, attr_name: &str, default_value: Option<&str>) -> PyResult<&str> {
            Ok("...")
        }

        fn setattr(&mut self, attr_name: &str, attr_value: &str) -> PyResult<()> {
            Ok(())
        }

        fn delattr(&mut self, attr_name: &str) -> PyResult<()> {
            Ok(())
        }

        #[pyo3(signature = (element_id, case_insensitive=false))]
        fn get_element_by_id(&self, element_id: &str, case_insensitive: Option<bool>) -> PyResult<DOMNode> {
            Ok(DOMNode {})
        }

        #[pyo3(signature = (attr_name, attr_value, case_insensitive=false))]
        fn get_elements_by_attr(&self, attr_name: &str, attr_value: &str, case_insensitive: Option<bool>) -> PyResult<DOMCollection> {
            Ok(DOMCollection {})
        }

        #[pyo3(signature = (class_name, case_insensitive=false))]
        fn get_elements_by_class_name(&self, class_name: &str, case_insensitive: Option<bool>) -> PyResult<DOMCollection> {
            Ok(DOMCollection {})
        }

        fn get_elements_by_tag_name(&self, tag_name: &str) -> PyResult<DOMCollection> {
            Ok(DOMCollection {})
        }

        fn query_selector(&self, selector: &str) -> PyResult<DOMNode> {
            Ok(DOMNode {})
        }

        fn query_selector_all(&self, selector: &str) -> PyResult<DOMCollection> {
            Ok(DOMCollection {})
        }

        fn matches(&self, selector: &str) -> PyResult<bool> {
            Ok(false)
        }

        fn append_child(&mut self, node: &DOMNode) -> PyResult<DOMNode> {
            Ok(DOMNode {})
        }

        fn insert_before(&mut self, node: &DOMNode, reference: &DOMNode) -> PyResult<DOMNode> {
            Ok(DOMNode {})
        }

        fn replace_child(&mut self, new_child: &DOMNode, old_child: &DOMNode) -> PyResult<DOMNode> {
            Ok(DOMNode {})
        }

        fn remove_child(&mut self, node: &DOMNode) -> PyResult<DOMNode> {
            Ok(DOMNode {})
        }

        fn decompose(&mut self) -> PyResult<()> {
            Ok(())
        }
    }

    #[pyclass]
    struct HTMLTree {}

    #[pymethods]
    impl HTMLTree {
        #[staticmethod]
        fn parse(document: &str) -> PyResult<Self> {
            Ok(Self {})
        }

        #[staticmethod]
        #[pyo3(signature = (document, encoding="utf-8", errors="ignore"))]
        fn parse_from_bytes(document: &[u8], encoding: &str, errors: &str) -> PyResult<Self> {
            Ok(Self {})
        }

        fn create_element(&mut self, tag_name: &str) -> PyResult<DOMNode> {
            Ok(DOMNode {})
        }

        fn create_text_node(&mut self, text: &str) -> PyResult<DOMNode> {
            Ok(DOMNode {})
        }

        #[getter]
        fn document(&self) -> PyResult<DOMNode> {
            Ok(DOMNode {})
        }

        #[getter]
        fn head(&self) -> PyResult<DOMNode> {
            Ok(DOMNode {})
        }

        #[getter]
        fn body(&self) -> PyResult<DOMNode> {
            Ok(DOMNode {})
        }

        #[getter]
        fn title(&self) -> PyResult<&str> {
            Ok("...")
        }
    }
}
