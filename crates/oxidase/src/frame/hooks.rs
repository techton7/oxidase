//! High-level Dioxus hooks and futures for frame scheduling.

use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};

use dioxus::prelude::*;

use super::types::{FrameInfo, FrameRequestGuard};

/// Declarative Dioxus hook for running continuous frame-driven animations.
///
/// Automatically subscribes to the host frame engine (`requestAnimationFrame` on Web,
/// VSync `RedrawRequested` on Native) when the component mounts, and cleanly deregisters
/// and transitions the engine to an idle state when the component unmounts.
///
/// # Panics
/// Panics if the host frame runtime is unavailable (e.g. headless environment without
/// frame support or failed registration).
///
/// # Example
/// ```rust,no_run
/// use dioxus::prelude::*;
/// use oxidase::prelude::*;
///
/// #[component]
/// fn Spinner() -> Element {
///     let mut angle = use_signal(|| 0.0);
///     use_frame(move |info| {
///         angle.set(angle() + info.delta.as_secs_f64() * 360.0);
///     });
///     rsx! { div { "Angle: {angle}" } }
/// }
/// ```
pub fn use_frame(mut on_frame: impl FnMut(FrameInfo) + 'static) {
    use_hook(|| {
        let guard = super::start_frame_loop(move |info| {
            on_frame(info);
        })
        .expect("use_frame failed to register animation frame loop with host runtime");
        Rc::new(guard)
    });
}

/// Asynchronously waits until the next frame tick.
///
/// Returns a [`FrameInfo`] struct containing monotonic elapsed time (`now`)
/// and the elapsed delta time (`delta`) since the previous frame tick.
///
/// If dropped before resolving (e.g. timeout or component unmount), the scheduled
/// single-frame request is automatically cancelled.
///
/// # Panics
/// Panics if the host frame runtime fails to schedule the frame request (e.g.
/// unsupported host or missing browser window).
///
/// # Example
/// ```rust,no_run
/// use dioxus::prelude::*;
/// use oxidase::prelude::*;
///
/// #[component]
/// fn FadeIn() -> Element {
///     let mut opacity = use_signal(|| 0.0);
///     use_future(move || async move {
///         while opacity() < 1.0 {
///             let info = next_frame().await;
///             opacity.set((opacity() + info.delta.as_secs_f64() * 2.0).min(1.0));
///         }
///     });
///     rsx! { div { style: "opacity: {opacity};", "Hello" } }
/// }
/// ```
pub fn next_frame() -> NextFrameFuture {
    NextFrameFuture {
        state: Rc::new(RefCell::new(NextFrameState {
            result: None,
            waker: None,
        })),
        guard: None,
    }
}

/// Future returned by [`next_frame`].
pub struct NextFrameFuture {
    state: Rc<RefCell<NextFrameState>>,
    guard: Option<FrameRequestGuard>,
}

struct NextFrameState {
    result: Option<FrameInfo>,
    waker: Option<Waker>,
}

impl Future for NextFrameFuture {
    type Output = FrameInfo;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(info) = self.state.borrow_mut().result.take() {
            return Poll::Ready(info);
        }

        self.state.borrow_mut().waker = Some(cx.waker().clone());

        if self.guard.is_none() {
            let state = self.state.clone();
            match super::request_next_frame(move |info| {
                let mut s = state.borrow_mut();
                s.result = Some(info);
                if let Some(w) = s.waker.take() {
                    w.wake();
                }
            }) {
                Ok(guard) => {
                    self.guard = Some(guard);
                }
                Err(err) => {
                    panic!("next_frame failed to schedule with host frame runtime: {err}");
                }
            }
        }

        Poll::Pending
    }
}
