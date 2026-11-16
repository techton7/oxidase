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
            let w = win.inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(0.0);
            let h = win.inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(0.0);
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
    pub fn measure_unscaled_rect(&self, element_id: &str) -> Option<crate::runtime::geometry::Rect> {
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
        let doc = self
            .doc
            .as_ref()
            .ok_or_else(|| crate::error::HostError::Unsupported("No active browser document".into()))?;
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
}

pub(crate) fn observe_element_resize(
    element_id: &str,
    mut callback: Box<dyn FnMut(crate::dom::observer::ResizeEntry) + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;

    let sub_id = crate::internal::next_subscription_id();
    let win = web_sys::window().ok_or_else(|| crate::error::Error::Unsupported("No window".into()))?;
    let doc = win.document().ok_or_else(|| crate::error::Error::Unsupported("No document".into()))?;
    let target_el = doc.get_element_by_id(element_id).ok_or_else(|| crate::error::Error::ElementNotFound(element_id.to_string()))?;

    let tid = element_id.to_string();
    let js_cb = Closure::<dyn FnMut(js_sys::Array)>::new(move |entries: js_sys::Array| {
        for entry in entries.iter() {
            if let Ok(obs_entry) = entry.dyn_into::<web_sys::ResizeObserverEntry>() {
                let rect = obs_entry.content_rect();
                callback(crate::dom::observer::ResizeEntry {
                    target_id: tid.clone(),
                    content_rect: crate::runtime::geometry::Rect::new(rect.x(), rect.y(), rect.width(), rect.height()),
                });
            }
        }
    });

    let observer = web_sys::ResizeObserver::new(js_cb.as_ref().unchecked_ref())
        .map_err(|e| crate::error::Error::Unsupported(format!("{:?}", e)))?;

    observer.observe(&target_el);

    Ok(crate::watcher_guard::WatcherGuard::with_cleanup(
        "web_resize_observer",
        sub_id,
        None,
        move || {
            observer.disconnect();
            drop(js_cb);
        },
    ))
}

pub(crate) fn observe_capture_events(
    callback: Box<dyn FnMut(crate::dom::observer::CaptureEvent) + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;

    let sub_id = crate::internal::next_subscription_id();
    let win = web_sys::window().ok_or_else(|| crate::error::Error::Unsupported("No window".into()))?;
    let doc = win.document().ok_or_else(|| crate::error::Error::Unsupported("No document".into()))?;

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

    let cb_pointer = cb_cell.clone();
    let pointer_handler = Closure::<dyn FnMut(web_sys::PointerEvent)>::new(move |event: web_sys::PointerEvent| {
        let path_ids = read_path_ids(&event);
        let target_id = path_ids.first().cloned();
        cb_pointer.borrow_mut()(crate::dom::observer::CaptureEvent {
            kind: crate::dom::observer::CaptureEventKind::PointerDown,
            target_id,
            path_ids,
        });
    });

    let cb_focus = cb_cell.clone();
    let focus_handler = Closure::<dyn FnMut(web_sys::FocusEvent)>::new(move |event: web_sys::FocusEvent| {
        let path_ids = read_path_ids(&event);
        let target_id = path_ids.first().cloned();
        cb_focus.borrow_mut()(crate::dom::observer::CaptureEvent {
            kind: crate::dom::observer::CaptureEventKind::FocusIn,
            target_id,
            path_ids,
        });
    });

    let cb_key = cb_cell.clone();
    let key_handler = Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(move |event: web_sys::KeyboardEvent| {
        let key = event.key();
        let path_ids = read_path_ids(&event);
        let target_id = path_ids.first().cloned();
        cb_key.borrow_mut()(crate::dom::observer::CaptureEvent {
            kind: crate::dom::observer::CaptureEventKind::KeyDown { key },
            target_id,
            path_ids,
        });
    });

    let capture_opts = web_sys::AddEventListenerOptions::new();
    capture_opts.set_capture(true);

    let doc_target: web_sys::EventTarget = doc.clone().into();
    let _ = doc_target.add_event_listener_with_callback_and_add_event_listener_options(
        "pointerdown",
        pointer_handler.as_ref().unchecked_ref(),
        &capture_opts,
    );
    let _ = doc_target.add_event_listener_with_callback_and_add_event_listener_options(
        "focusin",
        focus_handler.as_ref().unchecked_ref(),
        &capture_opts,
    );
    let _ = doc_target.add_event_listener_with_callback_and_add_event_listener_options(
        "keydown",
        key_handler.as_ref().unchecked_ref(),
        &capture_opts,
    );

    let doc_cleanup = doc_target.clone();
    Ok(crate::watcher_guard::WatcherGuard::with_cleanup(
        "web_capture_observer",
        sub_id,
        None,
        move || {
            let _ = doc_cleanup.remove_event_listener_with_callback_and_bool(
                "pointerdown",
                pointer_handler.as_ref().unchecked_ref(),
                true,
            );
            let _ = doc_cleanup.remove_event_listener_with_callback_and_bool(
                "focusin",
                focus_handler.as_ref().unchecked_ref(),
                true,
            );
            let _ = doc_cleanup.remove_event_listener_with_callback_and_bool(
                "keydown",
                key_handler.as_ref().unchecked_ref(),
                true,
            );
        },
    ))
}

