//! Host-neutral programmatic observer surfaces across Web and Blitz Native targets.
//!
//! Provides Web-compatible observer surfaces for:
//! 1. Element resize observation (ResizeObserver equivalent)
//! 2. Capture-phase interaction listening (PointerDown, FocusIn, KeyDown with node path resolution)
//! 3. Transition and animation lifecycle with resolved-state fallbacks
//! 4. Form reset observation on enclosing forms

use crate::error::{HostError, Result};
use crate::runtime::geometry::Rect;
use crate::watcher_guard::WatcherGuard;

/// Host-neutral state of an observed image request.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImageLoadStatus {
    #[default]
    Loading,
    Loaded,
    Error,
}

/// HTML image request policies which affect fetching, where supported by the host.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImageLoadOptions {
    pub crossorigin: Option<String>,
    pub referrerpolicy: Option<String>,
}

/// Host document visibility state (`document.visibilityState` on Web, window occlusion on Native).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentVisibility {
    Visible,
    Hidden,
}

impl DocumentVisibility {
    /// Returns `true` when the document is hidden/occluded.
    #[inline]
    pub const fn is_hidden(&self) -> bool {
        matches!(self, Self::Hidden)
    }

    /// Returns `true` when the document is visible.
    #[inline]
    pub const fn is_visible(&self) -> bool {
        matches!(self, Self::Visible)
    }
}

/// Element resize entry containing the target element ID and its updated bounding/content rect.
#[derive(Clone, Debug, PartialEq)]
pub struct ResizeEntry {
    pub target_id: String,
    pub content_rect: Rect,
}

/// The box model sizing convention used by [`observe_element_resize_with_box`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ResizeObserverBox {
    #[default]
    BorderBox,
    ContentBox,
}

/// Bitmask selecting which global capture-phase event categories to subscribe to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CaptureEventMask(u8);

impl CaptureEventMask {
    pub const NONE: Self = Self(0);
    pub const POINTER_DOWN: Self = Self(1 << 0);
    pub const CLICK: Self = Self(1 << 1);
    pub const FOCUS_IN: Self = Self(1 << 2);
    pub const KEY_DOWN: Self = Self(1 << 3);
    pub const ALL: Self =
        Self(Self::POINTER_DOWN.0 | Self::CLICK.0 | Self::FOCUS_IN.0 | Self::KEY_DOWN.0);

    /// Returns `true` if all bits in `other` are present in `self`.
    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Returns `true` if no event categories are enabled.
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns the raw bit representation of this mask.
    #[inline]
    pub const fn bits(self) -> u8 {
        self.0
    }
}

impl std::ops::BitOr for CaptureEventMask {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for CaptureEventMask {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// The kind of global capture interaction intercepted.
#[derive(Clone, Debug, PartialEq)]
pub enum CaptureEventKind {
    PointerDown {
        pointer_type: String,
        client_x: f64,
        client_y: f64,
    },
    Click {
        is_trusted: bool,
        detail: i32,
        client_x: f64,
        client_y: f64,
    },
    FocusIn,
    KeyDown {
        key: String,
        is_composing: bool,
    },
}

impl CaptureEventKind {
    /// Returns the single-bit [`CaptureEventMask`] corresponding to this event variant.
    #[inline]
    pub const fn mask(&self) -> CaptureEventMask {
        match self {
            Self::PointerDown { .. } => CaptureEventMask::POINTER_DOWN,
            Self::Click { .. } => CaptureEventMask::CLICK,
            Self::FocusIn => CaptureEventMask::FOCUS_IN,
            Self::KeyDown { .. } => CaptureEventMask::KEY_DOWN,
        }
    }

    /// Convenience constructor for a mouse `PointerDown` capture event at `(client_x, client_y)`.
    pub fn mouse_down(client_x: f64, client_y: f64) -> Self {
        Self::PointerDown {
            pointer_type: "mouse".to_string(),
            client_x,
            client_y,
        }
    }

    /// Convenience constructor for a touch `PointerDown` capture event at `(client_x, client_y)`.
    pub fn touch_down(client_x: f64, client_y: f64) -> Self {
        Self::PointerDown {
            pointer_type: "touch".to_string(),
            client_x,
            client_y,
        }
    }

    /// Convenience constructor for a `Click` capture event.
    pub fn click(is_trusted: bool, detail: i32, client_x: f64, client_y: f64) -> Self {
        Self::Click {
            is_trusted,
            detail,
            client_x,
            client_y,
        }
    }

