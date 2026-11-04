// Copyright 2026 Janek Bevendorff
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

use crate::third_party::lexbor::lxb_ns_id_enum_t::*;
use crate::third_party::lexbor::lxb_tag_id_enum_t::LXB_TAG__LAST_ENTRY;
use crate::third_party::lexbor::*;
use rstest::rstest;

#[rstest]
#[case::node(lxb_dom_node_interface_create, lxb_dom_node_interface_destroy)]
#[case::attr(lxb_dom_attr_interface_create, lxb_dom_attr_interface_destroy)]
#[case::character_data(lxb_dom_character_data_interface_create, lxb_dom_character_data_interface_destroy)]
#[case::text(lxb_dom_text_interface_create, lxb_dom_text_interface_destroy)]
#[case::cdata_section(lxb_dom_cdata_section_interface_create, lxb_dom_cdata_section_interface_destroy)]
#[case::comment(lxb_dom_comment_interface_create, lxb_dom_comment_interface_destroy)]
#[case::processing_instruction(
    lxb_dom_processing_instruction_interface_create,
    lxb_dom_processing_instruction_interface_destroy
)]
#[case::document(lxb_dom_document_interface_create, lxb_dom_document_interface_destroy)]
#[case::document_type(lxb_dom_document_type_interface_create, lxb_dom_document_type_interface_destroy)]
#[case::document_fragment(
    lxb_dom_document_fragment_interface_create,
    lxb_dom_document_fragment_interface_destroy
)]
#[case::shadow_root(lxb_dom_shadow_root_interface_create, lxb_dom_shadow_root_interface_destroy)]
#[case::element(lxb_dom_element_interface_create, lxb_dom_element_interface_destroy)]
fn dom_interface_reference_counts<T>(
    #[case] create: unsafe extern "C" fn(*mut lxb_dom_document_t) -> *mut T,
    #[case] destroy: unsafe extern "C" fn(*mut T) -> *mut T,
) {
    unsafe {
        let document = lxb_html_document_create();
        assert!(!document.is_null());
        let doc = document.cast::<lxb_dom_document_t>();
        let allocations = (*(*doc).mraw).ref_count;
        let interface = create(document.cast());
        assert!(!interface.is_null());
        let node = interface.cast::<lxb_dom_node_t>();
        assert_eq!((*node).ref_count, 1);
        assert_eq!((*node).owner_document, doc);

        // A concrete destructor must preserve a retained interface and return its pointer.
        (*node).ref_count += 2;
        assert_eq!(destroy(interface), interface);
        assert_eq!((*node).ref_count, 2);
        // The generic dispatcher must honour the same contract, including uncommon node kinds.
        assert_eq!(lxb_dom_interface_destroy(interface.cast()), interface.cast());
        assert_eq!((*node).ref_count, 1);
        assert!(lxb_dom_interface_destroy(interface.cast()).is_null());
        assert_eq!((*(*doc).mraw).ref_count, allocations);
        assert_eq!((*doc).node.ref_count, 1);
        lxb_html_document_destroy(document);
    }
}

#[rstest]
fn html_interface_reference_counts(
    #[values(
        LXB_NS__UNDEF,
        LXB_NS__ANY,
        LXB_NS_HTML,
        LXB_NS_MATH,
        LXB_NS_SVG,
        LXB_NS_XLINK,
        LXB_NS_XML,
        LXB_NS_XMLNS
    )]
    namespace: lxb_ns_id_enum_t::Type,
) {
    unsafe {
        let document = lxb_html_document_create();
        assert!(!document.is_null());
        // Walk the generated dispatch table so new upstream tags cannot escape this check.
        for tag in 0..LXB_TAG__LAST_ENTRY {
            let node = lxb_html_interface_create(document, tag as usize, namespace as usize).cast::<lxb_dom_node_t>();
            assert!(!node.is_null(), "tag {tag}");
            assert_eq!((*node).ref_count, 1, "tag {tag}");
            (*node).ref_count += 1;
            // Non-element constructors still need their text/identifier fields initialised before cloning.
            if (*node).type_ == lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_ELEMENT {
                let cloned = lxb_html_interface_clone(document.cast(), node.cast()).cast::<lxb_dom_node_t>();
                assert!(!cloned.is_null(), "tag {tag}");
                assert_eq!((*cloned).ref_count, 1, "tag {tag}");
                assert!(lxb_html_interface_destroy(cloned.cast()).is_null(), "tag {tag}");
            }
            assert_eq!((*node).ref_count, 2, "tag {tag}");
            assert_eq!(lxb_html_interface_destroy(node.cast()), node.cast(), "tag {tag}");
            assert_eq!((*node).ref_count, 1, "tag {tag}");
            assert!(lxb_html_interface_destroy(node.cast()).is_null(), "tag {tag}");
        }
        lxb_html_document_destroy(document);
    }
}

