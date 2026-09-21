//! Integration tests for `oxidase::frame` and `oxidase::prelude`.

use oxidase::frame::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

#[test]
fn test_prelude_high_level_exports_availability() {
    use oxidase::prelude::*;

    // 1. High-level types and hooks are exported in prelude
    let info = FrameInfo {
        now: Duration::from_millis(100),
        delta: Duration::from_millis(16),
    };
    assert_eq!(info.now.as_millis(), 100);
    assert_eq!(info.delta.as_millis(), 16);

    let _ = next_frame;
    let _: fn(fn(FrameInfo)) = use_frame;
}

#[test]
fn test_frame_module_low_level_exports_availability() {
    // 2. Low-level error types and primitives are under oxidase::frame
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

    let received_delta = Rc::new(RefCell::new(Duration::ZERO));
    let rd = received_delta.clone();

    let _guard = request_next_frame(move |info| {
        *f.borrow_mut() = true;
        *rd.borrow_mut() = info.delta;
    })
    .expect("request_next_frame should succeed in manual/headless mode");

    assert!(!*fired.borrow());
    tick(Duration::from_millis(16));
    assert!(*fired.borrow());
    assert_eq!(*received_delta.borrow(), Duration::from_millis(16));

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

    let guard = request_next_frame(move |_info| {
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

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn test_hosted_native_frame_redraw_requester_lifecycle() {
    let redraw_requests = Rc::new(RefCell::new(0usize));
    let r_req = redraw_requests.clone();

    // Register host redraw requester (simulating window.request_redraw())
    let redraw_guard = set_host_redraw_requester(move || {
        *r_req.borrow_mut() += 1;
    });

    // Initially no frames pending, 0 redraw requests
    assert_eq!(*redraw_requests.borrow(), 0);
    assert!(!has_pending_frames());

    // 1. Starting a frame loop triggers the host redraw requester immediately
    let frame_ticks = Rc::new(RefCell::new(Vec::new()));
    let f_ticks = frame_ticks.clone();
    let loop_guard = start_frame_loop(move |info| {
        f_ticks.borrow_mut().push(info.delta);
    })
    .expect("start_frame_loop should succeed");

    assert!(has_pending_frames());
    assert_eq!(*redraw_requests.borrow(), 1, "start_frame_loop must request host redraw");

    // 2. Host dispatches a redraw via step_hosted_frame()
    let dt = step_hosted_frame();
    assert!(dt >= Duration::from_millis(1));
    assert_eq!(frame_ticks.borrow().len(), 1);
    // Because the loop is still active, step_hosted_frame requested the NEXT frame redraw
    assert_eq!(*redraw_requests.borrow(), 2, "active loop must schedule next redraw");

    // 3. Host dispatches a second redraw
    step_hosted_frame();
    assert_eq!(frame_ticks.borrow().len(), 2);
    assert_eq!(*redraw_requests.borrow(), 3);

    // 4. Cancel the loop
    drop(loop_guard);
    assert!(!has_pending_frames());

    // 5. Subsequent step_hosted_frame finds no pending frames and enters IDLE (does NOT request redraw)
    let requests_before = *redraw_requests.borrow();
    step_hosted_frame();
    assert_eq!(*redraw_requests.borrow(), requests_before, "idle state must not request redraw");

    // Dropping the host redraw guard cleans up cleanly
    drop(redraw_guard);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn test_hosted_native_one_shot_request_and_idle() {
    let redraw_requests = Rc::new(RefCell::new(0usize));
    let r_req = redraw_requests.clone();

    let _redraw_guard = set_host_redraw_requester(move || {
        *r_req.borrow_mut() += 1;
    });

    let one_shot_fired = Rc::new(RefCell::new(false));
    let osf = one_shot_fired.clone();

    // 1. Request single frame -> requests host redraw
    let _guard = request_next_frame(move |_now| {
        *osf.borrow_mut() = true;
    })
    .expect("request_next_frame should succeed");

    assert!(has_pending_frames());
    assert_eq!(*redraw_requests.borrow(), 1);

    // 2. Dispatch the frame
    step_hosted_frame();
    assert!(*one_shot_fired.borrow(), "one shot callback must execute");
    assert!(!has_pending_frames(), "one-shot must be drained after execution");

    // 3. After single execution, system is idle: does NOT request another redraw
    assert_eq!(*redraw_requests.borrow(), 1, "completed one-shot must not schedule another redraw");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn test_next_frame_future_execution_and_cancellation() {
    use std::pin::Pin;
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

    unsafe fn clone(_: *const ()) -> RawWaker {
        RawWaker::new(std::ptr::null(), &VTABLE)
    }
    unsafe fn wake(_: *const ()) {}
    unsafe fn wake_by_ref(_: *const ()) {}
    unsafe fn vtable_drop(_: *const ()) {}
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, vtable_drop);
    let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };
    let mut cx = Context::from_waker(&waker);

    // 1. Initial poll registers the one-shot and returns Pending
    let mut fut = next_frame();
    assert_eq!(Pin::new(&mut fut).poll(&mut cx), Poll::Pending);
    assert!(has_pending_frames());

    // 2. Tick frame advances clock and delivers FrameInfo
    tick(Duration::from_millis(16));

    // 3. Second poll resolves to Ready(FrameInfo)
    match Pin::new(&mut fut).poll(&mut cx) {
        Poll::Ready(info) => {
            assert_eq!(info.delta, Duration::from_millis(16));
            assert!(info.now >= Duration::from_millis(16));
        }
        Poll::Pending => panic!("next_frame should be ready after tick"),
    }

    // 4. Cancellation proof: dropping future cancels request
    let mut fut2 = next_frame();
    assert_eq!(Pin::new(&mut fut2).poll(&mut cx), Poll::Pending);
    assert!(has_pending_frames());
    drop(fut2);
    assert!(!has_pending_frames(), "dropping next_frame future must cancel pending request");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn test_use_frame_virtual_dom_lifecycle() {
    use dioxus::prelude::*;

    let frame_count = Rc::new(RefCell::new(0usize));
    let fc = frame_count.clone();

    #[component]
    fn FrameApp(fc: Rc<RefCell<usize>>) -> Element {
        use_frame(move |_info| {
            *fc.borrow_mut() += 1;
        });
        rsx! { div {} }
    }

    let mut dom = VirtualDom::new_with_props(FrameApp, FrameAppProps { fc });
    dom.rebuild_in_place();

    assert_eq!(*frame_count.borrow(), 0);
    assert!(has_pending_frames());

    // Tick 3 frames
    tick(Duration::from_millis(16));
    tick(Duration::from_millis(16));
    tick(Duration::from_millis(16));
    assert_eq!(*frame_count.borrow(), 3);

    // Dropping VirtualDom drops scope hooks and unregisters subscriber
    drop(dom);
    assert!(!has_pending_frames());
    tick(Duration::from_millis(16));
    assert_eq!(*frame_count.borrow(), 3, "unmounted component must not receive ticks");
}


