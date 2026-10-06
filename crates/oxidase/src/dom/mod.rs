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
}
