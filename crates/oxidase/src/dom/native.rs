//! Native (Blitz) DOM backend implementation.

#[allow(unused_imports)]
use super::types::*;
use blitz_dom::BaseDocument;
use std::cell::RefCell;
use std::rc::Rc;

/// Native Blitz Document handle wrapping the real `BaseDocument`.
#[derive(Clone)]
pub struct Document {
    inner: Rc<RefCell<BaseDocument>>,
}

impl std::fmt::Debug for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Document")
            .field("document_id", &self.inner.borrow().id())
            .finish()
    }
}

impl Document {
    /// Returns the active native Blitz document if available in current thread or Dioxus context.
    pub fn current() -> Option<Self> {
        // 1. Check Dioxus root context if running within an active Dioxus runtime
        if dioxus::core::Runtime::try_current().is_some() {
            if let Some(doc) = dioxus::prelude::try_consume_context::<Document>() {
                return Some(doc);
            }
        }

        // 2. Check thread-local (active during harness dispatch or scoped execution in Blitz)
        if let Some(doc) = CURRENT_NATIVE_DOC.with(|cell| cell.borrow().clone()) {
            return Some(doc);
        }

        None
    }

    /// Construct a Document wrapping a real Blitz BaseDocument.
    pub fn from_base(inner: Rc<RefCell<BaseDocument>>) -> Self {
        Self { inner }
    }

    /// Access the underlying real `BaseDocument`.
    pub fn base(&self) -> &Rc<RefCell<BaseDocument>> {
        &self.inner
    }

    /// Sets the thread-local active native Document for the duration of a closure.
    pub fn with_current<R>(doc: Self, f: impl FnOnce() -> R) -> R {
        CURRENT_NATIVE_DOC.with(|cell| {
            let prev = cell.borrow_mut().replace(doc);
            let res = f();
            *cell.borrow_mut() = prev;
            res
        })
    }

    /// Sets or clears the current thread-local active native Document.
    pub fn set_current(doc: Option<Self>) {
        CURRENT_NATIVE_DOC.with(|cell| {
            *cell.borrow_mut() = doc;
        });
    }

    /// Provide this Document into the active Dioxus component context.
    pub fn provide_context(self) {
        dioxus::prelude::provide_context(self);
    }

    /// Queries the live viewport dimensions and scroll offsets from the underlying Blitz document.
    pub fn viewport(&self) -> crate::runtime::geometry::Viewport {
        let base = self.inner.borrow();
        let vp = base.viewport();
        let scroll = base.viewport_scroll();
        let scale = (vp.hidpi_scale * vp.zoom).max(0.001) as f64;
        let width = vp.window_size.0 as f64 / scale;
        let height = vp.window_size.1 as f64 / scale;
        crate::runtime::geometry::Viewport {
            width,
            height,
            scroll_x: scroll.x,
            scroll_y: scroll.y,
        }
    }

    /// Measures the bounding client rectangle of an element by ID by accumulating
    /// layout coordinates over the ancestor hierarchy and accounting for viewport scroll.
    pub fn measure_rect(&self, element_id: &str) -> Option<crate::runtime::geometry::Rect> {
        let base = self.inner.borrow();
        let node_id = base.get_element_by_id(element_id)?;
        let node = base.get_node(node_id)?;
        let layout = node.final_layout();
        let width = layout.size.width as f64;
        let height = layout.size.height as f64;

        let mut x = layout.location.x as f64;
        let mut y = layout.location.y as f64;

        let root_id = base.root_node().id;
        let mut current_parent = node.parent;
        while let Some(parent_id) = current_parent {
            if parent_id == root_id {
                break;
            }
            if let Some(parent_node) = base.get_node(parent_id) {
                let parent_layout = parent_node.final_layout();
                x += parent_layout.location.x as f64 - parent_node.scroll_offset().x;
                y += parent_layout.location.y as f64 - parent_node.scroll_offset().y;
                current_parent = parent_node.parent;
            } else {
                break;
            }
        }

        let vp_scroll = base.viewport_scroll();
        x -= vp_scroll.x;
        y -= vp_scroll.y;

        Some(crate::runtime::geometry::Rect::new(x, y, width, height))
    }

    /// Requests focus for the element with the specified ID in the Blitz DOM.
    pub fn set_focus(&self, element_id: &str) -> Result<(), crate::capability::HostError> {
        let mut base = self.inner.borrow_mut();
        let node_id = base
            .get_element_by_id(element_id)
            .ok_or_else(|| crate::capability::HostError::ElementNotFound(element_id.to_string()))?;
        base.set_focus_to(node_id);
        Ok(())
    }

    /// Checks if the element with the specified ID currently holds document focus in the Blitz DOM.
    pub fn is_element_active(&self, element_id: &str) -> bool {
        let base = self.inner.borrow();
        let Some(node_id) = base.get_element_by_id(element_id) else {
            return false;
        };
        base.active_focus_node_id() == Some(node_id)
    }

    /// Checks whether any anchor element is occluded or outside viewport bounds.
    pub fn is_reference_hidden(&self, anchor_ids: &[&str]) -> bool {
        let vp = self.viewport();
        for &id in anchor_ids {
            let Some(rect) = self.measure_rect(id) else {
                return true;
            };
            if rect.width <= 0.0 && rect.height <= 0.0 {
                return true;
            }
            if rect.bottom < 0.0 || rect.top > vp.height {
                return true;
            }
            if rect.right < 0.0 || rect.left > vp.width {
                return true;
            }
        }
        false
    }
}

std::thread_local! {
    static CURRENT_NATIVE_DOC: RefCell<Option<Document>> = const { RefCell::new(None) };
}

/// Native Blitz Element handle (placeholder for next slice).
#[derive(Clone, Debug, Default)]
pub struct Element;
