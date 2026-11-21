#[cfg(feature = "native")]
use oxidase::dom::Document;
use oxidase::dom::observer::{
    ResizeObserverBox, observe_element_resize, observe_element_resize_with_box,
};

fn block_on<F: std::future::Future>(mut f: F) -> F::Output {
    use std::pin::Pin;
    use std::task::{Context, RawWaker, RawWakerVTable, Waker};
    fn dummy_raw_waker() -> RawWaker {
        fn no_op(_: *const ()) {}
        fn clone(_: *const ()) -> RawWaker {
            dummy_raw_waker()
        }
        static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, no_op, no_op, no_op);
        RawWaker::new(std::ptr::null(), &VTABLE)
    }
    let waker = unsafe { Waker::from_raw(dummy_raw_waker()) };
    let mut cx = Context::from_waker(&waker);
    let mut pin = unsafe { Pin::new_unchecked(&mut f) };
    match pin.as_mut().poll(&mut cx) {
        std::task::Poll::Ready(v) => v,
        std::task::Poll::Pending => panic!("Future did not resolve immediately"),
    }
}

#[test]
fn test_resize_observer_box_enum_defaults() {
    assert_eq!(ResizeObserverBox::default(), ResizeObserverBox::BorderBox);
    assert_ne!(ResizeObserverBox::BorderBox, ResizeObserverBox::ContentBox);
}

#[test]
fn test_mock_element_scroll_metrics_and_sync_methods() {
    let el = oxidase::dom::element::Element::mock();
    assert_eq!(el.scroll_x_sync().unwrap(), 0.0);
    assert_eq!(el.scroll_y_sync().unwrap(), 0.0);
    assert_eq!(el.scroll_width_sync().unwrap(), 0.0);
    assert_eq!(el.scroll_height_sync().unwrap(), 0.0);
    assert_eq!(el.client_width_sync().unwrap(), 0.0);
    assert_eq!(el.client_height_sync().unwrap(), 0.0);
    assert!(el.scroll_to_sync(100.0, 50.0).is_ok());
    assert!(el.scroll_by_sync(10.0, 10.0).is_ok());
    assert!(el.set_style_property("user-select", "none").is_ok());
    assert!(el.remove_style_property("user-select").is_ok());

    assert_eq!(block_on(el.scroll_x()).unwrap(), 0.0);
    assert_eq!(block_on(el.scroll_y()).unwrap(), 0.0);
    assert_eq!(block_on(el.scroll_width()).unwrap(), 0.0);
    assert_eq!(block_on(el.scroll_height()).unwrap(), 0.0);
    assert_eq!(block_on(el.client_width()).unwrap(), 0.0);
    assert_eq!(block_on(el.client_height()).unwrap(), 0.0);
    assert!(block_on(el.scroll_to(200.0, 150.0)).is_ok());
    assert!(block_on(el.scroll_by(20.0, 15.0)).is_ok());
}

#[test]
fn test_mock_resize_observer_registration() {
    let guard_default = observe_element_resize("target", |_entry| {}).unwrap();
    assert!(guard_default.name().contains("resize"));

    let guard_border =
        observe_element_resize_with_box("target", ResizeObserverBox::BorderBox, |_entry| {})
            .unwrap();
    assert!(guard_border.name().contains("resize"));

    let guard_content =
        observe_element_resize_with_box("target", ResizeObserverBox::ContentBox, |_entry| {})
            .unwrap();
    assert!(guard_content.name().contains("resize"));
}

