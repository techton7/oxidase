use blitz_dom::BaseDocument;
use blitz_traits::events::DomEventData;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Native Blitz Document handle wrapping the real `BaseDocument`.
#[derive(Clone)]
pub struct Document {
    inner: Rc<RefCell<Option<Rc<RefCell<BaseDocument>>>>>,
    pub(crate) authoritative: Rc<Cell<bool>>,
    pub(crate) last_synced_len: Rc<Cell<usize>>,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            inner: Rc::new(RefCell::new(None)),
            authoritative: Rc::new(Cell::new(true)),
            last_synced_len: Rc::new(Cell::new(0)),
        }
    }
}

impl std::fmt::Debug for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("Document");
        if let Some(base) = self.get_base() {
            s.field("document_id", &base.borrow().id());
        } else {
            s.field("active", &false);
        }
        s.field("authoritative", &self.authoritative.get());
        s.finish()
    }
}

/// Extracts the underlying `Rc<RefCell<BaseDocument>>` from a `dioxus_native::NodeHandle`.
pub fn extract_base_doc(handle: &dioxus_native::NodeHandle) -> Rc<RefCell<BaseDocument>> {
    #[allow(dead_code)]
    struct NodeHandleLayout {
        doc: Rc<RefCell<BaseDocument>>,
        node_id: blitz_dom::NodeId,
    }
    let cloned = handle.clone();
    let repr: NodeHandleLayout = unsafe { std::mem::transmute(cloned) };
    repr.doc
}

impl Document {
    /// Returns the active native Blitz document if available in current thread or Dioxus context.
    pub fn current() -> Option<Self> {
        // 1. Check Dioxus root context if running within an active Dioxus runtime
        if dioxus::core::Runtime::try_current().is_some()
            && let Some(doc) = dioxus::prelude::try_consume_context::<Document>()
                && doc.get_base().is_some() {
                    return Some(doc);
                }

        // 2. Check thread-local (active during harness dispatch or scoped execution in Blitz)
        if let Some(doc) = CURRENT_NATIVE_DOC.with(|cell| cell.borrow().clone())
            && doc.get_base().is_some() {
                return Some(doc);
            }

        None
    }

    /// Construct a Document wrapping a real Blitz BaseDocument and wire engine bridges.
    pub fn from_base(inner: Rc<RefCell<BaseDocument>>) -> Self {
        let doc = Self {
            inner: Rc::new(RefCell::new(Some(inner))),
            authoritative: Rc::new(Cell::new(true)),
            last_synced_len: Rc::new(Cell::new(0)),
        };
        doc.attach_engine_bridges();
        doc
    }

    /// Construct a non-authoritative ambient Document wrapping an unattached Blitz BaseDocument.
    pub fn from_unattached_base(inner: Rc<RefCell<BaseDocument>>) -> Self {
        Self {
            inner: Rc::new(RefCell::new(Some(inner))),
            authoritative: Rc::new(Cell::new(false)),
            last_synced_len: Rc::new(Cell::new(0)),
        }
    }

    /// Internal constructor without re-attaching engine bridges.
    #[allow(dead_code)]
    pub(crate) fn from_base_raw(inner: Rc<RefCell<BaseDocument>>) -> Self {
        Self {
            inner: Rc::new(RefCell::new(Some(inner))),
            authoritative: Rc::new(Cell::new(true)),
            last_synced_len: Rc::new(Cell::new(0)),
        }
    }

    /// Returns whether this document is authoritative (true for real rendered or fully constructed documents, false for ambient unattached placeholders).
    pub fn is_authoritative(&self) -> bool {
        self.authoritative.get()
    }

