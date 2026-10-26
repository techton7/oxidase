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
            let doc = crate::dom::Document::from_unattached_base(base_doc);
            if dioxus::core::Runtime::try_current().is_some() {
                dioxus::prelude::provide_context(doc.clone());
            }
            crate::dom::Document::set_current(Some(doc));
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

/// Transparent root wrapper component injected by `#[oxidase::main]`.
///
/// Wraps `children` in a container (`div` with `display: contents;`) to capture the live window's
/// `NodeHandle` on mount and bind its `BaseDocument` via `Document::bind_live_base`.
///
/// When the `blitz-host` feature is enabled on `oxidase`, additionally wraps in
/// `<blitz_host::BlitzHost>` to service control requests and synthetic actions.
#[component]
pub fn HostedRootWrapper(children: Element) -> Element {
    #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
    {
        let wrapped_children = rsx! {
            div {
                style: "display: contents;",
                onmounted: move |evt: Event<MountedData>| {
                    if let Some(handle) = evt.downcast::<dioxus_native::NodeHandle>() {
                        let base_doc = crate::dom::Document::extract_base_doc(handle);
                        if let Some(doc) = dioxus::prelude::try_consume_context::<crate::dom::Document>() {
                            doc.bind_live_base(base_doc.clone());
                        }
                        if let Some(doc) = crate::dom::Document::current() {
                            doc.bind_live_base(base_doc.clone());
                        } else {
                            crate::dom::Document::set_current(Some(crate::dom::Document::from_base(base_doc)));
                        }
                    }
                },
                {children}
            }
        };

        #[cfg(feature = "blitz-host")]
        {
            rsx! {
                ::blitz_host::BlitzHost {
                    {wrapped_children}
                }
            }
        }
        #[cfg(not(feature = "blitz-host"))]
        {
            wrapped_children
        }
    }
    #[cfg(not(all(not(target_arch = "wasm32"), feature = "native")))]
    {
        children
    }
}

/// Automatically initializes `blitz-host` debug control if the `blitz-host` feature
/// is enabled on `oxidase`.
pub fn init_debug_control_if_available() {
    #[cfg(feature = "blitz-host")]
    {
        ::blitz_host::init_default();
    }
}


/// Returns whether `blitz-host` debug control is currently active in the running process.
pub fn is_debug_control_active() -> bool {
    #[cfg(feature = "blitz-host")]
    {
        ::blitz_host::HostControl::is_global_active()
    }
    #[cfg(not(feature = "blitz-host"))]
    {
        false
    }
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
        let doc = crate::dom::Document::from_unattached_base(base_doc);

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
