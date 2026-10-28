//! Floating auto-update layout watcher for anchored overlay components.
//!
//! Monitors reference resizing and ancestor scrolling with substrate-level frame coalescing (at most 1 notification per 16ms render frame).

use crate::dom::observer::{observe_element_resize, observe_element_scroll, observe_scroll, ResizeEntry};
use crate::error::Result;
use crate::watcher_guard::WatcherGuard;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::rc::Rc;

struct CoalesceState {
    is_emitting: bool,
    pending_trailing: Option<FloatingAutoUpdateEvent>,
    trailing_guard: Option<crate::frame::FrameRequestGuard>,
    cancelled: bool,
}

#[inline]
fn merge_event(existing: &mut Option<FloatingAutoUpdateEvent>, incoming: FloatingAutoUpdateEvent) {
    match existing {
        Some(FloatingAutoUpdateEvent::Update) => {}
        _ => *existing = Some(incoming),
    }
}

struct EmittingGuard {
    state: Rc<RefCell<CoalesceState>>,
}

impl Drop for EmittingGuard {
    fn drop(&mut self) {
        if let Ok(mut s) = self.state.try_borrow_mut() {
            s.is_emitting = false;
        }
    }
}

fn schedule_trailing_frame(
    state: &Rc<RefCell<CoalesceState>>,
    handler: &Rc<RefCell<dyn FnMut(FloatingAutoUpdateEvent)>>,
) {
    let state_weak = Rc::downgrade(state);
    let handler_weak = Rc::downgrade(handler);

    let guard = match crate::frame::request_next_frame(move |_frame_info| {
        let Some(state_rc) = state_weak.upgrade() else {
            return;
        };
        let Some(handler_rc) = handler_weak.upgrade() else {
            return;
        };

        let trailing_event = {
            let mut s = state_rc.borrow_mut();
            if s.cancelled {
                s.trailing_guard = None;
                s.pending_trailing = None;
                return;
            }
            let Some(event) = s.pending_trailing.take() else {
                s.trailing_guard = None;
                return;
            };
            s.is_emitting = true;
            event
        };

        {
            let _guard = EmittingGuard {
                state: state_rc.clone(),
            };
            if let Ok(mut h) = handler_rc.try_borrow_mut() {
                h(trailing_event);
            }
        }

        let mut s = state_rc.borrow_mut();
        s.trailing_guard = None;
        if !s.cancelled && s.pending_trailing.is_some() {
            drop(s);
            schedule_trailing_frame(&state_rc, &handler_rc);
        }
    }) {
        Ok(guard) => guard,
        Err(e) => {
            tracing::warn!("Failed to schedule trailing frame for floating auto-update: {:?}", e);
            return;
        }
    };

    let mut s = state.borrow_mut();
    if s.cancelled {
        drop(guard);
    } else {
        s.trailing_guard = Some(guard);
    }
}

/// Notification payload for floating auto-update events.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FloatingAutoUpdateEvent {
    Scroll,
    Update,
}

/// Backward-compatible alias matching `FloatingAutoUpdateEvent`.
pub type FloatingAutoUpdatePayload = FloatingAutoUpdateEvent;

/// Configuration options for floating auto-update monitoring.
#[derive(Clone, Debug)]
pub struct FloatingAutoUpdateOptions {
    pub anchor_ids: Vec<String>,
    pub content_id: String,
    pub ancestor_scroll: bool,
    pub ancestor_resize: bool,
    pub element_resize: bool,
}

impl FloatingAutoUpdateOptions {
    /// Creates options tracking the specified anchor IDs and floating content ID.
    pub fn new(anchor_ids: &[&str], content_id: &str) -> Self {
        Self {
            anchor_ids: anchor_ids.iter().map(|s| s.to_string()).collect(),
            content_id: content_id.to_string(),
            ancestor_scroll: true,
            ancestor_resize: true,
            element_resize: true,
        }
    }
}

