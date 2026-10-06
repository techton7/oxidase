//! Core types, traits, and configurations for Floating UI engine.

use std::collections::HashMap;
use std::fmt::Debug;
use std::ptr;

use serde::{Serialize, de::DeserializeOwned};

use crate::detect_overflow::DetectOverflowOptions;
use crate::geometry::{
    ClientRectObject, Coords, Dimensions, ElementOrVirtual, ElementOrWindow, ElementRects, Length,
    OwnedElementOrWindow, Placement, Rect, SideObject, Strategy,
};

pub type DerivableFn<'a, Element, Window, T> = &'a dyn Fn(MiddlewareState<Element, Window>) -> T;

pub enum Derivable<'a, Element: Clone + 'static, Window: Clone, T: Clone> {
    Value(T),
    Fn(DerivableFn<'a, Element, Window, T>),
}

impl<Element: Clone, Window: Clone, T: Clone> Clone for Derivable<'_, Element, Window, T> {
    fn clone(&self) -> Self {
        match self {
            Self::Value(value) => Self::Value(value.clone()),
            Self::Fn(value) => Self::Fn(*value),
        }
    }
}

impl<Element: Clone, Window: Clone, T: Clone> Derivable<'_, Element, Window, T> {
    pub fn evaluate(&self, state: MiddlewareState<Element, Window>) -> T {
        match self {
            Derivable::Value(value) => value.clone(),
            Derivable::Fn(func) => func(state),
        }
    }
}

impl<Element: Clone, Window: Clone, T: Clone> From<T> for Derivable<'_, Element, Window, T> {
    fn from(value: T) -> Self {
        Derivable::Value(value)
    }
}

impl<'a, Element: Clone, Window: Clone, T: Clone> From<DerivableFn<'a, Element, Window, T>>
    for Derivable<'a, Element, Window, T>
{
    fn from(value: DerivableFn<'a, Element, Window, T>) -> Self {
        Derivable::Fn(value)
    }
}

impl<Element: Clone, Window: Clone, T: Clone + PartialEq> PartialEq
    for Derivable<'_, Element, Window, T>
{
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Value(a), Self::Value(b)) => a == b,
            (Self::Fn(a), Self::Fn(b)) => ptr::eq(a, b),
            _ => false,
        }
    }
}

/// Arguments for [`Platform::get_element_rects`].
pub struct GetElementRectsArgs<'a, Element: Clone + 'static> {
    pub reference: ElementOrVirtual<'a, Element>,
    pub floating: &'a Element,
    pub strategy: Strategy,
}

/// Arguments for [`Platform::get_clipping_rect`].
pub struct GetClippingRectArgs<'a, Element> {
    pub element: &'a Element,
    pub boundary: Boundary<Element>,
    pub root_boundary: RootBoundary,
    pub strategy: Strategy,
}

/// Arguments for [`Platform::convert_offset_parent_relative_rect_to_viewport_relative_rect`].
pub struct ConvertOffsetParentRelativeRectToViewportRelativeRectArgs<
    'a,
    Element: Clone + 'static,
    Window: Clone,
> {
    pub elements: Option<Elements<'a, Element>>,
    pub rect: Rect,
    pub offset_parent: Option<ElementOrWindow<'a, Element, Window>>,
    pub strategy: Strategy,
}

/// Boundary constraint for overflow detection.
#[derive(Clone, Debug, PartialEq)]
pub enum Boundary<Element = String> {
    ClippingAncestors,
    Element(Element),
    Elements(Vec<Element>),
    RootViewport,
    Document,
    Custom(Rect),
}

/// Root boundary constraint for overflow detection.
#[derive(Clone, Debug, PartialEq)]
pub enum RootBoundary {
    Viewport,
    LayoutViewport,
    Document,
    Rect(Rect),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ElementContext {
    Reference,
    Floating,
}

/// Element-level platform interface methods parameterized by concrete element and window handles.
///
/// This trait matches upstream Floating UI core architecture, allowing [`compute_position`][crate::compute_position]
/// and middlewares to operate generically over host element types (`web_sys::Element` on Web, `blitz_dom::NodeId`
/// on Native Blitz, or `String` IDs).
///
/// For the cross-boundary host-neutral platform substrate operating on element ID strings, see [`FloatingPlatform`].
pub trait Platform<Element: Clone + 'static, Window: Clone + 'static>: Debug {
    fn get_element_rects(&self, args: GetElementRectsArgs<Element>) -> ElementRects;

