//! Web (`wasm32`) DOM backend implementation backed by `web-sys`.

fn parse_css_transform_scale(transform: &str) -> (f64, f64) {
    let trimmed = transform.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
        return (1.0, 1.0);
    }
    if let Some(inner) = trimmed
        .strip_prefix("matrix3d(")
        .and_then(|s| s.strip_suffix(')'))
    {
        let mut vals = [0.0_f64; 16];
        let mut count = 0;
        for part in inner.split(',') {
            if count >= 16 {
                return (1.0, 1.0);
            }
            let Ok(v) = part.trim().parse::<f64>() else {
                return (1.0, 1.0);
            };
            vals[count] = v;
            count += 1;
        }
        if count == 16 {
            let sx = (vals[0] * vals[0] + vals[1] * vals[1] + vals[2] * vals[2]).sqrt();
            let sy = (vals[4] * vals[4] + vals[5] * vals[5] + vals[6] * vals[6]).sqrt();
            if sx.is_finite() && sy.is_finite() {
                return (sx, sy);
            }
        }
        return (1.0, 1.0);
    }
    if let Some(inner) = trimmed
        .strip_prefix("matrix(")
        .and_then(|s| s.strip_suffix(')'))
    {
        let mut vals = [0.0_f64; 6];
        let mut count = 0;
        for part in inner.split(',') {
            if count >= 6 {
                return (1.0, 1.0);
            }
            let Ok(v) = part.trim().parse::<f64>() else {
                return (1.0, 1.0);
            };
            vals[count] = v;
            count += 1;
        }
        if count == 6 {
            let sx = vals[0].hypot(vals[1]);
            let sy = vals[2].hypot(vals[3]);
            if sx.is_finite() && sy.is_finite() {
                return (sx, sy);
            }
        }
    }
    (1.0, 1.0)
}

fn compute_cumulative_transform_scale(win: &web_sys::Window, el: &web_sys::Element) -> (f64, f64) {
    let mut sx = 1.0_f64;
    let mut sy = 1.0_f64;
    let mut cur_opt = Some(el.clone());
    while let Some(cur) = cur_opt {
        if let Ok(Some(style)) = win.get_computed_style(&cur)
            && let Ok(transform) = style.get_property_value("transform")
        {
            let (step_x, step_y) = parse_css_transform_scale(&transform);
            sx *= step_x;
            sy *= step_y;
        }
        cur_opt = cur.parent_element();
    }
    (sx, sy)
}

/// Web browser Document implementation.
#[derive(Clone, Debug, Default)]
pub struct Document {
    doc: Option<web_sys::Document>,
}

impl Document {
    /// Returns the current active browser document.
    pub fn current() -> Option<Self> {
        let win = web_sys::window()?;
        let doc = win.document()?;
        Some(Self { doc: Some(doc) })
    }

    /// Creates a Document from a raw `web_sys::Document`.
    #[inline]
    pub fn from_raw(doc: web_sys::Document) -> Self {
        Self { doc: Some(doc) }
    }

    /// Returns whether this document is authoritative. Always true on web.
    #[inline]
    pub fn is_authoritative(&self) -> bool {
        true
    }

    /// Access the underlying `web_sys::Document`.
    #[inline]
    pub fn raw(&self) -> &web_sys::Document {
        self.doc
            .as_ref()
            .expect("Document has no active web_sys::Document")
    }

    /// Returns the element with the specified ID if present in the active browser document.
    pub fn element_by_id(&self, id: &str) -> Option<crate::dom::element::Element> {
        let doc = self.doc.as_ref()?;
        let el = doc.get_element_by_id(id)?;
        Some(crate::dom::element::Element::from_web(el))
    }

    /// Queries the first element in the browser document matching the given CSS selector.
    pub fn query_selector(&self, selector: &str) -> Option<crate::dom::element::Element> {
        let doc = self.doc.as_ref()?;
        let el = doc.query_selector(selector).ok().flatten()?;
        Some(crate::dom::element::Element::from_web(el))
    }

    /// Requests focus for the element with the specified ID in the browser document.
    pub fn set_focus(&self, element_id: &str) -> Result<(), crate::error::HostError> {
        self.set_focus_with_options(element_id, crate::dom::element::FocusOptions::default())
    }

    /// Requests focus for the element with the specified ID and focus options in the browser document.
    pub fn set_focus_with_options(
        &self,
        element_id: &str,
        options: crate::dom::element::FocusOptions,
    ) -> Result<(), crate::error::HostError> {
        let el = self
            .element_by_id(element_id)
            .ok_or_else(|| crate::error::HostError::ElementNotFound(element_id.to_string()))?;
        el.focus_sync_with_options(options)
    }

    /// Checks if the element with the specified ID currently holds document focus in the browser document.
    pub fn is_element_active(&self, element_id: &str) -> bool {
        self.element_by_id(element_id)
            .map(|el| el.has_focus())
            .unwrap_or(false)
    }

    /// Checks whether document active focus currently resides on or inside `element_id`.
    pub fn is_focus_within(&self, element_id: &str) -> bool {
        let Some(container) = self.element_by_id(element_id) else {
            return false;
        };
        let Some(active) = self.active_element() else {
            return false;
        };
        container.contains(&active)
    }

    /// Removes keyboard focus from the currently active element in the browser document.
    pub fn blur_active(&self) -> Result<(), crate::error::HostError> {
        if let Some(active) = self.active_element() {
            active.blur_sync()?;
        }
        Ok(())
    }

    /// Checks if the element with the specified ID currently matches `:focus-visible` in the browser document.
    pub fn is_element_focus_visible(&self, element_id: &str) -> bool {
        self.element_by_id(element_id)
            .map(|el| el.matches_focus_visible())
            .unwrap_or(false)
    }

    /// Returns the activeElement if currently focused in the browser document.
    pub fn active_element(&self) -> Option<crate::dom::element::Element> {
        let doc = self.doc.as_ref()?;
        let el = doc.active_element()?;
        Some(crate::dom::element::Element::from_web(el))
    }

    /// Returns the Window handle associated with this document.
    pub fn window(&self) -> crate::window::Window {
        crate::window::window()
    }

    /// Sets the document viewport scroll offset.
    pub fn set_viewport_scroll(&self, x: f64, y: f64) {
        if let Some(win) = web_sys::window() {
            win.scroll_to_with_x_and_y(x, y);
        }
    }

    /// Queries the live viewport dimensions (width, height).
    pub fn inner_size(&self) -> (f64, f64) {
        if let Some(win) = web_sys::window() {
            let w = win
                .inner_width()
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let h = win
                .inner_height()
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            (w, h)
        } else {
            (0.0, 0.0)
        }
    }

    /// Queries the live viewport scroll offset (x, y).
    pub fn viewport_scroll_offset(&self) -> (f64, f64) {
        if let Some(win) = web_sys::window() {
            let x = win.scroll_x().unwrap_or(0.0);
            let y = win.scroll_y().unwrap_or(0.0);
            (x, y)
        } else {
            (0.0, 0.0)
        }
    }

    /// Queries the live viewport dimensions and scroll offsets from the browser window.
    pub fn viewport(&self) -> crate::runtime::geometry::Viewport {
        let (width, height) = self.inner_size();
        let (scroll_x, scroll_y) = self.viewport_scroll_offset();
        crate::runtime::geometry::Viewport {
            width,
            height,
            scroll_x,
            scroll_y,
        }
    }

    /// Measures the bounding client rectangle of an element by ID relative to the viewport.
    pub fn measure_rect(&self, element_id: &str) -> Option<crate::runtime::geometry::Rect> {
        let doc = self.doc.as_ref()?;
        let el = doc.get_element_by_id(element_id)?;
        let br = el.get_bounding_client_rect();
        Some(crate::runtime::geometry::Rect::new(
            br.x(),
            br.y(),
            br.width(),
            br.height(),
        ))
    }

