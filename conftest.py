import os
import sys

# Keep directory handles alive for the entire test session.
_dll_directories = []

if sys.platform == 'win32' and sys.version_info >= (3, 8):
    # Restore pre-3.8 DLL loading behaviour for linked libraries in PATH,
    # including dependencies installed by Vcpkg.
    for path in os.environ.get('PATH', '').split(os.pathsep):
        if path and os.path.isdir(path):
            _dll_directories.append(os.add_dll_directory(path))
