//! Curated high-level public imports for consumers of `oxidase`.
//!
//! Exposes consumer-facing Dioxus hooks, async yield futures, and ambient DOM capabilities.
//! For low-level frame runtime primitives and fallible scheduling controls, import
//! from [`oxidase::frame`](crate::frame).

pub use crate::dom::Document;
pub use crate::frame::{next_frame, use_frame, FrameInfo, NextFrameFuture};
pub use crate::runtime::{get_viewport, measure_rect};
pub use crate::use_watcher;
pub use crate::watcher_guard::WatcherGuard;
pub use oxidase_macro::{bind_js, main};
