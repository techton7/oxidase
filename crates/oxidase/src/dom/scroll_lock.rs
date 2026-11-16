//! Cross-host document body scroll-lock coordination with reference-counted lock IDs
//! and optional delayed restoration.

use std::{cell::RefCell, collections::HashSet, time::Duration};

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
struct ScrollLockState {
    locks: HashSet<String>,
    is_locked: bool,
    original_overflow: String,
    original_padding_right: String,
    cleanup_generation: u64,
}

thread_local! {
    static STATE: RefCell<ScrollLockState> = RefCell::new(ScrollLockState {
        locks: HashSet::new(),
        is_locked: false,
        original_overflow: String::new(),
        original_padding_right: String::new(),
        cleanup_generation: 0,
    });
}

/// Returns whether a document scroll lock is currently active on this thread.
pub fn is_scroll_locked() -> bool {
    STATE.with(|state| {
        let state = state.borrow();
        state.is_locked || !state.locks.is_empty()
    })
}

/// Acquires a named scroll lock on the active host document.
///
/// On Web (`wasm32`), applies `overflow: hidden` and scrollbar-gutter compensation (`padding-right`
/// and `--scrollbar-width`) to `document.body` when the first lock is acquired.
/// On Native/Mock hosts, maintains deterministic lock reference counting and generation state
/// without mutating `<body>` styles.
pub fn acquire_scroll_lock(lock_id: &str) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.cleanup_generation = state.cleanup_generation.wrapping_add(1);

        #[cfg(target_arch = "wasm32")]
        if !state.is_locked {
            if let Some(window) = web_sys::window()
                && let Some(document) = window.document()
                && let Some(body) = document.body()
            {
                let body_style = body.style();
                state.original_overflow = body_style
                    .get_property_value("overflow")
                    .unwrap_or_default();
                state.original_padding_right = body_style
                    .get_property_value("padding-right")
                    .unwrap_or_default();

                let doc_elem = document.document_element();
                let html_style = doc_elem
                    .as_ref()
                    .and_then(|el| window.get_computed_style(el).ok().flatten());
                let computed_body_style = window.get_computed_style(&body).ok().flatten();

                let has_stable_gutter = html_style
                    .and_then(|s| s.get_property_value("scrollbar-gutter").ok())
                    .unwrap_or_default()
                    .contains("stable")
                    || computed_body_style
                        .and_then(|s| s.get_property_value("scrollbar-gutter").ok())
                        .unwrap_or_default()
                        .contains("stable");

                let window_inner_width = window
                    .inner_width()
                    .ok()
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0) as i32;
                let doc_client_width = doc_elem.map(|el| el.client_width()).unwrap_or(0);
                let scrollbar_width = window_inner_width - doc_client_width;

                if scrollbar_width > 0 && !has_stable_gutter {
                    let px_val = format!("{scrollbar_width}px");
                    let _ = body_style.set_property("padding-right", &px_val);
                    let _ = body_style.set_property("--scrollbar-width", &px_val);
                }
                let _ = body_style.set_property("overflow", "hidden");
            }
        }

        state.is_locked = true;
        state.locks.insert(lock_id.to_string());
    });
}

/// Releases a named scroll lock, restoring host scroll state immediately or after `restore_delay` milliseconds
/// once the last active lock is released.
///
/// If `restore_delay` is `Some(ms)` with `ms > 0` and an active Dioxus runtime exists, restoration is
/// scheduled via `safe_spawn` + `futures_timer::Delay`, guarded by `cleanup_generation` so a subsequent
/// `acquire_scroll_lock` cancels stale restoration. Outside an active Dioxus runtime, falls back to
/// immediate restoration so locks never leak in synchronous tests or teardown paths.
pub fn release_scroll_lock(lock_id: &str, restore_delay: Option<u64>) {
    let (should_restore, token) = STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.locks.remove(lock_id);
        if !state.locks.is_empty() || !state.is_locked {
            return (false, 0);
        }
        state.cleanup_generation = state.cleanup_generation.wrapping_add(1);
        (true, state.cleanup_generation)
    });

    if !should_restore {
        return;
    }

    let do_restore = move || {
        STATE.with(|state| {
            let mut state = state.borrow_mut();
            if state.cleanup_generation != token || !state.locks.is_empty() {
                return;
            }
            state.is_locked = false;

            #[cfg(target_arch = "wasm32")]
            if let Some(window) = web_sys::window()
                && let Some(document) = window.document()
                && let Some(body) = document.body()
            {
                let body_style = body.style();
                let _ = body_style.set_property("overflow", &state.original_overflow);
                let _ = body_style.set_property("padding-right", &state.original_padding_right);
                let _ = body_style.remove_property("--scrollbar-width");
            }
        });
    };

    let delay_ms = restore_delay.unwrap_or(0);
    if delay_ms > 0 && dioxus::core::Runtime::try_current().is_some() {
        let delay = futures_timer::Delay::new(Duration::from_millis(delay_ms));
        let do_restore_async = do_restore;
        let spawned = crate::internal::safe_spawn(async move {
            delay.await;
            do_restore_async();
        });
        if spawned.is_none() {
            do_restore();
        }
    } else {
        do_restore();
    }
}
