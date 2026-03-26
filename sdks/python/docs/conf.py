import os
import sys

sys.path.insert(0, os.path.abspath(".."))

project = "Aether Python SDK"
copyright = "2026 Aether Core Team"
author = "Aether Core Team"

master_doc = "index"

extensions = [
    "sphinx.ext.autodoc",
    "sphinx.ext.autodoc.typehints",
    "sphinx.ext.napoleon",
    "sphinx.ext.viewcode",
    "sphinx.ext.intersphinx",
]

templates_path = ["_templates"]
exclude_patterns = [
    "tests",
    "*.test.*",
    "__pycache__",
    "_build",
    "Thumbs.db",
    ".DS_Store",
]

intersphinx_mapping = {
    "python": ("https://docs.python.org/3", None),
}

try:
    import sphinx_rtd_theme

    html_theme = "sphinx_rtd_theme"
    html_theme_path = [sphinx_rtd_theme.get_html_theme_path()]
except ImportError:
    html_theme = "alabaster"

html_static_path = ["_static"]

autodoc_default_options = {
    "members": True,
    "undoc-members": True,
    "show-inheritance": True,
}

autodoc_default_flags = ["members", "undoc-members", "show-inheritance"]

nitpicky = False
