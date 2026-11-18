use oxidase::dom::Document;
use oxidase::error::HostError;

#[test]
fn test_default_unattached_collect_form_data_fails_closed() {
    let doc = Document::default();
    let res = doc.collect_form_data("checkout-form");
    assert!(matches!(res, Err(HostError::Unsupported(_))));
}

#[cfg(feature = "native")]
mod native_tests {
    use blitz_dom::{Attribute, BaseDocument, DocumentConfig, QualName, ns};
    use blitz_traits::events::{
        BlitzPointerEvent, BlitzPointerId, MouseEventButton, MouseEventButtons, PointerCoords,
        PointerDetails, UiEvent,
    };
    use dioxus::prelude::keyboard_types::Modifiers;
    use oxidase::dom::Document;
    use oxidase::error::HostError;
    use oxidase::watcher::watch_form_reset;
    use std::cell::Cell;
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

    fn click_at(doc: &Document, x: f32, y: f32) {
        let down = BlitzPointerEvent {
            id: BlitzPointerId::Mouse,
            is_primary: true,
            coords: PointerCoords {
                page_x: x,
                page_y: y,
                client_x: x,
                client_y: y,
                screen_x: x,
                screen_y: y,
            },
            button: MouseEventButton::Main,
            buttons: MouseEventButtons::from(MouseEventButton::Main),
            mods: Modifiers::empty(),
            details: PointerDetails::default(),
            element: Default::default(),
            active_pointers: Default::default(),
        };
        let up = BlitzPointerEvent {
            buttons: MouseEventButtons::None,
            ..down.clone()
        };
        doc.handle_ui_event(UiEvent::PointerDown(down));
        doc.handle_ui_event(UiEvent::PointerUp(up));
    }

