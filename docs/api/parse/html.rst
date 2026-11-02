.. _parse-html-api:

HTML Parsing
============

Resiliparse HTML parsing and DOM traversal utilities API documentation.

The public classes and functions are re-exported from ``resiliparse.parse.html``. They can also be imported directly
from the following submodules:

* :mod:`resiliparse.parse.html.tree`: HTML parsing and document trees.
* :mod:`resiliparse.parse.html.node`: specialised node types and traversal.
* :mod:`resiliparse.parse.html.coll`: node collections and token lists.
* :mod:`resiliparse.parse.html.exception`: parser and DOM exceptions.

.. automodule:: resiliparse.parse.html
   :members:
   :exclude-members: NodeType
   :special-members: __iter__, __getitem__, __setitem__

   .. autoclass:: NodeType
      :members:
      :undoc-members:
      :member-order: bysource

      DOM node type enum.

.. automodule:: resiliparse.parse.html.tree
   :no-members:

.. automodule:: resiliparse.parse.html.node
   :no-members:

.. automodule:: resiliparse.parse.html.coll
   :no-members:

.. automodule:: resiliparse.parse.html.exception
   :no-members:
