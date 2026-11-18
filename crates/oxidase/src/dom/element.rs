use crate::error::HostError;
use crate::runtime::geometry::Rect;
use crate::scroll::Scrollable;
use std::future::Future;
use std::rc::Rc;

/// Options configuring element focus behavior.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FocusOptions {
    pub prevent_scroll: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus_visible: Option<bool>,
}

impl FocusOptions {
    /// Creates a new `FocusOptions` with the specified `prevent_scroll` flag and inherited `:focus-visible` modality (`None`).
    pub const fn new(prevent_scroll: bool) -> Self {
        Self {
            prevent_scroll,
            focus_visible: None,
        }
    }

    /// Creates a new `FocusOptions` with explicit `prevent_scroll` and `focus_visible` overrides.
    pub const fn with_focus_visible(prevent_scroll: bool, focus_visible: Option<bool>) -> Self {
        Self {
            prevent_scroll,
            focus_visible,
        }
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
        #[cfg(target_arch = "wasm32")]
        if let Some(el) = handle.downcast::<web_sys::Element>() {
            return Self::from_web(el.clone());
        }
        #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
        if let Some(nh) = handle.downcast::<dioxus_native::NodeHandle>() {
            return Self::from_native(nh.base_document(), nh.node_id());
        }
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
                let Ok(base) = doc.try_borrow() else {
                    return Ok(None);
                };
                let Some(node) = base.get_node(*node_id) else {
                    return Ok(None);
                };
                let layout = node.final_layout();
                let width = layout.size.width as f64;
                let height = layout.size.height as f64;
                let mut x = layout.location.x as f64;
                let mut y = layout.location.y as f64;

                let is_target_fixed = node.taffy_position() == blitz_dom::taffy::Position::Fixed;
                if !is_target_fixed {
                    let root_id = base.root_node().id;
                    let mut current_parent = node.parent;
                    let mut hit_fixed_ancestor = false;
                    while let Some(parent_id) = current_parent {
                        if parent_id == root_id {
                            break;
                        }
                        if let Some(parent_node) = base.get_node(parent_id) {
                            let parent_layout = parent_node.final_layout();
                            if parent_node.taffy_position() == blitz_dom::taffy::Position::Fixed {
                                x += parent_layout.location.x as f64 - parent_node.scroll_offset().x;
                                y += parent_layout.location.y as f64 - parent_node.scroll_offset().y;
                                hit_fixed_ancestor = true;
                                break;
                            }
                            x += parent_layout.location.x as f64 - parent_node.scroll_offset().x;
                            y += parent_layout.location.y as f64 - parent_node.scroll_offset().y;
                            current_parent = parent_node.parent;
                        } else {
                            break;
                        }
                    }

                    if !hit_fixed_ancestor {
                        let vp_scroll = base.viewport_scroll();
                        x -= vp_scroll.x;
                        y -= vp_scroll.y;
                    }
                }

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

    /// Synchronously grants keyboard focus to the element with default focus options when backed by a live DOM handle.
    pub fn focus_sync(&self) -> Result<(), HostError> {
        self.focus_sync_with_options(FocusOptions::default())
    }

    /// Synchronously grants keyboard focus to the element with specific focus options when backed by a live DOM handle.
    pub fn focus_sync_with_options(&self, options: FocusOptions) -> Result<(), HostError> {
        match &self.target {
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            ElementTarget::Native { doc, node_id } => {
                let blitz_opts = blitz_dom::FocusOptions {
                    prevent_scroll: options.prevent_scroll,
                    focus_visible: options.focus_visible,
                };
                if let Ok(mut base) = doc.try_borrow_mut() {
                    base.set_focus_to_with_options(
                        *node_id,
                        blitz_dom::FocusSource::Script,
                        blitz_opts,
                    );
                } else {
                    let doc_clone = doc.clone();
                    let nid = *node_id;
                    let _ = crate::internal::safe_spawn(async move {
                        if let Ok(mut base) = doc_clone.try_borrow_mut() {
                            base.set_focus_to_with_options(
                                nid,
                                blitz_dom::FocusSource::Script,
                                blitz_opts,
                            );
                        }
                    });
                }
                Ok(())
            }
            #[cfg(target_arch = "wasm32")]
            ElementTarget::Web(el) => {
                use wasm_bindgen::JsCast;
                use wasm_bindgen::JsValue;
                let opts_obj = js_sys::Object::new();
                let _ = js_sys::Reflect::set(
                    &opts_obj,
                    &JsValue::from_str("preventScroll"),
                    &JsValue::from_bool(options.prevent_scroll),
                );
                if let Some(fv) = options.focus_visible {
                    let _ = js_sys::Reflect::set(
                        &opts_obj,
                        &JsValue::from_str("focusVisible"),
                        &JsValue::from_bool(fv),
                    );
                }
                if let Ok(focus_val) = js_sys::Reflect::get(el, &JsValue::from_str("focus"))
                    && let Ok(focus_fn) = focus_val.dyn_into::<js_sys::Function>()
                {
                    let _ = focus_fn.call1(el, &opts_obj);
                } else if let Ok(html_el) = el.clone().dyn_into::<web_sys::HtmlElement>() {
                    let _ = html_el.focus();
                }
                Ok(())
            }
            ElementTarget::Mounted(handle) => {
                #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
                if let Some(nh) = handle.downcast::<dioxus_native::NodeHandle>() {
                    nh.set_focus_with_options(blitz_dom::FocusOptions {
                        prevent_scroll: options.prevent_scroll,
                        focus_visible: options.focus_visible,
                    });
                    return Ok(());
                }
                let _ = options;
                let handle = handle.clone();
                let _ = crate::internal::safe_spawn(async move {
                    let _ = handle.set_focus(true).await;
                });
                Ok(())
            }
            #[cfg(not(target_arch = "wasm32"))]
            ElementTarget::Mock => {
                let _ = options;
                Ok(())
            }
        }
    }

    /// Grants keyboard focus to the element with default focus options.
    pub async fn focus(&self) -> Result<(), HostError> {
        self.focus_with_options(FocusOptions::default()).await
    }

    /// Grants keyboard focus to the element with specific focus options.
    pub async fn focus_with_options(&self, options: FocusOptions) -> Result<(), HostError> {
        match &self.target {
            ElementTarget::Mounted(handle) => {
                #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
                if let Some(nh) = handle.downcast::<dioxus_native::NodeHandle>() {
                    nh.set_focus_with_options(blitz_dom::FocusOptions {
                        prevent_scroll: options.prevent_scroll,
                        focus_visible: options.focus_visible,
                    });
                    return Ok(());
                }
                let _ = options;
                let _ = handle.set_focus(true).await;
                Ok(())
            }
            _ => self.focus_sync_with_options(options),
        }
    }

    /// Synchronously removes keyboard focus from this element when it holds active focus.
    ///
    /// On Native, calls `BaseDocument::clear_focus()` so `active_focus_node_id()` becomes `None`
    /// and `:focus`/`:focus-visible`/`:focus-within` states clear cleanly. If `BaseDocument` is
    /// currently immutably borrowed (e.g. inside `observe_capture_events`), defers `clear_focus()`
    /// via `safe_spawn` rather than panicking.
    pub fn blur_sync(&self) -> Result<(), HostError> {
        match &self.target {
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            ElementTarget::Native { doc, node_id } => {
                let target_nid = *node_id;
                if let Ok(mut base) = doc.try_borrow_mut() {
                    if base.active_focus_node_id() == Some(target_nid) {
                        base.clear_focus();
                    }
                } else {
                    let is_focused = doc
                        .try_borrow()
                        .map(|b| b.active_focus_node_id() == Some(target_nid))
                        .unwrap_or(true);
                    if is_focused {
                        let doc_clone = doc.clone();
                        let _ = crate::internal::safe_spawn(async move {
                            if let Ok(mut base) = doc_clone.try_borrow_mut()
                                && base.active_focus_node_id() == Some(target_nid)
                            {
                                base.clear_focus();
                            }
                        });
                    }
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
                #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
                if let Some(nh) = handle.downcast::<dioxus_native::NodeHandle>() {
                    let el = Self::from_native(nh.base_document(), nh.node_id());
                    return el.blur_sync();
                }
                let handle = handle.clone();
                let _ = crate::internal::safe_spawn(async move {
                    let _ = handle.set_focus(false).await;
                });
                Ok(())
            }
            #[cfg(not(target_arch = "wasm32"))]
            ElementTarget::Mock => Ok(()),
        }
    }

    /// Removes keyboard focus from the element.
    pub async fn blur(&self) -> Result<(), HostError> {
        match &self.target {
            ElementTarget::Mounted(handle) => {
                #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
                if let Some(nh) = handle.downcast::<dioxus_native::NodeHandle>() {
                    let el = Self::from_native(nh.base_document(), nh.node_id());
                    return el.blur_sync();
                }
                let _ = handle.set_focus(false).await;
                Ok(())
            }
            _ => self.blur_sync(),
        }
    }

    /// Checks whether `other` is an inclusive descendant of `self` (`self == other` or contained within `self`).
    pub fn contains(&self, other: &Element) -> bool {
        let _ = other;
        match (&self.target, &other.target) {
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            (
                ElementTarget::Native {
                    doc,
                    node_id: container_id,
                },
                ElementTarget::Native {
                    doc: other_doc,
                    node_id: other_id,
                },
            ) => {
                if !Rc::ptr_eq(doc, other_doc) {
                    return false;
                }
                let Ok(base) = doc.try_borrow() else {
                    return false;
                };
                if base.get_node(*container_id).is_none() {
                    return false;
                }
                let mut cur = Some(*other_id);
                while let Some(nid) = cur {
                    if nid == *container_id {
                        return true;
                    }
                    cur = base.get_node(nid).and_then(|n| n.parent);
                }
                false
            }
            #[cfg(target_arch = "wasm32")]
            (ElementTarget::Web(el), ElementTarget::Web(other_el)) => el.contains(Some(other_el)),
            _ => false,
        }
    }

    /// Checks if this element currently holds document active focus.
    pub fn has_focus(&self) -> bool {
        match &self.target {
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            ElementTarget::Native { doc, node_id } => doc
                .try_borrow()
                .map(|b| b.active_focus_node_id() == Some(*node_id))
                .unwrap_or(false),
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

    /// Checks whether this element currently matches the W3C `:focus-visible` pseudo-class.
    pub fn matches_focus_visible(&self) -> bool {
        match &self.target {
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            ElementTarget::Native { doc, node_id } => doc
                .try_borrow()
                .map(|b| b.is_focus_visible(*node_id))
                .unwrap_or(false),
            #[cfg(target_arch = "wasm32")]
            ElementTarget::Web(el) => el.matches(":focus-visible").unwrap_or(false),
            ElementTarget::Mounted(_) => false,
            #[cfg(not(target_arch = "wasm32"))]
            ElementTarget::Mock => false,
        }
    }

    /// Alias for [`Element::matches_focus_visible`].
    #[inline]
    pub fn is_focus_visible(&self) -> bool {
        self.matches_focus_visible()
    }

    /// Acquires W3C pointer capture on this element for `pointer_id`.
    pub fn set_pointer_capture(&self, pointer_id: i32) -> Result<(), HostError> {
        let _ = pointer_id;
        match &self.target {
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            ElementTarget::Native { doc, node_id } => {
                let mut base = doc.try_borrow_mut().map_err(|_| {
                    HostError::Unsupported("Native document is currently borrowed".into())
                })?;
                base.set_pointer_capture(*node_id, pointer_id)
                    .map_err(|err| HostError::Unsupported(err.to_string()))
            }
            #[cfg(target_arch = "wasm32")]
            ElementTarget::Web(el) => el.set_pointer_capture(pointer_id).map_err(|err| {
                HostError::Js(crate::JsError::Exception {
                    message: format!("{err:?}"),
                    stack: None,
                })
            }),
            ElementTarget::Mounted(_) => Err(HostError::Unsupported(
                "MountedHandle does not support synchronous set_pointer_capture".into(),
            )),
            #[cfg(not(target_arch = "wasm32"))]
            ElementTarget::Mock => Err(HostError::Unsupported(
                "Mock Element does not support set_pointer_capture".into(),
            )),
        }
    }

    /// Releases W3C pointer capture on this element for `pointer_id`.
    pub fn release_pointer_capture(&self, pointer_id: i32) -> Result<(), HostError> {
        let _ = pointer_id;
        match &self.target {
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            ElementTarget::Native { doc, node_id } => {
                let mut base = doc.try_borrow_mut().map_err(|_| {
                    HostError::Unsupported("Native document is currently borrowed".into())
                })?;
                base.release_pointer_capture(*node_id, pointer_id)
                    .map_err(|err| HostError::Unsupported(err.to_string()))
            }
            #[cfg(target_arch = "wasm32")]
            ElementTarget::Web(el) => el.release_pointer_capture(pointer_id).map_err(|err| {
                HostError::Js(crate::JsError::Exception {
                    message: format!("{err:?}"),
                    stack: None,
                })
            }),
            ElementTarget::Mounted(_) => Err(HostError::Unsupported(
                "MountedHandle does not support synchronous release_pointer_capture".into(),
            )),
            #[cfg(not(target_arch = "wasm32"))]
            ElementTarget::Mock => Err(HostError::Unsupported(
                "Mock Element does not support release_pointer_capture".into(),
            )),
        }
    }

    /// Checks whether this element currently holds or has pending W3C pointer capture for `pointer_id`.
    pub fn has_pointer_capture(&self, pointer_id: i32) -> bool {
        let _ = pointer_id;
        match &self.target {
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            ElementTarget::Native { doc, node_id } => doc
                .try_borrow()
                .map(|b| b.has_pointer_capture(*node_id, pointer_id))
                .unwrap_or(false),
            #[cfg(target_arch = "wasm32")]
            ElementTarget::Web(el) => el.has_pointer_capture(pointer_id),
            ElementTarget::Mounted(_) => false,
            #[cfg(not(target_arch = "wasm32"))]
            ElementTarget::Mock => false,
        }
    }

    /// Returns the ASCII-lowercase tag name of this element if available.
    pub fn tag_name(&self) -> Option<String> {
        match &self.target {
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            ElementTarget::Native { doc, node_id } => {
                let base = doc.try_borrow().ok()?;
                let node = base.get_node(*node_id)?;
                node.element_data()
                    .map(|el| (&*el.name.local).to_ascii_lowercase())
            }
            #[cfg(target_arch = "wasm32")]
            ElementTarget::Web(el) => Some(el.tag_name().to_ascii_lowercase()),
            ElementTarget::Mounted(_) => None,
            #[cfg(not(target_arch = "wasm32"))]
            ElementTarget::Mock => None,
        }
    }

    /// Tests whether this element matches the given CSS selector.
    pub fn matches(&self, selector: &str) -> bool {
        let _ = selector;
        match &self.target {
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            ElementTarget::Native { doc, node_id } => doc
                .try_borrow()
                .ok()
                .filter(|b| b.get_node(*node_id).is_some())
                .and_then(|b| b.matches_selector(*node_id, selector).ok())
                .unwrap_or(false),
            #[cfg(target_arch = "wasm32")]
            ElementTarget::Web(el) => el.matches(selector).unwrap_or(false),
            ElementTarget::Mounted(_) => false,
            #[cfg(not(target_arch = "wasm32"))]
            ElementTarget::Mock => false,
        }
    }

    /// Finds the closest inclusive ancestor matching the given CSS selector.
    pub fn closest(&self, selector: &str) -> Option<Self> {
        let _ = selector;
        match &self.target {
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            ElementTarget::Native { doc, node_id } => {
                let nid = doc
                    .try_borrow()
                    .ok()
                    .filter(|b| b.get_node(*node_id).is_some())
                    .and_then(|b| b.closest(*node_id, selector).ok().flatten())?;
                Some(Self::from_native(doc.clone(), nid))
            }
            #[cfg(target_arch = "wasm32")]
            ElementTarget::Web(el) => el.closest(selector).ok().flatten().map(Self::from_web),
            ElementTarget::Mounted(_) => None,
            #[cfg(not(target_arch = "wasm32"))]
            ElementTarget::Mock => None,
        }
    }

    /// Finds the closest inclusive ancestor whose element tag name matches `tag` (ASCII case-insensitively).
    pub fn closest_tag(&self, tag: &str) -> Option<Self> {
        let _ = tag;
        match &self.target {
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            ElementTarget::Native { doc, node_id } => {
                let base = doc.try_borrow().ok()?;
                let mut cur = Some(*node_id);
                while let Some(nid) = cur {
                    let node = base.get_node(nid)?;
                    if let Some(el) = node.element_data()
                        && (&*el.name.local).eq_ignore_ascii_case(tag)
                    {
                        drop(base);
                        return Some(Self::from_native(doc.clone(), nid));
                    }
                    cur = node.parent;
                }
                None
            }
            #[cfg(target_arch = "wasm32")]
            ElementTarget::Web(el) => el.closest(tag).ok().flatten().map(Self::from_web),
            ElementTarget::Mounted(_) => None,
            #[cfg(not(target_arch = "wasm32"))]
            ElementTarget::Mock => None,
        }
    }

    /// Queries the first descendant element of this element matching the given CSS selector.
    pub fn query_selector(&self, selector: &str) -> Option<Self> {
        let _ = selector;
        match &self.target {
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            ElementTarget::Native { doc, node_id } => {
                let nid = doc
                    .try_borrow()
                    .ok()
                    .filter(|b| b.get_node(*node_id).is_some())
                    .and_then(|b| b.query_selector_in(*node_id, selector).ok().flatten())?;
                Some(Self::from_native(doc.clone(), nid))
            }
            #[cfg(target_arch = "wasm32")]
            ElementTarget::Web(el) => el.query_selector(selector).ok().flatten().map(Self::from_web),
            ElementTarget::Mounted(_) => None,
            #[cfg(not(target_arch = "wasm32"))]
            ElementTarget::Mock => None,
        }
    }

    /// Synchronously scrolls the element into the visible viewport with default options when backed by a live DOM handle.
    pub fn scroll_into_view_sync(&self) -> Result<(), HostError> {
        self.scroll_into_view_sync_with_options(ScrollIntoViewOptions::default())
    }

    /// Synchronously scrolls the element into the visible viewport with specific options when backed by a live DOM handle.
    pub fn scroll_into_view_sync_with_options(
        &self,
        options: ScrollIntoViewOptions,
    ) -> Result<(), HostError> {
        match &self.target {
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            ElementTarget::Native { doc, node_id } => {
                let behavior = match options.behavior {
                    ScrollBehavior::Auto => blitz_dom::ScrollBehavior::Auto,
                    ScrollBehavior::Instant => blitz_dom::ScrollBehavior::Instant,
                    ScrollBehavior::Smooth => blitz_dom::ScrollBehavior::Smooth,
                };
                let block = match options.block {
                    ScrollLogicalPosition::Start => blitz_dom::ScrollLogicalPosition::Start,
                    ScrollLogicalPosition::Center => blitz_dom::ScrollLogicalPosition::Center,
                    ScrollLogicalPosition::End => blitz_dom::ScrollLogicalPosition::End,
                    ScrollLogicalPosition::Nearest => blitz_dom::ScrollLogicalPosition::Nearest,
                };
                let inline = match options.inline {
                    ScrollLogicalPosition::Start => blitz_dom::ScrollLogicalPosition::Start,
                    ScrollLogicalPosition::Center => blitz_dom::ScrollLogicalPosition::Center,
                    ScrollLogicalPosition::End => blitz_dom::ScrollLogicalPosition::End,
                    ScrollLogicalPosition::Nearest => blitz_dom::ScrollLogicalPosition::Nearest,
                };
                if let Ok(mut base) = doc.try_borrow_mut() {
                    base.scroll_into_view(*node_id, behavior, block, inline);
                } else {
                    let doc_clone = doc.clone();
                    let nid = *node_id;
                    let _ = crate::internal::safe_spawn(async move {
                        if let Ok(mut base) = doc_clone.try_borrow_mut() {
                            base.scroll_into_view(nid, behavior, block, inline);
                        }
                    });
                }
                Ok(())
            }
            #[cfg(target_arch = "wasm32")]
            ElementTarget::Web(el) => {
                let web_options = web_sys::ScrollIntoViewOptions::new();
                web_options.set_behavior(match options.behavior {
                    ScrollBehavior::Auto => web_sys::ScrollBehavior::Auto,
                    ScrollBehavior::Instant => web_sys::ScrollBehavior::Instant,
                    ScrollBehavior::Smooth => web_sys::ScrollBehavior::Smooth,
                });
                web_options.set_block(match options.block {
                    ScrollLogicalPosition::Start => web_sys::ScrollLogicalPosition::Start,
                    ScrollLogicalPosition::Center => web_sys::ScrollLogicalPosition::Center,
                    ScrollLogicalPosition::End => web_sys::ScrollLogicalPosition::End,
                    ScrollLogicalPosition::Nearest => web_sys::ScrollLogicalPosition::Nearest,
                });
                web_options.set_inline(match options.inline {
                    ScrollLogicalPosition::Start => web_sys::ScrollLogicalPosition::Start,
                    ScrollLogicalPosition::Center => web_sys::ScrollLogicalPosition::Center,
                    ScrollLogicalPosition::End => web_sys::ScrollLogicalPosition::End,
                    ScrollLogicalPosition::Nearest => web_sys::ScrollLogicalPosition::Nearest,
                });
                el.scroll_into_view_with_scroll_into_view_options(&web_options);
                Ok(())
            }
            ElementTarget::Mounted(handle) => {
                #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
                if let Some(nh) = handle.downcast::<dioxus_native::NodeHandle>() {
                    let el = Self::from_native(nh.base_document(), nh.node_id());
                    return el.scroll_into_view_sync_with_options(options);
                }
                let behavior = match options.behavior {
                    ScrollBehavior::Smooth => dioxus::prelude::ScrollBehavior::Smooth,
                    ScrollBehavior::Auto | ScrollBehavior::Instant => {
                        dioxus::prelude::ScrollBehavior::Instant
                    }
                };
                let handle = handle.clone();
                let _ = crate::internal::safe_spawn(async move {
                    let _ = handle.scroll_to(behavior).await;
                });
                Ok(())
            }
            #[cfg(not(target_arch = "wasm32"))]
            ElementTarget::Mock => {
                let _ = options;
                Ok(())
            }
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
        options: ScrollIntoViewOptions,
    ) -> Result<(), HostError> {
        match &self.target {
            ElementTarget::Mounted(handle) => {
                #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
                if let Some(nh) = handle.downcast::<dioxus_native::NodeHandle>() {
                    let el = Self::from_native(nh.base_document(), nh.node_id());
                    return el.scroll_into_view_sync_with_options(options);
                }
                let behavior = match options.behavior {
                    ScrollBehavior::Smooth => dioxus::prelude::ScrollBehavior::Smooth,
                    ScrollBehavior::Auto | ScrollBehavior::Instant => {
                        dioxus::prelude::ScrollBehavior::Instant
                    }
                };
                let _ = handle.scroll_to(behavior).await;
                Ok(())
            }
            _ => self.scroll_into_view_sync_with_options(options),
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
                let nid = *node_id;
                {
                    let mut base = doc.borrow_mut();
                    let Some(node) = base.get_node_mut(nid) else {
                        return Err(HostError::ElementNotFound(format!("NodeId: {:?}", nid)));
                    };
                    let offset = node.scroll_offset_mut();
                    offset.x = x;
                    offset.y = y;
                }
                let native_doc = crate::dom::Document::from_base(doc.clone());
                native_doc.dispatch_element_scroll_by_node_id(nid);
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

    /// Returns the value of the specified attribute, if present.
    pub fn get_attribute(&self, name: &str) -> Option<String> {
        let _ = name;
        match &self.target {
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            ElementTarget::Native { doc, node_id } => {
                let Ok(base) = doc.try_borrow() else {
                    return None;
                };
                let node = base.get_node(*node_id)?;
                let elem = node.element_data()?;
                elem.attrs
                    .iter()
                    .find(|a| (a.name.ns == blitz_dom::ns!() || a.name.ns.is_empty()) && &*a.name.local == name)
                    .map(|a| a.value.to_string())
            }
            #[cfg(target_arch = "wasm32")]
            ElementTarget::Web(el) => el.get_attribute(name),
            ElementTarget::Mounted(_) => None,
            #[cfg(not(target_arch = "wasm32"))]
            ElementTarget::Mock => None,
        }
    }

    /// Sets the value of the specified attribute on this element using the empty namespace `ns!()`.
    pub fn set_attribute(&self, name: &str, value: &str) {
        let _ = (name, value);
        match &self.target {
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            ElementTarget::Native { doc, node_id } => {
                let Ok(mut base) = doc.try_borrow_mut() else {
                    return;
                };
                let qname = blitz_dom::QualName::new(
                    None,
                    blitz_dom::ns!(),
                    blitz_dom::LocalName::from(name),
                );
                base.mutate().set_attribute(*node_id, qname, value);
            }
            #[cfg(target_arch = "wasm32")]
            ElementTarget::Web(el) => {
                let _ = el.set_attribute(name, value);
            }
            ElementTarget::Mounted(_) => {}
            #[cfg(not(target_arch = "wasm32"))]
            ElementTarget::Mock => {}
        }
    }

    /// Removes the specified attribute from this element.
    pub fn remove_attribute(&self, name: &str) {
        let _ = name;
        match &self.target {
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            ElementTarget::Native { doc, node_id } => {
                let Ok(mut base) = doc.try_borrow_mut() else {
                    return;
                };
                let qname = blitz_dom::QualName::new(
                    None,
                    blitz_dom::ns!(),
                    blitz_dom::LocalName::from(name),
                );
                base.mutate().clear_attribute(*node_id, qname);
            }
            #[cfg(target_arch = "wasm32")]
            ElementTarget::Web(el) => {
                let _ = el.remove_attribute(name);
            }
            ElementTarget::Mounted(_) => {}
            #[cfg(not(target_arch = "wasm32"))]
            ElementTarget::Mock => {}
        }
    }

    /// Checks if this element has the specified attribute.
    pub fn has_attribute(&self, name: &str) -> bool {
        let _ = name;
        match &self.target {
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            ElementTarget::Native { doc, node_id } => {
                let Ok(base) = doc.try_borrow() else {
                    return false;
                };
                let Some(node) = base.get_node(*node_id) else {
                    return false;
                };
                let Some(elem) = node.element_data() else {
                    return false;
                };
                elem.attrs
                    .iter()
                    .any(|a| (a.name.ns == blitz_dom::ns!() || a.name.ns.is_empty()) && &*a.name.local == name)
            }
            #[cfg(target_arch = "wasm32")]
            ElementTarget::Web(el) => el.has_attribute(name),
            ElementTarget::Mounted(_) => false,
            #[cfg(not(target_arch = "wasm32"))]
            ElementTarget::Mock => false,
        }
    }
}

/// Extracts the W3C DOM `pointerId` from a Dioxus `PointerData` payload across Web and Native hosts.
pub fn extract_pointer_id(evt: &dioxus::prelude::PointerData) -> i32 {
    #[cfg(target_arch = "wasm32")]
    if let Some(web_event) = evt.downcast::<web_sys::PointerEvent>() {
        return web_event.pointer_id();
    }
    #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
    if let Some(native_event) = evt.downcast::<dioxus_native::NativePointerData>() {
        return native_event.inner().id.to_dom_pointer_id();
    }
    evt.pointer_id()
}

/// Extracts the originating DOM target `Element` from a Dioxus `PointerData` payload across Web and Native hosts.
pub fn pointer_event_target(evt: &dioxus::prelude::PointerData) -> Option<Element> {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        let web_event = evt.downcast::<web_sys::PointerEvent>()?;
        let target = web_event.target()?;
        let el = target.dyn_into::<web_sys::Element>().ok()?;
        return Some(Element::from_web(el));
    }
    #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
    {
        let native_event = evt.downcast::<dioxus_native::NativePointerData>()?;
        let nh = native_event.target_node_handle()?;
        return Some(Element::from_native(nh.base_document(), nh.node_id()));
    }
    #[cfg(all(not(target_arch = "wasm32"), not(feature = "native")))]
    {
        let _ = evt;
        None
    }
}

/// Checks whether a Dioxus `PointerData` event originated on or inside an interactive `<button>` element.
pub fn is_interactive_button_target(evt: &dioxus::prelude::PointerData) -> bool {
    pointer_event_target(evt)
        .and_then(|el| el.closest_tag("button"))
        .is_some()
}

/// Extracts the originating DOM target `Element` from a Dioxus `MouseData` payload across Web and Native hosts.
pub fn mouse_event_target(evt: &dioxus::prelude::MouseData) -> Option<Element> {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        let web_event: &web_sys::MouseEvent = evt
            .downcast::<web_sys::MouseEvent>()
            .or_else(|| evt.downcast::<web_sys::PointerEvent>().map(|pe| pe.as_ref()))?;
        let target = web_event.target()?;
        if let Ok(el) = target.clone().dyn_into::<web_sys::Element>() {
            return Some(Element::from_web(el));
        }
        let node = target.dyn_into::<web_sys::Node>().ok()?;
        let parent_el = node.parent_element()?;
        return Some(Element::from_web(parent_el));
    }
    #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
    {
        let native_event = evt.downcast::<dioxus_native::NativePointerData>()?;
        let nh = native_event.target_node_handle()?;
        return Some(Element::from_native(nh.base_document(), nh.node_id()));
    }
    #[cfg(all(not(target_arch = "wasm32"), not(feature = "native")))]
    {
        let _ = evt;
        None
    }
}

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
#[derive(Clone, Copy, Debug)]
pub(crate) struct NativeMouseClickTracker {
    last_mousedown_time: Option<std::time::Instant>,
    mousedown_position: (f64, f64),
    click_count: i32,
}

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
impl Default for NativeMouseClickTracker {
    fn default() -> Self {
        Self {
            last_mousedown_time: None,
            mousedown_position: (0.0, 0.0),
            click_count: 0,
        }
    }
}

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
impl NativeMouseClickTracker {
    pub(crate) fn record_primary_press(
        &mut self,
        now: std::time::Instant,
        x: f64,
        y: f64,
    ) -> i32 {
        if let Some(prev_time) = self.last_mousedown_time {
            let elapsed = now.saturating_duration_since(prev_time);
            if elapsed < std::time::Duration::from_micros(50)
                && (self.mousedown_position.0 - x).abs() == 0.0
                && (self.mousedown_position.1 - y).abs() == 0.0
                && self.click_count >= 1
            {
                return self.click_count;
            }
            if elapsed < std::time::Duration::from_millis(500)
                && (self.mousedown_position.0 - x).abs() <= 2.0
                && (self.mousedown_position.1 - y).abs() <= 2.0
            {
                self.click_count = self.click_count.saturating_add(1);
            } else {
                self.click_count = 1;
            }
        } else {
            self.click_count = 1;
        }
        self.last_mousedown_time = Some(now);
        self.mousedown_position = (x, y);
        self.click_count
    }
}

/// Returns the W3C UIEvent `detail` click count (`1` for single click, `2` for double click, etc.)
/// for a Dioxus `MouseData` payload across Web and Blitz Native hosts.
pub fn mouse_event_detail(evt: &dioxus::prelude::MouseData) -> i32 {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(web_event) = evt
            .downcast::<web_sys::MouseEvent>()
            .or_else(|| evt.downcast::<web_sys::PointerEvent>().map(|pe| pe.as_ref()))
        {
            return web_event.detail();
        }
        return 1;
    }
    #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
    {
        std::thread_local! {
            static CLICK_TRACKER: std::cell::RefCell<NativeMouseClickTracker> =
                std::cell::RefCell::new(NativeMouseClickTracker::default());
        }
        let Some(native_event) = evt.downcast::<dioxus_native::NativePointerData>() else {
            return 1;
        };
        let pe = native_event.inner();
        if pe.button != blitz_traits::events::MouseEventButton::Main {
            return 1;
        }
        let x = pe.page_x() as f64;
        let y = pe.page_y() as f64;
        let now = std::time::Instant::now();
        return CLICK_TRACKER.with(|cell| cell.borrow_mut().record_primary_press(now, x, y));
    }
    #[cfg(all(not(target_arch = "wasm32"), not(feature = "native")))]
    {
        let _ = evt;
        1
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
    Instant,
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
        let mut opts = ScrollIntoViewOptions::default();
        assert_eq!(opts.behavior, ScrollBehavior::Auto);
        assert_eq!(opts.block, ScrollLogicalPosition::Start);
        assert_eq!(opts.inline, ScrollLogicalPosition::Start);

        opts.behavior = ScrollBehavior::Instant;
        opts.block = ScrollLogicalPosition::Center;
        opts.inline = ScrollLogicalPosition::End;
        assert_eq!(opts.behavior, ScrollBehavior::Instant);
        assert_eq!(opts.block, ScrollLogicalPosition::Center);
        assert_eq!(opts.inline, ScrollLogicalPosition::End);

        #[cfg(not(target_arch = "wasm32"))]
        {
            let el = Element::mock();
            assert_eq!(el.id(), None);
            assert!(!el.is_connected());
            assert!(!el.has_focus());
            assert!(!el.matches_focus_visible());
            assert!(!el.has_pointer_capture(1));
            assert!(el.set_pointer_capture(1).is_err());
            assert!(el.release_pointer_capture(1).is_err());
            assert_eq!(el.tag_name(), None);
            assert!(el.closest_tag("button").is_none());
            assert!(el.query_selector("button").is_none());
            assert!(!el.has_attribute("data-state"));
            assert_eq!(el.get_attribute("data-state"), None);
            el.set_attribute("data-state", "open");
            el.remove_attribute("data-state");
            let debug_str = format!("{:?}", el);
            assert!(debug_str.contains("Element"));
        }

        let default_focus = FocusOptions::default();
        assert!(!default_focus.prevent_scroll);
        assert_eq!(default_focus.focus_visible, None);

        let no_scroll = FocusOptions::new(true);
        assert!(no_scroll.prevent_scroll);
        assert_eq!(no_scroll.focus_visible, None);

        let suppress_ring = FocusOptions::with_focus_visible(false, Some(false));
        assert!(!suppress_ring.prevent_scroll);
        assert_eq!(suppress_ring.focus_visible, Some(false));
    }

    #[test]
    fn test_native_mouse_click_tracker_parity() {
        use std::time::{Duration, Instant};

        let mut tracker = NativeMouseClickTracker::default();
        let t0 = Instant::now();

        // First primary press -> detail 1
        assert_eq!(tracker.record_primary_press(t0, 100.0, 200.0), 1);

        // Repeated read within < 50us at identical coords -> deduplicated (still 1)
        assert_eq!(
            tracker.record_primary_press(t0 + Duration::from_micros(10), 100.0, 200.0),
            1
        );

        // Second press within 500ms and <= 2.0px -> detail 2
        let t1 = t0 + Duration::from_millis(120);
        assert_eq!(tracker.record_primary_press(t1, 101.5, 198.5), 2);

        // Third press within 500ms and <= 2.0px -> detail 3
        let t2 = t1 + Duration::from_millis(150);
        assert_eq!(tracker.record_primary_press(t2, 102.0, 200.0), 3);

        // Press > 2.0px away resets to detail 1
        let t3 = t2 + Duration::from_millis(80);
        assert_eq!(tracker.record_primary_press(t3, 106.0, 200.0), 1);

        // Press >= 500ms later resets to detail 1
        let t4 = t3 + Duration::from_millis(500);
        assert_eq!(tracker.record_primary_press(t4, 106.0, 200.0), 1);
    }
}

