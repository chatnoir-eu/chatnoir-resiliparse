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

from resiliparse_inc.lexbor cimport *

from resiliparse.parse.encoding cimport bytes_to_str, map_encoding_to_html5


cdef inline DOMNode _node_from_dom(lxb_dom_node_t* dom_node):
    if dom_node == NULL:
        return None
    cdef DOMNode node = DOMNode.__new__(DOMNode)
    node.node = dom_node
    return node


cdef inline DOMAttribute _attr_from_dom(lxb_dom_attr_t* attr_node):
    if attr_node == NULL:
        return None
    cdef DOMAttribute node = DOMAttribute.__new__(DOMAttribute)
    node.attr = attr_node
    return node


cdef class DOMAttribute:
    """
    __init__(self)

    A DOM element attribute.

    An attribute is only valid as long as the owning :class:`HTMLTree` and :class:`Node` are
    alive and the DOM tree hasn't been modified. Do not access :class:`Attribute` instances after any
    sort of DOM tree manipulation.
    """

    def __cinit__(self):
        self.attr = NULL

    @property
    def name(self):
        """
        Attribute name.

        :rtype: str | None
        """
        if self.attr == NULL:
            return None

        cdef size_t name_len = 0
        cdef const lxb_char_t* name = lxb_dom_attr_local_name(self.attr, &name_len)
        if name == NULL:
            return None
        return bytes_to_str(name[:name_len])

    @property
    def value(self):
        """
        Attribute value.

        :rtype: str | None
        """
        if self.attr == NULL:
            return None
        cdef size_t val_len = 0
        cdef const lxb_char_t* val = lxb_dom_attr_value(self.attr, &val_len)
        return bytes_to_str(val[:val_len])

    def __repr__(self):
        return f'{self.name}="{self.value}"'

    def __str__(self):
        return self.value


