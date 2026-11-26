//! Document dismiss watcher for popovers, dialogs, selects, and tooltips.
//!
//! Intercepts pointerdown, click, focusin, and Escape key interactions with boundary ID
//! suppression, frame-0 opening self-dismiss protection, mouse/pen pointerdown/click
//! deduplication, touch-tap `< 10.0px` capture reconciliation, and untrusted click
//! microtask deferral.

use crate::dom::observer::{
    CaptureEventKind, CaptureEventMask, observe_capture_events_with_mask,
};
use crate::error::Result;
use crate::watcher_guard::WatcherGuard;
use serde::{Deserialize, Serialize};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Event emitted when an outside interaction or escape key dismissal occurs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DismissEvent {
    PointerDown {
        #[serde(rename = "pathIds", default)]
        path_ids: Vec<String>,
    },
    Click {
        #[serde(rename = "pathIds", default)]
        path_ids: Vec<String>,
    },
    FocusIn {
        #[serde(rename = "pathIds", default)]
        path_ids: Vec<String>,
    },
    Escape,
}

impl DismissEvent {
    /// Checks whether this event occurred outside all provided boundary element IDs.
    pub fn is_outside_boundary(&self, boundary_ids: &[&str]) -> bool {
        match self {
            Self::PointerDown { path_ids }
            | Self::Click { path_ids }
            | Self::FocusIn { path_ids } => {
                if boundary_ids.is_empty() {
                    return true;
                }
                !path_ids.iter().any(|id| boundary_ids.contains(&id.as_str()))
            }
            Self::Escape => true,
        }
    }

    /// Returns the outermost target element ID if present.
    pub fn target_id(&self) -> Option<&str> {
        match self {
            Self::PointerDown { path_ids }
            | Self::Click { path_ids }
            | Self::FocusIn { path_ids } => path_ids.first().map(|s| s.as_str()),
            Self::Escape => None,
        }
    }
}

/// Backward-compatible alias for `DismissEvent`.
pub type DocumentDismissEvent = DismissEvent;

/// Configuration options for the document dismiss watcher.
#[derive(Clone, Debug, Default)]
pub struct DismissOptions {
    pub boundary_ids: Vec<String>,
    pub listen_escape: bool,
    pub listen_pointer: bool,
    pub listen_focusin: bool,
}

impl DismissOptions {
    /// Creates a new `DismissOptions` registered to the given boundary IDs.
    pub fn new(boundary_ids: &[&str]) -> Self {
        Self {
            boundary_ids: boundary_ids.iter().map(|s| s.to_string()).collect(),
            listen_escape: true,
            listen_pointer: true,
            listen_focusin: true,
        }
    }
}

#[derive(Clone, Debug)]
struct PendingTouchTap {
    client_x: f64,
    client_y: f64,
    path_ids: Vec<String>,
}

/// Watches global document interactions and emits dismissal signals when interactions occur outside boundaries.
pub fn watch_document_dismiss(
    boundary_ids: &[&str],
    on_event: impl FnMut(DismissEvent) + 'static,
) -> Result<WatcherGuard> {
    watch_document_dismiss_with_options(DismissOptions::new(boundary_ids), on_event)
}

