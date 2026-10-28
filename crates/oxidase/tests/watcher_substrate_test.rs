#![cfg(feature = "native")]

use blitz_traits::events::{
    BlitzKeyEvent, BlitzPointerEvent, BlitzPointerId, KeyState, MouseEventButton,
    MouseEventButtons, PointerCoords, PointerDetails, UiEvent,
};
use dioxus::prelude::keyboard_types::{Code, Key, Location, Modifiers};
use blitz_dom::{Attribute, BaseDocument, DocumentConfig, QualName, local_name, ns};
use oxidase::dom::observer::{
    observe_capture_events, observe_element_resize, observe_element_scroll,
    observe_form_reset, observe_scroll, observe_transition_lifecycle,
    CaptureEventKind, TransitionLifecycleEvent,
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

    #[allow(clippy::too_many_arguments)]
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
                        ),
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
                        ),
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

        // 1. Initial size change emits update on frame tick
        ctx.update_element_layout(anchor_id, 50.0, 50.0, 120.0, 40.0);
        ctx.doc.notify_layout_observers();
        oxidase::frame::tick(std::time::Duration::from_millis(16));
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

        // Trailing frame delivery via tick
        oxidase::frame::tick(std::time::Duration::from_millis(16));
        assert_eq!(
            updates.borrow().len(),
            2,
            "Trailing update must be delivered on frame tick"
        );
        assert_eq!(updates.borrow()[1], FloatingAutoUpdateEvent::Update);

        // 3. Scroll event emission when viewport/ancestor scroll changes
        ctx.doc.set_viewport_scroll(10.0, 50.0);
        oxidase::frame::tick(std::time::Duration::from_millis(16));
        assert_eq!(updates.borrow().len(), 3);
        assert_eq!(updates.borrow()[2], FloatingAutoUpdateEvent::Scroll);
    });
}

// 6b. Layer 2: Floating auto-update trailing frame settling (Update dominates Scroll)
#[test]
fn test_watch_floating_auto_update_trailing_frame_settling() {
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

        // 1. Initial event fired on frame tick (leading emit: Scroll)
        ctx.doc.set_viewport_scroll(10.0, 20.0);
        oxidase::frame::tick(std::time::Duration::from_millis(16));
        assert_eq!(updates.borrow().len(), 1);
        assert_eq!(updates.borrow()[0], FloatingAutoUpdateEvent::Scroll);

        // 2. Rapid events within 16ms frame window: Scroll followed by Update
        ctx.doc.set_viewport_scroll(20.0, 40.0);
        ctx.update_element_layout(anchor_id, 50.0, 50.0, 140.0, 40.0);
        ctx.doc.notify_layout_observers();

        // Still 1 event because rapid events coalesce into pending trailing
        assert_eq!(
            updates.borrow().len(),
            1,
            "Rapid events within 16ms must coalesce into trailing queue"
        );

        // 3. After crate::frame::tick(), trailing event fires with Update dominating Scroll
        oxidase::frame::tick(std::time::Duration::from_millis(16));
        assert_eq!(updates.borrow().len(), 2);
        assert_eq!(
            updates.borrow()[1],
            FloatingAutoUpdateEvent::Update,
            "Update event must dominate Scroll in coalesced trailing delivery"
        );
    });
}

// 6c. Layer 2: Floating auto-update re-entrancy safe without RefCell borrow panic
#[test]
fn test_watch_floating_auto_update_reentrancy_safe() {
    let ctx = TestContext::new();
    let anchor_id = ctx.add_element(ctx.root_id, "button", "anchor", 50.0, 50.0, 100.0, 40.0);
    let _floating_id = ctx.add_element(ctx.root_id, "div", "floating", 50.0, 95.0, 150.0, 200.0);

    Document::with_current(ctx.doc.clone(), || {
        let events = Rc::new(RefCell::new(Vec::new()));
        let evs = events.clone();

        let doc_clone = ctx.doc.clone();
        let base_clone = ctx.base.clone();
        let reentrant_fired = Rc::new(RefCell::new(false));
        let rf = reentrant_fired.clone();

        let _guard = watch_floating_auto_update(&["anchor"], "floating", move |ev| {
            evs.borrow_mut().push(ev);

            // Mutate element bounds inside the auto-update callback on the first event
            if !*rf.borrow() {
                *rf.borrow_mut() = true;
                // Mutating element layout and notifying observers triggers synchronous ResizeObserver notification
                if let Some(node) = base_clone.borrow_mut().get_node_mut(anchor_id) {
                    let layout = &mut node.layout_data_mut().final_layout;
                    layout.size.width = 180.0;
                }
                doc_clone.notify_layout_observers();
            }
        })
        .expect("watch_floating_auto_update must succeed");

        // 1. Initial size change triggers leading callback, which mutates bounds re-entrantly
        ctx.update_element_layout(anchor_id, 50.0, 50.0, 130.0, 40.0);
        ctx.doc.notify_layout_observers();
        oxidase::frame::tick(std::time::Duration::from_millis(16));

        assert_eq!(
            events.borrow().len(),
            1,
            "Initial leading event emitted; re-entrant update queued safely"
        );
        assert_eq!(events.borrow()[0], FloatingAutoUpdateEvent::Update);
        assert!(*reentrant_fired.borrow(), "Re-entrant mutation block executed");

        // 2. Trailing frame tick delivers the re-entrant update safely
        oxidase::frame::tick(std::time::Duration::from_millis(16));
        assert_eq!(
            events.borrow().len(),
            2,
            "Trailing frame must successfully deliver re-entrantly queued update"
        );
        assert_eq!(events.borrow()[1], FloatingAutoUpdateEvent::Update);
    });
}

