//! Host frame scheduling and timing utility.
//!
//! Provides platform-normalized frame callbacks with [`FrameInfo`] timestamps,
//! drop-driven cancellation, and manual/headless ticking support.
//!
//! - **High-Level DX (Re-exported in [`oxidase::prelude`](crate::prelude))**:
//!   [`use_frame`] (declarative continuous animation hook) and [`next_frame`]
//!   (ergonomic async VSync yield returning [`FrameInfo`]).
//! - **Low-Level Runtime Primitives (Under [`oxidase::frame`](crate::frame))**:
//!   [`start_frame_loop`], [`request_next_frame`], [`FrameLoopError`], [`FrameLoopGuard`],
//!   [`FrameRequestGuard`], and [`tick`].
//! - On Web (`wasm32`), this drives via browser `requestAnimationFrame`.
//! - On Native (hosted), this integrates with OS VSync via `WindowEvent::RedrawRequested`
//!   and [`step_hosted_frame`].
//! - On Native (headless/tests), this provides deterministic manual ticking via [`tick`].

mod types;
pub use types::{FrameInfo, FrameLoopError, FrameLoopGuard, FrameRequestGuard};

mod hooks;
pub use hooks::{next_frame, use_frame, NextFrameFuture};

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
pub use web::{
    has_pending_frames, request_next_frame, set_host_redraw_requester, start_frame_loop,
    step_hosted_frame, tick, HostRedrawGuard,
};

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::{
    has_pending_frames, request_next_frame, set_host_redraw_requester, start_frame_loop,
    step_hosted_frame, tick, HostRedrawGuard,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn test_frame_loop_guard_drop_cancellation() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let c = cancelled.clone();

        let guard = FrameLoopGuard::new(move || {
            c.store(true, Ordering::SeqCst);
        });

        assert!(!cancelled.load(Ordering::SeqCst));
        drop(guard);
        assert!(cancelled.load(Ordering::SeqCst));
    }

    #[test]
    fn test_frame_request_guard_drop_cancellation() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let c = cancelled.clone();

        let guard = FrameRequestGuard::new(move || {
            c.store(true, Ordering::SeqCst);
        });

        assert!(!cancelled.load(Ordering::SeqCst));
        guard.cancel();
        assert!(cancelled.load(Ordering::SeqCst));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_native_manual_loop_ticking() {
        use std::cell::RefCell;
        use std::rc::Rc;

        let ticks = Rc::new(RefCell::new(Vec::new()));
        let t = ticks.clone();

        let guard = start_frame_loop(move |info| {
            t.borrow_mut().push((info.now, info.delta));
        })
        .expect("start_frame_loop should succeed in manual registry");

        tick(Duration::from_millis(16));
        tick(Duration::from_millis(16));

        assert_eq!(ticks.borrow().len(), 2);
        assert_eq!(ticks.borrow()[0].1, Duration::from_millis(16));
        assert_eq!(ticks.borrow()[1].1, Duration::from_millis(16));

        // Dropping guard stops subsequent ticks
        drop(guard);
        tick(Duration::from_millis(16));
        assert_eq!(ticks.borrow().len(), 2);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_native_manual_one_shot_request() {
        use std::cell::RefCell;
        use std::rc::Rc;

        let fired = Rc::new(RefCell::new(false));
        let f = fired.clone();

        let _guard = request_next_frame(move |_now| {
            *f.borrow_mut() = true;
        })
        .expect("request_next_frame should succeed in manual registry");

        assert!(!*fired.borrow());
        tick(Duration::from_millis(16));
        assert!(*fired.borrow());

        // Does not fire again on second tick
        *fired.borrow_mut() = false;
        tick(Duration::from_millis(16));
        assert!(!*fired.borrow());
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_native_manual_one_shot_cancellation() {
        use std::cell::RefCell;
        use std::rc::Rc;

        let fired = Rc::new(RefCell::new(false));
        let f = fired.clone();

        let guard = request_next_frame(move |_now| {
            *f.borrow_mut() = true;
        })
        .expect("request_next_frame should succeed in manual registry");

        guard.cancel();
        tick(Duration::from_millis(16));
        assert!(!*fired.borrow());
    }
}