pub(crate) fn observe_transition_lifecycle(
    element_id: &str,
    fallback_timeout_ms: u64,
    callback: Box<dyn FnMut(crate::dom::observer::TransitionLifecycleEvent) + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;

    let sub_id = crate::internal::next_subscription_id();
    let win = web_sys::window().ok_or_else(|| crate::error::Error::Unsupported("No window".into()))?;
    let doc = win.document().ok_or_else(|| crate::error::Error::Unsupported("No document".into()))?;

    let cb_cell = std::rc::Rc::new(std::cell::RefCell::new(Some(callback)));
    let target_el = doc.get_element_by_id(element_id);

    let mut anim_end_handler = None;
    let mut anim_cancel_handler = None;
    let mut trans_end_handler = None;
    let mut trans_cancel_handler = None;

    if let Some(ref el) = target_el {
        let cb_end = cb_cell.clone();
        let on_end = Closure::<dyn FnMut(web_sys::AnimationEvent)>::new(move |event: web_sys::AnimationEvent| {
            let cb = cb_end.borrow_mut().take();
            if let Some(mut cb) = cb {
                cb(crate::dom::observer::TransitionLifecycleEvent::AnimationEnd {
                    animation_name: event.animation_name(),
                });
            }
        });
        let target: web_sys::EventTarget = el.clone().into();
        let _ = target.add_event_listener_with_callback("animationend", on_end.as_ref().unchecked_ref());
        anim_end_handler = Some((target.clone(), on_end));

        let cb_cancel = cb_cell.clone();
        let on_cancel = Closure::<dyn FnMut(web_sys::AnimationEvent)>::new(move |event: web_sys::AnimationEvent| {
            let cb = cb_cancel.borrow_mut().take();
            if let Some(mut cb) = cb {
                cb(crate::dom::observer::TransitionLifecycleEvent::AnimationCancel {
                    animation_name: event.animation_name(),
                });
            }
        });
        let _ = target.add_event_listener_with_callback("animationcancel", on_cancel.as_ref().unchecked_ref());
        anim_cancel_handler = Some((target.clone(), on_cancel));

        let cb_trans_end = cb_cell.clone();
        let on_trans_end = Closure::<dyn FnMut(web_sys::TransitionEvent)>::new(move |event: web_sys::TransitionEvent| {
            let cb = cb_trans_end.borrow_mut().take();
            if let Some(mut cb) = cb {
                cb(crate::dom::observer::TransitionLifecycleEvent::TransitionEnd {
                    property_name: event.property_name(),
                });
            }
        });
        let _ = target.add_event_listener_with_callback("transitionend", on_trans_end.as_ref().unchecked_ref());
        trans_end_handler = Some((target.clone(), on_trans_end));

        let cb_trans_cancel = cb_cell.clone();
        let on_trans_cancel = Closure::<dyn FnMut(web_sys::TransitionEvent)>::new(move |event: web_sys::TransitionEvent| {
            let cb = cb_trans_cancel.borrow_mut().take();
            if let Some(mut cb) = cb {
                cb(crate::dom::observer::TransitionLifecycleEvent::TransitionCancel {
                    property_name: event.property_name(),
                });
            }
        });
        let _ = target.add_event_listener_with_callback("transitioncancel", on_trans_cancel.as_ref().unchecked_ref());
        trans_cancel_handler = Some((target, on_trans_cancel));
    }

    let cb_timeout = cb_cell.clone();
    let task = if fallback_timeout_ms > 0 {
        crate::internal::safe_spawn(async move {
            futures_timer::Delay::new(std::time::Duration::from_millis(fallback_timeout_ms)).await;
            let cb = cb_timeout.borrow_mut().take();
            if let Some(mut cb) = cb {
                cb(crate::dom::observer::TransitionLifecycleEvent::TimeoutFallback);
            }
        })
    } else {
        None
    };

    Ok(crate::watcher_guard::WatcherGuard::with_cleanup(
        "web_transition_lifecycle",
        sub_id,
        task,
        move || {
            if let Some((target, handler)) = anim_end_handler {
                let _ = target.remove_event_listener_with_callback("animationend", handler.as_ref().unchecked_ref());
            }
            if let Some((target, handler)) = anim_cancel_handler {
                let _ = target.remove_event_listener_with_callback("animationcancel", handler.as_ref().unchecked_ref());
            }
            if let Some((target, handler)) = trans_end_handler {
                let _ = target.remove_event_listener_with_callback("transitionend", handler.as_ref().unchecked_ref());
            }
            if let Some((target, handler)) = trans_cancel_handler {
                let _ = target.remove_event_listener_with_callback("transitioncancel", handler.as_ref().unchecked_ref());
            }
            if let Ok(mut cell) = cb_cell.try_borrow_mut() {
                cell.take();
            }
        },
    ))
}

