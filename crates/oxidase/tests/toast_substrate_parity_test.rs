use oxidase::dom::observer::{observe_visibility_change, DocumentVisibility};
use oxidase::dom::Document;
use oxidase::time::{monotonic_now_ms, unix_epoch_ms, MonotonicInstant};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

#[test]
fn test_monotonic_instant_and_now_ms() {
    let t0 = MonotonicInstant::now();
    let m0 = monotonic_now_ms();
    let u0 = unix_epoch_ms();

    std::thread::sleep(Duration::from_millis(5));

    let t1 = MonotonicInstant::now();
    let m1 = monotonic_now_ms();
    let u1 = unix_epoch_ms();

    assert!(t1 >= t0, "MonotonicInstant must be non-decreasing");
    let forward = t1.saturating_duration_since(t0);
    assert!(
        forward >= Duration::from_millis(1),
        "Elapsed monotonic duration should reflect sleep: {forward:?}"
    );
    assert_eq!(
        t0.saturating_duration_since(t1),
        Duration::ZERO,
        "Reverse saturating_duration_since must saturate to Duration::ZERO"
    );
    assert!(
        t0.elapsed() >= forward,
        "elapsed() must be at least t1.saturating_duration_since(t0)"
    );

    assert!(m0.is_finite() && m0 >= 0.0, "monotonic_now_ms must be finite and non-negative");
    assert!(m1.is_finite() && m1 >= m0, "monotonic_now_ms must be non-decreasing");

    assert!(
        u0.is_finite() && u0 > 1_700_000_000_000.0,
        "unix_epoch_ms must return realistic Unix epoch milliseconds: {u0}"
    );
    assert!(u1 >= u0, "unix_epoch_ms must not move backward across consecutive reads");
}

#[test]
fn test_visibility_change_observer_and_dedup() {
    #[cfg(feature = "native")]
    let doc = {
        let base = Rc::new(RefCell::new(blitz_dom::BaseDocument::new(
            blitz_dom::DocumentConfig::default(),
        )));
        Document::from_base(base)
    };

    #[cfg(not(feature = "native"))]
    let doc = Document::mock();

    // Normalize initial thread-local visibility state
    doc.set_document_hidden(false);
    assert!(!doc.is_document_hidden());

    let observed = Rc::new(RefCell::new(Vec::<DocumentVisibility>::new()));
    let obs_clone = observed.clone();
    let guard = observe_visibility_change(move |vis| {
        obs_clone.borrow_mut().push(vis);
    })
    .expect("observe_visibility_change must succeed");

    // Redundant Visible -> Visible must not emit
    doc.set_document_hidden(false);
    assert!(observed.borrow().is_empty());

    // Transition Visible -> Hidden emits once
    doc.set_document_hidden(true);
    assert!(doc.is_document_hidden());
    assert_eq!(*observed.borrow(), vec![DocumentVisibility::Hidden]);
    assert!(DocumentVisibility::Hidden.is_hidden());
    assert!(!DocumentVisibility::Hidden.is_visible());

    // Duplicate Hidden -> Hidden must be deduplicated
    doc.set_document_hidden(true);
    assert_eq!(*observed.borrow(), vec![DocumentVisibility::Hidden]);

    // Transition Hidden -> Visible emits once
    doc.set_document_hidden(false);
    assert!(!doc.is_document_hidden());
    assert_eq!(
        *observed.borrow(),
        vec![DocumentVisibility::Hidden, DocumentVisibility::Visible]
    );
    assert!(DocumentVisibility::Visible.is_visible());
    assert!(!DocumentVisibility::Visible.is_hidden());

    // Duplicate Visible -> Visible must be deduplicated
    doc.set_document_hidden(false);
    assert_eq!(observed.borrow().len(), 2);

    // Dropping WatcherGuard detaches listener cleanly
    drop(guard);
    doc.set_document_hidden(true);
    doc.set_document_hidden(false);
    assert_eq!(
        observed.borrow().len(),
        2,
        "Dropped WatcherGuard must stop visibility notifications"
    );
}

#[cfg(not(feature = "native"))]
#[test]
fn test_mock_parity_methods() {
    use oxidase::dom::Element;

    let doc = Document::mock();
    assert!(doc.measure_unscaled_rect("any").is_none());
    assert!(doc.measure_auto_height("any").is_none());
    assert!(!doc.is_focus_within("any"));
    assert!(doc.blur_active().is_ok());

    let a = Element::mock();
    let b = Element::mock();
    assert!(!a.contains(&b));
    assert!(a.blur_sync().is_ok());
}

