//! Pure-Rust cross-platform DOM abstraction layer.
//!
//! Provides a unified, synchronous DOM access API across Web (`wasm32`)
//! and Native (`blitz-dom`).

#[cfg(target_arch = "wasm32")]
#[path = "web.rs"]
mod backend;

#[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
#[path = "native.rs"]
mod backend;

#[cfg(all(not(target_arch = "wasm32"), not(feature = "native")))]
#[path = "mock.rs"]
mod backend;

pub mod element;
pub use element::*;

pub mod observer;
pub use observer::*;

pub use backend::Document;

/// Returns the ambient active `Document`.
pub fn document() -> Document {
    Document::current().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_document_current() {
        // On non-wasm host without active mock, current() returns None
        assert!(Document::current().is_none());
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
    #[test]
    fn test_native_document_resolution() {
        use blitz_dom::{BaseDocument, DocumentConfig};
        use std::cell::RefCell;
        use std::rc::Rc;

        // Initially None
        assert!(Document::current().is_none());

        // Create a real Blitz BaseDocument handle
        let base_doc = Rc::new(RefCell::new(BaseDocument::new(DocumentConfig::default())));
        let native_doc = Document::from_base(base_doc.clone());

        // Test with_current scope
        Document::with_current(native_doc.clone(), || {
            let cur = Document::current().expect("Document::current() must be Some within with_current");
            assert_eq!(cur.base().borrow().id(), base_doc.borrow().id());
        });

        // After scope, must be None again
        assert!(Document::current().is_none());

        // Test set_current
        Document::set_current(Some(native_doc));
        assert!(Document::current().is_some());
        Document::set_current(None);
        assert!(Document::current().is_none());
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
    #[test]
    fn test_native_document_query_descendant_order() {
        use blitz_dom::{BaseDocument, DocumentConfig};
        use std::cell::RefCell;
        use std::rc::Rc;

        let base_doc = Rc::new(RefCell::new(BaseDocument::new(DocumentConfig::default())));
        let native_doc = Document::from_base(base_doc.clone());

        let qual_name = |name: &'static str| blitz_dom::QualName {
            prefix: None,
            ns: blitz_dom::ns!(html),
            local: blitz_dom::LocalName::from(name),
        };
        let attr = |name: &'static str, value: &str| blitz_dom::Attribute {
            name: qual_name(name),
            value: value.to_string(),
        };

        {
            let mut base = base_doc.borrow_mut();
            let root_id = base.root_node().id;
            let mut mutator = base.mutate();
            let content = mutator.create_element(qual_name("div"), vec![attr("id", "content")]);
            let g1 = mutator.create_element(qual_name("div"), vec![attr("id", "g1")]);
            let g1_a = mutator.create_element(qual_name("div"), vec![attr("id", "g1-a")]);
            let g1_b = mutator.create_element(qual_name("div"), vec![attr("id", "g1-b")]);
            let g2 = mutator.create_element(qual_name("div"), vec![attr("id", "g2")]);
            let g2_c = mutator.create_element(qual_name("div"), vec![attr("id", "g2-c")]);
            let g2_d = mutator.create_element(qual_name("div"), vec![attr("id", "g2-d")]);

            mutator.append_children(g1, &[g1_a, g1_b]);
            mutator.append_children(g2, &[g2_c, g2_d]);
            mutator.append_children(content, &[g1, g2]);
            mutator.append_children(root_id, &[content]);
        }

        let order = native_doc
            .query_descendant_order("content", &["g2-d", "g1-b", "g2-c", "g1-a"])
            .expect("query_descendant_order should succeed");
        assert_eq!(order, vec!["g1-a", "g1-b", "g2-c", "g2-d"]);

        // Non-existent root returns ElementNotFound
        let missing = native_doc.query_descendant_order("nonexistent", &["g1-a"]);
        assert!(matches!(missing, Err(crate::error::HostError::ElementNotFound(_))));
    }
}