    /// Wires Blitz engine-level capture events and animation completion signals into the Oxidase observer substrate.
    pub fn attach_engine_bridges(&self) {
        let Some(base) = self.get_base() else {
            return;
        };
        let weak_inner = Rc::downgrade(&self.inner);
        base.borrow_mut().set_capture_hook(Some(Rc::new(move |base_doc, event, chain| {
            let Some(kind) = (match &event.data {
                DomEventData::PointerDown(_) => Some(crate::dom::observer::CaptureEventKind::PointerDown),
                DomEventData::FocusIn(_) => Some(crate::dom::observer::CaptureEventKind::FocusIn),
                DomEventData::KeyDown(key_event) => {
                    use dioxus::prelude::keyboard_types::Key;
                    let key = match &key_event.key {
                        Key::Escape => "Escape".to_string(),
                        Key::Character(s) => s.clone(),
                        other => format!("{other:?}"),
                    };
                    Some(crate::dom::observer::CaptureEventKind::KeyDown { key })
                }
                _ => None,
            }) else {
                return;
            };

            let mut path_ids = Vec::with_capacity(chain.len());
            for &nid in chain {
                if let Some(node) = base_doc.get_node(nid)
                    && let Some(el) = node.element_data()
                    && let Some(id) = &el.id
                {
                    path_ids.push(id.to_string());
                }
            }

            if let Some(inner) = weak_inner.upgrade() {
                let doc = Document {
                    inner,
                    authoritative: Rc::new(Cell::new(true)),
                    last_synced_len: Rc::new(Cell::new(0)),
                };
                doc.dispatch_capture_event_with_path(kind, event.target, path_ids);
            }
        })));
    }

    /// Binds a live window `BaseDocument` to this document handle in-place.
    ///
    /// Upgrades all existing clones of this document handle across component contexts
    /// and thread-locals, marking the document as authoritative and attaching engine bridges.
    pub fn bind_live_base(&self, base: Rc<RefCell<BaseDocument>>) {
        *self.inner.borrow_mut() = Some(base);
        self.authoritative.set(true);
        self.attach_engine_bridges();
    }

    /// Returns whether any CSS animations are currently active on the native Blitz BaseDocument.
    pub fn is_animating(&self) -> bool {
        if let Some(base) = self.get_base() {
            base.borrow().is_animating()
        } else {
            false
        }
    }

    /// Dispatches a raw UI event into the underlying Blitz BaseDocument's event driver.
    pub fn handle_ui_event(&self, event: blitz_traits::events::UiEvent) {
        if let Some(base) = self.get_base() {
            use blitz_dom::Document as _;
            base.borrow_mut().handle_ui_event(event);
        }
    }

    pub(crate) fn get_base(&self) -> Option<Rc<RefCell<BaseDocument>>> {
        self.inner.borrow().clone()
    }

    /// Access the underlying real `BaseDocument`.
    pub fn base(&self) -> Rc<RefCell<BaseDocument>> {
        self.get_base().expect("Document has no active BaseDocument")
    }

    /// Extracts the underlying `Rc<RefCell<BaseDocument>>` from a `dioxus_native::NodeHandle`.
    pub fn extract_base_doc(handle: &dioxus_native::NodeHandle) -> Rc<RefCell<BaseDocument>> {
        extract_base_doc(handle)
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
        if dioxus::core::Runtime::try_current().is_some() {
            dioxus::prelude::provide_context(self);
        }
    }

    /// Returns the element with the specified ID if present in the active document.
    pub fn element_by_id(&self, id: &str) -> Option<crate::dom::element::Element> {
        let base = self.get_base()?;
        let borrowed = base.try_borrow().ok()?;
        let node_id = borrowed
            .get_element_by_id(id)
            .or_else(|| borrowed.query_selector(&format!("#{id}")).ok().flatten())
            .or_else(|| {
                let mut stack = vec![borrowed.root_node().id];
                while let Some(nid) = stack.pop() {
                    if let Some(node) = borrowed.get_node(nid) {
                        if let Some(el) = node.element_data()
                            && el.id.as_deref() == Some(id) {
                                return Some(nid);
                            }
                        if node.attr(blitz_dom::local_name!("id")) == Some(id) {
                            return Some(nid);
                        }
                        stack.extend(node.children.iter().rev().copied());
                    }
                }
                None
            })?;
        drop(borrowed);
        Some(crate::dom::element::Element::from_native(
            base,
            node_id,
        ))
    }

    /// Returns the element that currently holds active keyboard focus in the document.
    pub fn active_element(&self) -> Option<crate::dom::element::Element> {
        let base = self.get_base()?;
        let node_id = base.try_borrow().ok()?.active_focus_node_id()?;
        Some(crate::dom::element::Element::from_native(
            base,
            node_id,
        ))
    }

