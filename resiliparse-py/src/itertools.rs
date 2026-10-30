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

use pyo3::exceptions::PyStopIteration;
use pyo3::prelude::*;
use pyo3::types::{PyModule, PyTuple};

fn make_iterator(value: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    Ok(value.try_iter()?.into_any().unbind())
}

fn next_item<'py>(iterator: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    iterator.call_method0("__next__")
}

fn is_stop_iteration(error: &PyErr, py: Python<'_>) -> bool {
    error.is_instance_of::<PyStopIteration>(py)
}

#[pyclass(module = "resiliparse.itertools", name = "_ExceptionLoop")]
struct ExceptionLoop {
    source: Py<PyAny>,
    iterator: Option<Py<PyAny>>,
    done: bool,
}

#[pymethods]
impl ExceptionLoop {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        if self.done {
            return Ok(None);
        }
        if self.iterator.is_none() {
            match make_iterator(self.source.bind(py)) {
                Ok(iterator) => self.iterator = Some(iterator),
                Err(error) => {
                    self.done = true;
                    return Err(error);
                }
            }
        }

        match next_item(self.iterator.as_ref().unwrap().bind(py)) {
            Ok(item) => {
                let result = PyTuple::new(py, [item, py.None().into_bound(py)])?;
                Ok(Some(result.into_any().unbind()))
            }
            Err(error) if is_stop_iteration(&error, py) => {
                self.done = true;
                Ok(None)
            }
            Err(error) => {
                let result = PyTuple::new(py, [py.None().into_bound(py), error.value(py).clone().into_any()])?;
                Ok(Some(result.into_any().unbind()))
            }
        }
    }
}

/// exc_loop(it)
///
/// Wraps an iterator into another iterator that catches and returns any exceptions raised
/// while evaluating the input iterator.
///
/// This is primarily useful for unreliable generators that may throw unpredictably at any time
/// for unknown reasons (e.g., generators reading from a network data source). If you do not want to
/// wrap the entire loop in a ``try/except`` clause, you can use an :func:`exc_loop` to catch
/// any such exceptions and return them.
/// Remember that a generator will end after throwing an exception, so if the input iterator is
/// a generator, you will have to create a new instance in order to retry or continue.
///
/// :param it: original iterator
/// :type it: t.Iterable[t.Any]
/// :return: iterator of items and ``None`` or ``None`` and exception instance
/// :rtype: t.Iterable[(t.Any or None, BaseException or None)]
#[pyfunction]
fn exc_loop(it: Py<PyAny>, py: Python<'_>) -> PyResult<Py<ExceptionLoop>> {
    Py::new(
        py,
        ExceptionLoop {
            source: it,
            iterator: None,
            done: false,
        },
    )
}

#[pyclass(module = "resiliparse.itertools", name = "_ProgressLoop")]
struct ProgressLoop {
    source: Py<PyAny>,
    context: Py<PyAny>,
    iterator: Option<Py<PyAny>>,
    progress_pending: bool,
    done: bool,
}

#[pymethods]
impl ProgressLoop {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        if self.done {
            return Ok(None);
        }
        if self.progress_pending {
            self.progress_pending = false;
            let result = PyModule::import(py, "resiliparse.process_guard")
                .and_then(|module| module.call_method1("progress", (self.context.bind(py),)));
            if let Err(error) = result {
                self.done = true;
                return Err(error);
            }
        }
        if self.iterator.is_none() {
            match make_iterator(self.source.bind(py)) {
                Ok(iterator) => self.iterator = Some(iterator),
                Err(error) => {
                    self.done = true;
                    return Err(error);
                }
            }
        }

        match next_item(self.iterator.as_ref().unwrap().bind(py)) {
            Ok(item) => {
                self.progress_pending = true;
                Ok(Some(item.unbind()))
            }
            Err(error) if is_stop_iteration(&error, py) => {
                self.done = true;
                Ok(None)
            }
            Err(error) => {
                self.done = true;
                Err(error)
            }
        }
    }
}

fn make_progress_loop(it: Py<PyAny>, ctx: Option<Py<PyAny>>, py: Python<'_>) -> PyResult<Py<ProgressLoop>> {
    Py::new(
        py,
        ProgressLoop {
            source: it,
            context: ctx.unwrap_or_else(|| py.None()),
            iterator: None,
            progress_pending: false,
            done: false,
        },
    )
}

