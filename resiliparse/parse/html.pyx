# Copyright 2021 Janek Bevendorff
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

# distutils: language = c++

import typing as t

from cpython.ref cimport PyObject

from resiliparse_inc.lexbor cimport *
from resiliparse.parse.encoding cimport bytes_to_str, map_encoding_to_html5


cdef inline DOMNode _create_dom_node(HTMLTree tree, lxb_dom_node_t* dom_node):
    if dom_node == NULL:
        return None
    if dom_node.user != NULL:
        return <DOMNode>dom_node.user
    cdef DOMNode node = DOMNode.__new__(DOMNode, tree)
    node.node = dom_node
    node.node.user = <PyObject*>node
    return node


cdef inline DOMCollection _create_dom_collection(HTMLTree tree, lxb_dom_collection_t* coll):
    cdef DOMCollection return_coll = DOMCollection.__new__(DOMCollection, tree)
    return_coll.coll = coll
    return return_coll


cdef inline bint check_node(DOMNode node):
    return node is not None and node.tree is not None and node.node != NULL


cdef lxb_dom_collection_t* get_elements_by_attr_impl(lxb_dom_node_t* node, bytes attr_name, bytes attr_value,
                                                     size_t init_size=5, bint case_insensitive=False):
    """
    Return a collection of elements matching the given attribute name and value.

    The caller must take ownership of the returned collection.

    :param node: anchor node
    :param attr_name: attribute name as UTF-8 bytes
    :param attr_value: attribute value as UTF-8 bytes
    :param init_size: initial collection size
    :param case_insensitive: match case-insensitive
    :return: pointer to created DOM collection or ``NULL`` if error occurred
    """
    cdef lxb_dom_collection_t * coll = lxb_dom_collection_make(node.owner_document, init_size)
    if coll == NULL:
        return NULL

    cdef lxb_status_t status = lxb_dom_elements_by_attr(<lxb_dom_element_t*>node, coll,
                                                        <lxb_char_t*>attr_name, len(attr_name),
                                                        <lxb_char_t*>attr_value, len(attr_value),
                                                        case_insensitive)

    if status != LXB_STATUS_OK:
        lxb_dom_collection_destroy(coll, True)
        return NULL

    return coll


cdef lxb_dom_collection_t* get_elements_by_tag_name_impl(lxb_dom_node_t* node, bytes tag_name):
    """
    Return a collection of elements matching the given tag name.

    The caller must take ownership of the returned collection.
    
    :param node: anchor node
    :param tag_name: tag name as UTF-8 bytes
    """
    cdef lxb_dom_collection_t* coll = lxb_dom_collection_make(node.owner_document, 20)
    if coll == NULL:
        raise RuntimeError('Failed to create DOM collection')

    cdef lxb_status_t status = lxb_dom_elements_by_tag_name(<lxb_dom_element_t*>node,
                                                            coll, <lxb_char_t*>tag_name, len(tag_name))
    return coll


cdef lxb_status_t css_select_callback(lxb_dom_node_t* node, lxb_css_selector_specificity_t* spec, void* ctx) nogil:
    cdef lxb_dom_collection_t* coll = <lxb_dom_collection_t*>ctx
    if node != NULL:
        lxb_dom_collection_append(coll, node)
    return LXB_STATUS_OK


cdef lxb_dom_collection_t* query_selector_impl(lxb_dom_node_t* node, HTMLTree tree, bytes selector,
                                               size_t init_size=32):
    """
    Return a collection of elements matching the given CSS selector.

    The caller must take ownership of the returned collection.

    :param node: anchor node
    :param tree: owning HTML tree
    :param selector: CSS selector as UTF-8 bytes
    :param init_size: initial collection size
    :return: pointer to created DOM collection or ``NULL`` if error occurred
    """
    tree.init_css_parser()

    cdef lxb_css_selector_list_t* sel_list = lxb_css_selectors_parse(tree.css_parser,
                                                                     <lxb_char_t*>selector, len(selector))
    cdef lxb_dom_collection_t* coll = lxb_dom_collection_make(node.owner_document, init_size)
    if lxb_selectors_find(tree.selectors, node, sel_list,
                          <lxb_selectors_cb_f>css_select_callback, coll) != LXB_STATUS_OK:
        return NULL

    return coll


cdef lxb_status_t css_match_callback(lxb_dom_node_t* node, lxb_css_selector_specificity_t* spec, void* ctx) nogil:
    cdef bint* matches = <bint*>ctx
    matches[0] |= node != NULL
    return LXB_STATUS_OK


