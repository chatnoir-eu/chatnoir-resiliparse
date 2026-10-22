# Configuration file for the Sphinx documentation builder.
#
# This file only contains a selection of the most common options. For a full
# list see the documentation:
# https://www.sphinx-doc.org/en/master/usage/configuration.html

# -- Path setup --------------------------------------------------------------

# If extensions (or modules to document with autodoc) are in another directory,
# add these directories to sys.path here. If the directory is relative to the
# documentation root, use os.path.abspath to make it absolute, like shown here.
#

import datetime
import importlib
from importlib.abc import MetaPathFinder
from importlib.machinery import EXTENSION_SUFFIXES, ModuleSpec, SourceFileLoader
from pathlib import Path
import re
import sys

src_dir = Path(__file__).parent.parent.absolute()
if src_dir.joinpath('fastwarc-py', 'fastwarc', f'_fastwarc{EXTENSION_SUFFIXES[0]}').is_file():
    sys.path[:0] = [
        str(src_dir.joinpath('resiliparse-py')),
        str(src_dir.joinpath('fastwarc-py'))
    ]

# -- Project information -----------------------------------------------------

project = 'ChatNoir Resiliparse'
copyright = f'2021-{datetime.datetime.today().year}, Janek Bevendorff'
author = 'Janek Bevendorff'
release = re.search(r'^version\s*=\s*"([^"]+)"$',
                    open(src_dir.joinpath('resiliparse-py', 'pyproject.toml')).read(), re.M).group(1)
master_doc = 'index'

# -- General configuration ---------------------------------------------------

# Add any Sphinx extension module names here, as strings. They can be
# extensions coming with Sphinx (named 'sphinx.ext.*') or your custom
# ones.
extensions = [
    'sphinx.ext.autodoc',
    'sphinx_click',
    'sphinx_rtd_theme',
    'sphinx_substitution_extensions',
]

# Add any paths that contain templates here, relative to this directory.
# templates_path = ['_templates']

# List of patterns, relative to source directory, that match files and
# directories to ignore when looking for source files.
# This pattern also affects html_static_path and html_extra_path.
exclude_patterns = ['_build', 'Thumbs.db', '.DS_Store', '*.swp', 'requirements.txt']

autodoc_member_order = 'groupwise'

autodoc_typehints = 'description'
autodoc_use_type_comments = False

# Inject version into epilog
rst_prolog = f"""
.. |project_release| replace:: {release}
.. |project_release_minor| replace:: {'.'.join(release.split('.')[:2])}
"""

# -- Options for HTML output -------------------------------------------------

# The theme to use for HTML and HTML Help pages.  See the documentation for
# a list of builtin themes.
#
html_theme = 'sphinx_rtd_theme'
html_theme_options = {
    'collapse_navigation': False,
    'version_selector': True,
    'style_external_links': True,
}

# Add any paths that contain custom static files (such as style sheets) here,
# relative to this directory. They are copied after the builtin static files,
# so a file named 'default.css' will overwrite the builtin 'default.css'.
html_static_path = ['_static']
html_css_files = [
    'style.css',
]

# -- Stub patching -----------------------------------------------------------

_fastwarc_pkg_dir = src_dir.joinpath('fastwarc-py', 'fastwarc')
_STUBBED_NATIVE_MODULES = {
    'fastwarc.warc': _fastwarc_pkg_dir.joinpath('warc.pyi'),
    'fastwarc.stream_io': _fastwarc_pkg_dir.joinpath('stream_io.pyi'),
}


class _NativeStubFinder(MetaPathFinder):
    """
    Make PyO3 submodules look like native extension modules to autodoc.

    Native Sphinx stub loading only activates when ``find_spec()`` returns a
    native-extension origin, for which we need to patch in the .pyi path.
    """

    def __init__(self, module_specs):
        self.pyi_paths = module_specs

    def find_spec(self, fullname, path=None, target=None):
        pyi_file: Path = self.pyi_paths.get(fullname)
        if pyi_file is None:
            return None

        mock_native_lib_path = str(pyi_file.parent.joinpath(pyi_file.stem + EXTENSION_SUFFIXES[0]))
        return ModuleSpec(
            fullname,
            SourceFileLoader(fullname, str(pyi_file)),
            origin=mock_native_lib_path
        )


def _copy_docstring(src, dst):
    if dst is not None and not getattr(dst, '__doc__', None) and getattr(src, '__doc__', None):
        dst.__doc__ = src.__doc__


def _hydrate_member_docstrings(dst, src):
    _copy_docstring(src, dst)

    dst_dict = getattr(dst, '__dict__', None)
    src_dict = getattr(src, '__dict__', None)
    if dst_dict is None or src_dict is None:
        return

    for name, dst_member in dst_dict.items():
        src_member = src_dict.get(name)
        if src_member is None:
            continue

        _copy_docstring(src_member, dst_member)

        if isinstance(dst_member, property) and isinstance(src_member, property):
            for accessor_name in ('fget', 'fset', 'fdel'):
                dst_accessor = getattr(dst_member, accessor_name)
                src_accessor = getattr(src_member, accessor_name)
                if dst_accessor is not None and src_accessor is not None:
                    _copy_docstring(src_accessor, dst_accessor)


def setup(_):
    from sphinx.ext.autodoc._dynamic import _importer
    original_importer = _importer._import_module

    native_mods = {}

    for m in _STUBBED_NATIVE_MODULES:
        # Import the parent package once, capture the native submodule object it
        # exposes, then remove the submodule import entry so autodoc can load the
        # stub-backed replacement later.
        parent, name = m.rsplit('.', 1)
        parent_mod = importlib.import_module(parent)
        native_mods[m] = getattr(parent_mod, name)
        sys.modules.pop(m, None)
        if hasattr(parent_mod, name):
            delattr(parent_mod, name)

    sys.meta_path.insert(0, _NativeStubFinder(_STUBBED_NATIVE_MODULES))

    def import_module(modname, try_reload=False):
        # Load new module
        module = original_importer(modname, try_reload=try_reload)
        if modname not in _STUBBED_NATIVE_MODULES:
            return module

        #  Copy docstrings from original module
        _copy_docstring(native_mods[modname], module)
        for name, member in vars(module).items():
            native_member = getattr(native_mods[modname], name, None)
            if native_member is not None:
                _hydrate_member_docstrings(member, native_member)

        return module

    # Patch _importer._import_module to load stub files properly
    _importer._import_module = import_module