    /// Measures the bounding rectangle of an element with ancestor CSS `transform` scale factored out of `width`/`height`.
    pub fn measure_unscaled_rect(
        &self,
        element_id: &str,
    ) -> Option<crate::runtime::geometry::Rect> {
        use wasm_bindgen::JsCast;

        let doc = self.doc.as_ref()?;
        let el = doc.get_element_by_id(element_id)?;
        let br = el.get_bounding_client_rect();
        let (sx, sy) = web_sys::window()
            .map(|win| compute_cumulative_transform_scale(&win, &el))
            .unwrap_or((1.0, 1.0));

        let width = if sx.is_finite() && sx > 1e-4 {
            br.width() / sx
        } else {
            el.clone()
                .dyn_into::<web_sys::HtmlElement>()
                .map(|h| h.offset_width() as f64)
                .unwrap_or_else(|_| br.width())
        };
        let height = if sy.is_finite() && sy > 1e-4 {
            br.height() / sy
        } else {
            el.clone()
                .dyn_into::<web_sys::HtmlElement>()
                .map(|h| h.offset_height() as f64)
                .unwrap_or_else(|_| br.height())
        };

        Some(crate::runtime::geometry::Rect::new(
            br.x(),
            br.y(),
            width,
            height,
        ))
    }

    /// Measures the unscaled natural `height: auto` border-box height of an element, restoring any inline `height` property immediately.
    pub fn measure_auto_height(&self, element_id: &str) -> Option<f64> {
        use wasm_bindgen::JsCast;

        let doc = self.doc.as_ref()?;
        let el = doc.get_element_by_id(element_id)?;
        let Ok(html_el) = el.clone().dyn_into::<web_sys::HtmlElement>() else {
            return self
                .measure_unscaled_rect(element_id)
                .map(|r| r.height)
                .filter(|&h| h > 0.0);
        };

        let style = html_el.style();
        let prev_height = style.get_property_value("height").unwrap_or_default();
        let prev_priority = style.get_property_priority("height");

        let _ = style.set_property("height", "auto");

        let sy = web_sys::window()
            .map(|win| compute_cumulative_transform_scale(&win, &el).1)
            .unwrap_or(1.0);
        let safe_sy = if sy.is_finite() && sy > 1e-4 { sy } else { 1.0 };

        let offset_h = html_el.offset_height() as f64;
        let rect_h = el.get_bounding_client_rect().height() / safe_sy;

        if prev_height.is_empty() {
            let _ = style.remove_property("height");
        } else {
            let _ = style.set_property_with_priority("height", &prev_height, &prev_priority);
        }

        if rect_h.is_finite() && rect_h > 0.0 && (rect_h - offset_h).abs() < 1.0 {
            Some(rect_h)
        } else if offset_h > 0.0 {
            Some(offset_h)
        } else if rect_h.is_finite() && rect_h > 0.0 {
            Some(rect_h)
        } else {
            None
        }
    }

    /// Measures the unscaled natural border-box dimensions (width, height) without animation or transition constraints.
    pub fn measure_without_animation(&self, element_id: &str) -> Option<(f64, f64)> {
        use wasm_bindgen::JsCast;

        let doc = self.doc.as_ref()?;
        let el = doc.get_element_by_id(element_id)?;
        let Ok(html_el) = el.clone().dyn_into::<web_sys::HtmlElement>() else {
            let r = self.measure_unscaled_rect(element_id)?;
            return (r.width > 0.0 || r.height > 0.0).then_some((r.width, r.height));
        };

        let style = html_el.style();
        let prev_transition = style
            .get_property_value("transition-duration")
            .unwrap_or_default();
        let prev_transition_priority = style.get_property_priority("transition-duration");
        let prev_animation = style
            .get_property_value("animation-name")
            .unwrap_or_default();
        let prev_animation_priority = style.get_property_priority("animation-name");

        let _ = style.set_property("transition-duration", "0s");
        let _ = style.set_property("animation-name", "none");

        let (sx, sy) = web_sys::window()
            .map(|win| compute_cumulative_transform_scale(&win, &el))
            .unwrap_or((1.0, 1.0));
        let safe_sx = if sx.is_finite() && sx > 1e-4 { sx } else { 1.0 };
        let safe_sy = if sy.is_finite() && sy > 1e-4 { sy } else { 1.0 };

        let rect = el.get_bounding_client_rect();
        let rect_w = rect.width() / safe_sx;
        let rect_h = rect.height() / safe_sy;

        let offset_w = html_el.offset_width() as f64;
        let offset_h = html_el.offset_height() as f64;

        if prev_transition.is_empty() {
            let _ = style.remove_property("transition-duration");
        } else {
            let _ = style.set_property_with_priority(
                "transition-duration",
                &prev_transition,
                &prev_transition_priority,
            );
        }

        if prev_animation.is_empty() {
            let _ = style.remove_property("animation-name");
        } else {
            let _ = style.set_property_with_priority(
                "animation-name",
                &prev_animation,
                &prev_animation_priority,
            );
        }

        let final_w = if rect_w.is_finite() && rect_w > 0.0 && (rect_w - offset_w).abs() < 1.0 {
            rect_w
        } else if offset_w > 0.0 {
            offset_w
        } else if rect_w.is_finite() && rect_w > 0.0 {
            rect_w
        } else {
            0.0
        };

        let final_h = if rect_h.is_finite() && rect_h > 0.0 && (rect_h - offset_h).abs() < 1.0 {
            rect_h
        } else if offset_h > 0.0 {
            offset_h
        } else if rect_h.is_finite() && rect_h > 0.0 {
            rect_h
        } else {
            0.0
        };

        if final_w > 0.0 || final_h > 0.0 {
            Some((final_w, final_h))
        } else {
            None
        }
    }

    /// Returns `true` when the element currently has a running or pending CSS animation or transition.
    pub fn is_element_animating(&self, element_id: &str) -> bool {
        let Some(doc) = self.doc.as_ref() else {
            return false;
        };
        let Some(el) = doc.get_element_by_id(element_id) else {
            return false;
        };
        element_has_running_animations(&el).unwrap_or(false)
    }

    /// Looks up `element_id` in this document and returns its CSSOM computed `z-index`
    /// string (`"auto"` or integer string), or `None` if not found / unavailable.
    pub fn computed_z_index_by_id(&self, element_id: &str) -> Option<String> {
        self.element_by_id(element_id)?.computed_z_index()
    }

    /// Checks whether any anchor element is occluded or outside viewport bounds.
    pub fn is_reference_hidden(&self, anchor_ids: &[&str]) -> bool {
        let vp = self.viewport();
        for &id in anchor_ids {
            let Some(rect) = self.measure_rect(id) else {
                return true;
            };
            if rect.width <= 0.0 && rect.height <= 0.0 {
                return true;
            }
            if rect.bottom < 0.0 || rect.top > vp.height {
                return true;
            }
            if rect.right < 0.0 || rect.left > vp.width {
                return true;
            }
        }
        false
    }

    /// Returns whether the browser document is currently hidden (`document.hidden` or `visibilityState === "hidden"`).
    pub fn is_document_hidden(&self) -> bool {
        let Some(doc) = self.doc.as_ref() else {
            return false;
        };
        doc.hidden() || doc.visibility_state() == web_sys::VisibilityState::Hidden
    }

    /// Returns `true` when the browser document currently has a non-empty text selection.
    pub fn has_text_selection(&self) -> bool {
        let Some(win) = web_sys::window() else {
            return false;
        };
        let Ok(Some(sel)) = win.get_selection() else {
            return false;
        };
        !sel.is_collapsed() && !String::from(js_sys::Object::to_string(&sel)).is_empty()
    }

    /// Host API parity setter for document visibility (browser owns `document.hidden` on Web).
    pub fn set_document_hidden(&self, _hidden: bool) {}

    /// Returns whether any CSS animations are currently active. Always false on web FFI.
    #[inline]
    pub fn is_animating(&self) -> bool {
        false
    }

    /// Synchronizes hosted observers. No-op on web FFI.
    #[inline]
    pub fn pump_hosted_observers(&self) {}

    /// Returns the active observer counts for element scroll listeners.
    pub fn element_scroll_observer_count(&self, _element_id: Option<&str>) -> usize {
        0
    }

    /// Dispatches an animation completion signal for testing / host API parity.
    pub fn dispatch_animation_end(&self, _element_id: &str, _animation_name: &str) {}

    /// Dispatches an animation cancellation signal for testing / host API parity.
    pub fn dispatch_animation_cancel(&self, _element_id: &str, _animation_name: &str) {}

