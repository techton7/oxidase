use dioxus::prelude::*;
use oxidase::dom::Document;

#[oxidase::main]
fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut status = use_signal(|| "Initializing...".to_string());
    let mut platform_name = use_signal(|| "Detecting...".to_string());
    let mut doc_details = use_signal(|| "None".to_string());
    let mut click_count = use_signal(|| 0);

    // Initial mount check
    use_effect(move || {
        #[cfg(target_arch = "wasm32")]
        platform_name.set("Web (wasm32)".to_string());

        #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
        platform_name.set("Native (Blitz)".to_string());

        #[cfg(all(not(target_arch = "wasm32"), not(feature = "native")))]
        platform_name.set("Host (Mock)".to_string());

        if let Some(_doc) = Document::current() {
            status.set("Active (Mounted)".to_string());

            #[cfg(target_arch = "wasm32")]
            {
                let title = _doc.raw().title();
                doc_details.set(if title.is_empty() {
                    "Browser Document Mounted (Title: empty)".to_string()
                } else {
                    format!("Browser Document Title: {}", title)
                });
            }

            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            {
                doc_details.set(format!("Blitz Native Document (ID: {})", _doc.base().borrow().id()));
            }

            #[cfg(all(not(target_arch = "wasm32"), not(feature = "native")))]
            {
                doc_details.set("Host Mock Document Available".to_string());
            }
        } else {
            status.set("Inactive (None)".to_string());
            doc_details.set("Document::current() returned None".to_string());
        }
    });

    let retest_doc = move |_| {
        click_count += 1;
        if let Some(_doc) = Document::current() {
            status.set(format!("Active (Verified on Click #{})", click_count()));

            #[cfg(target_arch = "wasm32")]
            {
                let title = _doc.raw().title();
                doc_details.set(format!("Click #{}: Active Browser Document (title: '{}')", click_count(), title));
            }

            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            {
                doc_details.set(format!("Click #{}: Active Blitz Document (ID: {})", click_count(), _doc.base().borrow().id()));
            }

            #[cfg(all(not(target_arch = "wasm32"), not(feature = "native")))]
            {
                doc_details.set(format!("Click #{}: Active Document Instance", click_count()));
            }
        } else {
            status.set(format!("Failed (Click #{} returned None)", click_count()));
        }
    };

    rsx! {
        div {
            id: "dom-surface-root",
            style: "min-height: 100vh; background-color: #0f172a; color: #f8fafc; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; padding: 40px 24px; box-sizing: border-box;",

            div {
                style: "max-width: 720px; margin: 0 auto; background: #1e293b; border-radius: 16px; padding: 32px; box-shadow: 0 10px 25px -5px rgba(0, 0, 0, 0.4); border: 1px solid #334155;",

                // Header
                div { style: "display: flex; align-items: center; justify-content: space-between; margin-bottom: 24px; border-bottom: 1px solid #334155; padding-bottom: 16px;",
                    div {
                        h1 { style: "margin: 0; font-size: 24px; font-weight: 700; color: #38bdf8;", "oxidase::dom Surface" }
                        p { style: "margin: 4px 0 0; font-size: 14px; color: #94a3b8;", "Universal Pure-Rust Cross-Platform DOM Verification" }
                    }
                    div {
                        id: "platform-badge",
                        style: "background: #0284c7; color: white; padding: 6px 14px; border-radius: 9999px; font-size: 13px; font-weight: 600;",
                        "{platform_name}"
                    }
                }

                // Status Card
                div {
                    style: "background: #0f172a; border-radius: 12px; padding: 20px; margin-bottom: 24px; border: 1px solid #1e293b;",

                    div { style: "font-size: 12px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em; color: #64748b; margin-bottom: 8px;", "Document State" }

                    div {
                        style: "display: flex; align-items: center; gap: 12px;",
                        div {
                            style: if status().starts_with("Active") {
                                "width: 12px; height: 12px; border-radius: 50%; background: #10b981; box-shadow: 0 0 12px #10b981;"
                            } else {
                                "width: 12px; height: 12px; border-radius: 50%; background: #ef4444; box-shadow: 0 0 12px #ef4444;"
                            }
                        }
                        span {
                            id: "doc-status",
                            style: "font-size: 18px; font-weight: 600; color: #f1f5f9;",
                            "{status}"
                        }
                    }

                    div {
                        id: "doc-details",
                        style: "margin-top: 12px; font-family: monospace; font-size: 13px; color: #94a3b8; word-break: break-all;",
                        "{doc_details}"
                    }
                }

                // Interactive Action Button
                div { style: "display: flex; justify-content: flex-end;",
                    button {
                        id: "btn-test-current",
                        onclick: retest_doc,
                        style: "background: #2563eb; hover:background: #1d4ed8; color: white; font-weight: 600; font-size: 14px; padding: 10px 20px; border: none; border-radius: 8px; cursor: pointer; transition: background 0.2s;",
                        "Re-test Document::current()"
                    }
                }
            }
        }
    }
}
