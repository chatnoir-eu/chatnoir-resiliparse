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

mod stream_io;
mod warc;

use pyo3::prelude::*;
use pyo3::types::PyModule;

#[pymodule]
pub mod _fastwarc {
    use super::*;

    #[pymodule_init]
    pub fn __init__(m: &Bound<'_, PyModule>) -> PyResult<()> {
        let py = m.py();

        // Load submodules.
        // Don't add with add_submodule(), otherwise they end up in __all__.
        let stream_io = pyo3::wrap_pymodule!(_stream_io)(py);
        let stream_io = stream_io.bind(py);

        let warc = pyo3::wrap_pymodule!(_warc)(py);
        let warc = warc.bind(py);

        // Register submodules to make them importable.
        let sys = PyModule::import(py, "sys")?;
        let modules = sys.getattr("modules")?;
        modules.set_item("fastwarc.stream_io", stream_io)?;
        modules.set_item("fastwarc.warc", warc)?;

        // Top-level exports
        m.add("WarcRecordType", warc.getattr("WarcRecordType")?)?;
        m.add("WarcHeader", warc.getattr("WarcHeader")?)?;
        m.add("WarcRecord", warc.getattr("WarcRecord")?)?;
        m.add("ArchiveIterator", warc.getattr("ArchiveIterator")?)?;

        m.add("GzipReader", stream_io.getattr("GzipReader")?)?;
        m.add("GzipWriter", stream_io.getattr("GzipWriter")?)?;
        m.add("Lz4Reader", stream_io.getattr("Lz4Reader")?)?;
        m.add("Lz4Writer", stream_io.getattr("Lz4Writer")?)?;
        m.add("ZstdReader", stream_io.getattr("ZstdReader")?)?;
        m.add("ZstdWriter", stream_io.getattr("ZstdWriter")?)?;

        Ok(())
    }
}

#[pymodule(name = "stream_io", module = "fastwarc")]
pub mod _stream_io {
    #[pymodule_export]
    pub use crate::stream_io::{WarcReaderPy, WarcWriterPy};

    #[pymodule_export]
    pub use crate::stream_io::gzip::{GzipReaderPy, GzipWriterPy};

    #[pymodule_export]
    pub use crate::stream_io::zstd::{
        ZstdReaderPy, ZstdWriterPy, zstd_train_dictionary_from_continuous, zstd_train_dictionary_from_files,
        zstd_train_dictionary_from_samples,
    };

    #[pymodule_export]
    pub use crate::stream_io::lz4::{Lz4ReaderPy, Lz4WriterPy};

    #[pymodule_export]
    pub use crate::stream_io::brotli::{BrotliReaderPy, BrotliWriterPy};

    #[pymodule_export]
    pub use crate::stream_io::chunked::{ChunkedReaderPy, ChunkedWriterPy};
}

#[pymodule(name = "warc", module = "fastwarc")]
pub mod _warc {
    use super::*;

    #[pymodule_init]
    fn __init__(m: &Bound<'_, PyModule>) -> PyResult<()> {
        // Re-export enum members as individual constants.
        m.add("warcinfo", WarcRecordTypePy::warcinfo)?;
        m.add("response", WarcRecordTypePy::response)?;
        m.add("resource", WarcRecordTypePy::resource)?;
        m.add("request", WarcRecordTypePy::request)?;
        m.add("metadata", WarcRecordTypePy::metadata)?;
        m.add("revisit", WarcRecordTypePy::revisit)?;
        m.add("conversion", WarcRecordTypePy::conversion)?;
        m.add("continuation", WarcRecordTypePy::continuation)?;
        m.add("unknown", WarcRecordTypePy::unknown)?;
        m.add("no_type", WarcRecordTypePy::no_type)?;
        m.add("any_type", WarcRecordTypePy::any_type)?;

        // Legacy name
        m.add("WarcHeaderMap", m.getattr("HeaderMap")?)?;

        Ok(())
    }

    #[pymodule_export]
    pub use crate::warc::{
        ArchiveIteratorPy, HeaderMapPy, WarcHeaderPy, WarcRecordPayloadReaderPy, WarcRecordPy, WarcRecordTypePy,
        has_block_digest_py, has_content_length_gte_py, has_content_length_lte_py, has_payload_digest_py,
        has_record_type_py, has_valid_block_digest_py, has_valid_payload_digest_py, is_concurrent_py, is_http_py,
        is_warc_10_py, is_warc_11_py,
    };
}