/// Watches document dismissal with explicit configuration options.
pub fn watch_document_dismiss_with_options(
    options: DismissOptions,
    on_event: impl FnMut(DismissEvent) + 'static,
) -> Result<WatcherGuard> {
    let boundary_ids = options.boundary_ids;
    let listen_escape = options.listen_escape;
    let listen_pointer = options.listen_pointer;
    let listen_focusin = options.listen_focusin;

    let active = Rc::new(Cell::new(true));
    let on_event_cell: Rc<RefCell<Box<dyn FnMut(DismissEvent)>>> =
        Rc::new(RefCell::new(Box::new(on_event)));

    let mut has_seen_pointer_down = false;
    let mut dedup_next_pointer_click = false;
    let mut suppress_inside_press_click = false;
    let mut pending_touch_tap: Option<PendingTouchTap> = None;
    let mut pending_pointer_focus_path: Option<Vec<String>> = None;

    let active_for_cb = active.clone();
    let on_event_for_cb = on_event_cell.clone();

    let mut mask = CaptureEventMask::NONE;
    if listen_pointer {
        mask |= CaptureEventMask::POINTER_DOWN | CaptureEventMask::CLICK;
    }
    if listen_focusin {
        mask |= CaptureEventMask::FOCUS_IN;
    }
    if listen_escape {
        mask |= CaptureEventMask::KEY_DOWN;
    }

    let inner_guard = observe_capture_events_with_mask(mask, move |event| {
        if !active_for_cb.get() {
            return;
        }
        match event.kind {
            CaptureEventKind::PointerDown {
                pointer_type,
                client_x,
                client_y,
            } => {
                has_seen_pointer_down = true;
                // Settle/reset any previous in-flight outside pointer or touch state (`cf522ba2`).
                pending_touch_tap = None;
                dedup_next_pointer_click = false;
                suppress_inside_press_click = false;

                if !listen_pointer {
                    pending_pointer_focus_path = None;
                    return;
                }
                let is_inside = event.path_ids.iter().any(|id| boundary_ids.contains(id));
                if is_inside {
                    pending_pointer_focus_path = None;
                    suppress_inside_press_click = true;
                } else {
                    pending_pointer_focus_path = Some(event.path_ids.clone());
                    if pointer_type.eq_ignore_ascii_case("touch") {
                        pending_touch_tap = Some(PendingTouchTap {
                            client_x,
                            client_y,
                            path_ids: event.path_ids,
                        });
                    } else {
                        dedup_next_pointer_click = true;
                        if let Ok(mut cb) = on_event_for_cb.try_borrow_mut() {
                            cb(DismissEvent::PointerDown {
                                path_ids: event.path_ids,
                            });
                        }
                    }
                }
            }
            CaptureEventKind::Click {
                is_trusted,
                detail,
                client_x,
                client_y,
            } => {
                pending_pointer_focus_path = None;
                let was_inside_press = std::mem::replace(&mut suppress_inside_press_click, false);
                let was_deduped_mouse = std::mem::replace(&mut dedup_next_pointer_click, false);
                let touch_tap = pending_touch_tap.take();

                if !listen_pointer {
                    return;
                }

                if was_inside_press {
                    return;
                }

                if was_deduped_mouse && is_trusted && detail > 0 {
                    return;
                }

                if let Some(tap) = touch_tap {
                    let dist = (client_x - tap.client_x).hypot(client_y - tap.client_y);
                    if dist < 10.0 {
                        let path_ids = if event.path_ids.is_empty() {
                            tap.path_ids
                        } else {
                            event.path_ids
                        };
                        let is_inside = path_ids.iter().any(|id| boundary_ids.contains(id));
                        if !is_inside
                            && let Ok(mut cb) = on_event_for_cb.try_borrow_mut()
                        {
                            cb(DismissEvent::Click { path_ids });
                        }
                    }
                    return;
                }

                // Frame-0 / opening-press self-dismiss guard (`d715acbf`):
                // A trusted pointer click (`detail > 0`) arriving before any capture-phase
                // `PointerDown` was observed belongs to the press that mounted this watcher.
                if !has_seen_pointer_down && is_trusted && detail > 0 {
                    has_seen_pointer_down = true;
                    return;
                }

                let is_inside = event.path_ids.iter().any(|id| boundary_ids.contains(id));
                if is_inside {
                    return;
                }

                let path_ids = event.path_ids;
                if !is_trusted {
                    // Programmatic / untrusted clicks defer to a microtask so synchronous
                    // `.click()` calls inside an active render/commit never conflict (`6c24e5f4`),
                    // aborting cleanly if the watcher tears down first (`723afd00`).
                    #[cfg(target_arch = "wasm32")]
                    {
                        let active_check = active_for_cb.clone();
                        let cb_deferred = on_event_for_cb.clone();
                        let path_ids_clone = path_ids.clone();
                        if crate::internal::safe_spawn(async move {
                            if active_check.get()
                                && let Ok(mut cb) = cb_deferred.try_borrow_mut()
                            {
                                cb(DismissEvent::Click {
                                    path_ids: path_ids_clone,
                                });
                            }
                        })
                        .is_none()
                        {
                            use wasm_bindgen::JsCast;
                            use wasm_bindgen::closure::Closure;
                            let active_js = active_for_cb.clone();
                            let cb_js = on_event_for_cb.clone();
                            let holder: Rc<
                                RefCell<Option<Closure<dyn Fn(wasm_bindgen::JsValue)>>>,
                            > = Rc::new(RefCell::new(None));
                            let holder_cleanup = holder.clone();
                            let closure = Closure::<dyn Fn(wasm_bindgen::JsValue)>::new(
                                move |_val: wasm_bindgen::JsValue| {
                                    let _held = holder_cleanup
                                        .try_borrow_mut()
                                        .ok()
                                        .and_then(|mut c| c.take());
                                    if active_js.get()
                                        && let Ok(mut cb) = cb_js.try_borrow_mut()
                                    {
                                        cb(DismissEvent::Click {
                                            path_ids: path_ids.clone(),
                                        });
                                    }
                                },
                            );
                            let promise =
                                js_sys::Promise::resolve(&wasm_bindgen::JsValue::UNDEFINED);
                            if let Ok(then_val) = js_sys::Reflect::get(
                                &promise,
                                &wasm_bindgen::JsValue::from_str("then"),
                            ) && let Ok(then_fn) = then_val.dyn_into::<js_sys::Function>()
                            {
                                let _ = then_fn.call1(&promise, closure.as_ref());
                            }
                            if let Ok(mut h) = holder.try_borrow_mut() {
                                *h = Some(closure);
                            }
                        }
                        return;
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        if dioxus::core::Runtime::try_current().is_some() {
                            let active_check = active_for_cb.clone();
                            let cb_deferred = on_event_for_cb.clone();
                            let path_ids_clone = path_ids.clone();
                            if crate::internal::safe_spawn(async move {
                                if active_check.get()
                                    && let Ok(mut cb) = cb_deferred.try_borrow_mut()
                                {
                                    cb(DismissEvent::Click {
                                        path_ids: path_ids_clone,
                                    });
                                }
                            })
                            .is_some()
                            {
                                return;
                            }
                        }
                    }
                }

                if let Ok(mut cb) = on_event_for_cb.try_borrow_mut() {
                    cb(DismissEvent::Click { path_ids });
                }
            }
            CaptureEventKind::FocusIn => {
                let suppressed_by_pointer = pending_pointer_focus_path
                    .take()
                    .and_then(|ptr_path| {
                        event
                            .path_ids
                            .first()
                            .map(|focus_target| ptr_path.contains(focus_target))
                    })
                    .unwrap_or(false);
                if suppressed_by_pointer || !listen_focusin {
                    return;
                }
                let is_inside = event.path_ids.iter().any(|id| boundary_ids.contains(id));
                if !is_inside
                    && let Ok(mut cb) = on_event_for_cb.try_borrow_mut()
                {
                    cb(DismissEvent::FocusIn {
                        path_ids: event.path_ids,
                    });
                }
            }
            CaptureEventKind::KeyDown { key, is_composing } => {
                pending_pointer_focus_path = None;
                pending_touch_tap = None;
                if !listen_escape || is_composing {
                    return;
                }
                if (key == "Escape" || key == "Esc")
                    && let Ok(mut cb) = on_event_for_cb.try_borrow_mut()
                {
                    cb(DismissEvent::Escape);
                }
            }
        }
    })?;

    let sub_id = inner_guard.subscription_id();
    Ok(WatcherGuard::with_cleanup(
        "watch_document_dismiss",
        sub_id,
        None,
        move || {
            active.set(false);
            inner_guard.stop();
        },
    ))
}