    #[test]
    fn test_native_reset_button_click_named_external_and_anonymous_forms() {
        let mut base_doc = BaseDocument::new(DocumentConfig::default());
        {
            let mut vp = base_doc.viewport_mut();
            vp.window_size = (1200, 800);
            vp.hidpi_scale = 1.0;
        }
        base_doc.add_user_agent_stylesheet(blitz_dom::UserAgentStylesheet::parse(
            "html, body, div, form, button, input { display: block; margin: 0; padding: 0; }",
        ));
        let doc_root = base_doc.root_node().id;

        {
            let mut m = base_doc.mutate();
            let html = m.create_element(
                qname("html"),
                vec![attr("style", "position: relative; width: 1200px; height: 800px;")],
            );
            m.append_children(doc_root, &[html]);

            let body = m.create_element(
                qname("body"),
                vec![attr("style", "position: relative; width: 1200px; height: 800px; margin: 0;")],
            );
            m.append_children(html, &[body]);

            let named_form = m.create_element(
                qname("form"),
                vec![
                    attr("id", "named-form"),
                    attr("style", "position: absolute; left: 0px; top: 0px; width: 600px; height: 200px;"),
                ],
            );
            let named_cb = m.create_element(
                qname("button"),
                vec![
                    attr("id", "named-cb"),
                    attr("type", "button"),
                    attr("style", "position: absolute; left: 10px; top: 10px; width: 40px; height: 40px;"),
                ],
            );
            let named_reset = m.create_element(
                qname("button"),
                vec![
                    attr("id", "named-reset"),
                    attr("type", "reset"),
                    attr("style", "position: absolute; left: 80px; top: 10px; width: 80px; height: 40px;"),
                ],
            );
            let disabled_reset = m.create_element(
                qname("button"),
                vec![
                    attr("id", "disabled-reset"),
                    attr("type", "reset"),
                    attr("disabled", ""),
                    attr("style", "position: absolute; left: 180px; top: 10px; width: 80px; height: 40px;"),
                ],
            );
            let regular_btn = m.create_element(
                qname("button"),
                vec![
                    attr("id", "regular-btn"),
                    attr("type", "button"),
                    attr("style", "position: absolute; left: 280px; top: 10px; width: 80px; height: 40px;"),
                ],
            );
            m.append_children(named_form, &[named_cb, named_reset, disabled_reset, regular_btn]);

            let ext_cb = m.create_element(
                qname("button"),
                vec![
                    attr("id", "ext-cb"),
                    attr("type", "button"),
                    attr("form", "named-form"),
                    attr("style", "position: absolute; left: 10px; top: 240px; width: 40px; height: 40px;"),
                ],
            );
            let ext_input_reset = m.create_element(
                qname("input"),
                vec![
                    attr("id", "ext-input-reset"),
                    attr("type", "reset"),
                    attr("form", "named-form"),
                    attr("style", "position: absolute; left: 80px; top: 240px; width: 80px; height: 40px;"),
                ],
            );

            // Anonymous `<form>` without an `id` attribute
            let anon_form = m.create_element(
                qname("form"),
                vec![attr("style", "position: absolute; left: 0px; top: 350px; width: 600px; height: 200px;")],
            );
            let anon_cb = m.create_element(
                qname("button"),
                vec![
                    attr("id", "anon-cb"),
                    attr("type", "button"),
                    attr("style", "position: absolute; left: 10px; top: 10px; width: 40px; height: 40px;"),
                ],
            );
            let anon_reset = m.create_element(
                qname("button"),
                vec![
                    attr("id", "anon-reset"),
                    attr("type", "reset"),
                    attr("style", "position: absolute; left: 80px; top: 10px; width: 80px; height: 40px;"),
                ],
            );
            m.append_children(anon_form, &[anon_cb, anon_reset]);
            m.append_children(body, &[named_form, ext_cb, ext_input_reset, anon_form]);
        }

        base_doc.resolve(0.0);

        let base = Rc::new(std::cell::RefCell::new(base_doc));
        let doc = Document::from_base(base);

        Document::with_current(doc.clone(), || {
            let named_count = Rc::new(Cell::new(0_u32));
            let ext_count = Rc::new(Cell::new(0_u32));
            let anon_count = Rc::new(Cell::new(0_u32));

            let nc = named_count.clone();
            let _g1 = watch_form_reset("named-cb", None, move |_| {
                nc.set(nc.get() + 1);
            })
            .expect("watch_form_reset named-cb");

            let ec = ext_count.clone();
            let _g2 = watch_form_reset("ext-cb", Some("named-form"), move |_| {
                ec.set(ec.get() + 1);
            })
            .expect("watch_form_reset ext-cb");

            let ac = anon_count.clone();
            let _g3 = watch_form_reset("anon-cb", None, move |_| {
                ac.set(ac.get() + 1);
            })
            .expect("watch_form_reset anon-cb");

            // 1. Clicking regular `<button type="button">` must NOT reset
            click_at(&doc, 300.0, 25.0);
            assert_eq!(named_count.get(), 0);
            assert_eq!(ext_count.get(), 0);
            assert_eq!(anon_count.get(), 0);

            // 2. Clicking disabled `<button type="reset" disabled>` must NOT reset
            click_at(&doc, 200.0, 25.0);
            assert_eq!(named_count.get(), 0);
            assert_eq!(ext_count.get(), 0);

            // 3. Clicking `<button type="reset">` inside `#named-form` resets enclosed & external controls
            click_at(&doc, 100.0, 25.0);
            assert_eq!(named_count.get(), 1);
            assert_eq!(ext_count.get(), 1);
            assert_eq!(anon_count.get(), 0);

            // 4. Clicking external `<input type="reset" form="named-form">` resets `#named-form` controls
            click_at(&doc, 100.0, 260.0);
            assert_eq!(named_count.get(), 2);
            assert_eq!(ext_count.get(), 2);
            assert_eq!(anon_count.get(), 0);

            // 5. Clicking `<button type="reset">` inside anonymous `<form>` resets `#anon-cb` only
            click_at(&doc, 100.0, 375.0);
            assert_eq!(named_count.get(), 2);
            assert_eq!(ext_count.get(), 2);
            assert_eq!(anon_count.get(), 1);
        });
    }

