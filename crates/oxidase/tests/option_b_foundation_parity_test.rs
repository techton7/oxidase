use dioxus::prelude::*;
use oxidase::dom::{
    Document, ScrollBehavior, ScrollIntoViewOptions, ScrollLogicalPosition, acquire_scroll_lock,
    is_scroll_locked, release_scroll_lock,
};
use oxidase::floating::HostFloatingPlatform;
use std::time::Duration;

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
    let _ = pin!(vdom.wait_for_work()).poll(&mut cx);
}

#[test]
fn test_scroll_lock_refcount_and_sync_fallback() {
    release_scroll_lock("lock-a", None);
    release_scroll_lock("lock-b", None);
    assert!(!is_scroll_locked(), "Initial state must be unlocked");

    // Reference-counted multi-lock lifecycle
    acquire_scroll_lock("lock-a");
    assert!(is_scroll_locked());

    acquire_scroll_lock("lock-b");
    assert!(is_scroll_locked());

    // Releasing only one active lock must keep scroll locked
    release_scroll_lock("lock-a", None);
    assert!(
        is_scroll_locked(),
        "Scroll lock must remain held while lock-b is active"
    );

    // Releasing unknown lock ID must not unlock
    release_scroll_lock("nonexistent-lock", None);
    assert!(is_scroll_locked());

    // Releasing final lock with None unlocks immediately
    release_scroll_lock("lock-b", None);
    assert!(!is_scroll_locked());

    // Outside an active Dioxus runtime, delayed release falls back to immediate restore
    acquire_scroll_lock("lock-outside-rt");
    assert!(is_scroll_locked());
    release_scroll_lock("lock-outside-rt", Some(50));
    assert!(
        !is_scroll_locked(),
        "Delayed release outside Dioxus runtime must fall back to immediate unlock"
    );
}

#[test]
fn test_scroll_lock_delayed_restore_and_generation_cancellation_in_vdom() {
    fn app() -> dioxus::prelude::Element {
        rsx! { div {} }
    }

    let mut vdom = VirtualDom::new(app);
    vdom.rebuild_in_place();

    vdom.in_scope(ScopeId::ROOT, || {
        acquire_scroll_lock("vdom-lock-1");
        assert!(is_scroll_locked());

        // Release with 40ms restore delay inside active Dioxus runtime
        release_scroll_lock("vdom-lock-1", Some(40));
        assert!(
            is_scroll_locked(),
            "Scroll lock must remain active during pending restore_delay"
        );
    });

    // Register task waker on the pending Delay; must still be locked at t~0ms
    poll_vdom_once(&mut vdom);
    assert!(is_scroll_locked());

    // Re-acquiring before delay elapses bumps cleanup_generation and cancels stale timer
    vdom.in_scope(ScopeId::ROOT, || {
        acquire_scroll_lock("vdom-lock-2");
        assert!(is_scroll_locked());
    });

    std::thread::sleep(Duration::from_millis(60));
    poll_vdom_once(&mut vdom);

    assert!(
        is_scroll_locked(),
        "Stale restore timer must not unlock after re-acquisition"
    );

    // Now release vdom-lock-2 with 25ms delay and let timer complete
    vdom.in_scope(ScopeId::ROOT, || {
        release_scroll_lock("vdom-lock-2", Some(25));
        assert!(is_scroll_locked());
    });
    poll_vdom_once(&mut vdom);
    assert!(is_scroll_locked());

    std::thread::sleep(Duration::from_millis(50));
    poll_vdom_once(&mut vdom);

    assert!(
        !is_scroll_locked(),
        "Delayed restore must unlock once timer elapses without re-acquisition"
    );
}

#[cfg(not(feature = "native"))]
#[test]
fn test_mock_option_b_parity_contracts() {
    use oxidase::dom::Element;

    assert!(
        HostFloatingPlatform::current().is_none(),
        "HostFloatingPlatform::current() must return None on headless mock builds"
    );

    let doc = Document::mock();
    doc.set_body_user_select_none();
    doc.restore_body_user_select();
    doc.teleport_portal_to_host("portal-el", None);
    doc.teleport_portal_to_host("portal-el", Some("custom-host"));
    doc.remove_portal_element_by_id("portal-el");

    let el = Element::mock();
    assert!(el.scroll_into_view_sync().is_ok());
    assert!(
        el.scroll_into_view_sync_with_options(ScrollIntoViewOptions {
            behavior: ScrollBehavior::Auto,
            block: ScrollLogicalPosition::Nearest,
            inline: ScrollLogicalPosition::Nearest,
        })
        .is_ok()
    );
}

