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

from libcpp.string cimport string
from resiliparse_inc.string_view cimport string_view
from resiliparse_inc.lexbor cimport *


cdef inline bint check_node(DOMNode node) noexcept nogil:
    """Check whether node is initialized and valid."""
    return node is not None and node.tree is not None and node.node != NULL

cdef void create_css_parser(lxb_css_memory_t** memory, lxb_css_parser_t** parser) noexcept nogil
cdef void destroy_css_parser(lxb_css_memory_t* memory, lxb_css_parser_t* parser) noexcept nogil
cdef void create_css_selectors(lxb_css_parser_t* parser) noexcept nogil
cdef void destroy_css_selectors(lxb_css_parser_t* parser) noexcept nogil
cdef lxb_css_selector_list_t* parse_css_selectors(lxb_css_parser_t* css_parser, const lxb_char_t* selector,
                                                  size_t selector_len) except NULL nogil

cdef lxb_dom_node_t* next_node(const lxb_dom_node_t* root_node, lxb_dom_node_t* node,
                               size_t* depth=*, bint* end_tag=*) noexcept nogil

cdef inline lxb_dom_node_t* next_element_node(const lxb_dom_node_t* root_node, lxb_dom_node_t* node,
                                              size_t* depth=NULL, bint* end_tag=NULL) noexcept nogil:
    node = next_node(root_node, node, depth, end_tag)
    while node and node.type != LXB_DOM_NODE_TYPE_ELEMENT:
        node = next_node(root_node, node, depth, end_tag)
    return node

cdef inline string_view get_node_attr_sv(lxb_dom_node_t* node, const string& attr) noexcept nogil:
    """Get node attribute value as string_view."""
    cdef size_t node_attr_len
    cdef const lxb_char_t* node_attr_data = lxb_dom_element_get_attribute(
        <lxb_dom_element_t*>node, <lxb_char_t*>attr.data(), attr.size(), &node_attr_len)
    return string_view(<const char*>node_attr_data, node_attr_len)

cdef string get_node_text(lxb_dom_node_t* node) noexcept nogil

cdef lxb_dom_node_t* get_element_by_id_impl(lxb_dom_node_t* node,
                                            const char* id_value, size_t id_value_len,
                                            bint case_insensitive=*) noexcept nogil
cdef lxb_dom_collection_t* get_elements_by_attr_impl(lxb_dom_node_t* node,
                                                     const char* attr_name, size_t attr_name_len,
                                                     const char* attr_value, size_t attr_value_len,
                                                     size_t init_size=*, bint case_insensitive=*) noexcept nogil
cdef lxb_dom_collection_t* get_elements_by_class_name_impl(lxb_dom_node_t* node, const char* class_name,
                                                           size_t class_name_len, size_t init_size=*) noexcept nogil
cdef lxb_dom_collection_t* get_elements_by_tag_name_impl(lxb_dom_node_t* node,
                                                         const char* tag_name, size_t tag_name_len) noexcept nogil
cdef lxb_dom_node_t* query_selector_impl(lxb_dom_node_t* node, HTMLTree tree,
                                         const char* selector, size_t select_len) except <lxb_dom_node_t*>-1 nogil
cdef lxb_dom_collection_t* query_selector_all_impl(lxb_dom_node_t* node, HTMLTree tree,
                                                   const char* selector, size_t selector_len,
                                                   size_t init_size=*) except <lxb_dom_collection_t*>-1 nogil
cdef bint matches_impl(lxb_dom_node_t* node, HTMLTree tree, const char* selector, size_t selector_len) noexcept nogil

cdef extern from "html.h" nogil:
    cdef lxb_tag_id_t BLOCK_ELEMENTS[]
    cdef size_t NUM_BLOCK_ELEMENTS


cdef class DOMElementClassList:
    cdef DOMNode node

    cdef list _create_list(self)
    cdef inline bytes _class_name_bytes(self)
    cpdef void add(self, str class_name)
    cpdef void remove(self, str class_name)


cdef class DOMNode:
    cdef HTMLTree tree
    cdef lxb_dom_node_t* node
    cdef DOMElementClassList class_list_singleton

    cpdef bint hasattr(self, str attr_name) except -1
    cdef bint _getattr_impl(self, const char* attr_name, size_t attr_name_len,
                            const char** attr_out_value, size_t* attr_out_len) except -1 nogil
    cpdef str getattr(self, str attr_name, str default_value=*)
    cdef bint _setattr_impl(self, const char* attr_name, size_t attr_name_len,
                            const char* attr_value, size_t attr_value_len) except -1 nogil
    cpdef setattr(self, str attr_name, str attr_value)
    cdef bint _delattr_impl(self, const char* attr_name, size_t attr_name_len) except -1 nogil
    cpdef delattr(self, str attr_name)

    cpdef DOMNode get_element_by_id(self, str element_id, bint case_insensitive=*)
    cpdef DOMCollection get_elements_by_attr(self, str attr_name, str attr_value, bint case_insensitive=*)
    cpdef DOMCollection get_elements_by_class_name(self, str class_name, bint case_insensitive=*)
    cpdef DOMCollection get_elements_by_tag_name(self, str tag_name)

    cpdef DOMNode query_selector(self, str selector)
    cpdef DOMCollection query_selector_all(self, str selector)
    cpdef bint matches(self, str selector) except -1

    cpdef DOMNode append_child(self, DOMNode node)
    cpdef DOMNode insert_before(self, DOMNode node, DOMNode reference)
    cpdef DOMNode replace_child(self, DOMNode new_child, DOMNode old_child)
    cpdef DOMNode remove_child(self, DOMNode node)
    cpdef decompose(self)


cdef class DOMCollection:
    cdef HTMLTree tree
    cdef lxb_dom_collection_t* coll

    cdef inline size_t _wrap_idx(self, ssize_t idx)
    cdef DOMCollection _forward_collection_match(self, bytes func, attrs)

    cpdef DOMNode get_element_by_id(self, str element_id, bint case_insensitive=*)
    cpdef DOMCollection get_elements_by_attr(self, str attr_name, str attr_value, bint case_insensitive=*)
    cpdef DOMCollection get_elements_by_class_name(self, str class_name, bint case_insensitive=*)
    cpdef DOMCollection get_elements_by_tag_name(self, str tag_name)

    cpdef DOMNode query_selector(self, str selector)
    cpdef DOMCollection query_selector_all(self, str selector)
    cpdef bint matches(self, str selector) except -1


# noinspection DuplicatedCode
cpdef enum NodeType:
    ELEMENT = 0x01,
    ATTRIBUTE = 0x02,
    TEXT = 0x03,
    CDATA_SECTION = 0x04,
    ENTITY_REFERENCE = 0x05,
    ENTITY = 0x06,
    PROCESSING_INSTRUCTION = 0x07,
    COMMENT = 0x08,
    DOCUMENT = 0x09,
    DOCUMENT_TYPE = 0x0A,
    DOCUMENT_FRAGMENT = 0x0B,
    NOTATION = 0x0C,


cdef HTMLTree create_html_tree(bytes document, bint reencode=*, str encoding=*, str errors=*)

cdef class HTMLTree:
    cdef lxb_html_document_t* dom_document
    cdef str encoding
    cdef lxb_css_parser_t* css_parser
    cdef lxb_css_memory_t* css_memory

    cpdef DOMNode create_element(self, str tag_name)
    cpdef DOMNode create_text_node(self, str text)

    cdef void init_css_parser(self) noexcept nogil

cdef bint is_block_element(lxb_tag_id_t tag_id) noexcept nogil
