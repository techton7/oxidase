#![cfg(feature = "native")]

use blitz_dom::dom_events::{
    BlitzKeyEvent, BlitzPointerEvent, BlitzPointerId, KeyState, MouseEventButton,
    MouseEventButtons, PointerCoords, PointerDetails, UiEvent,
};
use blitz_dom::keyboard_types::{Code, Key, Location, Modifiers};
use blitz_dom::{Attribute, BaseDocument, DocumentConfig, QualName, local_name, ns};
use oxidase::dom::observer::{
    observe_capture_events, observe_element_resize,
    observe_transition_lifecycle, CaptureEventKind, TransitionLifecycleEvent,
};
use oxidase::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

struct TestContext {
    doc: Document,
    root_id: blitz_dom::NodeId,
    base: Rc<RefCell<BaseDocument>>,
}

impl TestContext {
    fn new() -> Self {
        let mut base_doc = BaseDocument::new(DocumentConfig::default());
        {
            let mut vp = base_doc.viewport_mut();
            vp.window_size = (1920, 1080);
            vp.hidpi_scale = 1.0;
        }
        let doc_root = base_doc.root_node().id;
        let root_id = {
            let mut mutator = base_doc.mutate();
            let html_id = mutator.create_element(
                QualName::new(None, ns!(), local_name!("html")),
                vec![
                    Attribute {
                        name: QualName::new(None, ns!(), local_name!("id")),
                        value: "app-root".into(),
                    },
                    Attribute {
                        name: QualName::new(None, ns!(), local_name!("style")),
                        value: "width: 1920px; height: 1080px; margin: 0; position: relative;".into(),
                    },
                ],
            );
            mutator.append_children(doc_root, &[html_id]);
            html_id
        };
        let base = Rc::new(RefCell::new(base_doc));
        let doc = Document::from_base(base.clone());
        Self { doc, root_id, base }
    }

    fn add_element(
        &self,
        parent_id: blitz_dom::NodeId,
        tag: &str,
        id: &str,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    ) -> blitz_dom::NodeId {
        let local = match tag {
            "button" => local_name!("button"),
            "form" => local_name!("form"),
            "input" => local_name!("input"),
            _ => local_name!("div"),
        };
        let mut base = self.base.borrow_mut();
        let node_id = {
            let mut mutator = base.mutate();
            let nid = mutator.create_element(
                QualName::new(None, ns!(), local),
                vec![
                    Attribute {
                        name: QualName::new(None, ns!(), local_name!("id")),
                        value: id.into(),
                    },
                    Attribute {
                        name: QualName::new(None, ns!(), local_name!("style")),
                        value: format!(
                            "position: absolute; left: {}px; top: {}px; width: {}px; height: {}px;",
                            x, y, w, h
                        )
                        .into(),
                    },
                ],
            );
            mutator.append_children(parent_id, &[nid]);
            nid
        };

        if let Some(node) = base.get_node_mut(node_id) {
            let layout = &mut node.layout_data_mut().final_layout;
            layout.location.x = x;
            layout.location.y = y;
            layout.size.width = w;
            layout.size.height = h;
        }
        node_id
    }

    fn update_element_layout(&self, node_id: blitz_dom::NodeId, x: f32, y: f32, w: f32, h: f32) {
        let mut base = self.base.borrow_mut();
        if let Some(node) = base.get_node_mut(node_id) {
            let layout = &mut node.layout_data_mut().final_layout;
            layout.location.x = x;
            layout.location.y = y;
            layout.size.width = w;
            layout.size.height = h;
        }
    }

    fn add_scroll_container(
        &self,
        parent_id: blitz_dom::NodeId,
        id: &str,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    ) -> blitz_dom::NodeId {
        let mut base = self.base.borrow_mut();
        let node_id = {
            let mut mutator = base.mutate();
            let nid = mutator.create_element(
                QualName::new(None, ns!(), local_name!("div")),
                vec![
                    Attribute {
                        name: QualName::new(None, ns!(), local_name!("id")),
                        value: id.into(),
                    },
                    Attribute {
                        name: QualName::new(None, ns!(), local_name!("style")),
                        value: format!(
                            "position: absolute; left: {}px; top: {}px; width: {}px; height: {}px; overflow: scroll;",
                            x, y, w, h
                        )
                        .into(),
                    },
                ],
            );
            mutator.append_children(parent_id, &[nid]);
            nid
        };

        if let Some(node) = base.get_node_mut(node_id) {
            let layout = &mut node.layout_data_mut().final_layout;
            layout.location.x = x;
            layout.location.y = y;
            layout.size.width = w;
            layout.size.height = h;
        }
        node_id
    }
}

