#![cfg(feature = "native")]

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