#[cfg(feature = "native")]
#[test]
fn test_native_element_scroll_metrics_and_measure_without_animation() {
    use blitz_dom::{Attribute, BaseDocument, DocumentConfig, LocalName, QualName, ns};
    use std::cell::RefCell;
    use std::rc::Rc;

    let base_doc = Rc::new(RefCell::new(BaseDocument::new(DocumentConfig::default())));
    let doc = Document::from_base(base_doc.clone());

    fn qname(tag: &'static str) -> QualName {
        QualName::new(None, ns!(html), LocalName::from(tag))
    }
    fn attr(name: &'static str, val: &str) -> Attribute {
        Attribute {
            name: QualName::new(None, ns!(), LocalName::from(name)),
            value: val.to_string(),
        }
    }

    {
        let mut base = base_doc.borrow_mut();
        let doc_root = base.root_node().id;
        let mut mutator = base.mutate();

        let html_id = mutator.create_element(qname("html"), vec![]);
        let body_id = mutator.create_element(qname("body"), vec![]);

        // Scroll container with border and padding
        let scroll_container = mutator.create_element(
            qname("div"),
            vec![
                attr("id", "scroll-box"),
                attr(
                    "style",
                    "width: 200px; height: 100px; overflow: scroll; border: 5px solid black; padding: 10px;",
                ),
            ],
        );

        // Tall content inside scroll container to force overflow
        let content_item = mutator.create_element(
            qname("div"),
            vec![
                attr("id", "content-box"),
                attr("style", "width: 400px; height: 500px;"),
            ],
        );

        mutator.append_children(scroll_container, &[content_item]);
        mutator.append_children(body_id, &[scroll_container]);
        mutator.append_children(html_id, &[body_id]);
        mutator.append_children(doc_root, &[html_id]);
    }

    base_doc.borrow_mut().resolve(0.0);

    Document::with_current(doc.clone(), || {
        let el = doc.element_by_id("scroll-box").expect("scroll-box found");

        // Style property helpers
        assert!(el.set_style_property("user-select", "none").is_ok());
        assert!(el.remove_style_property("user-select").is_ok());

        // CSSOM metrics
        let cw = el.client_width_sync().expect("client_width");
        let ch = el.client_height_sync().expect("client_height");
        let sw = el.scroll_width_sync().expect("scroll_width");
        let sh = el.scroll_height_sync().expect("scroll_height");

        assert!(cw > 0.0, "client_width must be positive: {cw}");
        assert!(ch > 0.0, "client_height must be positive: {ch}");
        assert!(
            sw >= cw,
            "scroll_width ({sw}) must be >= client_width ({cw})"
        );
        assert!(
            sh >= ch,
            "scroll_height ({sh}) must be >= client_height ({ch})"
        );

        // Scrolling
        assert_eq!(el.scroll_x_sync().unwrap(), 0.0);
        assert_eq!(el.scroll_y_sync().unwrap(), 0.0);
        el.scroll_to_sync(50.0, 80.0).expect("scroll_to_sync");
        assert_eq!(el.scroll_x_sync().unwrap(), 50.0);
        assert_eq!(el.scroll_y_sync().unwrap(), 80.0);

        el.scroll_by_sync(10.0, -20.0).expect("scroll_by_sync");
        assert_eq!(el.scroll_x_sync().unwrap(), 60.0);
        assert_eq!(el.scroll_y_sync().unwrap(), 60.0);

        // measure_without_animation
        let dims = doc
            .measure_without_animation("scroll-box")
            .expect("measure_without_animation");
        assert!(dims.0 > 0.0, "width must be positive: {}", dims.0);
        assert!(dims.1 > 0.0, "height must be positive: {}", dims.1);

        // Nonexistent element measurement returns None
        assert_eq!(doc.measure_without_animation("nonexistent"), None);

        // Box model rect measurement
        let border_rect = doc
            .measure_rect_with_box("scroll-box", ResizeObserverBox::BorderBox)
            .expect("BorderBox rect");
        let content_rect = doc
            .measure_rect_with_box("scroll-box", ResizeObserverBox::ContentBox)
            .expect("ContentBox rect");

        assert!(
            content_rect.width <= border_rect.width,
            "ContentBox width ({}) <= BorderBox width ({})",
            content_rect.width,
            border_rect.width
        );
        assert!(
            content_rect.height <= border_rect.height,
            "ContentBox height ({}) <= BorderBox height ({})",
            content_rect.height,
            border_rect.height
        );
    });
}

