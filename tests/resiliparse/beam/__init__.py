import pytest
import sys

try:
    import apache_beam as beam
except ModuleNotFoundError:
    pytest.skip("Apache Beam dependency not installed.", allow_module_level=True)

if beam.__version__ < '2.37.0' and sys.version_info.minor >= 9:
    pytest.skip('Apache Beam < 2.37.0 not yet supported on Python 3.9 or higher.', allow_module_level=True)

if sys.version_info.minor > 9:
    pytest.skip('Apache Beam not yet supported on Python 3.10 or higher.', allow_module_level=True)