#[cfg(feature = "native")]
mod native_tests {
    use super::*;
    use blitz_dom::{Attribute, BaseDocument, DocumentConfig, QualName, local_name, ns};
    use oxidase_floating_ui::compute_position;
    use oxidase_floating_ui::geometry::{ElementOrVirtual, Placement, Strategy};
    use oxidase_floating_ui::middleware::{Offset, OffsetOptions, OffsetOptionsValues};
    use oxidase_floating_ui::platform::FloatingPlatform;
    use oxidase_floating_ui::types::{Boundary, ComputePositionConfig};
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn test_native_host_floating_platform_current_and_delegation() {
        Document::set_current(None);
        assert!(
            HostFloatingPlatform::current().is_none(),
            "HostFloatingPlatform::current() must be None when no native Document is current"
        );

        let mut base_doc = BaseDocument::new(DocumentConfig::default());
        {
            let mut vp = base_doc.viewport_mut();
            vp.window_size = (1280, 800);
            vp.hidpi_scale = 1.0;
        }
        let doc_root = base_doc.root_node().id;
        let _nodes = {
            let mut mutator = base_doc.mutate();
            let root = mutator.create_element(
                QualName::new(None, ns!(html), local_name!("div")),
                vec![
                    Attribute {
                        name: QualName::new(None, ns!(), local_name!("id")),
                        value: "app-root".into(),
                    },
                    Attribute {
                        name: QualName::new(None, ns!(), local_name!("style")),
                        value: "position: relative; width: 1280px; height: 800px; display: block;"
                            .into(),
                    },
                ],
            );
            let anchor = mutator.create_element(
                QualName::new(None, ns!(html), local_name!("button")),
                vec![
                    Attribute {
                        name: QualName::new(None, ns!(), local_name!("id")),
                        value: "anchor-btn".into(),
                    },
                    Attribute {
                        name: QualName::new(None, ns!(), local_name!("style")),
                        value: "position: absolute; left: 200px; top: 100px; width: 120px; height: 40px; padding: 0; border: none; box-sizing: border-box; display: block;".into(),
                    },
                ],
            );
            let floating = mutator.create_element(
                QualName::new(None, ns!(html), local_name!("div")),
                vec![
                    Attribute {
                        name: QualName::new(None, ns!(), local_name!("id")),
                        value: "floating-box".into(),
                    },
                    Attribute {
                        name: QualName::new(None, ns!(), local_name!("style")),
                        value: "position: fixed; left: 0px; top: 0px; width: 160px; height: 80px; display: block;".into(),
                    },
                ],
            );
            mutator.append_children(root, &[anchor, floating]);
            mutator.append_children(doc_root, &[root]);
            (root, anchor, floating)
        };
        base_doc.resolve(0.0);

        let base = Rc::new(RefCell::new(base_doc));
        let doc = Document::from_base(base);

        Document::with_current(doc, || {
            let platform = HostFloatingPlatform::current()
                .expect("HostFloatingPlatform::current() must resolve inside Document::with_current");

            let rects = platform
                .get_element_rects("anchor-btn", "floating-box")
                .expect("get_element_rects must succeed");
            assert_eq!(rects.reference.x, 200.0);
            assert_eq!(rects.reference.y, 100.0);
            assert_eq!(rects.reference.width, 120.0);
            assert_eq!(rects.reference.height, 40.0);
            assert_eq!(rects.floating.width, 160.0);
            assert_eq!(rects.floating.height, 80.0);

            let dims = platform
                .get_dimensions("floating-box")
                .expect("get_dimensions must succeed");
            assert_eq!(dims.width, 160.0);
            assert_eq!(dims.height, 80.0);

            let clip = platform
                .get_clipping_rect("floating-box", Boundary::ClippingAncestors)
                .expect("get_clipping_rect must succeed");
            assert_eq!(clip.width, 1280.0);
            assert_eq!(clip.height, 800.0);
            assert!(!platform.is_rtl("floating-box"));
            assert!(
                platform
                    .get_overflow_ancestors("floating-box")
                    .expect("get_overflow_ancestors must succeed")
                    .is_empty()
            );

            // Verify Platform<String, ()> delegation through compute_position
            let anchor_id = "anchor-btn".to_string();
            let floating_id = "floating-box".to_string();
            let cfg = ComputePositionConfig::new(&platform)
                .placement(Placement::BottomStart)
                .strategy(Strategy::Fixed)
                .middleware(vec![Box::new(Offset::new(OffsetOptions::Values(
                    OffsetOptionsValues::default().main_axis(8.0),
                )))]);
            let pos = compute_position(ElementOrVirtual::Element(&anchor_id), &floating_id, cfg);
            assert_eq!(pos.placement, Placement::BottomStart);
            assert_eq!(pos.x, 200.0);
            assert_eq!(pos.y, 148.0);
        });
    }

