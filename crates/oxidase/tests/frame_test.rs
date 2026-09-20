//! Integration tests for `oxidase::frame` and `oxidase::prelude`.

use oxidase::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

#[test]
fn test_prelude_reexports_availability() {
    // 1. Types are exported and constructible
    let info = FrameInfo {
        now: Duration::from_millis(100),
        delta: Duration::from_millis(16),
    };
    assert_eq!(info.now.as_millis(), 100);
    assert_eq!(info.delta.as_millis(), 16);

    // 2. Error types
    let err = FrameLoopError::RuntimeUnavailable;
    assert_eq!(err, FrameLoopError::RuntimeUnavailable);

    let err2 = FrameLoopError::UnsupportedHost;
    assert_eq!(err2, FrameLoopError::UnsupportedHost);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn test_native_frame_manual_loop_ticking() {
    let ticks = Rc::new(RefCell::new(Vec::new()));
    let t = ticks.clone();

    let guard = start_frame_loop(move |info| {
        t.borrow_mut().push((info.now, info.delta));
    })
    .expect("start_frame_loop should succeed in manual/headless mode");

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
fn test_native_frame_manual_one_shot_request() {
    let fired = Rc::new(RefCell::new(false));
    let f = fired.clone();

    let _guard = request_next_frame(move |_now| {
        *f.borrow_mut() = true;
    })
    .expect("request_next_frame should succeed in manual/headless mode");

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
fn test_native_frame_manual_one_shot_cancellation() {
    let fired = Rc::new(RefCell::new(false));
    let f = fired.clone();

    let guard = request_next_frame(move |_now| {
        *f.borrow_mut() = true;
    })
    .expect("request_next_frame should succeed in manual/headless mode");

    guard.cancel();
    tick(Duration::from_millis(16));
    assert!(!*fired.borrow());
}
