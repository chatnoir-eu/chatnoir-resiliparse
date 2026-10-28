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

/// Python argument conversion and bindings for HTML text extraction.
pub mod html2text;

use pyo3::prelude::*;

/// Native Python package exposing content extraction modules.
#[pymodule(name = "extract", module = "resiliparse")]
pub mod _extract {
    use super::*;

    /// Initialize the package path and register the native HTML text extraction submodule.
    #[pymodule_init]
    pub fn __init__(m: &Bound<'_, PyModule>) -> PyResult<()> {
        let py = m.py();
        crate::set_package_path(m, std::path::PathBuf::from("extract"))?;
        m.setattr("__all__", Vec::<String>::new())?;

        // Load submodules.
        // Don't add with add_submodule(), otherwise they end up in __all__.
        let html2text = pyo3::wrap_pymodule!(_html2text)(py);
        let html2text = html2text.bind(py);

        // Register submodules to make them importable.
        let sys = PyModule::import(py, "sys")?;
        let modules = sys.getattr("modules")?;
        modules.set_item("resiliparse.extract.html2text", html2text)?;
        m.setattr("html2text", html2text)?;

        Ok(())
    }
}

/// Extract text from HTML strings and parsed DOM trees.
#[pymodule(name = "html2text", module = "resiliparse.extract")]
pub mod _html2text {
    /// Public Python entry point for configurable HTML text extraction.
    #[pymodule_export]
    pub use super::html2text::extract_plain_text;
}
