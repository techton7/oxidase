use blitz_dom::BaseDocument;
use std::cell::RefCell;
use std::rc::Rc;

/// Native Blitz Document handle wrapping the real `BaseDocument`.
#[derive(Clone, Default)]
pub struct Document {
    inner: Option<Rc<RefCell<BaseDocument>>>,
}

impl std::fmt::Debug for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("Document");
        if let Some(base) = &self.inner {
            s.field("document_id", &base.borrow().id());
        } else {
            s.field("active", &false);
        }
        s.finish()
    }
}

impl Document {
    /// Returns the active native Blitz document if available in current thread or Dioxus context.
    pub fn current() -> Option<Self> {
        // 1. Check Dioxus root context if running within an active Dioxus runtime
        if dioxus::core::Runtime::try_current().is_some() {
            if let Some(doc) = dioxus::prelude::try_consume_context::<Document>() {
                if doc.inner.is_some() {
                    return Some(doc);
                }
            }
        }

        // 2. Check thread-local (active during harness dispatch or scoped execution in Blitz)
        if let Some(doc) = CURRENT_NATIVE_DOC.with(|cell| cell.borrow().clone()) {
            if doc.inner.is_some() {
                return Some(doc);
            }
        }

        None
    }

    /// Construct a Document wrapping a real Blitz BaseDocument and wire engine bridges.
    pub fn from_base(inner: Rc<RefCell<BaseDocument>>) -> Self {
        let doc = Self {
            inner: Some(inner),
        };
        doc.attach_engine_bridges();
        doc
    }

    /// Internal constructor without re-attaching engine bridges.
    #[allow(dead_code)]
    pub(crate) fn from_base_raw(inner: Rc<RefCell<BaseDocument>>) -> Self {
        Self {
            inner: Some(inner),
        }
    }

    /// Wires Blitz engine-level capture events and animation completion signals into the Oxidase observer substrate.
    pub fn attach_engine_bridges(&self) {
        // Reserved for Blitz engine-level capture events and animation completion signals.
    }

    /// Access the underlying real `BaseDocument`.
    pub fn base(&self) -> &Rc<RefCell<BaseDocument>> {
        self.inner
            .as_ref()
            .expect("Document has no active BaseDocument")
    }

    /// Sets the thread-local active native Document for the duration of a closure.
    pub fn with_current<R>(doc: Self, f: impl FnOnce() -> R) -> R {
        CURRENT_NATIVE_DOC.with(|cell| {
            let prev = cell.borrow_mut().replace(doc);
            let res = f();
            *cell.borrow_mut() = prev;
            res
        })
    }

    /// Sets or clears the current thread-local active native Document.
    pub fn set_current(doc: Option<Self>) {
        CURRENT_NATIVE_DOC.with(|cell| {
            *cell.borrow_mut() = doc;
        });
    }

    /// Provide this Document into the active Dioxus component context.
    pub fn provide_context(self) {
        dioxus::prelude::provide_context(self);
    }

    /// Returns the element with the specified ID if present in the active document.
    pub fn element_by_id(&self, id: &str) -> Option<crate::dom::element::Element> {
        let base = self.inner.as_ref()?;
        let node_id = base.try_borrow().ok()?.get_element_by_id(id)?;
        Some(crate::dom::element::Element::from_native(
            base.clone(),
            node_id,
        ))
    }

    /// Returns the element that currently holds active keyboard focus in the document.
    pub fn active_element(&self) -> Option<crate::dom::element::Element> {
        let base = self.inner.as_ref()?;
        let node_id = base.try_borrow().ok()?.active_focus_node_id()?;
        Some(crate::dom::element::Element::from_native(
            base.clone(),
            node_id,
        ))
    }

    /// Returns the Window handle associated with this document.
    pub fn window(&self) -> crate::window::Window {
        crate::window::window()
    }

    /// Sets the document viewport scroll offset and notifies registered scroll observers.
    pub fn set_viewport_scroll(&self, x: f64, y: f64) {
        if let Some(base) = &self.inner {
            base.borrow_mut()
                .set_viewport_scroll(blitz_dom::Point { x, y });
            self.dispatch_scroll();
        }
    }

    /// Queries the live viewport dimensions (width, height) directly from the Blitz document.
    pub fn inner_size(&self) -> (f64, f64) {
        let Some(base) = &self.inner else {
            return (0.0, 0.0);
        };
        let base = base.borrow();
        let vp = base.viewport();
        let scale = (vp.hidpi_scale * vp.zoom).max(0.001) as f64;
        (vp.window_size.0 as f64 / scale, vp.window_size.1 as f64 / scale)
    }

