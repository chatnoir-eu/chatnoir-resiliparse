.. _resiliparse-installation:

Installation Instructions
=========================

.. note::

  These installation instructions are for the main Resiliparse module. For installing FastWARC, see :ref:`fastwarc-installation`.

Pre-built Resiliparse binaries can be installed from `PyPi <https://pypi.org/project/Resiliparse/>`_:

.. code-block:: bash

  pip install resiliparse

To build Resiliparse from sources, you can either compile it from the PyPi source package or directly from this repository. To build Resiliparse from PyPi, run:

.. code-block:: bash

  pip install --no-binary resiliparse resiliparse

If you prefer to build directly from the `GitHub repository <https://github.com/chatnoir-eu/chatnoir-resiliparse>`_ instead, run:

.. code-block:: bash

  # Clone repository
  git clone https://github.com/chatnoir-eu/chatnoir-resiliparse.git
  cd chatnoir-resiliparse

  # Create venv (recommended, but not required)
  python3 -m venv venv && source venv/bin/activate

  # Install build dependencies
  sudo apt install libuchardet-dev
  pip install cython setuptools

  # Build and install
  BUILD_PACKAGES=resiliparse python setup.py install