#[cfg(feature = "native")]
#[test]
fn test_native_measure_without_animation_uses_last_animation_time_and_preserves_other_events() {
    use blitz_dom::{
        Attribute, BaseDocument, DocumentConfig, LocalName, QualName, UserAgentStylesheet, ns,
    };
    use blitz_traits::events::DomEventData;
    use std::cell::RefCell;
    use std::rc::Rc;

    let base_doc = Rc::new(RefCell::new(BaseDocument::new(DocumentConfig::default())));
    fn qname(tag: &'static str) -> QualName {
        QualName::new(None, ns!(html), LocalName::from(tag))
    }
    fn attr(name: &'static str, val: &str) -> Attribute {
        Attribute {
            name: QualName::new(None, ns!(), LocalName::from(name)),
            value: val.to_string(),
        }
    }

    let (measured_id, other_id) = {
        let mut base = base_doc.borrow_mut();
        base.add_user_agent_stylesheet(UserAgentStylesheet::parse(
            r#"
            @keyframes fade-probe {
                from { opacity: 1; }
                to { opacity: 0; }
            }
            #measured-box, #other-box {
                animation: fade-probe 1s linear;
            }
            "#,
        ));

        let doc_root = base.root_node().id;
        let mut mutator = base.mutate();
        let html_id = mutator.create_element(qname("html"), vec![]);
        let body_id = mutator.create_element(qname("body"), vec![]);
        let measured_id = mutator.create_element(
            qname("div"),
            vec![
                attr("id", "measured-box"),
                attr("style", "width: 120px; height: 60px;"),
            ],
        );
        let other_id = mutator.create_element(
            qname("div"),
            vec![
                attr("id", "other-box"),
                attr("style", "width: 80px; height: 40px;"),
            ],
        );
        mutator.append_children(body_id, &[measured_id, other_id]);
        mutator.append_children(html_id, &[body_id]);
        mutator.append_children(doc_root, &[html_id]);
        (measured_id, other_id)
    };

    let doc = Document::from_base(base_doc.clone());
    doc.resolve(42.0);
    assert_eq!(base_doc.borrow().last_animation_time(), 42.0);

    let prev_anim_len = base_doc.borrow().pending_animation_events.len();
    assert!(
        prev_anim_len >= 2,
        "initial resolve must queue AnimationStart events for both elements"
    );

    // Cancel the unrelated node's animation so the next resolve queues an event for `other_id`.
    base_doc
        .borrow_mut()
        .mutate()
        .set_style_property(other_id, "animation", "none");

    let dims = doc
        .measure_without_animation("measured-box")
        .expect("measure_without_animation must return dimensions");
    assert_eq!(dims, (120.0, 60.0));

    let base = base_doc.borrow();
    assert_eq!(
        base.last_animation_time(),
        42.0,
        "measure_without_animation must reuse base.last_animation_time() rather than wallclock time"
    );
    assert!(
        base.pending_animation_events.len() > prev_anim_len,
        "unrelated node's newly queued animation event must be preserved"
    );
    assert!(
        base.pending_animation_events[prev_anim_len..].iter().any(
            |ev| ev.node_id == other_id && matches!(ev.event, DomEventData::AnimationCancel(_))
        ),
        "unrelated node's AnimationCancel event added during measurement resolve must be kept"
    );
    assert!(
        !base.pending_animation_events[prev_anim_len..]
            .iter()
            .any(|ev| ev.node_id == measured_id),
        "events queued for measured node during measurement probe must be pruned"
    );
}

#[cfg(feature = "native")]
#[test]
fn test_native_is_element_animating_reflects_active_animations_and_transitions() {
    use blitz_dom::{
        Attribute, BaseDocument, DocumentConfig, LocalName, QualName, UserAgentStylesheet, ns,
    };
    use std::cell::RefCell;
    use std::rc::Rc;

    let base_doc = Rc::new(RefCell::new(BaseDocument::new(DocumentConfig::default())));
    fn qname(tag: &'static str) -> QualName {
        QualName::new(None, ns!(html), LocalName::from(tag))
    }
    fn attr(name: &'static str, val: &str) -> Attribute {
        Attribute {
            name: QualName::new(None, ns!(), LocalName::from(name)),
            value: val.to_string(),
        }
    }

    let (anim_id, trans_id, static_id) = {
        let mut base = base_doc.borrow_mut();
        base.add_user_agent_stylesheet(UserAgentStylesheet::parse(
            r#"
            @keyframes slide-active {
                from { opacity: 1; }
                to { opacity: 0; }
            }
            #anim-el {
                animation: slide-active 0.5s linear;
            }
            #trans-el {
                transition: opacity 0.5s linear;
                opacity: 1;
            }
            "#,
        ));

        let doc_root = base.root_node().id;
        let mut mutator = base.mutate();
        let html_id = mutator.create_element(qname("html"), vec![]);
        let body_id = mutator.create_element(qname("body"), vec![]);
        let anim_id = mutator.create_element(qname("div"), vec![attr("id", "anim-el")]);
        let trans_id = mutator.create_element(qname("div"), vec![attr("id", "trans-el")]);
        let static_id = mutator.create_element(qname("div"), vec![attr("id", "static-el")]);
        mutator.append_children(body_id, &[anim_id, trans_id, static_id]);
        mutator.append_children(html_id, &[body_id]);
        mutator.append_children(doc_root, &[html_id]);
        (anim_id, trans_id, static_id)
    };

    let doc = Document::from_base(base_doc.clone());

    doc.resolve(0.0);
    assert!(doc.is_element_animating("anim-el"));
    assert!(base_doc.borrow().node_has_active_animations(anim_id));
    assert!(!doc.is_element_animating("trans-el"));
    assert!(!doc.is_element_animating("static-el"));
    assert!(!base_doc.borrow().node_has_active_animations(static_id));
    assert!(!doc.is_element_animating("nonexistent"));

    doc.resolve(0.6);
    assert!(!doc.is_element_animating("anim-el"));

    base_doc
        .borrow_mut()
        .mutate()
        .set_style_property(trans_id, "opacity", "0");
    doc.resolve(1.0);
    assert!(doc.is_element_animating("trans-el"));
    assert!(base_doc.borrow().node_has_active_transitions(trans_id));

    doc.resolve(1.6);
    assert!(!doc.is_element_animating("trans-el"));
}

