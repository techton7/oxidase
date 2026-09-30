//! InteropRuntime - Core generic cross-platform runtime engine for Dioxus.

pub mod geometry;
pub use geometry::*;

use crate::bind_js;
use crate::JsError;

// Bind the TypeScript runtime using bind_js! macro (single-source canonical binding)
bind_js!("src/runtime/interop_runtime.ts"::{
    measure_rect as measure_rect_raw,
    get_viewport as get_viewport_raw,
    focus_element as focus_element_raw,
    is_element_active as is_element_active_raw,
    is_reference_hidden as is_reference_hidden_raw,
});

/// Measures the bounding client rectangle of an element by ID.
pub async fn measure_rect(element_id: &str) -> Result<Option<Rect>, JsError> {
    measure_rect_raw(element_id).await
}

/// Queries current viewport dimensions and scroll offsets.
pub async fn get_viewport() -> Result<Viewport, JsError> {
    get_viewport_raw().await
}

/// Requests focus for an element by ID with optional scroll prevention.
pub async fn focus_element(element_id: &str, prevent_scroll: bool) -> Result<bool, JsError> {
    focus_element_raw(element_id, prevent_scroll).await
}

/// Checks whether the element matching target_id is currently active.
pub async fn is_element_active(target_id: &str) -> Result<bool, JsError> {
    is_element_active_raw(target_id).await
}

/// Checks whether any anchor element is occluded or outside viewport bounds.
pub async fn is_reference_hidden(anchor_ids: &[&str]) -> Result<bool, JsError> {
    is_reference_hidden_raw(anchor_ids).await
}
