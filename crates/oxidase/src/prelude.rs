//! Curated ergonomic public imports for consumers of `oxidase`.
//!
//! Exposes foundational DOM, frame timing, and runtime utility types without
//! polluting downstream namespaces.

pub use crate::dom::Document;
pub use crate::frame::{
    request_next_frame, start_frame_loop, tick, FrameInfo, FrameLoopError, FrameLoopGuard,
    FrameRequestGuard,
};
pub use crate::runtime::{get_viewport, measure_rect};
pub use crate::use_watcher;
pub use crate::watcher_guard::WatcherGuard;
pub use oxidase_macro::{bind_js, main};