cdef bint matches_any_impl(lxb_dom_node_t* node, HTMLTree tree, bytes selector):
    """
    Check whether any element in the DOM subtree matches the given CSS selector.

    :param node: anchor node
    :param tree: owning HTML tree
    :param selector: CSS selector as bytes
    :return: boolean value indicating whether a matching element exists
    """
    tree.init_css_parser()

    cdef lxb_css_selector_list_t* sel_list = lxb_css_selectors_parse(tree.css_parser,
                                                                     <lxb_char_t*>selector, len(selector))
    cdef bint matches = False
    if lxb_selectors_find(tree.selectors, node, sel_list,
                          <lxb_selectors_cb_f>css_match_callback, <void*>&matches) != LXB_STATUS_OK:
        return False

    return matches


cdef class DOMNode:
    """
    __init__(self)

    DOM node.

    A DOM node is only valid as long as the owning :class:`HTMLTree` is alive
    and the DOM tree hasn't been modified. Do not access :class:`DOMNode` instances
    after any sort of DOM tree manipulation.
    """

    def __cinit__(self, HTMLTree tree):
        self.tree = tree
        self.node = NULL

    def __dealloc__(self):
        if self.node != NULL:
            self.node.user = NULL
            if self.node.parent == NULL and self.node != <lxb_dom_node_t*>self.node.owner_document:
                lxb_dom_node_destroy_deep(self.node)
                self.node = NULL

    def __iter__(self):
        """
        __iter__(self)

        Traverse the DOM tree in pre-order starting at the current node.

        :rtype: t.Iterable[DOMNode]
        """
        if not check_node(self):
            return

        yield self
        cdef lxb_dom_node_t* node = self.node
        while True:
            if node.first_child != NULL:
                node = node.first_child
            else:
                while node != self.node and node.next == NULL:
                    node = node.parent
                if node == self.node:
                    return
                node = node.next

            yield _create_dom_node(self.tree, node)

    @property
    def type(self):
        """
        DOM node type.

        :type: NodeType
        """
        if not check_node(self):
            return None
        return <NodeType>self.node.type

    @property
    def tag(self):
        """
        DOM node tag name if node is an Element node.

        :type: str or None
        """
        if not check_node(self) or self.node.type != LXB_DOM_NODE_TYPE_ELEMENT:
            return None
        cdef size_t name_len = 0
        cdef const lxb_char_t* name = lxb_dom_element_qualified_name(<lxb_dom_element_t*>self.node, &name_len)
        if name == NULL:
            return None
        return bytes_to_str(name[:name_len])

    @property
    def first_child(self):
        """
        First child element of this DOM node.

        :type: DOMNode or None
        """
        if not check_node(self):
            return None
        return _create_dom_node(self.tree, self.node.first_child)

    @property
    def last_child(self):
        """
        Last child element of this DOM node.

        :type: DOMNode or None
        """
        if not check_node(self):
            return None
        return _create_dom_node(self.tree, self.node.last_child)

    @property
    def child_nodes(self):
        """
        Iterable of child nodes.

        :type: t.Iterable[DOMNode]
        """
        if not check_node(self):
            return

        cdef lxb_dom_node_t* child = self.node.first_child
        while child != NULL:
            yield _create_dom_node(self.tree, child)
            child = child.next

    @property
    def parent(self):
        """
        Parent of this node.

        :type: DOMNode or None
        """
        if not check_node(self):
            return None
        return _create_dom_node(self.tree, self.node.parent)

    @property
    def next(self):
        """
        Next sibling node.

        :type: DOMNode or None
        """
        if not check_node(self):
            return None
        return _create_dom_node(self.tree, self.node.next)

    @property
    def prev(self):
        """
        Previous sibling node.

        :type: DOMNode or None
        """
        if not check_node(self):
            return None
        return _create_dom_node(self.tree, self.node.prev)

    @property
    def text(self):
        """
        Text contents of this DOM node and its children.

        The DOM node's inner text can be modified by assigning to this property.

        :type: str
        """
        if not check_node(self):
            return None
        cdef size_t text_len = 0
        cdef lxb_char_t* text = lxb_dom_node_text_content(self.node, &text_len)
        cdef str py_text = bytes_to_str(text[:text_len])
        lxb_dom_document_destroy_text(self.node.owner_document, text)
        return py_text

    @text.setter
    def text(self, str text):
        if not check_node(self):
            raise RuntimeError('Trying to set text contents of uninitialized DOM node')

        cdef bytes text_bytes = text.encode()
        lxb_dom_node_text_content_set(self.node, <lxb_char_t*>text_bytes, len(text_bytes))

    @property
    def html(self):
        """
        HTML contents of this DOM node and its children.

        The DOM node's inner HTML can be modified by assigning to this property.

        :type: str
        """
        if not check_node(self):
            return None
        cdef lexbor_str_t* html_str = lexbor_str_create()
        lxb_html_serialize_tree_str(self.node, html_str)
        cdef str py_text = bytes_to_str(html_str.data[:html_str.length])
        lexbor_str_destroy(html_str, self.node.owner_document.text, True)
        return py_text

    @html.setter
    def html(self, str html):
        if not check_node(self):
            raise RuntimeError('Trying to set HTML contents of uninitialized DOM node')

        cdef bytes html_bytes = html.encode()
        cdef lxb_html_element_t* element = lxb_html_element_inner_html_set(
            <lxb_html_element_t*>self.node, <lxb_char_t*>html_bytes, len(html_bytes))

    @property
    def attrs(self):
        """
        List of attribute names if node is an Element node.

        :type: t.List[str] or None
        """
        if not check_node(self) or self.node.type != LXB_DOM_NODE_TYPE_ELEMENT:
            return None

        cdef lxb_dom_attr_t* attr = lxb_dom_element_first_attribute(<lxb_dom_element_t*>self.node)
        cdef const lxb_char_t* local_name
        cdef size_t local_name_len = 0

        attrs = []
        while attr != NULL:
            local_name = lxb_dom_attr_local_name(attr, &local_name_len)
            attrs.append(bytes_to_str(local_name[:local_name_len]))
            attr = attr.next

        return attrs

    cpdef bint hasattr(self, str attr_name):
        """
        hasattr(self, attr_name)
        
        Check if node has an attribute with the given name.

        :param attr_name: attribute name
        :type attr_name: str
        :rtype: bool
        :raises ValueError: if node ist not an Element node
        """
        if not check_node(self) or self.node.type != LXB_DOM_NODE_TYPE_ELEMENT:
            raise ValueError('Node ist not an Element node.')

        cdef bytes attr_name_bytes = attr_name.encode()
        return <bint>lxb_dom_element_has_attribute(<lxb_dom_element_t*>self.node,
                                                   <lxb_char_t*>attr_name_bytes, len(attr_name_bytes))

    cpdef str getattr(self, str attr_name, str default_value=None):
        """
        getattr(self, attr_name, default_value=None)

        Get the value of the attribute ``attr_name`` or ``default_value`` if the element has no such attribute.

        :param attr_name: attribute name
        :type attr_name: str
        :param default_value: default value to return if attribute is unset
        :type default_value: str
        :return: attribute value
        :rtype: str
        :raises ValueError: if node ist not an Element node
        """
        if not check_node(self):
            return None

        cdef str value = self._getattr_impl(attr_name)
        if value is None:
            return default_value
        return value

    def __getitem__(self, str attr_name):
        """
        __getitem__(self, attr_name)

        Get the value of the an attribute.

        :param attr_name: attribute name
        :rtype: str
        :raises KeyError: if no such attribute exists
        :raises ValueError: if node ist not an Element node
        """
        if not check_node(self):
            return None

        cdef str value = self._getattr_impl(attr_name)
        if value is None:
            raise KeyError(f'No such attribute: {attr_name}')
        return value

    cdef str _getattr_impl(self, str attr_name):
        """
        Get an attribute value as string.
        
        :param attr_name: 
        :return: attribute value or None
        """
        if not check_node(self) or self.node.type != LXB_DOM_NODE_TYPE_ELEMENT:
            raise ValueError('Node ist not an Element node.')

        cdef bytes attr_name_bytes = attr_name.encode()
        cdef size_t value_len = 0
        cdef const lxb_char_t* value = lxb_dom_element_get_attribute(<lxb_dom_element_t*>self.node,
                                                                     <lxb_char_t*>attr_name_bytes, len(attr_name_bytes),
                                                                     &value_len)
        if value == NULL:
            return None
        return bytes_to_str(value[:value_len])

    cpdef DOMNode query_selector(self, str selector):
        """
        query_selector(self, selector)
         
        Find and return the first element matching the given CSS selector.

        :param selector: CSS selector
        :type selector: str
        :return: matching element or ``None``
        :rtype: DOMNode or None
        """
        cdef DOMCollection coll = self.query_selector_all(selector)
        if len(coll) == 0:
            return None
        return coll[0]

    cpdef DOMCollection query_selector_all(self, str selector):
        """
        query_selector_all(self, selector)
        
        Find all elements matching the given CSS selector and return a :class:`DOMCollection` with the results.

        :param selector: CSS selector
        :type selector: str
        :return: collection of matching elements
        :rtype: DOMCollection
        """
        cdef lxb_dom_collection_t* coll = query_selector_impl(self.node, self.tree, selector.encode())
        if coll == NULL:
            raise RuntimeError('Failed to match elements by CSS selector')

        return _create_dom_collection(self.tree, coll)

    cpdef bint matches_any(self, str selector):
        """
        matches_any(self, selector)
        
        Check whether any element in the DOM tree matches the given CSS selector.

        :param selector: CSS selector
        :type selector: str
        :return: boolean value indicating whether a matching element exists
        :rtype: bool
        """
        self.tree.init_css_parser()
        return matches_any_impl(self.node, self.tree, selector.encode())

    cpdef DOMCollection get_elements_by_attr(self, str attr_name, str attr_value, bint case_insensitive=False):
        """
        get_elements_by_attr(self, attr_name, attr_value, case_insensitive=False)
        
        Find all elements matching the given arbitrary attribute name and value and return a
        :class:`DOMCollection` with the results.
        
        :param attr_name: attribute name
        :type attr_name: str
        :param attr_value: attribute value
        :type attr_name: str
        :param case_insensitive: match attribute value case-insensitively
        :type case_insensitive: bool
        :return: collection of matching elements
        :rtype: DOMCollection or None
        """
        if not check_node(self):
            return None

        cdef lxb_dom_collection_t* coll = get_elements_by_attr_impl(self.node, attr_name.encode(), attr_value.encode(),
                                                                     10, case_insensitive)
        if coll == NULL:
            raise RuntimeError('Failed to match elements by attribute')

        return _create_dom_collection(self.tree, coll)

    cpdef DOMNode get_element_by_id(self, str element_id, bint case_insensitive=False):
        """
        get_element_by_id(self, element_id, case_insensitive=False)
        
        Find and return the element whose ID attribute matches ``element_id``.
        
        :param element_id: element ID
        :type element_id: str
        :param case_insensitive: match ID case-insensitively
        :type case_insensitive: bool
        :return: matching element or ``None`` if no such element exists
        :rtype: DOMNode or None
        """
        if not check_node(self):
            return None

        cdef lxb_dom_collection_t* coll = get_elements_by_attr_impl(self.node, b'id', element_id.encode(),
                                                                     1, case_insensitive)
        if coll == NULL:
            raise RuntimeError('Failed to match element by ID')

        cdef DOMNode return_node = None
        if lxb_dom_collection_length(coll) > 0:
            return_node = _create_dom_node(self.tree, lxb_dom_collection_node(coll, 0))
        lxb_dom_collection_destroy(coll, True)
        return return_node

    cpdef DOMCollection get_elements_by_class_name(self, str class_name, bint case_insensitive=False):
        """
        get_elements_by_class_name(self, element_class, case_insensitive=False)
        
        Find all elements matching the given class name and return a :class:`DOMCollection` with the results.
        
        :param class_name: element class
        :type class_name: str
        :param case_insensitive: match class name case-insensitively
        :type case_insensitive: bool
        :return: collection of matching elements
        :rtype: DOMCollection or None
        """
        if not check_node(self):
            return None

        cdef lxb_dom_collection_t * coll = get_elements_by_attr_impl(self.node, b'class', class_name.encode(),
                                                                      20, case_insensitive)
        if coll == NULL:
            raise RuntimeError('Failed to match elements by class name')

        return _create_dom_collection(self.tree, coll)

    cpdef DOMCollection get_elements_by_tag_name(self, str tag_name):
        """
        get_elements_by_tag_name(self, tag_name)
        
        Find all elements with the given tag name and return a :class:`DOMCollection` with the results.
        
        :param tag_name: tag name for matching elements
        :type tag_name: str
        :return: collection of matching elements
        :rtype: DOMCollection
        """
        if not check_node(self):
            return None

        return _create_dom_collection(self.tree, get_elements_by_tag_name_impl(self.node, tag_name.encode()))

    cpdef DOMNode append_child(self, DOMNode node):
        """
        append_child(self, node)
        
        Append a new child node to this DOM node.
        
        :param node: DOM node to append as new child node
        :type node: DOMNode
        :return: the appended child node
        :rtype: DOMNode
        :raises ValueError: if trying to append node to itself
        """
        if not check_node(self) or not check_node(node):
            raise RuntimeError('Append operation on uninitialized node')

        if node.node == self.node:
            raise ValueError('Trying to append child to itself')

        if node.node.parent != NULL:
            lxb_dom_node_remove(node.node)
        lxb_dom_node_insert_child(self.node, node.node)
        return node

    cpdef DOMNode insert_before(self, DOMNode node, DOMNode reference):
        """
        insert_before(self, node, reference)
        
        Insert ``node`` before ``reference`` as a new child node. The reference node must be
        a child of this node or ``None``. If ``reference`` is ``None``, the new node
        will be appended after the last child node.
        
        :param node: DOM node to insert as new child node
        :type node: DOMNode
        :param reference: child node before which to insert the new node or ``None``
        :type reference: DOMNode
        :return: the inserted child node
        :rtype: DOMNode
        :raises ValueError: if trying to add node as its own child or if ``reference`` is not a child
        """
        if not check_node(self) or not check_node(node) or not check_node(reference):
            raise RuntimeError('Insert operation on uninitialized node')

        if node.node == self.node:
            raise ValueError('Trying to insert node as its own child')

        if reference.node.parent != self.node:
            raise ValueError('Reference node must be a child node')

        if node.node.parent != NULL:
            lxb_dom_node_remove(node.node)
        lxb_dom_node_insert_before(reference.node, node.node)
        return node

    cpdef DOMNode replace_child(self, DOMNode new_child, DOMNode old_child):
        """
        replace_child(self, new_child, old_child)
        
        Replace the child node ``old_child`` with ``new_child``.
        
        :param new_child: new child node to insert
        :type new_child: DOMNode
        :param old_child: old child node to replace
        :type old_child: DOMNode
        :return: the old child node
        :rtype: DOMNode
        :raises ValueError: if ``old_child`` is not a child of this node
        """
        if not check_node(self) or not check_node(new_child) or not check_node(old_child):
            raise RuntimeError('Replace operation on uninitialized node')

        if old_child.node.parent != self.node:
            raise ValueError('Node is not a child of the current node')

        if new_child.node == old_child.node:
            return old_child

        if new_child.node.parent != NULL:
            lxb_dom_node_remove(new_child.node)
        lxb_dom_node_insert_after(old_child.node, new_child.node)
        lxb_dom_node_remove(old_child.node)

        return old_child

    cpdef DOMNode remove_child(self, DOMNode node):
        """
        remove_child(self, node)
        
        Remove the child node ``node`` from the DOM tree and return it.
        
        :param node: DOM node to remove
        :type node: DOMNode
        :return: the removed child node
        :rtype: DOMNode
        :raises ValueError: if ``node`` is not a child of this node
        """
        if not check_node(self) or not check_node(node):
            raise RuntimeError('Remove operation on uninitialized node')

        if node.node.parent != self.node:
            raise ValueError('Node is not a child of the current node')

        lxb_dom_node_remove(node.node)
        return node

    # noinspection PyAttributeOutsideInit
    cpdef void decompose(self):
        """
        decompose(self)
        
        Delete the current node and all its children.
        """
        if not check_node(self):
            raise RuntimeError('Decompose operation on uninitialized node')

        lxb_dom_node_destroy_deep(self.node)
        self.node = NULL
        self.tree = None

    def __repr__(self):
        if not check_node(self):
            return '<INVALID ELEMENT>'

        if self.node.type == LXB_DOM_NODE_TYPE_ELEMENT:
            attrs = ' '.join(f'{a}="{self[a]}"' for a in self.attrs)
            if attrs:
                attrs = ' ' + attrs
            return f'<{self.tag}{attrs}>'
        elif self.node.type == LXB_DOM_NODE_TYPE_TEXT:
            return self.text
        elif self.node.type == LXB_DOM_NODE_TYPE_DOCUMENT:
            return '[HTML Document]'
        elif self.node.type == LXB_DOM_NODE_TYPE_DOCUMENT_TYPE:
            return '<!DOCTYPE html>'

        return f'<{self.__class__.__name__} Element>'

    def __str__(self):
        return self.html

    def __eq__(self, other):
        if not check_node(self) or not isinstance(other, DOMNode):
            return False
        cdef DOMNode other_dom = <DOMNode>other
        return other_dom.node == self.node

    def __hash__(self):
        if not check_node(self):
            return 0
        return <size_t>self.node


