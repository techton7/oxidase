//! Non-WASM / Native frame driver implementation.
//!
//! Provides a unified, 2-tier native frame architecture:
//!
//! 1. **Hosted Native Mode (Host/Event-Loop-Driven)**:
//!    - Anchored to the real host redraw seam (`winit` / `blitz-shell` window redraw cadence).
//!    - When subscribers exist, requests redraws via [`set_host_redraw_requester`].
//!    - When host redraw occurs (`WindowEvent::RedrawRequested`), [`step_hosted_frame`] measures real
//!      elapsed time, advances monotonic clock, and dispatches callbacks on the UI thread.
//!    - Automatically idles (zero redraw requests, zero CPU waste) when no subscribers are active.
//!
//! 2. **Manual / Headless Mode (Deterministic Simulation & Tests)**:
//!    - `tick(dt)` advances monotonic clock explicitly by `dt` without host dependency.
//!    - Preserves 100% deterministic test-time execution, one-shot execution/cancellation, and multi-subscriber ticking.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use super::types::{FrameInfo, FrameLoopError, FrameLoopGuard, FrameRequestGuard};

struct LoopSubscriber {
    active: Rc<Cell<bool>>,
    callback: Rc<RefCell<Box<dyn FnMut(FrameInfo)>>>,
}

struct OneShotRequest {
    active: Rc<Cell<bool>>,
    callback: Rc<RefCell<Option<Box<dyn FnOnce(Duration)>>>>,
}

/// Guard for an active host redraw requester.
/// Dropping this guard unregisters the requester, cleanly detaching the host redraw link.
pub struct HostRedrawGuard {
    active: Rc<Cell<bool>>,
}

impl HostRedrawGuard {
    fn new(active: Rc<Cell<bool>>) -> Self {
        Self { active }
    }
}

impl Drop for HostRedrawGuard {
    fn drop(&mut self) {
        self.active.set(false);
        FRAME_STATE.with(|cell| {
            if let Ok(mut state) = cell.try_borrow_mut() {
                state.redraw_requester = None;
                state.last_real_time = None;
            }
        });
    }
}

#[derive(Default)]
struct NativeFrameState {
    now: Duration,
    next_id: u64,
    loop_subscribers: BTreeMap<u64, LoopSubscriber>,
    one_shot_requests: BTreeMap<u64, OneShotRequest>,
    last_real_time: Option<Instant>,
    redraw_requester: Option<(Rc<Cell<bool>>, Rc<dyn Fn()>)>,
}

impl NativeFrameState {
    fn has_pending_frames(&self) -> bool {
        !self.one_shot_requests.is_empty()
            || self.loop_subscribers.values().any(|sub| sub.active.get())
    }

    fn notify_host_redraw(&self) {
        if let Some((active, requester)) = &self.redraw_requester {
            if active.get() && self.has_pending_frames() {
                requester();
            }
        }
    }
}

thread_local! {
    static FRAME_STATE: RefCell<NativeFrameState> = RefCell::new(NativeFrameState::default());
}

/// Starts an ongoing animation frame loop in the native frame registry.
///
/// If a host redraw requester is registered (via [`set_host_redraw_requester`]), this automatically
/// requests host redraw to schedule the frame on the host event loop / VSync.
///
/// In manual/headless mode (without a host requester), frames are dispatched deterministically
/// when [`tick`] is called.
///
/// Dropping the returned [`FrameLoopGuard`] immediately unregisters the subscriber.
pub fn start_frame_loop(
    on_frame: impl FnMut(FrameInfo) + 'static,
) -> Result<FrameLoopGuard, FrameLoopError> {
    FRAME_STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let id = state.next_id;
        state.next_id += 1;

        let active = Rc::new(Cell::new(true));
        let callback = Rc::new(RefCell::new(Box::new(on_frame) as Box<dyn FnMut(FrameInfo)>));

        state.loop_subscribers.insert(
            id,
            LoopSubscriber {
                active: active.clone(),
                callback,
            },
        );

        // If a host redraw requester is active, immediately signal that a frame is needed
        state.notify_host_redraw();

        let cancel_active = active.clone();
        Ok(FrameLoopGuard::new(move || {
            cancel_active.set(false);
            FRAME_STATE.with(|inner| {
                if let Ok(mut s) = inner.try_borrow_mut() {
                    s.loop_subscribers.remove(&id);
                }
            });
        }))
    })
}