#[test]
fn cloned_processing_instruction_reference_counts() {
    unsafe {
        let document = lxb_html_document_create();
        assert!(!document.is_null());
        let node =
            lxb_dom_document_create_processing_instruction(document.cast(), b"pi".as_ptr(), 2, b"data".as_ptr(), 4)
                .cast::<lxb_dom_node_t>();
        assert!(!node.is_null());
        (*node).ref_count += 1;
        let cloned = lxb_html_interface_clone(document.cast(), node.cast()).cast::<lxb_dom_node_t>();
        assert!(!cloned.is_null());
        assert_eq!((*cloned).ref_count, 1);
        assert_eq!((*node).ref_count, 2);
        assert!(lxb_html_interface_destroy(cloned.cast()).is_null());
        assert_eq!(lxb_html_interface_destroy(node.cast()), node.cast());
        assert!(lxb_html_interface_destroy(node.cast()).is_null());
        lxb_html_document_destroy(document);
    }
}

#[test]
fn non_element_html_dispatch() {
    unsafe {
        let document = lxb_html_document_create();
        assert!(!document.is_null());
        let node = lxb_dom_shadow_root_interface_create(document.cast()).cast::<lxb_dom_node_t>();
        assert!(!node.is_null());
        (*node).ref_count += 1;
        assert_eq!(lxb_html_interface_destroy(node.cast()), node.cast());
        assert_eq!((*node).ref_count, 1);
        assert!(lxb_html_interface_destroy(node.cast()).is_null());
        lxb_html_document_destroy(document);
    }
}

#[test]
fn retained_html_document_keeps_parser() {
    unsafe {
        let document = lxb_html_document_create();
        assert!(!document.is_null());
        let html = b"<p>first</p>";
        assert_eq!(lxb_html_document_parse(document, html.as_ptr(), html.len()), 0);
        let parser = (*document).dom_document.parser.cast::<lxb_html_parser_t>();
        assert!(!parser.is_null());
        assert_eq!((*parser).ref_count, 1);
        assert_eq!(lxb_html_parser_ref(parser), parser);
        (*document).dom_document.node.ref_count += 1;
        assert_eq!(lxb_html_document_destroy(document), document);
        assert_eq!((*document).dom_document.parser, parser.cast());
        assert_eq!((*parser).ref_count, 2);
        assert!(!(*document).body.is_null());
        assert_eq!((*document).dom_document.node.ref_count, 1);
        assert!(lxb_html_document_destroy(document).is_null());
        assert_eq!((*parser).ref_count, 1);
        assert!(lxb_html_parser_unref(parser).is_null());
        assert!(lxb_html_document_destroy(std::ptr::null_mut()).is_null());
    }
}

#[test]
fn window_reference_counts() {
    unsafe {
        let document = lxb_html_document_create();
        assert!(!document.is_null());
        let window = lxb_html_window_create(document);
        assert!(!window.is_null());
        let node = window.cast::<lxb_dom_node_t>();
        // Window construction writes a full node header, so its allocation must contain one.
        assert!(size_of::<lxb_html_window_t>() >= size_of::<lxb_dom_node_t>());
        assert_eq!((*node).ref_count, 1);
        (*node).ref_count += 1;
        assert_eq!(lxb_html_window_destroy(window), window);
        assert_eq!((*node).ref_count, 1);
        assert!(lxb_html_window_destroy(window).is_null());
        lxb_html_document_destroy(document);
    }
}

#[test]
fn event_target_has_no_node_header() {
    unsafe {
        let document = lxb_html_document_create();
        assert!(!document.is_null());
        let target = lxb_dom_event_target_create(document.cast());
        assert!(!target.is_null());
        // EventTarget is a base interface, not a node; construction must fit its own allocation.
        assert_eq!(size_of::<lxb_dom_event_target_t>(), size_of::<*mut std::ffi::c_void>());
        assert!((*target).events.is_null());
        assert!(lxb_dom_event_target_destroy(target, document.cast()).is_null());
        lxb_html_document_destroy(document);
    }
}

