import pytest

try:
    import apache_beam.io
except ModuleNotFoundError as exc:
    pytest.skip(f"Apache Beam test dependencies unavailable: {exc.name}", allow_module_level=True)
