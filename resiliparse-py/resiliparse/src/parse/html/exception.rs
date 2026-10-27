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

use pyo3::create_exception;
use pyo3::exceptions::*;

create_exception!(
    resiliparse.parse.html.exception,
    DOMException,
    PyException,
    "Base exception for errors raised by the HTML DOM API."
);
pub use CSSParserException as CSSParserExceptionPy;

create_exception!(
    resiliparse.parse.html.exception,
    HTMLParserException,
    DOMException,
    "Exception raised when an HTML document cannot be parsed."
);
pub use DOMException as DOMExceptionPy;

create_exception!(
    resiliparse.parse.html.exception,
    CSSParserException,
    DOMException,
    "Exception raised when a CSS selector cannot be parsed."
);
pub use HTMLParserException as HTMLParserExceptionPy;
