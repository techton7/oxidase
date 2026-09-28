use crate::capability::types::{FocusOptions, HostError};
use crate::runtime::geometry::{Rect, Viewport};

/// Host mutations and command operations.
pub trait HostCommands {
    /// Requests focus for the element identified by `target_id`.
    fn focus_element(
        &self,
        target_id: &str,
        options: FocusOptions,
    ) -> impl std::future::Future<Output = Result<(), HostError>>;
}

/// Host measurement and state query operations.
pub trait HostQueries {
    /// Measures the bounding client rectangle of the element identified by `target_id`.
    fn measure_rect(
        &self,
        target_id: &str,
    ) -> impl std::future::Future<Output = Result<Option<Rect>, HostError>>;

    /// Queries current viewport dimensions and scroll offsets.
    fn get_viewport(
        &self,
    ) -> impl std::future::Future<Output = Result<Viewport, HostError>>;

    /// Checks whether the element identified by `target_id` is currently the active focused element.
    fn is_element_active(
        &self,
        target_id: &str,
    ) -> impl std::future::Future<Output = Result<bool, HostError>>;

    /// Checks whether any of the specified anchor elements are occluded or scrolled outside the viewport.
    fn is_reference_hidden(
        &self,
        anchor_ids: &[&str],
    ) -> impl std::future::Future<Output = Result<bool, HostError>>;
}
