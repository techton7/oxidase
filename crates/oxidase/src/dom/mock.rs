//! Mock/Host DOM backend implementation for non-browser / test environments.

/// Mock DOM Document implementation for host environments and unit testing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Document {
    _private: (),
}

impl Document {
    /// Returns the current active document. In headless host environments, returns None unless mocked.
    pub fn current() -> Option<Self> {
        None
    }

    /// Creates a mock Document for testing.
    pub fn mock() -> Self {
        Self { _private: () }
    }

    /// Returns the element with the specified ID if present in mock document.
    pub fn element_by_id(&self, _id: &str) -> Option<crate::dom::element::Element> {
        None
    }

    /// Returns the element that currently holds active keyboard focus in the mock document.
    pub fn active_element(&self) -> Option<crate::dom::element::Element> {
        None
    }

    /// Returns the Window handle associated with this document.
    pub fn window(&self) -> crate::window::Window {
        crate::window::window()
    }

    /// Sets the document viewport scroll offset.
    pub fn set_viewport_scroll(&self, _x: f64, _y: f64) {}

    /// Queries live viewport dimensions (width, height).
    pub fn inner_size(&self) -> (f64, f64) {
        (0.0, 0.0)
    }

    /// Queries live viewport scroll offset (x, y).
    pub fn viewport_scroll_offset(&self) -> (f64, f64) {
        (0.0, 0.0)
    }

    /// Queries mock viewport.
    pub fn viewport(&self) -> crate::runtime::geometry::Viewport {
        crate::runtime::geometry::Viewport::default()
    }

    /// Measures mock rect.
    pub fn measure_rect(&self, _element_id: &str) -> Option<crate::runtime::geometry::Rect> {
        None
    }

    /// Requests mock focus.
    pub fn set_focus(&self, element_id: &str) -> Result<(), crate::error::HostError> {
        Err(crate::error::HostError::ElementNotFound(element_id.to_string()))
    }

    /// Queries mock active element.
    pub fn is_element_active(&self, _element_id: &str) -> bool {
        false
    }

    /// Queries mock reference hidden.
    pub fn is_reference_hidden(&self, _anchor_ids: &[&str]) -> bool {
        false
    }
}