#[rstest]
#[case::character_data(lxb_dom_character_data_interface_create)]
#[case::text(lxb_dom_text_interface_create)]
#[case::cdata_section(lxb_dom_cdata_section_interface_create)]
#[case::comment(lxb_dom_comment_interface_create)]
#[case::processing_instruction(lxb_dom_processing_instruction_interface_create)]
fn character_data_lifetime<T>(#[case] create: unsafe extern "C" fn(*mut lxb_dom_document_t) -> *mut T) {
    unsafe {
        let document = lxb_html_document_create();
        assert!(!document.is_null());
        let doc = document.cast::<lxb_dom_document_t>();
        let allocations = (*(*doc).mraw).ref_count;
        let text_allocations = (*(*doc).text).ref_count;
        let data = create(doc).cast::<lxb_dom_character_data_t>();
        assert!(!data.is_null());
        assert!(!lexbor_str_init(&mut (*data).data, (*doc).text, 7).is_null());
        assert!(!lexbor_str_append(&mut (*data).data, (*doc).text, b"payload".as_ptr(), 7).is_null());

        // A retained node keeps its text allocation until the final release.
        (*data).node.ref_count += 1;
        assert_eq!(lxb_dom_interface_destroy(data.cast()), data.cast());
        assert_eq!((*data).node.ref_count, 1);
        assert_eq!(super::str_from_lxb_str_t(&(*data).data), Some("payload"));
        assert_eq!((*(*doc).text).ref_count, text_allocations + 1);
        assert!(lxb_dom_interface_destroy(data.cast()).is_null());
        assert_eq!((*(*doc).text).ref_count, text_allocations);
        assert_eq!((*(*doc).mraw).ref_count, allocations);
        assert!(lxb_html_document_destroy(document).is_null());
    }
}

#[test]
fn processing_instruction_strings_lifetime() {
    unsafe {
        let document = lxb_html_document_create();
        assert!(!document.is_null());
        let doc = document.cast::<lxb_dom_document_t>();
        let allocations = (*(*doc).mraw).ref_count;
        let text_allocations = (*(*doc).text).ref_count;
        let instruction =
            lxb_dom_document_create_processing_instruction(doc, b"target".as_ptr(), 6, b"data".as_ptr(), 4);
        assert!(!instruction.is_null());

        // Both the target and character data survive an intermediate release.
        (*instruction).char_data.node.ref_count += 1;
        assert_eq!(lxb_dom_interface_destroy(instruction.cast()), instruction.cast());
        assert_eq!(super::str_from_lxb_str_t(&(*instruction).target), Some("target"));
        assert_eq!(super::str_from_lxb_str_t(&(*instruction).char_data.data), Some("data"));
        assert!(lxb_dom_interface_destroy(instruction.cast()).is_null());
        assert_eq!((*(*doc).mraw).ref_count, allocations);
        assert_eq!((*(*doc).text).ref_count, text_allocations);
        assert!(lxb_html_document_destroy(document).is_null());
    }
}

#[test]
fn doctype_strings_lifetime() {
    unsafe {
        let document = lxb_html_document_create();
        assert!(!document.is_null());
        let doc = document.cast::<lxb_dom_document_t>();
        let allocations = (*(*doc).mraw).ref_count;
        let text_allocations = (*(*doc).text).ref_count;
        let mut code = 0;
        let doctype = lxb_dom_document_type_create(
            doc,
            b"html".as_ptr(),
            4,
            b"public".as_ptr(),
            6,
            b"system".as_ptr(),
            6,
            &mut code,
        );
        assert!(!doctype.is_null());
        assert_eq!(code, lxb_dom_exception_code_t::LXB_DOM_EXCEPTION_OK);

        // Public and system identifiers are freed only with the last reference.
        (*doctype).node.ref_count += 1;
        assert_eq!(lxb_dom_interface_destroy(doctype.cast()), doctype.cast());
        assert_eq!(super::str_from_lxb_str_t(&(*doctype).public_id), Some("public"));
        assert_eq!(super::str_from_lxb_str_t(&(*doctype).system_id), Some("system"));
        assert!(lxb_dom_interface_destroy(doctype.cast()).is_null());
        assert_eq!((*(*doc).mraw).ref_count, allocations);
        assert_eq!((*(*doc).text).ref_count, text_allocations);
        assert!(lxb_html_document_destroy(document).is_null());
    }
}

#[test]
fn element_attributes_lifetime() {
    unsafe {
        let document = lxb_html_document_create();
        assert!(!document.is_null());
        let doc = document.cast::<lxb_dom_document_t>();
        let allocations = (*(*doc).mraw).ref_count;
        let text_allocations = (*(*doc).text).ref_count;
        let element = lxb_dom_element_interface_create(doc);
        assert!(!element.is_null());
        let attr = lxb_dom_element_set_attribute(element, b"id".as_ptr(), 2, b"value".as_ptr(), 5);
        assert!(!attr.is_null());
        assert_eq!((*attr).node.ref_count, 1);

        (*element).node.ref_count += 1;
        assert_eq!(lxb_dom_interface_destroy(element.cast()), element.cast());
        assert_eq!((*element).node.ref_count, 1);
        assert_eq!((*element).first_attr, attr);
        assert_eq!((*attr).owner, element);
        assert_eq!((*attr).node.ref_count, 1);
        assert!(!(*attr).value.is_null());
        assert_eq!(super::str_from_lxb_str_t((*attr).value), Some("value"));

        // An independently retained attribute outlives its element.
        (*attr).node.ref_count += 1;
        assert!(lxb_dom_interface_destroy(element.cast()).is_null());
        assert_eq!((*attr).node.ref_count, 1);
        assert_eq!(super::str_from_lxb_str_t((*attr).value), Some("value"));
        assert!(lxb_dom_interface_destroy(attr.cast()).is_null());
        assert_eq!((*(*doc).mraw).ref_count, allocations);
        assert_eq!((*(*doc).text).ref_count, text_allocations);
        assert!(lxb_html_document_destroy(document).is_null());
    }
}

