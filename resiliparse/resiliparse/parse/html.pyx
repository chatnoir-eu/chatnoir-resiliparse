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

cimport cython
from cython.operator cimport preincrement as preinc, predecrement as predec
from cpython.ref cimport PyObject
from libcpp.set cimport set as unordered_set

from resiliparse_inc.lexbor cimport *
from resiliparse.parse.encoding cimport bytes_to_str, map_encoding_to_html5


cdef inline DOMNode _create_dom_node(HTMLTree tree, lxb_dom_node_t* dom_node):
    if not dom_node:
        return None
    if dom_node.user:
        return <DOMNode>dom_node.user
    cdef DOMNode node = DOMNode.__new__(DOMNode, tree)
    node.node = dom_node
    node.node.user = <PyObject*>node
    return node


cdef inline DOMCollection _create_dom_collection(HTMLTree tree, lxb_dom_collection_t* coll):
    cdef DOMCollection return_coll = DOMCollection.__new__(DOMCollection, tree)
    return_coll.coll = coll
    return return_coll


cdef bint init_css_parser(lxb_css_parser_t** parser) except 0:
    parser[0] = lxb_css_parser_create()
    if lxb_css_parser_init(parser[0], NULL, NULL) != LXB_STATUS_OK:
        raise RuntimeError('Failed to initialize CSS parser.')
    return True


cdef void destroy_css_parser(lxb_css_parser_t* parser) nogil:
    if parser:
        lxb_css_parser_destroy(parser, True)


cdef bint init_css_selectors(lxb_css_parser_t* parser, lxb_css_selectors_t** css_selectors,
                             lxb_selectors_t** selectors) except 0:
    css_selectors[0] = lxb_css_selectors_create()
    if lxb_css_selectors_init(css_selectors[0], 32) != LXB_STATUS_OK:
        raise RuntimeError('Failed to initialize CSS selectors.')

    lxb_css_parser_selectors_set(parser, css_selectors[0])

    selectors[0] = lxb_selectors_create()
    if lxb_selectors_init(selectors[0]) != LXB_STATUS_OK:
        raise RuntimeError('Failed to initialize elemetn selectors.')

    return True


cdef void destroy_css_selectors(lxb_css_selectors_t* css_selectors, lxb_selectors_t* selectors) nogil:
    if selectors:
        lxb_selectors_destroy(selectors, True)
    if css_selectors:
        lxb_css_selectors_destroy(css_selectors, True, True)


cdef inline void _log_serialize_cb(const lxb_char_t *data, size_t len, void *ctx) nogil:
    (<string*>ctx).append(<const char*>data, len)


cdef lxb_css_selector_list_t* parse_css_selectors(lxb_css_parser_t* css_parser, const lxb_char_t* selector,
                                                  size_t selector_len) except NULL:
    cdef lxb_css_selector_list_t* sel_list = lxb_css_selectors_parse(css_parser, selector, selector_len)
    cdef string err
    if css_parser.status != LXB_STATUS_OK:
        lxb_css_log_serialize(css_parser.log, <lexbor_serialize_cb_f>_log_serialize_cb, &err, <const lxb_char_t*>b'', 0)
        raise ValueError(f'CSS parser error: {err.decode().strip()}')
    return sel_list


cdef inline lxb_dom_node_t* next_node(const lxb_dom_node_t* root_node, lxb_dom_node_t* node,
                                      size_t* depth=NULL, bint* end_tag=NULL) nogil:
    """
    DOM tree pre-order traversal primitive.
    
    This is a more flexible, step-wise implementation of ``lxb_dom_node_simple_walk()``, which
    allows a client to react on closing tags.

    :param root_node: root node at which traversal started
    :param node: current node
    :param depth: tracks DOM depth if not ``NULL`` (needs to be initialized and passed back in at each step)
    :param end_tag: if not ``NULL``, tracks whether step was an end tag (needs to be initialized with ``False``
                    and passed back in at each step)
    :returns: next node or ``NULL`` if done
    """
    cdef bint is_end = end_tag and end_tag[0]

    if not is_end and node.first_child:
        if depth:
            preinc(depth[0])
        return node.first_child
    else:
        while not node.next and node != root_node:
            node = node.parent
            if depth:
                predec(depth[0])
            if end_tag:
                end_tag[0] = True
                return node

        if end_tag:
            end_tag[0] = False
        if node == root_node:
            return NULL
        return node.next


cdef inline string get_node_text(lxb_dom_node_t* node) nogil:
    """Get node inner text."""
    cdef lxb_dom_character_data_t* char_data
    if node.type == LXB_DOM_NODE_TYPE_TEXT:
        char_data = <lxb_dom_character_data_t*>node
        return string(<const char*>char_data.data.data, char_data.data.length)

    cdef size_t text_len = 0
    cdef lxb_char_t* text = lxb_dom_node_text_content(node, &text_len)
    if not text or not text_len:
        return string()
    cdef string text_str = string(<const char*>text, text_len)
    lxb_dom_document_destroy_text(node.owner_document, text)
    return text_str


