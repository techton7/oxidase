use oxidase::dom::observer::observe_window_blur as observer_observe_window_blur;
#[cfg(not(feature = "native"))]
use oxidase::dom::{Element, Rect};
use oxidase::dom::{Document, observe_window_blur};
use oxidase::time::local_today_ymd;

#[test]
fn test_local_today_ymd_returns_valid_calendar_date() {
    let (year, month, day) = local_today_ymd();
    assert!(year >= 1970, "unexpected year: {year}");
    assert!((1..=12).contains(&month), "invalid month: {month}");
    assert!((1..=31).contains(&day), "invalid day: {day}");
}

#[cfg(not(feature = "native"))]
#[test]
fn test_mock_batch2_substrate_apis() {
    let _doc = Document::mock();

    let el = Element::mock();
    let rect: Option<Rect> = el.offset_rect();
    assert_eq!(rect, None);

    let guard = observe_window_blur(|| {}).expect("mock observe_window_blur must succeed");
    drop(guard);
    let guard2 = observer_observe_window_blur(|| {}).expect("re-export must succeed");
    drop(guard2);
}

#[cfg(feature = "native")]
mod native_tests {
    use super::*;
    use blitz_dom::{Attribute, BaseDocument, DocumentConfig, LocalName, QualName, ns};
    use oxidase::dom::Rect;
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

    #[test]
    fn test_native_element_offset_rect() {
        let base_doc = Rc::new(RefCell::new(BaseDocument::new(DocumentConfig::default())));
        let doc = Document::from_base(base_doc.clone());

        {
            let mut base = base_doc.borrow_mut();
            let root_id = base.root_node().id;
            let mut mutator = base.mutate();
            let html = mutator.create_element(qname("html"), vec![]);
            let body = mutator.create_element(
                qname("body"),
                vec![attr("style", "margin: 0; padding: 0;")],
            );
            let outer = mutator.create_element(
                qname("div"),
                vec![attr(
                    "style",
                    "position: relative; margin-left: 100px; margin-top: 50px; padding-left: 10px; padding-top: 8px; border: 2px solid black;",
                )],
            );
            let target = mutator.create_element(
                qname("div"),
                vec![
                    attr("id", "offset-target"),
                    attr(
                        "style",
                        "margin-left: 24px; margin-top: 12px; width: 120px; height: 40px; display: block;",
                    ),
                ],
            );
            mutator.append_children(outer, &[target]);
            mutator.append_children(body, &[outer]);
            mutator.append_children(html, &[body]);
            mutator.append_children(root_id, &[html]);
        }
        doc.resolve(0.0);

        let target = doc
            .element_by_id("offset-target")
            .expect("offset-target must exist");
        let offset_rect: Rect = target
            .offset_rect()
            .expect("native Element::offset_rect must return Some(Rect)");

        // Offset relative to positioned `outer` padding edge: padding (10, 8) + margin (24, 12) = (34, 20)
        assert!((offset_rect.x - 34.0).abs() < 1.0, "got x={}", offset_rect.x);
        assert!((offset_rect.y - 20.0).abs() < 1.0, "got y={}", offset_rect.y);
        assert!(
            (offset_rect.width - 120.0).abs() < 1.0,
            "got width={}",
            offset_rect.width
        );
        assert!(
            (offset_rect.height - 40.0).abs() < 1.0,
            "got height={}",
            offset_rect.height
        );
    }

    #[test]
    fn test_native_observe_window_blur_and_guard_cleanup() {
        let base_doc = Rc::new(RefCell::new(BaseDocument::new(DocumentConfig::default())));
        let doc = Document::from_base(base_doc);

        let count = Rc::new(RefCell::new(0usize));
        let count_clone = count.clone();
        let guard = observer_observe_window_blur(move || {
            *count_clone.borrow_mut() += 1;
        })
        .expect("observe_window_blur via dom::observer must succeed");
        let guard_reexport =
            observe_window_blur(|| {}).expect("re-exported observe_window_blur must succeed");

        doc.dispatch_window_blur();
        assert_eq!(*count.borrow(), 1);

        doc.dispatch_window_blur();
        assert_eq!(*count.borrow(), 2);

        drop(guard);
        drop(guard_reexport);
        doc.dispatch_window_blur();
        assert_eq!(
            *count.borrow(),
            2,
            "Dropped observe_window_blur guard must remove observer"
        );
    }
}
