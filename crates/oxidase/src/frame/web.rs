//! Web (WASM) frame driver implementation backed by browser `requestAnimationFrame`.

use std::cell::RefCell;
use std::time::Duration;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use super::raf_registry::RafFrameRegistry;
use super::types::{FrameInfo, FrameLoopError, FrameLoopGuard, FrameRequestGuard};

thread_local! {
    static WEB_FRAME_STATE: RefCell<RafFrameRegistry> = RefCell::new(RafFrameRegistry::default());
    static WEB_RAF_CLOSURE: Closure<dyn Fn(f64)> = Closure::wrap(Box::new(|timestamp_ms: f64| {
        on_animation_frame(timestamp_ms);
    }) as Box<dyn Fn(f64)>);
}

fn cancel_browser_raf(raf_id: Option<i32>) {
    if let Some(id) = raf_id
        && let Some(window) = web_sys::window()
    {
        let _ = window.cancel_animation_frame(id);
    }
}

fn ensure_raf_scheduled(window: &web_sys::Window) -> Result<(), FrameLoopError> {
    let needs_schedule = WEB_FRAME_STATE.with(|cell| {
        let state = cell.borrow();
        state.scheduled_raf_id().is_none() && state.has_pending_frames()
    });
    if !needs_schedule {
        return Ok(());
    }

    let raf_id = WEB_RAF_CLOSURE
        .with(|cb| window.request_animation_frame(cb.as_ref().unchecked_ref()))
        .map_err(|_| FrameLoopError::RuntimeUnavailable)?;

    WEB_FRAME_STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        if state.has_pending_frames() && state.scheduled_raf_id().is_none() {
            state.set_scheduled_raf_id(Some(raf_id));
        } else {
            drop(state);
            let _ = window.cancel_animation_frame(raf_id);
        }
    });

    Ok(())
}

fn on_animation_frame(timestamp_ms: f64) {
    let cancel_raf_id =
        WEB_FRAME_STATE.with(|cell| RafFrameRegistry::dispatch_step(cell, timestamp_ms));
    cancel_browser_raf(cancel_raf_id);

    if let Some(window) = web_sys::window() {
        let _ = ensure_raf_scheduled(&window);
    }
}

/// Starts an ongoing animation frame loop driven by `window.requestAnimationFrame`.
pub fn start_frame_loop(
    on_frame: impl FnMut(FrameInfo) + 'static,
) -> Result<FrameLoopGuard, FrameLoopError> {
    let window = web_sys::window().ok_or(FrameLoopError::UnsupportedHost)?;

    let (id, active) = WEB_FRAME_STATE.with(|cell| cell.borrow_mut().register_loop(on_frame));

    if let Err(err) = ensure_raf_scheduled(&window) {
        active.set(false);
        let (removed, cancel_raf_id) =
            WEB_FRAME_STATE.with(|cell| cell.borrow_mut().cancel_loop(id));
        drop(removed);
        cancel_browser_raf(cancel_raf_id);
        return Err(err);
    }

    Ok(FrameLoopGuard::new(move || {
        active.set(false);
        let (removed, cancel_raf_id) = WEB_FRAME_STATE.with(|cell| {
            if let Ok(mut state) = cell.try_borrow_mut() {
                state.cancel_loop(id)
            } else {
                (None, None)
            }
        });
        drop(removed);
        cancel_browser_raf(cancel_raf_id);
    }))
}

/// Requests a single execution of the callback on the next animation frame.
pub fn request_next_frame(
    on_frame: impl FnOnce(FrameInfo) + 'static,
) -> Result<FrameRequestGuard, FrameLoopError> {
    let window = web_sys::window().ok_or(FrameLoopError::UnsupportedHost)?;

    let (id, active, callback) =
        WEB_FRAME_STATE.with(|cell| cell.borrow_mut().register_one_shot(on_frame));

    if let Err(err) = ensure_raf_scheduled(&window) {
        active.set(false);
        if let Ok(mut cb) = callback.try_borrow_mut() {
            let _ = cb.take();
        }
        let (removed, cancel_raf_id) =
            WEB_FRAME_STATE.with(|cell| cell.borrow_mut().cancel_one_shot(id));
        drop(removed);
        cancel_browser_raf(cancel_raf_id);
        return Err(err);
    }

    Ok(FrameRequestGuard::new(move || {
        active.set(false);
        if let Ok(mut cb) = callback.try_borrow_mut() {
            let _ = cb.take();
        }
        let (removed, cancel_raf_id) = WEB_FRAME_STATE.with(|cell| {
            if let Ok(mut state) = cell.try_borrow_mut() {
                state.cancel_one_shot(id)
            } else {
                (None, None)
            }
        });
        drop(removed);
        cancel_browser_raf(cancel_raf_id);
    }))
}

/// In a hosted browser environment driven by `requestAnimationFrame`, manual ticking is a safe no-op.
pub fn tick(_dt: Duration) {}

/// Guard for an active host redraw requester.
pub struct HostRedrawGuard;

/// In a browser environment, host redraw is implicitly handled by `requestAnimationFrame`.
pub fn set_host_redraw_requester(_requester: impl Fn() + 'static) -> HostRedrawGuard {
    HostRedrawGuard
}

/// In a browser environment, frame stepping is handled by the browser event loop.
pub fn step_hosted_frame() -> Duration {
    Duration::from_millis(16)
}

/// Returns whether any frame requests are pending.
pub fn has_pending_frames() -> bool {
    WEB_FRAME_STATE.with(|cell| cell.borrow().has_pending_frames())
}

/// Returns `true` when frames are driven automatically by an active browser host (`window`).
pub fn is_host_driven() -> bool {
    web_sys::window().is_some()
}
