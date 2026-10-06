#![cfg(feature = "native")]

use blitz_dom::{Attribute, BaseDocument, DocumentConfig, NodeId, QualName, local_name, ns};
use oxidase::prelude::*;
use oxidase_floating_ui::compute_position;
use oxidase_floating_ui::geometry::{ElementOrVirtual, Placement};
use oxidase_floating_ui::platform::NativeFloatingPlatform;
use oxidase_floating_ui::types::ComputePositionConfig;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

struct TestContext {
    doc: Document,
    root_id: NodeId,
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
                        value: "width: 1920px; height: 1080px; margin: 0; padding: 0; position: relative; display: block;".into(),
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
        parent_id: NodeId,
        tag: &str,
        id: &str,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    ) -> NodeId {
        let local = match tag {
            "button" => local_name!("button"),
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
                            "position: absolute; left: {}px; top: {}px; width: {}px; height: {}px; display: block;",
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

    fn add_scroll_container(
        &self,
        parent_id: NodeId,
        id: &str,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    ) -> NodeId {
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
                            "position: absolute; left: {}px; top: {}px; width: {}px; height: {}px; overflow: scroll; display: block;",
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

// 1. Multi-ancestor nested scroll recomputation:
//    outer scroll container -> inner scroll container -> anchor element
#[test]
fn test_multi_ancestor_nested_scroll_recomputation() {
    let ctx = TestContext::new();

    // Outer scroll container: at (0, 0), size 600x600
    let outer_id = ctx.add_scroll_container(ctx.root_id, "outer-scroll", 0.0, 0.0, 600.0, 600.0);
    // Inner scroll container inside outer: at (50, 50), size 400x400
    let inner_id = ctx.add_scroll_container(outer_id, "inner-scroll", 50.0, 50.0, 400.0, 400.0);
    // Anchor inside inner: at (100, 100), size 80x40 -> absolute x=150, y=150
    let anchor_id = ctx.add_element(inner_id, "button", "anchor-btn", 100.0, 100.0, 80.0, 40.0);
    // Floating content at root level: size 60x30
    let float_id = ctx.add_element(ctx.root_id, "div", "floating-tooltip", 0.0, 0.0, 60.0, 30.0);

    ctx.doc.resolve(0.0);

    Document::with_current(ctx.doc.clone(), || {
        let platform = NativeFloatingPlatform::new(ctx.base.clone());

        // Step 1: Initial position calculation
        // Anchor viewport: x = 0 + 50 + 100 = 150, y = 0 + 50 + 100 = 150, w = 80, h = 40
        // Floating: w = 60, h = 30
        // Placement::Bottom: x = 150 + (80 - 60)/2 = 160, y = 150 + 40 = 190
        let initial_pos = compute_position(
            ElementOrVirtual::Element(&anchor_id),
            &float_id,
            ComputePositionConfig::new(&platform).placement(Placement::Bottom),
        );
        assert_eq!(initial_pos.x, 160.0);
        assert_eq!(initial_pos.y, 190.0);

        let events = Rc::new(RefCell::new(Vec::new()));
        let events_clone = events.clone();

        // Step 2: Register auto-update watcher across multi-ancestor setup
        let _guard = watch_floating_auto_update(&["anchor-btn"], "floating-tooltip", move |evt| {
            events_clone.borrow_mut().push(evt);
        })
        .expect("watch_floating_auto_update must succeed across nested ancestors");

        // Step 3: Mutate inner container scroll
        let inner_el = ctx.doc.element_by_id("inner-scroll").expect("inner-scroll must exist");
        block_on(inner_el.scroll_to(0.0, 25.0)).expect("scroll_to inner must succeed");

        assert_eq!(
            events.borrow().len(),
            1,
            "Inner scroll mutation must emit FloatingAutoUpdateEvent::Scroll"
        );
        assert_eq!(events.borrow()[0], FloatingAutoUpdateEvent::Scroll);

        // Recompute position after inner container scroll:
        // Anchor viewport y moved up by 25: 150 - 25 = 125 -> bottom y = 125 + 40 = 165
        let pos_after_inner = compute_position(
            ElementOrVirtual::Element(&anchor_id),
            &float_id,
            ComputePositionConfig::new(&platform).placement(Placement::Bottom),
        );
        assert_eq!(pos_after_inner.x, 160.0);
        assert_eq!(
            pos_after_inner.y,
            initial_pos.y - 25.0,
            "Floating y must adapt exactly matching inner container scroll delta"
        );

        // Step 4: Mutate outer container scroll after coalesce window
        std::thread::sleep(Duration::from_millis(20));

        let outer_el = ctx.doc.element_by_id("outer-scroll").expect("outer-scroll must exist");
        block_on(outer_el.scroll_to(10.0, 35.0)).expect("scroll_to outer must succeed");

        assert_eq!(
            events.borrow().len(),
            2,
            "Outer scroll mutation must emit second FloatingAutoUpdateEvent::Scroll"
        );

        // Recompute position after outer container scroll:
        // Cumulative scroll: dx = 10, dy = 25 + 35 = 60
        // Expected x = 160 - 10 = 150, y = 190 - 60 = 130
        let pos_after_outer = compute_position(
            ElementOrVirtual::Element(&anchor_id),
            &float_id,
            ComputePositionConfig::new(&platform).placement(Placement::Bottom),
        );
        assert_eq!(
            pos_after_outer.x,
            initial_pos.x - 10.0,
            "Floating x must reflect outer container horizontal scroll delta"
        );
        assert_eq!(
            pos_after_outer.y,
            initial_pos.y - 60.0,
            "Floating y must reflect cumulative (inner + outer) vertical scroll delta"
        );
    });
}

// 2. Coalesced scroll update:
//    Mutate both containers within the 16ms window -> exactly 1 update event emitted,
//    and recomputation yields the cumulative position change.
#[test]
fn test_coalesced_nested_scroll_update() {
    let ctx = TestContext::new();

    let outer_id = ctx.add_scroll_container(ctx.root_id, "coal-outer", 0.0, 0.0, 500.0, 500.0);
    let inner_id = ctx.add_scroll_container(outer_id, "coal-inner", 40.0, 40.0, 300.0, 300.0);
    let anchor_id = ctx.add_element(inner_id, "button", "coal-anchor", 50.0, 50.0, 60.0, 30.0);
    let float_id = ctx.add_element(ctx.root_id, "div", "coal-float", 0.0, 0.0, 40.0, 20.0);

    ctx.doc.resolve(0.0);

    Document::with_current(ctx.doc.clone(), || {
        let platform = NativeFloatingPlatform::new(ctx.base.clone());

        // Initial coords:
        // Anchor viewport: x = 40 + 50 = 90, y = 40 + 50 = 90, w = 60, h = 30
        // Floating: w = 40, h = 20
        // Placement::Bottom: x = 90 + (60 - 40)/2 = 100, y = 90 + 30 = 120
        let initial_pos = compute_position(
            ElementOrVirtual::Element(&anchor_id),
            &float_id,
            ComputePositionConfig::new(&platform).placement(Placement::Bottom),
        );
        assert_eq!(initial_pos.x, 100.0);
        assert_eq!(initial_pos.y, 120.0);

        let event_count = Rc::new(RefCell::new(0));
        let count_clone = event_count.clone();

        let _guard = watch_floating_auto_update(&["coal-anchor"], "coal-float", move |_| {
            *count_clone.borrow_mut() += 1;
        })
        .expect("registration must succeed");

        let inner_el = ctx.doc.element_by_id("coal-inner").expect("coal-inner must exist");
        let outer_el = ctx.doc.element_by_id("coal-outer").expect("coal-outer must exist");

        // Mutate BOTH inner and outer containers rapidly without sleeping (< 16ms window)
        block_on(inner_el.scroll_to(0.0, 30.0)).expect("inner scroll");
        block_on(outer_el.scroll_to(15.0, 40.0)).expect("outer scroll");

        // Assert exactly 1 coalesced event is emitted
        assert_eq!(
            *event_count.borrow(),
            1,
            "Rapid scroll mutations within 16ms render window must be coalesced into exactly 1 event"
        );

        // But recomputation evaluates the live DOM state and reflects cumulative changes
        let pos_after_coalesced = compute_position(
            ElementOrVirtual::Element(&anchor_id),
            &float_id,
            ComputePositionConfig::new(&platform).placement(Placement::Bottom),
        );

        // Cumulative dx = 15, cumulative dy = 30 + 40 = 70
        assert_eq!(pos_after_coalesced.x, initial_pos.x - 15.0);
        assert_eq!(pos_after_coalesced.y, initial_pos.y - 70.0);
    });
}

// 3. RAII Teardown & Leak-proof verification:
//    - Behavioral teardown: after guard.stop() or drop, scrolling outer/inner container emits 0 events.
//    - Structural teardown: assert observer registry count for element scroll observers returns to baseline (0).
#[test]
fn test_raii_teardown_and_leak_proof_registry_verification() {
    let ctx = TestContext::new();

    let outer_id = ctx.add_scroll_container(ctx.root_id, "leak-outer", 0.0, 0.0, 500.0, 500.0);
    let inner_id = ctx.add_scroll_container(outer_id, "leak-inner", 30.0, 30.0, 300.0, 300.0);
    let _anchor_id = ctx.add_element(inner_id, "button", "leak-anchor", 40.0, 40.0, 80.0, 30.0);
    let _float_id = ctx.add_element(ctx.root_id, "div", "leak-float", 0.0, 0.0, 60.0, 30.0);

    ctx.doc.resolve(0.0);

    Document::with_current(ctx.doc.clone(), || {
        // 1. Baseline structural verification: 0 active element scroll observers
        assert_eq!(
            ctx.doc.element_scroll_observer_count(None),
            0,
            "Baseline observer count must be 0"
        );

        let events = Rc::new(RefCell::new(Vec::new()));

        // Scope containing the active guard
        {
            let ev = events.clone();
            let _guard = watch_floating_auto_update(&["leak-anchor"], "leak-float", move |e| {
                ev.borrow_mut().push(e);
            })
            .expect("registration must succeed");

            // 2. Active structural verification:
            // Both inner and outer ancestors must have active observers
            assert_eq!(
                ctx.doc.element_scroll_observer_count(Some("leak-inner")),
                1,
                "Inner ancestor must have 1 active element scroll observer"
            );
            assert_eq!(
                ctx.doc.element_scroll_observer_count(Some("leak-outer")),
                1,
                "Outer ancestor must have 1 active element scroll observer"
            );
            assert_eq!(
                ctx.doc.element_scroll_observer_count(None),
                2,
                "Total active element scroll observers must equal 2"
            );

            // Active behavioral verification
            let inner_el = ctx.doc.element_by_id("leak-inner").expect("leak-inner exists");
            block_on(inner_el.scroll_to(0.0, 10.0)).expect("scroll inner");
            assert_eq!(events.borrow().len(), 1, "Active watcher must receive event");
        } // <--- _guard drops here!

        // 3. Post-drop structural verification:
        // Observer registry count for element scroll observers MUST return to baseline (0)
        assert_eq!(
            ctx.doc.element_scroll_observer_count(None),
            0,
            "Structural leak-proof: observer registry count must return to baseline 0 after drop"
        );
        assert_eq!(
            ctx.doc.element_scroll_observer_count(Some("leak-inner")),
            0,
            "Structural leak-proof: inner ancestor observer must be removed from registry"
        );
        assert_eq!(
            ctx.doc.element_scroll_observer_count(Some("leak-outer")),
            0,
            "Structural leak-proof: outer ancestor observer must be removed from registry"
        );

        // 4. Post-drop behavioral verification:
        // Wait beyond coalesce window and mutate both containers -> exactly 0 new events
        std::thread::sleep(Duration::from_millis(20));

        let inner_el = ctx.doc.element_by_id("leak-inner").expect("leak-inner exists");
        let outer_el = ctx.doc.element_by_id("leak-outer").expect("leak-outer exists");

        block_on(inner_el.scroll_to(0.0, 50.0)).expect("scroll inner after drop");
        block_on(outer_el.scroll_to(0.0, 50.0)).expect("scroll outer after drop");

        assert_eq!(
            events.borrow().len(),
            1,
            "Behavioral teardown: no new events must be received after WatcherGuard drop"
        );
    });
}

// 4. Explicit guard.stop() teardown verification:
//    Validates that calling guard.stop() imperatively achieves identical structural & behavioral cleanup.
#[test]
fn test_explicit_guard_stop_teardown() {
    let ctx = TestContext::new();

    let outer_id = ctx.add_scroll_container(ctx.root_id, "stop-outer", 0.0, 0.0, 500.0, 500.0);
    let inner_id = ctx.add_scroll_container(outer_id, "stop-inner", 20.0, 20.0, 300.0, 300.0);
    let _anchor_id = ctx.add_element(inner_id, "button", "stop-anchor", 30.0, 30.0, 80.0, 30.0);
    let _float_id = ctx.add_element(ctx.root_id, "div", "stop-float", 0.0, 0.0, 50.0, 20.0);

    ctx.doc.resolve(0.0);

    Document::with_current(ctx.doc.clone(), || {
        let events = Rc::new(RefCell::new(0));
        let ev = events.clone();

        let guard = watch_floating_auto_update(&["stop-anchor"], "stop-float", move |_| {
            *ev.borrow_mut() += 1;
        })
        .expect("registration must succeed");

        assert_eq!(ctx.doc.element_scroll_observer_count(None), 2);

        // Imperative stop
        guard.stop();

        // Structural check: returns to 0 immediately
        assert_eq!(
            ctx.doc.element_scroll_observer_count(None),
            0,
            "guard.stop() must immediately clean up all element scroll observers in registry"
        );

        // Behavioral check: scroll emits 0 events
        std::thread::sleep(Duration::from_millis(20));
        let inner_el = ctx.doc.element_by_id("stop-inner").expect("stop-inner exists");
        block_on(inner_el.scroll_to(0.0, 40.0)).expect("scroll inner after stop");

        assert_eq!(
            *events.borrow(),
            0,
            "guard.stop() must prevent any future events from firing"
        );
    });
}