    fn get_clipping_rect(&self, args: GetClippingRectArgs<Element>) -> Rect;

    fn get_dimensions(&self, element: &Element) -> Dimensions;

    fn convert_offset_parent_relative_rect_to_viewport_relative_rect(
        &self,
        _args: ConvertOffsetParentRelativeRectToViewportRelativeRectArgs<Element, Window>,
    ) -> Option<Rect> {
        None
    }

    fn get_offset_parent(
        &self,
        _element: &Element,
    ) -> Option<OwnedElementOrWindow<Element, Window>> {
        None
    }

    fn get_document_element(&self, _element: &Element) -> Option<Element> {
        None
    }

    fn get_client_rects(
        &self,
        _element: ElementOrVirtual<Element>,
    ) -> Option<Vec<ClientRectObject>> {
        None
    }

    fn is_rtl(&self, _element: &Element) -> Option<bool> {
        None
    }

    fn get_scale(&self, _element: &Element) -> Option<Coords> {
        None
    }

    fn get_client_length(&self, _element: &Element, _length: Length) -> Option<f64> {
        None
    }

    fn detect_overflow(
        &self,
        state: MiddlewareState<Element, Window>,
        options: DetectOverflowOptions<Element>,
    ) -> SideObject {
        crate::detect_overflow::detect_overflow(state, options)
    }
}

/// Host-neutral platform interface for measuring elements and discovering clipping hierarchies,
/// as defined in the Phase 6 feature ledger.
///
/// Unlike [`Platform<Element, Window>`], which operates on concrete host DOM handles, `FloatingPlatform`
/// operates across framework boundaries using element IDs (`&str`). Both [`WebFloatingPlatform`][crate::platform::WebFloatingPlatform]
/// and [`NativeFloatingPlatform`][crate::platform::NativeFloatingPlatform] implement `FloatingPlatform` as their
/// primary substrate contract, while also implementing `Platform` to bridge directly into the pure positioning engine.
pub trait FloatingPlatform: Debug {
    /// Measures bounding client rects for reference and floating elements.
    fn get_element_rects(&self, reference_id: &str, floating_id: &str) -> crate::Result<ElementRects>;

    /// Computes the visible clipping boundary rect for an element constrained by boundary rules.
    fn get_clipping_rect(&self, element_id: &str, boundary: Boundary<String>) -> crate::Result<Rect>;

    /// Returns the layout dimensions (width, height) of an element.
    fn get_dimensions(&self, element_id: &str) -> crate::Result<Dimensions>;

    /// Checks if the element's computed text direction is right-to-left.
    fn is_rtl(&self, element_id: &str) -> bool;

    /// Traverses the element's ancestor tree, collecting all scrollable/clipping ancestor element IDs.
    fn get_overflow_ancestors(&self, element_id: &str) -> crate::Result<Vec<String>>;
}

/// Data stored by middleware.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MiddlewareData {
    values: HashMap<String, serde_json::Value>,
}

impl MiddlewareData {
    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        self.values.get(key)
    }

    pub fn get_as<D: DeserializeOwned>(&self, key: &str) -> Option<D> {
        self.values.get(key).map(|value| {
            serde_json::from_value::<D>(value.clone()).expect("JSON should be valid data.")
        })
    }

    pub fn set(&mut self, key: &str, value: serde_json::Value) {
        self.values.insert(key.into(), value);
    }

    pub fn set_as<S: Serialize>(&mut self, key: &str, value: S) {
        self.values.insert(
            key.into(),
            serde_json::to_value(value).expect("Data should be valid JSON."),
        );
    }
}

/// Options for [`compute_position`][crate::compute_position::compute_position].
#[derive(Clone)]
pub struct ComputePositionConfig<'a, Element: 'static, Window: 'static> {
    /// Object to interface with the current platform.
    pub platform: &'a dyn Platform<Element, Window>,

    /// Where to place the floating element relative to the reference element.
    ///
    /// Defaults to [`Placement::Bottom`].
    pub placement: Option<Placement>,

    /// The strategy to use when positioning the floating element.
    ///
    /// Defaults to [`Strategy::Absolute`].
    pub strategy: Option<Strategy>,

    /// Array of middleware objects to modify the positioning or provide data for rendering.
    ///
    /// Defaults to an empty vector.
    pub middleware: Option<Vec<Box<dyn Middleware<Element, Window>>>>,
}