    /// Queries the live viewport scroll offset (x, y) directly from the Blitz document.
    pub fn viewport_scroll_offset(&self) -> (f64, f64) {
        let Some(base) = &self.inner else {
            return (0.0, 0.0);
        };
        let scroll = base.borrow().viewport_scroll();
        (scroll.x, scroll.y)
    }

    /// Queries the live viewport dimensions and scroll offsets from the underlying Blitz document.
    pub fn viewport(&self) -> crate::runtime::geometry::Viewport {
        let Some(base) = &self.inner else {
            return crate::runtime::geometry::Viewport::default();
        };
        let base = base.borrow();
        let vp = base.viewport();
        let scroll = base.viewport_scroll();
        let scale = (vp.hidpi_scale * vp.zoom).max(0.001) as f64;
        let width = vp.window_size.0 as f64 / scale;
        let height = vp.window_size.1 as f64 / scale;
        crate::runtime::geometry::Viewport {
            width,
            height,
            scroll_x: scroll.x,
            scroll_y: scroll.y,
        }
    }

    /// Measures the bounding client rectangle of an element by ID by accumulating
    /// layout coordinates over the ancestor hierarchy and accounting for viewport scroll.
    pub fn measure_rect(&self, element_id: &str) -> Option<crate::runtime::geometry::Rect> {
        let base = self.inner.as_ref()?.borrow();
        let node_id = base.get_element_by_id(element_id)?;
        let node = base.get_node(node_id)?;
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

        Some(crate::runtime::geometry::Rect::new(x, y, width, height))
    }

    /// Requests focus for the element with the specified ID in the Blitz DOM.
    pub fn set_focus(&self, element_id: &str) -> Result<(), crate::error::HostError> {
        let base = self
            .inner
            .as_ref()
            .ok_or_else(|| crate::error::HostError::Unsupported("No active native document".into()))?;
        let mut base_mut = base.borrow_mut();
        let node_id = base_mut
            .get_element_by_id(element_id)
            .ok_or_else(|| crate::error::HostError::ElementNotFound(element_id.to_string()))?;
        base_mut.set_focus_to(node_id);
        Ok(())
    }

