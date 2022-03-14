import pytest

try:
    import apache_beam as beam
except ModuleNotFoundError:
    pytest.skip("Beam dependency not installed, skipping tests.", allow_module_level=True)
