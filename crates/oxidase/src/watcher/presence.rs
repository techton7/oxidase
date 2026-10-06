//! Presence lifecycle watcher for postponing DOM unmounting until exit animations finish.

use crate::dom::observer::{observe_transition_lifecycle, TransitionLifecycleEvent};
use crate::error::Result;
use crate::watcher_guard::WatcherGuard;
use serde::{Deserialize, Serialize};

/// Reasons an exit transition fell back to immediate completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresenceFallbackReason {
    Missing,
    Hidden,
    NoAnimation,
    Timeout,
}

/// Backward-compatible alias matching `PresenceFallbackReason`.
pub type PresenceMonitorFallback = PresenceFallbackReason;

/// Lifecycle events emitted during presence unmount monitoring.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PresenceEvent {
    Fallback {
        #[serde(rename = "cycleId", default)]
        cycle_id: u64,
        reason: PresenceFallbackReason,
    },
    AnimationEnd {
        #[serde(rename = "cycleId", default)]
        cycle_id: u64,
        #[serde(rename = "animationName")]
        animation_name: String,
    },
    AnimationCancel {
        #[serde(rename = "cycleId", default)]
        cycle_id: u64,
        #[serde(rename = "animationName")]
        animation_name: String,
    },
    Stopped {
        #[serde(rename = "cycleId", default)]
        cycle_id: u64,
    },
}

/// Backward-compatible alias matching `PresenceEvent`.
pub type PresenceEventPayload = PresenceEvent;

/// Configuration options for presence lifecycle observation.
#[derive(Clone, Debug)]
pub struct PresenceOptions {
    pub element_id: String,
    pub cycle_id: u64,
    pub fallback_timeout_ms: u64,
}

impl PresenceOptions {
    /// Creates options for the given element ID and unmount cycle ID.
    pub fn new(element_id: &str, cycle_id: u64) -> Self {
        Self {
            element_id: element_id.to_string(),
            cycle_id,
            fallback_timeout_ms: 500,
        }
    }

    /// Overrides the fallback timeout duration in milliseconds.
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.fallback_timeout_ms = timeout_ms;
        self
    }
}

/// Watches an element's exit animations and unblocks unmounting when animations finish or fallback timeout fires.
pub fn watch_presence(
    element_id: &str,
    cycle_id: u64,
    on_event: impl FnMut(PresenceEvent) + 'static,
) -> Result<WatcherGuard> {
    watch_presence_with_options(PresenceOptions::new(element_id, cycle_id), on_event)
}

/// Watches presence with explicit configuration options.
pub fn watch_presence_with_options(
    options: PresenceOptions,
    mut on_event: impl FnMut(PresenceEvent) + 'static,
) -> Result<WatcherGuard> {
    let cycle_id = options.cycle_id;
    let sub_id = crate::internal::next_subscription_id();

    // Pre-flight check: if active document exists and element is not in DOM, fall back immediately
    if let Some(doc) = crate::dom::Document::current() {
        if doc.element_by_id(&options.element_id).is_none() {
            on_event(PresenceEvent::Fallback {
                cycle_id,
                reason: PresenceFallbackReason::Missing,
            });
            return Ok(WatcherGuard::new("watch_presence_missing", sub_id, None));
        }
    }

    let mut finished = false;

    let inner_guard = observe_transition_lifecycle(
        &options.element_id,
        options.fallback_timeout_ms,
        move |event| {
            if finished {
                return;
            }
            finished = true;
            match event {
                TransitionLifecycleEvent::AnimationEnd { animation_name } => {
                    on_event(PresenceEvent::AnimationEnd {
                        cycle_id,
                        animation_name,
                    });
                }
                TransitionLifecycleEvent::AnimationCancel { animation_name } => {
                    on_event(PresenceEvent::AnimationCancel {
                        cycle_id,
                        animation_name,
                    });
                }
                TransitionLifecycleEvent::TimeoutFallback => {
                    on_event(PresenceEvent::Fallback {
                        cycle_id,
                        reason: PresenceFallbackReason::Timeout,
                    });
                }
                TransitionLifecycleEvent::Stopped => {
                    on_event(PresenceEvent::Stopped { cycle_id });
                }
            }
        },
    )?;

    Ok(WatcherGuard::with_cleanup(
        "watch_presence",
        sub_id,
        None,
        move || {
            inner_guard.stop();
        },
    ))
}