#[allow(dead_code)]
fn map_floating_error(err: oxidase_floating_ui::error::Error) -> crate::error::Error {
    match err {
        oxidase_floating_ui::error::Error::ElementNotFound(id) => {
            crate::error::Error::ElementNotFound(id)
        }
        oxidase_floating_ui::error::Error::Unsupported(msg) => {
            crate::error::Error::Unsupported(msg)
        }
        oxidase_floating_ui::error::Error::Timeout(msg) => {
            crate::error::Error::Timeout(msg)
        }
        oxidase_floating_ui::error::Error::MaxResetsExceeded(n) => {
            crate::error::Error::Unsupported(format!("Max resets exceeded: {n}"))
        }
        oxidase_floating_ui::error::Error::Other(msg) => {
            crate::error::Error::Unsupported(msg)
        }
    }
}

/// Discovers scrollable/clipping ancestor element IDs for the specified element ID across host platforms.
#[cfg(target_arch = "wasm32")]
pub fn get_overflow_ancestors_for_element(id: &str) -> Result<Vec<String>> {
    let platform = oxidase_floating_ui::platform::web::WebFloatingPlatform;
    platform.get_overflow_ancestors(id).map_err(map_floating_error)
}

/// Discovers scrollable/clipping ancestor element IDs for the specified element ID across host platforms.
#[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
pub fn get_overflow_ancestors_for_element(id: &str) -> Result<Vec<String>> {
    let doc = crate::dom::Document::current()
        .ok_or_else(|| crate::error::Error::ElementNotFound(id.to_string()))?;
    let platform = oxidase_floating_ui::platform::native::NativeFloatingPlatform::new(doc.base().clone());
    platform.get_overflow_ancestors(id).map_err(map_floating_error)
}

/// Discovers scrollable/clipping ancestor element IDs for the specified element ID across host platforms.
#[cfg(all(not(target_arch = "wasm32"), not(feature = "native")))]
pub fn get_overflow_ancestors_for_element(id: &str) -> Result<Vec<String>> {
    Err(crate::error::Error::ElementNotFound(id.to_string()))
}

/// Watches floating element updates across anchor and content elements.
pub fn watch_floating_auto_update(
    anchor_ids: &[&str],
    content_id: &str,
    on_event: impl FnMut(FloatingAutoUpdateEvent) + 'static,
) -> Result<WatcherGuard> {
    watch_floating_auto_update_with_options(
        FloatingAutoUpdateOptions::new(anchor_ids, content_id),
        on_event,
    )
}