cdef inline void _join_collections(lxb_dom_collection_t* target, lxb_dom_collection_t* source):
    """
    Append elements from ``source` to ``target``.
    
    :param target: target collection to append to
    :param source: source collection to append from
    """

    cdef size_t i = 0
    for i in range(lxb_dom_collection_length(source)):
        lxb_dom_collection_append(target, lxb_dom_collection_element(source, i))


cdef class DOMCollection:
    """
    __init__(self)

    Collection of DOM nodes that are the result set of an element matching operation.

    A node collection is only valid for as long as the owning :class:`HTMLTree` is alive
    and the DOM tree hasn't been modified. Do not access :class:`DOMCollection` instances
    after any sort of DOM tree manipulation.
    """

    def __cinit__(self, HTMLTree tree):
        self.tree = tree
        self.coll = NULL

    def __dealloc__(self):
        if self.coll != NULL:
            lxb_dom_collection_destroy(self.coll, True)
            self.coll = NULL

    cdef inline size_t _wrap_idx(self, ssize_t idx):
        if idx >= 0:
            return idx
        return idx % <ssize_t>lxb_dom_collection_length(self.coll)

    cdef _forward_element_match(self, bytes func, attrs, bint single):
        """
        Forward DOM element match operation to all items in the collection and aggregate the results.
        
        :param func: internal matching function as bytes (b'by_attr', b'by_tag', b'selector', or b'matches')
        :param attrs: tuple of attributes to pass to the matching function
        :param single: return first match only or entire collection
        :return: aggregated collection or single element
        """
        if self.tree is None or self.coll == NULL:
            raise RuntimeError('Trying to select items from uninitialized collection')

        cdef lxb_dom_collection_t* joined_coll = NULL
        if func != b'matches':
            joined_coll = lxb_dom_collection_make(self.coll.document, lxb_dom_collection_length(self.coll) * 2)

        cdef lxb_dom_collection_t* matches
        cdef lxb_dom_node_t* node = NULL
        cdef size_t i = 0
        for i in range(lxb_dom_collection_length(self.coll)):
            node = lxb_dom_collection_node(self.coll, i)
            if func == b'by_attr':
                matches = get_elements_by_attr_impl(node, attrs[0], attrs[1], attrs[2], attrs[3])
            elif func == b'by_tag':
                matches = get_elements_by_tag_name_impl(node, attrs[0])
            elif func == b'selector':
                matches = query_selector_impl(node, self.tree, attrs[0])
            elif func == b'matches':
                if matches_any_impl(node, self.tree, attrs[0]):
                    return True
                continue

            if single:
                if lxb_dom_collection_length(matches) > 0:
                    new_node = _create_dom_node(self.tree, <lxb_dom_node_t*>lxb_dom_collection_element(matches, 0))
                    lxb_dom_collection_destroy(matches, True)
                    return new_node
                lxb_dom_collection_destroy(matches, True)
            elif matches != NULL:
                _join_collections(joined_coll, matches)
                lxb_dom_collection_destroy(matches, True)

        # Didn't match anything if we are single-matching and made it to this point
        if func == b'matches':
            return False
        if single:
            return None

        return _create_dom_collection(self.tree, joined_coll)

    cpdef DOMNode get_element_by_id(self, str element_id, bint case_insensitive=False):
        """
        get_element_by_id(self, element_id, case_insensitive=False)

        Within all elements in this collection, find and return the element whose ID
        attribute matches ``element_id``.

        :param element_id: element ID
        :type element_id: str
        :param case_insensitive: match ID case-insensitively
        :type case_insensitive: bool
        :return: matching element or ``None`` if no such element exists
        :rtype: DOMNode or None
        """
        return self._forward_element_match(b'by_attr', (b'id', element_id.encode(), 1, case_insensitive), True)

    cpdef DOMCollection get_elements_by_attr(self, str attr_name, str attr_value, bint case_insensitive=False):
        """
        get_elements_by_attr(self, attr_name, attr_value, case_insensitive=False)

        Within all elements in this collection, find the elements matching the given arbitrary attribute
        name and value and return a :class:`DOMCollection` with the aggregated results.

        :param attr_name: attribute name
        :type attr_name: str
        :param attr_value: attribute value
        :type attr_name: str
        :param case_insensitive: match attribute value case-insensitively
        :type case_insensitive: bool
        :return: collection of matching elements
        :rtype: DOMCollection or None
        """
        return self._forward_element_match(b'by_attr',
                                           (attr_name.encode(), attr_value.encode(),  10, case_insensitive), False)

    cpdef DOMCollection get_elements_by_class_name(self, str class_name, bint case_insensitive=False):
        """
        get_elements_by_class_name(self, element_class, case_insensitive=False)

        Within all elements in this collection, find the elements matching the given class name
        and return a :class:`DOMCollection` with the aggregated results.

        :param class_name: element class
        :type class_name: str
        :param case_insensitive: match class name case-insensitively
        :type case_insensitive: bool
        :return: collection of matching elements
        :rtype: DOMCollection or None
        """
        return self._forward_element_match(b'by_attr', (b'class', class_name.encode(), 10, case_insensitive), False)

    cpdef DOMCollection get_elements_by_tag_name(self, str tag_name):
        """
        get_elements_by_tag_name(self, tag_name)

        Within all elements in this collection, find the elements with the given tag name and return
        a :class:`DOMCollection` with the aggregated results.

        :param tag_name: tag name for matching elements
        :type tag_name: str
        :return: collection of matching elements
        :rtype: DOMCollection
        """
        return self._forward_element_match(b'by_tag', (tag_name.encode(),), False)

    cpdef DOMNode query_selector(self, str selector):
        """
        query_selector(self, selector)

        Within all elements in this collection, find and return the first element matching
        the given CSS selector.

        :param selector: CSS selector
        :type selector: str
        :return: matching element or ``None``
        :rtype: DOMNode or None
        """
        return self._forward_element_match(b'selector', (selector.encode(),), True)

    cpdef DOMCollection query_selector_all(self, str selector):
        """
        query_selector_all(self, selector)

        Within all elements in this collection, find the elements matching the given CSS selector
        and return a :class:`DOMCollection` with the aggregated results.

        :param selector: CSS selector
        :type selector: str
        :return: collection of matching elements
        :rtype: DOMCollection
        """
        return self._forward_element_match(b'selector', (selector.encode(),), False)

    cpdef bint matches_any(self, str selector):
        """
        matches_any(self, selector)

        Within all elements in this collection, check whether any element in the DOM tree
        matches the given CSS selector.

        :param selector: CSS selector
        :type selector: str
        :return: boolean value indicating whether a matching element exists
        :rtype: bool
        """
        return self._forward_element_match(b'matches', (selector.encode(),), True)

    def __iter__(self):
        """
        __iter__(self)

        Iterate the DOM node collection.

        :rtype: t.Iterable[DOMNode]
        """
        if self.tree is None or self.coll == NULL:
            raise RuntimeError('Trying to get item of uninitialized collection')

        cdef size_t i = 0
        for i in range(lxb_dom_collection_length(self.coll)):
            yield _create_dom_node(self.tree, lxb_dom_collection_node(self.coll, i))

    def __len__(self):
        """
        __len__(self)

        Get the number of items in this collection.

        :rtype: int
        """
        if self.tree is None or self.coll == NULL:
            return 0
        return lxb_dom_collection_length(self.coll)

    def __getitem__(self, key):
        """
        __getitem__(self, key)

        Return the :class:`DOMNode` at the given index in this collection or another :class:`DOMCollection`
        if ``key`` is a slice object. Negative indexing is supported.

        :param key: index or slice
        :rtype: DOMNode or DOMCollection
        :raises IndexError: if ``key`` is out of range
        :raises TypeError: if ``key`` is not an ``int`` or ``slice``
        """
        if self.tree is None or self.coll == NULL:
            raise IndexError('Trying to get item of uninitialized collection')

        cdef size_t coll_len = lxb_dom_collection_length(self.coll)

        cdef DOMCollection slice_coll
        cdef lxb_dom_collection_t* dom_coll
        if isinstance(key, slice):
            start = key.start
            stop = key.stop
            step = key.step if key.step is not None else 1

            if start is None:
                start = coll_len - 1 if step < 0 else 0
            else:
                start = self._wrap_idx(min(start, coll_len - 1))

            if stop is None:
                stop = -1 if step < 0 else coll_len
            else:
                stop = self._wrap_idx(min(stop, coll_len))

            dom_coll = lxb_dom_collection_make(self.coll.document,
                                               min(coll_len, abs((stop - start) // step) + 1))
            for i in range(start, stop, step):
                lxb_dom_collection_append(dom_coll, lxb_dom_collection_element(self.coll, i))

            slice_coll = DOMCollection.__new__(DOMCollection, self.tree)
            slice_coll.coll = dom_coll
            return slice_coll

        if type(key) is not int:
            raise TypeError(f'Invalid key type: {type(key)}')

        if key >= coll_len:
            raise IndexError('Index out of range')

        return _create_dom_node(self.tree, lxb_dom_collection_node(self.coll, self._wrap_idx(key)))

    def __repr__(self):
        return f'{{{", ".join(repr(n) for n in self)}}}'

    def __str__(self):
        return self.__repr__()


cdef class HTMLTree:
    """
    __init__(self)

    HTML DOM tree parser.
    """
    def __cinit__(self):
        self.dom_document = lxb_html_document_create()
        if self.dom_document == NULL:
            raise RuntimeError('Failed to allocate HTML document')
        self.css_parser = NULL
        self.css_selectors = NULL
        self.selectors = NULL

    def __dealloc__(self):
        if self.dom_document != NULL:
            lxb_html_document_destroy(self.dom_document)
            self.dom_document = NULL

        if self.selectors != NULL:
            lxb_selectors_destroy(self.selectors, True)
        if self.css_parser != NULL:
            lxb_css_parser_destroy(self.css_parser, True)
            self.css_parser = NULL
        if self.css_selectors != NULL:
            lxb_css_selectors_destroy(self.css_selectors, True, True)
            self.css_selectors = NULL

    # noinspection PyAttributeOutsideInit
    cdef void init_css_parser(self):
        """
        Initialize CSS selector if not already initialized.
        """
        if self.css_parser == NULL:
            self.css_parser = lxb_css_parser_create()
            lxb_css_parser_init(self.css_parser, NULL, NULL)

            self.css_selectors = lxb_css_selectors_create()
            lxb_css_selectors_init(self.css_selectors, 32)
            lxb_css_parser_selectors_set(self.css_parser, self.css_selectors)

            self.selectors = lxb_selectors_create()
            lxb_selectors_init(self.selectors)

    cpdef void parse(self, str document):
        """
        parse(self, document)
        
        Parse HTML from a Unicode string into a DOM tree.
        
        :param document: input HTML document
        :raises ValueError: if HTML parsing fails for unknown reasons
        """
        self.parse_from_bytes(document.encode('utf-8'))

    cpdef void parse_from_bytes(self, bytes document, str encoding='utf-8', str errors='ignore'):
        """
        parse_from_bytes(self, document, encoding='utf-8', errors='ignore')
        
        Decode a raw HTML byte string and parse it into a DOM tree.
        
        The decoding routine uses :func:`~.parse.encoding.bytes_to_str` to take care of decoding errors,
        so it is sufficient if ``encoding`` is just a best guess of what the actual input encoding is.
        
        :param document: input byte string
        :param encoding: encoding for decoding byte string
        :param errors: decoding error policy (same as ``str.decode()``)
        :raises ValueError: if HTML parsing fails for unknown reasons
        """
        encoding = map_encoding_to_html5(encoding)
        if encoding != 'utf-8':
            document = bytes_to_str(document, encoding, errors).encode('utf-8')
        cdef lxb_status_t status = lxb_html_document_parse(self.dom_document,
                                                           <const lxb_char_t*>document, len(document))
        if status != LXB_STATUS_OK:
            raise ValueError('Failed to parse HTML document')

    @property
    def document(self):
        """
        Document root node.

        :type: DOMNode or None
        """
        if self.dom_document == NULL:
            return None

        return _create_dom_node(self, <lxb_dom_node_t*>&self.dom_document.dom_document)

    @property
    def head(self):
        """
        HTML head element or ``None`` if document has no head.

        :type: DOMNode or None
        """
        if self.dom_document == NULL:
            return None

        return _create_dom_node(self, <lxb_dom_node_t*>lxb_html_document_head_element(self.dom_document))

    @property
    def body(self):
        """
        HTML body element or ``None`` if document has no body.

        :type: DOMNode or None
        """
        if self.dom_document == NULL:
            return None

        return _create_dom_node(self, <lxb_dom_node_t*>lxb_html_document_body_element(self.dom_document))

    @property
    def title(self):
        """
        The HTML document title.

        :type: str or None
        """
        cdef size_t title_len = 0
        cdef const lxb_char_t* title = lxb_html_document_title(self.dom_document, &title_len)
        if title == NULL:
            return None
        return bytes_to_str(title[:title_len])

    cpdef DOMNode create_element(self, str tag_name):
        """
        create_element(self, tag_name)
        
        Create a new DOM Element node.
        
        :param tag_name: element tag name
        :type tag_name: str
        :return: new Element node
        :rtype: DOMNode
        """
        if self.dom_document == NULL:
            raise RuntimeError('Trying to create element in uninitialized document.')

        cdef bytes tag_name_bytes = tag_name.encode()
        cdef lxb_dom_element_t* element = lxb_dom_document_create_element(
            <lxb_dom_document_t*>self.dom_document, <lxb_char_t*>tag_name_bytes, len(tag_name_bytes), NULL)
        return _create_dom_node(self, <lxb_dom_node_t*>element)

    cpdef DOMNode create_text_node(self, str text):
        """
        create_text_node(self, text)

        Create a new DOM Element node.

        :param text: string contents of the new text element
        :type text: str
        :return: new text node
        :rtype: DOMNode
        """
        if self.dom_document == NULL:
            raise RuntimeError('Trying to create text node in uninitialized document.')

        cdef bytes text_bytes = text.encode()
        cdef lxb_dom_text_t* node = lxb_dom_document_create_text_node(
            <lxb_dom_document_t*>self.dom_document, <lxb_char_t*>text_bytes, len(text_bytes))
        return _create_dom_node(self, <lxb_dom_node_t*>node)
