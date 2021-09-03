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

from resiliparse_inc.lexbor cimport lxb_html_document_t, lxb_dom_node_t, lxb_dom_collection_t, \
    lxb_css_parser_t, lxb_selectors_t, lxb_css_selectors_t

cdef lxb_dom_collection_t* get_elements_by_attr_impl(lxb_dom_node_t* node, bytes attr_name, bytes attr_value,
                                                     size_t init_size=*, bint case_insensitive=*)
cdef lxb_dom_collection_t* get_elements_by_tag_name_impl(lxb_dom_node_t* node, bytes tag_name)
cdef lxb_dom_collection_t* query_selector_impl(lxb_dom_node_t* node, HTMLTree tree, bytes selector,
                                               size_t init_size=*)
cdef bint matches_any_impl(lxb_dom_node_t* node, HTMLTree tree, bytes selector)

cdef class DOMNode:
    cdef HTMLTree tree
    cdef lxb_dom_node_t* node

    cpdef bint hasattr(self, str attr_name)
    cpdef str getattr(self, str attr_name, str default_value=*)
    cdef str _getattr_impl(self, str attr_name)

    cpdef DOMNode get_element_by_id(self, str element_id, bint case_insensitive=*)
    cpdef DOMCollection get_elements_by_attr(self, str attr_name, str attr_value, bint case_insensitive=*)
    cpdef DOMCollection get_elements_by_class_name(self, str class_name, bint case_insensitive=*)
    cpdef DOMCollection get_elements_by_tag_name(self, str tag_name)

    cpdef DOMNode query_selector(self, str selector)
    cpdef DOMCollection query_selector_all(self, str selector)
    cpdef bint matches_any(self, str selector)

    cpdef DOMNode append_child(self, DOMNode node)
    cpdef DOMNode insert_before(self, DOMNode node, DOMNode reference)
    cpdef DOMNode replace_child(self, DOMNode new_child, DOMNode old_child)
    cpdef DOMNode remove_child(self, DOMNode node)
    cpdef void decompose(self)


cdef class DOMCollection:
    cdef HTMLTree tree
    cdef lxb_dom_collection_t* coll

    cdef inline size_t _wrap_idx(self, ssize_t idx)
    cdef _forward_element_match(self, bytes func, attrs, bint single)

    cpdef DOMNode get_element_by_id(self, str element_id, bint case_insensitive=*)
    cpdef DOMCollection get_elements_by_attr(self, str attr_name, str attr_value, bint case_insensitive=*)
    cpdef DOMCollection get_elements_by_class_name(self, str class_name, bint case_insensitive=*)
    cpdef DOMCollection get_elements_by_tag_name(self, str tag_name)

    cpdef DOMNode query_selector(self, str selector)
    cpdef DOMCollection query_selector_all(self, str selector)
    cpdef bint matches_any(self, str selector)


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
    LAST_ENTRY = 0x0D


cdef class HTMLTree:
    cdef lxb_html_document_t* dom_document
    cdef str encoding
    cdef lxb_css_parser_t* css_parser
    cdef lxb_selectors_t* selectors
    cdef lxb_css_selectors_t* css_selectors

    cpdef void parse(self, str document)
    cpdef void parse_from_bytes(self, bytes document, str encoding=*, str errors=*)

    cpdef create_element(self, str tag_name)
    cpdef create_text_node(self, str text)

    cdef void init_css_parser(self)
