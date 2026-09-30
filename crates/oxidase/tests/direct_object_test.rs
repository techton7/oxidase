#![cfg(feature = "native")]

use blitz_dom::{Attribute, BaseDocument, DocumentConfig, QualName, local_name, ns};
use oxidase::prelude::*;
use oxidase::Element;
use std::cell::RefCell;
use std::rc::Rc;

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

fn create_test_blitz_document() -> (Document, blitz_dom::NodeId) {
    let mut base_doc = BaseDocument::new(DocumentConfig::default());
    {
        let mut vp = base_doc.viewport_mut();
        vp.window_size = (1920, 1080);
        vp.hidpi_scale = 2.0; // logical 960x540
    }
    base_doc.set_viewport_scroll(blitz_dom::Point { x: 10.0, y: 20.0 });

    let root_id = base_doc.root_node().id;
    let btn_id = {
        let mut mutator = base_doc.mutate();
        let node_id = mutator.create_element(
            QualName::new(None, ns!(), local_name!("button")),
            vec![Attribute {
                name: QualName::new(None, ns!(), local_name!("id")),
                value: "btn-action".into(),
            }],
        );
        mutator.append_children(root_id, &[node_id]);
        node_id
    };

    let btn_node = base_doc.get_node_mut(btn_id).unwrap();
    let layout = &mut btn_node.layout_data_mut().final_layout;
    layout.location.x = 100.0;
    layout.location.y = 200.0;
    layout.size.width = 150.0;
    layout.size.height = 40.0;

    let doc = Document::from_base(Rc::new(RefCell::new(base_doc)));
    (doc, btn_id)
}

#[test]
fn test_window_direct_object_size_and_scroll() {
    let (doc, _) = create_test_blitz_document();

    Document::with_current(doc, || {
        let win = window();

        // BI-P3-WIN-SIZE: 1920x1080 with 2.0 scale -> logical 960x540
        let (width, height) = block_on(win.inner_size()).expect("inner_size failed");
        assert_eq!(width, 960.0);
        assert_eq!(height, 540.0);
        assert_eq!(block_on(win.inner_width()).unwrap(), 960.0);
        assert_eq!(block_on(win.inner_height()).unwrap(), 540.0);

        // BI-P3-WIN-SCROLL: Initial viewport scroll (10.0, 20.0)
        assert_eq!(block_on(win.scroll_x()).unwrap(), 10.0);
        assert_eq!(block_on(win.scroll_y()).unwrap(), 20.0);

        // Scroll to (50.0, 100.0)
        block_on(win.scroll_to(50.0, 100.0)).expect("scroll_to failed");
        assert_eq!(block_on(win.scroll_x()).unwrap(), 50.0);
        assert_eq!(block_on(win.scroll_y()).unwrap(), 100.0);

        // Relative scroll by (15.0, -20.0) -> (65.0, 80.0)
        block_on(win.scroll_by(15.0, -20.0)).expect("scroll_by failed");
        assert_eq!(block_on(win.scroll_x()).unwrap(), 65.0);
        assert_eq!(block_on(win.scroll_y()).unwrap(), 80.0);
    });
}

#[test]
fn test_document_element_lookup_and_active_focus() {
    let (doc, _) = create_test_blitz_document();

    Document::with_current(doc, || {
        let doc = document();

        // BI-P3-DOC-ELEMENT: existing element returns Some(Element)
        let element = doc.element_by_id("btn-action").expect("element 'btn-action' must exist");
        assert_eq!(element.id(), Some("btn-action".to_string()));
        assert!(element.is_connected());

        // Missing element returns None
        assert!(doc.element_by_id("missing-id").is_none());

        // Initially no active focused element
        assert!(doc.active_element().is_none());
        assert!(!element.has_focus());

        // BI-P3-EL-FOCUS: grant focus
        block_on(element.focus()).expect("focus failed");
        assert!(element.has_focus());

        // BI-P3-DOC-ACTIVE: document().active_element() returns the focused element
        let active = doc.active_element().expect("active element must exist after focus");
        assert_eq!(active.id(), Some("btn-action".to_string()));

        // Blur removes focus
        block_on(element.blur()).expect("blur failed");
        assert!(!element.has_focus());
        assert_ne!(doc.active_element().and_then(|e| e.id()), Some("btn-action".to_string()));
    });
}

#[test]
fn test_element_direct_geometry_and_scrolling() {
    let (doc, _) = create_test_blitz_document();

    Document::with_current(doc, || {
        let doc = document();
        let element = doc.element_by_id("btn-action").unwrap();

        // BI-P3-EL-RECT: Location (100.0, 200.0) minus scroll (10.0, 20.0) = (90.0, 180.0)
        let rect = block_on(element.client_rect()).expect("client_rect failed").expect("rect must exist");
        assert_eq!(rect.x, 90.0);
        assert_eq!(rect.y, 180.0);
        assert_eq!(rect.width, 150.0);
        assert_eq!(rect.height, 40.0);

        // BI-P3-EL-SCROLL: Container scroll methods via Scrollable trait
        assert_eq!(block_on(element.scroll_x()).unwrap(), 0.0);
        assert_eq!(block_on(element.scroll_y()).unwrap(), 0.0);

        block_on(element.scroll_to(25.0, 35.0)).expect("element scroll_to failed");
        assert_eq!(block_on(element.scroll_x()).unwrap(), 25.0);
        assert_eq!(block_on(element.scroll_y()).unwrap(), 35.0);

        block_on(element.scroll_by(5.0, -10.0)).expect("element scroll_by failed");
        assert_eq!(block_on(element.scroll_x()).unwrap(), 30.0);
        assert_eq!(block_on(element.scroll_y()).unwrap(), 25.0);

        // scroll_into_view
        block_on(element.scroll_into_view()).expect("scroll_into_view failed");
    });
}

#[test]
fn test_prelude_scrollable_and_direct_objects_availability() {
    // BI-P3-SCROLLABLE-PRELUDE: use oxidase::prelude::*; brings all direct object APIs into scope
    let win: Window = window();
    let doc: Document = document();
    let _element: Option<Element> = doc.element_by_id("none");
    let _dom_element: Option<DomElement> = doc.element_by_id("none");
    let _win_from_doc = doc.window();
    let _win_from_win = win;
}