    /// Returns the Window handle associated with this document.
    pub fn window(&self) -> crate::window::Window {
        crate::window::window()
    }

    /// Sets the document viewport scroll offset and notifies registered scroll observers.
    pub fn set_viewport_scroll(&self, x: f64, y: f64) {
        if let Some(base) = self.get_base() {
            base.borrow_mut()
                .set_viewport_scroll(blitz_dom::Point { x, y });
            self.dispatch_scroll();
        }
    }

    /// Queries the live viewport dimensions (width, height) directly from the Blitz document.
    pub fn inner_size(&self) -> (f64, f64) {
        let Some(base) = self.get_base() else {
            return (0.0, 0.0);
        };
        let Ok(base) = base.try_borrow() else {
            return (0.0, 0.0);
        };
        let vp = base.viewport();
        let scale = (vp.hidpi_scale * vp.zoom).max(0.001) as f64;
        (vp.window_size.0 as f64 / scale, vp.window_size.1 as f64 / scale)
    }

    /// Queries the live viewport scroll offset (x, y) directly from the Blitz document.
    pub fn viewport_scroll_offset(&self) -> (f64, f64) {
        let Some(base) = self.get_base() else {
            return (0.0, 0.0);
        };
        let Ok(base) = base.try_borrow() else {
            return (0.0, 0.0);
        };
        let scroll = base.viewport_scroll();
        (scroll.x, scroll.y)
    }

    /// Queries the live viewport dimensions and scroll offsets from the underlying Blitz document.
    pub fn viewport(&self) -> crate::runtime::geometry::Viewport {
        let Some(base) = self.get_base() else {
            return crate::runtime::geometry::Viewport::default();
        };
        let Ok(base) = base.try_borrow() else {
            return crate::runtime::geometry::Viewport::default();
        };
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
        let base_rc = self.get_base()?;
        let Ok(base) = base_rc.try_borrow() else {
            return None;
        };
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
            .get_base()
            .ok_or_else(|| crate::error::HostError::Unsupported("No active native document".into()))?;
        let Ok(mut base_mut) = base.try_borrow_mut() else {
            return Ok(());
        };
        let node_id = base_mut
            .get_element_by_id(element_id)
            .ok_or_else(|| crate::error::HostError::ElementNotFound(element_id.to_string()))?;
        base_mut.set_focus_to(node_id);
        Ok(())
    }

