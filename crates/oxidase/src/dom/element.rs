use crate::error::HostError;
use crate::runtime::geometry::Rect;
use crate::scroll::Scrollable;
use std::future::Future;
use std::rc::Rc;

/// Options configuring element focus behavior.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FocusOptions {
    pub prevent_scroll: bool,
}

impl FocusOptions {
    /// Creates a new `FocusOptions` with the specified `prevent_scroll` flag.
    pub fn new(prevent_scroll: bool) -> Self {
        Self { prevent_scroll }
    }
}

/// Handle representing a mounted element in Dioxus.
pub type MountedHandle = Rc<dioxus::prelude::MountedData>;

/// Direct live DOM element providing geometry measurements, focus control, and scroll actions.
#[derive(Clone)]
pub struct Element {
    target: ElementTarget,
}

#[derive(Clone)]
pub(crate) enum ElementTarget {
    #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
    Native {
        doc: Rc<std::cell::RefCell<blitz_dom::BaseDocument>>,
        node_id: blitz_dom::NodeId,
    },
    #[cfg(target_arch = "wasm32")]
    Web(web_sys::Element),
    Mounted(MountedHandle),
    #[cfg(not(target_arch = "wasm32"))]
    Mock,
}

impl Element {
    /// Constructs an Element from a Dioxus `MountedHandle`.
    pub fn from_mounted(handle: MountedHandle) -> Self {
        Self {
            target: ElementTarget::Mounted(handle),
        }
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
    /// Constructs a native Blitz Element.
    pub fn from_native(
        doc: Rc<std::cell::RefCell<blitz_dom::BaseDocument>>,
        node_id: blitz_dom::NodeId,
    ) -> Self {
        Self {
            target: ElementTarget::Native { doc, node_id },
        }
    }

    #[cfg(target_arch = "wasm32")]
    /// Constructs a web browser Element from `web_sys::Element`.
    pub fn from_web(el: web_sys::Element) -> Self {
        Self {
            target: ElementTarget::Web(el),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// Constructs a mock Element for host/test environments.
    pub fn mock() -> Self {
        Self {
            target: ElementTarget::Mock,
        }
    }

    /// Measures the bounding client rectangle of the element relative to the viewport.
    pub async fn client_rect(&self) -> Result<Option<Rect>, HostError> {
        match &self.target {
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            ElementTarget::Native { doc, node_id } => {
                let base = doc.borrow();
                let Some(node) = base.get_node(*node_id) else {
                    return Ok(None);
                };
                let layout = node.final_layout();
                let width = layout.size.width as f64;
                let height = layout.size.height as f64;
                let mut x = layout.location.x as f64;
                let mut y = layout.location.y as f64;

                let root_id = base.root_node().id;
                let mut current_parent = node.parent;
                while let Some(parent_id) = current_parent {
                    if parent_id == root_id {
                        break;
                    }
                    if let Some(parent_node) = base.get_node(parent_id) {
                        let parent_layout = parent_node.final_layout();
                        x += parent_layout.location.x as f64 - parent_node.scroll_offset().x;
                        y += parent_layout.location.y as f64 - parent_node.scroll_offset().y;
                        current_parent = parent_node.parent;
                    } else {
                        break;
                    }
                }

                let vp_scroll = base.viewport_scroll();
                x -= vp_scroll.x;
                y -= vp_scroll.y;

                Ok(Some(Rect::new(x, y, width, height)))
            }
            #[cfg(target_arch = "wasm32")]
            ElementTarget::Web(el) => {
                let dom_rect = el.get_bounding_client_rect();
                Ok(Some(Rect::new(
                    dom_rect.x(),
                    dom_rect.y(),
                    dom_rect.width(),
                    dom_rect.height(),
                )))
            }
            ElementTarget::Mounted(handle) => match handle.get_client_rect().await {
                Ok(r) => Ok(Some(Rect::new(
                    r.origin.x,
                    r.origin.y,
                    r.size.width,
                    r.size.height,
                ))),
                Err(_) => Ok(None),
            },
            #[cfg(not(target_arch = "wasm32"))]
            ElementTarget::Mock => Ok(None),
        }
    }

    /// Grants keyboard focus to the element with default focus options.
    pub async fn focus(&self) -> Result<(), HostError> {
        self.focus_with_options(FocusOptions::default()).await
    }

    /// Grants keyboard focus to the element with specific focus options.
    pub async fn focus_with_options(&self, _options: FocusOptions) -> Result<(), HostError> {
        match &self.target {
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            ElementTarget::Native { doc, node_id } => {
                let mut base = doc.borrow_mut();
                base.set_focus_to(*node_id);
                Ok(())
            }
            #[cfg(target_arch = "wasm32")]
            ElementTarget::Web(el) => {
                use wasm_bindgen::JsCast;
                if let Ok(html_el) = el.clone().dyn_into::<web_sys::HtmlElement>() {
                    let _ = html_el.focus();
                }
                Ok(())
            }
            ElementTarget::Mounted(handle) => {
                let _ = handle.set_focus(true).await;
                Ok(())
            }
            #[cfg(not(target_arch = "wasm32"))]
            ElementTarget::Mock => Ok(()),
        }
    }

    /// Removes keyboard focus from the element.
    pub async fn blur(&self) -> Result<(), HostError> {
        match &self.target {
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            ElementTarget::Native { doc, node_id } => {
                let mut base = doc.borrow_mut();
                if base.active_focus_node_id() == Some(*node_id) {
                    let root = base.root_node().id;
                    base.set_focus_to(root);
                }
                Ok(())
            }
            #[cfg(target_arch = "wasm32")]
            ElementTarget::Web(el) => {
                use wasm_bindgen::JsCast;
                if let Ok(html_el) = el.clone().dyn_into::<web_sys::HtmlElement>() {
                    let _ = html_el.blur();
                }
                Ok(())
            }
            ElementTarget::Mounted(handle) => {
                let _ = handle.set_focus(false).await;
                Ok(())
            }
            #[cfg(not(target_arch = "wasm32"))]
            ElementTarget::Mock => Ok(()),
        }
    }

    /// Checks if this element currently holds document active focus.
    pub fn has_focus(&self) -> bool {
        match &self.target {
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            ElementTarget::Native { doc, node_id } => {
                doc.borrow().active_focus_node_id() == Some(*node_id)
            }
            #[cfg(target_arch = "wasm32")]
            ElementTarget::Web(el) => web_sys::window()
                .and_then(|w| w.document())
                .and_then(|d| d.active_element())
                .as_ref()
                == Some(el),
            ElementTarget::Mounted(_) => false,
            #[cfg(not(target_arch = "wasm32"))]
            ElementTarget::Mock => false,
        }
    }

    /// Scrolls the element into the visible viewport.
    pub async fn scroll_into_view(&self) -> Result<(), HostError> {
        self.scroll_into_view_with_options(ScrollIntoViewOptions::default())
            .await
    }

    /// Scrolls the element into the visible viewport with specific options.
    pub async fn scroll_into_view_with_options(
        &self,
        _options: ScrollIntoViewOptions,
    ) -> Result<(), HostError> {
        match &self.target {
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            ElementTarget::Native { doc, .. } => {
                if let Some(rect) = self.client_rect().await? {
                    let mut base = doc.borrow_mut();
                    let vp = base.viewport();
                    let cur_scroll = base.viewport_scroll();
                    let scale = (vp.hidpi_scale * vp.zoom).max(0.001) as f64;
                    let win_w = vp.window_size.0 as f64 / scale;
                    let win_h = vp.window_size.1 as f64 / scale;

                    let mut target_y = cur_scroll.y;
                    if rect.y < 0.0 {
                        target_y += rect.y;
                    } else if rect.bottom > win_h {
                        target_y += rect.bottom - win_h;
                    }

                    let mut target_x = cur_scroll.x;
                    if rect.x < 0.0 {
                        target_x += rect.x;
                    } else if rect.right > win_w {
                        target_x += rect.right - win_w;
                    }

                    base.set_viewport_scroll(blitz_dom::Point {
                        x: target_x,
                        y: target_y,
                    });
                }
                Ok(())
            }
            #[cfg(target_arch = "wasm32")]
            ElementTarget::Web(el) => {
                el.scroll_into_view();
                Ok(())
            }
            ElementTarget::Mounted(handle) => {
                let _ = handle
                    .scroll_to(dioxus::prelude::ScrollBehavior::Instant)
                    .await;
                Ok(())
            }
            #[cfg(not(target_arch = "wasm32"))]
            ElementTarget::Mock => Ok(()),
        }
    }

    /// Queries the current horizontal scroll offset of the element.
    pub async fn scroll_x(&self) -> Result<f64, HostError> {
        match &self.target {
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            ElementTarget::Native { doc, node_id } => {
                let base = doc.borrow();
                let Some(node) = base.get_node(*node_id) else {
                    return Err(HostError::ElementNotFound(format!("NodeId: {:?}", node_id)));
                };
                Ok(node.scroll_offset().x)
            }
            #[cfg(target_arch = "wasm32")]
            ElementTarget::Web(el) => Ok(el.scroll_left() as f64),
            ElementTarget::Mounted(_) => Ok(0.0),
            #[cfg(not(target_arch = "wasm32"))]
            ElementTarget::Mock => Ok(0.0),
        }
    }

    /// Queries the current vertical scroll offset of the element.
    pub async fn scroll_y(&self) -> Result<f64, HostError> {
        match &self.target {
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            ElementTarget::Native { doc, node_id } => {
                let base = doc.borrow();
                let Some(node) = base.get_node(*node_id) else {
                    return Err(HostError::ElementNotFound(format!("NodeId: {:?}", node_id)));
                };
                Ok(node.scroll_offset().y)
            }
            #[cfg(target_arch = "wasm32")]
            ElementTarget::Web(el) => Ok(el.scroll_top() as f64),
            ElementTarget::Mounted(_) => Ok(0.0),
            #[cfg(not(target_arch = "wasm32"))]
            ElementTarget::Mock => Ok(0.0),
        }
    }

    /// Scrolls the element to the specified `(x, y)` coordinates.
    pub async fn scroll_to(&self, x: f64, y: f64) -> Result<(), HostError> {
        match &self.target {
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            ElementTarget::Native { doc, node_id } => {
                let mut base = doc.borrow_mut();
                let Some(node) = base.get_node_mut(*node_id) else {
                    return Err(HostError::ElementNotFound(format!("NodeId: {:?}", node_id)));
                };
                let offset = node.scroll_offset_mut();
                offset.x = x;
                offset.y = y;
                Ok(())
            }
            #[cfg(target_arch = "wasm32")]
            ElementTarget::Web(el) => {
                el.scroll_to_with_x_and_y(x, y);
                Ok(())
            }
            ElementTarget::Mounted(handle) => {
                let _ = handle
                    .scroll(
                        dioxus::html::geometry::euclid::Vector2D::new(x, y),
                        dioxus::prelude::ScrollBehavior::Instant,
                    )
                    .await;
                Ok(())
            }
            #[cfg(not(target_arch = "wasm32"))]
            ElementTarget::Mock => Ok(()),
        }
    }

    /// Scrolls the element by the relative offset `(dx, dy)`.
    pub async fn scroll_by(&self, dx: f64, dy: f64) -> Result<(), HostError> {
        let cur_x = self.scroll_x().await?;
        let cur_y = self.scroll_y().await?;
        self.scroll_to(cur_x + dx, cur_y + dy).await
    }

    /// Returns the element's DOM ID attribute if set.
    pub fn id(&self) -> Option<String> {
        match &self.target {
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            ElementTarget::Native { doc, node_id } => {
                let base = doc.borrow();
                let node = base.get_node(*node_id)?;
                node.attr(blitz_dom::local_name!("id")).map(|s| s.to_string())
            }
            #[cfg(target_arch = "wasm32")]
            ElementTarget::Web(el) => {
                let id = el.id();
                if id.is_empty() { None } else { Some(id) }
            }
            ElementTarget::Mounted(_) => None,
            #[cfg(not(target_arch = "wasm32"))]
            ElementTarget::Mock => None,
        }
    }

    /// Checks if the element is currently connected to an active document.
    pub fn is_connected(&self) -> bool {
        match &self.target {
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            ElementTarget::Native { doc, node_id } => {
                let base = doc.borrow();
                let Some(node) = base.get_node(*node_id) else {
                    return false;
                };
                node.flags.is_in_document() || node.parent.is_some()
            }
            #[cfg(target_arch = "wasm32")]
            ElementTarget::Web(el) => el.is_connected(),
            ElementTarget::Mounted(_) => true,
            #[cfg(not(target_arch = "wasm32"))]
            ElementTarget::Mock => false,
        }
    }
}

impl Scrollable for Element {
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

impl From<MountedHandle> for Element {
    fn from(handle: MountedHandle) -> Self {
        Self::from_mounted(handle)
    }
}

impl std::fmt::Debug for Element {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut d = f.debug_struct("Element");
        if let Some(id) = self.id() {
            d.field("id", &id);
        }
        d.field("is_connected", &self.is_connected());
        d.finish()
    }
}

/// Options controlling `scroll_into_view` behavior.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ScrollIntoViewOptions {
    pub behavior: ScrollBehavior,
    pub block: ScrollLogicalPosition,
    pub inline: ScrollLogicalPosition,
}

/// Scroll behavior animation option.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ScrollBehavior {
    #[default]
    Auto,
    Smooth,
}

/// Logical scroll alignment position.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ScrollLogicalPosition {
    #[default]
    Start,
    Center,
    End,
    Nearest,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_element_debug_and_options() {
        let opts = ScrollIntoViewOptions::default();
        assert_eq!(opts.behavior, ScrollBehavior::Auto);
        assert_eq!(opts.block, ScrollLogicalPosition::Start);

        #[cfg(not(target_arch = "wasm32"))]
        {
            let el = Element::mock();
            assert_eq!(el.id(), None);
            assert!(!el.is_connected());
            let debug_str = format!("{:?}", el);
            assert!(debug_str.contains("Element"));
        }
    }
}