/// Wraps an iterator into a pass-through iterator that reports progress to an active
/// :class:`resiliparse.process_guard.TimeGuard` context guard after each iteration.
///
/// :param it: original iterator
/// :type it: t.Iterable[t.Any]
/// :param ctx: active guard context (will use last global context from the call stack if unset)
/// :return: wrapped iterator
/// :rtype: t.Iterable[t.Any]
#[pyfunction]
#[pyo3(name = "_process_guard_progress_loop", signature = (it, ctx=None))]
fn process_guard_progress_loop(it: Py<PyAny>, ctx: Option<Py<PyAny>>, py: Python<'_>) -> PyResult<Py<ProgressLoop>> {
    make_progress_loop(it, ctx, py)
}

#[pyclass(module = "resiliparse.itertools", name = "_WarcRetry")]
struct WarcRetry {
    archive_iterator: Py<PyAny>,
    stream_factory: Py<PyAny>,
    retry_count: Option<usize>,
    seek: Option<bool>,
    iterator: Option<Py<PyAny>>,
    pos: Option<usize>,
    first_pos: Option<usize>,
    retries: usize,
    skip_next: bool,
    done: bool,
}

impl WarcRetry {
    fn retry_limit_reached(&self) -> bool {
        self.retry_count
            .is_some_and(|retry_count| retry_count != 0 && self.retries > retry_count)
    }

    fn initialize(&mut self, py: Python<'_>) -> PyResult<()> {
        // A fresh iterator starts at the stream's beginning. Its current
        // position is otherwise unavailable until it yields its first record.
        let pos = 0;
        self.pos = Some(pos);
        self.first_pos = Some(pos);
        self.iterator = Some(make_iterator(self.archive_iterator.bind(py))?);
        Ok(())
    }

    fn replacement_stream(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        let pos = self.pos.unwrap();
        let factory = self.stream_factory.bind(py);
        match self.seek {
            Some(true) => {
                let stream = factory.call0()?;
                stream.call_method1("seek", (pos,))?;
                Ok(Some(stream.unbind()))
            }
            Some(false) => Ok(Some(factory.call1((pos,))?.unbind())),
            None => {
                let stream = factory.call0()?;
                let mut consumed = 0usize;
                while consumed < pos {
                    let size = (pos - consumed).min(16384);
                    let data = stream.call_method1("read", (size,))?;
                    let read = data.len()?;
                    if read == 0 {
                        return Ok(None);
                    }
                    consumed += read;
                }
                Ok(Some(stream.unbind()))
            }
        }
    }

    fn recover(&mut self, py: Python<'_>) -> PyResult<bool> {
        let stream = loop {
            match self.replacement_stream(py) {
                Ok(Some(stream)) => break stream,
                Ok(None) => {
                    self.done = true;
                    return Ok(false);
                }
                Err(error) => {
                    self.retries = self.retries.saturating_add(1);
                    if self.retry_limit_reached() {
                        self.done = true;
                        return Err(error);
                    }
                }
            }
        };

        self.archive_iterator = self
            .archive_iterator
            .bind(py)
            .call_method1("restart", (stream.bind(py),))?
            .unbind();
        self.iterator = Some(make_iterator(self.archive_iterator.bind(py))?);
        self.skip_next = self.pos.unwrap() > self.first_pos.unwrap();
        Ok(true)
    }
}

#[pymethods]
impl WarcRetry {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        if self.done {
            return Ok(None);
        }
        if self.iterator.is_none() {
            if let Err(error) = self.initialize(py) {
                self.done = true;
                return Err(error);
            }
        }

