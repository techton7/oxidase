//! Web (WASM) frame driver implementation backed by browser `requestAnimationFrame`.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use super::types::{FrameInfo, FrameLoopError, FrameLoopGuard, FrameRequestGuard};

/// Starts an ongoing animation frame loop driven by `window.requestAnimationFrame`.
pub fn start_frame_loop(
    mut on_frame: impl FnMut(FrameInfo) + 'static,
) -> Result<FrameLoopGuard, FrameLoopError> {
    let window = web_sys::window().ok_or(FrameLoopError::UnsupportedHost)?;
    let performance = window.performance();

    let running = Rc::new(RefCell::new(true));
    let current_raf_id: Rc<RefCell<Option<i32>>> = Rc::new(RefCell::new(None));

    let start_ms = performance.as_ref().map(|p| p.now()).unwrap_or(0.0);
    let last_time_ms = Rc::new(RefCell::new(start_ms));

    type RafClosure = Closure<dyn FnMut(f64)>;
    let closure_holder: Rc<RefCell<Option<RafClosure>>> = Rc::new(RefCell::new(None));
    let closure_clone = closure_holder.clone();

    let running_flag = running.clone();
    let raf_id_cell = current_raf_id.clone();

    *closure_clone.borrow_mut() = Some(Closure::wrap(Box::new(move |timestamp: f64| {
        if !*running_flag.borrow() {
            return;
        }

        let prev = *last_time_ms.borrow();
        let dt_ms = if prev > 0.0 { (timestamp - prev).max(0.0) } else { 0.0 };
        *last_time_ms.borrow_mut() = timestamp;

        let info = FrameInfo {
            now: Duration::from_secs_f64(timestamp / 1000.0),
            delta: Duration::from_secs_f64(dt_ms / 1000.0),
        };

        on_frame(info);

        if *running_flag.borrow() {
            if let Some(w) = web_sys::window() {
                if let Some(ref cb) = *closure_holder.borrow() {
                    if let Ok(id) = w.request_animation_frame(cb.as_ref().unchecked_ref()) {
                        *raf_id_cell.borrow_mut() = Some(id);
                    }
                }
            }
        }
    }) as Box<dyn FnMut(f64)>));

    if let Some(ref cb) = *closure_clone.borrow() {
        match window.request_animation_frame(cb.as_ref().unchecked_ref()) {
            Ok(id) => {
                *current_raf_id.borrow_mut() = Some(id);
            }
            Err(_) => return Err(FrameLoopError::RuntimeUnavailable),
        }
    }

    let cancel_running = running.clone();
    let cancel_raf_id = current_raf_id.clone();
    let cancel_closure = closure_clone.clone();

    Ok(FrameLoopGuard::new(move || {
        *cancel_running.borrow_mut() = false;
        if let Some(id) = cancel_raf_id.borrow_mut().take() {
            if let Some(w) = web_sys::window() {
                let _ = w.cancel_animation_frame(id);
            }
        }
        let _ = cancel_closure.borrow_mut().take();
    }))
}

/// Requests a single execution of the callback on the next animation frame.
pub fn request_next_frame(
    on_frame: impl FnOnce(Duration) + 'static,
) -> Result<FrameRequestGuard, FrameLoopError> {
    let window = web_sys::window().ok_or(FrameLoopError::UnsupportedHost)?;

    let current_raf_id: Rc<RefCell<Option<i32>>> = Rc::new(RefCell::new(None));
    let on_frame_opt = Rc::new(RefCell::new(Some(on_frame)));

    let raf_id_cell = current_raf_id.clone();
    let on_frame_cell = on_frame_opt.clone();

    type RafClosure = Closure<dyn FnMut(f64)>;
    let closure_holder: Rc<RefCell<Option<RafClosure>>> = Rc::new(RefCell::new(None));
    let closure_clone = closure_holder.clone();

    *closure_clone.borrow_mut() = Some(Closure::wrap(Box::new(move |timestamp: f64| {
        let _ = raf_id_cell.borrow_mut().take();
        let _ = closure_holder.borrow_mut().take();
        if let Some(cb) = on_frame_cell.borrow_mut().take() {
            cb(Duration::from_secs_f64(timestamp / 1000.0));
        }
    }) as Box<dyn FnMut(f64)>));

    if let Some(ref cb) = *closure_clone.borrow() {
        match window.request_animation_frame(cb.as_ref().unchecked_ref()) {
            Ok(id) => {
                *current_raf_id.borrow_mut() = Some(id);
            }
            Err(_) => return Err(FrameLoopError::RuntimeUnavailable),
        }
    }

    let cancel_raf_id = current_raf_id.clone();
    let cancel_closure = closure_clone.clone();
    let cancel_cb = on_frame_opt.clone();

    Ok(FrameRequestGuard::new(move || {
        let _ = cancel_cb.borrow_mut().take();
        if let Some(id) = cancel_raf_id.borrow_mut().take() {
            if let Some(w) = web_sys::window() {
                let _ = w.cancel_animation_frame(id);
            }
        }
        let _ = cancel_closure.borrow_mut().take();
    }))
}

/// In a hosted browser environment driven by `requestAnimationFrame`, manual ticking is a safe no-op.
pub fn tick(_dt: Duration) {}
