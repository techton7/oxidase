//! # Canonical Cross-Host `oxidase` Example
//!
//! Demonstrates a single, unified Dioxus application code path running identically across:
//! - **Web (`wasm32-unknown-unknown`)**: Driven by browser `requestAnimationFrame` and `web-sys::Document`.
//! - **Native Desktop (macOS/Linux/Windows)**: Driven by Blitz 0.3.0 / Vello GPU VSync and native `BaseDocument`.
//! - **Native Debug Control**: When built with the `blitz-host` feature, debug control is automatically available with zero app-level boilerplate.
//!
//! ## Key `oxidase` Capabilities Demonstrated:
//! 1. `#[oxidase::main]` unified entrypoint macro.
//! 2. Ambient `Document::current()` across Web and Native.
//! 3. Declarative `use_frame` animation loop ticking automatically.
//! 4. Asynchronous `next_frame().await` resolving on the next VSync turn.
//! 5. Interactive state mutation via `<button id="test-interaction-button">`.

use std::time::Duration;

use dioxus::prelude::*;
use oxidase::prelude::*;

#[oxidase::main]
fn main() {
    println!("=================================================================");
    println!("[cross_host] Launching Canonical oxidase Cross-Host Example");
    #[cfg(target_arch = "wasm32")]
    println!("  • Platform : Web (Browser requestAnimationFrame)");
    #[cfg(not(target_arch = "wasm32"))]
    println!("  • Platform : Native (Blitz 0.3.0 / Vello GPU VSync)");
    println!("=================================================================");

    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    // 1. Cross-platform document binding
    let doc = Document::current().expect("Document::current() must be available via #[oxidase::main]");
    #[cfg(target_arch = "wasm32")]
    let doc_info = format!("Browser Document (Title: '{}')", doc.raw().title());
    #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
    let doc_info = format!("Blitz Native Document (ID: {})", doc.base().borrow().id());
    #[cfg(all(not(target_arch = "wasm32"), not(feature = "native")))]
    let doc_info = "Host Document Mock".to_string();

    // 2. Reactive animation and frame state
    let mut frame_count = use_signal(|| 0u64);
    let mut total_duration = use_signal(|| Duration::ZERO);
    let mut last_dt = use_signal(|| Duration::ZERO);
    let mut status_msg = use_signal(|| "Starting Frame Loop...".to_string());
    let mut click_count = use_signal(|| 0u32);
    let mut input_text = use_signal(|| String::new());
    let mut is_focused = use_signal(|| false);
    let mut is_hovered = use_signal(|| false);
    let mut is_pressed = use_signal(|| false);
    let mut scroll_pos = use_signal(|| 0.0f64);

    // 3. High-level DX: next_frame().await
    use_future(move || async move {
        let first_frame = next_frame().await;
        println!(
            "[cross_host] next_frame().await resolved: now={:?}, delta={:?}",
            first_frame.now, first_frame.delta
        );
    });

    // 4. High-level DX: use_frame declarative animation hook
    use_frame(move |info| {
        let count = frame_count() + 1;
        frame_count.set(count);
        last_dt.set(info.delta);
        total_duration.set(total_duration() + info.delta);

        if status_msg().starts_with("Starting") {
            status_msg.set(format!("Frame Loop Active (initial dt: {:?})", info.delta));
        }
    });

    // 5. FPS calculation
    let instant_fps = if last_dt().as_secs_f64() > 0.001 {
        format!("{:.1} FPS", 1.0 / last_dt().as_secs_f64())
    } else {
        "--".to_string()
    };
    let avg_fps = if total_duration().as_secs_f64() > 0.05 {
        format!("{:.1} FPS", frame_count() as f64 / total_duration().as_secs_f64())
    } else {
        "--".to_string()
    };

    let button_label = if click_count() == 0 {
        "Click to Test Event".to_string()
    } else {
        format!("Clicked {} times", click_count())
    };

    #[cfg(target_arch = "wasm32")]
    let platform_badge = "Web (rAF)";
    #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
    let platform_badge = "Native (Blitz / Vello)";
    #[cfg(all(not(target_arch = "wasm32"), not(feature = "native")))]
    let platform_badge = "Host";

    let is_debug_control = {
        #[cfg(feature = "blitz-host")]
        {
            is_debug_control_active()
        }
        #[cfg(not(feature = "blitz-host"))]
        {
            false
        }
    };

    rsx! {
        div {
            style: "width: 100vw; min-height: 100vh; background-color: #0f172a; color: #f8fafc; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; padding: 40px 20px; box-sizing: border-box; display: flex; flex-direction: column; justify-content: center; align-items: center;",

            div {
                style: "width: 100%; max-width: 660px; background-color: #1e293b; border-radius: 16px; padding: 32px; box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.5); border: 1px solid #334155;",

                // Header
                div {
                    style: "display: flex; align-items: center; justify-content: space-between; margin-bottom: 24px; border-bottom: 1px solid #334155; padding-bottom: 16px;",
                    div {
                        h1 { style: "margin: 0; font-size: 22px; font-weight: 700; color: #38bdf8;", "oxidase Cross-Host Example" }
                        p { style: "margin: 4px 0 0; font-size: 13px; color: #94a3b8;", "Universal Web & Native Dioxus Hosted Runtime" }
                    }
                    div { style: "display: flex; gap: 8px; align-items: center;",
                        if is_debug_control {
                            div {
                                style: "background: #7c3aed; color: #ede9fe; padding: 4px 10px; border-radius: 9999px; font-size: 11px; font-weight: 700;",
                                "Debug Control"
                            }
                        }
                        div {
                            id: "platform-badge",
                            style: "background: #0284c7; color: white; padding: 4px 10px; border-radius: 9999px; font-size: 11px; font-weight: 600;",
                            "{platform_badge}"
                        }
                    }
                }

                // Status card
                div {
                    style: "background: #0f172a; border-radius: 12px; padding: 20px; margin-bottom: 20px; border: 1px solid #1e293b;",

                    div { style: "display: flex; align-items: center; gap: 10px; margin-bottom: 12px;",
                        div { style: "width: 10px; height: 10px; border-radius: 50%; background: #10b981; box-shadow: 0 0 10px #10b981;" }
                        span { style: "font-size: 15px; font-weight: 600; color: #f1f5f9;", "{status_msg}" }
                    }

                    // Visual VSync pulse animation bar
                    div {
                        style: "width: 100%; height: 6px; background: #334155; border-radius: 3px; overflow: hidden; margin-bottom: 16px;",
                        div {
                            style: "height: 100%; width: {((frame_count() * 3) % 100)}%; background: #38bdf8; border-radius: 3px;",
                        }
                    }

                    // Metrics grid
                    div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 10px; font-size: 13px;",
                        div { style: "color: #94a3b8;", "Frames Executed:" }
                        div { style: "color: #38bdf8; font-weight: 700; font-family: monospace;", "{frame_count}" }

                        div { style: "color: #94a3b8;", "Frame Rate (Instant / Avg):" }
                        div { style: "color: #38bdf8; font-weight: 700; font-family: monospace;", "{instant_fps} (Avg: {avg_fps})" }

                        div { style: "color: #94a3b8;", "Last Frame Delta:" }
                        div { style: "color: #f1f5f9; font-family: monospace;", "{last_dt():?}" }

                        div { style: "color: #94a3b8;", "Total Elapsed:" }
                        div { style: "color: #f1f5f9; font-family: monospace;", "{total_duration():?}" }

                        div { style: "color: #94a3b8;", "Document:" }
                        div { style: "color: #f1f5f9; font-family: monospace;", "{doc_info}" }
                    }

                    // Text Input & Focus workflow verification
                    div {
                        style: "margin-top: 20px; padding-top: 16px; border-top: 1px solid #334155; display: flex; flex-direction: column; gap: 8px;",
                        div {
                            style: "display: flex; align-items: center; justify-content: space-between;",
                            span { style: "font-size: 13px; font-weight: 600; color: #94a3b8;", "Live Input Workflow Test:" }
                            if is_focused() {
                                span {
                                    id: "focus-indicator",
                                    style: "background: #065f46; color: #6ee7b7; padding: 2px 8px; border-radius: 4px; font-size: 11px; font-weight: 600;",
                                    "FOCUSED"
                                }
                            }
                        }
                        div {
                            style: "display: flex; gap: 10px; align-items: center;",
                            input {
                                id: "test-input",
                                style: "flex: 1; background: #0f172a; color: #f8fafc; border: 1px solid #475569; padding: 8px 12px; border-radius: 6px; font-size: 13px;",
                                placeholder: "Type here...",
                                value: "{input_text}",
                                onfocus: move |_| {
                                    is_focused.set(true);
                                    println!("[cross_host] Input focused!");
                                },
                                onblur: move |_| {
                                    is_focused.set(false);
                                    println!("[cross_host] Input blurred!");
                                },
                                oninput: move |evt: FormEvent| {
                                    let val = evt.value();
                                    println!("[cross_host] Input value changed: {}", val);
                                    input_text.set(val);
                                },
                            }
                        }
                        p {
                            id: "typed-text",
                            style: "margin: 0; font-size: 13px; color: #38bdf8; font-family: monospace;",
                            "Typed: {input_text}"
                        }
                    }

                    // Interaction button for click/event verification
                    div { style: "margin-top: 16px; display: flex; justify-content: flex-end;",
                        button {
                            id: "test-interaction-button",
                            style: "background: #2563eb; color: white; padding: 8px 16px; border: none; border-radius: 6px; font-size: 13px; font-weight: 600; cursor: pointer;",
                            onclick: move |_| {
                                click_count += 1;
                                println!("[cross_host] Button clicked! Count: {}", click_count());
                            },
                            "{button_label}"
                        }
                    }

                    // Mouse / Pointer Test Section
                    div {
                        style: "margin-top: 20px; padding-top: 16px; border-top: 1px solid #334155; display: flex; flex-direction: column; gap: 10px;",
                        div {
                            style: "display: flex; align-items: center; justify-content: space-between;",
                            span { style: "font-size: 13px; font-weight: 600; color: #94a3b8;", "Live Mouse / Pointer Test:" }
                            div { style: "display: flex; gap: 6px;",
                                span {
                                    id: "hover-status",
                                    style: if is_hovered() {
                                        "background: #0284c7; color: #e0f2fe; padding: 2px 8px; border-radius: 4px; font-size: 11px; font-weight: 600;"
                                    } else {
                                        "background: #334155; color: #94a3b8; padding: 2px 8px; border-radius: 4px; font-size: 11px; font-weight: 600;"
                                    },
                                    if is_hovered() { "HOVERED" } else { "NOT_HOVERED" }
                                }
                                span {
                                    id: "pressed-status",
                                    style: if is_pressed() {
                                        "background: #b91c1c; color: #fef2f2; padding: 2px 8px; border-radius: 4px; font-size: 11px; font-weight: 600;"
                                    } else {
                                        "background: #334155; color: #94a3b8; padding: 2px 8px; border-radius: 4px; font-size: 11px; font-weight: 600;"
                                    },
                                    if is_pressed() { "PRESSED" } else { "RELEASED" }
                                }
                            }
                        }
                        div {
                            id: "mouse-test-card",
                            style: "background: #0f172a; border: 1px solid #475569; padding: 12px; border-radius: 6px; text-align: center; cursor: pointer;",
                            onmouseenter: move |_| {
                                is_hovered.set(true);
                                println!("[cross_host] Mouse entered card");
                            },
                            onmouseleave: move |_| {
                                is_hovered.set(false);
                                println!("[cross_host] Mouse left card");
                            },
                            onpointerdown: move |_| {
                                is_pressed.set(true);
                                println!("[cross_host] Pointer down on card");
                            },
                            onpointerup: move |_| {
                                is_pressed.set(false);
                                println!("[cross_host] Pointer up on card");
                            },
                            span { style: "font-size: 13px; color: #e2e8f0;", "Pointer Target (Move / Down / Up / Drag)" }
                        }
                    }

                    // Wheel / Scroll Test Section
                    div {
                        style: "margin-top: 16px; padding-top: 16px; border-top: 1px solid #334155; display: flex; flex-direction: column; gap: 8px;",
                        div {
                            style: "display: flex; align-items: center; justify-content: space-between;",
                            span { style: "font-size: 13px; font-weight: 600; color: #94a3b8;", "Live Wheel / Scroll Test:" }
                            span {
                                id: "scroll-status",
                                style: "font-size: 12px; color: #38bdf8; font-family: monospace;",
                                "Scroll Y: {scroll_pos():.0}"
                            }
                        }
                        div {
                            id: "test-scroll-container",
                            style: "height: 80px; overflow-y: scroll; background: #0f172a; border: 1px solid #334155; border-radius: 6px; padding: 8px;",
                            onwheel: move |evt: WheelEvent| {
                                if let dioxus::html::geometry::WheelDelta::Pixels(v) = evt.delta() {
                                    let new_pos = (scroll_pos() + v.y).max(0.0);
                                    println!("[cross_host] Wheel scrolled delta_y={} -> new_pos={}", v.y, new_pos);
                                    scroll_pos.set(new_pos);
                                } else {
                                    scroll_pos.set(scroll_pos() + 45.0);
                                }
                            },
                            div { style: "height: 30px; color: #94a3b8; font-size: 12px;", "Scroll item 1" }
                            div { style: "height: 30px; color: #94a3b8; font-size: 12px;", "Scroll item 2" }
                            div { style: "height: 30px; color: #94a3b8; font-size: 12px;", "Scroll item 3" }
                            div { style: "height: 30px; color: #94a3b8; font-size: 12px;", "Scroll item 4" }
                            div { style: "height: 30px; color: #94a3b8; font-size: 12px;", "Scroll item 5" }
                        }
                    }
                }

                // Footer checklist
                div { style: "font-size: 12px; color: #64748b;",
                    p { style: "margin: 0 0 4px 0;", "✔ Unified #[oxidase::main] bootstrap macro" }
                    p { style: "margin: 0 0 4px 0;", "✔ Ambient Document::current() binding" }
                    p { style: "margin: 0 0 4px 0;", "✔ Declarative use_frame and async next_frame VSync driving" }
                    p { style: "margin: 0;", "✔ Interactive DOM state mutation" }
                }
            }
        }
    }
}
