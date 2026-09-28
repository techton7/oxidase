//! Host-neutral capability abstraction subsystem for Dioxus.
//!
//! Provides standardized `HostCommands` and `HostQueries` interfaces for measuring geometry,
//! querying viewport dimensions, controlling element focus, and detecting active elements
//! across Web and Native platforms.

pub mod types;
pub use types::*;

pub mod traits;
pub use traits::*;

pub mod web;
pub use web::WebCapability;

pub mod native;
pub use native::NativeCapability;

use crate::runtime::geometry::{Rect, Viewport};

/// Default host capability dispatcher that routes calls to the appropriate platform provider.
#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultCapability;

impl HostCommands for DefaultCapability {
    async fn focus_element(
        &self,
        target_id: &str,
        options: FocusOptions,
    ) -> Result<(), HostError> {
        #[cfg(target_arch = "wasm32")]
        {
            WebCapability.focus_element(target_id, options).await
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            if crate::dom::Document::current().is_some() {
                NativeCapability.focus_element(target_id, options).await
            } else {
                WebCapability.focus_element(target_id, options).await
            }
        }
    }
}

impl HostQueries for DefaultCapability {
    async fn measure_rect(&self, target_id: &str) -> Result<Option<Rect>, HostError> {
        #[cfg(target_arch = "wasm32")]
        {
            WebCapability.measure_rect(target_id).await
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            if crate::dom::Document::current().is_some() {
                NativeCapability.measure_rect(target_id).await
            } else {
                WebCapability.measure_rect(target_id).await
            }
        }
    }

    async fn get_viewport(&self) -> Result<Viewport, HostError> {
        #[cfg(target_arch = "wasm32")]
        {
            WebCapability.get_viewport().await
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            if crate::dom::Document::current().is_some() {
                NativeCapability.get_viewport().await
            } else {
                WebCapability.get_viewport().await
            }
        }
    }

    async fn is_element_active(&self, target_id: &str) -> Result<bool, HostError> {
        #[cfg(target_arch = "wasm32")]
        {
            WebCapability.is_element_active(target_id).await
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            if crate::dom::Document::current().is_some() {
                NativeCapability.is_element_active(target_id).await
            } else {
                WebCapability.is_element_active(target_id).await
            }
        }
    }

    async fn is_reference_hidden(&self, anchor_ids: &[&str]) -> Result<bool, HostError> {
        #[cfg(target_arch = "wasm32")]
        {
            WebCapability.is_reference_hidden(anchor_ids).await
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            if crate::dom::Document::current().is_some() {
                NativeCapability.is_reference_hidden(anchor_ids).await
            } else {
                WebCapability.is_reference_hidden(anchor_ids).await
            }
        }
    }
}

/// Requests focus for the element identified by `target_id` using the default host dispatcher.
pub async fn focus_element(target_id: &str, options: FocusOptions) -> Result<(), HostError> {
    DefaultCapability.focus_element(target_id, options).await
}

/// Measures the bounding client rectangle of the element identified by `target_id` using the default host dispatcher.
pub async fn measure_rect(target_id: &str) -> Result<Option<Rect>, HostError> {
    DefaultCapability.measure_rect(target_id).await
}

/// Queries current viewport dimensions and scroll offsets using the default host dispatcher.
pub async fn get_viewport() -> Result<Viewport, HostError> {
    DefaultCapability.get_viewport().await
}

/// Checks whether the element identified by `target_id` is currently active using the default host dispatcher.
pub async fn is_element_active(target_id: &str) -> Result<bool, HostError> {
    DefaultCapability.is_element_active(target_id).await
}

/// Checks whether any anchor element is occluded or outside viewport bounds using the default host dispatcher.
pub async fn is_reference_hidden(anchor_ids: &[&str]) -> Result<bool, HostError> {
    DefaultCapability.is_reference_hidden(anchor_ids).await
}

/// Returns the default host commands provider.
pub fn default_commands() -> DefaultCapability {
    DefaultCapability
}

