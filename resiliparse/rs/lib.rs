use pyo3::prelude::*;

#[pyfunction]
fn hello_world(py: Python<'_>) {
    py.run("print('Hello World')", None, None).expect("");
}

#[pymodule]
fn rust_test(_py: Python<'_>, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(hello_world, m)?)?;
    Ok(())
}
