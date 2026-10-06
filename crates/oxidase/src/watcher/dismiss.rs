//! Document dismiss watcher for popovers, dialogs, selects, and tooltips.
//!
//! Intercepts pointerdown, focusin, and Escape key interactions with boundary ID suppression.

use crate::dom::observer::{observe_capture_events, CaptureEventKind};
use crate::error::Result;
use crate::watcher_guard::WatcherGuard;
use serde::{Deserialize, Serialize};

/// Event emitted when an outside interaction or escape key dismissal occurs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DismissEvent {
    PointerDown {
        #[serde(rename = "pathIds", default)]
        path_ids: Vec<String>,
    },
    FocusIn {
        #[serde(rename = "pathIds", default)]
        path_ids: Vec<String>,
    },
    Escape,
}

impl DismissEvent {
    /// Checks whether this event occurred outside all provided boundary element IDs.
    pub fn is_outside_boundary(&self, boundary_ids: &[&str]) -> bool {
        match self {
            Self::PointerDown { path_ids } | Self::FocusIn { path_ids } => {
                if boundary_ids.is_empty() {
                    return true;
                }
                !path_ids.iter().any(|id| boundary_ids.contains(&id.as_str()))
            }
            Self::Escape => true,
        }
    }

    /// Returns the outermost target element ID if present.
    pub fn target_id(&self) -> Option<&str> {
        match self {
            Self::PointerDown { path_ids } | Self::FocusIn { path_ids } => {
                path_ids.first().map(|s| s.as_str())
            }
            Self::Escape => None,
        }
    }
}

/// Backward-compatible alias for `DismissEvent`.
pub type DocumentDismissEvent = DismissEvent;

/// Configuration options for the document dismiss watcher.
#[derive(Clone, Debug, Default)]
pub struct DismissOptions {
    pub boundary_ids: Vec<String>,
    pub listen_escape: bool,
    pub listen_pointer: bool,
    pub listen_focusin: bool,
}

impl DismissOptions {
    /// Creates a new `DismissOptions` registered to the given boundary IDs.
    pub fn new(boundary_ids: &[&str]) -> Self {
        Self {
            boundary_ids: boundary_ids.iter().map(|s| s.to_string()).collect(),
            listen_escape: true,
            listen_pointer: true,
            listen_focusin: true,
        }
    }
}

/// Watches global document interactions and emits dismissal signals when interactions occur outside boundaries.
pub fn watch_document_dismiss(
    boundary_ids: &[&str],
    on_event: impl FnMut(DismissEvent) + 'static,
) -> Result<WatcherGuard> {
    watch_document_dismiss_with_options(DismissOptions::new(boundary_ids), on_event)
}

/// Watches document dismissal with explicit configuration options.
pub fn watch_document_dismiss_with_options(
    options: DismissOptions,
    mut on_event: impl FnMut(DismissEvent) + 'static,
) -> Result<WatcherGuard> {
    let boundary_ids = options.boundary_ids;
    let listen_escape = options.listen_escape;
    let listen_pointer = options.listen_pointer;
    let listen_focusin = options.listen_focusin;

    observe_capture_events(move |event| {
        match event.kind {
            CaptureEventKind::PointerDown => {
                if !listen_pointer {
                    return;
                }
                let is_inside = event.path_ids.iter().any(|id| boundary_ids.contains(id));
                if !is_inside {
                    on_event(DismissEvent::PointerDown {
                        path_ids: event.path_ids,
                    });
                }
            }
            CaptureEventKind::FocusIn => {
                if !listen_focusin {
                    return;
                }
                let is_inside = event.path_ids.iter().any(|id| boundary_ids.contains(id));
                if !is_inside {
                    on_event(DismissEvent::FocusIn {
                        path_ids: event.path_ids,
                    });
                }
            }
            CaptureEventKind::KeyDown { key } => {
                if !listen_escape {
                    return;
                }
                if key == "Escape" || key == "Esc" {
                    on_event(DismissEvent::Escape);
                }
            }
        }
    })
}