/// Requests a single execution of the callback on the next frame in the native frame registry.
///
/// If a host redraw requester is registered, this requests a host redraw for the next VSync.
/// In manual/headless mode, the callback executes on the next invocation of [`tick`].
///
/// Dropping or cancelling the returned [`FrameRequestGuard`] prevents execution.
pub fn request_next_frame(
    on_frame: impl FnOnce(Duration) + 'static,
) -> Result<FrameRequestGuard, FrameLoopError> {
    FRAME_STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let id = state.next_id;
        state.next_id += 1;

        let active = Rc::new(Cell::new(true));
        let callback = Rc::new(RefCell::new(Some(Box::new(on_frame) as Box<dyn FnOnce(Duration)>)));

        state.one_shot_requests.insert(
            id,
            OneShotRequest {
                active: active.clone(),
                callback: callback.clone(),
            },
        );

        // If a host redraw requester is active, immediately signal that a frame is needed
        state.notify_host_redraw();

        let cancel_active = active.clone();
        let cancel_cb = callback.clone();
        Ok(FrameRequestGuard::new(move || {
            cancel_active.set(false);
            let _ = cancel_cb.borrow_mut().take();
            FRAME_STATE.with(|inner| {
                if let Ok(mut s) = inner.try_borrow_mut() {
                    s.one_shot_requests.remove(&id);
                }
            });
        }))
    })
}

/// Registers a host redraw requester (e.g. `window.request_redraw()` or Winit event-loop wake).
///
/// Once registered, any subscriber creation or pending one-shot request will automatically
/// signal the host window to schedule a redraw. When the host executes its redraw event
/// (`WindowEvent::RedrawRequested`), it should call [`step_hosted_frame`] to advance time
/// and dispatch frames.
///
/// Returns a [`HostRedrawGuard`] that unregisters the host link upon drop.
pub fn set_host_redraw_requester(requester: impl Fn() + 'static) -> HostRedrawGuard {
    let active = Rc::new(Cell::new(true));
    let guard = HostRedrawGuard::new(active.clone());
    FRAME_STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        state.redraw_requester = Some((active, Rc::new(requester)));
        state.last_real_time = None;
        state.notify_host_redraw();
    });
    guard
}

/// Checks whether there are currently active animation loop subscribers or pending one-shot frame requests.
pub fn has_pending_frames() -> bool {
    FRAME_STATE.with(|cell| cell.borrow().has_pending_frames())
}

/// Dispatches a single hosted animation frame tied to host redraw.
///
/// Automatically measures real elapsed time since the last frame, advances the monotonic clock,
/// and dispatches to all queued callbacks on the current UI thread. If any subscribers or one-shots
/// remain active, it requests the next redraw on the host.
///
/// If no subscribers remain, it cleanly transitions to an idle state (clearing timers and not requesting redraw).
///
/// Returns the elapsed `Duration` dispatched for this frame.
pub fn step_hosted_frame() -> Duration {
    let now_instant = Instant::now();
    let dt = FRAME_STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let dt = match state.last_real_time {
            Some(last) => {
                let d = now_instant.duration_since(last);
                // Clamp delta to safe bounds (1ms to 100ms) to maintain stability across frame hitches
                if d < Duration::from_millis(1) {
                    Duration::from_millis(1)
                } else if d > Duration::from_millis(100) {
                    Duration::from_millis(100)
                } else {
                    d
                }
            }
            None => Duration::from_millis(16), // ~60Hz baseline for initial frame
        };
        state.last_real_time = Some(now_instant);
        dt
    });

    tick(dt);

    // If frames are still pending, schedule next redraw on the host
    FRAME_STATE.with(|cell| {
        let state = cell.borrow();
        if state.has_pending_frames() {
            state.notify_host_redraw();
        } else {
            // Idle state reached
            drop(state);
            if let Ok(mut s) = cell.try_borrow_mut() {
                s.last_real_time = None;
            }
        }
    });

    dt
}

/// Advances the manual clock by `dt`, dispatching to all active frame loop subscribers
/// and resolving queued one-shot frame requests.
///
/// In manual/headless mode, this provides pure deterministic progression for tests, CI,
/// and simulation environments.
pub fn tick(dt: Duration) {
    let (now, one_shots, loops) = FRAME_STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        state.now += dt;
        let now = state.now;

        // Drain one-shots
        let one_shots = std::mem::take(&mut state.one_shot_requests);

        // Snapshot current loop subscribers so callbacks can register new subscribers
        let loops: Vec<(u64, Rc<Cell<bool>>, Rc<RefCell<Box<dyn FnMut(FrameInfo)>>>)> = state
            .loop_subscribers
            .iter()
            .map(|(&id, sub)| (id, sub.active.clone(), sub.callback.clone()))
            .collect();

        (now, one_shots, loops)
    });

    // 1. Dispatch one-shots
    for (_, req) in one_shots {
        if req.active.get() {
            if let Some(cb) = req.callback.borrow_mut().take() {
                cb(now);
            }
        }
    }

    // 2. Dispatch recurring loops
    let info = FrameInfo { now, delta: dt };
    for (_id, active, cb) in loops {
        if active.get() {
            cb.borrow_mut()(info);
        }
    }

    // 3. Clean up inactive subscribers
    FRAME_STATE.with(|cell| {
        if let Ok(mut state) = cell.try_borrow_mut() {
            state.loop_subscribers.retain(|_, sub| sub.active.get());
        }
    });
}