cdef class DOMNode:
    """
    __init__(self)

    A DOM node.

    DOM nodes and their children are iterable and will be traversed in pre-order.

    A DOM node is only valid as long as the owning :class:`HTMLTree` is alive
    and the DOM tree hasn't been modified. Do not access :class:`Node` instances
    after any sort of DOM tree manipulation.
    """

    def __cinit__(self):
        self.node = NULL

    def __iter__(self):
        """
        __iter__(self)

        Run a pre-order traversal of the DOM tree starting at the current node.

        :rtype: t.Iterable[DOMNode]
        """
        if self.node == NULL:
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

            yield _node_from_dom(node)

    @property
    def type(self):
        """
        DOM node type.

        :rtype: NodeType | None
        """
        if self.node == NULL:
            return None
        return <NodeType>self.node.type

    @property
    def tag(self):
        """
        DOM node tag name.

        :rtype: str | None
        """
        if self.node == NULL or self.node.type != LXB_DOM_NODE_TYPE_ELEMENT:
            return None
        cdef size_t name_len = 0
        cdef unsigned char* name = <unsigned char*>lxb_dom_element_qualified_name(
            <lxb_dom_element_t*>self.node, &name_len)
        if name == NULL:
            return None
        return bytes_to_str(name[:name_len])

    @property
    def first_child(self):
        """
        First child element of this DOM node.

        :rtype: DOMNode | None
        """
        if self.node == NULL:
            return None
        return _node_from_dom(self.node.first_child)

    @property
    def last_child(self):
        """
        Last child element of this DOM node.

        :rtype: DOMNode | None
        """
        if self.node == NULL:
            return None
        return _node_from_dom(self.node.last_child)

    @property
    def parent(self):
        """
        Parent of this node.

        :rtype: DOMNode | None
        """
        if self.node == NULL:
            return None
        return _node_from_dom(self.node.parent)

    @property
    def next(self):
        """
        Next sibling node.

        :rtype: DOMNode | None
        """
        if self.node == NULL:
            return None
        return _node_from_dom(self.node.next)

    @property
    def prev(self):
        """
        Previous sibling node.

        :rtype: DOMNode | None
        """
        if self.node == NULL:
            return None
        return _node_from_dom(self.node.prev)

    @property
    def text(self):
        """
        Text contents of this DOM node and its children.

        :rtype: str | None
        """
        if self.node == NULL:
            return None
        cdef size_t text_len = 0
        cdef lxb_char_t* text = lxb_dom_node_text_content(self.node, &text_len)
        return bytes_to_str(text[:text_len])

    @property
    def attrs(self):
        """
        List of attributes.

        :rtype: List[DOMAttribute]
        """
        attrs = []
        if self.node == NULL or self.node.type != LXB_DOM_NODE_TYPE_ELEMENT:
            return attrs

        cdef lxb_dom_attr_t* attr = lxb_dom_element_first_attribute(<lxb_dom_element_t*>self.node)
        while attr != NULL:
            attrs.append(_attr_from_dom(attr))
            attr = attr.next

        return attrs

    cpdef bint hasattr(self, str attr_name):
        """
        hasattr(self, attr_name)
        
        Check if node has attribute.

        :param attr_name: attribute name
        :rtype: bool
        """
        if self.node == NULL or self.node.type != LXB_DOM_NODE_TYPE_ELEMENT:
            return False
        cdef bytes attr_name_bytes = attr_name.encode()
        return <bint>lxb_dom_element_has_attribute(<lxb_dom_element_t*>self.node,
                                                   <lxb_char_t*>attr_name_bytes, len(attr_name_bytes))

    cdef DOMAttribute _getattr_impl(self, str attr_name):
        if self.node == NULL or self.node.type != LXB_DOM_NODE_TYPE_ELEMENT:
            raise ValueError('Node ist not an Element node.')

        cdef bytes attr_name_bytes = attr_name.encode()
        cdef lxb_dom_attr_t* attr = lxb_dom_element_attr_by_name(<lxb_dom_element_t*>self.node,
                                                                 <lxb_char_t*>attr_name_bytes, len(attr_name_bytes))
        if attr == NULL:
            raise KeyError(f'No such attribute: {attr_name_bytes}')

        return _attr_from_dom(attr)

    cdef lxb_dom_collection_t* _match_by_attr(self, bytes attr_name, bytes attr_value, size_t init_size=5,
                                              bint case_insensitive=False):
        """
        Return a collection of elements matching the given attribute name and value.
        
        The caller must take ownership of the returned collection.
        
        :param attr_name: attribute name as bytes
        :param attr_value: attribute value as bytes
        :param init_size: initial collection size
        :param case_insensitive: match case-insensitive
        :return: pointer to created DOM collection or ``NULL`` if error occurred
        """
        cdef lxb_dom_collection_t* coll = lxb_dom_collection_make(self.node.owner_document, 1)
        if coll == NULL:
            return NULL

        cdef lxb_status_t status = lxb_dom_elements_by_attr(<lxb_dom_element_t*> self.node, coll,
                                                            <lxb_char_t*>attr_name, len(attr_name),
                                                            <lxb_char_t*>attr_value, len(attr_value),
                                                            case_insensitive)
        if status != LXB_STATUS_OK:
            lxb_dom_collection_destroy(coll, True)
            return NULL

        return coll

    cpdef DOMNode get_element_by_id(self, str element_id, bint case_insensitive=False):
        """
        get_element_by_id(self, element_id, case_insensitive=False)
        
        Return element matching element ID ``element_id`` or ``None`` if no such element exists.
        
        :param element_id: element ID
        :type element_id: str
        :param case_insensitive: match ID case-insensitively
        :type case_insensitive: bool
        :return: matching :class:`Node` or `None`
        :rtype: DOMNode | None
        """
        if self.node == NULL:
            return None

        cdef lxb_dom_collection_t* coll = self._match_by_attr(b'id', element_id.encode(), 5, case_insensitive)
        if coll == NULL:
            raise RuntimeError('Failed to match element by ID')

        cdef DOMNode return_node = None
        if lxb_dom_collection_length(coll) > 0:
            return_node = _node_from_dom(<lxb_dom_node_t*>lxb_dom_collection_element(coll, 0))
        lxb_dom_collection_destroy(coll, True)
        return return_node

    cpdef DOMNodeCollection get_elements_by_class_name(self, str element_class, bint case_insensitive=False):
        """
        get_elements_by_class_name(self, element_class, case_insensitive=False)
        
        Get a :class:`NodeCollection` with all DOM elements matching the class attribute ``element_class``.
        
        :param element_class: element class
        :type element_class: str
        :param case_insensitive: match class name case-insensitively
        :type case_insensitive: bool
        :return: matching :class:`Node` or `None`
        :rtype: DOMNode | None
        """
        if self.node == NULL:
            return None

        cdef lxb_dom_collection_t * coll = self._match_by_attr(b'class', element_class.encode(), 5, case_insensitive)
        if coll == NULL:
            raise RuntimeError('Failed to match element by class name')

        cdef DOMNodeCollection result_coll = DOMNodeCollection.__new__(DOMNodeCollection)
        result_coll.coll = coll
        return result_coll

    cpdef DOMNodeCollection get_elements_by_tag_name(self, str tag_name):
        """
        get_elements_by_tag_name(self, tag_name)
        
        Get a :class:`NodeCollection` with all DOM elements matching the tag name ``tag_name``.
        
        :param tag_name: tag name for matching elements
        :type tag_name: str
        :return: :class:`NodeCollection` of matching elements
        :rtype: DOMNodeCollection
        """
        if self.node == NULL:
            return None

        cdef lxb_dom_collection_t* coll = lxb_dom_collection_make(self.node.owner_document, 5)
        if coll == NULL:
            raise RuntimeError('Failed to create DOM collection')

        cdef bytes tag_bytes = tag_name.encode()
        cdef lxb_status_t status = lxb_dom_elements_by_tag_name(<lxb_dom_element_t*>self.node,
                                                                coll, <lxb_char_t*>tag_bytes, len(tag_bytes))
        if status != LXB_STATUS_OK:
            raise RuntimeError('Failed to match elements')
        cdef DOMNodeCollection result_coll = DOMNodeCollection.__new__(DOMNodeCollection)
        result_coll.coll = coll
        return result_coll

    cpdef getattr(self, str attr_name, default_value=None):
        """
        getattr(self, attr_name, default_value=None)
        
        Get attribute or ``default_value``.

        :param attr_name: attribute name
        :param default_value: default value to return if attribute is unset
        """
        if not self.hasattr(attr_name):
            return default_value

        return self._getattr_impl(attr_name)

    def __getitem__(self, str attr_name):
        """
        __getitem__(self, attr_name)

        Get attribute.

        :param attr_name: attribute name
        :rtype: DOMAttribute | None
        :raises KeyError: if no such attribute exists
        :raises ValueError: if node ist not an Element node
        """
        return self._getattr_impl(attr_name)

    def __repr__(self):
        if self.node.type == LXB_DOM_NODE_TYPE_ELEMENT:
            attrs = ' '.join(repr(a) for a in self.attrs)
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
        return self.__repr__()


