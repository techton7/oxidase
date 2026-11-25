use oxidase::dom::{Document, Element};

#[cfg(not(feature = "native"))]
#[test]
fn test_mock_computed_z_index_hooks_and_element() {
    let doc = Document::mock();

    // ElementTarget::Mock always returns None
    assert_eq!(Element::mock().computed_z_index(), None);

    // Unregistered element returns None
    doc.clear_computed_z_index("zix-missing");
    assert_eq!(doc.computed_z_index_by_id("zix-missing"), None);

    // Integer, zero, negative, and "auto" values are preserved verbatim
    doc.set_computed_z_index("zix-50", "50");
    doc.set_computed_z_index("zix-0", "0");
    doc.set_computed_z_index("zix-neg", "-1");
    doc.set_computed_z_index("zix-auto", "auto");
    doc.set_computed_z_index("zix-trim", "  75  ");
    doc.set_computed_z_index("zix-empty", "   ");

    assert_eq!(doc.computed_z_index_by_id("zix-50").as_deref(), Some("50"));
    assert_eq!(doc.computed_z_index_by_id("zix-0").as_deref(), Some("0"));
    assert_eq!(
        doc.computed_z_index_by_id("zix-neg").as_deref(),
        Some("-1")
    );
    assert_eq!(
        doc.computed_z_index_by_id("zix-auto").as_deref(),
        Some("auto")
    );
    assert_eq!(
        doc.computed_z_index_by_id("zix-trim").as_deref(),
        Some("75")
    );
    assert_eq!(doc.computed_z_index_by_id("zix-empty"), None);

    // Clearing one ID removes only that entry
    doc.clear_computed_z_index("zix-50");
    assert_eq!(doc.computed_z_index_by_id("zix-50"), None);
    assert_eq!(doc.computed_z_index_by_id("zix-0").as_deref(), Some("0"));

    // Clean up thread-local mock entries
    for id in ["zix-0", "zix-neg", "zix-auto", "zix-trim", "zix-empty"] {
        doc.clear_computed_z_index(id);
    }
    assert_eq!(doc.computed_z_index_by_id("zix-0"), None);
}

#[cfg(feature = "native")]
mod native_tests {
    use super::*;
    use blitz_dom::{Attribute, BaseDocument, DocumentConfig, LocalName, QualName, ns};
    use std::cell::RefCell;
    use std::rc::Rc;

    fn qname(tag: &'static str) -> QualName {
        QualName::new(None, ns!(html), LocalName::from(tag))
    }

    fn attr(name: &'static str, val: &str) -> Attribute {
        Attribute {
            name: QualName::new(None, ns!(), LocalName::from(name)),
            value: val.to_string(),
        }
    }

    #[test]
    fn test_native_computed_z_index_preserves_static_zero_negative_and_auto() {
        let base_doc = Rc::new(RefCell::new(BaseDocument::new(DocumentConfig::default())));
        let doc = Document::from_base(base_doc.clone());

        {
            let mut base = base_doc.borrow_mut();
            let root_id = base.root_node().id;
            let mut mutator = base.mutate();

            let html = mutator.create_element(qname("html"), vec![]);
            let head = mutator.create_element(qname("head"), vec![]);
            let style_el = mutator.create_element(qname("style"), vec![]);
            let style_text = mutator.create_text_node(".z-50 { z-index: 50; }");
            mutator.append_children(style_el, &[style_text]);
            mutator.append_children(head, &[style_el]);

            let body = mutator.create_element(qname("body"), vec![]);
            let static_class_50 = mutator.create_element(
                qname("div"),
                vec![attr("id", "static-class-50"), attr("class", "z-50")],
            );
            let static_inline_50 = mutator.create_element(
                qname("div"),
                vec![
                    attr("id", "static-inline-50"),
                    attr("style", "position: static; z-index: 50;"),
                ],
            );
            let z_zero = mutator.create_element(
                qname("div"),
                vec![attr("id", "z-zero"), attr("style", "z-index: 0;")],
            );
            let z_neg = mutator.create_element(
                qname("div"),
                vec![attr("id", "z-neg"), attr("style", "z-index: -1;")],
            );
            let z_auto = mutator.create_element(
                qname("div"),
                vec![attr("id", "z-auto"), attr("style", "z-index: auto;")],
            );
            let z_unset = mutator.create_element(qname("div"), vec![attr("id", "z-unset")]);

            mutator.append_children(
                body,
                &[
                    static_class_50,
                    static_inline_50,
                    z_zero,
                    z_neg,
                    z_auto,
                    z_unset,
                ],
            );
            mutator.append_children(html, &[head, body]);
            mutator.append_children(root_id, &[html]);
        }

        doc.resolve(0.0);

        // FM-ZIX-1: position: static + z-50 (stylesheet class and inline style) -> Some("50")
        assert_eq!(
            doc.computed_z_index_by_id("static-class-50").as_deref(),
            Some("50")
        );
        assert_eq!(
            doc.element_by_id("static-class-50")
                .expect("static-class-50 must exist")
                .computed_z_index()
                .as_deref(),
            Some("50")
        );
        assert_eq!(
            doc.computed_z_index_by_id("static-inline-50").as_deref(),
            Some("50")
        );

        // FM-ZIX-2: z-index: 0 vs -1 vs auto vs unset
        assert_eq!(doc.computed_z_index_by_id("z-zero").as_deref(), Some("0"));
        assert_eq!(doc.computed_z_index_by_id("z-neg").as_deref(), Some("-1"));
        assert_eq!(
            doc.computed_z_index_by_id("z-auto").as_deref(),
            Some("auto")
        );
        assert_eq!(
            doc.computed_z_index_by_id("z-unset").as_deref(),
            Some("auto")
        );

        // Missing element and ElementTarget::Mock return None
        assert_eq!(doc.computed_z_index_by_id("nonexistent"), None);
        assert_eq!(Element::mock().computed_z_index(), None);
    }
}
