import os
import sys

src_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), '..'))
sys.path.extend([
    os.path.join(src_dir, 'resiliparse'),
    os.path.join(src_dir, 'fastwarc')
])

if sys.platform == 'win32' and sys.version_info.minor >= 8:
    # Restore pre-3.8 DLL loading behaviour by adding PATH components to the DLL search paths.
    # We need this to load linked DLLs from non-standard places (such as Vcpkg), otherwise those
    # DLLs would have to be copied all over the place for running tests.
    for p in os.environ['PATH'].split(';'):
        if p:
            os.add_dll_directory(p)
