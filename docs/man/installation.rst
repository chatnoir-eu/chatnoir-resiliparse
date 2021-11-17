.. _resiliparse-installation:

Installation Instructions
=========================

.. note::

  These installation instructions are for the main Resiliparse module. For installing FastWARC, see :ref:`fastwarc-installation`.

Pre-built Resiliparse binaries can be installed from `PyPi <https://pypi.org/project/Resiliparse/>`_:

.. code-block:: bash

  pip install resiliparse

Building Resiliparse From Source
--------------------------------

You can compile Resiliparse either from the `PyPi <https://pypi.org/project/Resiliparse/>`_ source package or directly from the `Github repository <https://github.com/chatnoir-eu/chatnoir-resiliparse>`_, though in any case, you need to install all required build-time dependencies first. On Ubuntu, this is done as follows:

.. code-block:: bash

  # Add Lexbor repository
  curl -L https://lexbor.com/keys/lexbor_signing.key | sudo apt-key add -
  echo "deb https://packages.lexbor.com/ubuntu/ $(lsb_release -sc) liblexbor" | \
      sudo tee /etc/apt/sources.list.d/lexbor.list

  # Install build dependencies
  sudo apt update
  sudo apt install build-essential python3-dev libuchardet-dev liblexbor-dev libre2-dev

To build and install Resiliparse from PyPi, run

.. code-block:: bash

  pip install --no-binary resiliparse resiliparse

That's it. If you prefer to build directly from the GitHub repository instead, run:

.. code-block:: bash

  # Clone repository
  git clone https://github.com/chatnoir-eu/chatnoir-resiliparse.git
  cd chatnoir-resiliparse

  # Optional: Create a fresh venv
  python3 -m venv venv && source venv/bin/activate

  pip install -e resiliparse

To build the wheels without installing them, run:

.. code-block:: bash

  pip wheel -e resiliparse

  # Or:
  pip install build && python -m build --wheel resiliparse
