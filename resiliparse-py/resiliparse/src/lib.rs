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
mod itertools;
mod parse;
mod process_guard;

use pyo3::prelude::*;
use std::path::PathBuf;

/// Set a native module's `__path__` to the matching directory in the Python package.
///
/// PyO3 creates these native modules separately from their Python packages. Extending
/// their package paths lets Python find sibling modules. The `subpath` is a `PathBuf`
/// so callers can build it from components and preserve platform-native separators.
fn set_package_path(m: &Bound<'_, PyModule>, subpath: PathBuf) -> PyResult<()> {
    let package = PyModule::import(m.py(), "resiliparse")?;
    let paths: Vec<String> = package.getattr("__path__")?.extract()?;
    let paths: Vec<_> = paths
        .into_iter()
        .map(|path| {
            std::path::Path::new(&path)
                .join(&subpath)
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    m.setattr("__path__", paths)
}

/// Add lazy loader functions for submodules.
macro_rules! native_package_loader {
    ($loader:ident, $module:path) => {
        #[pyfunction]
        fn $loader(py: Python<'_>) -> Py<PyModule> {
            pyo3::wrap_pymodule!($module)(py)
        }
    };
}

native_package_loader!(_load_parse, parse::_parse);
native_package_loader!(_load_extract, extract::_extract);
native_package_loader!(_load_process_guard, process_guard::_process_guard);

#[pymodule]
pub mod _resiliparse {
    use super::*;

    #[pymodule_init]
    pub fn __init__(m: &Bound<'_, PyModule>) -> PyResult<()> {
        m.add_function(wrap_pyfunction!(_load_parse, m)?)?;
        m.add_function(wrap_pyfunction!(_load_extract, m)?)?;
        m.add_function(wrap_pyfunction!(_load_process_guard, m)?)?;
        let py = m.py();
        let sys = PyModule::import(py, "sys")?;
        let modules = sys.getattr("modules")?;
        let itertools = pyo3::wrap_pymodule!(itertools::_itertools)(py);
        modules.set_item("resiliparse.itertools", itertools.bind(py))?;
        PyModule::import(py, "resiliparse")?.setattr("itertools", itertools)?;
        m.setattr("__all__", Vec::<String>::new())?;
        Ok(())
    }
}
