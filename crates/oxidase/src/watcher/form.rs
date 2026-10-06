//! Form reset watcher for resetting headless input controls when their enclosing form resets.

use crate::dom::observer::observe_form_reset;
use crate::error::Result;
use crate::watcher_guard::WatcherGuard;
use serde::{Deserialize, Serialize};

/// Signal payload emitted when a form reset occurs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FormResetEvent {
    Reset,
}

/// Backward-compatible alias matching `FormResetEvent`.
pub type FormResetEventPayload = FormResetEvent;

/// Configuration options for the form reset watcher.
#[derive(Clone, Debug)]
pub struct FormResetOptions {
    pub element_id: String,
    pub form_id: Option<String>,
}

impl FormResetOptions {
    /// Creates options for the given element ID and optional explicit form ID.
    pub fn new(element_id: &str, form_id: Option<&str>) -> Self {
        Self {
            element_id: element_id.to_string(),
            form_id: form_id.map(ToString::to_string),
        }
    }
}

/// Watches form reset events on an element's parent form or explicit form target.
pub fn watch_form_reset(
    element_id: &str,
    form_id: Option<&str>,
    on_event: impl FnMut(FormResetEvent) + 'static,
) -> Result<WatcherGuard> {
    watch_form_reset_with_options(FormResetOptions::new(element_id, form_id), on_event)
}

/// Watches form reset events with explicit configuration options.
pub fn watch_form_reset_with_options(
    options: FormResetOptions,
    mut on_event: impl FnMut(FormResetEvent) + 'static,
) -> Result<WatcherGuard> {
    let sub_id = crate::internal::next_subscription_id();
    let inner_guard = observe_form_reset(
        &options.element_id,
        options.form_id.as_deref(),
        move || {
            on_event(FormResetEvent::Reset);
        },
    )?;

    Ok(WatcherGuard::with_cleanup(
        "watch_form_reset",
        sub_id,
        None,
        move || {
            inner_guard.stop();
        },
    ))
}
