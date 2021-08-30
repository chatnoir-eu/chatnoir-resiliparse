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

from libcpp.string cimport string
from resiliparse_inc.lexbor cimport *

from resiliparse.parse.encoding cimport bytes_to_str


cdef class Node:
    cdef str encoding
    cdef lxb_dom_element_t* element

    def __cinit__(self, str encoding='utf-8'):
        self.encoding = encoding
        self.element = NULL

    @property
    def text(self):
        if self.element == NULL:
            return None
        cdef size_t text_len = 0
        cdef lxb_char_t* text = lxb_dom_node_text_content(&self.element.node, &text_len)
        return bytes_to_str(text[:text_len], self.encoding)


cdef class HTMLTree:
    cdef lxb_html_document_t* document
    cdef str encoding

    def __cinit__(self, str encoding='utf-8'):
        self.encoding = encoding
        self.document = lxb_html_document_create()
        if self.document == NULL:
            raise RuntimeError('Failed to allocate HTML document')

    def __dealloc__(self):
        if self.document != NULL:
            lxb_html_document_destroy(self.document)

    cpdef parse(self, str document):
        cdef string document_bytes = document.encode('utf-8')
        self.parse_from_bytes(document_bytes)

    cpdef parse_from_bytes(self, const string& document):
        cdef lxb_status_t status = lxb_html_document_parse(self.document,
                                                           <const lxb_char_t*>document.data(), document.size())
        if status != LXB_STATUS_OK:
            raise ValueError('Failed to parse HTML document')

    @property
    def head(self):
        if self.document == NULL:
            return None

        cdef lxb_html_head_element_t* head = lxb_html_document_head_element(self.document)
        if head == NULL:
            return None
        cdef Node node = Node.__new__(Node, self.encoding)
        node.element = <lxb_dom_element_t*>head
        return node

    @property
    def body(self):
        if self.document == NULL:
            return None

        cdef lxb_html_body_element_t* body = lxb_html_document_body_element(self.document)
        if body == NULL:
            return None
        cdef Node node = Node.__new__(Node, self.encoding)
        node.element = <lxb_dom_element_t*>body
        return node
