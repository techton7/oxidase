//! Type definitions, guard handles, and error types for frame scheduling.

use std::time::Duration;
use thiserror::Error;

/// Timing metrics passed to frame loop callbacks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameInfo {
    /// Monotonic duration elapsed since session or page start.
    pub now: Duration,
    /// Duration elapsed since the preceding frame tick.
    pub delta: Duration,
}

/// Potential errors encountered when scheduling or starting frame loops.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FrameLoopError {
    /// The host environment does not support frame timing callbacks (e.g. headless without display).
    #[error("Host environment does not support frame timing")]
    UnsupportedHost,

    /// Frame runtime is temporarily unavailable or uninitialized on the current target.
    #[error("Frame runtime is unavailable")]
    RuntimeUnavailable,
}

/// Active cancellation guard for a running frame loop.
///
/// Dropping this guard stops subsequent frame callbacks and cleans up resources.
#[must_use = "Dropping FrameLoopGuard immediately terminates the frame loop"]
pub struct FrameLoopGuard {
    pub(crate) cancel_fn: Box<dyn FnOnce()>,
}

impl FrameLoopGuard {
    /// Creates a new `FrameLoopGuard` with an associated cancellation routine.
    #[allow(dead_code)]
    pub(crate) fn new(cancel_fn: impl FnOnce() + 'static) -> Self {
        Self {
            cancel_fn: Box::new(cancel_fn),
        }
    }

    /// Explicitly stops the frame loop and consumes the guard.
    pub fn cancel(self) {
        drop(self);
    }
}

impl std::fmt::Debug for FrameLoopGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrameLoopGuard").finish()
    }
}

impl Drop for FrameLoopGuard {
    fn drop(&mut self) {
        let cancel = std::mem::replace(&mut self.cancel_fn, Box::new(|| {}));
        cancel();
    }
}

/// Active cancellation guard for a pending single-frame request.
///
/// Dropping this guard cancels the pending frame callback.
#[must_use = "Dropping FrameRequestGuard cancels the pending frame request"]
pub struct FrameRequestGuard {
    pub(crate) cancel_fn: Box<dyn FnOnce()>,
}

impl FrameRequestGuard {
    /// Creates a new `FrameRequestGuard` with an associated cancellation routine.
    #[allow(dead_code)]
    pub(crate) fn new(cancel_fn: impl FnOnce() + 'static) -> Self {
        Self {
            cancel_fn: Box::new(cancel_fn),
        }
    }

    /// Explicitly cancels the pending frame request and consumes the guard.
    pub fn cancel(self) {
        drop(self);
    }
}

impl std::fmt::Debug for FrameRequestGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrameRequestGuard").finish()
    }
}

impl Drop for FrameRequestGuard {
    fn drop(&mut self) {
        let cancel = std::mem::replace(&mut self.cancel_fn, Box::new(|| {}));
        cancel();
    }
}