cdef lxb_dom_collection_t* get_elements_by_class_name_impl(lxb_dom_node_t* node, bytes class_name, size_t init_size=5):
    """
    Return a collection of elements matching the given attribute name and value.

    The caller must take ownership of the returned collection.

    :param node: anchor node
    :param class_name: class name as UTF-8 bytes
    :param init_size: initial collection size
    :return: pointer to created DOM collection or ``NULL`` if error occurred
    """
    cdef lxb_dom_collection_t * coll = lxb_dom_collection_make(node.owner_document, init_size)
    if not coll:
        return NULL

    cdef lxb_status_t status = lxb_dom_elements_by_class_name(<lxb_dom_element_t*>node, coll,
                                                              <const lxb_char_t*>class_name, len(class_name))

    if status != LXB_STATUS_OK:
        lxb_dom_collection_destroy(coll, True)
        return NULL

    return coll


cdef struct element_by_id_match_ctx:
    const lxb_char_t* id_val
    size_t id_val_len
    lxb_dom_node_t* result_node

ctypedef element_by_id_match_ctx element_by_id_match_ctx_t


cdef inline lexbor_action_t element_by_id_callback(lxb_dom_node_t* node, void* ctx) nogil:
    if node.type != LXB_DOM_NODE_TYPE_ELEMENT:
        return LEXBOR_ACTION_OK

    cdef element_by_id_match_ctx_t* match_ctx = <element_by_id_match_ctx_t*>ctx
    cdef const lxb_char_t* id_val
    cdef size_t id_val_len = 0

    id_val = lxb_dom_element_id(<lxb_dom_element_t*>node, &id_val_len)
    if not id_val or id_val_len != match_ctx.id_val_len:
        return LEXBOR_ACTION_OK

    if lexbor_str_data_ncmp(id_val, match_ctx.id_val, id_val_len):
        match_ctx.result_node = node
        return LEXBOR_ACTION_STOP

    return LEXBOR_ACTION_OK


cdef lxb_dom_node_t* get_element_by_id_impl(lxb_dom_node_t* node, bytes id_value, bint case_insensitive=False):
    """
    Return a pointer to the first element matching the given ID or NULL.

    :param node: anchor node
    :param id_value: ID value as UTF-8 bytes
    :param case_insensitive: match case-insensitive
    :return: pointer to created DOM collection or ``NULL`` if error occurred
    """

    cdef element_by_id_match_ctx ctx = [<const lxb_char_t*>id_value, len(id_value), NULL]
    lxb_dom_node_simple_walk(node, <lxb_dom_node_simple_walker_f>element_by_id_callback, &ctx)
    return ctx.result_node


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
    if not coll:
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
    if not coll:
        raise RuntimeError('Failed to create DOM collection')

    cdef lxb_status_t status = lxb_dom_elements_by_tag_name(<lxb_dom_element_t*>node,
                                                            coll, <const lxb_char_t*>tag_name, len(tag_name))
    return coll


cdef inline lxb_status_t css_select_callback(lxb_dom_node_t* node, lxb_css_selector_specificity_t* spec,
                                             void* ctx) nogil:
    lxb_dom_collection_append(<lxb_dom_collection_t*>ctx, node)
    return LXB_STATUS_OK


cdef inline lxb_status_t css_select_callback_single(lxb_dom_node_t* node, lxb_css_selector_specificity_t* spec,
                                                    void* ctx) nogil:
    (<lxb_dom_node_t**>ctx)[0] = node
    return LXB_STATUS_STOP


cdef lxb_dom_node_t* query_selector_impl(lxb_dom_node_t* node, HTMLTree tree,
                                         bytes selector) except <lxb_dom_node_t*>-1:
    """
    Return a pointer to the first element matching the given CSS selector.

    :param node: anchor node
    :param tree: owning HTML tree
    :param selector: CSS selector as UTF-8 bytes
    :return: pointer to created DOM collection or ``NULL`` if error occurred
    """
    tree.init_css_parser()

    cdef lxb_css_selector_list_t* sel_list = parse_css_selectors(tree.css_parser,
                                                                 <const lxb_char_t*>selector, len(selector))
    cdef lxb_dom_node_t* result_node = NULL
    if lxb_selectors_find(tree.selectors, node, sel_list,
                          <lxb_selectors_cb_f>css_select_callback_single, &result_node) != LXB_STATUS_OK:
        return NULL

    return result_node


cdef lxb_dom_collection_t* query_selector_all_impl(lxb_dom_node_t* node, HTMLTree tree, bytes selector,
                                                   size_t init_size=32) except <lxb_dom_collection_t*>-1:
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

    cdef lxb_css_selector_list_t* sel_list = parse_css_selectors(tree.css_parser,
                                                                 <const lxb_char_t*>selector, len(selector))
    cdef lxb_dom_collection_t* coll = lxb_dom_collection_make(node.owner_document, init_size)
    if lxb_selectors_find(tree.selectors, node, sel_list,
                          <lxb_selectors_cb_f>css_select_callback, coll) != LXB_STATUS_OK:
        return NULL

    return coll


cdef inline lxb_status_t css_match_callback(lxb_dom_node_t* node, lxb_css_selector_specificity_t* spec,
                                            void* ctx) nogil:
    (<bint*>ctx)[0] = True
    return LXB_STATUS_STOP