#[test]
fn cloned_text_lifetime() {
    unsafe {
        let document = lxb_html_document_create();
        assert!(!document.is_null());
        let text = lxb_dom_document_create_text_node(document.cast(), b"data".as_ptr(), 4);
        assert!(!text.is_null());
        (*text).char_data.node.ref_count += 2;
        let cloned = lxb_dom_interface_clone(std::ptr::null_mut(), text.cast()).cast::<lxb_dom_text_t>();
        assert!(!cloned.is_null());
        assert_ne!(cloned, text);
        assert_eq!((*cloned).char_data.node.ref_count, 1);
        assert_eq!((*text).char_data.node.ref_count, 3);

        // Destroying the clone must preserve the source text and its references.
        assert!(lxb_dom_interface_destroy(cloned.cast()).is_null());
        assert_eq!(super::str_from_lxb_str_t(&(*text).char_data.data), Some("data"));
        assert_eq!(lxb_dom_interface_destroy(text.cast()), text.cast());
        assert_eq!(lxb_dom_interface_destroy(text.cast()), text.cast());
        assert!(lxb_dom_interface_destroy(text.cast()).is_null());
        assert!(lxb_html_document_destroy(document).is_null());
    }
}

#[test]
fn derived_document_lifetime() {
    unsafe {
        let doc = lxb_dom_document_create(std::ptr::null_mut());
        assert!(!doc.is_null());
        assert_eq!(
            lxb_dom_document_init(
                doc,
                std::ptr::null_mut(),
                Some(lxb_dom_interface_create),
                Some(lxb_dom_interface_clone),
                Some(lxb_dom_interface_destroy),
                lxb_dom_document_dtype_t::LXB_DOM_DOCUMENT_DTYPE_UNDEF,
                LXB_NS__UNDEF,
            ),
            lexbor_status_t::LXB_STATUS_OK,
        );
        assert_eq!((*doc).node.ref_count, 1);
        (*doc).node.ref_count += 1;
        assert_eq!(lxb_dom_document_destroy(doc), doc);
        assert_eq!((*doc).node.ref_count, 1);

        // Releasing a derived document must not release its owner's allocation pool.
        let derived = lxb_dom_document_interface_create(doc);
        assert!(!derived.is_null());
        assert_eq!((*derived).node.ref_count, 1);
        (*derived).node.ref_count += 1;
        assert_eq!(lxb_dom_document_destroy(derived), derived);
        assert_eq!((*derived).node.ref_count, 1);
        assert!(lxb_dom_document_destroy(derived).is_null());
        assert_eq!((*doc).node.ref_count, 1);
        assert_eq!((*(*doc).mraw).ref_count, 0);
        assert!(lxb_dom_document_destroy(doc).is_null());
        assert!(lxb_dom_document_destroy(std::ptr::null_mut()).is_null());
        assert!(lxb_dom_interface_destroy(std::ptr::null_mut()).is_null());
    }
}

#[test]
fn template_content_lifetime() {
    unsafe {
        let document = lxb_html_document_create();
        assert!(!document.is_null());
        let doc = document.cast::<lxb_dom_document_t>();
        let allocations = (*(*doc).mraw).ref_count;
        let element =
            lxb_html_interface_create(document, lxb_tag_id_enum_t::LXB_TAG_TEMPLATE as usize, LXB_NS_HTML as usize)
                .cast::<lxb_html_template_element_t>();
        assert!(!element.is_null());
        let node = element.cast::<lxb_dom_node_t>();
        assert_eq!((*node).ref_count, 1);
        let content = (*element).content;
        assert!(!content.is_null());
        assert_eq!((*content).node.ref_count, 1);

        // Retaining the template also keeps its separately allocated content fragment alive.
        (*node).ref_count += 1;
        assert_eq!(lxb_html_interface_destroy(element.cast()), element.cast());
        assert_eq!((*element).content, content);
        assert_eq!((*content).node.ref_count, 1);
        assert_eq!((*(*doc).mraw).ref_count, allocations + 2);
        assert!(lxb_html_interface_destroy(element.cast()).is_null());
        assert_eq!((*(*doc).mraw).ref_count, allocations);
        assert!(lxb_html_document_destroy(document).is_null());
    }
}