    /// Dispatches a transition completion signal for testing / host API parity.
    pub fn dispatch_transition_end(&self, _element_id: &str, _property_name: &str) {}

    /// Dispatches a transition cancellation signal for testing / host API parity.
    pub fn dispatch_transition_cancel(&self, _element_id: &str, _property_name: &str) {}

    /// Queries the descendant tree order of elements matching `candidate_ids` within `root_id`.
    ///
    /// Verifies containment and sorts elements by document position using
    /// `web_sys::Node::DOCUMENT_POSITION_FOLLOWING` / `PRECEDING`.
    pub fn query_descendant_order(
        &self,
        root_id: &str,
        candidate_ids: &[&str],
    ) -> Result<Vec<String>, crate::error::HostError> {
        let doc = self.doc.as_ref().ok_or_else(|| {
            crate::error::HostError::Unsupported("No active browser document".into())
        })?;
        let root = doc
            .get_element_by_id(root_id)
            .ok_or_else(|| crate::error::HostError::ElementNotFound(root_id.to_string()))?;

        let mut matched_candidates = Vec::new();
        for &id in candidate_ids {
            if let Some(el) = doc.get_element_by_id(id)
                && root.contains(Some(&el))
            {
                matched_candidates.push((id.to_string(), el));
            }
        }

        matched_candidates.sort_by(|(_, a), (_, b)| {
            let pos = a.compare_document_position(b);
            if (pos & web_sys::Node::DOCUMENT_POSITION_FOLLOWING) != 0 {
                std::cmp::Ordering::Less
            } else if (pos & web_sys::Node::DOCUMENT_POSITION_PRECEDING) != 0 {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        });

        let ordered_ids = matched_candidates.into_iter().map(|(id, _)| id).collect();
        Ok(ordered_ids)
    }

    /// Suppresses text selection on `document.body` during pointer drag gestures.
    pub fn set_body_user_select_none(&self) {
        let Some(doc) = self
            .doc
            .clone()
            .or_else(|| web_sys::window().and_then(|w| w.document()))
        else {
            return;
        };
        if let Some(body) = doc.body() {
            let style = body.style();
            let _ = style.set_property("user-select", "none");
            let _ = style.set_property("-webkit-user-select", "none");
        }
    }

    /// Restores text selection on `document.body` after pointer drag gestures complete.
    pub fn restore_body_user_select(&self) {
        let Some(doc) = self
            .doc
            .clone()
            .or_else(|| web_sys::window().and_then(|w| w.document()))
        else {
            return;
        };
        if let Some(body) = doc.body() {
            let style = body.style();
            let _ = style.remove_property("user-select");
            let _ = style.remove_property("-webkit-user-select");
        }
    }

    /// Teleports a portal element by ID into `host_id` (if non-empty) or falls back to `#main` / `document.body`.
    pub fn teleport_portal_to_host(&self, element_id: &str, host_id: Option<&str>) {
        let Some(document) = self
            .doc
            .clone()
            .or_else(|| web_sys::window().and_then(|w| w.document()))
        else {
            return;
        };
        let Some(el) = document.get_element_by_id(element_id) else {
            return;
        };

        let host: Option<web_sys::Node> = match host_id.filter(|id| !id.is_empty()) {
            Some(id) => document.get_element_by_id(id).map(Into::into),
            None => document
                .get_element_by_id("main")
                .map(Into::into)
                .or_else(|| document.body().map(Into::into)),
        };

        if let Some(host) = host
            && el.parent_node() != Some(host.clone())
        {
            let _ = host.append_child(&el);
        }
    }

    /// Removes a teleported portal element by ID from the browser DOM on cleanup.
    pub fn remove_portal_element_by_id(&self, element_id: &str) {
        let Some(document) = self
            .doc
            .clone()
            .or_else(|| web_sys::window().and_then(|w| w.document()))
        else {
            return;
        };
        if let Some(el) = document.get_element_by_id(element_id) {
            el.remove();
        }
    }

    /// Serializes live submittable `(name, value)` entries for the `<form>` matching `form_element_id`
    /// using the browser's native `FormData` algorithm.
    pub fn collect_form_data(
        &self,
        form_element_id: &str,
    ) -> Result<Vec<(String, String)>, crate::error::HostError> {
        use wasm_bindgen::JsCast;

        let doc = self
            .doc
            .clone()
            .or_else(|| web_sys::window().and_then(|w| w.document()))
            .ok_or_else(|| {
                crate::error::HostError::Unsupported("No active browser document".into())
            })?;
        let el = doc
            .get_element_by_id(form_element_id)
            .ok_or_else(|| crate::error::HostError::ElementNotFound(form_element_id.to_string()))?;
        let form = el.dyn_into::<web_sys::HtmlFormElement>().map_err(|_| {
            crate::error::HostError::Unsupported(format!(
                "Element '{form_element_id}' is not an HTMLFormElement"
            ))
        })?;
        let form_data = web_sys::FormData::new_with_form(&form).map_err(|err| {
            crate::error::HostError::Unsupported(format!("FormData construction failed: {err:?}"))
        })?;

        let mut entries = Vec::new();
        let iter = js_sys::try_iter(&form_data).ok().flatten().ok_or_else(|| {
            crate::error::HostError::Unsupported("FormData is not iterable".into())
        })?;
        for item in iter {
            if let Ok(pair) = item
                && let Some(arr) = pair.dyn_ref::<js_sys::Array>()
                && arr.length() >= 2
                && let Some(name) = arr.get(0).as_string()
                && let Some(val) = arr.get(1).as_string()
            {
                entries.push((name, val));
            }
        }
        Ok(entries)
    }
}

struct ResizeSubscriber {
    target_id: String,
    active: std::cell::Cell<bool>,
    pending_entry: std::cell::RefCell<Option<crate::dom::observer::ResizeEntry>>,
    frame_guard: std::cell::RefCell<Option<crate::frame::FrameRequestGuard>>,
    callback: std::cell::RefCell<Box<dyn FnMut(crate::dom::observer::ResizeEntry) + 'static>>,
}

struct PooledTarget {
    element: web_sys::Element,
    subscribers: Vec<(u64, std::rc::Rc<ResizeSubscriber>)>,
}

struct BoxPool {
    box_model: crate::dom::observer::ResizeObserverBox,
    observer: Option<web_sys::ResizeObserver>,
    js_cb: Option<wasm_bindgen::closure::Closure<dyn Fn(js_sys::Array)>>,
    element_keys: js_sys::Map,
    next_target_key: u64,
    targets: std::collections::HashMap<u64, PooledTarget>,
}

impl BoxPool {
    fn new(box_model: crate::dom::observer::ResizeObserverBox) -> Self {
        Self {
            box_model,
            observer: None,
            js_cb: None,
            element_keys: js_sys::Map::new(),
            next_target_key: 1,
            targets: std::collections::HashMap::new(),
        }
    }

    fn observer_options(&self) -> web_sys::ResizeObserverOptions {
        let options = web_sys::ResizeObserverOptions::new();
        match self.box_model {
            crate::dom::observer::ResizeObserverBox::BorderBox => {
                options.set_box(web_sys::ResizeObserverBoxOptions::BorderBox);
            }
            crate::dom::observer::ResizeObserverBox::ContentBox => {
                options.set_box(web_sys::ResizeObserverBoxOptions::ContentBox);
            }
        }
        options
    }
}

struct ResizePools {
    border_box: BoxPool,
    content_box: BoxPool,
}

impl ResizePools {
    fn new() -> Self {
        Self {
            border_box: BoxPool::new(crate::dom::observer::ResizeObserverBox::BorderBox),
            content_box: BoxPool::new(crate::dom::observer::ResizeObserverBox::ContentBox),
        }
    }

    fn pool(&self, box_model: crate::dom::observer::ResizeObserverBox) -> &BoxPool {
        match box_model {
            crate::dom::observer::ResizeObserverBox::BorderBox => &self.border_box,
            crate::dom::observer::ResizeObserverBox::ContentBox => &self.content_box,
        }
    }

