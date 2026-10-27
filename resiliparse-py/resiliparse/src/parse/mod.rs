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

pub mod html;

use pyo3::prelude::*;

#[pymodule(name = "parse", module = "resiliparse")]
pub mod _parse {
    use super::*;

    #[pymodule_init]
    pub fn __init__(m: &Bound<'_, PyModule>) -> PyResult<()> {
        let py = m.py();
        crate::set_package_path(m, "parse")?;
        m.setattr("__all__", Vec::<String>::new())?;

        // Load submodules.
        // Don't add with add_submodule(), otherwise they end up in __all__.
        let html = pyo3::wrap_pymodule!(html::_html)(py);
        let html = html.bind(py);

        // Register submodules to make them importable.
        let sys = PyModule::import(py, "sys")?;
        let modules = sys.getattr("modules")?;
        modules.set_item("resiliparse.parse.html", html)?;
        m.setattr("html", html)?;

        Ok(())
    }
}
