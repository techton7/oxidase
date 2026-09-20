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
/// Note on Native & Frame Looping (Spike Findings):
/// 1. Sovereign windowed launching requires an active platform event loop (e.g. Winit via `dioxus-native`).
///    Without a platform runner (such as `dioxus-desktop` or `dioxus-native`), calling `dioxus::launch` directly
///    panics at runtime with "No platform feature enabled".
/// 2. Hosted native frame auto-looping cannot be safely synthesized via timers inside `launch` or `#[oxidase::main]`
///    without breaking redraw alignment with VSync. True redraw truth belongs in the sovereign window's
///    `WindowEvent::RedrawRequested` cycle, and is therefore deferred pending upstream `dioxus-native` parity (ISSUE-0001).
/// 3. For tests, headless environments, and current native execution, use deterministic manual ticking via
///    [`crate::frame::tick`].
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
            // In native environments without an upstream platform runner (e.g. desktop/web),
            // mount the VirtualDom directly with ambient Blitz Document context.
            let mut vdom = VirtualDom::new(native_bootstrap_root);
            vdom.in_scope(ScopeId::ROOT, || {
                if let Some(doc) = crate::dom::Document::current() {
                    doc.provide_context();
                }
            });
            vdom.rebuild_in_place();
            println!(
                "[oxidase::launch] Native Dioxus VirtualDom mounted successfully with Blitz Document."
            );
        });
    }

    #[cfg(all(not(target_arch = "wasm32"), not(feature = "native")))]
    {
        dioxus::launch(app);
    }
}