fn block_on<F: std::future::Future>(mut fut: F) -> F::Output {
    use std::pin::Pin;
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

    unsafe fn clone(_: *const ()) -> RawWaker {
        RawWaker::new(std::ptr::null(), &VTABLE)
    }
    unsafe fn wake(_: *const ()) {}
    unsafe fn wake_by_ref(_: *const ()) {}
    unsafe fn vtable_drop(_: *const ()) {}
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, vtable_drop);
    let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };
    let mut cx = Context::from_waker(&waker);

    let mut pinned = unsafe { Pin::new_unchecked(&mut fut) };
    match pinned.as_mut().poll(&mut cx) {
        Poll::Ready(res) => res,
        Poll::Pending => panic!("Future did not resolve immediately in synchronous test"),
    }
}

// 1. Layer 1: Native non-polling resize observation
#[test]
fn test_native_resize_observation_non_polling() {
    let ctx = TestContext::new();
    let panel_id = ctx.add_element(ctx.root_id, "div", "panel", 10.0, 10.0, 100.0, 100.0);

    Document::with_current(ctx.doc.clone(), || {
        let observed = Rc::new(RefCell::new(Vec::new()));
        let obs = observed.clone();

        let _guard = observe_element_resize("panel", move |entry| {
            obs.borrow_mut().push(entry);
        })
        .expect("resize observer registration must succeed");

        // Mutate layout dimensions (simulate relayout)
        ctx.update_element_layout(panel_id, 10.0, 10.0, 250.0, 180.0);

        // Notify observers upon layout resolve (event-driven, non-polling)
        ctx.doc.notify_layout_observers();

        let entries = observed.borrow();
        assert_eq!(entries.len(), 1, "Observer must fire on layout resolution without polling");
        assert_eq!(entries[0].target_id, "panel");
        assert_eq!(entries[0].content_rect.width, 250.0);
        assert_eq!(entries[0].content_rect.height, 180.0);
        drop(entries);

        // Unchanged layout resolve must be deduplicated
        ctx.doc.notify_layout_observers();
        assert_eq!(observed.borrow().len(), 1, "Unchanged layout must not emit duplicate entries");
    });
}

// 2. Layer 1: Native capture-phase event chain before default dispatch
#[test]
fn test_native_capture_event_chain_pre_dispatch() {
    let ctx = TestContext::new();
    let container_id = ctx.add_element(ctx.root_id, "div", "container", 0.0, 0.0, 300.0, 300.0);
    let trigger_id = ctx.add_element(container_id, "button", "trigger", 20.0, 20.0, 80.0, 30.0);

    Document::with_current(ctx.doc.clone(), || {
        let captured = Rc::new(RefCell::new(Vec::new()));
        let cap = captured.clone();

        let _guard = observe_capture_events(move |ev| {
            cap.borrow_mut().push(ev);
        })
        .expect("capture observer registration must succeed");

        // Dispatch a capture-phase event on the nested child
        ctx.doc.dispatch_capture_event(CaptureEventKind::PointerDown, trigger_id);

        let events = captured.borrow();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, CaptureEventKind::PointerDown);
        assert_eq!(events[0].target_id.as_deref(), Some("trigger"));
        // Ancestor chain must preserve order from target up to container
        assert!(
            events[0].path_ids.contains(&"trigger".to_string())
                && events[0].path_ids.contains(&"container".to_string()),
            "Captured path IDs must contain target and ancestors: {:?}",
            events[0].path_ids
        );
        // Inside boundary test
        assert!(!events[0].is_outside_boundary(&["container"]));
        // Outside boundary test
        assert!(events[0].is_outside_boundary(&["other-container"]));
    });
}

