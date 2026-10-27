import pytest

try:
    import apache_beam as beam
except ModuleNotFoundError:
    pytest.skip("Dependency apache_beam not installed.", allow_module_level=True)