// 6d. Layer 2: Floating auto-update cancellation cancels pending trailing frame
#[test]
fn test_watch_floating_auto_update_cancellation_cancels_trailing() {
    let ctx = TestContext::new();
    let anchor_id = ctx.add_element(ctx.root_id, "button", "anchor", 50.0, 50.0, 100.0, 40.0);
    let _floating_id = ctx.add_element(ctx.root_id, "div", "floating", 50.0, 95.0, 150.0, 200.0);

    Document::with_current(ctx.doc.clone(), || {
        let updates = Rc::new(RefCell::new(Vec::new()));
        let u = updates.clone();

        let guard = watch_floating_auto_update(&["anchor"], "floating", move |ev| {
            u.borrow_mut().push(ev);
        })
        .expect("watch_floating_auto_update must succeed");

        // 1. Initial event
        ctx.update_element_layout(anchor_id, 50.0, 50.0, 120.0, 40.0);
        ctx.doc.notify_layout_observers();
        oxidase::frame::tick(std::time::Duration::from_millis(16));
        assert_eq!(updates.borrow().len(), 1);

        // 2. Second event within 16ms window queues trailing frame
        ctx.update_element_layout(anchor_id, 50.0, 50.0, 140.0, 40.0);
        ctx.doc.notify_layout_observers();
        assert_eq!(updates.borrow().len(), 1);

        // 3. Drop guard before frame tick
        drop(guard);

        // 4. Tick frame
        oxidase::frame::tick(std::time::Duration::from_millis(16));

        // Must still be 1; pending trailing frame was cancelled by drop
        assert_eq!(
            updates.borrow().len(),
            1,
            "Dropping WatcherGuard before tick must cancel pending trailing frame"
        );
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

#[test]
fn test_watch_presence_synthetic_transition_dispatch() {
    let ctx = TestContext::new();
    let _dialog_id = ctx.add_element(ctx.root_id, "div", "trans-dialog-el", 0.0, 0.0, 300.0, 200.0);

    Document::with_current(ctx.doc.clone(), || {
        let presence_events = Rc::new(RefCell::new(Vec::new()));
        let p = presence_events.clone();

        let _guard = watch_presence("trans-dialog-el", 101, move |ev| {
            p.borrow_mut().push(ev);
        })
        .expect("watch_presence must succeed");

        // Dispatch transition end for cycle 101
        ctx.doc.dispatch_transition_end("trans-dialog-el", "opacity");

        let events = presence_events.borrow();
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0],
            PresenceEvent::TransitionEnd {
                cycle_id: 101,
                property_name: "opacity".to_string(),
            }
        );
        drop(events);

        // Test transition cancel with another watcher
        let cancel_events = Rc::new(RefCell::new(Vec::new()));
        let c = cancel_events.clone();
        let _guard_cancel = watch_presence("trans-dialog-el", 102, move |ev| {
            c.borrow_mut().push(ev);
        })
        .expect("watch_presence must succeed");

        ctx.doc.dispatch_transition_cancel("trans-dialog-el", "transform");

        let events = cancel_events.borrow();
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0],
            PresenceEvent::TransitionCancel {
                cycle_id: 102,
                property_name: "transform".to_string(),
            }
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

// 12b. Gap 2 Live Engine Proof: Live CSS transition completion dispatch in Stylo into watch_presence
#[test]
fn test_live_blitz_transition_completion_dispatch() {
    let ctx = TestContext::new();
    let modal_id = ctx.add_element(ctx.root_id, "div", "modal-box", 0.0, 0.0, 200.0, 200.0);

    // Set up stylesheet with transition: opacity 0.1s; opacity: 1;
    ctx.base.borrow_mut().add_user_agent_stylesheet(r#"
        #modal-box {
            transition: opacity 0.1s linear;
            opacity: 1;
        }
    "#);

    Document::with_current(ctx.doc.clone(), || {
        let presence_events = Rc::new(RefCell::new(Vec::new()));
        let p = presence_events.clone();

        // Initial tick at t=0.0
        ctx.doc.resolve(0.0);

        let _guard = watch_presence("modal-box", 88, move |ev| {
            p.borrow_mut().push(ev);
        })
        .expect("watch_presence must succeed");

        // Mutate style: set opacity to 0
        ctx.base.borrow_mut().mutate().set_style_property(modal_id, "opacity", "0");

        // Tick at t=0.0 to start transition
        ctx.doc.resolve(0.0);
        assert!(
            presence_events.borrow().is_empty(),
            "Transition should be running at t=0.0 and not yet emit completion"
        );

        // Advance time past the 0.1s transition duration to t=0.2
        ctx.doc.resolve(0.2);

        let events = presence_events.borrow();
        assert_eq!(
            events.len(),
            1,
            "Live Blitz transition end must trigger presence watcher"
        );
        assert_eq!(
            events[0],
            PresenceEvent::TransitionEnd {
                cycle_id: 88,
                property_name: "opacity".to_string(),
            }
        );
        drop(events);

        // Verify pending_animation_events is NOT empty (events preserved for flush_animation_events)
        assert!(
            !ctx.base.borrow().pending_animation_events.is_empty(),
            "Pending animation/transition events must be preserved for flush_animation_events"
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
        oxidase::frame::tick(std::time::Duration::from_millis(16));

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
        oxidase::frame::tick(std::time::Duration::from_millis(16));

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
            oxidase::frame::tick(std::time::Duration::from_millis(16));
            assert_eq!(*counter.borrow(), 1, "Active watcher must receive scroll event");
        } // _guard drops here and cleans up all inner scroll & resize listeners

        // Scroll again after guard drop - sleep beyond coalesce window to be sure
        std::thread::sleep(std::time::Duration::from_millis(20));
        let container_el = ctx.doc.element_by_id("clean-scroll").expect("clean-scroll must exist");
        block_on(container_el.scroll_to(0.0, 80.0)).expect("scroll_to must succeed");
        oxidase::frame::tick(std::time::Duration::from_millis(16));

        assert_eq!(
            *counter.borrow(),
            1,
            "Dropped WatcherGuard must cancel all ancestor scroll listeners and receive no further events"
        );
    });
}

// 18. Transition lifecycle strict one-shot semantics
#[test]
fn test_transition_lifecycle_strict_one_shot_semantics() {
    let ctx = TestContext::new();
    let _dialog_id = ctx.add_element(ctx.root_id, "div", "dialog-box", 10.0, 10.0, 200.0, 100.0);

    Document::with_current(ctx.doc.clone(), || {
        let invocations = Rc::new(RefCell::new(Vec::new()));
        let inv = invocations.clone();

        let _guard = observe_transition_lifecycle("dialog-box", 50, move |ev| {
            inv.borrow_mut().push(ev);
        })
        .expect("registration must succeed");

        // Primary signal: animation end
        ctx.doc.dispatch_animation_end("dialog-box", "fade-in");

        // Subsequent signals must safely no-op due to strict one-shot removal
        ctx.doc.dispatch_animation_cancel("dialog-box", "fade-in");
        ctx.doc.dispatch_transition_fallback("dialog-box");

        // Sleep to let fallback timer elapse
        std::thread::sleep(std::time::Duration::from_millis(80));

        let events = invocations.borrow();
        assert_eq!(
            events.len(),
            1,
            "Strict one-shot semantics must guarantee at-most-once callback invocation"
        );
        assert_eq!(
            events[0],
            TransitionLifecycleEvent::AnimationEnd {
                animation_name: "fade-in".into()
            }
        );
    });
}

// 19. Persistent observers two-phase decoupled dispatch and re-entrancy safety
#[test]
fn test_observer_two_phase_dispatch_reentrancy_safe() {
    let ctx = TestContext::new();
    let form_id = ctx.add_element(ctx.root_id, "form", "test-form", 0.0, 0.0, 300.0, 300.0);
    let input_id = ctx.add_element(form_id, "input", "test-input", 10.0, 10.0, 100.0, 30.0);
    let _scroll_id = ctx.add_scroll_container(ctx.root_id, "test-scroll-box", 0.0, 0.0, 200.0, 200.0);

    ctx.doc.resolve(0.0);

    Document::with_current(ctx.doc.clone(), || {
        // A. Capture observer re-entrancy: callback accesses Document and registers another observer
        {
            let doc_clone = ctx.doc.clone();
            let sub_registered = Rc::new(RefCell::new(false));
            let sub_reg = sub_registered.clone();

            let _guard = observe_capture_events(move |_ev| {
                // Querying document methods inside capture observer must not panic
                let rect = doc_clone.measure_rect("test-input");
                assert!(rect.is_some());

                // Registering an observer from within a callback must not panic
                let _sub = observe_capture_events(|_| {});
                *sub_reg.borrow_mut() = true;
            })
            .expect("capture observer registration must succeed");

            ctx.doc.dispatch_capture_event(CaptureEventKind::PointerDown, input_id);
            assert!(*sub_registered.borrow(), "Capture observer re-entrancy must execute cleanly");
        }

        // B. Form reset re-entrancy: callback inspects DOM and calls Document methods
        {
            let doc_clone = ctx.doc.clone();
            let form_reset_fired = Rc::new(RefCell::new(false));
            let fr_fired = form_reset_fired.clone();

            let _guard = observe_form_reset("test-input", Some("test-form"), move || {
                let active = doc_clone.element_by_id("test-input");
                assert!(active.is_some());
                *fr_fired.borrow_mut() = true;
            })
            .expect("form reset observer registration must succeed");

            ctx.doc.dispatch_form_reset("test-form");
            assert!(*form_reset_fired.borrow(), "Form reset observer must execute cleanly");
        }

        // C. Scroll observer re-entrancy: document viewport scroll callback calls document methods
        {
            let doc_clone = ctx.doc.clone();
            let scroll_fired = Rc::new(RefCell::new(false));
            let s_fired = scroll_fired.clone();

            let _guard = observe_scroll(move || {
                let vp = doc_clone.viewport();
                assert!(vp.width > 0.0);
                *s_fired.borrow_mut() = true;
            })
            .expect("scroll observer registration must succeed");

            ctx.doc.set_viewport_scroll(10.0, 20.0);
            assert!(*scroll_fired.borrow(), "Viewport scroll observer must execute cleanly");
        }

        // D. Element scroll observer re-entrancy: guard self-dropping and count inspection inside callback
        {
            let guard_holder: Rc<RefCell<Option<WatcherGuard>>> = Rc::new(RefCell::new(None));
            let holder_clone = guard_holder.clone();
            let doc_clone = ctx.doc.clone();
            let dropped_inside = Rc::new(RefCell::new(false));
            let d_inside = dropped_inside.clone();

            let guard = observe_element_scroll("test-scroll-box", move || {
                // Drop our own guard from inside the callback
                let _ = holder_clone.borrow_mut().take();
                // Check observer count under borrow
                let _count = doc_clone.element_scroll_observer_count(Some("test-scroll-box"));
                *d_inside.borrow_mut() = true;
            })
            .expect("element scroll observer registration must succeed");

            *guard_holder.borrow_mut() = Some(guard);

            ctx.doc.dispatch_element_scroll("test-scroll-box");
            assert!(*dropped_inside.borrow(), "Self-cleanup during element scroll dispatch must succeed");
            assert_eq!(
                ctx.doc.element_scroll_observer_count(Some("test-scroll-box")),
                0,
                "Observer must be cleanly removed after self-drop"
            );
        }
    });
}

// 20. Transition lifecycle guard drop inside callback regression
#[test]
fn test_transition_lifecycle_guard_drop_inside_callback_no_panic() {
    let ctx = TestContext::new();
    let _dialog_id = ctx.add_element(ctx.root_id, "div", "trans-dialog", 10.0, 10.0, 200.0, 100.0);

    Document::with_current(ctx.doc.clone(), || {
        let guard_cell: Rc<RefCell<Option<WatcherGuard>>> = Rc::new(RefCell::new(None));
        let guard_cell_clone = guard_cell.clone();
        let callback_executed = Rc::new(RefCell::new(false));
        let cb_flag = callback_executed.clone();

        let guard = observe_transition_lifecycle("trans-dialog", 500, move |_ev| {
            // Drop the WatcherGuard from inside the callback
            let dropped_guard = guard_cell_clone.borrow_mut().take();
            assert!(dropped_guard.is_some(), "Guard must be present before dropping");
            drop(dropped_guard);
            *cb_flag.borrow_mut() = true;
        })
        .expect("transition lifecycle registration must succeed");

        *guard_cell.borrow_mut() = Some(guard);

        // Dispatch animation completion
        ctx.doc.dispatch_animation_end("trans-dialog", "fade-out");

        assert!(
            *callback_executed.borrow(),
            "Callback must execute cleanly and drop guard without panic"
        );
        assert!(
            guard_cell.borrow().is_none(),
            "Guard must be dropped after callback execution"
        );
    });
}

// 21. Live BaseDocument upgrade and authoritative presence resolution
#[test]
fn test_hosted_window_live_base_document_upgrade_and_presence_resolve() {
    Document::set_current(None);

    // 1. Initialize ambient document context via ensure_document_context() (simulating #[oxidase::main] before window mount)
    oxidase::launch::ensure_document_context();

    // 2. Confirm Document::current().unwrap().is_authoritative() == false
    let doc = Document::current().expect("ambient Document context must be present");
    assert!(
        !doc.is_authoritative(),
        "Ambient unattached document must not be authoritative before window mount"
    );
    assert!(
        !Document::current().unwrap().is_authoritative(),
        "Document::current() must reflect non-authoritative status"
    );

    // 3. Create a live BaseDocument with an element <div id="hosted-card"> and styles
    let mut base_doc = BaseDocument::new(DocumentConfig::default());
    {
        let mut vp = base_doc.viewport_mut();
        vp.window_size = (1920, 1080);
        vp.hidpi_scale = 1.0;
    }
    let doc_root = base_doc.root_node().id;
    let _card_id = {
        let mut mutator = base_doc.mutate();
        let card_id = mutator.create_element(
            QualName::new(None, ns!(), local_name!("div")),
            vec![
                Attribute {
                    name: QualName::new(None, ns!(), local_name!("id")),
                    value: "hosted-card".into(),
                },
                Attribute {
                    name: QualName::new(None, ns!(), local_name!("style")),
                    value: "width: 100px; height: 100px; margin: 0;".into(),
                },
            ],
        );
        mutator.append_children(doc_root, &[card_id]);
        card_id
    };
    let live_base = Rc::new(RefCell::new(base_doc));

    // 4. Call doc.bind_live_base(live_base) (simulating HostedRootWrapper::onmounted)
    doc.bind_live_base(live_base.clone());

    // 5. Verify doc.is_authoritative() == true, Document::current().unwrap().is_authoritative() == true, and doc.element_by_id("hosted-card").is_some()
    assert!(
        doc.is_authoritative(),
        "doc.is_authoritative() must be true after bind_live_base"
    );
    assert!(
        Document::current().unwrap().is_authoritative(),
        "Document::current().unwrap().is_authoritative() must be true after bind_live_base"
    );
    assert!(
        doc.element_by_id("hosted-card").is_some(),
        "Element #hosted-card must be found via element_by_id"
    );

    Document::set_current(None);
}

// 22. Hosted frame step pumping external animation completion
#[test]
fn test_hosted_window_step_hosted_frame_pump_external_animation_completion() {
    Document::set_current(None);

    // 1. Bind live BaseDocument to Document::current()
    oxidase::launch::ensure_document_context();
    let doc = Document::current().expect("ambient document must exist");
    let ctx = TestContext::new();
    doc.bind_live_base(ctx.base.clone());
    assert!(
        doc.is_authoritative(),
        "Bound document must be authoritative"
    );
    assert!(
        Document::current().unwrap().is_authoritative(),
        "Document::current() must reflect authoritative status"
    );

    // 2. Add an element with CSS animation or transition (e.g. opacity transition)
    let target_id = ctx.add_element(ctx.root_id, "div", "target-el", 0.0, 0.0, 200.0, 200.0);
    ctx.base.borrow_mut().add_user_agent_stylesheet(r#"
        #target-el {
            transition: opacity 0.1s linear;
            opacity: 1;
        }
    "#);

    // Initial external resolve at t=0.0 to compute initial styles (opacity: 1)
    ctx.base.borrow_mut().resolve(0.0);

    // Track host redraw requests during animation
    let redraw_count = Rc::new(RefCell::new(0usize));
    let rc = redraw_count.clone();
    let _redraw_guard = oxidase::launch::bind_host_redraw_requester(move || {
        *rc.borrow_mut() += 1;
    });

    // 3. Attach watch_presence("target-el", 42, ...)
    let presence_events = Rc::new(RefCell::new(Vec::new()));
    let p = presence_events.clone();
    let _guard = watch_presence("target-el", 42, move |ev| {
        p.borrow_mut().push(ev);
    })
    .expect("watch_presence must succeed");

    // 4. Mutate style property on BaseDocument directly (mutator.set_style_property(..., "opacity", "0"))
    ctx.base.borrow_mut().mutate().set_style_property(target_id, "opacity", "0");

    // 5. Call base.borrow_mut().resolve(0.0) directly (simulating external blitz-shell View::redraw without calling Document::resolve)
    ctx.base.borrow_mut().resolve(0.0);
    assert!(
        presence_events.borrow().is_empty(),
        "Transition should be running at t=0.0 and not yet emit completion"
    );
    assert!(
        ctx.base.borrow().is_animating(),
        "BaseDocument must be animating during active transition"
    );

    // Calling step_hosted_frame while animating requests host redraw even with 0 frame subscribers
    oxidase::launch::step_hosted_frame();
    assert_eq!(
        *redraw_count.borrow(),
        1,
        "is_animating must request host redraw in step_hosted_frame"
    );

    // 6. Advance time externally: base.borrow_mut().resolve(0.2)
    ctx.base.borrow_mut().resolve(0.2);
    assert!(
        !ctx.base.borrow().is_animating(),
        "BaseDocument must finish animating after 0.2s duration elapsed"
    );
    assert!(
        presence_events.borrow().is_empty(),
        "Events must not be pumped until step_hosted_frame or resolve"
    );

    // 7. Call oxidase::launch::step_hosted_frame() (simulating WindowEvent::RedrawRequested)
    oxidase::launch::step_hosted_frame();

    // 8. Verify watch_presence received PresenceEvent::TransitionEnd (or AnimationEnd) automatically without ever having called Document::resolve()
    let events = presence_events.borrow();
    assert_eq!(
        events.len(),
        1,
        "watch_presence must receive TransitionEnd via step_hosted_frame without Document::resolve"
    );
    assert_eq!(
        events[0],
        PresenceEvent::TransitionEnd {
            cycle_id: 42,
            property_name: "opacity".to_string(),
        }
    );
    drop(events);

    // 9. Verify pending_animation_events was NOT drained
    assert!(
        !ctx.base.borrow().pending_animation_events.is_empty(),
        "pending_animation_events must not be drained"
    );

    // After animation finished and no frame subscribers, step_hosted_frame idles (no additional redraw requested)
    assert_eq!(
        *redraw_count.borrow(),
        1,
        "step_hosted_frame must idle once animation completes"
    );

    Document::set_current(None);
}

// 23. Parity Proof: Attribute toggle data-state matches CSS selector [data-state="closed"] with keyframes animation
#[test]
fn test_attribute_toggle_data_state_css_keyframes_animation_presence() {
    let ctx = TestContext::new();
    let _modal_id = ctx.add_element(ctx.root_id, "div", "anim-state-box", 0.0, 0.0, 200.0, 200.0);

    // CSS rule using attribute selector [data-state="closed"] with @keyframes fade-out
    ctx.base.borrow_mut().add_user_agent_stylesheet(r#"
        @keyframes fade-out {
            from { opacity: 1; }
            to { opacity: 0; }
        }
        [data-state="open"] {
            opacity: 1;
        }
        [data-state="closed"] {
            animation: fade-out 0.1s;
        }
    "#);

    Document::with_current(ctx.doc.clone(), || {
        let el = ctx
            .doc
            .element_by_id("anim-state-box")
            .expect("anim-state-box element must exist");

        // Set initial state to "open"
        el.set_attribute("data-state", "open");
        assert_eq!(el.get_attribute("data-state").as_deref(), Some("open"));

        // Initial layout and style resolve
        ctx.doc.resolve(0.0);

        let presence_events = Rc::new(RefCell::new(Vec::new()));
        let p = presence_events.clone();

        let _guard = watch_presence("anim-state-box", 301, move |ev| {
            p.borrow_mut().push(ev);
        })
        .expect("watch_presence must succeed");

        // Toggle data-state attribute from "open" to "closed" via Element::set_attribute
        el.set_attribute("data-state", "closed");
        assert_eq!(el.get_attribute("data-state").as_deref(), Some("closed"));

        // Resolve at t=0.0: Stylo matches [data-state="closed"] selector and starts fade-out keyframes animation
        ctx.doc.resolve(0.0);
        assert!(
            presence_events.borrow().is_empty(),
            "Animation should be running at t=0.0 and not yet emit completion"
        );

        // Advance time past the 0.1s duration to t=0.2
        ctx.doc.resolve(0.2);

        let events = presence_events.borrow();
        assert_eq!(
            events.len(),
            1,
            "watch_presence must receive AnimationEnd upon CSS keyframes animation completion triggered by data-state toggle"
        );
        assert_eq!(
            events[0],
            PresenceEvent::AnimationEnd {
                cycle_id: 301,
                animation_name: "fade-out".to_string(),
            }
        );
    });
}

// 24. Parity Proof: Attribute toggle data-state matches CSS selector [data-state="closed"] with transition
#[test]
fn test_attribute_toggle_data_state_css_transition_presence() {
    let ctx = TestContext::new();
    let _modal_id = ctx.add_element(ctx.root_id, "div", "trans-state-box", 0.0, 0.0, 200.0, 200.0);

    // CSS rule using attribute selector [data-state="closed"] with transition: opacity 0.1s
    ctx.base.borrow_mut().add_user_agent_stylesheet(r#"
        [data-state="open"] {
            opacity: 1;
            transition: opacity 0.1s linear;
        }
        [data-state="closed"] {
            opacity: 0;
            transition: opacity 0.1s linear;
        }
    "#);

    Document::with_current(ctx.doc.clone(), || {
        let el = ctx
            .doc
            .element_by_id("trans-state-box")
            .expect("trans-state-box element must exist");

        // Set initial state to "open"
        el.set_attribute("data-state", "open");
        assert_eq!(el.get_attribute("data-state").as_deref(), Some("open"));

        // Initial resolve to establish baseline style (opacity: 1)
        ctx.doc.resolve(0.0);

        let presence_events = Rc::new(RefCell::new(Vec::new()));
        let p = presence_events.clone();

        let _guard = watch_presence("trans-state-box", 402, move |ev| {
            p.borrow_mut().push(ev);
        })
        .expect("watch_presence must succeed");

        // Toggle data-state attribute from "open" to "closed" via Element::set_attribute
        el.set_attribute("data-state", "closed");
        assert_eq!(el.get_attribute("data-state").as_deref(), Some("closed"));

        // Resolve at t=0.0: Stylo matches [data-state="closed"], opacity changes 1 -> 0, starting transition
        ctx.doc.resolve(0.0);
        assert!(
            presence_events.borrow().is_empty(),
            "Transition should be running at t=0.0 and not yet emit completion"
        );

        // Advance time past the 0.1s duration to t=0.2
        ctx.doc.resolve(0.2);

        let events = presence_events.borrow();
        assert_eq!(
            events.len(),
            1,
            "watch_presence must receive TransitionEnd upon CSS transition completion triggered by data-state toggle"
        );
        assert_eq!(
            events[0],
            PresenceEvent::TransitionEnd {
                cycle_id: 402,
                property_name: "opacity".to_string(),
            }
        );
    });
}

// 25. End-to-end dioxus_native / DioxusDocument integration test:
// Coexistence between Oxidase watch_presence and Dioxus flush_animation_events.
#[test]
fn test_dioxus_document_watch_presence_and_flush_animation_events_coexistence() {
    use dioxus::prelude::*;

    #[derive(Clone, Default)]
    struct CoexistTransitionLog(Rc<RefCell<Vec<String>>>);

    fn coexist_app() -> Element {
        let log = consume_context::<CoexistTransitionLog>();
        rsx! {
            div {
                id: "coexist-modal",
                ontransitionend: move |evt| {
                    let debug = format!("{:?}", evt.data());
                    // TransitionData debug format is: TransitionData { property_name: "opacity", ... }
                    let prop = if debug.contains("property_name: \"opacity\"") {
                        "opacity".to_string()
                    } else {
                        debug
                    };
                    log.0.borrow_mut().push(prop);
                },
                "Coexist Modal"
            }
        }
    }

    let log = CoexistTransitionLog::default();
    let vdom = VirtualDom::new(coexist_app).with_root_context(log.clone());
    let mut doc = dioxus_native::DioxusDocument::new(vdom, DocumentConfig::default());
    doc.initial_build();

    let oxidase_doc = oxidase::dom::Document::from_base(doc.inner.clone());
    oxidase::dom::Document::with_current(oxidase_doc.clone(), || {
        // 4. Inject stylesheet with transition rules
        doc.inner.borrow_mut().add_user_agent_stylesheet(r#"
            #coexist-modal {
                transition: opacity 0.1s linear;
                opacity: 1;
            }
            #coexist-modal[data-state="closed"] {
                opacity: 0;
            }
        "#);

        // 5. Tick initial frame at t=0.0
        oxidase_doc.resolve(0.0);

        // 6. Attach Oxidase watch_presence("coexist-modal", 501, ...) logging PresenceEvents
        let presence_events = Rc::new(RefCell::new(Vec::new()));
        let p1 = presence_events.clone();
        let _guard1 = watch_presence("coexist-modal", 501, move |ev| {
            p1.borrow_mut().push(ev);
        })
        .expect("watch_presence must succeed");

        // 7. Toggle el.set_attribute("data-state", "closed") using Oxidase Element::set_attribute
        let el = oxidase_doc
            .element_by_id("coexist-modal")
            .expect("coexist-modal element must exist");
        el.set_attribute("data-state", "closed");
        assert_eq!(el.get_attribute("data-state").as_deref(), Some("closed"));

        // 8. Tick frame at t=0.0 (starts transition), advance time to t=0.2 (transition completes)
        oxidase_doc.resolve(0.0);
        assert!(
            presence_events.borrow().is_empty(),
            "Transition should be running at t=0.0 and not yet emit completion"
        );

        oxidase_doc.resolve(0.2);

        // 9. Assert Oxidase watch_presence received PresenceEvent::TransitionEnd { cycle_id: 501, property_name: "opacity" }
        assert_eq!(
            *presence_events.borrow(),
            vec![PresenceEvent::TransitionEnd {
                cycle_id: 501,
                property_name: "opacity".to_string(),
            }],
            "watch_presence must receive TransitionEnd for cycle 501"
        );

        // 10. Assert doc.inner.borrow().pending_animation_events was NOT drained by Oxidase (events preserved for Dioxus)
        assert!(
            !doc.inner.borrow().pending_animation_events.is_empty(),
            "doc.inner pending_animation_events was NOT drained by Oxidase"
        );
        assert!(
            doc.inner.borrow().pending_animation_events.iter().any(|e| {
                matches!(&e.event, blitz_traits::events::DomEventData::TransitionEnd(t) if t.property_name == "opacity")
            }),
            "TransitionEnd event must be preserved in pending_animation_events for Dioxus"
        );

        // 11. Call doc.flush_animation_events()
        doc.flush_animation_events();

        // 12. Assert the Dioxus component's ontransitionend handler received the event (e.g. property_name: "opacity")
        assert_eq!(
            *log.0.borrow(),
            vec!["opacity".to_string()],
            "Dioxus component ontransitionend handler must receive the event"
        );

        // 13. Assert doc.inner.borrow().pending_animation_events is now drained and empty
        assert!(
            doc.inner.borrow().pending_animation_events.is_empty(),
            "doc.inner pending_animation_events must be drained and empty after Dioxus flush"
        );

        // 14. Run a second transition cycle (e.g. el.set_attribute("data-state", "open"), tick at 0.2, advance to 0.4),
        // proving Oxidase resets its last_synced_len cursor gracefully and dispatches subsequent events to watch_presence without desynchronization or starvation.
        let presence_events_cycle2 = Rc::new(RefCell::new(Vec::new()));
        let p2 = presence_events_cycle2.clone();
        let _guard2 = watch_presence("coexist-modal", 502, move |ev| {
            p2.borrow_mut().push(ev);
        })
        .expect("watch_presence for cycle 502 must succeed");

        el.set_attribute("data-state", "open");
        assert_eq!(el.get_attribute("data-state").as_deref(), Some("open"));

        // Tick at t=0.2 (transition begins: opacity 0 -> 1)
        oxidase_doc.resolve(0.2);
        assert!(
            presence_events_cycle2.borrow().is_empty(),
            "Cycle 2 transition running at t=0.2 and not yet emit completion"
        );

        // Advance to t=0.4 (transition completes)
        oxidase_doc.resolve(0.4);

        // Assert Oxidase watch_presence received cycle 502 event
        assert_eq!(
            *presence_events_cycle2.borrow(),
            vec![PresenceEvent::TransitionEnd {
                cycle_id: 502,
                property_name: "opacity".to_string(),
            }],
            "watch_presence must receive TransitionEnd for cycle 502"
        );

        // Assert pending_animation_events has the new event preserved for Dioxus
        assert!(
            !doc.inner.borrow().pending_animation_events.is_empty(),
            "doc.inner pending_animation_events preserved for Dioxus in cycle 2"
        );
        assert!(
            doc.inner.borrow().pending_animation_events.iter().any(|e| {
                matches!(&e.event, blitz_traits::events::DomEventData::TransitionEnd(t) if t.property_name == "opacity")
            }),
            "Cycle 2 TransitionEnd must be preserved in pending_animation_events for Dioxus"
        );

        // Call doc.flush_animation_events()
        doc.flush_animation_events();

        // Assert Dioxus component received the second event
        assert_eq!(
            *log.0.borrow(),
            vec!["opacity".to_string(), "opacity".to_string()],
            "Dioxus component ontransitionend handler must receive both cycle 1 and cycle 2 events"
        );

        // Assert pending_animation_events is drained and empty again
        assert!(
            doc.inner.borrow().pending_animation_events.is_empty(),
            "doc.inner pending_animation_events must be drained and empty after cycle 2 Dioxus flush"
        );
    });
}

