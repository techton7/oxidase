//! Host-neutral reactive watcher substrate for Dioxus headless UI primitives.
//!
//! Provides unified cross-platform watchers for:
//! 1. `watch_document_dismiss` (outside interaction and Escape dismissal)
//! 2. `watch_floating_auto_update` (anchor resize/scroll with frame coalescing)
//! 3. `watch_presence` (exit animation lifecycle and timeout fallback)
//! 4. `watch_form_reset` (form reset signal propagation)

pub mod dismiss;
pub mod floating;
pub mod form;
pub mod presence;

pub use dismiss::*;
pub use floating::*;
pub use form::*;
pub use presence::*;