// 3. Layer 1: Transition animation completion signal
#[test]
fn test_lifecycle_animation_completion_signal() {
    let ctx = TestContext::new();
    let _modal_id = ctx.add_element(ctx.root_id, "div", "modal-el", 0.0, 0.0, 200.0, 200.0);

    Document::with_current(ctx.doc.clone(), || {
        let received = Rc::new(RefCell::new(Vec::new()));
        let rec = received.clone();

        let _guard = observe_transition_lifecycle("modal-el", 1000, move |ev| {
            rec.borrow_mut().push(ev);
        })
        .expect("transition observer registration must succeed");

        // Dispatch animation completion
        ctx.doc.dispatch_animation_end("modal-el", "fade-out");

        let events = received.borrow();
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0],
            TransitionLifecycleEvent::AnimationEnd {
                animation_name: "fade-out".into()
            }
        );
    });
}

// 4. Layer 1: Lifecycle fallback timer / signal
#[test]
fn test_lifecycle_fallback_timer() {
    let ctx = TestContext::new();
    let _drawer_id = ctx.add_element(ctx.root_id, "div", "drawer", 0.0, 0.0, 150.0, 400.0);

    Document::with_current(ctx.doc.clone(), || {
        let received = Rc::new(RefCell::new(Vec::new()));
        let rec = received.clone();

        let _guard = observe_transition_lifecycle("drawer", 50, move |ev| {
            rec.borrow_mut().push(ev);
        })
        .expect("transition observer registration must succeed");

        // Dispatch explicit fallback when animation stalls or is missing
        ctx.doc.dispatch_transition_fallback("drawer");

        let events = received.borrow();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], TransitionLifecycleEvent::TimeoutFallback);
    });
}

// 5. Layer 2: Document dismiss watcher boundary filtering
#[test]
fn test_watch_document_dismiss_boundary_filtering() {
    let ctx = TestContext::new();
    let popover_id = ctx.add_element(ctx.root_id, "div", "popover-content", 50.0, 50.0, 200.0, 200.0);
    let inside_btn_id = ctx.add_element(popover_id, "button", "inside-btn", 60.0, 60.0, 80.0, 30.0);
    let outside_btn_id = ctx.add_element(ctx.root_id, "button", "outside-btn", 300.0, 300.0, 80.0, 30.0);

    Document::with_current(ctx.doc.clone(), || {
        let dismiss_events = Rc::new(RefCell::new(Vec::new()));
        let d = dismiss_events.clone();

        let _guard = watch_document_dismiss(&["popover-content"], move |ev| {
            d.borrow_mut().push(ev);
        })
        .expect("watch_document_dismiss must succeed");

        // 1. Click inside popover -> should be suppressed
        ctx.doc.dispatch_capture_event(CaptureEventKind::PointerDown, inside_btn_id);
        assert!(
            dismiss_events.borrow().is_empty(),
            "Interactions inside boundary must NOT trigger dismiss"
        );

        // 2. Click outside popover -> triggers dismiss
        ctx.doc.dispatch_capture_event(CaptureEventKind::PointerDown, outside_btn_id);
        assert_eq!(dismiss_events.borrow().len(), 1);
        match &dismiss_events.borrow()[0] {
            DismissEvent::PointerDown { path_ids } => {
                assert!(path_ids.contains(&"outside-btn".to_string()));
            }
            other => panic!("Expected PointerDown, got {:?}", other),
        }

        // 3. Escape key -> triggers dismiss unconditionally
        ctx.doc.dispatch_capture_event(
            CaptureEventKind::KeyDown {
                key: "Escape".into(),
            },
            outside_btn_id,
        );
        assert_eq!(dismiss_events.borrow().len(), 2);
        assert_eq!(dismiss_events.borrow()[1], DismissEvent::Escape);
    });
}

// 6. Layer 2: Floating auto-update frame coalescing
#[test]
fn test_watch_floating_auto_update_frame_coalescing() {
    let ctx = TestContext::new();
    let anchor_id = ctx.add_element(ctx.root_id, "button", "anchor", 50.0, 50.0, 100.0, 40.0);
    let _floating_id = ctx.add_element(ctx.root_id, "div", "floating", 50.0, 95.0, 150.0, 200.0);

    Document::with_current(ctx.doc.clone(), || {
        let updates = Rc::new(RefCell::new(Vec::new()));
        let u = updates.clone();

        let _guard = watch_floating_auto_update(&["anchor"], "floating", move |ev| {
            u.borrow_mut().push(ev);
        })
        .expect("watch_floating_auto_update must succeed");

        // 1. Initial size change emits update
        ctx.update_element_layout(anchor_id, 50.0, 50.0, 120.0, 40.0);
        ctx.doc.notify_layout_observers();
        assert_eq!(updates.borrow().len(), 1);
        assert_eq!(updates.borrow()[0], FloatingAutoUpdateEvent::Update);

        // 2. Immediate second update within the 16ms frame window is throttled
        ctx.update_element_layout(anchor_id, 50.0, 50.0, 140.0, 40.0);
        ctx.doc.notify_layout_observers();
        assert_eq!(
            updates.borrow().len(),
            1,
            "Frame coalescing must throttle updates occurring within the same frame window"
        );

        // 3. Scroll event emission when viewport/ancestor scroll changes after throttle window
        std::thread::sleep(std::time::Duration::from_millis(20));
        ctx.doc.set_viewport_scroll(10.0, 50.0);
        assert_eq!(updates.borrow().len(), 2);
        assert_eq!(updates.borrow()[1], FloatingAutoUpdateEvent::Scroll);
    });
}