#[cfg(feature = "native")]
#[test]
fn test_native_transition_observer_no_animation_gated_by_resolve_generation() {
    use blitz_dom::{Attribute, BaseDocument, DocumentConfig, LocalName, QualName, ns};
    use oxidase::dom::observer::{TransitionLifecycleEvent, observe_transition_lifecycle};
    use std::cell::RefCell;
    use std::rc::Rc;

    let base_doc = Rc::new(RefCell::new(BaseDocument::new(DocumentConfig::default())));
    fn qname(tag: &'static str) -> QualName {
        QualName::new(None, ns!(html), LocalName::from(tag))
    }
    fn attr(name: &'static str, val: &str) -> Attribute {
        Attribute {
            name: QualName::new(None, ns!(), LocalName::from(name)),
            value: val.to_string(),
        }
    }

    {
        let mut base = base_doc.borrow_mut();
        let doc_root = base.root_node().id;
        let mut mutator = base.mutate();
        let html_id = mutator.create_element(qname("html"), vec![]);
        let body_id = mutator.create_element(qname("body"), vec![]);
        let target_id = mutator.create_element(
            qname("div"),
            vec![
                attr("id", "still-el"),
                attr("style", "width: 100px; height: 50px;"),
            ],
        );
        mutator.append_children(body_id, &[target_id]);
        mutator.append_children(html_id, &[body_id]);
        mutator.append_children(doc_root, &[html_id]);
    }

    let doc = Document::from_base(base_doc.clone());
    doc.resolve(0.0);
    let gen_before = base_doc.borrow().resolve_generation();
    assert!(gen_before >= 1);

    Document::with_current(doc.clone(), || {
        let events_early = Rc::new(RefCell::new(Vec::new()));
        let early_clone = events_early.clone();
        let _guard_early = observe_transition_lifecycle("still-el", move |ev| {
            early_clone.borrow_mut().push(ev);
        })
        .expect("observe_transition_lifecycle must succeed");

        // Pumping observers before resolve() advances resolve_generation must NOT fire NoAnimation.
        doc.pump_hosted_observers();
        assert!(
            events_early.borrow().is_empty(),
            "NoAnimation must not fire before resolve() advances resolve_generation"
        );

        // Advance BaseDocument resolve_generation externally, then register a second observer on the same target.
        base_doc.borrow_mut().resolve(0.1);
        assert_eq!(base_doc.borrow().resolve_generation(), gen_before + 1);

        let events_late = Rc::new(RefCell::new(Vec::new()));
        let late_clone = events_late.clone();
        let _guard_late = observe_transition_lifecycle("still-el", move |ev| {
            late_clone.borrow_mut().push(ev);
        })
        .expect("second observe_transition_lifecycle must succeed");

        // Only the earlier-generation observer fires; the same-generation observer remains pending.
        doc.pump_hosted_observers();
        assert_eq!(
            *events_early.borrow(),
            vec![TransitionLifecycleEvent::NoAnimation]
        );
        assert!(
            events_late.borrow().is_empty(),
            "same-generation observer for the same target_id must not be dropped or fired prematurely"
        );

        // Advancing resolve_generation again fires NoAnimation for the second observer.
        doc.resolve(0.2);
        assert_eq!(
            *events_late.borrow(),
            vec![TransitionLifecycleEvent::NoAnimation]
        );
    });
}

