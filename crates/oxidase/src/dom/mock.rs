//! Mock/Host DOM backend implementation for non-browser / test environments.

#[allow(unused_imports)]
use super::types::*;

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

    /// Queries mock viewport.
    pub fn viewport(&self) -> crate::runtime::geometry::Viewport {
        crate::runtime::geometry::Viewport::default()
    }

    /// Measures mock rect.
    pub fn measure_rect(&self, _element_id: &str) -> Option<crate::runtime::geometry::Rect> {
        None
    }

    /// Requests mock focus.
    pub fn set_focus(&self, element_id: &str) -> Result<(), crate::capability::HostError> {
        Err(crate::capability::HostError::ElementNotFound(element_id.to_string()))
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

/// Mock DOM Element implementation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Element {
    _private: (),
}

impl Element {
    /// Creates a mock Element for testing.
    pub fn mock() -> Self {
        Self { _private: () }
    }
}