    #[test]
    fn test_native_scroll_into_view_sync_and_document_noops() {
        let mut base_doc = BaseDocument::new(DocumentConfig::default());
        {
            let mut vp = base_doc.viewport_mut();
            vp.window_size = (800, 600);
            vp.hidpi_scale = 1.0;
        }
        let doc_root = base_doc.root_node().id;
        let (scroll_container_id, portal_parent_id, portal_node_id) = {
            let mut mutator = base_doc.mutate();
            let html = mutator.create_element(
                QualName::new(None, ns!(html), local_name!("html")),
                vec![Attribute {
                    name: QualName::new(None, ns!(), local_name!("style")),
                    value: "width: 800px; height: 600px; margin: 0; padding: 0; display: block;"
                        .into(),
                }],
            );
            let container = mutator.create_element(
                QualName::new(None, ns!(html), local_name!("div")),
                vec![
                    Attribute {
                        name: QualName::new(None, ns!(), local_name!("id")),
                        value: "scroll-list".into(),
                    },
                    Attribute {
                        name: QualName::new(None, ns!(), local_name!("style")),
                        value: "position: relative; width: 200px; height: 100px; overflow: scroll; display: block;".into(),
                    },
                ],
            );
            let spacer = mutator.create_element(
                QualName::new(None, ns!(html), local_name!("div")),
                vec![Attribute {
                    name: QualName::new(None, ns!(), local_name!("style")),
                    value: "width: 200px; height: 240px; display: block;".into(),
                }],
            );
            let item = mutator.create_element(
                QualName::new(None, ns!(html), local_name!("div")),
                vec![
                    Attribute {
                        name: QualName::new(None, ns!(), local_name!("id")),
                        value: "item-below-fold".into(),
                    },
                    Attribute {
                        name: QualName::new(None, ns!(), local_name!("style")),
                        value: "width: 200px; height: 40px; display: block;".into(),
                    },
                ],
            );
            let portal_parent = mutator.create_element(
                QualName::new(None, ns!(html), local_name!("div")),
                vec![Attribute {
                    name: QualName::new(None, ns!(), local_name!("id")),
                    value: "vdom-owner".into(),
                }],
            );
            let portal = mutator.create_element(
                QualName::new(None, ns!(html), local_name!("div")),
                vec![
                    Attribute {
                        name: QualName::new(None, ns!(), local_name!("id")),
                        value: "portal-node".into(),
                    },
                    Attribute {
                        name: QualName::new(None, ns!(), local_name!("style")),
                        value: "position: fixed; left: 10px; top: 10px; width: 50px; height: 50px;"
                            .into(),
                    },
                ],
            );
            mutator.append_children(container, &[spacer, item]);
            mutator.append_children(portal_parent, &[portal]);
            mutator.append_children(html, &[container, portal_parent]);
            mutator.append_children(doc_root, &[html]);
            (container, portal_parent, portal)
        };
        base_doc.resolve(0.0);

        let base = Rc::new(RefCell::new(base_doc));
        let doc = Document::from_base(base.clone());

        assert_eq!(
            base.borrow()
                .get_node(scroll_container_id)
                .unwrap()
                .scroll_offset()
                .y,
            0.0
        );

        let item_el = doc
            .element_by_id("item-below-fold")
            .expect("item-below-fold must exist");
        item_el
            .scroll_into_view_sync_with_options(ScrollIntoViewOptions {
                behavior: ScrollBehavior::Auto,
                block: ScrollLogicalPosition::Nearest,
                inline: ScrollLogicalPosition::Nearest,
            })
            .expect("scroll_into_view_sync_with_options must succeed");

        let scrolled_y = base
            .borrow()
            .get_node(scroll_container_id)
            .unwrap()
            .scroll_offset()
            .y;
        assert!(
            scrolled_y > 0.0,
            "Synchronous scroll_into_view_sync_with_options must scroll container immediately, got {scrolled_y}"
        );

        // Reset scroll to 0 and verify borrow-contention fallback via safe_spawn inside VirtualDom
        base.borrow_mut()
            .get_node_mut(scroll_container_id)
            .unwrap()
            .scroll_offset_mut()
            .y = 0.0;

        fn dummy_app() -> dioxus::prelude::Element {
            rsx! { div {} }
        }
        let mut vdom = VirtualDom::new(dummy_app);
        vdom.rebuild_in_place();

        vdom.in_scope(ScopeId::ROOT, || {
            let held_borrow = base.borrow();
            item_el
                .scroll_into_view_sync_with_options(ScrollIntoViewOptions {
                    behavior: ScrollBehavior::Auto,
                    block: ScrollLogicalPosition::Nearest,
                    inline: ScrollLogicalPosition::Nearest,
                })
                .expect("borrowed scroll_into_view_sync_with_options must not panic");
            drop(held_borrow);
        });

        poll_vdom_once(&mut vdom);
        let deferred_y = base
            .borrow()
            .get_node(scroll_container_id)
            .unwrap()
            .scroll_offset()
            .y;
        assert!(
            deferred_y > 0.0,
            "Deferred safe_spawn fallback must scroll container on next poll, got {deferred_y}"
        );

        // Verify Document user-select and portal methods are clean VDOM-preserving no-ops on Native
        doc.set_body_user_select_none();
        doc.restore_body_user_select();
        doc.teleport_portal_to_host("portal-node", None);
        doc.teleport_portal_to_host("portal-node", Some("scroll-list"));
        doc.remove_portal_element_by_id("portal-node");

        let b = base.borrow();
        let p_node = b
            .get_node(portal_node_id)
            .expect("portal node must remain intact in BaseDocument");
        assert_eq!(
            p_node.parent,
            Some(portal_parent_id),
            "Native teleport/remove no-ops must preserve VDOM-owned parent hierarchy"
        );
    }
}
