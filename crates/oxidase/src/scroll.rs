use crate::error::HostError;
use std::future::Future;

/// Common scrolling operations shared by `Window` and `Element`.
pub trait Scrollable {
    /// Queries the current horizontal scroll offset.
    fn scroll_x(&self) -> impl Future<Output = Result<f64, HostError>>;

    /// Queries the current vertical scroll offset.
    fn scroll_y(&self) -> impl Future<Output = Result<f64, HostError>>;

    /// Scrolls to the specified absolute coordinates `(x, y)`.
    fn scroll_to(&self, x: f64, y: f64) -> impl Future<Output = Result<(), HostError>>;

    /// Scrolls relatively by `(dx, dy)` from the current scroll position.
    fn scroll_by(&self, dx: f64, dy: f64) -> impl Future<Output = Result<(), HostError>>;
}
