.. _resiliparse-installation:

Installation Instructions
=========================

.. note::

  These installation instructions are for the main Resiliparse module. For installing FastWARC, see :ref:`fastwarc-installation`.

Pre-built Resiliparse binaries can be installed from `PyPi <https://pypi.org/project/Resiliparse/>`__:

.. code-block:: console

  $ pip install resiliparse

To install the Resiliparse CLI and its dependencies, install the package with the `cli` flag (or alternatively the `all` flag):

.. code-block:: console

  $ pip install 'resiliparse[cli]'


Building Resiliparse From Source
--------------------------------

You can compile Resiliparse either from the `PyPi <https://pypi.org/project/Resiliparse/>`__source package or directly from the `Github repository <https://github.com/chatnoir-eu/chatnoir-resiliparse>`_.

In either case, you need to install all required build-time dependencies listed in `vcpkg.json`. It's possible to install them globally via your package manager, but the easiest and most consistent way is to use [vcpkg](https://vcpkg.io/en/):

.. code:: bash

    # Install vcpkg itself (skip if you have a working vcpkg installation already)
    git clone https://github.com/Microsoft/vcpkg
    ./vcpkg/bootstrap-vcpkg.sh

    # Install dependencies to vcpkg_installed (must be run from sources root)
    ./vcpkg/vcpkg install --triplet=x64-linux

Replace the triplet value with one suitable for your platform. Valid values are: ``x64-windows``, ``x64-osx``, ``arm64-osx``, ``arm64-linux`` (or any of the vcpkg default triplets).

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

    Native vcpkg builds statically link their libraries. Linux manylinux wheel builds use libraries from the build image and bundle them into the wheel. Installed wheels therefore do not need the build-time ``vcpkg_installed`` directory. Custom builds using ``RESILIPARSE_USE_SYSTEM_LIBS=1`` must provide those libraries at runtime.
