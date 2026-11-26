use blitz_dom::{BaseDocument, DocumentConfig};
use dioxus::prelude::*;
use oxidase::dom::Document;
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn test_blitz_document_unwired_returns_none() {
    // 1. Negative / Baseline check:
    // Without active Blitz runtime or registered document, Document::current() MUST be None.
    assert!(
        Document::current().is_none(),
        "Baseline failed: Document::current() must be None before wiring"
    );
}

#[component]
fn VerificationApp() -> Element {
    let mut status = use_signal(|| "Initial".to_string());
    let mut doc_id = use_signal(|| 0usize);

    use_effect(move || {
        if let Some(doc) = Document::current() {
            status.set("Active (Mounted)".to_string());
            doc_id.set(doc.base().borrow().id());
        } else {
            status.set("Inactive (None)".to_string());
        }
    });

    rsx! {
        div { id: "test-root",
            div { id: "doc-status", "{status}" }
            div { id: "doc-id", "Document ID: {doc_id}" }
        }
    }
}

#[test]
fn test_blitz_document_scoped_and_root_context() {
    let base_doc = Rc::new(RefCell::new(BaseDocument::new(DocumentConfig::default())));
    let expected_doc_id = base_doc.borrow().id();
    let real_doc = Document::from_base(base_doc.clone());

    // 1. Verify thread-local scoped execution
    Document::with_current(real_doc.clone(), || {
        let current = Document::current().expect("Document::current() must return Some in with_current scope");
        assert_eq!(current.base().borrow().id(), expected_doc_id);
    });

    // Cleanup: outside scope, reverts cleanly to None
    assert!(
        Document::current().is_none(),
        "Cleanup failed: Document::current() must revert to None after scope exit"
    );

    // 2. Verify Dioxus ScopeId::ROOT context injection
    let mut vdom = VirtualDom::new(VerificationApp);
    vdom.in_scope(ScopeId::ROOT, || {
        provide_context(real_doc);
    });
    vdom.rebuild_in_place();

    // Verify Document::current() inside root scope
    vdom.in_scope(ScopeId::ROOT, || {
        let current = Document::current().expect("Document::current() must return Some in Dioxus root context");
        assert_eq!(current.base().borrow().id(), expected_doc_id);
    });
}

#[test]
fn test_measure_rect_2d_transform_fixed_and_child() {
    use blitz_dom::{Attribute, QualName, local_name, ns};

    let mut base_doc = BaseDocument::new(DocumentConfig::default());
    {
        let mut vp = base_doc.viewport_mut();
        vp.window_size = (1280, 720);
        vp.hidpi_scale = 1.0;
    }
    let doc_root = base_doc.root_node().id;

    {
        let mut mutator = base_doc.mutate();
        let html = mutator.create_element(
            QualName::new(None, ns!(), local_name!("html")),
            vec![Attribute {
                name: QualName::new(None, ns!(), local_name!("style")),
                value: "margin: 0; padding: 0; width: 1280px; height: 2000px; display: block;"
                    .into(),
            }],
        );
        let body = mutator.create_element(
            QualName::new(None, ns!(), local_name!("body")),
            vec![Attribute {
                name: QualName::new(None, ns!(), local_name!("style")),
                value: "margin: 0; padding: 0; width: 1280px; height: 2000px; display: block;"
                    .into(),
            }],
        );
        let wrapper = mutator.create_element(
            QualName::new(None, ns!(), local_name!("div")),
            vec![
                Attribute {
                    name: QualName::new(None, ns!(), local_name!("id")),
                    value: "fixed-wrapper".into(),
                },
                Attribute {
                    name: QualName::new(None, ns!(), local_name!("style")),
                    value: "position: fixed; left: 0px; top: 0px; width: 200px; height: 100px; transform: translate(120px, 80px); display: block;".into(),
                },
            ],
        );
        let child = mutator.create_element(
            QualName::new(None, ns!(), local_name!("div")),
            vec![
                Attribute {
                    name: QualName::new(None, ns!(), local_name!("id")),
                    value: "fixed-child".into(),
                },
                Attribute {
                    name: QualName::new(None, ns!(), local_name!("style")),
                    value: "position: relative; left: 10px; top: 15px; width: 80px; height: 40px; display: block;".into(),
                },
            ],
        );
        mutator.append_children(wrapper, &[child]);
        mutator.append_children(body, &[wrapper]);
        mutator.append_children(html, &[body]);
        mutator.append_children(doc_root, &[html]);
    }

    let doc = Document::from_base(Rc::new(RefCell::new(base_doc)));
    doc.resolve(0.0);

    let wrapper_rect = doc
        .measure_rect("fixed-wrapper")
        .expect("fixed-wrapper rect must resolve");
    assert!(
        (wrapper_rect.x - 120.0).abs() < 1e-3,
        "wrapper x: {}",
        wrapper_rect.x
    );
    assert!(
        (wrapper_rect.y - 80.0).abs() < 1e-3,
        "wrapper y: {}",
        wrapper_rect.y
    );
    assert!((wrapper_rect.width - 200.0).abs() < 1e-3);
    assert!((wrapper_rect.height - 100.0).abs() < 1e-3);

    let child_rect = doc
        .measure_rect("fixed-child")
        .expect("fixed-child rect must resolve");
    assert!(
        (child_rect.x - 130.0).abs() < 1e-3,
        "child x: {}",
        child_rect.x
    );
    assert!(
        (child_rect.y - 95.0).abs() < 1e-3,
        "child y: {}",
        child_rect.y
    );
    assert!((child_rect.width - 80.0).abs() < 1e-3);
    assert!((child_rect.height - 40.0).abs() < 1e-3);

    // Viewport scroll must not shift fixed wrapper or its child in viewport coordinates
    doc.set_viewport_scroll(0.0, 250.0);
    let wrapper_after_scroll = doc
        .measure_rect("fixed-wrapper")
        .expect("fixed-wrapper rect after scroll");
    assert!((wrapper_after_scroll.x - 120.0).abs() < 1e-3);
    assert!((wrapper_after_scroll.y - 80.0).abs() < 1e-3);

    let child_after_scroll = doc
        .measure_rect("fixed-child")
        .expect("fixed-child rect after scroll");
    assert!((child_after_scroll.x - 130.0).abs() < 1e-3);
    assert!((child_after_scroll.y - 95.0).abs() < 1e-3);
}
