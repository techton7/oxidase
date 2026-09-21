//! Application launch bootstrap module providing ambient cross-platform DOM capability.

use dioxus::prelude::*;

#[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
std::thread_local! {
    static TARGET_APP: std::cell::Cell<Option<fn() -> Element>> = const { std::cell::Cell::new(None) };
}

#[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
fn native_bootstrap_root() -> Element {
    if let Some(doc) = crate::dom::Document::current() {
        doc.provide_context();
    }
    if let Some(app) = TARGET_APP.with(|c| c.get()) {
        app()
    } else {
        rsx! {}
    }
}

/// Ensures ambient native Document context is active, initializing with default BaseDocument if none exists.
pub fn ensure_document_context() {
    #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
    {
        use blitz_dom::{BaseDocument, DocumentConfig};
        use std::cell::RefCell;
        use std::rc::Rc;

        if crate::dom::Document::current().is_none() {
            let base_doc = Rc::new(RefCell::new(BaseDocument::new(DocumentConfig::default())));
            dioxus::prelude::provide_context(crate::dom::Document::from_base(base_doc));
        }
        if let Some(doc) = crate::dom::Document::current() {
            doc.provide_context();
        }
    }
}

/// Helper to bind the active host redraw requester.
pub fn bind_host_redraw_requester(request_redraw: impl Fn() + 'static) -> crate::frame::HostRedrawGuard {
    crate::frame::set_host_redraw_requester(request_redraw)
}

/// Helper to step a single hosted native frame.
pub fn step_hosted_frame() -> std::time::Duration {
    crate::frame::step_hosted_frame()
}

/// Launch a Dioxus application with ambient cross-platform DOM capability.
///
/// - On Web targets (`wasm32`), delegates to `dioxus::launch(app)` backed by browser RAF.
/// - On Native targets with the `native` feature enabled, binds ambient `Document` state and
///   mounts in-memory VirtualDom for headless execution/testing.
/// - For full sovereign desktop window execution with automatic VSync frame loop injection,
///   use `#[oxidase::main]`.
pub fn launch(app: fn() -> Element) {
    #[cfg(target_arch = "wasm32")]
    {
        dioxus::launch(app);
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
    {
        use blitz_dom::{BaseDocument, DocumentConfig};
        use std::cell::RefCell;
        use std::rc::Rc;

        TARGET_APP.with(|c| c.set(Some(app)));
        let base_doc = Rc::new(RefCell::new(BaseDocument::new(DocumentConfig::default())));
        let doc = crate::dom::Document::from_base(base_doc);

        crate::dom::Document::with_current(doc, || {
            let mut vdom = VirtualDom::new(native_bootstrap_root);
            vdom.in_scope(ScopeId::ROOT, || {
                if let Some(doc) = crate::dom::Document::current() {
                    doc.provide_context();
                }
            });
            vdom.rebuild_in_place();
            println!(
                "[oxidase::launch] Native Dioxus VirtualDom mounted successfully in headless mode."
            );
        });
    }

    #[cfg(all(not(target_arch = "wasm32"), not(feature = "native")))]
    {
        dioxus::launch(app);
    }
}
