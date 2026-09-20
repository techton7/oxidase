//! Non-WASM / Native frame driver implementation.
//!
//! Provides a deterministic manual/headless frame registry:
//! - `start_frame_loop(...)` registers ongoing loop subscribers.
//! - `request_next_frame(...)` registers one-shot callbacks for the next tick.
//! - `tick(dt)` advances the monotonic clock and dispatches frames.
//!
//! Hosted native auto-looping (VSync/Winit display link) is deferred to a future milestone (tracked in `ISSUE-0001`).

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::Duration;

use super::types::{FrameInfo, FrameLoopError, FrameLoopGuard, FrameRequestGuard};

struct LoopSubscriber {
    active: Rc<Cell<bool>>,
    callback: Rc<RefCell<Box<dyn FnMut(FrameInfo)>>>,
}

struct OneShotRequest {
    active: Rc<Cell<bool>>,
    callback: Rc<RefCell<Option<Box<dyn FnOnce(Duration)>>>>,
}

#[derive(Default)]
struct NativeFrameState {
    now: Duration,
    next_id: u64,
    loop_subscribers: BTreeMap<u64, LoopSubscriber>,
    one_shot_requests: BTreeMap<u64, OneShotRequest>,
}

thread_local! {
    static FRAME_STATE: RefCell<NativeFrameState> = RefCell::new(NativeFrameState::default());
}

/// Starts an ongoing animation frame loop in the manual/headless frame registry.
///
/// In this manual/headless mode, frames are dispatched deterministically when [`tick`] is called.
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

/// Requests a single execution of the callback on the next frame in the manual/headless frame registry.
///
/// In this manual/headless mode, the callback executes on the next invocation of [`tick`].
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

/// Advances the manual clock by `dt`, dispatching to all active frame loop subscribers
/// and resolving queued one-shot frame requests.
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
