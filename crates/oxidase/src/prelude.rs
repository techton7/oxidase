//! Curated high-level public imports for consumers of `oxidase`.
//!
//! Exposes consumer-facing Dioxus hooks, async yield futures, and ambient DOM capabilities.
//! For low-level frame runtime primitives and fallible scheduling controls, import
//! from [`oxidase::frame`](crate::frame).

pub use crate::dom::{document, Document, MountedHandle};
pub use crate::dom::element::{
    Element as DomElement, FocusOptions, ScrollBehavior, ScrollIntoViewOptions,
    ScrollLogicalPosition,
};
pub use crate::error::{Error, HostError, Result};
pub use crate::scroll::Scrollable;
pub use crate::window::{window, Window};
pub use crate::runtime::geometry::{Point, Rect, Size};
pub use crate::frame::{next_frame, use_frame, FrameInfo, NextFrameFuture};
pub use crate::use_watcher;
pub use crate::watcher_guard::WatcherGuard;
pub use crate::watcher::{
    watch_document_dismiss, watch_floating_auto_update, watch_form_reset, watch_presence,
    DismissEvent, DismissOptions, DocumentDismissEvent, FloatingAutoUpdateEvent,
    FloatingAutoUpdateOptions, FloatingAutoUpdatePayload, FormResetEvent, FormResetEventPayload,
    FormResetOptions, PresenceEvent, PresenceEventPayload, PresenceFallbackReason,
    PresenceMonitorFallback, PresenceOptions,
};
pub use crate::launch::is_debug_control_active;
pub use oxidase_macro::{bind_js, main};
