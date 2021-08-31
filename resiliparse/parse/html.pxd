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

from resiliparse_inc.lexbor cimport lxb_html_document_t, lxb_dom_node_t, lxb_dom_attr_t

cdef class Node:
    cdef lxb_dom_node_t* node
    cpdef bint hasattr(self, str attr_name)
    cpdef getattr(self, str attr_name, default_value=*)
    cdef Attribute _getattr_impl(self, str attr_name)

cdef class Attribute:
    cdef lxb_dom_attr_t* attr


# noinspection DuplicatedCode
cdef enum NodeType:
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

    cpdef void parse(self, str document)
    cpdef void parse_from_bytes(self, bytes document, str encoding=*, str errors=*)
