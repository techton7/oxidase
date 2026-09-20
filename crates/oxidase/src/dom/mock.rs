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
