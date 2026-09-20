//! Native (Blitz) DOM backend implementation.

#[allow(unused_imports)]
use super::types::*;
use blitz_dom::BaseDocument;
use std::cell::RefCell;
use std::rc::Rc;

/// Native Blitz Document handle wrapping the real `BaseDocument`.
#[derive(Clone)]
pub struct Document {
    inner: Rc<RefCell<BaseDocument>>,
}

impl std::fmt::Debug for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Document")
            .field("document_id", &self.inner.borrow().id())
            .finish()
    }
}

impl Document {
    /// Returns the active native Blitz document if available in current thread or Dioxus context.
    pub fn current() -> Option<Self> {
        // 1. Check Dioxus root context if running within an active Dioxus runtime
        if dioxus::core::Runtime::try_current().is_some() {
            if let Some(doc) = dioxus::prelude::try_consume_context::<Document>() {
                return Some(doc);
            }
        }

        // 2. Check thread-local (active during harness dispatch or scoped execution in Blitz)
        if let Some(doc) = CURRENT_NATIVE_DOC.with(|cell| cell.borrow().clone()) {
            return Some(doc);
        }

        None
    }

    /// Construct a Document wrapping a real Blitz BaseDocument.
    pub fn from_base(inner: Rc<RefCell<BaseDocument>>) -> Self {
        Self { inner }
    }

    /// Access the underlying real `BaseDocument`.
    pub fn base(&self) -> &Rc<RefCell<BaseDocument>> {
        &self.inner
    }

    /// Sets the thread-local active native Document for the duration of a closure.
    pub fn with_current<R>(doc: Self, f: impl FnOnce() -> R) -> R {
        CURRENT_NATIVE_DOC.with(|cell| {
            let prev = cell.borrow_mut().replace(doc);
            let res = f();
            *cell.borrow_mut() = prev;
            res
        })
    }

    /// Sets or clears the current thread-local active native Document.
    pub fn set_current(doc: Option<Self>) {
        CURRENT_NATIVE_DOC.with(|cell| {
            *cell.borrow_mut() = doc;
        });
    }

    /// Provide this Document into the active Dioxus component context.
    pub fn provide_context(self) {
        dioxus::prelude::provide_context(self);
    }
}

std::thread_local! {
    static CURRENT_NATIVE_DOC: RefCell<Option<Document>> = const { RefCell::new(None) };
}

/// Native Blitz Element handle (placeholder for next slice).
#[derive(Clone, Debug, Default)]
pub struct Element;