cdef class DOMNodeCollection:
    """
    __init__(self)

    Collection of DOM nodes that are a the result set of an element match operation.

    A node collection is only valid as long as the owning :class:`HTMLTree` is alive
    and the DOM tree hasn't been modified. Do not access :class:`NodeCollection` instances
    after any sort of DOM tree manipulation.
    """

    def __cinit__(self):
        self.coll = NULL

    def __dealloc__(self):
        if self.coll != NULL:
            lxb_dom_collection_destroy(self.coll, True)
            self.coll = NULL

    cdef inline size_t _wrap_idx(self, ssize_t idx):
        if idx >= 0:
            return idx
        return idx % <ssize_t>lxb_dom_collection_length(self.coll)

    def __iter__(self):
        """
        __iter__(self)

        Iterate DOM node collection.

        :rtype: t.Iterable[DOMNode]
        """
        if self.coll == NULL:
            return

        cdef size_t i = 0
        for i in range(lxb_dom_collection_length(self.coll)):
            yield _node_from_dom(<lxb_dom_node_t*>lxb_dom_collection_element(self.coll, i))

    def __len__(self):
        """
        __len__(self)

        Collection length.

        :rtype: int
        """
        if self.coll == NULL:
            return 0
        return lxb_dom_collection_length(self.coll)

    def __getitem__(self, key):
        """
        __getitem__(self, key)

        Return the :class:`Node` at the given index in this collection or another :class:`NodeCollection`
        if ``key`` is a slice object. Negative indexing is supported.

        :param key: index or slice
        :rtype: DOMNode | DOMNodeCollection
        :raises IndexError: if ``key`` is out of range
        :raises TypeError: if ``key`` is not an ``int`` or ``slice``
        """
        if self.coll == NULL:
            raise IndexError('Trying to get item of uninitialized collection')

        cdef size_t coll_len = lxb_dom_collection_length(self.coll)

        cdef DOMNodeCollection slice_coll
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

            slice_coll = DOMNodeCollection.__new__(DOMNodeCollection)
            slice_coll.coll = dom_coll
            return slice_coll

        if type(key) is not int:
            raise TypeError(f'Invalid key type: {type(key)}')

        if key >= coll_len:
            raise IndexError('Index out of range')

        return _node_from_dom(<lxb_dom_node_t*>lxb_dom_collection_element(self.coll, self._wrap_idx(key)))

    def __repr__(self):
        return f'{{{", ".join(str(n) for n in self)}}}'

    def __str__(self):
        return self.__repr__()


cdef class HTMLTree:
    """
    __init__(self)

    HTML DOM tree parser.
    """
    def __cinit__(self):
        self.document = lxb_html_document_create()
        if self.document == NULL:
            raise RuntimeError('Failed to allocate HTML document')

    def __dealloc__(self):
        if self.document != NULL:
            lxb_html_document_destroy(self.document)

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
        
        :param document: input byte string
        :param encoding: encoding for decoding byte string
        :param errors: decoding error policy (same as ``str.decode()``)
        :raises ValueError: if HTML parsing fails for unknown reasons
        """
        encoding = map_encoding_to_html5(encoding)
        if encoding != 'utf-8':
            document = bytes_to_str(document, encoding, errors).encode('utf-8')
        cdef lxb_status_t status = lxb_html_document_parse(self.document, <const lxb_char_t*>document, len(document))
        if status != LXB_STATUS_OK:
            raise ValueError('Failed to parse HTML document')

    @property
    def root(self):
        """
        Document root element.

        :rtype: DOMNode | None
        """
        if self.document == NULL:
            return None

        return _node_from_dom(<lxb_dom_node_t*>&self.document.dom_document.node)

    @property
    def head(self):
        """
        HTML head element or ``None`` if document has no head.

        :rtype: DOMNode | None
        """
        if self.document == NULL:
            return None

        return _node_from_dom(<lxb_dom_node_t*>lxb_html_document_head_element(self.document))

    @property
    def body(self):
        """
        HTML body element or ``None`` if document has no body.

        :rtype: DOMNode | None
        """
        if self.document == NULL:
            return None

        return _node_from_dom(<lxb_dom_node_t*>lxb_html_document_body_element(self.document))
