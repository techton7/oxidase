use oxidase::dom::observer::observe_pointer_up as observer_observe_pointer_up;
#[cfg(not(feature = "native"))]
use oxidase::dom::Element;
use oxidase::dom::{Document, observe_pointer_up};

#[cfg(not(feature = "native"))]
#[test]
fn test_mock_batch1_substrate_apis() {
    let doc = Document::mock();
    assert!(!doc.has_text_selection());

    let guard = observe_pointer_up(|| {}).expect("mock observe_pointer_up must succeed");
    drop(guard);
    let guard2 = observer_observe_pointer_up(|| {}).expect("re-export must succeed");
    drop(guard2);

    let el = Element::mock();
    assert!(el.query_selector_all("button").is_empty());
    assert!(el.click_sync().is_ok());
    assert_eq!(el.selection_range(), None);
    assert!(el.set_selection_range(0, 3).is_ok());
    assert!(el.select().is_ok());
    assert!(el.set_input_value("123456").is_ok());
}

#[cfg(feature = "native")]
mod native_tests {
    use super::*;
    use blitz_dom::{
        Attribute, BaseDocument, DocumentConfig, EventDriver, LocalName, NoopEventHandler,
        QualName, ns,
    };
    use blitz_traits::events::{
        BlitzPointerEvent, BlitzPointerId, MouseEventButton, MouseEventButtons, PointerCoords,
        PointerDetails, UiEvent,
    };
    use oxidase::dom::observer::{CaptureEventKind, observe_capture_events};
    use std::cell::RefCell;
    use std::rc::Rc;

    fn qname(tag: &'static str) -> QualName {
        QualName::new(None, ns!(html), LocalName::from(tag))
    }

    fn attr(name: &'static str, val: &str) -> Attribute {
        Attribute {
            name: QualName::new(None, ns!(), LocalName::from(name)),
            value: val.to_string(),
        }
    }

    fn mk_pe(x: f32, y: f32, buttons: MouseEventButtons) -> BlitzPointerEvent {
        BlitzPointerEvent {
            id: BlitzPointerId::Mouse,
            is_primary: true,
            coords: PointerCoords {
                page_x: x,
                page_y: y,
                screen_x: x,
                screen_y: y,
                client_x: x,
                client_y: y,
            },
            button: MouseEventButton::Main,
            buttons,
            mods: Default::default(),
            details: PointerDetails::default(),
            element: Default::default(),
            active_pointers: Default::default(),
        }
    }

    #[test]
    fn test_native_document_has_text_selection() {
        let base_doc = Rc::new(RefCell::new(BaseDocument::new(DocumentConfig::default())));
        let doc = Document::from_base(base_doc.clone());

        let paragraph_id = {
            let mut base = base_doc.borrow_mut();
            let root_id = base.root_node().id;
            let mut mutator = base.mutate();
            let html = mutator.create_element(qname("html"), vec![]);
            let body = mutator.create_element(qname("body"), vec![]);
            let p = mutator.create_element(qname("p"), vec![attr("id", "selectable-p")]);
            let txt = mutator.create_text_node("Hello HoverCard");
            mutator.append_children(p, &[txt]);
            mutator.append_children(body, &[p]);
            mutator.append_children(html, &[body]);
            mutator.append_children(root_id, &[html]);
            p
        };
        doc.resolve(0.0);

        assert!(!doc.has_text_selection());

        // Collapsed selection is not an active text selection
        base_doc
            .borrow_mut()
            .set_text_selection(paragraph_id, 2, paragraph_id, 2);
        assert!(!doc.has_text_selection());

        // Non-empty selection returns true
        base_doc
            .borrow_mut()
            .set_text_selection(paragraph_id, 0, paragraph_id, 5);
        assert!(doc.has_text_selection());

        // Clearing selection returns false
        base_doc.borrow_mut().clear_text_selection();
        assert!(!doc.has_text_selection());
    }

