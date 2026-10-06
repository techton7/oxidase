use thiserror::Error;

/// Errors arising during positioning calculations or platform interactions.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum Error {
    #[error("Target element '{0}' not found on host")]
    ElementNotFound(String),
    #[error("Host operation unsupported on current platform: {0}")]
    Unsupported(String),
    #[error("Host operation timed out: {0}")]
    Timeout(String),
    #[error("Max reset iterations ({0}) exceeded during position computation")]
    MaxResetsExceeded(usize),
    #[error("{0}")]
    Other(String),
}

/// Result alias for positioning operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;