// 7. Layer 2: Presence exit animation completion and fallback
#[test]
fn test_watch_presence_exit_and_fallback() {
    let ctx = TestContext::new();
    let _toast_id = ctx.add_element(ctx.root_id, "div", "toast-notification", 0.0, 0.0, 250.0, 60.0);

    Document::with_current(ctx.doc.clone(), || {
        let presence_events = Rc::new(RefCell::new(Vec::new()));
        let p = presence_events.clone();

        let _guard = watch_presence("toast-notification", 42, move |ev| {
            p.borrow_mut().push(ev);
        })
        .expect("watch_presence must succeed");

        // Dispatch animation end for cycle 42
        ctx.doc.dispatch_animation_end("toast-notification", "slide-out");

        let events = presence_events.borrow();
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0],
            PresenceEvent::AnimationEnd {
                cycle_id: 42,
                animation_name: "slide-out".to_string(),
            }
        );
        drop(events);

        // Subsequent dispatches for finished cycle are discarded
        ctx.doc.dispatch_animation_end("toast-notification", "slide-out");
        assert_eq!(presence_events.borrow().len(), 1);

        // Missing element pre-flight fallback proof
        let fallback_events = Rc::new(RefCell::new(Vec::new()));
        let fe = fallback_events.clone();
        let _missing_guard = watch_presence("non-existent-element", 99, move |ev| {
            fe.borrow_mut().push(ev);
        })
        .expect("watch_presence on missing element must return guard");

        assert_eq!(fallback_events.borrow().len(), 1);
        assert_eq!(
            fallback_events.borrow()[0],
            PresenceEvent::Fallback {
                cycle_id: 99,
                reason: PresenceFallbackReason::Missing,
            },
            "Non-existent element must trigger immediate Missing fallback"
        );
    });
}

// 8. Layer 2: Form reset dispatch on enclosing and explicit forms
#[test]
fn test_watch_form_reset_dispatch() {
    let ctx = TestContext::new();
    let form_id = ctx.add_element(ctx.root_id, "form", "checkout-form", 0.0, 0.0, 400.0, 500.0);
    let _input_id = ctx.add_element(form_id, "input", "card-number", 10.0, 10.0, 200.0, 30.0);
    let _detached_id = ctx.add_element(ctx.root_id, "input", "detached-field", 10.0, 600.0, 200.0, 30.0);

    Document::with_current(ctx.doc.clone(), || {
        let enclosed_resets = Rc::new(RefCell::new(0));
        let explicit_resets = Rc::new(RefCell::new(0));

        let er = enclosed_resets.clone();
        let _guard1 = watch_form_reset("card-number", None, move |_| {
            *er.borrow_mut() += 1;
        })
        .expect("watch_form_reset on enclosed element must succeed");

        let ex = explicit_resets.clone();
        let _guard2 = watch_form_reset("detached-field", Some("checkout-form"), move |_| {
            *ex.borrow_mut() += 1;
        })
        .expect("watch_form_reset with explicit form override must succeed");

        // Trigger form reset on checkout-form
        ctx.doc.dispatch_form_reset("checkout-form");

        assert_eq!(*enclosed_resets.borrow(), 1, "Enclosed element must receive reset via ancestor traversal");
        assert_eq!(*explicit_resets.borrow(), 1, "Detached element must receive reset via explicit form ID");
    });
}

