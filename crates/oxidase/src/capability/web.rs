use crate::capability::traits::{HostCommands, HostQueries};
use crate::capability::types::{FocusOptions, HostError};
use crate::runtime::geometry::{Rect, Viewport};

mod bridge {
    use crate::runtime::geometry::{Rect, Viewport};

    crate::bind_js!("src/runtime/interop_runtime.ts"::{
        measure_rect as measure_rect_raw,
        get_viewport as get_viewport_raw,
        focus_element as focus_element_raw,
        is_element_active as is_element_active_raw,
        is_reference_hidden as is_reference_hidden_raw,
    });
}

/// Web platform capability provider backed by `interop_runtime.ts` via `bind_js!`.
#[derive(Clone, Copy, Debug, Default)]
pub struct WebCapability;

impl WebCapability {
    /// Creates a new `WebCapability` provider.
    pub fn new() -> Self {
        Self
    }
}

impl HostCommands for WebCapability {
    async fn focus_element(
        &self,
        target_id: &str,
        options: FocusOptions,
    ) -> Result<(), HostError> {
        let success = bridge::focus_element_raw(target_id, options.prevent_scroll)
            .await
            .map_err(HostError::Js)?;
        if success {
            Ok(())
        } else {
            Err(HostError::ElementNotFound(target_id.to_string()))
        }
    }
}

impl HostQueries for WebCapability {
    async fn measure_rect(&self, target_id: &str) -> Result<Option<Rect>, HostError> {
        bridge::measure_rect_raw(target_id)
            .await
            .map_err(HostError::Js)
    }

    async fn get_viewport(&self) -> Result<Viewport, HostError> {
        bridge::get_viewport_raw()
            .await
            .map_err(HostError::Js)
    }

    async fn is_element_active(&self, target_id: &str) -> Result<bool, HostError> {
        bridge::is_element_active_raw(target_id)
            .await
            .map_err(HostError::Js)
    }

    async fn is_reference_hidden(&self, anchor_ids: &[&str]) -> Result<bool, HostError> {
        bridge::is_reference_hidden_raw(anchor_ids)
            .await
            .map_err(HostError::Js)
    }
}