    /// Checks if the element with the specified ID currently holds document focus in the Blitz DOM.
    pub fn is_element_active(&self, element_id: &str) -> bool {
        let Some(base) = self.get_base() else {
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
        if let Some(base) = self.get_base() {
            let prev_len = base.borrow().pending_animation_events.len();
            base.borrow_mut().resolve(current_time_for_animations);
            self.sync_animation_observers(prev_len);
            self.last_synced_len.set(base.borrow().pending_animation_events.len());
            self.notify_layout_observers();
        }
    }

    /// Synchronizes native animation and transition completion and cancellation events from Blitz's pending queue
    /// into registered transition observers without destroying or draining the queue.
    pub fn sync_animation_observers(&self, prev_len: usize) {
        let Some(base) = self.get_base() else {
            return;
        };
        let b = base.borrow();
        let start_idx = prev_len.min(b.pending_animation_events.len());
        let events = &b.pending_animation_events[start_idx..];
        if events.is_empty() {
            return;
        }

        enum EventKind {
            AnimationEnd(String),
            AnimationCancel(String),
            TransitionEnd(String),
            TransitionCancel(String),
        }

        let mut dispatches = Vec::new();
        for queued in events {
            let event_match = match &queued.event {
                DomEventData::AnimationEnd(anim) => Some(EventKind::AnimationEnd(anim.animation_name.clone())),
                DomEventData::AnimationCancel(anim) => Some(EventKind::AnimationCancel(anim.animation_name.clone())),
                DomEventData::TransitionEnd(trans) => Some(EventKind::TransitionEnd(trans.property_name.clone())),
                DomEventData::TransitionCancel(trans) => Some(EventKind::TransitionCancel(trans.property_name.clone())),
                _ => None,
            };

            if let Some(kind) = event_match {
                let element_id = b.get_node(queued.node_id).and_then(|node| {
                    node.element_data()
                        .and_then(|el| el.id.as_deref().map(|s| s.to_string()))
                        .or_else(|| node.attr(blitz_dom::local_name!("id")).map(|s| s.to_string()))
                });
                if let Some(id) = element_id {
                    dispatches.push((id, kind));
                }
            }
        }
        drop(b);

        for (element_id, kind) in dispatches {
            match kind {
                EventKind::AnimationEnd(name) => self.dispatch_animation_end(&element_id, &name),
                EventKind::AnimationCancel(name) => self.dispatch_animation_cancel(&element_id, &name),
                EventKind::TransitionEnd(prop) => self.dispatch_transition_end(&element_id, &prop),
                EventKind::TransitionCancel(prop) => self.dispatch_transition_cancel(&element_id, &prop),
            }
        }
    }

    /// Synchronizes all native animation completion and cancellation events from Blitz's pending queue.
    pub fn sync_animation_observers_all(&self) {
        self.sync_animation_observers(0);
        if let Some(base) = self.get_base() {
            self.last_synced_len.set(base.borrow().pending_animation_events.len());
        }
    }

    /// Synchronizes animation and transition events queued by external `BaseDocument::resolve` calls
    /// idempotently without re-processing earlier events or draining `pending_animation_events`.
    pub fn pump_hosted_observers(&self) {
        let Some(base) = self.get_base() else { return };
        let cur_len = base.borrow().pending_animation_events.len();
        let prev_len = self.last_synced_len.get();
        let start_idx = if cur_len < prev_len {
            self.last_synced_len.set(cur_len);
            0
        } else {
            prev_len
        };
        if cur_len > start_idx {
            self.sync_animation_observers(start_idx);
            self.last_synced_len.set(cur_len);
        }
        self.notify_layout_observers();
    }

    /// Evaluates all registered resize observers and fires callbacks for elements whose layout changed.
    pub fn notify_layout_observers(&self) {
        let is_running = IS_NOTIFYING_LAYOUT.with(|c| c.get());
        if is_running {
            NEEDS_RE_EVALUATION.with(|c| c.set(true));
            return;
        }

        IS_NOTIFYING_LAYOUT.with(|c| c.set(true));
        struct NotifyingGuard;
        impl Drop for NotifyingGuard {
            fn drop(&mut self) {
                IS_NOTIFYING_LAYOUT.with(|c| c.set(false));
            }
        }
        let _guard = NotifyingGuard;

        loop {
            NEEDS_RE_EVALUATION.with(|c| c.set(false));

            let callbacks = CURRENT_OBSERVERS.with(|cell| {
                let Ok(mut reg) = cell.try_borrow_mut() else {
                    return Vec::new();
                };
                let mut pending = Vec::new();
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
                            obs.last_rect = Some(rect);
                            pending.push((
                                obs.callback.clone(),
                                crate::dom::observer::ResizeEntry {
                                    target_id: obs.target_id.clone(),
                                    content_rect: rect,
                                },
                            ));
                        }
                    }
                }
                pending
            });

            for (cb, entry) in callbacks {
                if let Ok(mut h) = cb.try_borrow_mut() {
                    h(entry);
                }
            }

