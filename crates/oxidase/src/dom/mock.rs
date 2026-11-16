//! Mock/Host DOM backend implementation for non-browser / test environments.

/// Mock DOM Document implementation for host environments and unit testing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Document {
    _private: (),
}

impl Document {
    /// Returns the current active document. In headless host environments, returns None unless mocked.
    pub fn current() -> Option<Self> {
        None
    }

    /// Creates a mock Document for testing.
    pub fn mock() -> Self {
        Self { _private: () }
    }

    /// Returns whether this document is authoritative. Always true for mock documents.
    pub fn is_authoritative(&self) -> bool {
        true
    }

    /// Returns the element with the specified ID if present in mock document.
    pub fn element_by_id(&self, _id: &str) -> Option<crate::dom::element::Element> {
        None
    }

    /// Queries the first element matching the given CSS selector in mock document.
    pub fn query_selector(&self, _selector: &str) -> Option<crate::dom::element::Element> {
        None
    }

    /// Returns the element that currently holds active keyboard focus in the mock document.
    pub fn active_element(&self) -> Option<crate::dom::element::Element> {
        None
    }

    /// Returns the Window handle associated with this document.
    pub fn window(&self) -> crate::window::Window {
        crate::window::window()
    }

    /// Sets the document viewport scroll offset.
    pub fn set_viewport_scroll(&self, _x: f64, _y: f64) {}

    /// Queries live viewport dimensions (width, height).
    pub fn inner_size(&self) -> (f64, f64) {
        (0.0, 0.0)
    }

    /// Queries live viewport scroll offset (x, y).
    pub fn viewport_scroll_offset(&self) -> (f64, f64) {
        (0.0, 0.0)
    }

    /// Queries mock viewport.
    pub fn viewport(&self) -> crate::runtime::geometry::Viewport {
        crate::runtime::geometry::Viewport::default()
    }

    /// Measures mock rect.
    pub fn measure_rect(&self, _element_id: &str) -> Option<crate::runtime::geometry::Rect> {
        None
    }

    /// Measures pre-transform unscaled rect in mock backend.
    pub fn measure_unscaled_rect(&self, _element_id: &str) -> Option<crate::runtime::geometry::Rect> {
        None
    }

    /// Measures natural auto border-box height in mock backend.
    pub fn measure_auto_height(&self, _element_id: &str) -> Option<f64> {
        None
    }

    /// Requests mock focus.
    pub fn set_focus(&self, element_id: &str) -> Result<(), crate::error::HostError> {
        self.set_focus_with_options(element_id, crate::dom::element::FocusOptions::default())
    }

    /// Requests mock focus with explicit focus options.
    pub fn set_focus_with_options(
        &self,
        element_id: &str,
        _options: crate::dom::element::FocusOptions,
    ) -> Result<(), crate::error::HostError> {
        Err(crate::error::HostError::ElementNotFound(element_id.to_string()))
    }

    /// Blurs any active element in mock backend.
    pub fn blur_active(&self) -> Result<(), crate::error::HostError> {
        Ok(())
    }

    /// Queries mock active element.
    pub fn is_element_active(&self, _element_id: &str) -> bool {
        false
    }

    /// Queries mock focus containment.
    pub fn is_focus_within(&self, _element_id: &str) -> bool {
        false
    }

    /// Queries mock `:focus-visible` state.
    pub fn is_element_focus_visible(&self, _element_id: &str) -> bool {
        false
    }

    /// Returns whether the mock document is currently marked hidden.
    pub fn is_document_hidden(&self) -> bool {
        MOCK_DOCUMENT_HIDDEN.with(|c| c.get())
    }

    /// Sets mock document visibility state and notifies registered visibility observers on transitions.
    pub fn set_document_hidden(&self, hidden: bool) {
        let prev = MOCK_DOCUMENT_HIDDEN.with(|c| c.replace(hidden));
        if prev == hidden {
            return;
        }
        let vis = if hidden {
            crate::dom::observer::DocumentVisibility::Hidden
        } else {
            crate::dom::observer::DocumentVisibility::Visible
        };
        let handles = MOCK_VISIBILITY_OBSERVERS.with(|cell| {
            let Ok(reg) = cell.try_borrow() else {
                return Vec::new();
            };
            reg.values().cloned().collect::<Vec<_>>()
        });
        for handle in handles {
            if let Ok(mut cb) = handle.try_borrow_mut() {
                cb(vis);
            }
        }
    }

