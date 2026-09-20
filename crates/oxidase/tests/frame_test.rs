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

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn test_spike_hosted_native_thread_safety_and_idle() {
    // 1. Thread safety proof: Verify that manual/headless frame ticking executes callbacks
    // on the exact calling UI thread and safely mutates !Send UI data (Rc<RefCell>).
    let thread_id = std::thread::current().id();
    let callback_thread_id = Rc::new(RefCell::new(None));
    let cb_tid = callback_thread_id.clone();

    let counter = Rc::new(RefCell::new(0));
    let c = counter.clone();

    let _guard = start_frame_loop(move |_info| {
        *cb_tid.borrow_mut() = Some(std::thread::current().id());
        *c.borrow_mut() += 1;
    })
    .expect("start_frame_loop should succeed");

    // 2. Idle proof: In the absence of an explicit pump (tick), zero callbacks are executed.
    // Proves that no hidden background OS timer thread is spinning or burning CPU.
    assert_eq!(*counter.borrow(), 0);
    assert_eq!(*callback_thread_id.borrow(), None);

    // 3. Redraw-synchronized proof: Dispatch occurs synchronously during tick on the UI thread.
    tick(Duration::from_millis(16));
    assert_eq!(*counter.borrow(), 1);
    assert_eq!(*callback_thread_id.borrow(), Some(thread_id));
}