// 9. RAII / Teardown: WatcherGuard drop cancels subscription
#[test]
fn test_watcher_guard_drop_cancels_subscription() {
    let ctx = TestContext::new();
    let btn_id = ctx.add_element(ctx.root_id, "button", "action-btn", 0.0, 0.0, 100.0, 40.0);

    Document::with_current(ctx.doc.clone(), || {
        let counter = Rc::new(RefCell::new(0));

        {
            let c = counter.clone();
            let _guard = observe_capture_events(move |_| {
                *c.borrow_mut() += 1;
            })
            .expect("registration must succeed");

            ctx.doc.dispatch_capture_event(CaptureEventKind::PointerDown, btn_id);
            assert_eq!(*counter.borrow(), 1, "Active observer must receive event");
        } // _guard drops here

        // Dispatch after drop
        ctx.doc.dispatch_capture_event(CaptureEventKind::PointerDown, btn_id);
        assert_eq!(
            *counter.borrow(),
            1,
            "Dropped WatcherGuard must cancel subscription and stop receiving events"
        );
    });
}

// 10. RAII / Teardown: WatcherGuard manual stop unregisters immediately
#[test]
fn test_watcher_guard_manual_stop_unregisters_immediately() {
    let ctx = TestContext::new();
    let btn_id = ctx.add_element(ctx.root_id, "button", "action-btn", 0.0, 0.0, 100.0, 40.0);

    Document::with_current(ctx.doc.clone(), || {
        let counter = Rc::new(RefCell::new(0));
        let c = counter.clone();

        let guard = observe_capture_events(move |_| {
            *c.borrow_mut() += 1;
        })
        .expect("registration must succeed");

        ctx.doc.dispatch_capture_event(CaptureEventKind::PointerDown, btn_id);
        assert_eq!(*counter.borrow(), 1);

        // Manually stop the guard
        guard.stop();

        ctx.doc.dispatch_capture_event(CaptureEventKind::PointerDown, btn_id);
        assert_eq!(
            *counter.borrow(),
            1,
            "Manually stopped guard must immediately unregister observer"
        );
    });
}

// 11. Gap 1 Live Engine Proof: Live OS UiEvent routing through Blitz EventDriver into watch_document_dismiss
#[test]
fn test_live_blitz_input_capture_pointer_and_keyboard() {
    let ctx = TestContext::new();
    let popover_id = ctx.add_element(ctx.root_id, "div", "menu-content", 50.0, 50.0, 200.0, 200.0);
    let _inside_btn = ctx.add_element(popover_id, "button", "inside-item", 60.0, 60.0, 80.0, 30.0);
    let _outside_btn = ctx.add_element(ctx.root_id, "button", "outside-item", 300.0, 300.0, 80.0, 30.0);

    // Initial layout resolve so elements are positioned
    ctx.doc.resolve(0.0);

    Document::with_current(ctx.doc.clone(), || {
        let dismiss_events = Rc::new(RefCell::new(Vec::new()));
        let d = dismiss_events.clone();

        let _guard = watch_document_dismiss(&["menu-content"], move |ev| {
            d.borrow_mut().push(ev);
        })
        .expect("watch_document_dismiss must succeed");

        // 1. Send live OS pointer down on outside button area (320.0, 320.0)
        let ptr_event = BlitzPointerEvent {
            id: BlitzPointerId::Mouse,
            is_primary: true,
            coords: PointerCoords {
                page_x: 320.0,
                page_y: 320.0,
                client_x: 320.0,
                client_y: 320.0,
                screen_x: 320.0,
                screen_y: 320.0,
            },
            button: MouseEventButton::Main,
            buttons: MouseEventButtons::Primary,
            mods: Modifiers::empty(),
            details: PointerDetails::default(),
            element: Default::default(),
            active_pointers: Default::default(),
        };

        // Live engine event routing via Blitz EventDriver (no manual dispatch_capture_event!)
        ctx.doc.handle_ui_event(UiEvent::PointerDown(ptr_event));

        assert_eq!(
            dismiss_events.borrow().len(),
            1,
            "Live OS pointer down must trigger dismiss watcher through EventDriver"
        );
        match &dismiss_events.borrow()[0] {
            DismissEvent::PointerDown { path_ids } => {
                assert!(
                    path_ids.contains(&"outside-item".to_string()),
                    "Captured path IDs must contain target element: {:?}",
                    path_ids
                );
            }
            other => panic!("Expected PointerDown, got {:?}", other),
        }

        // 2. Send live OS Escape keydown
        let key_event = BlitzKeyEvent {
            key: Key::Escape,
            code: Code::Escape,
            modifiers: Modifiers::empty(),
            location: Location::Standard,
            is_auto_repeating: false,
            is_composing: false,
            state: KeyState::Pressed,
            text: None,
        };

        // Live engine event routing via Blitz EventDriver
        ctx.doc.handle_ui_event(UiEvent::KeyDown(key_event));

        assert_eq!(
            dismiss_events.borrow().len(),
            2,
            "Live OS Escape keydown must trigger dismiss watcher through EventDriver"
        );
        assert_eq!(dismiss_events.borrow()[1], DismissEvent::Escape);
    });
}