    /// Queries mock reference hidden.
    pub fn is_reference_hidden(&self, _anchor_ids: &[&str]) -> bool {
        false
    }

    /// Returns whether any CSS animations are currently active. Always false in mock backend.
    #[inline]
    pub fn is_animating(&self) -> bool {
        false
    }

    /// Synchronizes hosted observers. No-op in mock backend.
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

    /// Queries mock descendant order. Fails closed with Unsupported.
    pub fn query_descendant_order(
        &self,
        _root_id: &str,
        _candidate_ids: &[&str],
    ) -> Result<Vec<String>, crate::error::HostError> {
        Err(crate::error::HostError::Unsupported(
            "Mock DOM does not support query_descendant_order".into(),
        ))
    }

    /// Intentional no-op in mock backend.
    pub fn set_body_user_select_none(&self) {}

    /// Intentional no-op in mock backend.
    pub fn restore_body_user_select(&self) {}

    /// Intentional no-op in mock backend: portal tree structure is VDOM-owned outside browser DOM hosts.
    pub fn teleport_portal_to_host(&self, _element_id: &str, _host_id: Option<&str>) {}

    /// Intentional no-op in mock backend: portal tree structure is VDOM-owned outside browser DOM hosts.
    pub fn remove_portal_element_by_id(&self, _element_id: &str) {}
}

pub(crate) fn observe_element_resize(
    _element_id: &str,
    _callback: Box<dyn FnMut(crate::dom::observer::ResizeEntry) + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    Ok(crate::watcher_guard::WatcherGuard::new("mock_resize", 0, None))
}

pub(crate) fn observe_capture_events(
    _callback: Box<dyn FnMut(crate::dom::observer::CaptureEvent) + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    Ok(crate::watcher_guard::WatcherGuard::new("mock_capture", 0, None))
}

pub(crate) fn observe_transition_lifecycle(
    _element_id: &str,
    _fallback_timeout_ms: u64,
    _callback: Box<dyn FnMut(crate::dom::observer::TransitionLifecycleEvent) + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    Ok(crate::watcher_guard::WatcherGuard::new("mock_transition", 0, None))
}

pub(crate) fn observe_form_reset(
    _element_id: &str,
    _form_id: Option<&str>,
    _callback: Box<dyn FnMut() + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    Ok(crate::watcher_guard::WatcherGuard::new("mock_form_reset", 0, None))
}

pub(crate) fn observe_pointer_move(
    _callback: Box<dyn FnMut(f64, f64) + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    Ok(crate::watcher_guard::WatcherGuard::new("mock_pointer_move", 0, None))
}

pub(crate) fn observe_scroll(
    _callback: Box<dyn FnMut() + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    Ok(crate::watcher_guard::WatcherGuard::new("mock_scroll", 0, None))
}

pub(crate) fn observe_element_scroll(
    _element_id: &str,
    _callback: Box<dyn FnMut() + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    Ok(crate::watcher_guard::WatcherGuard::new("mock_element_scroll", 0, None))
}

std::thread_local! {
    static MOCK_DOCUMENT_HIDDEN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static MOCK_VISIBILITY_OBSERVERS: std::cell::RefCell<
        std::collections::BTreeMap<
            u64,
            std::rc::Rc<std::cell::RefCell<Box<dyn FnMut(crate::dom::observer::DocumentVisibility)>>>,
        >,
    > = const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
}

pub(crate) fn observe_visibility_change(
    callback: Box<dyn FnMut(crate::dom::observer::DocumentVisibility) + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    let sub_id = crate::internal::next_subscription_id();
    MOCK_VISIBILITY_OBSERVERS.with(|cell| {
        cell.borrow_mut()
            .insert(sub_id, std::rc::Rc::new(std::cell::RefCell::new(callback)));
    });
    Ok(crate::watcher_guard::WatcherGuard::with_cleanup(
        "mock_visibility_observer",
        sub_id,
        None,
        move || {
            MOCK_VISIBILITY_OBSERVERS.with(|cell| {
                if let Ok(mut reg) = cell.try_borrow_mut() {
                    reg.remove(&sub_id);
                }
            });
        },
    ))
}



