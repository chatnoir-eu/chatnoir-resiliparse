.. _resiliparse-installation:

Installation Instructions
=========================

.. note::

  These installation instructions cover the Resiliparse Python package. For installing FastWARC, see
  :ref:`fastwarc-installation` For the Rust crate, see the `Rust documentation <https://docs.rs/fastwarc/latest/fastwarc/>`__.


Installing for Python
---------------------

Pre-built Resiliparse binaries can be installed from `PyPI <https://pypi.org/project/Resiliparse/>`__:

.. code-block:: console

  $ pip install resiliparse

To install the Resiliparse CLI and its dependencies, install the package with the `cli` flag (or alternatively the `all`
flag):

.. code-block:: console

  $ pip install 'resiliparse[cli]'

Building Resiliparse From Source
--------------------------------

Prerequisites
^^^^^^^^^^^^^

The Python bindings require Rust and Cargo, a C/C++ compiler, CMake, and libclang. Install the Rust toolchain with
`rustup <https://rustup.rs/>`__ if necessary:

.. code:: bash

    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

To build from the `GitHub repository <https://github.com/chatnoir-eu/chatnoir-resiliparse>`__, clone the sources and
make `vcpkg <https://vcpkg.io/>`__ available on your ``PATH``:

.. code:: bash

    git clone https://github.com/chatnoir-eu/chatnoir-resiliparse.git
    cd chatnoir-resiliparse

    # Install vcpkg itself (skip if you have a working vcpkg installation already)
    git clone https://github.com/microsoft/vcpkg.git
    ./vcpkg/bootstrap-vcpkg.sh
    export PATH="$PWD/vcpkg:$PATH"

On Windows, run ``vcpkg/bootstrap-vcpkg.bat`` and add the ``vcpkg`` directory to ``PATH``.

The Resiliparse crate includes its native build manifests and overlays. The build script installs native dependencies
through vcpkg under Cargo's build output directory and links them statically.

Building the Python Bindings
^^^^^^^^^^^^^^^^^^^^^^^^^^^^

The Python bindings use PyO3 and maturin. Pip installs the build backend automatically. From the repository root,
build FastWARC alongside Resiliparse so that the matching Python dependency is available locally:

.. code:: bash

    # Create a fresh venv first (recommended)
    python3 -m venv venv && source venv/bin/activate

    # Option 1: Build and install in editable mode (best for development)
    python3 -m pip install -e ./fastwarc-py -e ./resiliparse-py

    # Option 2 (alternative): Build and install wheels in separate steps (best for redistribution)
    python3 -m pip wheel -w build ./fastwarc-py ./resiliparse-py
    python3 -m pip install --no-index --find-links build resiliparse

On Windows, activate the virtual environment with ``venv\Scripts\activate``.

Using Existing Native Libraries
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

For either Rust or Python builds, set ``RESILIPARSE_VCPKG_PATH`` to use an existing vcpkg installation. It must point to
the directory containing the platform triplet directories:

.. code:: bash

    export RESILIPARSE_VCPKG_PATH="$(pwd)/vcpkg_installed"

Alternatively, set ``RESILIPARSE_USE_SYSTEM_LIBS=1`` to skip vcpkg and use system headers and shared libraries. Make
their include and library directories available to the compiler and linker.

Set ``RESILIPARSE_STATIC_LIBS=1`` as well to link system libraries statically instead. Their static archives must be
available to rustc; use ``RUSTFLAGS="-L native=/path/to/lib"`` for non-standard library directories. The C++ runtime remains
dynamically linked on Linux and macOS. The Docker image enables static system libraries by default.