#[cfg(feature = "native")]
#[test]
fn test_native_measure_without_animation_ignores_active_height_animation() {
    use blitz_dom::{
        Attribute, BaseDocument, DocumentConfig, LocalName, QualName, UserAgentStylesheet, ns,
    };
    use std::cell::RefCell;
    use std::rc::Rc;

    let base_doc = Rc::new(RefCell::new(BaseDocument::new(DocumentConfig::default())));
    fn qname(tag: &'static str) -> QualName {
        QualName::new(None, ns!(html), LocalName::from(tag))
    }
    fn attr(name: &'static str, val: &str) -> Attribute {
        Attribute {
            name: QualName::new(None, ns!(), LocalName::from(name)),
            value: val.to_string(),
        }
    }

    let anim_id = {
        let mut base = base_doc.borrow_mut();
        base.add_user_agent_stylesheet(UserAgentStylesheet::parse(
            r#"
            @keyframes clamp-height {
                from { height: 0px; }
                to { height: 120px; }
            }
            #animated-height-box {
                width: 200px;
                overflow: hidden;
                animation: clamp-height 1s linear;
            }
            "#,
        ));

        let doc_root = base.root_node().id;
        let mut mutator = base.mutate();
        let html_id = mutator.create_element(qname("html"), vec![]);
        let body_id = mutator.create_element(qname("body"), vec![]);
        let anim_id = mutator.create_element(qname("div"), vec![attr("id", "animated-height-box")]);
        let child_id = mutator.create_element(
            qname("div"),
            vec![attr("style", "width: 200px; height: 120px;")],
        );
        mutator.append_children(anim_id, &[child_id]);
        mutator.append_children(body_id, &[anim_id]);
        mutator.append_children(html_id, &[body_id]);
        mutator.append_children(doc_root, &[html_id]);
        anim_id
    };

    let doc = Document::from_base(base_doc.clone());
    doc.resolve(0.0);

    assert!(doc.is_element_animating("animated-height-box"));
    let clamped_height = base_doc
        .borrow()
        .get_node(anim_id)
        .expect("animated-height-box node")
        .unrounded_layout()
        .size
        .height as f64;
    assert_eq!(
        clamped_height, 0.0,
        "active @keyframes animation must initially clamp height to 0px"
    );

    let dims = doc
        .measure_without_animation("animated-height-box")
        .expect("measure_without_animation must return natural dimensions");
    assert_eq!(dims, (200.0, 120.0));
}