/// Watches floating element updates with explicit options.
pub fn watch_floating_auto_update_with_options(
    options: FloatingAutoUpdateOptions,
    on_event: impl FnMut(FloatingAutoUpdateEvent) + 'static,
) -> Result<WatcherGuard> {
    let sub_id = crate::internal::next_subscription_id();

    let state = Rc::new(RefCell::new(CoalesceState {
        is_emitting: false,
        pending_trailing: None,
        trailing_guard: None,
        cancelled: false,
    }));

    let handler: Rc<RefCell<dyn FnMut(FloatingAutoUpdateEvent)>> = Rc::new(RefCell::new(on_event));
    let mut inner_guards = Vec::new();

    let emit_throttled = {
        let state = state.clone();
        let handler = handler.clone();
        Rc::new(move |kind: FloatingAutoUpdateEvent| {
            let mut s = state.borrow_mut();
            if s.cancelled {
                return;
            }
            merge_event(&mut s.pending_trailing, kind);
            if s.trailing_guard.is_none() {
                drop(s);
                schedule_trailing_frame(&state, &handler);
            }
        })
    };

    // 1. Ancestor scroll observation
    let mut deduplicated_ancestor_ids = Vec::new();
    if options.ancestor_scroll {
        let mut all_ancestor_ids = Vec::new();

        // Fail-fast if any anchor element is missing
        for anchor_id in &options.anchor_ids {
            let ancestors = get_overflow_ancestors_for_element(anchor_id)?;
            all_ancestor_ids.extend(ancestors);
        }

        // Tolerate missing content element non-fatally (e.g. content unmounted)
        match get_overflow_ancestors_for_element(&options.content_id) {
            Ok(ancestors) => all_ancestor_ids.extend(ancestors),
            Err(crate::error::Error::ElementNotFound(_)) => {}
            Err(e) => return Err(e),
        }

        // Deduplicate discovered ancestor IDs while preserving order
        let mut seen = std::collections::HashSet::new();
        for id in all_ancestor_ids {
            if seen.insert(id.clone()) {
                deduplicated_ancestor_ids.push(id);
            }
        }

        // Register global viewport scroll observer
        let emit = emit_throttled.clone();
        let scroll_guard = observe_scroll(move || {
            emit(FloatingAutoUpdateEvent::Scroll);
        })?;
        inner_guards.push(scroll_guard);

        // Register element scroll observers on each deduplicated ancestor
        for anc_id in &deduplicated_ancestor_ids {
            let emit = emit_throttled.clone();
            let el_scroll_guard = observe_element_scroll(anc_id, move || {
                emit(FloatingAutoUpdateEvent::Scroll);
            })?;
            inner_guards.push(el_scroll_guard);
        }
    }

    // 2. Element resize observation
    if options.element_resize {
        // Register anchor resize observers (fail-fast on missing anchor)
        for anchor_id in &options.anchor_ids {
            let emit = emit_throttled.clone();
            let guard = observe_element_resize(anchor_id, move |_entry: ResizeEntry| {
                emit(FloatingAutoUpdateEvent::Update);
            })?;
            inner_guards.push(guard);
        }

        // Register content resize observer (tolerate missing content)
        let emit = emit_throttled.clone();
        match observe_element_resize(&options.content_id, move |_entry: ResizeEntry| {
            emit(FloatingAutoUpdateEvent::Update);
        }) {
            Ok(guard) => inner_guards.push(guard),
            Err(crate::error::Error::ElementNotFound(_)) => {}
            Err(e) => return Err(e),
        }
    }

    // 3. Ancestor resize observation (if requested)
    if options.ancestor_resize && !deduplicated_ancestor_ids.is_empty() {
        for anc_id in &deduplicated_ancestor_ids {
            let emit = emit_throttled.clone();
            if let Ok(guard) = observe_element_resize(anc_id, move |_entry: ResizeEntry| {
                emit(FloatingAutoUpdateEvent::Update);
            }) {
                inner_guards.push(guard);
            }
        }
    }

    // 4. Return combined WatcherGuard collecting all inner guards
    let state_teardown = state.clone();
    Ok(WatcherGuard::with_cleanup(
        "watch_floating_auto_update",
        sub_id,
        None,
        move || {
            {
                let mut s = state_teardown.borrow_mut();
                s.cancelled = true;
                s.trailing_guard.take();
                s.pending_trailing = None;
            }
            for guard in inner_guards {
                guard.stop();
            }
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_merge_semantics_update_dominates_scroll() {
        let mut evt = None;
        merge_event(&mut evt, FloatingAutoUpdateEvent::Scroll);
        assert_eq!(evt, Some(FloatingAutoUpdateEvent::Scroll));

        merge_event(&mut evt, FloatingAutoUpdateEvent::Update);
        assert_eq!(evt, Some(FloatingAutoUpdateEvent::Update));

        // Update dominates subsequent Scroll
        merge_event(&mut evt, FloatingAutoUpdateEvent::Scroll);
        assert_eq!(evt, Some(FloatingAutoUpdateEvent::Update));

        // Update with Update remains Update
        merge_event(&mut evt, FloatingAutoUpdateEvent::Update);
        assert_eq!(evt, Some(FloatingAutoUpdateEvent::Update));
    }

    #[test]
    fn test_emitting_guard_drop_resets_flag() {
        let state = Rc::new(RefCell::new(CoalesceState {
            is_emitting: true,
            pending_trailing: None,
            trailing_guard: None,
            cancelled: false,
        }));

        {
            let _guard = EmittingGuard {
                state: state.clone(),
            };
            assert!(state.borrow().is_emitting);
        }
        assert!(!state.borrow().is_emitting);
    }
}
