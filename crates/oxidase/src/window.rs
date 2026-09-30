use crate::error::HostError;
use crate::scroll::Scrollable;
use std::future::Future;

/// Public Window object providing viewport dimensions and scrolling capabilities.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Window;

/// Returns the ambient `Window` handle.
pub fn window() -> Window {
    Window
}

impl Window {
    /// Queries the viewport dimensions `(width, height)`.
    pub async fn inner_size(&self) -> Result<(f64, f64), HostError> {
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(w) = web_sys::window() {
                let width = w.inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(0.0);
                let height = w.inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(0.0);
                return Ok((width, height));
            }
        }
        #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
        {
            if let Some(doc) = crate::dom::Document::current() {
                return Ok(doc.inner_size());
            }
        }
        Ok((0.0, 0.0))
    }

    /// Queries the viewport width.
    pub async fn inner_width(&self) -> Result<f64, HostError> {
        Ok(self.inner_size().await?.0)
    }

    /// Queries the viewport height.
    pub async fn inner_height(&self) -> Result<f64, HostError> {
        Ok(self.inner_size().await?.1)
    }

    /// Queries the current horizontal scroll offset.
    pub async fn scroll_x(&self) -> Result<f64, HostError> {
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(w) = web_sys::window() {
                return Ok(w.scroll_x().unwrap_or(0.0));
            }
        }
        #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
        {
            if let Some(doc) = crate::dom::Document::current() {
                return Ok(doc.viewport_scroll_offset().0);
            }
        }
        Ok(0.0)
    }

    /// Queries the current vertical scroll offset.
    pub async fn scroll_y(&self) -> Result<f64, HostError> {
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(w) = web_sys::window() {
                return Ok(w.scroll_y().unwrap_or(0.0));
            }
        }
        #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
        {
            if let Some(doc) = crate::dom::Document::current() {
                return Ok(doc.viewport_scroll_offset().1);
            }
        }
        Ok(0.0)
    }

    /// Scrolls the viewport to the specified coordinates `(x, y)`.
    pub async fn scroll_to(&self, x: f64, y: f64) -> Result<(), HostError> {
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(w) = web_sys::window() {
                w.scroll_to_with_x_and_y(x, y);
                return Ok(());
            }
        }
        #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
        {
            if let Some(doc) = crate::dom::Document::current() {
                doc.set_viewport_scroll(x, y);
                return Ok(());
            }
        }
        let _ = (x, y);
        Ok(())
    }

    /// Scrolls relatively by `(dx, dy)` from the current scroll position.
    pub async fn scroll_by(&self, dx: f64, dy: f64) -> Result<(), HostError> {
        let cur_x = self.scroll_x().await?;
        let cur_y = self.scroll_y().await?;
        self.scroll_to(cur_x + dx, cur_y + dy).await
    }

    /// Returns the active document context.
    pub fn document(&self) -> crate::dom::Document {
        crate::dom::document()
    }
}

impl Scrollable for Window {
    fn scroll_x(&self) -> impl Future<Output = Result<f64, HostError>> {
        self.scroll_x()
    }

    fn scroll_y(&self) -> impl Future<Output = Result<f64, HostError>> {
        self.scroll_y()
    }

    fn scroll_to(&self, x: f64, y: f64) -> impl Future<Output = Result<(), HostError>> {
        self.scroll_to(x, y)
    }

    fn scroll_by(&self, dx: f64, dy: f64) -> impl Future<Output = Result<(), HostError>> {
        self.scroll_by(dx, dy)
    }
}