    #[test]
    fn test_native_observe_pointer_up_and_guard_cleanup() {
        let base_doc = Rc::new(RefCell::new(BaseDocument::new(DocumentConfig::default())));
        let doc = Document::from_base(base_doc.clone());

        {
            let mut base = base_doc.borrow_mut();
            let root_id = base.root_node().id;
            let mut mutator = base.mutate();
            let html = mutator.create_element(qname("html"), vec![]);
            let body = mutator.create_element(qname("body"), vec![]);
            let target = mutator.create_element(
                qname("div"),
                vec![
                    attr("id", "up-target"),
                    attr("style", "width: 200px; height: 200px; display: block;"),
                ],
            );
            mutator.append_children(body, &[target]);
            mutator.append_children(html, &[body]);
            mutator.append_children(root_id, &[html]);
        }
        doc.resolve(0.0);

        let count = Rc::new(RefCell::new(0usize));
        let count_clone = count.clone();
        let guard = observer_observe_pointer_up(move || {
            *count_clone.borrow_mut() += 1;
        })
        .expect("observe_pointer_up via dom::observer must succeed");
        let guard_reexport = observe_pointer_up(|| {}).expect("re-exported observe_pointer_up must succeed");

        doc.handle_ui_event(UiEvent::PointerDown(mk_pe(
            20.0,
            20.0,
            MouseEventButtons::Primary,
        )));
        assert_eq!(*count.borrow(), 0);

        doc.handle_ui_event(UiEvent::PointerUp(mk_pe(
            20.0,
            20.0,
            MouseEventButtons::None,
        )));
        assert_eq!(*count.borrow(), 1);

        doc.handle_ui_event(UiEvent::PointerUp(mk_pe(
            50.0,
            50.0,
            MouseEventButtons::None,
        )));
        assert_eq!(*count.borrow(), 2);

        drop(guard);
        drop(guard_reexport);
        doc.handle_ui_event(UiEvent::PointerUp(mk_pe(
            50.0,
            50.0,
            MouseEventButtons::None,
        )));
        assert_eq!(
            *count.borrow(),
            2,
            "Dropped observe_pointer_up guard must remove observer"
        );
    }

    #[test]
    fn test_native_element_query_selector_all() {
        let base_doc = Rc::new(RefCell::new(BaseDocument::new(DocumentConfig::default())));
        let doc = Document::from_base(base_doc.clone());

        {
            let mut base = base_doc.borrow_mut();
            let root_id = base.root_node().id;
            let mut mutator = base.mutate();
            let html = mutator.create_element(qname("html"), vec![]);
            let body = mutator.create_element(qname("body"), vec![]);
            let scope = mutator.create_element(qname("div"), vec![attr("id", "scope")]);
            let btn1 = mutator.create_element(qname("button"), vec![attr("id", "btn-1")]);
            let nested = mutator.create_element(qname("div"), vec![attr("id", "nested")]);
            let link1 = mutator.create_element(
                qname("a"),
                vec![attr("id", "link-1"), attr("href", "https://example.com")],
            );
            let btn2 = mutator.create_element(qname("button"), vec![attr("id", "btn-2")]);
            let outside_btn =
                mutator.create_element(qname("button"), vec![attr("id", "outside-btn")]);

            mutator.append_children(nested, &[link1, btn2]);
            mutator.append_children(scope, &[btn1, nested]);
            mutator.append_children(body, &[scope, outside_btn]);
            mutator.append_children(html, &[body]);
            mutator.append_children(root_id, &[html]);
        }
        doc.resolve(0.0);

        let scope = doc.element_by_id("scope").expect("scope must exist");
        let matched = scope.query_selector_all("button, a[href]");
        let ids: Vec<Option<String>> = matched.iter().map(|el| el.id()).collect();
        assert_eq!(
            ids,
            vec![
                Some("btn-1".to_string()),
                Some("link-1".to_string()),
                Some("btn-2".to_string()),
            ]
        );

        // Invalid CSS selector fails gracefully with empty Vec
        assert!(scope.query_selector_all("[[invalid").is_empty());
    }

    #[test]
    fn test_native_element_click_sync() {
        let base_doc = Rc::new(RefCell::new(BaseDocument::new(DocumentConfig::default())));
        let doc = Document::from_base(base_doc.clone());

        {
            let mut base = base_doc.borrow_mut();
            let root_id = base.root_node().id;
            let mut mutator = base.mutate();
            let html = mutator.create_element(qname("html"), vec![]);
            let body = mutator.create_element(qname("body"), vec![]);
            let checkbox = mutator.create_element(
                qname("input"),
                vec![attr("id", "cb-1"), attr("type", "checkbox")],
            );
            mutator.append_children(body, &[checkbox]);
            mutator.append_children(html, &[body]);
            mutator.append_children(root_id, &[html]);
        }
        doc.resolve(0.0);

        let observed_clicks = Rc::new(RefCell::new(Vec::<(Option<String>, bool)>::new()));
        let obs_clone = observed_clicks.clone();
        let _capture_guard = observe_capture_events(move |evt| {
            if let CaptureEventKind::Click { is_trusted, .. } = evt.kind {
                obs_clone.borrow_mut().push((evt.target_id, is_trusted));
            }
        })
        .expect("observe_capture_events must succeed");

        let cb = doc.element_by_id("cb-1").expect("cb-1 must exist");
        cb.click_sync().expect("click_sync must succeed");
        assert!(base_doc.borrow().has_pending_events());

        {
            let mut doc_ref = base_doc.borrow_mut();
            EventDriver::new(&mut *doc_ref, NoopEventHandler).flush_pending_events();
        }

        assert_eq!(
            *observed_clicks.borrow(),
            vec![(Some("cb-1".to_string()), false)]
        );
        let base = base_doc.borrow();
        let is_checked = base
            .get_element_by_id("cb-1")
            .and_then(|nid| base.get_node(nid))
            .and_then(|n| n.element_data())
            .and_then(|el| el.checkbox_input_checked());
        assert_eq!(is_checked, Some(true));
    }

