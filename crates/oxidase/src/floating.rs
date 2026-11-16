//! Cross-host Floating UI platform adapter unifying Web (`web-sys`) and Native (`blitz-dom`).

use oxidase_floating_ui::geometry::{
    ClientRectObject, Coords, Dimensions, ElementOrVirtual, ElementRects, Length,
    OwnedElementOrWindow, Rect,
};
use oxidase_floating_ui::platform::FloatingPlatform;
use oxidase_floating_ui::types::{
    Boundary, ConvertOffsetParentRelativeRectToViewportRelativeRectArgs, GetClippingRectArgs,
    GetElementRectsArgs, Platform,
};

/// Unified Floating UI host platform adapter delegating to the active host backend.
#[derive(Clone, Debug)]
pub struct HostFloatingPlatform(Inner);

#[derive(Clone, Debug)]
enum Inner {
    #[cfg(target_arch = "wasm32")]
    Web(oxidase_floating_ui::platform::WebFloatingPlatform),
    #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
    Native(oxidase_floating_ui::platform::NativeFloatingPlatform),
    #[cfg(not(any(target_arch = "wasm32", feature = "native")))]
    #[allow(dead_code)]
    Uninhabited(std::convert::Infallible),
}

impl HostFloatingPlatform {
    /// Resolves the active host's Floating UI platform adapter, if available.
    ///
    /// - On Web (`wasm32`), returns `Some` wrapping `WebFloatingPlatform`.
    /// - On Native (`not(wasm32)` + `feature = "native"`), resolves `Document::current()`
    ///   and wraps its `BaseDocument` in `NativeFloatingPlatform`.
    /// - On headless/mock builds without `native` or `wasm32`, returns `None`.
    pub fn current() -> Option<Self> {
        #[cfg(target_arch = "wasm32")]
        {
            Some(Self(Inner::Web(
                oxidase_floating_ui::platform::WebFloatingPlatform,
            )))
        }
        #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
        {
            crate::dom::Document::current().map(|doc| {
                Self(Inner::Native(
                    oxidase_floating_ui::platform::NativeFloatingPlatform::new(doc.base().clone()),
                ))
            })
        }
        #[cfg(not(any(target_arch = "wasm32", feature = "native")))]
        {
            None
        }
    }
}

impl FloatingPlatform for HostFloatingPlatform {
    fn get_element_rects(
        &self,
        reference_id: &str,
        floating_id: &str,
    ) -> oxidase_floating_ui::Result<ElementRects> {
        match &self.0 {
            #[cfg(target_arch = "wasm32")]
            Inner::Web(platform) => {
                FloatingPlatform::get_element_rects(platform, reference_id, floating_id)
            }
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            Inner::Native(platform) => {
                FloatingPlatform::get_element_rects(platform, reference_id, floating_id)
            }
            #[cfg(not(any(target_arch = "wasm32", feature = "native")))]
            Inner::Uninhabited(never) => {
                let _ = (reference_id, floating_id);
                match *never {}
            }
        }
    }

    fn get_clipping_rect(
        &self,
        element_id: &str,
        boundary: Boundary<String>,
    ) -> oxidase_floating_ui::Result<Rect> {
        match &self.0 {
            #[cfg(target_arch = "wasm32")]
            Inner::Web(platform) => {
                FloatingPlatform::get_clipping_rect(platform, element_id, boundary)
            }
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            Inner::Native(platform) => {
                FloatingPlatform::get_clipping_rect(platform, element_id, boundary)
            }
            #[cfg(not(any(target_arch = "wasm32", feature = "native")))]
            Inner::Uninhabited(never) => {
                let _ = (element_id, boundary);
                match *never {}
            }
        }
    }

    fn get_dimensions(&self, element_id: &str) -> oxidase_floating_ui::Result<Dimensions> {
        match &self.0 {
            #[cfg(target_arch = "wasm32")]
            Inner::Web(platform) => FloatingPlatform::get_dimensions(platform, element_id),
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            Inner::Native(platform) => FloatingPlatform::get_dimensions(platform, element_id),
            #[cfg(not(any(target_arch = "wasm32", feature = "native")))]
            Inner::Uninhabited(never) => {
                let _ = element_id;
                match *never {}
            }
        }
    }

    fn is_rtl(&self, element_id: &str) -> bool {
        match &self.0 {
            #[cfg(target_arch = "wasm32")]
            Inner::Web(platform) => FloatingPlatform::is_rtl(platform, element_id),
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            Inner::Native(platform) => FloatingPlatform::is_rtl(platform, element_id),
            #[cfg(not(any(target_arch = "wasm32", feature = "native")))]
            Inner::Uninhabited(never) => {
                let _ = element_id;
                match *never {}
            }
        }
    }

    fn get_overflow_ancestors(
        &self,
        element_id: &str,
    ) -> oxidase_floating_ui::Result<Vec<String>> {
        match &self.0 {
            #[cfg(target_arch = "wasm32")]
            Inner::Web(platform) => FloatingPlatform::get_overflow_ancestors(platform, element_id),
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            Inner::Native(platform) => {
                FloatingPlatform::get_overflow_ancestors(platform, element_id)
            }
            #[cfg(not(any(target_arch = "wasm32", feature = "native")))]
            Inner::Uninhabited(never) => {
                let _ = element_id;
                match *never {}
            }
        }
    }
}