#[cfg(feature = "native")]
mod native_tests {
    use super::*;
    use blitz_dom::{
        Attribute, BaseDocument, DocumentConfig, QualName, UserAgentStylesheet, local_name, ns,
    };
    use dioxus::prelude::*;
    use oxidase::dom::observer::observe_element_resize;

    fn poll_vdom_once(vdom: &mut VirtualDom) {
        use std::future::Future;
        use std::pin::pin;
        use std::task::{Context, RawWaker, RawWakerVTable, Waker};

        unsafe fn clone(_: *const ()) -> RawWaker {
            RawWaker::new(std::ptr::null(), &VTABLE)
        }
        unsafe fn wake(_: *const ()) {}
        unsafe fn wake_by_ref(_: *const ()) {}
        unsafe fn vtable_drop(_: *const ()) {}
        static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, vtable_drop);

        let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };
        let mut cx = Context::from_waker(&waker);
        let fut = pin!(vdom.wait_for_work());
        let _ = fut.poll(&mut cx);
    }

    #[test]
    fn test_native_measure_auto_height_overrides_css_clamped_height_and_scale() {
        let mut base_doc = BaseDocument::new(DocumentConfig::default());
        {
            let mut vp = base_doc.viewport_mut();
            vp.window_size = (1280, 800);
            vp.hidpi_scale = 1.0;
        }
        base_doc.add_user_agent_stylesheet(UserAgentStylesheet::parse(
            r#"
            #clamped-toast {
                display: block;
                width: 320px;
                height: 40px;
                overflow: hidden;
                padding: 12px 16px;
                border: 2px solid black;
                box-sizing: border-box;
                transform: scale(0.9);
                transition: height 350ms ease;
            }
            #auto-toast {
                display: block;
                width: 320px;
                height: auto;
                padding: 10px;
                box-sizing: border-box;
            }
            "#,
        ));

        let doc_root = base_doc.root_node().id;
        {
            let mut mutator = base_doc.mutate();
            let html_id = mutator.create_element(
                QualName::new(None, ns!(html), local_name!("html")),
                vec![
                    Attribute {
                        name: QualName::new(None, ns!(), local_name!("id")),
                        value: "app-root".into(),
                    },
                    Attribute {
                        name: QualName::new(None, ns!(), local_name!("style")),
                        value: "display: block; width: 1280px; height: 800px; margin: 0; padding: 0;"
                            .into(),
                    },
                ],
            );
            let clamped = mutator.create_element(
                QualName::new(None, ns!(html), local_name!("div")),
                vec![
                    Attribute {
                        name: QualName::new(None, ns!(), local_name!("id")),
                        value: "clamped-toast".into(),
                    },
                    Attribute {
                        name: QualName::new(None, ns!(), local_name!("style")),
                        value: "pointer-events: auto; --y: -14px;".into(),
                    },
                ],
            );
            let child_a = mutator.create_element(
                QualName::new(None, ns!(html), local_name!("div")),
                vec![Attribute {
                    name: QualName::new(None, ns!(), local_name!("style")),
                    value: "display: block; height: 48px;".into(),
                }],
            );
            let child_b = mutator.create_element(
                QualName::new(None, ns!(html), local_name!("div")),
                vec![Attribute {
                    name: QualName::new(None, ns!(), local_name!("style")),
                    value: "display: block; height: 60px;".into(),
                }],
            );
            let auto_card = mutator.create_element(
                QualName::new(None, ns!(html), local_name!("div")),
                vec![Attribute {
                    name: QualName::new(None, ns!(), local_name!("id")),
                    value: "auto-toast".into(),
                }],
            );
            let auto_child = mutator.create_element(
                QualName::new(None, ns!(html), local_name!("div")),
                vec![Attribute {
                    name: QualName::new(None, ns!(), local_name!("style")),
                    value: "display: block; height: 55px;".into(),
                }],
            );

            mutator.append_children(clamped, &[child_a, child_b]);
            mutator.append_children(auto_card, &[auto_child]);
            mutator.append_children(html_id, &[clamped, auto_card]);
            mutator.append_children(doc_root, &[html_id]);
        }

        let base = Rc::new(RefCell::new(base_doc));
        let doc = Document::from_base(base.clone());

        Document::with_current(doc.clone(), || {
            doc.resolve(0.0);

            // Clamped box starts at 40px CSS height with transform: scale(0.9) -> 36px visual rect, 40px unscaled
            let clamped_rect = doc
                .measure_rect("clamped-toast")
                .expect("clamped-toast must resolve");
            assert!(
                (clamped_rect.height - 36.0).abs() < 0.01,
                "Expected transform-scaled clamped height 36.0 (40.0 * 0.9), got {}",
                clamped_rect.height
            );
            let unscaled_clamped = doc
                .measure_unscaled_rect("clamped-toast")
                .expect("unscaled rect must resolve");
            assert!(
                (unscaled_clamped.height - 40.0).abs() < 0.01,
                "Expected unscaled clamped height 40.0, got {}",
                unscaled_clamped.height
            );

            let resize_events = Rc::new(RefCell::new(Vec::new()));
            let re_clone = resize_events.clone();
            let _resize_guard = observe_element_resize("clamped-toast", move |entry| {
                re_clone.borrow_mut().push(entry);
            })
            .expect("observe_element_resize must succeed");

            assert!(base.borrow().pending_animation_events.is_empty());

            // Measure natural auto height through CSS height clamp (40px) + transform: scale(0.9) + transition
            let natural_h = doc
                .measure_auto_height("clamped-toast")
                .expect("measure_auto_height must return Some");
            assert!(
                (natural_h - 136.0).abs() < 0.01,
                "Expected unscaled natural border-box height 136.0 (48 + 60 + 24 + 4), got {natural_h}"
            );

            // Verify clamped 40px box (36px scaled), exact inline style attribute, and animation queue are untouched
            let after_rect = doc
                .measure_rect("clamped-toast")
                .expect("clamped-toast must remain measurable");
            assert!(
                (after_rect.height - 36.0).abs() < 0.01,
                "Clamped 40px * 0.9 box must remain intact after measure_auto_height, got {}",
                after_rect.height
            );
            let after_unscaled = doc
                .measure_unscaled_rect("clamped-toast")
                .expect("clamped-toast unscaled rect must remain measurable");
            assert!(
                (after_unscaled.height - 40.0).abs() < 0.01,
                "Clamped unscaled 40px box must remain intact after measure_auto_height, got {}",
                after_unscaled.height
            );
            let clamped_el = doc
                .element_by_id("clamped-toast")
                .expect("clamped-toast element must exist");
            assert_eq!(
                clamped_el.get_attribute("style").as_deref(),
                Some("pointer-events: auto; --y: -14px;"),
                "Inline style attribute must be restored verbatim"
            );
            assert!(
                base.borrow().pending_animation_events.is_empty(),
                "pending_animation_events must stay clean after measure_auto_height"
            );
            assert!(
                !doc.is_animating(),
                "measure_auto_height must not start any CSS height transition"
            );

            // Advance resolve time and verify zero spurious transitions or resize notifications
            doc.resolve(0.1);
            assert!(
                (doc.measure_unscaled_rect("clamped-toast").unwrap().height - 40.0).abs() < 0.01,
                "Subsequent resolve must keep height clamped at 40px without transitioning"
            );
            assert!(
                (doc.measure_rect("clamped-toast").unwrap().height - 36.0).abs() < 0.01,
                "Subsequent resolve must keep scaled height at 36px without transitioning"
            );
            assert!(base.borrow().pending_animation_events.is_empty());
            assert!(
                resize_events.borrow().is_empty(),
                "measure_auto_height probe must not trigger ResizeObserver notifications"
            );

            // Fast path: element whose computed height is already `auto` (55 + 20 padding = 75px)
            let auto_h = doc
                .measure_auto_height("auto-toast")
                .expect("fast-path measure_auto_height must succeed");
            assert!(
                (auto_h - 75.0).abs() < 0.01,
                "Expected fast-path auto height 75.0, got {auto_h}"
            );
            assert_eq!(
                doc.element_by_id("auto-toast")
                    .unwrap()
                    .get_attribute("style"),
                None
            );

            // Contended borrow fallback: holding immutable borrow must not panic
            let held = base.borrow();
            let fallback_h = doc.measure_auto_height("clamped-toast");
            assert_eq!(fallback_h, Some(40.0));
            drop(held);
        });
    }

    #[component]
    fn FocusTreeApp() -> Element {
        rsx! {
            div {
                id: "viewport",
                tabindex: "0",
                div {
                    id: "card-1",
                    tabindex: "0",
                    button { id: "action-1", "Action" }
                }
            }
            button { id: "outside-btn", "Outside" }
        }
    }

    #[test]
    fn test_native_is_focus_within_element_contains_and_clear_focus_blur_sync() {
        let vdom = VirtualDom::new(FocusTreeApp);
        let mut dioxus_doc = dioxus_native::DioxusDocument::new(vdom, DocumentConfig::default());
        dioxus_doc.initial_build();

        let doc = Document::from_base(dioxus_doc.inner.clone());
        Document::with_current(doc.clone(), || {
            doc.resolve(0.0);

            let viewport = doc.element_by_id("viewport").expect("viewport must exist");
            let card_1 = doc.element_by_id("card-1").expect("card-1 must exist");
            let action_1 = doc.element_by_id("action-1").expect("action-1 must exist");
            let outside_btn = doc
                .element_by_id("outside-btn")
                .expect("outside-btn must exist");

            // 1. Element::contains inclusive & ancestor/sibling semantics
            assert!(viewport.contains(&viewport), "Element::contains must be reflexive");
            assert!(viewport.contains(&card_1), "Parent must contain child");
            assert!(viewport.contains(&action_1), "Ancestor must contain nested descendant");
            assert!(!card_1.contains(&viewport), "Child must not contain parent");
            assert!(!viewport.contains(&outside_btn), "Sibling must not be contained");

            // 2. Initial focus state is None
            assert!(doc.active_element().is_none());
            assert!(!doc.is_focus_within("viewport"));
            assert!(!doc.is_focus_within("nonexistent"));

            // 3. Focus nested button -> is_focus_within matches self and ancestors
            doc.set_focus("action-1").expect("set_focus must succeed");
            assert_eq!(
                doc.active_element().and_then(|e| e.id()).as_deref(),
                Some("action-1")
            );
            assert!(doc.is_focus_within("action-1"));
            assert!(doc.is_focus_within("card-1"));
            assert!(doc.is_focus_within("viewport"));
            assert!(!doc.is_focus_within("outside-btn"));

            // Blurring an unfocused element must be a no-op
            outside_btn.blur_sync().expect("blur_sync on unfocused element must succeed");
            assert_eq!(
                doc.active_element().and_then(|e| e.id()).as_deref(),
                Some("action-1")
            );

            // 4. Direct synchronous blur_sync clears focus to None (NOT root_node)
            action_1.blur_sync().expect("blur_sync on active element must succeed");
            assert!(
                doc.active_element().is_none(),
                "blur_sync must leave active_element() == None"
            );
            assert!(
                dioxus_doc.inner.borrow().active_focus_node_id().is_none(),
                "BaseDocument::active_focus_node_id() must be None"
            );
            assert!(!doc.is_focus_within("viewport"));

            // 5. Direct synchronous Document::blur_active clears focus to None
            doc.set_focus("card-1").expect("set_focus card-1 must succeed");
            assert!(doc.is_focus_within("viewport"));
            doc.blur_active().expect("blur_active must succeed");
            assert!(doc.active_element().is_none());
            assert!(!doc.is_focus_within("viewport"));

            // 6. Contended borrow (simulating capture_hook immutable borrow) defers blur_sync via safe_spawn
            doc.set_focus("action-1").expect("set_focus action-1 must succeed");
            assert_eq!(
                doc.active_element().and_then(|e| e.id()).as_deref(),
                Some("action-1")
            );
            dioxus_doc.vdom.in_scope(ScopeId::ROOT, || {
                let held_ref = dioxus_doc.inner.borrow();
                action_1
                    .blur_sync()
                    .expect("blur_sync must not panic or error under immutable borrow");
                // Still focused while borrow is held
                assert_eq!(held_ref.active_focus_node_id().is_some(), true);
                drop(held_ref);
            });
            poll_vdom_once(&mut dioxus_doc.vdom);
            assert!(
                doc.active_element().is_none(),
                "Deferred safe_spawn blur_sync must clear active_element() once borrow drops"
            );
            assert!(!doc.is_focus_within("viewport"));

            // 7. Contended borrow defers Document::blur_active via safe_spawn
            doc.set_focus("card-1").expect("set_focus card-1 must succeed");
            assert!(doc.is_focus_within("viewport"));
            dioxus_doc.vdom.in_scope(ScopeId::ROOT, || {
                let held_ref = dioxus_doc.inner.borrow();
                doc.blur_active()
                    .expect("blur_active must not panic or error under immutable borrow");
                assert!(held_ref.active_focus_node_id().is_some());
                drop(held_ref);
            });
            poll_vdom_once(&mut dioxus_doc.vdom);
            assert!(
                doc.active_element().is_none(),
                "Deferred safe_spawn blur_active must clear active_element() once borrow drops"
            );
            assert!(!doc.is_focus_within("viewport"));
        });
    }
}
