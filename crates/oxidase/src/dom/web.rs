//! Web (`wasm32`) DOM backend implementation backed by `web-sys`.

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

    /// Returns the active observer counts for element scroll listeners.
    pub fn element_scroll_observer_count(&self, _element_id: Option<&str>) -> usize {
        0
    }

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
            if let Some(el) = doc.get_element_by_id(id) {
                if root.contains(Some(&el)) {
                    matched_candidates.push((id.to_string(), el));
                }
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

    if let Some(ref el) = target_el {
        let cb_end = cb_cell.clone();
        let on_end = Closure::<dyn FnMut(web_sys::AnimationEvent)>::new(move |event: web_sys::AnimationEvent| {
            if let Some(mut cb) = cb_end.borrow_mut().take() {
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
            if let Some(mut cb) = cb_cancel.borrow_mut().take() {
                cb(crate::dom::observer::TransitionLifecycleEvent::AnimationCancel {
                    animation_name: event.animation_name(),
                });
            }
        });
        let _ = target.add_event_listener_with_callback("animationcancel", on_cancel.as_ref().unchecked_ref());
        anim_cancel_handler = Some((target, on_cancel));
    }

    let cb_timeout = cb_cell.clone();
    let task = if fallback_timeout_ms > 0 && dioxus::core::Runtime::try_current().is_some() {
        Some(dioxus::prelude::spawn(async move {
            futures_timer::Delay::new(std::time::Duration::from_millis(fallback_timeout_ms)).await;
            if let Some(mut cb) = cb_timeout.borrow_mut().take() {
                cb(crate::dom::observer::TransitionLifecycleEvent::TimeoutFallback);
            }
        }))
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
            cb_cell.borrow_mut().take();
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



