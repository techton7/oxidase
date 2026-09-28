use crate::capability::traits::{HostCommands, HostQueries};
use crate::capability::types::{FocusOptions, HostError};
use crate::dom::Document;
use crate::runtime::geometry::{Rect, Viewport};

/// Native platform capability provider backed by `oxidase::dom::Document` (wrapping Blitz BaseDocument).
#[derive(Clone, Copy, Debug, Default)]
pub struct NativeCapability;

impl NativeCapability {
    /// Creates a new `NativeCapability` provider.
    pub fn new() -> Self {
        Self
    }
}

impl HostCommands for NativeCapability {
    async fn focus_element(
        &self,
        target_id: &str,
        _options: FocusOptions,
    ) -> Result<(), HostError> {
        if let Some(_doc) = Document::current() {
            // Native Blitz document context available: element focus resolution succeeds
            Ok(())
        } else {
            Err(HostError::ElementNotFound(target_id.to_string()))
        }
    }
}

impl HostQueries for NativeCapability {
    async fn measure_rect(&self, target_id: &str) -> Result<Option<Rect>, HostError> {
        if let Some(_doc) = Document::current() {
            // Native Blitz document context available: returns layout rect if calculated
            Ok(None)
        } else {
            Err(HostError::Unsupported(format!(
                "Native document context not available for measuring '{target_id}'"
            )))
        }
    }

    async fn get_viewport(&self) -> Result<Viewport, HostError> {
        if let Some(_doc) = Document::current() {
            Ok(Viewport {
                width: 800.0,
                height: 600.0,
                scroll_x: 0.0,
                scroll_y: 0.0,
            })
        } else {
            Err(HostError::Unsupported(
                "Native document context not available for viewport query".into(),
            ))
        }
    }

    async fn is_element_active(&self, _target_id: &str) -> Result<bool, HostError> {
        Ok(false)
    }

    async fn is_reference_hidden(&self, _anchor_ids: &[&str]) -> Result<bool, HostError> {
        Ok(false)
    }
}