cdef bint matches_impl(lxb_dom_node_t* node, HTMLTree tree, bytes selector):
    """
    Check whether any element in the DOM subtree matches the given CSS selector.

    :param node: anchor node
    :param tree: owning HTML tree
    :param selector: CSS selector as bytes
    :return: boolean value indicating whether a matching element exists
    """
    tree.init_css_parser()

    cdef lxb_css_selector_list_t* sel_list = parse_css_selectors(tree.css_parser,
                                                                 <const lxb_char_t*>selector, len(selector))
    cdef bint matches = False
    if lxb_selectors_find(tree.selectors, node, sel_list,
                          <lxb_selectors_cb_f>css_match_callback, <void*>&matches) != LXB_STATUS_OK:
        return False

    return matches


cdef inline bint is_whitespace(const char c):
    return c == b' ' or c == b'\t' or c == b'\n' or c == b'\f' or c == b'\r'


cdef class DOMElementClassList:
    """Class name list of an Element DOM node."""

    def __cinit__(self, DOMNode node):
        self.node = node

    cdef list _create_list(self):
        if self.node is None or not self.node.node:
            return []

        cdef size_t class_name_len = 0
        cdef const lxb_char_t* class_name = lxb_dom_element_class(<lxb_dom_element_t*>self.node.node, &class_name_len)
        if not class_name:
            return []

        cdef list class_list = []
        cdef start = 0, end = 0
        cdef size_t i
        for i in range(class_name_len):

            if is_whitespace(class_name[start]):
                start = i + 1
                continue

            if is_whitespace(class_name[i]) or i == class_name_len - 1:
                end = i if i < class_name_len - 1 else i + 1
                if start < end:
                    class_list.append(class_name[start:end].decode())
                    start = i + 1

        return class_list

    cdef inline bytes _class_name_bytes(self):
        cdef size_t class_name_len = 0
        cdef const lxb_char_t* class_name = lxb_dom_element_class(<lxb_dom_element_t*>self.node.node, &class_name_len)
        if not class_name:
            return b''
        return class_name[:class_name_len]

    cpdef void add(self, str class_name):
        """
        add(self, class_name)
        
        Add new class name to Element node if not already present.
        
        :param class_name: new class name
        :type class_name: str
        """
        if self.node is None or not self.node.node:
            return

        cdef list l = self._create_list()
        if class_name in l:
            return

        cdef bytes new_class_name = self._class_name_bytes()
        if not is_whitespace(new_class_name[-1]):
            class_name = ' ' + class_name
        new_class_name = new_class_name + class_name.encode()
        # noinspection PyProtectedMember
        self.node._setattr_impl(b'class', new_class_name)

    cpdef void remove(self, str class_name):
        """
        remove(self, class_name)
        
        Remove a class name from this Element node.
        
        :param class_name: new class name
        :type class_name: str
        """
        if self.node is None or not self.node.node:
            return

        cdef list l = [c for c in self._create_list() if c != class_name]
        # noinspection PyProtectedMember
        self.node._setattr_impl(b'class', b' '.join([c.encode() for c in l]))

    def __contains__(self, item):
        """
        __contains__(self, item):
        """
        return item in self._create_list()

    def __getitem__(self, item):
        """
        __contains__(self, item)
        """
        return self._create_list()[item]

    def __eq__(self, other):
        """
        __eq__(self, other)
        """
        return other == self._create_list()

    def __len__(self):
        """
        __len__(self)
        """
        return len(self._create_list())

    def __iter__(self):
        """
        __iter__(self)
        """
        return iter(self._create_list())

    def __repr__(self):
        """
        __repr__(self)
        """
        return repr(self._create_list())

    def __str__(self):
        """
        __str__(self)
        """
        return str(self._create_list())


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
        self.class_list_singleton = None

    def __dealloc__(self):
        if not self.node or self.tree is None:
            return

        self.node.user = NULL

        # Cannot be done until https://github.com/lexbor/lexbor/issues/132 is fixed
        # If you create lots of unparented DOMNodes, we may leak memory
        # if not self.node.parent and self.node != <lxb_dom_node_t*>self.node.owner_document:
        #     lxb_dom_node_destroy_deep(self.node)
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
            node = next_node(self.node, node)
            if not node:
                return
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
        DOM element tag or node name.

        :type: str or None
        """
        if not check_node(self):
            return None

        cdef size_t name_len = 0
        cdef const lxb_char_t* name = lxb_dom_node_name(self.node, &name_len)
        if not name:
            return None
        return name[:name_len].decode().lower()

    # noinspection DuplicatedCode
    @property
    def first_child(self):
        """
        First child element of this DOM node.

        :type: DOMNode or None
        """
        if not check_node(self):
            return None
        return _create_dom_node(self.tree, self.node.first_child)

    # noinspection DuplicatedCode
    @property
    def last_child(self):
        """
        Last child element of this DOM node.

        :type: DOMNode or None
        """
        if not check_node(self):
            return None
        return _create_dom_node(self.tree, self.node.last_child)

    # noinspection DuplicatedCode
    @property
    def first_element_child(self):
        """
        First element child of this DOM node.

        :type: DOMNode or None
        """
        if not check_node(self):
            return None
        cdef lxb_dom_node_t* child_node = self.node.first_child
        while child_node and child_node.type != LXB_DOM_NODE_TYPE_ELEMENT:
            child_node = child_node.next
        return _create_dom_node(self.tree, child_node)

    # noinspection DuplicatedCode
    @property
    def last_element_child(self):
        """
        Last element child element of this DOM node.

        :type: DOMNode or None
        """
        if not check_node(self):
            return None
        cdef lxb_dom_node_t* child_node = self.node.last_child
        while child_node and child_node.type != LXB_DOM_NODE_TYPE_ELEMENT:
            child_node = child_node.prev
        return _create_dom_node(self.tree, child_node)

    @property
    def child_nodes(self):
        """
        List of child nodes.

        :type: t.List[DOMNode]
        """
        if not check_node(self):
            return

        cdef lxb_dom_node_t* child = self.node.first_child
        child_nodes = []
        while child:
            child_nodes.append(_create_dom_node(self.tree, child))
            child = child.next
        return child_nodes

    @property
    def parent(self):
        """
        Parent of this node.

        :type: DOMNode or None
        """
        if not check_node(self):
            return None
        return _create_dom_node(self.tree, self.node.parent)

    # noinspection DuplicatedCode
    @property
    def next(self):
        """
        Next sibling node.

        :type: DOMNode or None
        """
        if not check_node(self):
            return None
        return _create_dom_node(self.tree, self.node.next)

    # noinspection DuplicatedCode
    @property
    def prev(self):
        """
        Previous sibling node.

        :type: DOMNode or None
        """
        if not check_node(self):
            return None
        return _create_dom_node(self.tree, self.node.prev)

    # noinspection DuplicatedCode
    @property
    def next_element(self):
        """
        Next sibling element node.

        :type: DOMNode or None
        """
        if not check_node(self):
            return None

        cdef lxb_dom_node_t* next_node = self.node.next
        while next_node and next_node.type != LXB_DOM_NODE_TYPE_ELEMENT:
            next_node = next_node.next
        return _create_dom_node(self.tree, next_node)

    # noinspection DuplicatedCode
    @property
    def prev_element(self):
        """
        Previous sibling element node.

        :type: DOMNode or None
        """
        if not check_node(self):
            return None

        cdef lxb_dom_node_t* prev_node = self.node.prev
        while prev_node and prev_node.type != LXB_DOM_NODE_TYPE_ELEMENT:
            prev_node = prev_node.prev
        return _create_dom_node(self.tree, prev_node)

    @property
    def value(self):
        """
        Node text value.

        :type: str or None
        """
        if self.node.type not in [LXB_DOM_NODE_TYPE_TEXT,
                                  LXB_DOM_NODE_TYPE_PROCESSING_INSTRUCTION,
                                  LXB_DOM_NODE_TYPE_COMMENT]:
            return None

        cdef lxb_dom_character_data_t* cd = <lxb_dom_character_data_t*>self.node
        return cd.data.data[:cd.data.length].decode()

    @property
    def text(self):
        """
        Text contents of this DOM node and its children.

        The DOM node's inner text can be modified by assigning to this property.

        :type: str
        """
        if not check_node(self):
            return None

        return get_node_text(self.node).decode()

    @text.setter
    def text(self, str text):
        if not check_node(self):
            raise RuntimeError('Trying to set text contents of uninitialized DOM node')

        cdef bytes text_bytes = text.encode()
        lxb_dom_node_text_content_set(self.node, <const lxb_char_t*>text_bytes, len(text_bytes))

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
        cdef str py_text = html_str.data[:html_str.length].decode()
        lexbor_str_destroy(html_str, self.node.owner_document.text, True)
        return py_text

    @html.setter
    def html(self, str html):
        if not check_node(self):
            raise RuntimeError('Trying to set HTML contents of uninitialized DOM node')

        cdef bytes html_bytes = html.encode()
        lxb_html_element_inner_html_set(<lxb_html_element_t*>self.node, <const lxb_char_t*>html_bytes, len(html_bytes))

    @property
    def id(self):
        """
        ID attribute of this Element node (empty string if unset).

        :type: str
        """
        if not check_node(self) or self.node.type != LXB_DOM_NODE_TYPE_ELEMENT:
            return None

        cdef size_t value_len = 0
        cdef const lxb_char_t* value = lxb_dom_element_id(<lxb_dom_element_t*>self.node, &value_len)
        if not value:
            return ''
        return value[:value_len].decode()

    @id.setter
    def id(self, str class_name):
        self._setattr_impl(b'id', class_name.encode())

    @property
    def class_name(self):
        """
        Class name attribute of this Element node (empty string if unset).

        :type: str
        """
        if not check_node(self) or self.node.type != LXB_DOM_NODE_TYPE_ELEMENT:
            return None

        cdef size_t value_len = 0
        cdef const lxb_char_t* value = lxb_dom_element_class(<lxb_dom_element_t*>self.node, &value_len)
        if not value:
            return ''
        return value[:value_len].decode()

    @class_name.setter
    def class_name(self, str class_name):
        self._setattr_impl(b'class', class_name.encode())

    @property
    def class_list(self):
        """
        List of class names set on this Element node.

        :type: DOMElementClassList
        """
        if not check_node(self) or self.node.type != LXB_DOM_NODE_TYPE_ELEMENT:
            return None

        if self.class_list_singleton is None:
            # noinspection PyAttributeOutsideInit
            self.class_list_singleton = DOMElementClassList.__new__(DOMElementClassList, self)
        return self.class_list_singleton

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
        while attr:
            local_name = lxb_dom_attr_local_name(attr, &local_name_len)
            attrs.append(local_name[:local_name_len].decode())
            attr = attr.next

        return attrs

    cpdef bint hasattr(self, str attr_name) except -1:
        """
        hasattr(self, attr_name)
        
        Check if node has an attribute with the given name.

        :param attr_name: attribute name
        :type attr_name: str
        :rtype: bool
        :raises ValueError: if node is not an Element node
        """
        if not check_node(self) or self.node.type != LXB_DOM_NODE_TYPE_ELEMENT:
            raise ValueError('Node ist not an Element node.')

        cdef bytes attr_name_bytes = attr_name.encode()
        return <bint>lxb_dom_element_has_attribute(<lxb_dom_element_t*>self.node,
                                                   <const lxb_char_t*>attr_name_bytes, len(attr_name_bytes))

    cdef str _getattr_impl(self, const string& attr_name, bint raise_if_not_exists=True):
        """
        Get an attribute value as string.
        
        :param attr_name: 
        :return: attribute value or None
        :raises KeyError: if attribute does not exist
        """
        if not check_node(self) or self.node.type != LXB_DOM_NODE_TYPE_ELEMENT:
            raise ValueError('Node ist not an Element node.')

        if raise_if_not_exists and not lxb_dom_element_has_attribute(
                <lxb_dom_element_t*>self.node, <const lxb_char_t*>attr_name.data(), attr_name.size()):
            raise KeyError(f'No such attribute: {attr_name.decode()}')

        cdef size_t node_attr_len
        cdef const lxb_char_t* node_attr_data = lxb_dom_element_get_attribute(
            <lxb_dom_element_t*>self.node, <const lxb_char_t*>attr_name.data(), attr_name.size(), &node_attr_len)
        return node_attr_data[:node_attr_len].decode()

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
        :raises ValueError: if node is not an Element node
        """
        if not check_node(self):
            return None

        try:
            return self._getattr_impl(attr_name.encode())
        except KeyError:
            return default_value

    def __getitem__(self, str attr_name):
        """
        __getitem__(self, attr_name)

        Get the value of the an attribute.

        :param attr_name: attribute name
        :rtype: str
        :raises KeyError: if no such attribute exists
        :raises ValueError: if node is not an Element node
        """
        if not check_node(self):
            return None

        return self._getattr_impl(attr_name.encode())

    cdef bint _setattr_impl(self, bytes attr_name, bytes attr_value) except -1:
        """
        Insert or update an attribute with the given value.
        
        :param attr_name: attribute value
        :param attr_value: attribute name
        :return: attribute value or None
        """
        if not check_node(self) or self.node.type != LXB_DOM_NODE_TYPE_ELEMENT:
            raise ValueError('Node ist not an Element node.')

        # Lexbor's check of existing attributes is buggy
        cdef lxb_dom_attr_t* attr = lxb_dom_element_attr_is_exist(<lxb_dom_element_t*>self.node,
                                                                  <const lxb_char_t*>attr_name, len(attr_name))

        if attr:
            lxb_dom_attr_set_value(attr, <const lxb_char_t*>attr_value, len(attr_value))
            return True

        lxb_dom_element_set_attribute(<lxb_dom_element_t*>self.node,
                                      <const lxb_char_t*>attr_name, len(attr_name),
                                      <const lxb_char_t*>attr_value, len(attr_value))
        return True

    cpdef setattr(self, str attr_name, str attr_value):
        """
        setattr(self, attr_name, attr_value)
        
        Insert or update an attribute with the given name to the given value.

        :param attr_name: attribute name
        :type attr_name: str
        :param attr_value: attribute value
        :type attr_value: str
        :return: attribute value
        :raises ValueError: if node is not an Element node
        """
        if not check_node(self):
            return

        self._setattr_impl(attr_name.encode(), attr_value.encode())

    def __setitem__(self, str attr_name, str attr_value):
        """
        __setitem__(self, attr_name, attr_value)

        Insert or update an attribute with the given name to the given value.

        :param attr_name: attribute name
        :type attr_name: str
        :param attr_value: attribute value
        :type attr_value: str
        :return: attribute value
        :raises ValueError: if node is not an Element node
        """
        if not check_node(self):
            return

        self._setattr_impl(attr_name.encode(), attr_value.encode())

    cdef bint _delattr_impl(self, bytes attr_name) except -1:
        """
        Remove the given attribute.

        :param attr_name: attribute to remove
        """
        if not check_node(self) or self.node.type != LXB_DOM_NODE_TYPE_ELEMENT:
            raise ValueError('Node ist not an Element node.')

        cdef lxb_dom_attr_t* attr = lxb_dom_element_attr_is_exist(<lxb_dom_element_t*>self.node,
                                                                  <const lxb_char_t*>attr_name, len(attr_name))
        if not attr:
            return False

        lxb_dom_element_attr_remove(<lxb_dom_element_t*>self.node, attr)
        return True

    cpdef delattr(self, str attr_name):
        """
        delattr(self, attr_name)
        
        Remove the given attribute if it exists.

        :param attr_name: attribute to remove
        :type attr_name: str
        :raises ValueError: if node is not an Element node
        """
        self._delattr_impl(attr_name.encode())

    def __delitem__(self, str attr_name):
        """
        __delitem__(self, attr_name)

        Remove the given attribute.

        :param attr_name: attribute to remove
        :type attr_name: str
        :raises ValueError: if node is not an Element node
        :raises KeyError: if node 
        """
        cdef bint ret = self._delattr_impl(attr_name.encode())
        if not ret:
            raise KeyError(f'No such attribute: {attr_name}')

    cpdef DOMNode query_selector(self, str selector):
        """
        query_selector(self, selector)
         
        Find and return the first element matching the given CSS selector. This is more efficient
        than matching with :meth:`query_selector_all` and discarding additional elements.

        :param selector: CSS selector
        :type selector: str
        :return: matching element or ``None``
        :rtype: DOMNode or None
        """
        cdef lxb_dom_node_t* node = query_selector_impl(self.node, self.tree, selector.encode())
        if not node:
            return None
        return _create_dom_node(self.tree, node)

    cpdef DOMCollection query_selector_all(self, str selector):
        """
        query_selector_all(self, selector)
        
        Find all elements matching the given CSS selector and return a :class:`DOMCollection` with the results.

        :param selector: CSS selector
        :type selector: str
        :return: collection of matching elements
        :rtype: DOMCollection
        """
        cdef lxb_dom_collection_t* coll = query_selector_all_impl(self.node, self.tree, selector.encode())
        if not coll:
            raise RuntimeError('Failed to match elements by CSS selector')

        return _create_dom_collection(self.tree, coll)

    cpdef bint matches(self, str selector) except -1:
        """
        matches(self, selector)
        
        Check whether any element in the DOM tree matches the given CSS selector. This is more efficient
        than matching with :meth:`query_selector_all` and checking the size of the returned collection.

        :param selector: CSS selector
        :type selector: str
        :return: boolean value indicating whether a matching element exists
        :rtype: bool
        """
        self.tree.init_css_parser()
        return matches_impl(self.node, self.tree, selector.encode())

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

        return _create_dom_collection(self.tree,
                                      get_elements_by_attr_impl(self.node, attr_name.encode(), attr_value.encode(),
                                                                10, case_insensitive))

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

        cdef lxb_dom_node_t* result_node = get_element_by_id_impl(self.node, element_id.encode(), case_insensitive)
        if not result_node:
            return None
        return _create_dom_node(self.tree, result_node)

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

        return _create_dom_collection(self.tree, get_elements_by_class_name_impl(self.node, class_name.encode(), 10))

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

        if node.node.parent:
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

        if node.node.parent:
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

        if new_child.node.parent:
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
    cpdef decompose(self):
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
        if self.coll:
            lxb_dom_collection_destroy(self.coll, True)
            self.coll = NULL

    @cython.cdivision(False)
    cdef inline size_t _wrap_idx(self, ssize_t idx):
        if idx >= 0:
            return idx
        return idx % <ssize_t>lxb_dom_collection_length(self.coll)

    cdef DOMCollection _forward_collection_match(self, bytes func, attrs):
        """
        Forward DOM multi-element match operations to all items in the collection and aggregate the results.
        
        :param func: internal matching function as bytes (b'by_attr', b'by_class', b'by_tag', or b'selector_all')
        :param attrs: tuple of attributes to pass to the matching function
        :return: aggregated collection or single element
        """
        if self.tree is None or not self.coll:
            raise RuntimeError('Trying to select items from uninitialized collection')

        if not lxb_dom_collection_length(self.coll):
            return self

        cdef lxb_dom_collection_t* joined_coll = lxb_dom_collection_make(self.coll.document,
                                                                         lxb_dom_collection_length(self.coll) * 2)

        cdef lxb_dom_node_t* node = NULL
        cdef lxb_dom_collection_t* matches = NULL
        cdef size_t i = 0
        for i in range(lxb_dom_collection_length(self.coll)):
            node = lxb_dom_collection_node(self.coll, i)
            if func == b'by_attr':
                matches = get_elements_by_attr_impl(node, attrs[0], attrs[1], attrs[2], attrs[3])
            if func == b'by_class':
                matches = get_elements_by_class_name_impl(node, attrs[0], attrs[1])
            elif func == b'by_tag':
                matches = get_elements_by_tag_name_impl(node, attrs[0])
            elif func == b'selector_all':
                matches = query_selector_all_impl(node, self.tree, attrs[0])

            if matches:
                _join_collections(joined_coll, matches)
                lxb_dom_collection_destroy(matches, True)

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
        if self.tree is None or not self.coll:
            raise RuntimeError('Trying to select items from uninitialized collection')

        cdef lxb_dom_node_t* node
        cdef size_t i = 0
        cdef bytes id_bytes = element_id.encode()
        for i in range(lxb_dom_collection_length(self.coll)):
            node = get_element_by_id_impl(lxb_dom_collection_node(self.coll, i), id_bytes, case_insensitive)
            if node:
                return _create_dom_node(self.tree, node)

        return None

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
        return self._forward_collection_match(b'by_attr',
                                              (attr_name.encode(), attr_value.encode(),  10, case_insensitive))

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
        return self._forward_collection_match(b'by_class', (class_name.encode(), 10))

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
        return self._forward_collection_match(b'by_tag', (tag_name.encode(),))

    cpdef DOMNode query_selector(self, str selector):
        """
        query_selector(self, selector)

        Within all elements in this collection, find and return the first element matching
        the given CSS selector.This is more efficient than matching with :meth:`query_selector_all`
        and discarding additional elements.

        :param selector: CSS selector
        :type selector: str
        :return: matching element or ``None``
        :rtype: DOMNode or None
        """
        if self.tree is None or not self.coll:
            raise RuntimeError('Trying to select items from uninitialized collection')

        cdef lxb_dom_node_t* node
        cdef size_t i = 0
        cdef bytes selector_bytes = selector.encode()
        for i in range(lxb_dom_collection_length(self.coll)):
            node = query_selector_impl(lxb_dom_collection_node(self.coll, i), self.tree, selector_bytes)
            if node:
                return _create_dom_node(self.tree, node)

        return None

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
        return self._forward_collection_match(b'selector_all', (selector.encode(),))

    cpdef bint matches(self, str selector) except -1:
        """
        matches(self, selector)

        Within all elements in this collection, check whether any element in the DOM tree
        matches the given CSS selector. This is more efficient than matching with
        :meth:`query_selector_all` and checking the size of the returned collection.

        :param selector: CSS selector
        :type selector: str
        :return: boolean value indicating whether a matching element exists
        :rtype: bool
        """
        if self.tree is None or not self.coll:
            raise RuntimeError('Trying to select items from uninitialized collection')

        cdef size_t i = 0
        cdef bytes selector_bytes = selector.encode()
        for i in range(lxb_dom_collection_length(self.coll)):
            if matches_impl(lxb_dom_collection_node(self.coll, i), self.tree, selector_bytes):
                return True

        return False

    def __iter__(self):
        """
        __iter__(self)

        Iterate the DOM node collection.

        :rtype: t.Iterable[DOMNode]
        """
        if self.tree is None or not self.coll:
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
        if self.tree is None or not self.coll:
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
        if self.tree is None or not self.coll:
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


