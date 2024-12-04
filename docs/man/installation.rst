.. _resiliparse-installation:

Installation Instructions
=========================

.. note::

  These installation instructions are for the main Resiliparse module. For installing FastWARC, see :ref:`fastwarc-installation`.

Pre-built Resiliparse binaries can be installed from `PyPi <https://pypi.org/project/Resiliparse/>`_:

.. code-block:: console

  $ pip install resiliparse

To install the Resiliparse CLI and its dependencies, install the package with the `cli` flag (or alternatively the `all` flag):

.. code-block:: console

  $ pip install 'resiliparse[cli]'


Building Resiliparse From Source
--------------------------------

You can compile Resiliparse either from the `PyPi <https://pypi.org/project/Resiliparse/>`_ source package or directly from the `Github repository <https://github.com/chatnoir-eu/chatnoir-resiliparse>`_.

In either case, you need to install all required build-time dependencies listed in `vcpkg.json`. It's possible to install them globally via your package manager, but the easiest and most consistent way is to use [vcpkg](https://vcpkg.io/en/):

.. code:: bash

    # Install vcpkg itself (skip if you have a working vcpkg installation already)
    git clone https://github.com/Microsoft/vcpkg
    ./vcpkg/bootstrap-vcpkg.sh

    # Install dependencies to vcpkg_installed (must be run from sources root)
    ./vcpkg/vcpkg install --triplet=x64-linux

Replace the triplet value with one suitable for your platform. Valid values are: ``x64-windows``, ``x64-osx``, ``arm64-osx``, ``aarch64-linux`` (or any of the vcpkg default triplets).

After installing the dependencies, you can build the actual Python packages:

.. code:: bash

    # Create a fresh venv first (recommended)
    python3 -m venv venv && source venv/bin/activate

    # Option 1: Build and install in editable mode (best for development)
    python3 -m pip install -e ./fastwarc ./resiliparse

    # Option 2 (alternative): Build and install wheels in separate steps (best for redistribution)
    python3 -m pip wheel ./fastwarc ./resiliparse
    ls ./FastWARC-*.whl ./Resiliparse-*.whl | xargs python3 -m pip install

In most cases, the build routine should be smart enough to detect the location of the installed vcpkg dependencies. However, in some cases you may be getting errors about missing header files or undefined symbols. This can happen if you don't build from the source repository, use Python's new ``build`` module, or run ``pip wheel`` with ``--isolated``. To work around that, set the ``RESILIPARSE_VCPKG_PATH`` environment variable to the absolute path of the vcpkg installation directory:

.. code:: bash

    export RESILIPARSE_VCPKG_PATH="$(pwd)/vcpkg_installed"

.. note::

    Unless you fix up the wheels to embed the linked shared libraries (via `auditwheel <https://github.com/pypa/auditwheel>`__ on Linux, `delocate-wheel <https://github.com/matthew-brett/delocate>`__ on macOS, or `delvewheel <https://github.com/adang1345/delvewheel>`__ on Windows), you will have to add the vcpkg library directory (``vcpkg_installed/TRIPLET/lib``) to your library search path to use them.

    On Linux, add the directory path to the ``LD_LIBRARY_PATH`` environment variable, on macOS to ``DYLD_LIBRARY_PATH``. On Windows, you have to add the directory to the ``Path`` environment variable.

    Here's an example of how to use ``auditwheel`` on Linux to fix up the build wheels:

    .. code:: bash

        LD_LIBRARY_PATH=$(pwd)/vcpkg_installed/x64-linux/lib \
            auditwheel repair --plat linux_x86_64 build/Resiliparse*.whl build/FastWARC*.whl

    (Please note that ``linux_x86_64`` platform wheels are `not suitable for general redistribution <https://packaging.python.org/en/latest/specifications/platform-compatibility-tags/#platform-tag>`__.)