#[cfg(feature = "native")]
#[test]
fn test_native_wheel_scroll_observer_deferred_outside_base_borrow() {
    use blitz_dom::{Attribute, BaseDocument, DocumentConfig, LocalName, QualName, ns};
    use blitz_traits::events::{
        BlitzPointerEvent, BlitzPointerId, BlitzWheelDelta, BlitzWheelEvent, BlitzWheelPhase,
        BlitzWheelSource, MouseEventButton, MouseEventButtons, PointerCoords, PointerDetails,
        UiEvent,
    };
    use dioxus::prelude::keyboard_types::Modifiers;
    use oxidase::dom::observer::observe_element_scroll;
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    let mut raw_base = BaseDocument::new(DocumentConfig::default());
    {
        let mut vp = raw_base.viewport_mut();
        vp.window_size = (800, 600);
        vp.hidpi_scale = 1.0;
    }
    let base_doc = Rc::new(RefCell::new(raw_base));

    fn qname(tag: &'static str) -> QualName {
        QualName::new(None, ns!(html), LocalName::from(tag))
    }
    fn attr(name: &'static str, val: &str) -> Attribute {
        Attribute {
            name: QualName::new(None, ns!(), LocalName::from(name)),
            value: val.to_string(),
        }
    }

    {
        let mut base = base_doc.borrow_mut();
        let doc_root = base.root_node().id;
        let mut mutator = base.mutate();
        let html_id = mutator.create_element(qname("html"), vec![]);
        let body_id = mutator.create_element(qname("body"), vec![]);
        let scroll_vp = mutator.create_element(
            qname("div"),
            vec![
                attr("id", "scroll-vp"),
                attr(
                    "style",
                    "display: block; position: absolute; left: 0px; top: 0px; width: 200px; height: 100px; overflow-y: auto;",
                ),
            ],
        );
        let child = mutator.create_element(
            qname("div"),
            vec![
                attr("id", "scroll-vp-child"),
                attr("style", "display: block; width: 200px; height: 400px;"),
            ],
        );
        let thumb = mutator.create_element(
            qname("div"),
            vec![
                attr("id", "scroll-vp-thumb"),
                attr("style", "display: block; width: 6px; height: 25px;"),
            ],
        );
        mutator.append_children(scroll_vp, &[child]);
        mutator.append_children(body_id, &[scroll_vp, thumb]);
        mutator.append_children(html_id, &[body_id]);
        mutator.append_children(doc_root, &[html_id]);
    }

    let doc = Document::from_base(base_doc.clone());
    doc.resolve(0.0);

    Document::with_current(doc.clone(), || {
        let observed_scroll_top = Rc::new(Cell::new(None::<f64>));
        let style_mutated = Rc::new(Cell::new(false));
        let redraw_count = Rc::new(Cell::new(0usize));

        let rc_redraw = redraw_count.clone();
        let _redraw_guard = oxidase::frame::set_host_redraw_requester(move || {
            rc_redraw.set(rc_redraw.get() + 1);
        });

        let top_cell = observed_scroll_top.clone();
        let style_cell = style_mutated.clone();
        let _scroll_guard = observe_element_scroll("scroll-vp", move || {
            let current_doc = Document::current().expect("current document must be active");
            let vp_el = current_doc
                .element_by_id("scroll-vp")
                .expect("scroll-vp must exist");
            let scroll_top = vp_el
                .scroll_y_sync()
                .expect("scroll_y_sync must succeed outside BaseDocument borrow");
            let scroll_height = vp_el
                .scroll_height_sync()
                .expect("scroll_height_sync must succeed outside BaseDocument borrow");
            let client_height = vp_el
                .client_height_sync()
                .expect("client_height_sync must succeed outside BaseDocument borrow");
            assert!(scroll_height > client_height);
            top_cell.set(Some(scroll_top));

            let thumb_el = current_doc
                .element_by_id("scroll-vp-thumb")
                .expect("scroll-vp-thumb must exist");
            thumb_el
                .set_style_property(
                    "transform",
                    &format!("translate3d(0px, {scroll_top}px, 0px)"),
                )
                .expect("set_style_property must succeed outside BaseDocument borrow");
            style_cell.set(true);
        })
        .expect("observe_element_scroll must succeed");

        let coords = PointerCoords {
            page_x: 50.0,
            page_y: 50.0,
            client_x: 50.0,
            client_y: 50.0,
            screen_x: 50.0,
            screen_y: 50.0,
        };
        doc.handle_ui_event(UiEvent::PointerMove(BlitzPointerEvent {
            id: BlitzPointerId::Mouse,
            is_primary: true,
            coords,
            button: MouseEventButton::Main,
            buttons: MouseEventButtons::empty(),
            mods: Modifiers::empty(),
            details: PointerDetails::default(),
            element: Default::default(),
            active_pointers: Default::default(),
        }));
        doc.handle_ui_event(UiEvent::Wheel(BlitzWheelEvent {
            delta: BlitzWheelDelta::Pixels(0.0, -40.0),
            phase: BlitzWheelPhase::Moved,
            source: BlitzWheelSource::Wheel,
            coords,
            buttons: MouseEventButtons::empty(),
            mods: Modifiers::empty(),
            element: Default::default(),
        }));

        assert!(
            observed_scroll_top.get().is_none(),
            "scroll observer must be deferred while BaseDocument is mutably borrowed in capture_hook"
        );
        assert!(
            redraw_count.get() >= 1,
            "deferring scroll observer must request a host redraw"
        );

        oxidase::frame::tick(std::time::Duration::from_millis(16));

        let top = observed_scroll_top
            .get()
            .expect("deferred scroll observer must run during frame::tick");
        assert!(top > 0.0, "expected scroll_top > 0.0, got {top}");
        assert!(
            style_mutated.get(),
            "scroll observer must successfully mutate style outside BaseDocument borrow"
        );
    });
}