    fn pool_mut(&mut self, box_model: crate::dom::observer::ResizeObserverBox) -> &mut BoxPool {
        match box_model {
            crate::dom::observer::ResizeObserverBox::BorderBox => &mut self.border_box,
            crate::dom::observer::ResizeObserverBox::ContentBox => &mut self.content_box,
        }
    }
}

std::thread_local! {
    static RESIZE_POOLS: std::cell::RefCell<ResizePools> =
        std::cell::RefCell::new(ResizePools::new());
}

fn extract_resize_rect(
    entry: &web_sys::ResizeObserverEntry,
    target: &web_sys::Element,
    box_model: crate::dom::observer::ResizeObserverBox,
) -> crate::runtime::geometry::Rect {
    use wasm_bindgen::JsCast;

    match box_model {
        crate::dom::observer::ResizeObserverBox::BorderBox => {
            let bbs = entry.border_box_size();
            let (w, h) = if bbs.length() > 0 {
                if let Ok(size) = bbs.get(0).dyn_into::<web_sys::ResizeObserverSize>() {
                    (size.inline_size(), size.block_size())
                } else {
                    let cr = entry.content_rect();
                    (cr.width(), cr.height())
                }
            } else if let Ok(html_el) = target.clone().dyn_into::<web_sys::HtmlElement>() {
                let ow = html_el.offset_width() as f64;
                let oh = html_el.offset_height() as f64;
                if ow > 0.0 || oh > 0.0 {
                    (ow, oh)
                } else {
                    let cr = entry.content_rect();
                    (cr.width(), cr.height())
                }
            } else {
                let cr = entry.content_rect();
                (cr.width(), cr.height())
            };
            let cr = entry.content_rect();
            crate::runtime::geometry::Rect::new(cr.x(), cr.y(), w, h)
        }
        crate::dom::observer::ResizeObserverBox::ContentBox => {
            let cr = entry.content_rect();
            crate::runtime::geometry::Rect::new(cr.x(), cr.y(), cr.width(), cr.height())
        }
    }
}

fn dispatch_resize_entries(
    box_model: crate::dom::observer::ResizeObserverBox,
    entries: js_sys::Array,
) {
    use std::rc::Rc;
    use wasm_bindgen::JsCast;

    for idx in 0..entries.length() {
        let Ok(entry) = entries.get(idx).dyn_into::<web_sys::ResizeObserverEntry>() else {
            continue;
        };
        let target = entry.target();
        let rect = extract_resize_rect(&entry, &target, box_model);

        let subscribers: Vec<Rc<ResizeSubscriber>> = RESIZE_POOLS.with(|cell| {
            let Ok(pools) = cell.try_borrow() else {
                return Vec::new();
            };
            let pool = pools.pool(box_model);
            let Some(target_key) = pool
                .element_keys
                .get(target.as_ref())
                .as_f64()
                .map(|k| k as u64)
            else {
                return Vec::new();
            };
            let Some(pooled) = pool.targets.get(&target_key) else {
                return Vec::new();
            };
            pooled
                .subscribers
                .iter()
                .filter(|(_, sub)| sub.active.get())
                .map(|(_, sub)| Rc::clone(sub))
                .collect()
        });

        for sub in subscribers {
            if !sub.active.get() {
                continue;
            }
            if let Ok(mut pending) = sub.pending_entry.try_borrow_mut() {
                *pending = Some(crate::dom::observer::ResizeEntry {
                    target_id: sub.target_id.clone(),
                    content_rect: rect,
                });
            }
            let needs_schedule = sub.frame_guard.try_borrow().is_ok_and(|g| g.is_none());
            if needs_schedule {
                let sub_for_frame = Rc::clone(&sub);
                if let Ok(guard) = crate::frame::request_next_frame(move |_| {
                    if let Ok(mut fg) = sub_for_frame.frame_guard.try_borrow_mut() {
                        *fg = None;
                    }
                    if !sub_for_frame.active.get() {
                        return;
                    }
                    let entry = sub_for_frame
                        .pending_entry
                        .try_borrow_mut()
                        .ok()
                        .and_then(|mut p| p.take());
                    if let Some(entry) = entry
                        && let Ok(mut cb) = sub_for_frame.callback.try_borrow_mut()
                    {
                        cb(entry);
                    }
                }) {
                    if sub.active.get()
                        && let Ok(mut fg) = sub.frame_guard.try_borrow_mut()
                    {
                        *fg = Some(guard);
                    }
                }
            }
        }
    }
}

fn unsubscribe_resize_pool(
    box_model: crate::dom::observer::ResizeObserverBox,
    target_key: u64,
    sub_id: u64,
) {
    let torn_down = RESIZE_POOLS.with(|cell| {
        let Ok(mut pools) = cell.try_borrow_mut() else {
            return None;
        };
        let pool = pools.pool_mut(box_model);
        let mut remove_target = false;
        if let Some(pooled) = pool.targets.get_mut(&target_key) {
            pooled.subscribers.retain(|(id, _)| *id != sub_id);
            remove_target = pooled.subscribers.is_empty();
        }
        if remove_target
            && let Some(removed) = pool.targets.remove(&target_key)
        {
            if let Some(observer) = pool.observer.as_ref() {
                observer.unobserve(&removed.element);
            }
            pool.element_keys.delete(removed.element.as_ref());
        }
        if pool.targets.is_empty() {
            let observer = pool.observer.take();
            let js_cb = pool.js_cb.take();
            Some((observer, js_cb))
        } else {
            None
        }
    });

    if let Some((observer, js_cb)) = torn_down {
        if let Some(observer) = observer {
            observer.disconnect();
        }
        drop(js_cb);
    }
}

pub(crate) fn observe_element_resize_with_box(
    element_id: &str,
    box_model: crate::dom::observer::ResizeObserverBox,
    callback: Box<dyn FnMut(crate::dom::observer::ResizeEntry) + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    let sub_id = crate::internal::next_subscription_id();
    let win =
        web_sys::window().ok_or_else(|| crate::error::Error::Unsupported("No window".into()))?;
    let doc = win
        .document()
        .ok_or_else(|| crate::error::Error::Unsupported("No document".into()))?;
    let target_el = doc
        .get_element_by_id(element_id)
        .ok_or_else(|| crate::error::Error::ElementNotFound(element_id.to_string()))?;

    let subscriber = Rc::new(ResizeSubscriber {
        target_id: element_id.to_string(),
        active: Cell::new(true),
        pending_entry: RefCell::new(None),
        frame_guard: RefCell::new(None),
        callback: RefCell::new(callback),
    });

    let target_key = RESIZE_POOLS.with(|cell| -> crate::error::Result<u64> {
        let mut pools = cell
            .try_borrow_mut()
            .map_err(|_| crate::error::Error::Unsupported("ResizeObserver pool busy".into()))?;
        let pool = pools.pool_mut(box_model);

        if pool.observer.is_none() {
            let js_cb = Closure::<dyn Fn(js_sys::Array)>::new(move |entries: js_sys::Array| {
                dispatch_resize_entries(box_model, entries);
            });
            let observer = web_sys::ResizeObserver::new(js_cb.as_ref().unchecked_ref())
                .map_err(|e| crate::error::Error::Unsupported(format!("{:?}", e)))?;
            pool.observer = Some(observer);
            pool.js_cb = Some(js_cb);
        }

        let options = pool.observer_options();
        let el_js: &wasm_bindgen::JsValue = target_el.as_ref();
        let existing_key = pool.element_keys.get(el_js).as_f64().map(|k| k as u64);

        if let Some(key) = existing_key
            && let Some(pooled) = pool.targets.get_mut(&key)
        {
            pooled.subscribers.push((sub_id, Rc::clone(&subscriber)));
            if let Some(observer) = pool.observer.as_ref() {
                observer.unobserve(&target_el);
                observer.observe_with_options(&target_el, &options);
            }
            Ok(key)
        } else {
            let key = pool.next_target_key;
            pool.next_target_key = pool.next_target_key.wrapping_add(1);
            pool.element_keys
                .set(el_js, &wasm_bindgen::JsValue::from_f64(key as f64));
            pool.targets.insert(
                key,
                PooledTarget {
                    element: target_el.clone(),
                    subscribers: vec![(sub_id, Rc::clone(&subscriber))],
                },
            );
            if let Some(observer) = pool.observer.as_ref() {
                observer.observe_with_options(&target_el, &options);
            }
            Ok(key)
        }
    })?;

    Ok(crate::watcher_guard::WatcherGuard::with_cleanup(
        "web_resize_observer",
        sub_id,
        None,
        move || {
            subscriber.active.set(false);
            if let Ok(mut pending) = subscriber.pending_entry.try_borrow_mut() {
                *pending = None;
            }
            if let Ok(mut fg) = subscriber.frame_guard.try_borrow_mut() {
                *fg = None;
            }
            unsubscribe_resize_pool(box_model, target_key, sub_id);
        },
    ))
}

#[allow(dead_code)]
pub(crate) fn observe_element_resize(
    element_id: &str,
    callback: Box<dyn FnMut(crate::dom::observer::ResizeEntry) + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    observe_element_resize_with_box(
        element_id,
        crate::dom::observer::ResizeObserverBox::BorderBox,
        callback,
    )
}

pub(crate) fn observe_capture_events_with_mask(
    mask: crate::dom::observer::CaptureEventMask,
    callback: Box<dyn FnMut(crate::dom::observer::CaptureEvent) + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    use crate::dom::observer::CaptureEventMask;
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    if mask.is_empty() {
        return Ok(crate::watcher_guard::WatcherGuard::new(
            "web_capture_observer_noop",
            0,
            None,
        ));
    }

    let sub_id = crate::internal::next_subscription_id();
    let win =
        web_sys::window().ok_or_else(|| crate::error::Error::Unsupported("No window".into()))?;
    let doc = win
        .document()
        .ok_or_else(|| crate::error::Error::Unsupported("No document".into()))?;

    let read_path_ids = |event: &web_sys::Event| -> Vec<String> {
        let path = event.composed_path();
        let mut ids = Vec::new();
        for node in path.iter() {
            if let Ok(el) = node.dyn_into::<web_sys::Element>() {
                let id = el.id();
                if !id.is_empty() {
                    ids.push(id);
                }
            }
        }
        ids
    };

    let cb_cell = std::rc::Rc::new(std::cell::RefCell::new(callback));

    let capture_opts = web_sys::AddEventListenerOptions::new();
    capture_opts.set_capture(true);
    let doc_target: web_sys::EventTarget = doc.clone().into();

    let pointer_handler = if mask.contains(CaptureEventMask::POINTER_DOWN) {
        let cb_pointer = cb_cell.clone();
        let handler =
            Closure::<dyn Fn(web_sys::PointerEvent)>::new(move |event: web_sys::PointerEvent| {
                let path_ids = read_path_ids(&event);
                let target_id = path_ids.first().cloned();
                if let Ok(mut cb) = cb_pointer.try_borrow_mut() {
                    cb(crate::dom::observer::CaptureEvent {
                        kind: crate::dom::observer::CaptureEventKind::PointerDown {
                            pointer_type: event.pointer_type(),
                            client_x: f64::from(event.client_x()),
                            client_y: f64::from(event.client_y()),
                        },
                        target_id,
                        path_ids,
                    });
                }
            });
        let _ = doc_target.add_event_listener_with_callback_and_add_event_listener_options(
            "pointerdown",
            handler.as_ref().unchecked_ref(),
            &capture_opts,
        );
        Some(handler)
    } else {
        None
    };

    let click_handler = if mask.contains(CaptureEventMask::CLICK) {
        let cb_click = cb_cell.clone();
        let handler =
            Closure::<dyn Fn(web_sys::MouseEvent)>::new(move |event: web_sys::MouseEvent| {
                let path_ids = read_path_ids(&event);
                let target_id = path_ids.first().cloned();
                if let Ok(mut cb) = cb_click.try_borrow_mut() {
                    cb(crate::dom::observer::CaptureEvent {
                        kind: crate::dom::observer::CaptureEventKind::Click {
                            is_trusted: event.is_trusted(),
                            detail: event.detail(),
                            client_x: f64::from(event.client_x()),
                            client_y: f64::from(event.client_y()),
                        },
                        target_id,
                        path_ids,
                    });
                }
            });
        let _ = doc_target.add_event_listener_with_callback_and_add_event_listener_options(
            "click",
            handler.as_ref().unchecked_ref(),
            &capture_opts,
        );
        Some(handler)
    } else {
        None
    };

    let focus_handler = if mask.contains(CaptureEventMask::FOCUS_IN) {
        let cb_focus = cb_cell.clone();
        let handler =
            Closure::<dyn Fn(web_sys::FocusEvent)>::new(move |event: web_sys::FocusEvent| {
                let path_ids = read_path_ids(&event);
                let target_id = path_ids.first().cloned();
                if let Ok(mut cb) = cb_focus.try_borrow_mut() {
                    cb(crate::dom::observer::CaptureEvent {
                        kind: crate::dom::observer::CaptureEventKind::FocusIn,
                        target_id,
                        path_ids,
                    });
                }
            });
        let _ = doc_target.add_event_listener_with_callback_and_add_event_listener_options(
            "focusin",
            handler.as_ref().unchecked_ref(),
            &capture_opts,
        );
        Some(handler)
    } else {
        None
    };

    let key_handler = if mask.contains(CaptureEventMask::KEY_DOWN) {
        let cb_key = cb_cell.clone();
        let handler =
            Closure::<dyn Fn(web_sys::KeyboardEvent)>::new(move |event: web_sys::KeyboardEvent| {
                let key = event.key();
                let is_composing =
                    event.is_composing() || event.key_code() == 229 || key == "Process";
                let path_ids = read_path_ids(&event);
                let target_id = path_ids.first().cloned();
                if let Ok(mut cb) = cb_key.try_borrow_mut() {
                    cb(crate::dom::observer::CaptureEvent {
                        kind: crate::dom::observer::CaptureEventKind::KeyDown { key, is_composing },
                        target_id,
                        path_ids,
                    });
                }
            });
        let _ = doc_target.add_event_listener_with_callback_and_add_event_listener_options(
            "keydown",
            handler.as_ref().unchecked_ref(),
            &capture_opts,
        );
        Some(handler)
    } else {
        None
    };

    let doc_cleanup = doc_target.clone();
    Ok(crate::watcher_guard::WatcherGuard::with_cleanup(
        "web_capture_observer",
        sub_id,
        None,
        move || {
            if let Some(pointer_handler) = pointer_handler {
                let _ = doc_cleanup.remove_event_listener_with_callback_and_bool(
                    "pointerdown",
                    pointer_handler.as_ref().unchecked_ref(),
                    true,
                );
            }
            if let Some(click_handler) = click_handler {
                let _ = doc_cleanup.remove_event_listener_with_callback_and_bool(
                    "click",
                    click_handler.as_ref().unchecked_ref(),
                    true,
                );
            }
            if let Some(focus_handler) = focus_handler {
                let _ = doc_cleanup.remove_event_listener_with_callback_and_bool(
                    "focusin",
                    focus_handler.as_ref().unchecked_ref(),
                    true,
                );
            }
            if let Some(key_handler) = key_handler {
                let _ = doc_cleanup.remove_event_listener_with_callback_and_bool(
                    "keydown",
                    key_handler.as_ref().unchecked_ref(),
                    true,
                );
            }
        },
    ))
}

pub(crate) fn observe_transition_lifecycle(
    element_id: &str,
    callback: Box<dyn FnMut(crate::dom::observer::TransitionLifecycleEvent) + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    let sub_id = crate::internal::next_subscription_id();
    let win =
        web_sys::window().ok_or_else(|| crate::error::Error::Unsupported("No window".into()))?;
    let doc = win
        .document()
        .ok_or_else(|| crate::error::Error::Unsupported("No document".into()))?;

    type LifecycleCallback =
        Box<dyn FnMut(crate::dom::observer::TransitionLifecycleEvent) + 'static>;
    let cb_cell = std::rc::Rc::new(std::cell::RefCell::new(Some(callback)));
    let raf_cell = std::rc::Rc::new(std::cell::RefCell::new(None));
    let target_el = doc.get_element_by_id(element_id);

    let mut anim_end_handler = None;
    let mut anim_cancel_handler = None;
    let mut trans_end_handler = None;
    let mut trans_cancel_handler = None;

    if let Some(ref el) = target_el {
        let cb_end = cb_cell.clone();
        let raf_end = raf_cell.clone();
        let listener_target = el.clone();
        let on_end = Closure::<dyn Fn(web_sys::AnimationEvent)>::new(
            move |event: web_sys::AnimationEvent| {
                if event
                    .target()
                    .is_none_or(|target| target != listener_target.clone().into())
                {
                    return;
                }
                let cb: Option<LifecycleCallback> = cb_end
                    .try_borrow_mut()
                    .ok()
                    .and_then(|mut c| c.take());
                if let Some(mut cb) = cb {
                    let _ = raf_end.try_borrow_mut().ok().and_then(|mut c| c.take());
                    cb(
                        crate::dom::observer::TransitionLifecycleEvent::AnimationEnd {
                            animation_name: event.animation_name(),
                        },
                    );
                }
            },
        );
        let target: web_sys::EventTarget = el.clone().into();
        let _ = target
            .add_event_listener_with_callback("animationend", on_end.as_ref().unchecked_ref());
        anim_end_handler = Some((target.clone(), on_end));

        let cb_cancel = cb_cell.clone();
        let raf_cancel = raf_cell.clone();
        let listener_target = el.clone();
        let on_cancel = Closure::<dyn Fn(web_sys::AnimationEvent)>::new(
            move |event: web_sys::AnimationEvent| {
                if event
                    .target()
                    .is_none_or(|target| target != listener_target.clone().into())
                {
                    return;
                }
                if element_has_running_animations(&listener_target) == Some(true) {
                    return;
                }
                let cb: Option<LifecycleCallback> = cb_cancel
                    .try_borrow_mut()
                    .ok()
                    .and_then(|mut c| c.take());
                if let Some(mut cb) = cb {
                    let _ = raf_cancel.try_borrow_mut().ok().and_then(|mut c| c.take());
                    cb(
                        crate::dom::observer::TransitionLifecycleEvent::AnimationCancel {
                            animation_name: event.animation_name(),
                        },
                    );
                }
            },
        );
        let _ = target.add_event_listener_with_callback(
            "animationcancel",
            on_cancel.as_ref().unchecked_ref(),
        );
        anim_cancel_handler = Some((target.clone(), on_cancel));

        let cb_trans_end = cb_cell.clone();
        let raf_trans_end = raf_cell.clone();
        let listener_target = el.clone();
        let on_trans_end = Closure::<dyn Fn(web_sys::TransitionEvent)>::new(
            move |event: web_sys::TransitionEvent| {
                if event
                    .target()
                    .is_none_or(|target| target != listener_target.clone().into())
                {
                    return;
                }
                let cb: Option<LifecycleCallback> = cb_trans_end
                    .try_borrow_mut()
                    .ok()
                    .and_then(|mut c| c.take());
                if let Some(mut cb) = cb {
                    let _ = raf_trans_end
                        .try_borrow_mut()
                        .ok()
                        .and_then(|mut c| c.take());
                    cb(
                        crate::dom::observer::TransitionLifecycleEvent::TransitionEnd {
                            property_name: event.property_name(),
                        },
                    );
                }
            },
        );
        let _ = target.add_event_listener_with_callback(
            "transitionend",
            on_trans_end.as_ref().unchecked_ref(),
        );
        trans_end_handler = Some((target.clone(), on_trans_end));

        let cb_trans_cancel = cb_cell.clone();
        let raf_trans_cancel = raf_cell.clone();
        let listener_target = el.clone();
        let on_trans_cancel = Closure::<dyn Fn(web_sys::TransitionEvent)>::new(
            move |event: web_sys::TransitionEvent| {
                if event
                    .target()
                    .is_none_or(|target| target != listener_target.clone().into())
                {
                    return;
                }
                if element_has_running_animations(&listener_target) == Some(true) {
                    return;
                }
                let cb: Option<LifecycleCallback> = cb_trans_cancel
                    .try_borrow_mut()
                    .ok()
                    .and_then(|mut c| c.take());
                if let Some(mut cb) = cb {
                    let _ = raf_trans_cancel
                        .try_borrow_mut()
                        .ok()
                        .and_then(|mut c| c.take());
                    cb(
                        crate::dom::observer::TransitionLifecycleEvent::TransitionCancel {
                            property_name: event.property_name(),
                        },
                    );
                }
            },
        );
        let _ = target.add_event_listener_with_callback(
            "transitioncancel",
            on_trans_cancel.as_ref().unchecked_ref(),
        );
        trans_cancel_handler = Some((target, on_trans_cancel));
    }

    let element_id = element_id.to_string();
    let cb_frame = cb_cell.clone();
    let raf_frame = raf_cell.clone();
    let doc_frame = doc.clone();
    let frame_guard = crate::frame::request_next_frame(move |_| {
        let event = if doc_frame.hidden()
            || doc_frame.visibility_state() == web_sys::VisibilityState::Hidden
        {
            Some(crate::dom::observer::TransitionLifecycleEvent::Hidden)
        } else {
            let Some(element) = doc_frame.get_element_by_id(&element_id) else {
                let cb: Option<LifecycleCallback> = cb_frame
                    .try_borrow_mut()
                    .ok()
                    .and_then(|mut c| c.take());
                if let Some(mut cb) = cb {
                    cb(crate::dom::observer::TransitionLifecycleEvent::Missing);
                }
                let _ = raf_frame.try_borrow_mut().ok().and_then(|mut c| c.take());
                return;
            };
            let Some(window) = web_sys::window() else {
                return;
            };
            let mut current = Some(element);
            let mut hidden = false;
            let mut active_animation = false;
            let mut is_target = true;
            while let Some(node) = current.take() {
                let Ok(style) = window.get_computed_style(&node) else {
                    return;
                };
                let Some(style) = style else {
                    return;
                };
                if style
                    .get_property_value("display")
                    .is_ok_and(|value| value.trim() == "none")
                {
                    hidden = true;
                    break;
                }
                if is_target
                    && element_has_running_animations(&node)
                        .unwrap_or_else(|| style_has_active_motion(&style))
                {
                    active_animation = true;
                }
                current = node.parent_element();
                is_target = false;
            }
            if hidden {
                Some(crate::dom::observer::TransitionLifecycleEvent::Hidden)
            } else if active_animation {
                None
            } else {
                Some(crate::dom::observer::TransitionLifecycleEvent::NoAnimation)
            }
        };
        if let Some(event) = event {
            let cb: Option<LifecycleCallback> = cb_frame
                .try_borrow_mut()
                .ok()
                .and_then(|mut c| c.take());
            if let Some(mut cb) = cb {
                cb(event);
            }
        }
        let _ = raf_frame.try_borrow_mut().ok().and_then(|mut c| c.take());
    })
    .map_err(|_| crate::error::Error::Unsupported("Animation frame unavailable".into()))?;
    raf_cell.replace(Some(frame_guard));

    Ok(crate::watcher_guard::WatcherGuard::with_cleanup(
        "web_transition_lifecycle",
        sub_id,
        None,
        move || {
            if let Some((target, handler)) = anim_end_handler {
                let _ = target.remove_event_listener_with_callback(
                    "animationend",
                    handler.as_ref().unchecked_ref(),
                );
            }
            if let Some((target, handler)) = anim_cancel_handler {
                let _ = target.remove_event_listener_with_callback(
                    "animationcancel",
                    handler.as_ref().unchecked_ref(),
                );
            }
            if let Some((target, handler)) = trans_end_handler {
                let _ = target.remove_event_listener_with_callback(
                    "transitionend",
                    handler.as_ref().unchecked_ref(),
                );
            }
            if let Some((target, handler)) = trans_cancel_handler {
                let _ = target.remove_event_listener_with_callback(
                    "transitioncancel",
                    handler.as_ref().unchecked_ref(),
                );
            }
            if let Ok(mut cell) = cb_cell.try_borrow_mut() {
                cell.take();
            }
            if let Ok(mut cell) = raf_cell.try_borrow_mut() {
                cell.take();
            }
        },
    ))
}

#[cfg(target_arch = "wasm32")]
fn element_has_running_animations(el: &web_sys::Element) -> Option<bool> {
    use wasm_bindgen::{JsCast, JsValue};

    let el_js: &JsValue = el.as_ref();
    let get_anims = js_sys::Reflect::get(el_js, &JsValue::from_str("getAnimations")).ok()?;
    let func = get_anims.dyn_into::<js_sys::Function>().ok()?;
    let arr = func.call0(el_js).ok()?.dyn_into::<js_sys::Array>().ok()?;
    for idx in 0..arr.length() {
        let anim = arr.get(idx);
        if let Ok(state) = js_sys::Reflect::get(&anim, &JsValue::from_str("playState"))
            && let Some(state_str) = state.as_string()
            && (state_str == "running" || state_str == "pending")
        {
            return Some(true);
        }
    }
    Some(false)
}

#[cfg(target_arch = "wasm32")]
fn style_has_active_motion(style: &web_sys::CssStyleDeclaration) -> bool {
    fn positive_duration(value: String) -> bool {
        value.split(',').any(|part| {
            let part = part.trim();
            if let Some(seconds) = part.strip_suffix("ms") {
                seconds
                    .trim()
                    .parse::<f64>()
                    .is_ok_and(|duration| duration > 0.0)
            } else if let Some(seconds) = part.strip_suffix('s') {
                seconds
                    .trim()
                    .parse::<f64>()
                    .is_ok_and(|duration| duration > 0.0)
            } else {
                false
            }
        })
    }

    let animation_name = style
        .get_property_value("animation-name")
        .unwrap_or_default();
    let transition_property = style
        .get_property_value("transition-property")
        .unwrap_or_default();
    (animation_name.trim() != "none"
        && style
            .get_property_value("animation-duration")
            .is_ok_and(positive_duration))
        || (transition_property.trim() != "none"
            && style
                .get_property_value("transition-duration")
                .is_ok_and(positive_duration))
}

pub(crate) fn observe_form_reset(
    element_id: &str,
    form_id: Option<&str>,
    callback: Box<dyn FnMut() + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    let win =
        web_sys::window().ok_or_else(|| crate::error::Error::Unsupported("No window".into()))?;
    let doc = win
        .document()
        .ok_or_else(|| crate::error::Error::Unsupported("No document".into()))?;

    let form_el = if let Some(fid) = form_id {
        doc.get_element_by_id(fid)
            .and_then(|el| el.dyn_into::<web_sys::HtmlFormElement>().ok())
    } else if let Some(el) = doc.get_element_by_id(element_id) {
        let mut curr = el.parent_element();
        let mut found = None;
        while let Some(parent) = curr {
            if let Ok(form) = parent.clone().dyn_into::<web_sys::HtmlFormElement>() {
                found = Some(form);
                break;
            }
            curr = parent.parent_element();
        }
        found
    } else {
        None
    };

    let Some(form) = form_el else {
        return Ok(crate::watcher_guard::WatcherGuard::new(
            "web_form_reset_noop",
            0,
            None,
        ));
    };

    let sub_id = crate::internal::next_subscription_id();
    let cb_cell = std::rc::Rc::new(std::cell::RefCell::new(callback));
    let handler = Closure::<dyn Fn(web_sys::Event)>::new(move |_event: web_sys::Event| {
        if let Ok(mut cb) = cb_cell.try_borrow_mut() {
            cb();
        }
    });

    let target: web_sys::EventTarget = form.clone().into();
    let _ = target.add_event_listener_with_callback("reset", handler.as_ref().unchecked_ref());

    Ok(crate::watcher_guard::WatcherGuard::with_cleanup(
        "web_form_reset_observer",
        sub_id,
        None,
        move || {
            let _ = target
                .remove_event_listener_with_callback("reset", handler.as_ref().unchecked_ref());
        },
    ))
}

pub(crate) fn observe_pointer_move(
    callback: Box<dyn FnMut(f64, f64) + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    let sub_id = crate::internal::next_subscription_id();
    let win =
        web_sys::window().ok_or_else(|| crate::error::Error::Unsupported("No window".into()))?;
    let doc = win
        .document()
        .ok_or_else(|| crate::error::Error::Unsupported("No document".into()))?;

    let cb_cell = std::rc::Rc::new(std::cell::RefCell::new(callback));
    let handler =
        Closure::<dyn Fn(web_sys::PointerEvent)>::new(move |event: web_sys::PointerEvent| {
            if let Ok(mut cb) = cb_cell.try_borrow_mut() {
                cb(f64::from(event.client_x()), f64::from(event.client_y()));
            }
        });

    let opts = web_sys::AddEventListenerOptions::new();
    opts.set_passive(true);

    let doc_target: web_sys::EventTarget = doc.into();
    let _ = doc_target.add_event_listener_with_callback_and_add_event_listener_options(
        "pointermove",
        handler.as_ref().unchecked_ref(),
        &opts,
    );

    Ok(crate::watcher_guard::WatcherGuard::with_cleanup(
        "web_pointer_move_observer",
        sub_id,
        None,
        move || {
            let _ = doc_target.remove_event_listener_with_callback(
                "pointermove",
                handler.as_ref().unchecked_ref(),
            );
        },
    ))
}

pub(crate) fn observe_pointer_up(
    callback: Box<dyn FnMut() + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    let sub_id = crate::internal::next_subscription_id();
    let win =
        web_sys::window().ok_or_else(|| crate::error::Error::Unsupported("No window".into()))?;
    let doc = win
        .document()
        .ok_or_else(|| crate::error::Error::Unsupported("No document".into()))?;

    let cb_cell = std::rc::Rc::new(std::cell::RefCell::new(callback));
    let handler =
        Closure::<dyn Fn(web_sys::PointerEvent)>::new(move |_event: web_sys::PointerEvent| {
            if let Ok(mut cb) = cb_cell.try_borrow_mut() {
                cb();
            }
        });

    let opts = web_sys::AddEventListenerOptions::new();
    opts.set_passive(true);

    let doc_target: web_sys::EventTarget = doc.into();
    let _ = doc_target.add_event_listener_with_callback_and_add_event_listener_options(
        "pointerup",
        handler.as_ref().unchecked_ref(),
        &opts,
    );

    Ok(crate::watcher_guard::WatcherGuard::with_cleanup(
        "web_pointer_up_observer",
        sub_id,
        None,
        move || {
            let _ = doc_target.remove_event_listener_with_callback(
                "pointerup",
                handler.as_ref().unchecked_ref(),
            );
        },
    ))
}

pub(crate) fn observe_window_blur(
    callback: Box<dyn FnMut() + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    let sub_id = crate::internal::next_subscription_id();
    let win =
        web_sys::window().ok_or_else(|| crate::error::Error::Unsupported("No window".into()))?;

    let cb_cell = std::rc::Rc::new(std::cell::RefCell::new(callback));
    let handler = Closure::<dyn Fn(web_sys::Event)>::new(move |_event: web_sys::Event| {
        if let Ok(mut cb) = cb_cell.try_borrow_mut() {
            cb();
        }
    });

    let win_target: web_sys::EventTarget = win.into();
    let _ = win_target.add_event_listener_with_callback("blur", handler.as_ref().unchecked_ref());

    Ok(crate::watcher_guard::WatcherGuard::with_cleanup(
        "web_window_blur_observer",
        sub_id,
        None,
        move || {
            let _ = win_target
                .remove_event_listener_with_callback("blur", handler.as_ref().unchecked_ref());
        },
    ))
}

pub(crate) fn observe_scroll(
    callback: Box<dyn FnMut() + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    let sub_id = crate::internal::next_subscription_id();
    let win =
        web_sys::window().ok_or_else(|| crate::error::Error::Unsupported("No window".into()))?;

    let cb_cell = std::rc::Rc::new(std::cell::RefCell::new(callback));
    let handler = Closure::<dyn Fn(web_sys::Event)>::new(move |_event: web_sys::Event| {
        if let Ok(mut cb) = cb_cell.try_borrow_mut() {
            cb();
        }
    });

    let capture_opts = web_sys::AddEventListenerOptions::new();
    capture_opts.set_capture(true);
    capture_opts.set_passive(true);

    let win_target: web_sys::EventTarget = win.clone().into();
    let _ = win_target.add_event_listener_with_callback_and_add_event_listener_options(
        "scroll",
        handler.as_ref().unchecked_ref(),
        &capture_opts,
    );

    Ok(crate::watcher_guard::WatcherGuard::with_cleanup(
        "web_scroll_observer",
        sub_id,
        None,
        move || {
            let _ = win_target.remove_event_listener_with_callback_and_bool(
                "scroll",
                handler.as_ref().unchecked_ref(),
                true,
            );
        },
    ))
}

pub(crate) fn observe_viewport_scroll(
    callback: Box<dyn FnMut() + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    let sub_id = crate::internal::next_subscription_id();
    let win =
        web_sys::window().ok_or_else(|| crate::error::Error::Unsupported("No window".into()))?;

    let cb_cell = std::rc::Rc::new(std::cell::RefCell::new(callback));
    let handler = Closure::<dyn Fn(web_sys::Event)>::new(move |_event: web_sys::Event| {
        if let Ok(mut cb) = cb_cell.try_borrow_mut() {
            cb();
        }
    });

    let opts = web_sys::AddEventListenerOptions::new();
    opts.set_capture(false);
    opts.set_passive(true);

    let win_target: web_sys::EventTarget = win.clone().into();
    let _ = win_target.add_event_listener_with_callback_and_add_event_listener_options(
        "scroll",
        handler.as_ref().unchecked_ref(),
        &opts,
    );

    Ok(crate::watcher_guard::WatcherGuard::with_cleanup(
        "web_viewport_scroll_observer",
        sub_id,
        None,
        move || {
            let _ = win_target.remove_event_listener_with_callback_and_bool(
                "scroll",
                handler.as_ref().unchecked_ref(),
                false,
            );
        },
    ))
}

pub(crate) fn observe_element_scroll(
    element_id: &str,
    callback: Box<dyn FnMut() + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    let sub_id = crate::internal::next_subscription_id();
    let win =
        web_sys::window().ok_or_else(|| crate::error::Error::Unsupported("No window".into()))?;
    let doc = win
        .document()
        .ok_or_else(|| crate::error::Error::Unsupported("No document".into()))?;
    let target_el = doc
        .get_element_by_id(element_id)
        .ok_or_else(|| crate::error::Error::ElementNotFound(element_id.to_string()))?;

    let cb_cell = std::rc::Rc::new(std::cell::RefCell::new(callback));
    let handler = Closure::<dyn Fn(web_sys::Event)>::new(move |_event: web_sys::Event| {
        if let Ok(mut cb) = cb_cell.try_borrow_mut() {
            cb();
        }
    });

    let opts = web_sys::AddEventListenerOptions::new();
    opts.set_passive(true);

    let target: web_sys::EventTarget = target_el.clone().into();
    let _ = target.add_event_listener_with_callback_and_add_event_listener_options(
        "scroll",
        handler.as_ref().unchecked_ref(),
        &opts,
    );

    Ok(crate::watcher_guard::WatcherGuard::with_cleanup(
        "web_element_scroll_observer",
        sub_id,
        None,
        move || {
            let _ = target
                .remove_event_listener_with_callback("scroll", handler.as_ref().unchecked_ref());
        },
    ))
}

pub(crate) fn observe_visibility_change(
    callback: Box<dyn FnMut(crate::dom::observer::DocumentVisibility) + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    let sub_id = crate::internal::next_subscription_id();
    let win =
        web_sys::window().ok_or_else(|| crate::error::Error::Unsupported("No window".into()))?;
    let doc = win
        .document()
        .ok_or_else(|| crate::error::Error::Unsupported("No document".into()))?;

    let initial_hidden = doc.hidden() || doc.visibility_state() == web_sys::VisibilityState::Hidden;
    let last_hidden = std::cell::Cell::new(initial_hidden);
    let doc_for_cb = doc.clone();
    let cb_cell = std::rc::Rc::new(std::cell::RefCell::new(callback));

    let handler = Closure::<dyn Fn(web_sys::Event)>::new(move |_event: web_sys::Event| {
        let hidden = doc_for_cb.hidden()
            || doc_for_cb.visibility_state() == web_sys::VisibilityState::Hidden;
        if last_hidden.replace(hidden) != hidden {
            let vis = if hidden {
                crate::dom::observer::DocumentVisibility::Hidden
            } else {
                crate::dom::observer::DocumentVisibility::Visible
            };
            if let Ok(mut cb) = cb_cell.try_borrow_mut() {
                cb(vis);
            }
        }
    });

    let doc_target: web_sys::EventTarget = doc.into();
    let _ = doc_target
        .add_event_listener_with_callback("visibilitychange", handler.as_ref().unchecked_ref());

    Ok(crate::watcher_guard::WatcherGuard::with_cleanup(
        "web_visibility_observer",
        sub_id,
        None,
        move || {
            let _ = doc_target.remove_event_listener_with_callback(
                "visibilitychange",
                handler.as_ref().unchecked_ref(),
            );
        },
    ))
}

pub(crate) fn watch_image_load(
    src: &str,
    options: crate::dom::observer::ImageLoadOptions,
    callback: Box<dyn FnMut(crate::dom::observer::ImageLoadStatus) + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    let doc = web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| crate::error::HostError::Unsupported("No browser document".into()))?;
    let element = doc.create_element("img").map_err(|_| {
        crate::error::HostError::Unsupported("Unable to create image preloader".into())
    })?;
    let image = element
        .dyn_into::<web_sys::HtmlImageElement>()
        .map_err(|_| {
            crate::error::HostError::Unsupported("Created element is not an image".into())
        })?;
    let target: web_sys::EventTarget = image.clone().into();

