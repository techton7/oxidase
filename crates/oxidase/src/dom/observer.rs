//! Host-neutral programmatic observer surfaces across Web and Blitz Native targets.
//!
//! Provides Web-compatible observer surfaces for:
//! 1. Element resize observation (ResizeObserver equivalent)
//! 2. Capture-phase interaction listening (PointerDown, FocusIn, KeyDown with node path resolution)
//! 3. Transition and animation lifecycle with guaranteed fallback timeouts
//! 4. Form reset observation on enclosing forms

use crate::error::Result;
use crate::runtime::geometry::Rect;
use crate::watcher_guard::WatcherGuard;

/// Element resize entry containing the target element ID and its updated bounding/content rect.
#[derive(Clone, Debug, PartialEq)]
pub struct ResizeEntry {
    pub target_id: String,
    pub content_rect: Rect,
}

/// The kind of global capture interaction intercepted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CaptureEventKind {
    PointerDown,
    FocusIn,
    KeyDown { key: String },
}

/// Intercepted capture-phase event carrying target information and resolved ancestor path IDs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureEvent {
    pub kind: CaptureEventKind,
    pub target_id: Option<String>,
    pub path_ids: Vec<String>,
}

impl CaptureEvent {
    /// Checks whether the interaction occurred outside all provided boundary element IDs.
    pub fn is_outside_boundary(&self, boundary_ids: &[&str]) -> bool {
        if boundary_ids.is_empty() {
            return true;
        }
        !self.path_ids.iter().any(|id| boundary_ids.contains(&id.as_str()))
    }
}

/// Lifecycle events for CSS animations and transitions during element presence phases.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransitionLifecycleEvent {
    AnimationEnd { animation_name: String },
    AnimationCancel { animation_name: String },
    TimeoutFallback,
    Stopped,
}

/// Observes layout and dimensional changes of an element across Web and Native hosts.
pub fn observe_element_resize(
    element_id: &str,
    callback: impl FnMut(ResizeEntry) + 'static,
) -> Result<WatcherGuard> {
    crate::dom::backend::observe_element_resize(element_id, Box::new(callback))
}

/// Observes global capture-phase interactions across Web and Native hosts.
pub fn observe_capture_events(
    callback: impl FnMut(CaptureEvent) + 'static,
) -> Result<WatcherGuard> {
    crate::dom::backend::observe_capture_events(Box::new(callback))
}

/// Observes transition and animation lifecycle for an element with a guaranteed fallback timeout.
pub fn observe_transition_lifecycle(
    element_id: &str,
    fallback_timeout_ms: u64,
    callback: impl FnMut(TransitionLifecycleEvent) + 'static,
) -> Result<WatcherGuard> {
    crate::dom::backend::observe_transition_lifecycle(element_id, fallback_timeout_ms, Box::new(callback))
}

/// Observes reset events dispatched on an element's enclosing form.
pub fn observe_form_reset(
    element_id: &str,
    form_id: Option<&str>,
    callback: impl FnMut() + 'static,
) -> Result<WatcherGuard> {
    crate::dom::backend::observe_form_reset(element_id, form_id, Box::new(callback))
}

/// Observes scroll events across viewport and scrollable ancestor containers.
pub fn observe_scroll(
    callback: impl FnMut() + 'static,
) -> Result<WatcherGuard> {
    crate::dom::backend::observe_scroll(Box::new(callback))
}

/// Observes scroll events on a specific element container across Web and Native hosts.
pub fn observe_element_scroll(
    element_id: &str,
    callback: impl FnMut() + 'static,
) -> Result<WatcherGuard> {
    crate::dom::backend::observe_element_scroll(element_id, Box::new(callback))
}