// 12. Gap 2 Live Engine Proof: Live CSS animation completion dispatch in Stylo into watch_presence
#[test]
fn test_live_blitz_animation_completion_dispatch() {
    let ctx = TestContext::new();
    let _modal_id = ctx.add_element(ctx.root_id, "div", "modal-box", 0.0, 0.0, 200.0, 200.0);

    // Add author stylesheet with @keyframes animation
    ctx.base.borrow_mut().add_user_agent_stylesheet(r#"
        @keyframes fade-out {
            from { opacity: 1; }
            to { opacity: 0; }
        }
        #modal-box {
            animation: fade-out 0.1s;
        }
    "#);

    Document::with_current(ctx.doc.clone(), || {
        let presence_events = Rc::new(RefCell::new(Vec::new()));
        let p = presence_events.clone();

        let _guard = watch_presence("modal-box", 77, move |ev| {
            p.borrow_mut().push(ev);
        })
        .expect("watch_presence must succeed");

        // 1. Initial frame tick at t=0.0: Stylo initializes and starts the animation
        ctx.doc.resolve(0.0);
        assert!(
            presence_events.borrow().is_empty(),
            "Animation should be running at t=0.0 and not yet emit completion"
        );

        // 2. Advance time past the 0.1s animation duration to t=0.2:
        // Stylo detects animation.has_ended(now), transitions to Finished,
        // and emits the live animationend signal to the presence watcher
        ctx.doc.resolve(0.2);

        let events = presence_events.borrow();
        assert_eq!(
            events.len(),
            1,
            "Live Stylo animation end must trigger presence watcher without manual injection"
        );
        assert_eq!(
            events[0],
            PresenceEvent::AnimationEnd {
                cycle_id: 77,
                animation_name: "fade-out".to_string(),
            }
        );
    });
}

// 13. Layer 2: Floating auto-update nested ancestor scroll observation
#[test]
fn test_watch_floating_auto_update_nested_ancestor_scroll() {
    let ctx = TestContext::new();
    let scroll_id = ctx.add_scroll_container(ctx.root_id, "scroll-box", 0.0, 0.0, 400.0, 400.0);
    let _anchor_id = ctx.add_element(scroll_id, "button", "anchor-btn", 20.0, 20.0, 100.0, 30.0);
    let _floating_id = ctx.add_element(ctx.root_id, "div", "floating-content", 50.0, 50.0, 120.0, 80.0);

    ctx.doc.resolve(0.0);

    Document::with_current(ctx.doc.clone(), || {
        let updates = Rc::new(RefCell::new(Vec::new()));
        let u = updates.clone();

        let _guard = watch_floating_auto_update(&["anchor-btn"], "floating-content", move |ev| {
            u.borrow_mut().push(ev);
        })
        .expect("watch_floating_auto_update must succeed");

        // Scroll the ancestor element container
        let container_el = ctx.doc.element_by_id("scroll-box").expect("scroll-box element must exist");
        block_on(container_el.scroll_to(0.0, 50.0)).expect("element scroll_to must succeed");

        let events = updates.borrow();
        assert_eq!(events.len(), 1, "Ancestor container scroll must emit FloatingAutoUpdateEvent::Scroll");
        assert_eq!(events[0], FloatingAutoUpdateEvent::Scroll);
    });
}