cdef HTMLTree create_html_tree(bytes document, bint reencode=True, str encoding='utf-8', str errors='ignore'):
    """
    Create :class:`HTMLTree` instance from bytes content.
    
    :param document: input document bytes
    :param reencode: whether to reencode the bytes to erase encoding errors
    :param encoding: input encoding (best guess)
    :param errors: decoding error policy
    :return: :class:`HTMLTree` instance
    """
    encoding = map_encoding_to_html5(encoding)
    if reencode:
        document = bytes_to_str(document, encoding, errors).encode()
    cdef lxb_status_t status
    cdef const lxb_char_t* html = <const lxb_char_t*>document
    cdef size_t html_len = len(document)
    cdef HTMLTree tree = HTMLTree.__new__(HTMLTree)
    with nogil:
        status = lxb_html_document_parse(tree.dom_document, html, html_len)
    if status != LXB_STATUS_OK:
        raise ValueError('Failed to parse HTML document')
    return tree


cdef class HTMLTree:
    """
    __init__(self)

    HTML DOM tree parser.
    """

    def __init__(self, *args, **kwargs):
        raise SyntaxError('Cannot create instance directly. Use HTMLParser.parse() or HTMLParser.parse_from_bytes()')

    def __cinit__(self):
        self.dom_document = lxb_html_document_create()
        if not self.dom_document:
            raise RuntimeError('Failed to allocate HTML document')
        self.css_parser = NULL
        self.css_selectors = NULL
        self.selectors = NULL

    def __dealloc__(self):
        if self.dom_document:
            lxb_html_document_destroy(self.dom_document)
            self.dom_document = NULL

        if self.css_parser or self.css_parser or self.css_selectors:
            destroy_css_selectors(self.css_selectors, self.selectors)
            destroy_css_parser(self.css_parser)
            self.css_parser = NULL
            self.css_parser = NULL
            self.css_selectors = NULL

    # noinspection PyAttributeOutsideInit
    cdef inline void init_css_parser(self):
        """
        Initialize CSS selector if not already initialized.
        """
        if not self.css_parser:
            init_css_parser(&self.css_parser)
            init_css_selectors(self.css_parser, &self.css_selectors, &self.selectors)


    @classmethod
    def parse(cls, str document):
        """
        parse(self, document)
        
        Parse HTML from a Unicode string into a DOM tree.
        
        :param document: input HTML document
        :return: HTML DOM tree
        :rtype: HTMLTree
        :raises ValueError: if HTML parsing fails for unknown reasons
        """
        return create_html_tree(document.encode(), reencode=False)

    @classmethod
    def parse_from_bytes(cls, bytes document, str encoding='utf-8', str errors='ignore'):
        """
        parse_from_bytes(self, document, encoding='utf-8', errors='ignore')
        
        Decode a raw HTML byte string and parse it into a DOM tree.
        
        The decoding routine uses :func:`~.parse.encoding.bytes_to_str` to take care of decoding errors,
        so it is sufficient if ``encoding`` is just a best guess of what the actual input encoding is.
        The encoding name will be remapped according to the WHATWG specification by calling
        :func:`~.parse.encoding.map_encoding_to_html5` before trying to decode the byte string with it.
        
        :param document: input byte string
        :param encoding: encoding for decoding byte string
        :param errors: decoding error policy (same as ``str.decode()``)
        :return: HTML DOM tree
        :rtype: HTMLTree
        :raises ValueError: if HTML parsing fails for unknown reasons
        """
        return create_html_tree(document, True, encoding, errors)

    @property
    def document(self):
        """
        Document root node.

        :type: DOMNode or None
        """
        if not self.dom_document:
            return None

        return _create_dom_node(self, <lxb_dom_node_t*>&self.dom_document.dom_document)

    @property
    def head(self):
        """
        HTML head element or ``None`` if document has no head.

        :type: DOMNode or None
        """
        if not self.dom_document:
            return None

        return _create_dom_node(self, <lxb_dom_node_t*>lxb_html_document_head_element(self.dom_document))

    @property
    def body(self):
        """
        HTML body element or ``None`` if document has no body.

        :type: DOMNode or None
        """
        if not self.dom_document:
            return None

        return _create_dom_node(self, <lxb_dom_node_t*>lxb_html_document_body_element(self.dom_document))

    @property
    def title(self):
        """
        The HTML document title.

        :type: str or None
        """
        if not self.dom_document or not self.head:
            return None

        cdef size_t title_len = 0
        cdef const lxb_char_t* title = lxb_html_document_title(self.dom_document, &title_len)
        if not title:
            return ''
        return title[:title_len].decode()

    cpdef DOMNode create_element(self, str tag_name):
        """
        create_element(self, tag_name)
        
        Create a new DOM Element node.
        
        :param tag_name: element tag name
        :type tag_name: str
        :return: new Element node
        :rtype: DOMNode
        """
        if not self.dom_document:
            raise RuntimeError('Trying to create element in uninitialized document.')

        cdef bytes tag_name_bytes = tag_name.encode()
        cdef lxb_dom_element_t* element = lxb_dom_document_create_element(
            <lxb_dom_document_t*>self.dom_document, <const lxb_char_t*>tag_name_bytes, len(tag_name_bytes), NULL)
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
        if not self.dom_document:
            raise RuntimeError('Trying to create text node in uninitialized document.')

        cdef bytes text_bytes = text.encode()
        cdef lxb_dom_text_t* node = lxb_dom_document_create_text_node(
            <lxb_dom_document_t*>self.dom_document, <const lxb_char_t*>text_bytes, len(text_bytes))
        return _create_dom_node(self, <lxb_dom_node_t*>node)

    def __str__(self):
        cdef DOMNode doc = self.document
        if doc is not None:
            return doc.html
        return ''


