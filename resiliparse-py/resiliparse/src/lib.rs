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

mod extract;
mod parse;

use pyo3::prelude::*;

/// Let native packages discover Python modules alongside the extension.
fn set_package_path(m: &Bound<'_, PyModule>, subpath: &str) -> PyResult<()> {
    let package = PyModule::import(m.py(), "resiliparse")?;
    let paths: Vec<String> = package.getattr("__path__")?.extract()?;
    let paths: Vec<_> = paths
        .into_iter()
        .map(|path| std::path::Path::new(&path).join(subpath).to_string_lossy().into_owned())
        .collect();
    m.setattr("__path__", paths)
}

#[pymodule]
pub mod _resiliparse {
    use super::*;

    #[pymodule_init]
    pub fn __init__(m: &Bound<'_, PyModule>) -> PyResult<()> {
        let py = m.py();
        m.setattr("__all__", Vec::<String>::new())?;

        // Load submodules.
        // Don't add with add_submodule(), otherwise they end up in __all__.
        let parse = pyo3::wrap_pymodule!(parse::_parse)(py);
        let parse = parse.bind(py);

        let extract = pyo3::wrap_pymodule!(extract::_extract)(py);
        let extract = extract.bind(py);

        // Register submodules to make them importable.
        let sys = PyModule::import(py, "sys")?;
        let modules = sys.getattr("modules")?;
        modules.set_item("resiliparse.parse", parse)?;
        modules.set_item("resiliparse.extract", extract)?;

        // Keep the original uniform-node API at the historical entry point.
        // The native submodules retain their typed DOM classes.
        let compat = py.import("resiliparse.parse.html._compat")?;
        let html = parse.getattr("html")?;
        for name in compat.getattr("__all__")?.extract::<Vec<String>>()? {
            html.setattr(&name, compat.getattr(&name)?)?;
        }

        Ok(())
    }
}