    #[test]
    fn test_native_collect_form_data_serialization_rules() {
        let mut base_doc = BaseDocument::new(DocumentConfig::default());
        let doc_root = base_doc.root_node().id;

        {
            let mut m = base_doc.mutate();
            let html = m.create_element(qname("html"), vec![]);
            let body = m.create_element(qname("body"), vec![]);

            let form = m.create_element(qname("form"), vec![attr("id", "checkout-form")]);
            let other_form = m.create_element(qname("form"), vec![attr("id", "other-form")]);

            // 1. Checked checkbox with default "on" value
            let cb_default = m.create_element(
                qname("input"),
                vec![attr("type", "checkbox"), attr("name", "terms"), attr("checked", "true")],
            );
            // 2. Checked checkbox with custom value
            let cb_custom = m.create_element(
                qname("input"),
                vec![
                    attr("type", "checkbox"),
                    attr("name", "addons"),
                    attr("value", "priority"),
                    attr("checked", "true"),
                ],
            );
            // 3. Unchecked checkbox (no `checked` presence attribute) -> skipped
            let cb_unchecked = m.create_element(
                qname("input"),
                vec![
                    attr("type", "checkbox"),
                    attr("name", "addons"),
                    attr("value", "backup"),
                ],
            );
            // 4. Disabled checked checkbox -> skipped
            let cb_disabled = m.create_element(
                qname("input"),
                vec![
                    attr("type", "checkbox"),
                    attr("name", "disabled_opt"),
                    attr("value", "yes"),
                    attr("checked", ""),
                    attr("disabled", ""),
                ],
            );
            // 5. Radio group: starter (unchecked) vs pro (checked)
            let r_starter = m.create_element(
                qname("input"),
                vec![
                    attr("type", "radio"),
                    attr("name", "plan"),
                    attr("value", "starter"),
                ],
            );
            let r_pro = m.create_element(
                qname("input"),
                vec![
                    attr("type", "radio"),
                    attr("name", "plan"),
                    attr("value", "pro"),
                    attr("checked", "true"),
                ],
            );
            // 6. Unselected radio group -> 0 entries
            let r_unsel = m.create_element(
                qname("input"),
                vec![attr("type", "radio"), attr("name", "empty_radio"), attr("value", "a")],
            );
            // 7. Hidden input
            let hidden = m.create_element(
                qname("input"),
                vec![attr("type", "hidden"), attr("name", "csrf"), attr("value", "tok-99")],
            );
            // 8. Control inside `#checkout-form` overridden with `form="other-form"` -> excluded from checkout-form
            let overridden = m.create_element(
                qname("input"),
                vec![
                    attr("type", "checkbox"),
                    attr("name", "leaked"),
                    attr("value", "no"),
                    attr("checked", "true"),
                    attr("form", "other-form"),
                ],
            );
            // 9. Button inside form -> excluded from `collect_form_data`
            let submit_btn = m.create_element(
                qname("button"),
                vec![attr("type", "submit"), attr("name", "action"), attr("value", "save")],
            );

            m.append_children(
                form,
                &[
                    cb_default,
                    cb_custom,
                    cb_unchecked,
                    cb_disabled,
                    r_starter,
                    r_pro,
                    r_unsel,
                    hidden,
                    overridden,
                    submit_btn,
                ],
            );

            // 10. External control associated via `form="checkout-form"`
            let ext_cb = m.create_element(
                qname("input"),
                vec![
                    attr("type", "checkbox"),
                    attr("name", "external_opt"),
                    attr("value", "attached"),
                    attr("checked", "true"),
                    attr("form", "checkout-form"),
                ],
            );

            let non_form_div = m.create_element(qname("div"), vec![attr("id", "not-a-form")]);

            m.append_children(body, &[form, other_form, ext_cb, non_form_div]);
            m.append_children(html, &[body]);
            m.append_children(doc_root, &[html]);
        }

        let doc = Document::from_base(Rc::new(std::cell::RefCell::new(base_doc)));

        let entries = doc
            .collect_form_data("checkout-form")
            .expect("collect_form_data should succeed");
        assert_eq!(
            entries,
            vec![
                ("terms".to_string(), "on".to_string()),
                ("addons".to_string(), "priority".to_string()),
                ("plan".to_string(), "pro".to_string()),
                ("csrf".to_string(), "tok-99".to_string()),
                ("external_opt".to_string(), "attached".to_string()),
            ]
        );

        let other_entries = doc
            .collect_form_data("other-form")
            .expect("other-form should succeed");
        assert_eq!(
            other_entries,
            vec![("leaked".to_string(), "no".to_string())]
        );

        assert!(matches!(
            doc.collect_form_data("missing-form"),
            Err(HostError::ElementNotFound(_))
        ));
        assert!(matches!(
            doc.collect_form_data("not-a-form"),
            Err(HostError::Unsupported(_))
        ));
    }
}