impl<'a, Element, Window> ComputePositionConfig<'a, Element, Window> {
    pub fn new(platform: &'a dyn Platform<Element, Window>) -> Self {
        ComputePositionConfig {
            platform,
            placement: None,
            strategy: None,
            middleware: None,
        }
    }

    pub fn platform(mut self, value: &'a dyn Platform<Element, Window>) -> Self {
        self.platform = value;
        self
    }

    pub fn placement(mut self, value: Placement) -> Self {
        self.placement = Some(value);
        self
    }

    pub fn strategy(mut self, value: Strategy) -> Self {
        self.strategy = Some(value);
        self
    }

    pub fn middleware(mut self, value: Vec<Box<dyn Middleware<Element, Window>>>) -> Self {
        self.middleware = Some(value);
        self
    }
}

/// Return value of [`compute_position`][crate::compute_position::compute_position].
#[derive(Clone, Debug, PartialEq)]
pub struct ComputePositionReturn {
    pub x: f64,
    pub y: f64,

    /// The final chosen placement of the floating element.
    pub placement: Placement,

    /// The strategy used to position the floating element.
    pub strategy: Strategy,

    /// Object containing data returned from all middleware, keyed by their name.
    pub middleware_data: MiddlewareData,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ResetRects {
    True,
    Value(ElementRects),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResetValue {
    pub placement: Option<Placement>,
    pub rects: Option<ResetRects>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Reset {
    True,
    Value(ResetValue),
}

/// Return value of [`Middleware::compute`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MiddlewareReturn {
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub data: Option<serde_json::Value>,
    pub reset: Option<Reset>,
}

/// Object-safe clone support for `Middleware`.
pub trait MiddlewareClone<Element: Clone + 'static, Window: Clone + 'static> {
    fn clone_box(&self) -> Box<dyn Middleware<Element, Window>>;
}

impl<T, Element: Clone + 'static, Window: Clone + 'static> MiddlewareClone<Element, Window> for T
where
    T: Middleware<Element, Window> + Clone + 'static,
{
    fn clone_box(&self) -> Box<dyn Middleware<Element, Window>> {
        Box::new(self.clone())
    }
}

/// Middleware used by [`compute_position`][crate::compute_position::compute_position].
pub trait Middleware<Element: Clone + 'static, Window: Clone + 'static>:
    MiddlewareClone<Element, Window>
{
    /// The name of this middleware.
    fn name(&self) -> &'static str;

    /// Executes this middleware.
    fn compute(&self, state: MiddlewareState<Element, Window>) -> MiddlewareReturn;
}

impl<Element: Clone + 'static, Window: Clone + 'static> Clone
    for Box<dyn Middleware<Element, Window>>
{
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// Middleware with options.
pub trait MiddlewareWithOptions<Element: Clone, Window: Clone, O: Clone> {
    /// The options passed to this middleware.
    fn options(&self) -> &Derivable<'_, Element, Window, O>;
}

pub struct Elements<'a, Element: Clone + 'static> {
    pub reference: ElementOrVirtual<'a, Element>,
    pub floating: &'a Element,
}

impl<'a, Element: Clone + 'static> Elements<'a, Element> {
    pub fn get_element_context(
        &self,
        element_context: ElementContext,
    ) -> ElementOrVirtual<'a, Element> {
        match element_context {
            ElementContext::Reference => self.reference.clone(),
            ElementContext::Floating => self.floating.into(),
        }
    }
}

impl<Element: Clone + 'static> Clone for Elements<'_, Element> {
    fn clone(&self) -> Self {
        Self {
            reference: self.reference.clone(),
            floating: self.floating,
        }
    }
}

/// State passed to [`Middleware::compute`].
pub struct MiddlewareState<'a, Element: Clone + 'static, Window: Clone> {
    pub x: f64,
    pub y: f64,
    pub initial_placement: Placement,
    pub placement: Placement,
    pub strategy: Strategy,
    pub middleware_data: &'a MiddlewareData,
    pub elements: Elements<'a, Element>,
    pub rects: &'a ElementRects,
    pub platform: &'a dyn Platform<Element, Window>,
}

impl<Element: Clone + 'static, Window: Clone> Clone for MiddlewareState<'_, Element, Window> {
    fn clone(&self) -> Self {
        Self {
            x: self.x,
            y: self.y,
            initial_placement: self.initial_placement,
            placement: self.placement,
            strategy: self.strategy,
            middleware_data: self.middleware_data,
            elements: self.elements.clone(),
            rects: self.rects,
            platform: self.platform,
        }
    }
}