// 14. Layer 2: Floating auto-update ancestor deduplication
#[test]
fn test_watch_floating_auto_update_ancestor_deduplication() {
    let ctx = TestContext::new();
    let shared_scroll_id = ctx.add_scroll_container(ctx.root_id, "shared-ancestor", 0.0, 0.0, 500.0, 500.0);
    let _anchor_id = ctx.add_element(shared_scroll_id, "button", "anchor-item", 10.0, 10.0, 80.0, 30.0);
    let _floating_id = ctx.add_element(shared_scroll_id, "div", "floating-item", 100.0, 10.0, 80.0, 30.0);

    ctx.doc.resolve(0.0);

    Document::with_current(ctx.doc.clone(), || {
        let updates = Rc::new(RefCell::new(Vec::new()));
        let u = updates.clone();

        let _guard = watch_floating_auto_update(&["anchor-item"], "floating-item", move |ev| {
            u.borrow_mut().push(ev);
        })
        .expect("watch_floating_auto_update must succeed");

        // Scroll the shared ancestor container once
        let container_el = ctx.doc.element_by_id("shared-ancestor").expect("shared-ancestor must exist");
        block_on(container_el.scroll_to(0.0, 30.0)).expect("element scroll_to must succeed");

        let events = updates.borrow();
        assert_eq!(
            events.len(),
            1,
            "Shared ancestor between anchor and content must be deduplicated and only emit once"
        );
        assert_eq!(events[0], FloatingAutoUpdateEvent::Scroll);
    });
}

// 15. Layer 2: Floating auto-update missing anchor error (fail fast)
#[test]
fn test_watch_floating_auto_update_missing_anchor_error() {
    let ctx = TestContext::new();
    let _floating_id = ctx.add_element(ctx.root_id, "div", "floating-content", 50.0, 50.0, 120.0, 80.0);

    ctx.doc.resolve(0.0);

    Document::with_current(ctx.doc.clone(), || {
        let res = watch_floating_auto_update(&["non-existent-anchor"], "floating-content", |_| {});
        match res {
            Err(Error::ElementNotFound(id)) => {
                assert_eq!(id, "non-existent-anchor", "Missing anchor must return Error::ElementNotFound with anchor ID");
            }
            other => panic!("Expected Err(Error::ElementNotFound), got {:?}", other),
        }
    });
}

// 16. Layer 2: Floating auto-update missing content tolerated
#[test]
fn test_watch_floating_auto_update_missing_content_tolerated() {
    let ctx = TestContext::new();
    let _anchor_id = ctx.add_element(ctx.root_id, "button", "anchor-btn", 10.0, 10.0, 80.0, 30.0);

    ctx.doc.resolve(0.0);

    Document::with_current(ctx.doc.clone(), || {
        // Missing content_id "unmounted-content" must be handled non-fatally
        let res = watch_floating_auto_update(&["anchor-btn"], "unmounted-content", |_| {});
        assert!(res.is_ok(), "Missing unmounted content must be tolerated non-fatally");
    });
}

// 17. Layer 2: Floating auto-update guard cleanup
#[test]
fn test_watch_floating_auto_update_guard_cleanup() {
    let ctx = TestContext::new();
    let scroll_id = ctx.add_scroll_container(ctx.root_id, "clean-scroll", 0.0, 0.0, 400.0, 400.0);
    let _anchor_id = ctx.add_element(scroll_id, "button", "clean-anchor", 10.0, 10.0, 80.0, 30.0);
    let _floating_id = ctx.add_element(ctx.root_id, "div", "clean-content", 50.0, 50.0, 120.0, 80.0);

    ctx.doc.resolve(0.0);

    Document::with_current(ctx.doc.clone(), || {
        let counter = Rc::new(RefCell::new(0));

        {
            let c = counter.clone();
            let _guard = watch_floating_auto_update(&["clean-anchor"], "clean-content", move |_| {
                *c.borrow_mut() += 1;
            })
            .expect("registration must succeed");

            let container_el = ctx.doc.element_by_id("clean-scroll").expect("clean-scroll must exist");
            block_on(container_el.scroll_to(0.0, 20.0)).expect("scroll_to must succeed");
            assert_eq!(*counter.borrow(), 1, "Active watcher must receive scroll event");
        } // _guard drops here and cleans up all inner scroll & resize listeners

        // Scroll again after guard drop - sleep beyond coalesce window to be sure
        std::thread::sleep(std::time::Duration::from_millis(20));
        let container_el = ctx.doc.element_by_id("clean-scroll").expect("clean-scroll must exist");
        block_on(container_el.scroll_to(0.0, 80.0)).expect("scroll_to must succeed");

        assert_eq!(
            *counter.borrow(),
            1,
            "Dropped WatcherGuard must cancel all ancestor scroll listeners and receive no further events"
        );
    });
}