    #[test]
    fn test_native_input_selection_and_value_ascii_and_unicode() {
        let base_doc = Rc::new(RefCell::new(BaseDocument::new(DocumentConfig::default())));
        let doc = Document::from_base(base_doc.clone());

        let (input_nid, div_nid) = {
            let mut base = base_doc.borrow_mut();
            let root_id = base.root_node().id;
            let mut mutator = base.mutate();
            let html = mutator.create_element(qname("html"), vec![]);
            let body = mutator.create_element(qname("body"), vec![]);
            let input = mutator.create_element(
                qname("input"),
                vec![attr("id", "otp-input"), attr("type", "text")],
            );
            let non_input = mutator.create_element(qname("div"), vec![attr("id", "non-input")]);
            mutator.append_children(body, &[input, non_input]);
            mutator.append_children(html, &[body]);
            mutator.append_children(root_id, &[html]);
            (input, non_input)
        };
        // Intentionally do NOT call doc.resolve(0.0) first for one check:
        // ensure_native_text_input_initialized handles lazy TextInputData creation.
        let input = doc.element_by_id("otp-input").expect("otp-input must exist");
        let non_input = doc.element_by_id("non-input").expect("non-input must exist");
        let _ = div_nid;

        assert_eq!(non_input.selection_range(), None);
        assert!(non_input.set_selection_range(0, 1).is_err());
        assert!(non_input.select().is_err());
        assert!(non_input.set_input_value("abc").is_err());

        // ASCII round-trip
        input
            .set_input_value("123456")
            .expect("set_input_value ASCII");
        input
            .set_selection_range(2, 5)
            .expect("set_selection_range ASCII");
        assert_eq!(input.selection_range(), Some((2, 5)));

        input.select().expect("select ASCII");
        assert_eq!(input.selection_range(), Some((0, 6)));

        // Multi-byte UTF-8 + non-BMP UTF-16 surrogate pair round-trip:
        // 'a' (1B, 1 UTF-16), '😀' (4B, 2 UTF-16), '한' (3B, 1 UTF-16), 'b' (1B, 1 UTF-16)
        input
            .set_input_value("a😀한b")
            .expect("set_input_value unicode");
        input.select().expect("select unicode");
        assert_eq!(input.selection_range(), Some((0, 5)));

        // UTF-16 range 1..4 spans '😀한' => UTF-8 byte range 1..8
        input
            .set_selection_range(1, 4)
            .expect("set_selection_range unicode 1..4");
        assert_eq!(input.selection_range(), Some((1, 4)));
        {
            let base = base_doc.borrow();
            let raw_range = base
                .get_node(input_nid)
                .and_then(|n| n.element_data())
                .and_then(|el| el.text_input_data())
                .map(|ti| ti.editor.raw_selection().text_range());
            assert_eq!(raw_range, Some(1..8));
        }

        // UTF-16 range 3..5 spans '한b' => UTF-8 byte range 5..9
        input
            .set_selection_range(3, 5)
            .expect("set_selection_range unicode 3..5");
        assert_eq!(input.selection_range(), Some((3, 5)));
        {
            let base = base_doc.borrow();
            let raw_range = base
                .get_node(input_nid)
                .and_then(|n| n.element_data())
                .and_then(|el| el.text_input_data())
                .map(|ti| ti.editor.raw_selection().text_range());
            assert_eq!(raw_range, Some(5..9));
        }

        // Out-of-bounds offsets clamp cleanly
        input
            .set_selection_range(99, 120)
            .expect("clamped out-of-bounds range");
        assert_eq!(input.selection_range(), Some((5, 5)));
    }
}
