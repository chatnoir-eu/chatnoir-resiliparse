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
import os
import re
import sys

src_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), '..'))
sys.path.extend([
    os.path.join(src_dir, 'resiliparse'),
    os.path.join(src_dir, 'fastwarc')
])

# -- Project information -----------------------------------------------------

project = 'ChatNoir Resiliparse'
copyright = '2023, Janek Bevendorff'
author = 'Janek Bevendorff'
release = re.search(r'^version\s*=\s*"([\d.]+)"$',
                    open(os.path.join(src_dir, 'resiliparse', 'pyproject.toml')).read(), re.M).group(1)
master_doc = 'index'

# -- General configuration ---------------------------------------------------

# Add any Sphinx extension module names here, as strings. They can be
# extensions coming with Sphinx (named 'sphinx.ext.*') or your custom
# ones.
extensions = [
    'sphinx.ext.autodoc',
    'sphinx_click',
    # 'sphinx_rtd_theme'
]

# Add any paths that contain templates here, relative to this directory.
# templates_path = ['_templates']

# List of patterns, relative to source directory, that match files and
# directories to ignore when looking for source files.
# This pattern also affects html_static_path and html_extra_path.
exclude_patterns = ['_build', 'Thumbs.db', '.DS_Store', '*.swp', 'requirements.txt']

autodoc_member_order = 'groupwise'
highlight_language = "none"

# -- Options for HTML output -------------------------------------------------

# The theme to use for HTML and HTML Help pages.  See the documentation for
# a list of builtin themes.
#
# html_theme = 'sphinx_rtd_theme'
# html_theme_options = {
#     'collapse_navigation': False,
#     'display_version': True,
#     'style_external_links': True,
# }

html_theme = "sphinx_nefertiti"
html_theme_options = {
    "documentation_font": "Open Sans",
    "documentation_font_size": "1.0rem",
    "monospace_font": "Ubuntu Mono",
    "monospace_font_size": "1.1rem",

    "style": "red",
    "pygments_light_style": "pastie",
    "pygments_dark_style": "dracula",

    "logo": "chatnoir.svg",
    "logo_width": 50,
    "logo_height": 36,
    "logo_alt": "Nefertiti-for-Sphinx",

    "repository_url": "https://github.com/chatnoir-eu/chatnoir-resiliparse",
    "repository_name": "chatnoir-resiliparse",

    "header_links": [
        {
            "text": "Chatnoir",
            "link": "https://www.chatnoir.eu"
        },
        {
            "text": "Resiliparse",
            "dropdown": (
                {
                    "text": "Parsing Utilities",
                    "link": "man/parse",
                    "match": "/man/parse/*",
                },
                {
                    "text": "Extraction Utilities",
                    "link": "man/extract",
                    "match": "/man/extract/*",
                },
                {
                    "text": "Process Guards",
                    "link": "man/process-guard",
                },
                {
                    "text": "Itertools",
                    "link": "man/itertools",
                }
            ),
        },
        {
            "text": "FastWARC",
            "link": "man/fastwarc",
            "match": "/man/fastwarc/*"
        }
    ],
}


# Add any paths that contain custom static files (such as style sheets) here,
# relative to this directory. They are copied after the builtin static files,
# so a file named 'default.css' will overwrite the builtin 'default.css'.
html_static_path = ['_static']
html_css_files = [
    'style.css',
]