/// Returns the default host queries provider.
pub fn default_queries() -> DefaultCapability {
    DefaultCapability
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn test_host_error_serde_roundtrip() {
        let err = HostError::ElementNotFound("trigger-btn".into());
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("trigger-btn"));
        let parsed: HostError = serde_json::from_str(&json).unwrap();
        assert_eq!(err, parsed);

        let unsupp = HostError::Unsupported("Native gesture not available".into());
        let json_u = serde_json::to_string(&unsupp).unwrap();
        let parsed_u: HostError = serde_json::from_str(&json_u).unwrap();
        assert_eq!(unsupp, parsed_u);
    }

    #[test]
    fn test_focus_options_construction() {
        let default_opt = FocusOptions::default();
        assert!(!default_opt.prevent_scroll);

        let custom_opt = FocusOptions::new(true);
        assert!(custom_opt.prevent_scroll);

        let json = serde_json::to_string(&custom_opt).unwrap();
        let deserialized: FocusOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(custom_opt, deserialized);
    }

    fn block_on<F: std::future::Future>(mut fut: F) -> F::Output {
        use std::pin::Pin;
        use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

        unsafe fn clone(_: *const ()) -> RawWaker {
            RawWaker::new(std::ptr::null(), &VTABLE)
        }
        unsafe fn wake(_: *const ()) {}
        unsafe fn wake_by_ref(_: *const ()) {}
        unsafe fn vtable_drop(_: *const ()) {}
        static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, vtable_drop);
        let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };
        let mut cx = Context::from_waker(&waker);

        let mut pinned = unsafe { Pin::new_unchecked(&mut fut) };
        match pinned.as_mut().poll(&mut cx) {
            Poll::Ready(res) => res,
            Poll::Pending => panic!("Future did not resolve immediately in synchronous test"),
        }
    }

    #[test]
    fn test_native_capability_without_context() {
        // Without active native document context, verify safe fallback / error behavior
        let native = NativeCapability::new();

        let focus_res = block_on(native.focus_element("btn-1", FocusOptions::default()));
        assert_eq!(
            focus_res,
            Err(HostError::Unsupported("Native document context not available".into()))
        );

        let measure_res = block_on(native.measure_rect("btn-1"));
        assert!(matches!(measure_res, Err(HostError::Unsupported(_))));

        let vp_res = block_on(native.get_viewport());
        assert!(matches!(vp_res, Err(HostError::Unsupported(_))));

        let active_res = block_on(native.is_element_active("btn-1"));
        assert_eq!(active_res, Ok(false));

        let hidden_res = block_on(native.is_reference_hidden(&["anchor-1"]));
        assert_eq!(hidden_res, Ok(false));
    }

    #[test]
    fn test_mock_capability_dispatch() {
        struct MockProvider;

        impl HostCommands for MockProvider {
            async fn focus_element(
                &self,
                target_id: &str,
                _options: FocusOptions,
            ) -> Result<(), HostError> {
                if target_id == "valid-target" {
                    Ok(())
                } else {
                    Err(HostError::ElementNotFound(target_id.to_string()))
                }
            }
        }

        impl HostQueries for MockProvider {
            async fn measure_rect(&self, target_id: &str) -> Result<Option<Rect>, HostError> {
                if target_id == "valid-target" {
                    Ok(Some(Rect::new(10.0, 20.0, 100.0, 50.0)))
                } else {
                    Ok(None)
                }
            }

            async fn get_viewport(&self) -> Result<Viewport, HostError> {
                Ok(Viewport {
                    width: 1920.0,
                    height: 1080.0,
                    scroll_x: 0.0,
                    scroll_y: 0.0,
                })
            }

            async fn is_element_active(&self, target_id: &str) -> Result<bool, HostError> {
                Ok(target_id == "valid-target")
            }

            async fn is_reference_hidden(&self, anchor_ids: &[&str]) -> Result<bool, HostError> {
                Ok(anchor_ids.is_empty())
            }
        }

        let provider = MockProvider;

        // BI-P2-001: focus_element
        assert_eq!(
            block_on(provider.focus_element("valid-target", FocusOptions::default())),
            Ok(())
        );
        assert_eq!(
            block_on(provider.focus_element("missing-target", FocusOptions::default())),
            Err(HostError::ElementNotFound("missing-target".to_string()))
        );

        // BI-P2-002: measure_rect & get_viewport
        let rect = block_on(provider.measure_rect("valid-target")).unwrap();
        assert_eq!(rect, Some(Rect::new(10.0, 20.0, 100.0, 50.0)));
        let vp = block_on(provider.get_viewport()).unwrap();
        assert_eq!(vp.width, 1920.0);

        // BI-P2-003: is_element_active & is_reference_hidden
        assert_eq!(block_on(provider.is_element_active("valid-target")), Ok(true));
        assert_eq!(block_on(provider.is_element_active("other")), Ok(false));
        assert_eq!(block_on(provider.is_reference_hidden(&[])), Ok(true));
        assert_eq!(block_on(provider.is_reference_hidden(&["anchor-1"])), Ok(false));
    }

    #[test]
    fn test_default_capability_provider_construction() {
        let _cmds = default_commands();
        let _queries = default_queries();
    }
}