    let callback = std::rc::Rc::new(std::cell::RefCell::new(Some(callback)));
    let completed = std::rc::Rc::new(std::cell::Cell::new(false));
    let finish = std::rc::Rc::new({
        let callback = callback.clone();
        let completed = completed.clone();
        move |status| {
            if completed.replace(true) {
                return;
            }
            if let Some(mut callback) = callback
                .try_borrow_mut()
                .ok()
                .and_then(|mut c| c.take())
            {
                callback(status);
            }
        }
    });
    let load_finish = finish.clone();
    let load_handler = Closure::<dyn Fn(web_sys::Event)>::new(move |_| {
        load_finish(crate::dom::observer::ImageLoadStatus::Loaded);
    });
    let error_finish = finish.clone();
    let error_handler = Closure::<dyn Fn(web_sys::Event)>::new(move |_| {
        error_finish(crate::dom::observer::ImageLoadStatus::Error);
    });
    target
        .add_event_listener_with_callback("load", load_handler.as_ref().unchecked_ref())
        .map_err(|_| crate::error::HostError::Unsupported("Unable to observe image load".into()))?;
    target
        .add_event_listener_with_callback("error", error_handler.as_ref().unchecked_ref())
        .map_err(|_| {
            crate::error::HostError::Unsupported("Unable to observe image error".into())
        })?;

