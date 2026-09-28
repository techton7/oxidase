use crate::capability::traits::{HostCommands, HostQueries};
use crate::capability::types::{FocusOptions, HostError};
use crate::dom::Document;
use crate::runtime::geometry::{Rect, Viewport};

/// Native platform capability provider backed by `oxidase::dom::Document` (wrapping Blitz BaseDocument).
#[derive(Clone, Copy, Debug, Default)]
pub struct NativeCapability;

impl NativeCapability {
    /// Creates a new `NativeCapability` provider.
    pub fn new() -> Self {
        Self
    }
}

impl HostCommands for NativeCapability {
    async fn focus_element(
        &self,
        target_id: &str,
        _options: FocusOptions,
    ) -> Result<(), HostError> {
        let doc = Document::current().ok_or_else(|| {
            HostError::Unsupported("Native document context not available".into())
        })?;
        doc.set_focus(target_id)
    }
}

impl HostQueries for NativeCapability {
    async fn measure_rect(&self, target_id: &str) -> Result<Option<Rect>, HostError> {
        let doc = Document::current().ok_or_else(|| {
            HostError::Unsupported(format!(
                "Native document context not available for measuring '{target_id}'"
            ))
        })?;
        Ok(doc.measure_rect(target_id))
    }

    async fn get_viewport(&self) -> Result<Viewport, HostError> {
        let doc = Document::current().ok_or_else(|| {
            HostError::Unsupported(
                "Native document context not available for viewport query".into(),
            )
        })?;
        Ok(doc.viewport())
    }

    async fn is_element_active(&self, target_id: &str) -> Result<bool, HostError> {
        let Some(doc) = Document::current() else {
            return Ok(false);
        };
        Ok(doc.is_element_active(target_id))
    }

    async fn is_reference_hidden(&self, anchor_ids: &[&str]) -> Result<bool, HostError> {
        let Some(doc) = Document::current() else {
            return Ok(false);
        };
        Ok(doc.is_reference_hidden(anchor_ids))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

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

    #[test]
    fn test_native_capability_without_context() {
        let native = NativeCapability::new();
        // In the absence of an ambient native document context, proper errors are returned
        let focus_res = block_on(native.focus_element("btn-1", FocusOptions::default()));
        assert_eq!(
            focus_res,
            Err(HostError::Unsupported("Native document context not available".into()))
        );

        let measure_res = block_on(native.measure_rect("btn-1"));
        assert!(matches!(measure_res, Err(HostError::Unsupported(_))));

        let vp_res = block_on(native.get_viewport());
        assert!(matches!(vp_res, Err(HostError::Unsupported(_))));

        let active_res = block_on(native.is_element_active("btn-1"));
        assert_eq!(active_res, Ok(false));

        let hidden_res = block_on(native.is_reference_hidden(&["anchor-1"]));
        assert_eq!(hidden_res, Ok(false));
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
    #[test]
    fn test_native_capability_live_blitz_document() {
        use blitz_dom::{Attribute, BaseDocument, DocumentConfig, QualName, local_name, ns};
        use std::cell::RefCell;
        use std::rc::Rc;

        let mut base_doc = BaseDocument::new(DocumentConfig::default());
        // Configure physical size 1920x1080 with 2.0 scale factor -> logical 960x540
        {
            let mut vp = base_doc.viewport_mut();
            vp.window_size = (1920, 1080);
            vp.hidpi_scale = 2.0;
        }
        base_doc.set_viewport_scroll(blitz_dom::Point { x: 10.0, y: 20.0 });

        let root_id = base_doc.root_node().id;
        let btn_id = {
            let mut mutator = base_doc.mutate();
            let node_id = mutator.create_element(
                QualName::new(None, ns!(), local_name!("button")),
                vec![Attribute {
                    name: QualName::new(None, ns!(), local_name!("id")),
                    value: "btn-submit".into(),
                }],
            );
            mutator.append_children(root_id, &[node_id]);
            node_id
        };

        // Populate layout dimensions on btn-submit
        let btn_node = base_doc.get_node_mut(btn_id).unwrap();
        let layout = &mut btn_node.layout_data_mut().final_layout;
        layout.location.x = 100.0;
        layout.location.y = 150.0;
        layout.size.width = 200.0;
        layout.size.height = 50.0;

        let native_doc = Document::from_base(Rc::new(RefCell::new(base_doc)));
        let native_cap = NativeCapability::new();

        Document::with_current(native_doc, || {
            // BI-P2B-001: get_viewport live calculation
            let vp = block_on(native_cap.get_viewport()).expect("get_viewport should succeed");
            assert_eq!(vp.width, 960.0);
            assert_eq!(vp.height, 540.0);
            assert_eq!(vp.scroll_x, 10.0);
            assert_eq!(vp.scroll_y, 20.0);

            // BI-P2B-002: measure_rect live layout accumulation with viewport scroll offset
            let rect_opt = block_on(native_cap.measure_rect("btn-submit"))
                .expect("measure_rect should succeed");
            let rect = rect_opt.expect("rect must be Some for existing node");
            // location (100 - scroll 10 = 90, 150 - scroll 20 = 130)
            assert_eq!(rect.x, 90.0);
            assert_eq!(rect.y, 130.0);
            assert_eq!(rect.width, 200.0);
            assert_eq!(rect.height, 50.0);

            // measure_rect on non-existent element returns Ok(None)
            let missing_rect = block_on(native_cap.measure_rect("missing-elem"))
                .expect("measure_rect on missing element should return Ok(None)");
            assert_eq!(missing_rect, None);

            // BI-P2B-003: focus_element & is_element_active live focus transfer
            assert_eq!(
                block_on(native_cap.is_element_active("btn-submit")).unwrap(),
                false
            );

            block_on(native_cap.focus_element("btn-submit", FocusOptions::default()))
                .expect("focus_element should succeed on existing node");

            assert_eq!(
                block_on(native_cap.is_element_active("btn-submit")).unwrap(),
                true
            );

            // focus_element on missing element returns ElementNotFound error
            let err = block_on(native_cap.focus_element("missing-elem", FocusOptions::default()))
                .unwrap_err();
            assert_eq!(err, HostError::ElementNotFound("missing-elem".into()));

            // BI-P2B-004: is_reference_hidden live visibility evaluation
            // btn-submit is at (90, 130) with size 200x50 inside (960x540) viewport -> visible (false)
            let hidden = block_on(native_cap.is_reference_hidden(&["btn-submit"])).unwrap();
            assert_eq!(hidden, false);

            // Reference to missing element is hidden (true)
            let hidden_missing = block_on(native_cap.is_reference_hidden(&["missing-elem"])).unwrap();
            assert_eq!(hidden_missing, true);
        });
    }
}