pub(crate) fn observe_form_reset(
    element_id: &str,
    form_id: Option<&str>,
    mut callback: Box<dyn FnMut() + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;

    let sub_id = crate::internal::next_subscription_id();
    let win = web_sys::window().ok_or_else(|| crate::error::Error::Unsupported("No window".into()))?;
    let doc = win.document().ok_or_else(|| crate::error::Error::Unsupported("No document".into()))?;

    let form_el = if let Some(fid) = form_id {
        doc.get_element_by_id(fid).and_then(|el| el.dyn_into::<web_sys::HtmlFormElement>().ok())
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
        return Ok(crate::watcher_guard::WatcherGuard::new("web_form_reset_noop", sub_id, None));
    };

    let handler = Closure::<dyn FnMut(web_sys::Event)>::new(move |_event: web_sys::Event| {
        callback();
    });

    let target: web_sys::EventTarget = form.clone().into();
    let _ = target.add_event_listener_with_callback("reset", handler.as_ref().unchecked_ref());

    Ok(crate::watcher_guard::WatcherGuard::with_cleanup(
        "web_form_reset_observer",
        sub_id,
        None,
        move || {
            let _ = target.remove_event_listener_with_callback("reset", handler.as_ref().unchecked_ref());
        },
    ))
}

pub(crate) fn observe_pointer_move(
    mut callback: Box<dyn FnMut(f64, f64) + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;

    let sub_id = crate::internal::next_subscription_id();
    let win = web_sys::window().ok_or_else(|| crate::error::Error::Unsupported("No window".into()))?;
    let doc = win.document().ok_or_else(|| crate::error::Error::Unsupported("No document".into()))?;

    let handler = Closure::<dyn FnMut(web_sys::PointerEvent)>::new(move |event: web_sys::PointerEvent| {
        callback(f64::from(event.client_x()), f64::from(event.client_y()));
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

pub(crate) fn observe_scroll(
    mut callback: Box<dyn FnMut() + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;

    let sub_id = crate::internal::next_subscription_id();
    let win = web_sys::window().ok_or_else(|| crate::error::Error::Unsupported("No window".into()))?;

    let handler = Closure::<dyn FnMut(web_sys::Event)>::new(move |_event: web_sys::Event| {
        callback();
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

pub(crate) fn observe_element_scroll(
    element_id: &str,
    mut callback: Box<dyn FnMut() + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;

    let sub_id = crate::internal::next_subscription_id();
    let win = web_sys::window().ok_or_else(|| crate::error::Error::Unsupported("No window".into()))?;
    let doc = win.document().ok_or_else(|| crate::error::Error::Unsupported("No document".into()))?;
    let target_el = doc.get_element_by_id(element_id)
        .ok_or_else(|| crate::error::Error::ElementNotFound(element_id.to_string()))?;

    let handler = Closure::<dyn FnMut(web_sys::Event)>::new(move |_event: web_sys::Event| {
        callback();
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
            let _ = target.remove_event_listener_with_callback(
                "scroll",
                handler.as_ref().unchecked_ref(),
            );
        },
    ))
}

pub(crate) fn observe_visibility_change(
    mut callback: Box<dyn FnMut(crate::dom::observer::DocumentVisibility) + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;

    let sub_id = crate::internal::next_subscription_id();
    let win =
        web_sys::window().ok_or_else(|| crate::error::Error::Unsupported("No window".into()))?;
    let doc = win
        .document()
        .ok_or_else(|| crate::error::Error::Unsupported("No document".into()))?;

    let initial_hidden =
        doc.hidden() || doc.visibility_state() == web_sys::VisibilityState::Hidden;
    let last_hidden = std::cell::Cell::new(initial_hidden);
    let doc_for_cb = doc.clone();

    let handler = Closure::<dyn FnMut(web_sys::Event)>::new(move |_event: web_sys::Event| {
        let hidden =
            doc_for_cb.hidden() || doc_for_cb.visibility_state() == web_sys::VisibilityState::Hidden;
        if last_hidden.replace(hidden) != hidden {
            let vis = if hidden {
                crate::dom::observer::DocumentVisibility::Hidden
            } else {
                crate::dom::observer::DocumentVisibility::Visible
            };
            callback(vis);
        }
    });

    let doc_target: web_sys::EventTarget = doc.into();
    let _ = doc_target.add_event_listener_with_callback(
        "visibilitychange",
        handler.as_ref().unchecked_ref(),
    );

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