class DOMContext:
    """
    __init__()

    DOM node traversal context object.

    The context object has two attributes that are set by the traversal function for
    keeping track of the current :class:`DOMNode` and the current traversal depth.
    Besides these, the context object is arbitrarily mutable and can be used for
    maintaining custom state.

    :ivar DOMNode node: the current :class:`DOMNode`
    :ivar int depth: the current traversal depth
    """
    def __init__(self):
        self.node = None
        self.depth = 0


def traverse_dom(DOMNode base_node, start_callback, end_callback=None, context=None, bint elements_only=False):
    """
    traverse_dom(base_node, start_callback, end_callback=None, context=None, elements_only=False)

    DOM traversal helper.

    Traverses the DOM tree starting at ``base_node`` in pre-order and calls ``start_callback``
    at each child node. If ``end_callback`` is not ``None``, it will be called each time
    a DOM element's end tag is encountered.

    The callbacks are expected to take exactly one :class:`DOMContext` context parameter,
    which keeps track of the current node and traversal depth. The context object will be
    the same throughout the whole traversal process, so it can be mutated with custom data.

    :param base_node: root node of the traversal
    :type base_node: DOMNode
    :param start_callback: callback for each DOM node on the way (takes a :class:`DOMNode`
                           and ``context`` as a parameter)
    :type start_callback: t.Callable[[DOMContext], None]
    :param end_callback: optional callback for element node end tags (takes a :class:`DOMNode`
                         and ``context`` as a parameter)
    :type end_callback: t.Callable[[DOMContext], None] or None
    :param context: optional pre-initialized context object
    :type context: DOMContext
    :param elements_only: traverse only element nodes
    :type elements_only: bool
    """

    cdef lxb_dom_node_t* node = base_node.node
    cdef size_t depth = 0
    cdef bint is_end_tag = False
    cdef bint* is_end_tag_ptr = &is_end_tag if end_callback is not None else NULL

    if elements_only and base_node.node.type != LXB_DOM_NODE_TYPE_ELEMENT:
        return

    context = context or DOMContext()

    while node:
        context.node = _create_dom_node(base_node.tree, node)
        context.depth = depth

        if not is_end_tag:
            start_callback(context)
        else:
            end_callback(context)

        if elements_only:
            node = next_element_node(base_node.node, node, &depth, is_end_tag_ptr)
        else:
            node = next_node(base_node.node, node, &depth, is_end_tag_ptr)


cdef unordered_set[lxb_tag_id_t] BLOCK_ELEMENT_SET

cdef inline void _init_block_element_set() nogil:
    if not BLOCK_ELEMENT_SET.empty():
        return
    cdef size_t i
    for i in range(NUM_BLOCK_ELEMENTS):
        BLOCK_ELEMENT_SET.insert(BLOCK_ELEMENTS[i])

_init_block_element_set()


cdef inline bint is_block_element(lxb_tag_id_t tag_id) nogil:
    """
    Check whether an element is a block-level element.
    """
    return BLOCK_ELEMENT_SET.find(tag_id) != BLOCK_ELEMENT_SET.end()