impl Platform<String, ()> for HostFloatingPlatform {
    fn get_element_rects(&self, args: GetElementRectsArgs<String>) -> ElementRects {
        match &self.0 {
            #[cfg(target_arch = "wasm32")]
            Inner::Web(platform) => Platform::<String, ()>::get_element_rects(platform, args),
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            Inner::Native(platform) => Platform::<String, ()>::get_element_rects(platform, args),
            #[cfg(not(any(target_arch = "wasm32", feature = "native")))]
            Inner::Uninhabited(never) => {
                let _ = args;
                match *never {}
            }
        }
    }

    fn get_clipping_rect(&self, args: GetClippingRectArgs<String>) -> Rect {
        match &self.0 {
            #[cfg(target_arch = "wasm32")]
            Inner::Web(platform) => Platform::<String, ()>::get_clipping_rect(platform, args),
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            Inner::Native(platform) => Platform::<String, ()>::get_clipping_rect(platform, args),
            #[cfg(not(any(target_arch = "wasm32", feature = "native")))]
            Inner::Uninhabited(never) => {
                let _ = args;
                match *never {}
            }
        }
    }

    fn get_dimensions(&self, element: &String) -> Dimensions {
        match &self.0 {
            #[cfg(target_arch = "wasm32")]
            Inner::Web(platform) => Platform::<String, ()>::get_dimensions(platform, element),
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            Inner::Native(platform) => Platform::<String, ()>::get_dimensions(platform, element),
            #[cfg(not(any(target_arch = "wasm32", feature = "native")))]
            Inner::Uninhabited(never) => {
                let _ = element;
                match *never {}
            }
        }
    }

    fn convert_offset_parent_relative_rect_to_viewport_relative_rect(
        &self,
        args: ConvertOffsetParentRelativeRectToViewportRelativeRectArgs<String, ()>,
    ) -> Option<Rect> {
        match &self.0 {
            #[cfg(target_arch = "wasm32")]
            Inner::Web(platform) => Platform::<String, ()>::convert_offset_parent_relative_rect_to_viewport_relative_rect(platform, args),
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            Inner::Native(platform) => Platform::<String, ()>::convert_offset_parent_relative_rect_to_viewport_relative_rect(platform, args),
            #[cfg(not(any(target_arch = "wasm32", feature = "native")))]
            Inner::Uninhabited(never) => {
                let _ = args;
                match *never {}
            }
        }
    }

    fn get_offset_parent(&self, element: &String) -> Option<OwnedElementOrWindow<String, ()>> {
        match &self.0 {
            #[cfg(target_arch = "wasm32")]
            Inner::Web(platform) => Platform::<String, ()>::get_offset_parent(platform, element),
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            Inner::Native(platform) => Platform::<String, ()>::get_offset_parent(platform, element),
            #[cfg(not(any(target_arch = "wasm32", feature = "native")))]
            Inner::Uninhabited(never) => {
                let _ = element;
                match *never {}
            }
        }
    }

    fn get_document_element(&self, element: &String) -> Option<String> {
        match &self.0 {
            #[cfg(target_arch = "wasm32")]
            Inner::Web(platform) => Platform::<String, ()>::get_document_element(platform, element),
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            Inner::Native(platform) => {
                Platform::<String, ()>::get_document_element(platform, element)
            }
            #[cfg(not(any(target_arch = "wasm32", feature = "native")))]
            Inner::Uninhabited(never) => {
                let _ = element;
                match *never {}
            }
        }
    }

    fn get_client_rects(
        &self,
        element: ElementOrVirtual<String>,
    ) -> Option<Vec<ClientRectObject>> {
        match &self.0 {
            #[cfg(target_arch = "wasm32")]
            Inner::Web(platform) => Platform::<String, ()>::get_client_rects(platform, element),
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            Inner::Native(platform) => Platform::<String, ()>::get_client_rects(platform, element),
            #[cfg(not(any(target_arch = "wasm32", feature = "native")))]
            Inner::Uninhabited(never) => {
                let _ = element;
                match *never {}
            }
        }
    }

    fn is_rtl(&self, element: &String) -> Option<bool> {
        match &self.0 {
            #[cfg(target_arch = "wasm32")]
            Inner::Web(platform) => Platform::<String, ()>::is_rtl(platform, element),
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            Inner::Native(platform) => Platform::<String, ()>::is_rtl(platform, element),
            #[cfg(not(any(target_arch = "wasm32", feature = "native")))]
            Inner::Uninhabited(never) => {
                let _ = element;
                match *never {}
            }
        }
    }

    fn get_scale(&self, element: &String) -> Option<Coords> {
        match &self.0 {
            #[cfg(target_arch = "wasm32")]
            Inner::Web(platform) => Platform::<String, ()>::get_scale(platform, element),
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            Inner::Native(platform) => Platform::<String, ()>::get_scale(platform, element),
            #[cfg(not(any(target_arch = "wasm32", feature = "native")))]
            Inner::Uninhabited(never) => {
                let _ = element;
                match *never {}
            }
        }
    }

    fn get_client_length(&self, element: &String, length: Length) -> Option<f64> {
        match &self.0 {
            #[cfg(target_arch = "wasm32")]
            Inner::Web(platform) => {
                Platform::<String, ()>::get_client_length(platform, element, length)
            }
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            Inner::Native(platform) => {
                Platform::<String, ()>::get_client_length(platform, element, length)
            }
            #[cfg(not(any(target_arch = "wasm32", feature = "native")))]
            Inner::Uninhabited(never) => {
                let _ = (element, length);
                match *never {}
            }
        }
    }
}