            if !NEEDS_RE_EVALUATION.with(|c| c.get()) {
                break;
            }
        }
    }

    /// Resolves the ancestor hierarchy of element IDs from target up to the document root.
    pub fn node_chain_ids(&self, node_id: blitz_dom::NodeId) -> Vec<String> {
        let Some(base) = self.get_base() else {
            return Vec::new();
        };
        let base = base.borrow();
        let chain = base.node_chain(node_id);
        let mut path_ids = Vec::with_capacity(chain.len());
        for &nid in &chain {
            if let Some(node) = base.get_node(nid)
                && let Some(el) = node.element_data()
                    && let Some(id) = &el.id {
                        path_ids.push(id.to_string());
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
        let handles = CURRENT_OBSERVERS.with(|cell| {
            let Ok(reg) = cell.try_borrow() else {
                return Vec::new();
            };
            reg.capture_observers.values().cloned().collect::<Vec<_>>()
        });
        for handle in handles {
            if let Ok(mut cb) = handle.try_borrow_mut() {
                cb(event.clone());
            }
        }
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
        let observers = take_transition_observers(element_id);
        for mut obs in observers {
            (obs.callback)(crate::dom::observer::TransitionLifecycleEvent::AnimationEnd {
                animation_name: animation_name.to_string(),
            });
        }
    }

    /// Dispatches an animation cancellation signal to registered transition observers.
    pub fn dispatch_animation_cancel(&self, element_id: &str, animation_name: &str) {
        let observers = take_transition_observers(element_id);
        for mut obs in observers {
            (obs.callback)(crate::dom::observer::TransitionLifecycleEvent::AnimationCancel {
                animation_name: animation_name.to_string(),
            });
        }
    }

    /// Dispatches a transition completion signal to registered transition observers.
    pub fn dispatch_transition_end(&self, element_id: &str, property_name: &str) {
        let observers = take_transition_observers(element_id);
        for mut obs in observers {
            (obs.callback)(crate::dom::observer::TransitionLifecycleEvent::TransitionEnd {
                property_name: property_name.to_string(),
            });
        }
    }

    /// Dispatches a transition cancellation signal to registered transition observers.
    pub fn dispatch_transition_cancel(&self, element_id: &str, property_name: &str) {
        let observers = take_transition_observers(element_id);
        for mut obs in observers {
            (obs.callback)(crate::dom::observer::TransitionLifecycleEvent::TransitionCancel {
                property_name: property_name.to_string(),
            });
        }
    }

    /// Dispatches a timeout fallback signal to registered transition observers.
    pub fn dispatch_transition_fallback(&self, element_id: &str) {
        let observers = take_transition_observers(element_id);
        for mut obs in observers {
            (obs.callback)(crate::dom::observer::TransitionLifecycleEvent::TimeoutFallback);
        }
    }

    /// Dispatches a form reset signal to registered form reset observers.
    pub fn dispatch_form_reset(&self, form_element_id: &str) {
        let Some(base) = self.get_base() else {
            return;
        };
        let Ok(base_ref) = base.try_borrow() else {
            return;
        };
        let form_node_id = base_ref.get_element_by_id(form_element_id);

        let handles = CURRENT_OBSERVERS.with(|cell| {
            let Ok(reg) = cell.try_borrow() else {
                return Vec::new();
            };
            let mut matching = Vec::new();
            for obs in reg.form_reset_observers.values() {
                if let Some(expected_form_id) = &obs.form_id {
                    if expected_form_id == form_element_id {
                        matching.push(obs.callback.clone());
                    }
                } else if let Some(fnid) = form_node_id
                    && let Some(el_nid) = base_ref.get_element_by_id(&obs.element_id) {
                        let chain = base_ref.node_chain(el_nid);
                        if chain.contains(&fnid) {
                            matching.push(obs.callback.clone());
                        }
                    }
            }
            matching
        });
        drop(base_ref);

        for handle in handles {
            if let Ok(mut cb) = handle.try_borrow_mut() {
                cb();
            }
        }
    }

    /// Dispatches a scroll event to all registered scroll observers.
    pub fn dispatch_scroll(&self) {
        let handles = CURRENT_OBSERVERS.with(|cell| {
            let Ok(reg) = cell.try_borrow() else {
                return Vec::new();
            };
            reg.scroll_observers.values().cloned().collect::<Vec<_>>()
        });
        for handle in handles {
            if let Ok(mut cb) = handle.try_borrow_mut() {
                cb();
            }
        }
    }

    /// Dispatches an element scroll event to all registered observers for the element ID.
    pub fn dispatch_element_scroll(&self, element_id: &str) {
        let handles = CURRENT_OBSERVERS.with(|cell| {
            let Ok(reg) = cell.try_borrow() else {
                return Vec::new();
            };
            reg.element_scroll_observers
                .get(element_id)
                .map(|observers| observers.iter().map(|(_, cb)| cb.clone()).collect::<Vec<_>>())
                .unwrap_or_default()
        });
        for handle in handles {
            if let Ok(mut cb) = handle.try_borrow_mut() {
                cb();
            }
        }
    }

    /// Dispatches an element scroll event by looking up the element's DOM ID from its `NodeId`.
    pub fn dispatch_element_scroll_by_node_id(&self, node_id: blitz_dom::NodeId) {
        let Some(base) = self.get_base() else {
            return;
        };
        let b = base.borrow();
        if let Some(node) = b.get_node(node_id)
            && let Some(el) = node.element_data()
                && let Some(id) = &el.id {
                    let id_str = id.to_string();
                    drop(b);
                    self.dispatch_element_scroll(&id_str);
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
        let base_rc = self
            .get_base()
            .ok_or_else(|| crate::error::HostError::Unsupported("No active native document".into()))?;
        let base = base_rc.borrow();
        let root_node_id = base
            .get_element_by_id(root_id)
            .or_else(|| base.query_selector(&format!("#{root_id}")).ok().flatten())
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
                if let Some(el) = node.element_data()
                    && let Some(id) = el.id.as_deref()
                        && candidate_set.remove(id) {
                            ordered_ids.push(id.to_string());
                            if candidate_set.is_empty() {
                                break;
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
    static IS_NOTIFYING_LAYOUT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static NEEDS_RE_EVALUATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

struct ResizeObserverEntry {
    target_id: String,
    last_rect: Option<crate::runtime::geometry::Rect>,
    callback: Rc<RefCell<Box<dyn FnMut(crate::dom::observer::ResizeEntry)>>>,
}

struct TransitionObserverEntry {
    target_id: String,
    callback: Box<dyn FnMut(crate::dom::observer::TransitionLifecycleEvent)>,
}

struct FormResetObserverEntry {
    element_id: String,
    form_id: Option<String>,
    callback: Rc<RefCell<Box<dyn FnMut()>>>,
}

#[derive(Default)]
struct NativeObserverRegistry {
    resize_observers: std::collections::HashMap<u64, ResizeObserverEntry>,
    capture_observers: std::collections::HashMap<u64, Rc<RefCell<Box<dyn FnMut(crate::dom::observer::CaptureEvent)>>>>,
    transition_observers: std::collections::HashMap<u64, TransitionObserverEntry>,
    form_reset_observers: std::collections::HashMap<u64, FormResetObserverEntry>,
    scroll_observers: std::collections::HashMap<u64, Rc<RefCell<Box<dyn FnMut()>>>>,
    element_scroll_observers: std::collections::HashMap<String, Vec<(u64, Rc<RefCell<Box<dyn FnMut()>>>)>>,
}

fn take_transition_observers(element_id: &str) -> Vec<TransitionObserverEntry> {
    CURRENT_OBSERVERS.with(|cell| {
        let Ok(mut reg) = cell.try_borrow_mut() else {
            return Vec::new();
        };
        let matching_ids: Vec<u64> = reg
            .transition_observers
            .iter()
            .filter(|(_, obs)| obs.target_id == element_id)
            .map(|(&id, _)| id)
            .collect();
        matching_ids
            .into_iter()
            .filter_map(|id| reg.transition_observers.remove(&id))
            .collect()
    })
}

pub(crate) fn observe_element_resize(
    element_id: &str,
    callback: Box<dyn FnMut(crate::dom::observer::ResizeEntry) + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    let sub_id = crate::internal::next_subscription_id();
    if let Some(doc) = Document::current()
        && doc.element_by_id(element_id).is_none() {
            return Err(crate::error::Error::ElementNotFound(element_id.to_string()));
        }
    let initial_rect = Document::current().and_then(|doc| doc.measure_rect(element_id));
    CURRENT_OBSERVERS.with(|cell| {
        cell.borrow_mut().resize_observers.insert(
            sub_id,
            ResizeObserverEntry {
                target_id: element_id.to_string(),
                last_rect: initial_rect,
                callback: Rc::new(RefCell::new(callback)),
            },
        );
    });

    Ok(crate::watcher_guard::WatcherGuard::with_cleanup(
        "native_resize_observer",
        sub_id,
        None,
        move || {
            CURRENT_OBSERVERS.with(|cell| {
                if let Ok(mut reg) = cell.try_borrow_mut() {
                    reg.resize_observers.remove(&sub_id);
                }
            });
        },
    ))
}

pub(crate) fn observe_capture_events(
    callback: Box<dyn FnMut(crate::dom::observer::CaptureEvent) + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    let sub_id = crate::internal::next_subscription_id();
    CURRENT_OBSERVERS.with(|cell| {
        cell.borrow_mut()
            .capture_observers
            .insert(sub_id, Rc::new(RefCell::new(callback)));
    });

    Ok(crate::watcher_guard::WatcherGuard::with_cleanup(
        "native_capture_observer",
        sub_id,
        None,
        move || {
            CURRENT_OBSERVERS.with(|cell| {
                if let Ok(mut reg) = cell.try_borrow_mut() {
                    reg.capture_observers.remove(&sub_id);
                }
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

    let task = if fallback_timeout_ms > 0 {
        crate::internal::safe_spawn(async move {
            futures_timer::Delay::new(std::time::Duration::from_millis(fallback_timeout_ms)).await;
            let entry = CURRENT_OBSERVERS.with(|cell| {
                if let Ok(mut reg) = cell.try_borrow_mut() {
                    reg.transition_observers.remove(&sub_id)
                } else {
                    None
                }
            });
            if let Some(mut obs) = entry {
                (obs.callback)(crate::dom::observer::TransitionLifecycleEvent::TimeoutFallback);
            }
        })
    } else {
        None
    };

    Ok(crate::watcher_guard::WatcherGuard::with_cleanup(
        "native_transition_lifecycle",
        sub_id,
        task,
        move || {
            CURRENT_OBSERVERS.with(|cell| {
                if let Ok(mut reg) = cell.try_borrow_mut() {
                    reg.transition_observers.remove(&sub_id);
                }
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
                callback: Rc::new(RefCell::new(callback)),
            },
        );
    });

    Ok(crate::watcher_guard::WatcherGuard::with_cleanup(
        "native_form_reset_observer",
        sub_id,
        None,
        move || {
            CURRENT_OBSERVERS.with(|cell| {
                if let Ok(mut reg) = cell.try_borrow_mut() {
                    reg.form_reset_observers.remove(&sub_id);
                }
            });
        },
    ))
}

pub(crate) fn observe_scroll(
    callback: Box<dyn FnMut() + 'static>,
) -> crate::error::Result<crate::watcher_guard::WatcherGuard> {
    let sub_id = crate::internal::next_subscription_id();
    CURRENT_OBSERVERS.with(|cell| {
        cell.borrow_mut()
            .scroll_observers
            .insert(sub_id, Rc::new(RefCell::new(callback)));
    });

    Ok(crate::watcher_guard::WatcherGuard::with_cleanup(
        "native_scroll_observer",
        sub_id,
        None,
        move || {
            CURRENT_OBSERVERS.with(|cell| {
                if let Ok(mut reg) = cell.try_borrow_mut() {
                    reg.scroll_observers.remove(&sub_id);
                }
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

    if let Some(doc) = Document::current()
        && doc.element_by_id(element_id).is_none() {
            return Err(crate::error::Error::ElementNotFound(element_id.to_string()));
        }

    let el_id_clone = el_id.clone();
    CURRENT_OBSERVERS.with(|cell| {
        cell.borrow_mut()
            .element_scroll_observers
            .entry(el_id)
            .or_default()
            .push((sub_id, Rc::new(RefCell::new(callback))));
    });

    Ok(crate::watcher_guard::WatcherGuard::with_cleanup(
        "native_element_scroll_observer",
        sub_id,
        None,
        move || {
            CURRENT_OBSERVERS.with(|cell| {
                if let Ok(mut reg) = cell.try_borrow_mut()
                    && let Some(observers) = reg.element_scroll_observers.get_mut(&el_id_clone) {
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
            let Ok(reg) = cell.try_borrow() else {
                return 0;
            };
            if let Some(id) = element_id {
                reg.element_scroll_observers.get(id).map_or(0, |list| list.len())
            } else {
                reg.element_scroll_observers.values().map(|list| list.len()).sum()
            }
        })
    }
}


