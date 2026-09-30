//! Web (`wasm32`) DOM backend implementation backed by `web-sys`.

/// Web browser Document implementation.
#[derive(Clone, Debug, Default)]
pub struct Document {
    doc: Option<web_sys::Document>,
}

impl Document {
    /// Returns the current active browser document.
    pub fn current() -> Option<Self> {
        let win = web_sys::window()?;
        let doc = win.document()?;
        Some(Self { doc: Some(doc) })
    }

    /// Creates a Document from a raw `web_sys::Document`.
    #[inline]
    pub fn from_raw(doc: web_sys::Document) -> Self {
        Self { doc: Some(doc) }
    }

    /// Access the underlying `web_sys::Document`.
    #[inline]
    pub fn raw(&self) -> &web_sys::Document {
        self.doc
            .as_ref()
            .expect("Document has no active web_sys::Document")
    }

    /// Returns the element with the specified ID if present in the active browser document.
    pub fn element_by_id(&self, id: &str) -> Option<crate::dom::element::Element> {
        let doc = self.doc.as_ref()?;
        let el = doc.get_element_by_id(id)?;
        Some(crate::dom::element::Element::from_web(el))
    }

    /// Returns the activeElement if currently focused in the browser document.
    pub fn active_element(&self) -> Option<crate::dom::element::Element> {
        let doc = self.doc.as_ref()?;
        let el = doc.active_element()?;
        Some(crate::dom::element::Element::from_web(el))
    }

    /// Returns the Window handle associated with this document.
    pub fn window(&self) -> crate::window::Window {
        crate::window::window()
    }

    /// Sets the document viewport scroll offset.
    pub fn set_viewport_scroll(&self, x: f64, y: f64) {
        if let Some(win) = web_sys::window() {
            win.scroll_to_with_x_and_y(x, y);
        }
    }

    /// Queries the live viewport dimensions (width, height).
    pub fn inner_size(&self) -> (f64, f64) {
        if let Some(win) = web_sys::window() {
            let w = win.inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(0.0);
            let h = win.inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(0.0);
            (w, h)
        } else {
            (0.0, 0.0)
        }
    }

    /// Queries the live viewport scroll offset (x, y).
    pub fn viewport_scroll_offset(&self) -> (f64, f64) {
        if let Some(win) = web_sys::window() {
            let x = win.scroll_x().unwrap_or(0.0);
            let y = win.scroll_y().unwrap_or(0.0);
            (x, y)
        } else {
            (0.0, 0.0)
        }
    }
}
