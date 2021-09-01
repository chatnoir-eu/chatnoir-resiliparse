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

from resiliparse_inc.lexbor cimport lxb_html_document_t, lxb_dom_node_t, lxb_dom_attr_t, lxb_dom_collection_t, \
    lxb_css_parser_t, lxb_selectors_t, lxb_css_selectors_t

cdef class DOMAttribute:
    cdef DOMNode node
    cdef lxb_dom_attr_t* attr


cdef class DOMNode:
    cdef HTMLTree tree
    cdef lxb_dom_node_t* node

    cpdef bint hasattr(self, str attr_name)
    cpdef getattr(self, str attr_name, default_value=*)
    cdef DOMAttribute _getattr_impl(self, str attr_name)

    cdef lxb_dom_collection_t* _match_by_attr(self, bytes attr_name, bytes attr_value, size_t init_size=*,
                                              bint case_insensitive=*)
    cpdef DOMNode get_element_by_id(self, str element_id, bint case_insensitive=*)
    cpdef DOMNodeCollection get_elements_by_class_name(self, str element_class, bint case_insensitive=*)
    cpdef DOMNodeCollection get_elements_by_tag_name(self, str tag_name)

    cdef lxb_dom_collection_t * _match_by_selector(self, bytes selector, size_t init_size=*)
    cpdef DOMNode query_selector(self, str selector)
    cpdef DOMNodeCollection query_selector_all(self, str selector)
    cpdef bint matches_any(self, str selector)


cdef class DOMNodeCollection:
    cdef HTMLTree tree
    cdef lxb_dom_collection_t* coll

    cdef inline size_t _wrap_idx(self, ssize_t idx)


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
    cdef lxb_html_document_t* document
    cdef str encoding
    cdef lxb_css_parser_t* css_parser
    cdef lxb_selectors_t* selectors
    cdef lxb_css_selectors_t* css_selectors

    cpdef void parse(self, str document)
    cpdef void parse_from_bytes(self, bytes document, str encoding=*, str errors=*)

    cdef void init_css_parser(self)