    /// Convenience constructor for a non-composing `KeyDown` capture event.
    pub fn key_down(key: impl Into<String>) -> Self {
        Self::KeyDown {
            key: key.into(),
            is_composing: false,
        }
    }
}

/// Intercepted capture-phase event carrying target information and resolved ancestor path IDs.
#[derive(Clone, Debug, PartialEq)]
pub struct CaptureEvent {
    pub kind: CaptureEventKind,
    pub target_id: Option<String>,
    pub path_ids: Vec<String>,
}

impl CaptureEvent {
    /// Checks whether the interaction occurred outside all provided boundary element IDs.
    pub fn is_outside_boundary(&self, boundary_ids: &[&str]) -> bool {
        if boundary_ids.is_empty() {
            return true;
        }
        !self
            .path_ids
            .iter()
            .any(|id| boundary_ids.contains(&id.as_str()))
    }
}

/// Lifecycle events for CSS animations and transitions during element presence phases.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransitionLifecycleEvent {
    Missing,
    Hidden,
    NoAnimation,
    AnimationEnd { animation_name: String },
    AnimationCancel { animation_name: String },
    TransitionEnd { property_name: String },
    TransitionCancel { property_name: String },
    Stopped,
}

/// Observes layout and dimensional changes of an element using a specific box model.
pub fn observe_element_resize_with_box(
    element_id: &str,
    box_model: ResizeObserverBox,
    callback: impl FnMut(ResizeEntry) + 'static,
) -> Result<WatcherGuard> {
    crate::dom::backend::observe_element_resize_with_box(element_id, box_model, Box::new(callback))
}

/// Observes layout and dimensional changes of an element across Web and Native hosts using default border-box sizing.
pub fn observe_element_resize(
    element_id: &str,
    callback: impl FnMut(ResizeEntry) + 'static,
) -> Result<WatcherGuard> {
    observe_element_resize_with_box(element_id, ResizeObserverBox::BorderBox, callback)
}

/// Observes selected global capture-phase interactions across Web and Native hosts.
pub fn observe_capture_events_with_mask(
    mask: CaptureEventMask,
    callback: impl FnMut(CaptureEvent) + 'static,
) -> Result<WatcherGuard> {
    crate::dom::backend::observe_capture_events_with_mask(mask, Box::new(callback))
}

/// Observes global capture-phase interactions across Web and Native hosts.
pub fn observe_capture_events(
    callback: impl FnMut(CaptureEvent) + 'static,
) -> Result<WatcherGuard> {
    observe_capture_events_with_mask(CaptureEventMask::ALL, callback)
}

/// Observes transition and animation lifecycle for an element, including resolved-state fallbacks.
pub fn observe_transition_lifecycle(
    element_id: &str,
    callback: impl FnMut(TransitionLifecycleEvent) + 'static,
) -> Result<WatcherGuard> {
    crate::dom::backend::observe_transition_lifecycle(element_id, Box::new(callback))
}

/// Observes reset events dispatched on an element's enclosing form.
pub fn observe_form_reset(
    element_id: &str,
    form_id: Option<&str>,
    callback: impl FnMut() + 'static,
) -> Result<WatcherGuard> {
    crate::dom::backend::observe_form_reset(element_id, form_id, Box::new(callback))
}

/// Observes passive document-wide pointer move coordinates `(client_x, client_y)` across Web and Native hosts.
///
/// Uses passive document observation and never acquires W3C pointer capture.
pub fn observe_pointer_move(callback: impl FnMut(f64, f64) + 'static) -> Result<WatcherGuard> {
    crate::dom::backend::observe_pointer_move(Box::new(callback))
}

/// Observes passive document-wide `pointerup` events across Web and Native hosts.
pub fn observe_pointer_up(callback: impl FnMut() + 'static) -> Result<WatcherGuard> {
    crate::dom::backend::observe_pointer_up(Box::new(callback))
}

/// Observes host window `blur` events across Web and Native hosts.
pub fn observe_window_blur(callback: impl FnMut() + 'static) -> Result<WatcherGuard> {
    crate::dom::backend::observe_window_blur(Box::new(callback))
}

/// Observes scroll events across viewport and scrollable ancestor containers.
pub fn observe_scroll(callback: impl FnMut() + 'static) -> Result<WatcherGuard> {
    crate::dom::backend::observe_scroll(Box::new(callback))
}

/// Observes viewport or document-level scroll events across Web and Native hosts.
pub fn observe_viewport_scroll(
    callback: impl FnMut() + 'static,
) -> Result<WatcherGuard> {
    crate::dom::backend::observe_viewport_scroll(Box::new(callback))
}

/// Observes scroll events on a specific element container across Web and Native hosts.
pub fn observe_element_scroll(
    element_id: &str,
    callback: impl FnMut() + 'static,
) -> Result<WatcherGuard> {
    crate::dom::backend::observe_element_scroll(element_id, Box::new(callback))
}

/// Observes host document visibility transitions across Web, Native, and Mock backends.
pub fn observe_visibility_change(
    callback: impl FnMut(DocumentVisibility) + 'static,
) -> std::result::Result<WatcherGuard, HostError> {
    crate::dom::backend::observe_visibility_change(Box::new(callback))
}

/// Observes loading of an image through an off-DOM browser preloader or the native resource pipeline.
pub fn watch_image_load(
    src: &str,
    options: ImageLoadOptions,
    callback: impl FnMut(ImageLoadStatus) + 'static,
) -> Result<WatcherGuard> {
    crate::dom::backend::watch_image_load(src, options, Box::new(callback))
}
