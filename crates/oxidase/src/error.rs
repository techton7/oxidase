use crate::JsError;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Structured host runtime and DOM errors.
#[derive(Debug, Error, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Error {
    #[error("Host JS error: {0}")]
    Js(#[from] JsError),
    #[error("Target element '{0}' not found on host")]
    ElementNotFound(String),
    #[error("Host operation unsupported on current platform: {0}")]
    Unsupported(String),
    #[error("Host operation timed out: {0}")]
    Timeout(String),
}

/// Standard Result alias for Oxidase operations, defaulting to `Error`.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Compatibility alias preserving previous `HostError` naming.
pub type HostError = Error;
