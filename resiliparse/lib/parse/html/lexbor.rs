// Copyright 2023 Janek Bevendorff
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.


use std::slice;
use std::ptr::{addr_of, addr_of_mut};
use crate::third_party::lexbor::*;


// ----------------------------------------- Lexbor Helpers ----------------------------------------


pub unsafe fn str_from_lxb_char_t<'a>(cdata: *const lxb_char_t, size: usize) -> Option<&'a str> {
    if !cdata.is_null() {
        Some(std::str::from_utf8_unchecked(slice::from_raw_parts(cdata, size)))
    } else {
        None
    }
}

#[inline]
pub unsafe fn str_from_lxb_str_t<'a>(s: *const lexbor_str_t) -> Option<&'a str> {
    str_from_lxb_char_t((*s).data, (*s).length)
}

#[inline]
pub unsafe fn str_from_dom_node<'a>(node: *const lxb_dom_node_t) -> Option<&'a str> {
    let cdata = node as *const lxb_dom_character_data_t;
    str_from_lxb_str_t(addr_of!((*cdata).data))
}

pub unsafe fn str_from_lxb_str_cb<'a, Node, Fn>(
    node: *mut Node, lxb_fn: unsafe extern "C" fn(*mut Fn, *mut usize) -> *const lxb_char_t) -> Option<&'a str> {
    if node.is_null() {
        return None;
    }
    let mut size = 0;
    let name = lxb_fn(node.cast(), addr_of_mut!(size));
    if name.is_null() {
        None
    } else {
        str_from_lxb_char_t(name, size)
    }
}