    /// Checks if the element with the specified ID currently holds document focus in the Blitz DOM.
    pub fn is_element_active(&self, element_id: &str) -> bool {
        let Some(base) = &self.inner else {
            return false;
        };
        let base = base.borrow();
        let Some(node_id) = base.get_element_by_id(element_id) else {
            return false;
        };
        base.active_focus_node_id() == Some(node_id)
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

    /// Restyles the tree, relayouts it, and triggers observer notifications on changed elements.
    pub fn resolve(&self, current_time_for_animations: f64) {
        if let Some(base) = &self.inner {
            base.borrow_mut().resolve(current_time_for_animations);
            self.notify_layout_observers();
        }
    }

    /// Evaluates all registered resize observers and fires callbacks for elements whose layout changed.
    pub fn notify_layout_observers(&self) {
        CURRENT_OBSERVERS.with(|cell| {
            let mut reg = cell.borrow_mut();
            for obs in reg.resize_observers.values_mut() {
                if let Some(rect) = self.measure_rect(&obs.target_id) {
                    let changed = if let Some(last) = &obs.last_rect {
                        (last.width - rect.width).abs() > 0.001
                            || (last.height - rect.height).abs() > 0.001
                            || (last.x - rect.x).abs() > 0.001
                            || (last.y - rect.y).abs() > 0.001
                    } else {
                        true
                    };
                    if changed {
                        obs.last_rect = Some(rect.clone());
                        (obs.callback)(crate::dom::observer::ResizeEntry {
                            target_id: obs.target_id.clone(),
                            content_rect: rect,
                        });
                    }
                }
            }
        });
    }

    /// Resolves the ancestor hierarchy of element IDs from target up to the document root.
    pub fn node_chain_ids(&self, node_id: blitz_dom::NodeId) -> Vec<String> {
        let Some(base) = &self.inner else {
            return Vec::new();
        };
        let base = base.borrow();
        let chain = base.node_chain(node_id);
        let mut path_ids = Vec::with_capacity(chain.len());
        for &nid in &chain {
            if let Some(node) = base.get_node(nid) {
                if let Some(el) = node.element_data() {
                    if let Some(id) = &el.id {
                        path_ids.push(id.to_string());
                    }
                }
            }
        }
        path_ids
    }

    /// Intercepts and dispatches a global capture-phase event with precomputed path_ids.
    pub fn dispatch_capture_event_with_path(
        &self,
        kind: crate::dom::observer::CaptureEventKind,
        _target_node_id: blitz_dom::NodeId,
        path_ids: Vec<String>,
    ) {
        let target_id = path_ids.first().cloned();
        let event = crate::dom::observer::CaptureEvent {
            kind,
            target_id,
            path_ids,
        };
        CURRENT_OBSERVERS.with(|cell| {
            let mut reg = cell.borrow_mut();
            for cb in reg.capture_observers.values_mut() {
                cb(event.clone());
            }
        });
    }

    /// Intercepts and dispatches a global capture-phase event to all active capture observers before default target handling.
    pub fn dispatch_capture_event(
        &self,
        kind: crate::dom::observer::CaptureEventKind,
        target_node_id: blitz_dom::NodeId,
    ) {
        let path_ids = self.node_chain_ids(target_node_id);
        self.dispatch_capture_event_with_path(kind, target_node_id, path_ids);
    }

    /// Dispatches an animation completion signal to registered transition observers.
    pub fn dispatch_animation_end(&self, element_id: &str, animation_name: &str) {
        CURRENT_OBSERVERS.with(|cell| {
            let mut reg = cell.borrow_mut();
            for obs in reg.transition_observers.values_mut() {
                if obs.target_id == element_id {
                    (obs.callback)(crate::dom::observer::TransitionLifecycleEvent::AnimationEnd {
                        animation_name: animation_name.to_string(),
                    });
                }
            }
        });
    }

    /// Dispatches an animation cancellation signal to registered transition observers.
    pub fn dispatch_animation_cancel(&self, element_id: &str, animation_name: &str) {
        CURRENT_OBSERVERS.with(|cell| {
            let mut reg = cell.borrow_mut();
            for obs in reg.transition_observers.values_mut() {
                if obs.target_id == element_id {
                    (obs.callback)(crate::dom::observer::TransitionLifecycleEvent::AnimationCancel {
                        animation_name: animation_name.to_string(),
                    });
                }
            }
        });
    }

    /// Dispatches a timeout fallback signal to registered transition observers.
    pub fn dispatch_transition_fallback(&self, element_id: &str) {
        CURRENT_OBSERVERS.with(|cell| {
            let mut reg = cell.borrow_mut();
            for obs in reg.transition_observers.values_mut() {
                if obs.target_id == element_id {
                    (obs.callback)(crate::dom::observer::TransitionLifecycleEvent::TimeoutFallback);
                }
            }
        });
    }

    /// Dispatches a form reset signal to registered form reset observers.
    pub fn dispatch_form_reset(&self, form_element_id: &str) {
        let Some(base) = &self.inner else {
            return;
        };
        let base = base.borrow();
        let form_node_id = base.get_element_by_id(form_element_id);

        CURRENT_OBSERVERS.with(|cell| {
            let mut reg = cell.borrow_mut();
            for obs in reg.form_reset_observers.values_mut() {
                if let Some(expected_form_id) = &obs.form_id {
                    if expected_form_id == form_element_id {
                        (obs.callback)();
                    }
                } else if let Some(fnid) = form_node_id {
                    if let Some(el_nid) = base.get_element_by_id(&obs.element_id) {
                        let chain = base.node_chain(el_nid);
                        if chain.contains(&fnid) {
                            (obs.callback)();
                        }
                    }
                }
            }
        });
    }

    /// Dispatches a scroll event to all registered scroll observers.
    pub fn dispatch_scroll(&self) {
        CURRENT_OBSERVERS.with(|cell| {
            let mut reg = cell.borrow_mut();
            for cb in reg.scroll_observers.values_mut() {
                cb();
            }
        });
    }

    /// Dispatches an element scroll event to all registered observers for the element ID.
    pub fn dispatch_element_scroll(&self, element_id: &str) {
        CURRENT_OBSERVERS.with(|cell| {
            let mut reg = cell.borrow_mut();
            if let Some(observers) = reg.element_scroll_observers.get_mut(element_id) {
                for (_, cb) in observers.iter_mut() {
                    cb();
                }
            }
        });
    }

    /// Dispatches an element scroll event by looking up the element's DOM ID from its `NodeId`.
    pub fn dispatch_element_scroll_by_node_id(&self, node_id: blitz_dom::NodeId) {
        let Some(base) = &self.inner else {
            return;
        };
        let b = base.borrow();
        if let Some(node) = b.get_node(node_id) {
            if let Some(el) = node.element_data() {
                if let Some(id) = &el.id {
                    let id_str = id.to_string();
                    drop(b);
                    self.dispatch_element_scroll(&id_str);
                }
            }
        }
    }

    /// Queries the descendant tree order of elements matching `candidate_ids` within `root_id`.
    ///
    /// Performs iterative depth-first pre-order traversal over `node.children`,
    /// matching candidate IDs against `node.element_data().and_then(|el| el.id.as_deref())`.
    pub fn query_descendant_order(
        &self,
        root_id: &str,
        candidate_ids: &[&str],
    ) -> Result<Vec<String>, crate::error::HostError> {
        let base = self
            .inner
            .as_ref()
            .ok_or_else(|| crate::error::HostError::Unsupported("No active native document".into()))?;
        let base = base.borrow();
        let root_node_id = base
            .get_element_by_id(root_id)
            .ok_or_else(|| crate::error::HostError::ElementNotFound(root_id.to_string()))?;

        use std::collections::HashSet;
        let mut candidate_set: HashSet<&str> = candidate_ids.iter().copied().collect();
        let mut ordered_ids = Vec::new();

        if candidate_set.is_empty() {
            return Ok(ordered_ids);
        }

        let mut stack = Vec::new();
        if let Some(root_node) = base.get_node(root_node_id) {
            for &child_id in root_node.children.iter().rev() {
                stack.push(child_id);
            }
        }

        while let Some(current_id) = stack.pop() {
            if let Some(node) = base.get_node(current_id) {
                if let Some(el) = node.element_data() {
                    if let Some(id) = el.id.as_deref() {
                        if candidate_set.remove(id) {
                            ordered_ids.push(id.to_string());
                            if candidate_set.is_empty() {
                                break;
                            }
                        }
                    }
                }
                for &child_id in node.children.iter().rev() {
                    stack.push(child_id);
                }
            }
        }

        Ok(ordered_ids)
    }
}

std::thread_local! {
    static CURRENT_NATIVE_DOC: RefCell<Option<Document>> = const { RefCell::new(None) };
    static CURRENT_OBSERVERS: RefCell<NativeObserverRegistry> = RefCell::new(NativeObserverRegistry::default());
}

struct ResizeObserverEntry {
    target_id: String,
    last_rect: Option<crate::runtime::geometry::Rect>,
    callback: Box<dyn FnMut(crate::dom::observer::ResizeEntry)>,
}

struct TransitionObserverEntry {
    target_id: String,
    callback: Box<dyn FnMut(crate::dom::observer::TransitionLifecycleEvent)>,
}

struct FormResetObserverEntry {
    element_id: String,
    form_id: Option<String>,
    callback: Box<dyn FnMut()>,
}

#[derive(Default)]
struct NativeObserverRegistry {
    resize_observers: std::collections::HashMap<u64, ResizeObserverEntry>,
    capture_observers: std::collections::HashMap<u64, Box<dyn FnMut(crate::dom::observer::CaptureEvent)>>,
    transition_observers: std::collections::HashMap<u64, TransitionObserverEntry>,
    form_reset_observers: std::collections::HashMap<u64, FormResetObserverEntry>,
    scroll_observers: std::collections::HashMap<u64, Box<dyn FnMut()>>,
    element_scroll_observers: std::collections::HashMap<String, Vec<(u64, Box<dyn FnMut()>)>>,
}

pub(crate) fn observe_element_resize(
    element_id: &str,
    callback: Box<dyn FnMut(crate::dom::observer::ResizeEntry) + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    let sub_id = crate::internal::next_subscription_id();
    if let Some(doc) = Document::current() {
        if doc.element_by_id(element_id).is_none() {
            return Err(crate::error::Error::ElementNotFound(element_id.to_string()));
        }
    }
    let initial_rect = Document::current().and_then(|doc| doc.measure_rect(element_id));
    CURRENT_OBSERVERS.with(|cell| {
        cell.borrow_mut().resize_observers.insert(
            sub_id,
            ResizeObserverEntry {
                target_id: element_id.to_string(),
                last_rect: initial_rect,
                callback,
            },
        );
    });

    Ok(crate::watcher_guard::WatcherGuard::with_cleanup(
        "native_resize_observer",
        sub_id,
        None,
        move || {
            CURRENT_OBSERVERS.with(|cell| {
                cell.borrow_mut().resize_observers.remove(&sub_id);
            });
        },
    ))
}

pub(crate) fn observe_capture_events(
    callback: Box<dyn FnMut(crate::dom::observer::CaptureEvent) + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    let sub_id = crate::internal::next_subscription_id();
    CURRENT_OBSERVERS.with(|cell| {
        cell.borrow_mut().capture_observers.insert(sub_id, callback);
    });

    Ok(crate::watcher_guard::WatcherGuard::with_cleanup(
        "native_capture_observer",
        sub_id,
        None,
        move || {
            CURRENT_OBSERVERS.with(|cell| {
                cell.borrow_mut().capture_observers.remove(&sub_id);
            });
        },
    ))
}

pub(crate) fn observe_transition_lifecycle(
    element_id: &str,
    fallback_timeout_ms: u64,
    callback: Box<dyn FnMut(crate::dom::observer::TransitionLifecycleEvent) + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    let sub_id = crate::internal::next_subscription_id();
    CURRENT_OBSERVERS.with(|cell| {
        cell.borrow_mut().transition_observers.insert(
            sub_id,
            TransitionObserverEntry {
                target_id: element_id.to_string(),
                callback,
            },
        );
    });

    let task = if fallback_timeout_ms > 0 && dioxus::core::Runtime::try_current().is_some() {
        Some(dioxus::prelude::spawn(async move {
            futures_timer::Delay::new(std::time::Duration::from_millis(fallback_timeout_ms)).await;
            let entry = CURRENT_OBSERVERS.with(|cell| {
                cell.borrow_mut().transition_observers.remove(&sub_id)
            });
            if let Some(mut obs) = entry {
                (obs.callback)(crate::dom::observer::TransitionLifecycleEvent::TimeoutFallback);
            }
        }))
    } else {
        None
    };

    Ok(crate::watcher_guard::WatcherGuard::with_cleanup(
        "native_transition_lifecycle",
        sub_id,
        task,
        move || {
            CURRENT_OBSERVERS.with(|cell| {
                cell.borrow_mut().transition_observers.remove(&sub_id);
            });
        },
    ))
}

pub(crate) fn observe_form_reset(
    element_id: &str,
    form_id: Option<&str>,
    callback: Box<dyn FnMut() + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    let sub_id = crate::internal::next_subscription_id();
    CURRENT_OBSERVERS.with(|cell| {
        cell.borrow_mut().form_reset_observers.insert(
            sub_id,
            FormResetObserverEntry {
                element_id: element_id.to_string(),
                form_id: form_id.map(ToString::to_string),
                callback,
            },
        );
    });

    Ok(crate::watcher_guard::WatcherGuard::with_cleanup(
        "native_form_reset_observer",
        sub_id,
        None,
        move || {
            CURRENT_OBSERVERS.with(|cell| {
                cell.borrow_mut().form_reset_observers.remove(&sub_id);
            });
        },
    ))
}

pub(crate) fn observe_scroll(
    callback: Box<dyn FnMut() + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    let sub_id = crate::internal::next_subscription_id();
    CURRENT_OBSERVERS.with(|cell| {
        cell.borrow_mut().scroll_observers.insert(sub_id, callback);
    });

    Ok(crate::watcher_guard::WatcherGuard::with_cleanup(
        "native_scroll_observer",
        sub_id,
        None,
        move || {
            CURRENT_OBSERVERS.with(|cell| {
                cell.borrow_mut().scroll_observers.remove(&sub_id);
            });
        },
    ))
}

pub(crate) fn observe_element_scroll(
    element_id: &str,
    callback: Box<dyn FnMut() + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    let sub_id = crate::internal::next_subscription_id();
    let el_id = element_id.to_string();

    if let Some(doc) = Document::current() {
        if doc.element_by_id(element_id).is_none() {
            return Err(crate::error::Error::ElementNotFound(element_id.to_string()));
        }
    }

    let el_id_clone = el_id.clone();
    CURRENT_OBSERVERS.with(|cell| {
        cell.borrow_mut()
            .element_scroll_observers
            .entry(el_id)
            .or_default()
            .push((sub_id, callback));
    });

    Ok(crate::watcher_guard::WatcherGuard::with_cleanup(
        "native_element_scroll_observer",
        sub_id,
        None,
        move || {
            CURRENT_OBSERVERS.with(|cell| {
                let mut reg = cell.borrow_mut();
                if let Some(observers) = reg.element_scroll_observers.get_mut(&el_id_clone) {
                    observers.retain(|(id, _)| *id != sub_id);
                    if observers.is_empty() {
                        reg.element_scroll_observers.remove(&el_id_clone);
                    }
                }
            });
        },
    ))
}

impl Document {
    /// Returns the active observer counts for element scroll listeners.
    ///
    /// Used for structural leak verification and teardown validation.
    pub fn element_scroll_observer_count(&self, element_id: Option<&str>) -> usize {
        CURRENT_OBSERVERS.with(|cell| {
            let reg = cell.borrow();
            if let Some(id) = element_id {
                reg.element_scroll_observers.get(id).map_or(0, |list| list.len())
            } else {
                reg.element_scroll_observers.values().map(|list| list.len()).sum()
            }
        })
    }
}


