use dioxus::prelude::*;
use oxidase::dom::modal_shield::{
    ModalShieldGuard, ModalShieldMode, hide_others, inert_others, shield_others_with_mode,
    suppress_others,
};
use oxidase::dom::{
    ScrollLockOptions, acquire_scroll_lock, is_body_pointer_events_locked, is_scroll_locked,
    mouse_event_detail, mouse_event_target, release_scroll_lock,
};
#[cfg(feature = "native")]
use oxidase::dom::acquire_scroll_lock_with_options;
use oxidase::watcher::{DismissEvent, DocumentDismissEvent};
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
fn test_dismiss_event_click_serde_and_boundary_helpers() {
    let json = r#"{"kind":"click","pathIds":["outside-btn","app-root"]}"#;
    let parsed: DocumentDismissEvent = serde_json::from_str(json).expect("valid click json");
    assert_eq!(
        parsed,
        DismissEvent::Click {
            path_ids: vec!["outside-btn".to_string(), "app-root".to_string()],
        }
    );
    assert_eq!(parsed.target_id(), Some("outside-btn"));
    assert!(parsed.is_outside_boundary(&["dialog-content"]));
    assert!(!parsed.is_outside_boundary(&["app-root"]));
    assert!(parsed.is_outside_boundary(&[]));

    let serialized = serde_json::to_string(&parsed).expect("serialize click");
    assert!(serialized.contains(r#""kind":"click""#));
}

#[test]
fn test_reexports_and_mock_fallbacks() {
    let default_opts = ScrollLockOptions::default();
    assert!(default_opts.prevent_body_pointer_events);

    let mode = ModalShieldMode::default();
    assert_eq!(mode, ModalShieldMode::Suppress);

    let g1: ModalShieldGuard = suppress_others(&["nonexistent"]);
    g1.release();
    assert!(g1.is_released());

    let g2 = shield_others_with_mode(&["nonexistent"], ModalShieldMode::AriaHiddenOnly);
    let g3 = hide_others(&["nonexistent"]);
    let g4 = inert_others(&["nonexistent"]);
    drop((g2, g3, g4));

    // Verify mouse_event_target / mouse_event_detail fallback on generic Synthetic MouseData
    struct DummyMousePayload;
    impl dioxus::html::InteractionLocation for DummyMousePayload {
        fn client_coordinates(&self) -> dioxus::html::geometry::ClientPoint {
            Default::default()
        }
        fn screen_coordinates(&self) -> dioxus::html::geometry::ScreenPoint {
            Default::default()
        }
        fn page_coordinates(&self) -> dioxus::html::geometry::PagePoint {
            Default::default()
        }
    }
    impl dioxus::html::InteractionElementOffset for DummyMousePayload {
        fn element_coordinates(&self) -> dioxus::html::geometry::ElementPoint {
            Default::default()
        }
    }
    impl dioxus::html::ModifiersInteraction for DummyMousePayload {
        fn modifiers(&self) -> dioxus::html::input_data::keyboard_types::Modifiers {
            Default::default()
        }
    }
    impl dioxus::html::PointerInteraction for DummyMousePayload {
        fn trigger_button(&self) -> Option<dioxus::html::input_data::MouseButton> {
            Some(dioxus::html::input_data::MouseButton::Primary)
        }
        fn held_buttons(&self) -> dioxus::html::input_data::MouseButtonSet {
            Default::default()
        }
    }
    impl dioxus::html::HasMouseData for DummyMousePayload {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    let mouse_data = dioxus::prelude::MouseData::new(DummyMousePayload);
    assert!(mouse_event_target(&mouse_data).is_none());
    assert_eq!(mouse_event_detail(&mouse_data), 1);
}

#[test]
fn test_scroll_lock_synchronous_pointer_events_unlock_before_restore_delay() {
    fn empty_app() -> dioxus::prelude::Element {
        rsx! { div {} }
    }

    release_scroll_lock("sync-pe-lock", None);
    assert!(!is_scroll_locked());
    assert!(!is_body_pointer_events_locked());

    let mut vdom = VirtualDom::new(empty_app);
    vdom.rebuild_in_place();

    vdom.in_scope(ScopeId::ROOT, || {
        acquire_scroll_lock("sync-pe-lock");
        assert!(is_scroll_locked());
        assert!(is_body_pointer_events_locked());

        release_scroll_lock("sync-pe-lock", Some(50));
        assert!(
            !is_body_pointer_events_locked(),
            "pointer-events lock must release synchronously (0ms) before restore_delay"
        );
        assert!(
            is_scroll_locked(),
            "overflow scroll lock must remain active until restore_delay elapses"
        );
    });

    poll_vdom_once(&mut vdom);
    assert!(is_scroll_locked());
    assert!(!is_body_pointer_events_locked());

    std::thread::sleep(Duration::from_millis(70));
    poll_vdom_once(&mut vdom);
    assert!(!is_scroll_locked());
    assert!(!is_body_pointer_events_locked());
}

#[cfg(feature = "native")]
mod native_tests {
    use super::*;
    use blitz_dom::{Attribute, BaseDocument, DocumentConfig, NodeId, QualName, ns};
    use blitz_traits::events::{
        BlitzKeyEvent, BlitzPointerEvent, BlitzPointerId, KeyState, MouseEventButton,
        MouseEventButtons, PointerCoords, PointerDetails, UiEvent,
    };
    use dioxus::prelude::keyboard_types::{Code, Key, Location, Modifiers, NamedKey};
    use oxidase::dom::Document;
    use oxidase::dom::observer::CaptureEventKind;
    use oxidase::watcher::watch_document_dismiss;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn qname(tag: &'static str) -> QualName {
        QualName::new(None, ns!(html), blitz_dom::LocalName::from(tag))
    }

    fn attr(name: &'static str, val: &str) -> Attribute {
        Attribute {
            name: QualName::new(None, ns!(), blitz_dom::LocalName::from(name)),
            value: val.to_string(),
        }
    }

    fn make_pointer_event(
        id: BlitzPointerId,
        button: MouseEventButton,
        buttons: MouseEventButtons,
        x: f32,
        y: f32,
    ) -> BlitzPointerEvent {
        BlitzPointerEvent {
            id,
            is_primary: true,
            coords: PointerCoords {
                page_x: x,
                page_y: y,
                client_x: x,
                client_y: y,
                screen_x: x,
                screen_y: y,
            },
            button,
            buttons,
            mods: Modifiers::empty(),
            details: PointerDetails::default(),
            element: Default::default(),
            active_pointers: Default::default(),
        }
    }

    #[test]
    fn test_native_modal_shield_exemptions_uncontrolled_and_nested_refcount() {
        let mut base_doc = BaseDocument::new(DocumentConfig::default());
        let doc_root = base_doc.root_node().id;

        let (
            _body_id,
            header_id,
            pre_hidden_id,
            pre_inert_id,
            toast_vp_id,
            toast_child_id,
            script_id,
            style_id,
            template_id,
            modal1_id,
            modal1_btn_id,
            modal2_id,
        ) = {
            let mut m = base_doc.mutate();
            let html = m.create_element(qname("html"), vec![]);
            let body = m.create_element(qname("body"), vec![]);
            let app_shell = m.create_element(qname("div"), vec![attr("id", "app-shell")]);
            let header = m.create_element(qname("header"), vec![attr("id", "site-header")]);
            let pre_hidden = m.create_element(
                qname("div"),
                vec![attr("id", "pre-hidden"), attr("aria-hidden", "true")],
            );
            let pre_inert = m.create_element(
                qname("div"),
                vec![attr("id", "pre-inert"), attr("inert", "")],
            );
            let toast_vp = m.create_element(
                qname("section"),
                vec![attr("id", "toast-viewport"), attr("aria-live", "polite")],
            );
            let toast_child = m.create_element(qname("div"), vec![attr("id", "toast-item")]);
            let script = m.create_element(qname("script"), vec![attr("id", "app-script")]);
            let style = m.create_element(qname("style"), vec![attr("id", "app-style")]);
            let template = m.create_element(qname("template"), vec![attr("id", "app-tpl")]);
            let modal1 = m.create_element(qname("div"), vec![attr("id", "modal-1")]);
            let modal1_btn = m.create_element(qname("button"), vec![attr("id", "modal-1-btn")]);
            let modal2 = m.create_element(qname("div"), vec![attr("id", "modal-2")]);

            m.append_children(toast_vp, &[toast_child]);
            m.append_children(modal1, &[modal1_btn]);
            m.append_children(app_shell, &[header, modal1, modal2]);
            m.append_children(
                body,
                &[
                    app_shell, pre_hidden, pre_inert, toast_vp, script, style, template,
                ],
            );
            m.append_children(html, &[body]);
            m.append_children(doc_root, &[html]);

            (
                body,
                header,
                pre_hidden,
                pre_inert,
                toast_vp,
                toast_child,
                script,
                style,
                template,
                modal1,
                modal1_btn,
                modal2,
            )
        };

        let base_rc = Rc::new(RefCell::new(base_doc));
        let doc = Document::from_base(base_rc.clone());

        let get_attr = |nid: NodeId, name: &str| -> Option<String> {
            oxidase::dom::Element::from_native(base_rc.clone(), nid).get_attribute(name)
        };
        let has_attr = |nid: NodeId, name: &str| -> bool {
            oxidase::dom::Element::from_native(base_rc.clone(), nid).has_attribute(name)
        };

        Document::with_current(doc, || {
            // Open Modal 1
            let guard1 = suppress_others(&["modal-1"]);

            // Sibling inside #app-shell (#site-header and #modal-2) must be shielded
            assert_eq!(get_attr(header_id, "aria-hidden").as_deref(), Some("true"));
            assert!(has_attr(header_id, "inert"));
            assert_eq!(get_attr(modal2_id, "aria-hidden").as_deref(), Some("true"));
            assert!(has_attr(modal2_id, "inert"));

            // Modal 1 and its children must NOT be shielded
            assert_eq!(get_attr(modal1_id, "aria-hidden"), None);
            assert!(!has_attr(modal1_id, "inert"));
            assert_eq!(get_attr(modal1_btn_id, "aria-hidden"), None);
            assert!(!has_attr(modal1_btn_id, "inert"));

            // [aria-live] ToastViewport and its children must NOT be shielded
            assert_eq!(get_attr(toast_vp_id, "aria-hidden"), None);
            assert!(!has_attr(toast_vp_id, "inert"));
            assert_eq!(get_attr(toast_child_id, "aria-hidden"), None);
            assert!(!has_attr(toast_child_id, "inert"));

            // SKIPPED_ELEMENTS must remain untouched
            for skipped_nid in [script_id, style_id, template_id] {
                assert_eq!(get_attr(skipped_nid, "aria-hidden"), None);
                assert!(!has_attr(skipped_nid, "inert"));
            }

            // Stack Modal 2 on top of Modal 1 (so Modal 1 becomes shielded, header has refcount 2)
            let guard2 = suppress_others(&["modal-2"]);
            assert_eq!(get_attr(modal1_id, "aria-hidden").as_deref(), Some("true"));
            assert!(has_attr(modal1_id, "inert"));
            assert_eq!(get_attr(header_id, "aria-hidden").as_deref(), Some("true"));
            assert!(has_attr(header_id, "inert"));

            // Close Modal 2 -> Modal 1 unshielded, but #site-header stays shielded under guard1!
            guard2.release();
            assert_eq!(get_attr(modal1_id, "aria-hidden"), None);
            assert!(!has_attr(modal1_id, "inert"));
            assert_eq!(get_attr(header_id, "aria-hidden").as_deref(), Some("true"));
            assert!(has_attr(header_id, "inert"));

            // Close Modal 1 -> #site-header restored, while uncontrolled pre-hidden & pre-inert stay!
            drop(guard1);
            assert_eq!(get_attr(header_id, "aria-hidden"), None);
            assert!(!has_attr(header_id, "inert"));
            assert!(!has_attr(header_id, "data-oxidase-aria-hidden"));
            assert!(!has_attr(header_id, "data-oxidase-inert"));

            assert_eq!(
                get_attr(pre_hidden_id, "aria-hidden").as_deref(),
                Some("true"),
                "Pre-existing consumer aria-hidden must be preserved"
            );
            assert!(
                !has_attr(pre_hidden_id, "inert"),
                "Controlled inert added to pre-hidden element must be removed"
            );
            assert!(
                has_attr(pre_inert_id, "inert"),
                "Pre-existing consumer inert must be preserved"
            );
            assert_eq!(
                get_attr(pre_inert_id, "aria-hidden"),
                None,
                "Controlled aria-hidden added to pre-inert element must be removed"
            );

            // Verify AriaHiddenOnly and InertOnly modes independently
            let g_aria = shield_others_with_mode(&["modal-1"], ModalShieldMode::AriaHiddenOnly);
            assert_eq!(get_attr(header_id, "aria-hidden").as_deref(), Some("true"));
            assert!(!has_attr(header_id, "inert"));
            drop(g_aria);

            let g_inert = shield_others_with_mode(&["modal-1"], ModalShieldMode::InertOnly);
            assert_eq!(get_attr(header_id, "aria-hidden"), None);
            assert!(has_attr(header_id, "inert"));
            drop(g_inert);
        });
    }

    #[test]
    fn test_native_dismiss_click_dedup_touch_tap_frame0_and_deferred_untrusted_click() {
        let mut base_doc = BaseDocument::new(DocumentConfig::default());
        {
            let mut vp = base_doc.viewport_mut();
            vp.window_size = (800, 600);
            vp.hidpi_scale = 1.0;
        }
        base_doc.add_user_agent_stylesheet(blitz_dom::UserAgentStylesheet::parse(
            "html, body, div, button { display: block; margin: 0; padding: 0; }",
        ));
        let doc_root = base_doc.root_node().id;
        let (inside_btn_id, outside_btn_id) = {
            let mut m = base_doc.mutate();
            let html = m.create_element(
                qname("html"),
                vec![attr("style", "width: 800px; height: 600px;")],
            );
            let body = m.create_element(
                qname("body"),
                vec![attr("style", "width: 800px; height: 600px;")],
            );
            let dialog = m.create_element(
                qname("div"),
                vec![
                    attr("id", "dialog-box"),
                    attr(
                        "style",
                        "position: absolute; left: 20px; top: 20px; width: 200px; height: 200px;",
                    ),
                ],
            );
            let inside_btn = m.create_element(
                qname("button"),
                vec![
                    attr("id", "inside-btn"),
                    attr(
                        "style",
                        "position: absolute; left: 10px; top: 10px; width: 80px; height: 40px;",
                    ),
                ],
            );
            let outside_btn = m.create_element(
                qname("button"),
                vec![
                    attr("id", "outside-btn"),
                    attr(
                        "style",
                        "position: absolute; left: 320px; top: 320px; width: 100px; height: 40px;",
                    ),
                ],
            );
            m.append_children(dialog, &[inside_btn]);
            m.append_children(body, &[dialog, outside_btn]);
            m.append_children(html, &[body]);
            m.append_children(doc_root, &[html]);
            (inside_btn, outside_btn)
        };
        base_doc.resolve(0.0);

        let base_rc = Rc::new(RefCell::new(base_doc));
        let doc = Document::from_base(base_rc.clone());

        Document::with_current(doc.clone(), || {
            let events = Rc::new(RefCell::new(Vec::new()));
            let ev_clone = events.clone();

            let _guard = watch_document_dismiss(&["dialog-box"], move |ev| {
                ev_clone.borrow_mut().push(ev);
            })
            .expect("watch_document_dismiss must succeed");

            // 1. Frame-0 self-dismiss guard: orphan trusted pointer click (detail=1) without prior PointerDown
            doc.dispatch_capture_event(
                CaptureEventKind::click(true, 1, 340.0, 340.0),
                outside_btn_id,
            );
            assert!(
                events.borrow().is_empty(),
                "Frame-0 orphan opening click must NOT self-dismiss"
            );

            // 2. Live mouse PointerDown + PointerUp -> Click pair must emit PointerDown ONCE (deduplicating Click)
            doc.handle_ui_event(UiEvent::PointerDown(make_pointer_event(
                BlitzPointerId::Mouse,
                MouseEventButton::Main,
                MouseEventButtons::Primary,
                340.0,
                340.0,
            )));
            doc.handle_ui_event(UiEvent::PointerUp(make_pointer_event(
                BlitzPointerId::Mouse,
                MouseEventButton::Main,
                MouseEventButtons::None,
                340.0,
                340.0,
            )));
            assert_eq!(
                events.borrow().len(),
                1,
                "Mouse pointerdown + click pair must emit exactly once"
            );
            assert!(matches!(
                &events.borrow()[0],
                DismissEvent::PointerDown { path_ids } if path_ids.contains(&"outside-btn".to_string())
            ));

            // 3. Press inside dialog, drag outside, release outside -> must NOT dismiss
            doc.dispatch_capture_event(CaptureEventKind::mouse_down(50.0, 50.0), inside_btn_id);
            doc.dispatch_capture_event(
                CaptureEventKind::click(true, 1, 340.0, 340.0),
                outside_btn_id,
            );
            assert_eq!(events.borrow().len(), 1);

            // 4. Touch pan (>= 10px) outside -> must NOT dismiss on PointerDown OR distant Click
            doc.dispatch_capture_event(CaptureEventKind::touch_down(340.0, 340.0), outside_btn_id);
            assert_eq!(
                events.borrow().len(),
                1,
                "Touch pointerdown must wait for stationary tap click rather than dismissing on pan start"
            );
            doc.dispatch_capture_event(
                CaptureEventKind::click(true, 1, 355.0, 340.0),
                outside_btn_id,
            );
            assert_eq!(
                events.borrow().len(),
                1,
                "Touch movement >= 10px must NOT dismiss"
            );

            // 5. Touch tap (< 10px) via live Blitz Finger PointerDown + PointerUp -> emits DismissEvent::Click
            doc.handle_ui_event(UiEvent::PointerDown(make_pointer_event(
                BlitzPointerId::Finger(1),
                MouseEventButton::Main,
                MouseEventButtons::Primary,
                340.0,
                340.0,
            )));
            assert_eq!(events.borrow().len(), 1);
            doc.handle_ui_event(UiEvent::PointerUp(make_pointer_event(
                BlitzPointerId::Finger(1),
                MouseEventButton::Main,
                MouseEventButtons::None,
                343.0,
                344.0,
            )));
            assert_eq!(
                events.borrow().len(),
                2,
                "Stationary touch tap (< 10px) must emit DismissEvent::Click"
            );
            assert!(matches!(
                &events.borrow()[1],
                DismissEvent::Click { path_ids } if path_ids.contains(&"outside-btn".to_string())
            ));

            // 6. Keyboard Enter on focused outside button -> synthesizes Click(detail=0) -> emits DismissEvent::Click
            base_rc.borrow_mut().set_focus_to(outside_btn_id);
            events.borrow_mut().clear();
            doc.handle_ui_event(UiEvent::KeyDown(BlitzKeyEvent {
                key: Key::Named(NamedKey::Enter),
                code: Code::Enter,
                modifiers: Modifiers::empty(),
                location: Location::Standard,
                is_auto_repeating: false,
                is_composing: false,
                state: KeyState::Pressed,
                text: None,
            }));
            assert_eq!(events.borrow().len(), 1);
            assert!(matches!(
                &events.borrow()[0],
                DismissEvent::Click { path_ids } if path_ids.contains(&"outside-btn".to_string())
            ));
        });

        // 7. Programmatic / untrusted click (`!is_trusted`) inside VirtualDom defers to microtask & respects teardown
        fn app() -> dioxus::prelude::Element {
            rsx! { div {} }
        }
        let mut vdom = VirtualDom::new(app);
        vdom.rebuild_in_place();

        let deferred_events = Rc::new(RefCell::new(Vec::new()));
        let de_clone = deferred_events.clone();

        let guard = vdom.in_scope(ScopeId::ROOT, || {
            Document::with_current(doc.clone(), || {
                let g = watch_document_dismiss(&["dialog-box"], move |ev| {
                    de_clone.borrow_mut().push(ev);
                })
                .unwrap();

                doc.dispatch_capture_event(
                    CaptureEventKind::click(false, 0, 340.0, 340.0),
                    outside_btn_id,
                );
                assert!(
                    deferred_events.borrow().is_empty(),
                    "Untrusted programmatic click must defer to microtask during active runtime scope"
                );
                g
            })
        });

        poll_vdom_once(&mut vdom);
        assert_eq!(deferred_events.borrow().len(), 1);
        assert!(matches!(
            &deferred_events.borrow()[0],
            DismissEvent::Click { path_ids } if path_ids.contains(&"outside-btn".to_string())
        ));

        // Now dispatch another untrusted click and drop the guard BEFORE polling -> must cancel cleanly
        vdom.in_scope(ScopeId::ROOT, || {
            Document::with_current(doc.clone(), || {
                doc.dispatch_capture_event(
                    CaptureEventKind::click(false, 0, 340.0, 340.0),
                    outside_btn_id,
                );
            });
        });
        guard.stop();
        poll_vdom_once(&mut vdom);
        assert_eq!(
            deferred_events.borrow().len(),
            1,
            "Stopped WatcherGuard must cancel pending deferred untrusted click microtasks"
        );
    }

    #[test]
    fn test_native_scroll_lock_cssom_snapshot_restore_and_style_attr_hygiene() {
        release_scroll_lock("native-lock-1", None);
        release_scroll_lock("native-lock-2", None);

        // Case A: Pre-existing overflow-y, !important margin-right, and custom color
        let mut base_doc = BaseDocument::new(DocumentConfig::default());
        let doc_root = base_doc.root_node().id;
        let body_id = {
            let mut m = base_doc.mutate();
            let html = m.create_element(qname("html"), vec![]);
            let body = m.create_element(
                qname("body"),
                vec![attr(
                    "style",
                    "overflow-y: scroll; margin-right: 12px !important; color: navy;",
                )],
            );
            m.append_children(html, &[body]);
            m.append_children(doc_root, &[html]);
            body
        };
        let base_rc = Rc::new(RefCell::new(base_doc));
        let doc = Document::from_base(base_rc.clone());
        let body_el = oxidase::dom::Element::from_native(base_rc.clone(), body_id);

        fn app() -> dioxus::prelude::Element {
            rsx! { div {} }
        }
        let mut vdom = VirtualDom::new(app);
        vdom.rebuild_in_place();

        vdom.in_scope(ScopeId::ROOT, || {
            Document::with_current(doc.clone(), || {
                acquire_scroll_lock("native-lock-1");
                let locked_style = body_el.get_attribute("style").unwrap_or_default();
                assert!(locked_style.contains("overflow: hidden;"));
                assert!(locked_style.contains("pointer-events: none;"));
                assert!(locked_style.contains("margin-right: 12px !important;"));
                assert!(locked_style.contains("color: navy;"));
                assert!(!locked_style.contains("overflow-y: scroll"));

                // Release with 40ms restore_delay -> pointer-events restores synchronously (0ms)!
                release_scroll_lock("native-lock-1", Some(40));
                let mid_style = body_el.get_attribute("style").unwrap_or_default();
                assert!(
                    !mid_style.contains("pointer-events"),
                    "pointer-events must be removed synchronously on release: {mid_style}"
                );
                assert!(
                    mid_style.contains("overflow: hidden;"),
                    "overflow: hidden must remain during restore_delay: {mid_style}"
                );
            });
        });

        poll_vdom_once(&mut vdom);
        std::thread::sleep(Duration::from_millis(60));
        poll_vdom_once(&mut vdom);

        let restored_style = body_el.get_attribute("style").unwrap_or_default();
        assert!(!restored_style.contains("overflow: hidden"));
        assert!(!restored_style.contains("pointer-events"));
        assert!(restored_style.contains("overflow-y: scroll;"));
        assert!(restored_style.contains("margin-right: 12px !important;"));
        assert!(restored_style.contains("color: navy;"));

        // Case B: Body with NO initial `style` attribute + prevent_body_pointer_events: false
        let mut clean_doc = BaseDocument::new(DocumentConfig::default());
        let clean_root = clean_doc.root_node().id;
        let clean_body_id = {
            let mut m = clean_doc.mutate();
            let html = m.create_element(qname("html"), vec![]);
            let body = m.create_element(qname("body"), vec![]);
            m.append_children(html, &[body]);
            m.append_children(clean_root, &[html]);
            body
        };
        let clean_rc = Rc::new(RefCell::new(clean_doc));
        let clean_native_doc = Document::from_base(clean_rc.clone());
        let clean_body_el = oxidase::dom::Element::from_native(clean_rc, clean_body_id);

        assert!(!clean_body_el.has_attribute("style"));
        Document::with_current(clean_native_doc, || {
            acquire_scroll_lock_with_options(
                "native-lock-2",
                ScrollLockOptions {
                    prevent_body_pointer_events: false,
                },
            );
            let s = clean_body_el.get_attribute("style").unwrap_or_default();
            assert!(s.contains("overflow: hidden;"));
            assert!(!s.contains("pointer-events"));

            release_scroll_lock("native-lock-2", None);
            assert!(
                !clean_body_el.has_attribute("style"),
                "Empty style attribute must be removed when !had_initial_style_attribute"
            );
        });
    }

    #[test]
    fn test_native_mouse_event_target_and_detail() {
        let mut base_doc = BaseDocument::new(DocumentConfig::default());
        let doc_root = base_doc.root_node().id;
        let (label_id, input_id) = {
            let mut m = base_doc.mutate();
            let html = m.create_element(qname("html"), vec![]);
            let body = m.create_element(qname("body"), vec![]);
            let label = m.create_element(qname("label"), vec![attr("id", "my-label")]);
            let span = m.create_element(qname("span"), vec![attr("id", "label-text")]);
            let input = m.create_element(qname("input"), vec![attr("id", "nested-input")]);
            m.append_children(label, &[span, input]);
            m.append_children(body, &[label]);
            m.append_children(html, &[body]);
            m.append_children(doc_root, &[html]);
            (label, input)
        };
        base_doc.resolve(0.0);

        let base_rc = Rc::new(RefCell::new(base_doc));

        let input_pe = make_pointer_event(
            BlitzPointerId::Mouse,
            MouseEventButton::Main,
            MouseEventButtons::Primary,
            50.0,
            50.0,
        );
        let input_mouse = dioxus::prelude::MouseData::new(
            dioxus_native::NativePointerData::with_target(
                input_pe,
                Rc::downgrade(&base_rc),
                input_id,
            ),
        );

        let target_el = mouse_event_target(&input_mouse).expect("must resolve target Element");
        assert_eq!(target_el.id().as_deref(), Some("nested-input"));
        assert!(
            target_el
                .closest("button, input, select, textarea")
                .is_some()
        );

        // First press on label at (10.0, 10.0)
        let label_pe1 = make_pointer_event(
            BlitzPointerId::Mouse,
            MouseEventButton::Main,
            MouseEventButtons::Primary,
            10.0,
            10.0,
        );
        let label_mouse1 = dioxus::prelude::MouseData::new(
            dioxus_native::NativePointerData::with_target(
                label_pe1,
                Rc::downgrade(&base_rc),
                label_id,
            ),
        );
        let label_el = mouse_event_target(&label_mouse1).expect("must resolve label Element");
        assert_eq!(label_el.id().as_deref(), Some("my-label"));
        assert!(
            label_el
                .closest("button, input, select, textarea")
                .is_none()
        );

        let detail_1 = mouse_event_detail(&label_mouse1);
        assert_eq!(detail_1, 1);

        // Wait > 50us so second distinct click increments detail to 2
        std::thread::sleep(Duration::from_micros(120));
        let label_pe2 = make_pointer_event(
            BlitzPointerId::Mouse,
            MouseEventButton::Main,
            MouseEventButtons::Primary,
            11.0,
            10.5,
        );
        let label_mouse2 = dioxus::prelude::MouseData::new(
            dioxus_native::NativePointerData::with_target(
                label_pe2,
                Rc::downgrade(&base_rc),
                label_id,
            ),
        );
        assert_eq!(mouse_event_detail(&label_mouse2), 2);

        // Secondary button returns 1
        let right_click = dioxus::prelude::MouseData::new(
            dioxus_native::NativePointerData::with_target(
                make_pointer_event(
                    BlitzPointerId::Mouse,
                    MouseEventButton::Secondary,
                    MouseEventButtons::Secondary,
                    11.0,
                    10.5,
                ),
                Rc::downgrade(&base_rc),
                label_id,
            ),
        );
        assert_eq!(mouse_event_detail(&right_click), 1);
    }
}
