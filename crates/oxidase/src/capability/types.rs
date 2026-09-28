use crate::JsError;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Structured host capability errors spanning JS, DOM, and native execution boundaries.
#[derive(Debug, Error, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HostError {
    #[error("Host JS error: {0}")]
    Js(#[from] JsError),
    #[error("Target element '{0}' not found on host")]
    ElementNotFound(String),
    #[error("Host capability unsupported: {0}")]
    Unsupported(String),
    #[error("Host operation timed out: {0}")]
    Timeout(String),
}

/// Options configuring element focus behavior.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FocusOptions {
    pub prevent_scroll: bool,
}

impl FocusOptions {
    /// Creates a new `FocusOptions` with the specified `prevent_scroll` flag.
    pub fn new(prevent_scroll: bool) -> Self {
        Self { prevent_scroll }
    }
}
