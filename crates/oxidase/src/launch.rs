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

/// Launch a Dioxus application with ambient cross-platform DOM capability.
///
/// On Web targets (`wasm32`), this delegates to `dioxus::launch(app)`.
/// On Native targets with the `native` feature enabled, this binds thread-local native
/// `Document` state and injects typed [`crate::dom::Document`] into root Dioxus context
/// via `native_bootstrap_root` before invoking `dioxus::launch`.
///
/// Note: Full sovereign windowed native launching is pending upstream release of
/// `dioxus-native` / `dioxus-native-dom` compatible with `blitz-dom 0.3.0-beta.2` and `dioxus 0.7.x`.
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
            dioxus::launch(native_bootstrap_root);
        });
    }

    #[cfg(all(not(target_arch = "wasm32"), not(feature = "native")))]
    {
        dioxus::launch(app);
    }
}