        loop {
            let next = (|| {
                let iterator = self.iterator.as_ref().unwrap().bind(py);
                if self.skip_next {
                    next_item(iterator)?;
                    self.skip_next = false;
                }
                next_item(iterator)
            })();

            match next {
                Ok(record) => {
                    match record.getattr("stream_pos").and_then(|pos| pos.extract::<usize>()) {
                        Ok(pos) => self.pos = Some(pos),
                        Err(error) => {
                            self.done = true;
                            return Err(error);
                        }
                    }
                    return Ok(Some(record.unbind()));
                }
                Err(error) if is_stop_iteration(&error, py) => {
                    self.done = true;
                    return Ok(None);
                }
                Err(error) => {
                    self.retries = self.retries.saturating_add(1);
                    if self.retry_limit_reached() {
                        self.done = true;
                        return Err(error);
                    }
                    match self.recover(py) {
                        Ok(true) => {}
                        Ok(false) => return Ok(None),
                        Err(recovery_error) => {
                            self.done = true;
                            return Err(recovery_error);
                        }
                    }
                }
            }
        }
    }
}

/// warc_retry(archive_iterator, stream_factory, retry_count=3, seek=True)
///
/// Wrap a :class:`fastwarc.warc.ArchiveIterator` to try to continue reading after a stream failure.
///
/// Use if the underlying stream is unreliable, such as when reading from a network data source.
/// If an exception other than :exc:`StopIteration` is raised while consuming the iterator, the WARC
/// reading process will be retried up to `retry_count` times. When a stream failure occurs,
/// ``archive_iterator`` will be reinitialised with a new stream object by calling ``stream_factory``.
///
/// The new stream object returned by ``stream_factory()`` must be seekable. If the stream does not
/// support seeking, you can set ``seek=False``. In this case, the stream position in bytes of the last
/// successfully read record will be passed as a parameter to ``stream_factory()``. The factory is then
/// expected to return a stream that already starts at this exact position (or else reading would
/// restart from the beginning resulting in duplicate records). This is primarily useful for streams
/// that are not inherently seekable, but have an external facility for starting them at the correct
/// position (such as S3 HTTPS streams created from range requests).
///
/// As another option, ``seek`` can also be ``None``, which instructs :func:`warc_retry` to consume the
/// stream up to the continuation position. The stream returned by ``stream_factory()`` must start at
/// the beginning and will be read normally, but all bytes before the last record will be skipped over
/// before continuing to parse the contents. This is the most expensive method of "seeking" on a stream
/// and should only be used if the stream is not seekable and there is no other option for starting it
/// at the correct offset.
///
/// Exceptions raised inside ``stream_factory()`` will be caught and count towards ``retry_count``.
///
/// :param archive_iterator: input WARC iterator
/// :type archive_iterator: fastwarc.warc.ArchiveIterator
/// :param stream_factory: callable returning a new stream instance to continue iteration in case of failure
/// :type stream_factory: t.Callable[[], t.Any] or t.Callable[[int], t.Any]
/// :param retry_count: maximum number of retries before giving up (set to ``None`` or zero for no limit)
/// :type retry_count: int
/// :param seek: whether to seek to previous position on new stream object (or ``None`` for "stream consumption")
/// :type seek: bool or None
/// :return: wrapped :class:`~fastwarc.warc.ArchiveIterator`
/// :rtype: t.Iterable[fastwarc.warc.WarcRecord]
#[pyfunction]
#[pyo3(signature = (archive_iterator, stream_factory, retry_count=3, seek=true))]
fn warc_retry(
    archive_iterator: Py<PyAny>,
    stream_factory: Py<PyAny>,
    retry_count: Option<usize>,
    seek: Option<bool>,
    py: Python<'_>,
) -> PyResult<Py<WarcRetry>> {
    Py::new(
        py,
        WarcRetry {
            archive_iterator,
            stream_factory,
            retry_count,
            seek,
            iterator: None,
            pos: None,
            first_pos: None,
            retries: 0,
            skip_next: false,
            done: false,
        },
    )
}

#[pymodule(name = "itertools", module = "resiliparse")]
pub mod _itertools {
    use super::*;

    #[pymodule_init]
    pub fn __init__(m: &Bound<'_, PyModule>) -> PyResult<()> {
        m.add_class::<ExceptionLoop>()?;
        m.add_class::<ProgressLoop>()?;
        m.add_class::<WarcRetry>()?;
        m.add_function(wrap_pyfunction!(exc_loop, m)?)?;
        m.add_function(wrap_pyfunction!(process_guard_progress_loop, m)?)?;
        m.add_function(wrap_pyfunction!(warc_retry, m)?)?;
        m.setattr("__all__", ["exc_loop", "warc_retry"])?;
        Ok(())
    }
}