    if let Some(crossorigin) = options.crossorigin.as_deref() {
        image.set_cross_origin(Some(crossorigin));
    }
    if let Some(referrerpolicy) = options.referrerpolicy.as_deref() {
        image.set_referrer_policy(referrerpolicy);
    }
    if let Ok(mut callback) = callback.try_borrow_mut()
        && let Some(callback) = callback.as_mut()
    {
        callback(crate::dom::observer::ImageLoadStatus::Loading);
    }
    image.set_src(src);
    if image.complete() {
        finish(if image.natural_width() > 0 {
            crate::dom::observer::ImageLoadStatus::Loaded
        } else {
            crate::dom::observer::ImageLoadStatus::Error
        });
    }

    let callback_cleanup = callback;
    let sub_id = crate::internal::next_subscription_id();
    Ok(crate::watcher_guard::WatcherGuard::with_cleanup(
        "web_image_load_observer",
        sub_id,
        None,
        move || {
            let _ = target
                .remove_event_listener_with_callback("load", load_handler.as_ref().unchecked_ref());
            let _ = target.remove_event_listener_with_callback(
                "error",
                error_handler.as_ref().unchecked_ref(),
            );
            if let Ok(mut callback) = callback_cleanup.try_borrow_mut() {
                callback.take();
            }
            image.set_src("");
        },
    ))
}
