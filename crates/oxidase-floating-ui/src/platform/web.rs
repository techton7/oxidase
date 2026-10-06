//! Web (`wasm32`) Floating UI platform adapter backed by `web-sys`.

use std::sync::atomic::{AtomicU64, Ordering};

use wasm_bindgen::JsCast;
use web_sys::{CssStyleDeclaration, Document, Element, HtmlElement, Window};

use crate::detect_overflow::DetectOverflowOptions;
use crate::error::{Error, Result};
use crate::geometry::{
    Coords, Dimensions, ElementOrVirtual, ElementRects, OwnedElementOrWindow, Rect, SideObject,
    Strategy,
};
use crate::types::{
    Boundary, FloatingPlatform, GetClippingRectArgs, GetElementRectsArgs, MiddlewareState,
    Platform,
};

static GENERATED_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Returns the active browser Window.
pub fn get_window() -> Result<Window> {
    web_sys::window().ok_or_else(|| Error::Unsupported("No active window in web host".into()))
}

/// Returns the active browser Document.
pub fn get_document() -> Result<Document> {
    get_window()?
        .document()
        .ok_or_else(|| Error::Unsupported("No active document in web host".into()))
}

/// Queries an element by ID from the active browser document.
pub fn get_element_by_id(id: &str) -> Result<Element> {
    get_document()?
        .get_element_by_id(id)
        .ok_or_else(|| Error::ElementNotFound(id.to_string()))
}

/// Returns the computed CSS style for an element.
pub fn get_computed_style(element: &Element) -> Result<CssStyleDeclaration> {
    get_window()?
        .get_computed_style(element)
        .map_err(|e| Error::Unsupported(format!("Failed to get computed style: {:?}", e)))?
        .ok_or_else(|| Error::Unsupported("Element has no computed style".into()))
}

/// CSS and offset dimensions with a fallback indicator.
#[derive(Clone, Debug, PartialEq)]
pub struct CssDimensions {
    pub dimensions: Dimensions,
    pub should_fallback: bool,
}

/// Computes element dimensions accounting for subpixel scaling and offset sizing.
pub fn get_css_dimensions(element: &Element) -> Result<CssDimensions> {
    let css = get_computed_style(element)?;
    let width = css
        .get_property_value("width")
        .unwrap_or_default()
        .replace("px", "")
        .trim()
        .parse::<f64>()
        .unwrap_or(0.0);
    let height = css
        .get_property_value("height")
        .unwrap_or_default()
        .replace("px", "")
        .trim()
        .parse::<f64>()
        .unwrap_or(0.0);

    let (offset_width, offset_height) = if element.is_instance_of::<HtmlElement>() {
        let html_el = element.unchecked_ref::<HtmlElement>();
        (html_el.offset_width() as f64, html_el.offset_height() as f64)
    } else {
        (width, height)
    };

    let should_fallback = width.round() != offset_width || height.round() != offset_height;
    let dimensions = if should_fallback {
        Dimensions {
            width: offset_width,
            height: offset_height,
        }
    } else {
        Dimensions { width, height }
    };

    Ok(CssDimensions {
        dimensions,
        should_fallback,
    })
}

/// Returns layout dimensions (width, height) for an element.
pub fn get_element_dimensions(element: &Element) -> Result<Dimensions> {
    get_css_dimensions(element).map(|d| d.dimensions)
}

/// Computes the visual scale factors (x, y) for an element.
pub fn get_scale(element: &Element) -> Coords {
    let rect = element.get_bounding_client_rect();
    let css_dim = get_css_dimensions(element).unwrap_or(CssDimensions {
        dimensions: Dimensions {
            width: rect.width(),
            height: rect.height(),
        },
        should_fallback: false,
    });

    let mut x = if css_dim.should_fallback {
        rect.width().round()
    } else {
        rect.width()
    } / css_dim.dimensions.width.max(0.001);

    let mut y = if css_dim.should_fallback {
        rect.height().round()
    } else {
        rect.height()
    } / css_dim.dimensions.height.max(0.001);

    if x == 0.0 || x.is_nan() || x.is_infinite() {
        x = 1.0;
    }
    if y == 0.0 || y.is_nan() || y.is_infinite() {
        y = 1.0;
    }

    Coords { x, y }
}

/// Checks if an element has right-to-left text direction.
pub fn is_rtl(element: &Element) -> bool {
    get_computed_style(element)
        .ok()
        .and_then(|css| css.get_property_value("direction").ok())
        .map(|dir| dir == "rtl")
        .unwrap_or(false)
}

/// Checks if an element establishes a CSS containing block.
pub fn is_containing_block(element: &Element) -> bool {
    let css = match get_computed_style(element) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let transform = css.get_property_value("transform").unwrap_or_default();
    let perspective = css.get_property_value("perspective").unwrap_or_default();
    let contain = css.get_property_value("contain").unwrap_or_default();
    let will_change = css.get_property_value("will-change").unwrap_or_default();

    (transform != "none" && !transform.is_empty())
        || (perspective != "none" && !perspective.is_empty())
        || contain == "paint"
        || contain == "layout"
        || contain == "strict"
        || contain == "content"
        || will_change.contains("transform")
        || will_change.contains("perspective")
}

/// Resolves the nearest offset parent for an element.
pub fn get_offset_parent(element: &Element) -> Option<Element> {
    if element.is_instance_of::<HtmlElement>() {
        let html_el = element.unchecked_ref::<HtmlElement>();
        if let Some(parent) = html_el.offset_parent() {
            return Some(parent);
        }
    }

    let mut curr = element.parent_element();
    while let Some(parent) = curr {
        let tag = parent.tag_name().to_lowercase();
        if tag == "html" || tag == "body" {
            return Some(parent);
        }
        if is_containing_block(&parent) {
            return Some(parent);
        }
        curr = parent.parent_element();
    }
    None
}

/// Checks whether an element qualifies as a scroll/clipping container.
pub fn is_overflow_element(element: &Element) -> bool {
    let css = match get_computed_style(element) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let display = css.get_property_value("display").unwrap_or_default();
    if display == "inline" || display == "contents" {
        return false;
    }
    let overflow = css.get_property_value("overflow").unwrap_or_default();
    let overflow_x = css.get_property_value("overflow-x").unwrap_or_default();
    let overflow_y = css.get_property_value("overflow-y").unwrap_or_default();
    let combined = format!("{overflow} {overflow_x} {overflow_y}");
    const OVERFLOW_VALUES: [&str; 5] = ["auto", "scroll", "overlay", "hidden", "clip"];
    OVERFLOW_VALUES.iter().any(|&val| combined.contains(val))
}

/// Collects all scrollable/clipping ancestor elements in order.
pub fn get_overflow_ancestors(element: &Element) -> Vec<Element> {
    let mut ancestors = Vec::new();
    let mut curr = element.parent_element();
    while let Some(parent) = curr {
        let tag = parent.tag_name().to_lowercase();
        if tag == "html" || tag == "body" {
            break;
        }
        if is_overflow_element(&parent) {
            ancestors.push(parent.clone());
        }
        curr = parent.parent_element();
    }
    ancestors
}

/// Queries the root viewport rectangle.
pub fn get_viewport_rect() -> Result<Rect> {
    let win = get_window()?;
    let width = win.inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(0.0);
    let height = win.inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(0.0);
    Ok(Rect::new(0.0, 0.0, width, height))
}

/// Queries the full document bounding rectangle.
pub fn get_document_rect() -> Result<Rect> {
    let doc = get_document()?;
    let doc_el = doc
        .document_element()
        .ok_or_else(|| Error::Unsupported("No documentElement".into()))?;
    let scroll_width = doc_el.scroll_width() as f64;
    let scroll_height = doc_el.scroll_height() as f64;
    let client_width = doc_el.client_width() as f64;
    let client_height = doc_el.client_height() as f64;
    let width = scroll_width.max(client_width);
    let height = scroll_height.max(client_height);
    Ok(Rect::new(0.0, 0.0, width, height))
}

/// Intersects two rectangles, returning an empty rectangle if there is no intersection.
fn intersect_rect(a: &Rect, b: &Rect) -> Rect {
    let left = a.x.max(b.x);
    let top = a.y.max(b.y);
    let right = (a.x + a.width).min(b.x + b.width);
    let bottom = (a.y + a.height).min(b.y + b.height);
    let width = (right - left).max(0.0);
    let height = (bottom - top).max(0.0);
    Rect::new(left, top, width, height)
}

/// Calculates the clipping boundary rectangle for an element under boundary constraints.
pub fn get_clipping_rect(element: &Element, boundary: Boundary<String>) -> Result<Rect> {
    let vp_rect = get_viewport_rect()?;
    match boundary {
        Boundary::RootViewport => Ok(vp_rect),
        Boundary::Document => get_document_rect(),
        Boundary::Custom(rect) => Ok(rect),
        Boundary::Element(id) => {
            let el = get_element_by_id(&id)?;
            let r = el.get_bounding_client_rect();
            let el_rect = Rect::new(r.left(), r.top(), r.width(), r.height());
            Ok(intersect_rect(&el_rect, &vp_rect))
        }
        Boundary::Elements(ids) => {
            let mut current = vp_rect;
            for id in ids {
                let el = get_element_by_id(&id)?;
                let r = el.get_bounding_client_rect();
                let el_rect = Rect::new(r.left(), r.top(), r.width(), r.height());
                current = intersect_rect(&current, &el_rect);
            }
            Ok(current)
        }
        Boundary::ClippingAncestors => {
            let ancestors = get_overflow_ancestors(element);
            let mut current = vp_rect;
            for anc in ancestors {
                let r = anc.get_bounding_client_rect();
                let client_left = anc.client_left() as f64;
                let client_top = anc.client_top() as f64;
                let client_width = anc.client_width() as f64;
                let client_height = anc.client_height() as f64;
                let anc_rect = if client_width > 0.0 && client_height > 0.0 {
                    Rect::new(
                        r.left() + client_left,
                        r.top() + client_top,
                        client_width,
                        client_height,
                    )
                } else {
                    Rect::new(r.left(), r.top(), r.width(), r.height())
                };
                current = intersect_rect(&current, &anc_rect);
            }
            Ok(current)
        }
    }
}

/// Measures element rects for positioning.
pub fn get_element_rects(
    reference: &Element,
    floating: &Element,
    strategy: Strategy,
) -> Result<ElementRects> {
    let ref_dom_rect = reference.get_bounding_client_rect();
    let floating_dim = get_element_dimensions(floating)?;

    let offset_parent = get_offset_parent(floating);
    let reference_rect = if let Some(parent) = offset_parent {
        let parent_rect = parent.get_bounding_client_rect();
        let scroll_x = parent.scroll_left() as f64;
        let scroll_y = parent.scroll_top() as f64;
        let client_left = parent.client_left() as f64;
        let client_top = parent.client_top() as f64;

        if strategy == Strategy::Fixed {
            Rect::new(
                ref_dom_rect.left(),
                ref_dom_rect.top(),
                ref_dom_rect.width(),
                ref_dom_rect.height(),
            )
        } else {
            Rect::new(
                ref_dom_rect.left() - parent_rect.left() - client_left + scroll_x,
                ref_dom_rect.top() - parent_rect.top() - client_top + scroll_y,
                ref_dom_rect.width(),
                ref_dom_rect.height(),
            )
        }
    } else {
        Rect::new(
            ref_dom_rect.left(),
            ref_dom_rect.top(),
            ref_dom_rect.width(),
            ref_dom_rect.height(),
        )
    };

    Ok(ElementRects {
        reference: reference_rect,
        floating: Rect::new(0.0, 0.0, floating_dim.width, floating_dim.height),
    })
}

/// Web platform adapter for Floating UI backed by `web-sys`.
///
/// Implements both the host-neutral [`FloatingPlatform`] substrate trait and the
/// generic [`Platform<Element, Window>`][Platform] trait for web browser targets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WebFloatingPlatform;

impl WebFloatingPlatform {
    /// Measures bounding client rects for reference and floating elements by ID.
    pub fn get_element_rects(&self, reference_id: &str, floating_id: &str) -> Result<ElementRects> {
        let reference = get_element_by_id(reference_id)?;
        let floating = get_element_by_id(floating_id)?;
        get_element_rects(&reference, &floating, Strategy::Absolute)
    }

    /// Computes the visible clipping boundary rect for an element constrained by boundary rules.
    pub fn get_clipping_rect(&self, element_id: &str, boundary: Boundary<String>) -> Result<Rect> {
        let element = get_element_by_id(element_id)?;
        get_clipping_rect(&element, boundary)
    }

    /// Returns the layout dimensions (width, height) of an element by ID.
    pub fn get_dimensions(&self, element_id: &str) -> Result<Dimensions> {
        let element = get_element_by_id(element_id)?;
        get_element_dimensions(&element)
    }

    /// Checks if the element's computed text direction is right-to-left.
    pub fn is_rtl(&self, element_id: &str) -> bool {
        get_element_by_id(element_id).map(|el| is_rtl(&el)).unwrap_or(false)
    }

    /// Traverses the element's ancestor tree, collecting all scrollable/clipping ancestor element IDs.
    pub fn get_overflow_ancestors(&self, element_id: &str) -> Result<Vec<String>> {
        let element = get_element_by_id(element_id)?;
        let ancestors = get_overflow_ancestors(&element);
        let ids = ancestors
            .into_iter()
            .map(|el| {
                let id = el.id();
                if id.is_empty() {
                    let gen_id = format!(
                        "ox-oa-{}",
                        GENERATED_ID_COUNTER.fetch_add(1, Ordering::Relaxed)
                    );
                    el.set_id(&gen_id);
                    gen_id
                } else {
                    id
                }
            })
            .collect();
        Ok(ids)
    }
}

impl FloatingPlatform for WebFloatingPlatform {
    fn get_element_rects(&self, reference_id: &str, floating_id: &str) -> Result<ElementRects> {
        self.get_element_rects(reference_id, floating_id)
    }

    fn get_clipping_rect(&self, element_id: &str, boundary: Boundary<String>) -> Result<Rect> {
        self.get_clipping_rect(element_id, boundary)
    }

    fn get_dimensions(&self, element_id: &str) -> Result<Dimensions> {
        self.get_dimensions(element_id)
    }

    fn is_rtl(&self, element_id: &str) -> bool {
        self.is_rtl(element_id)
    }

    fn get_overflow_ancestors(&self, element_id: &str) -> Result<Vec<String>> {
        self.get_overflow_ancestors(element_id)
    }
}

impl Platform<Element, Window> for WebFloatingPlatform {
    fn get_element_rects(&self, args: GetElementRectsArgs<Element>) -> ElementRects {
        let ref_el = match args.reference {
            ElementOrVirtual::Element(e) => e,
            ElementOrVirtual::VirtualElement(v) => {
                let r = v.get_bounding_client_rect();
                let dim = get_element_dimensions(args.floating).unwrap_or_default();
                return ElementRects {
                    reference: r.into(),
                    floating: Rect::new(0.0, 0.0, dim.width, dim.height),
                };
            }
        };
        get_element_rects(ref_el, args.floating, args.strategy).unwrap_or_else(|_| ElementRects {
            reference: Rect::default(),
            floating: Rect::default(),
        })
    }

    fn get_clipping_rect(&self, args: GetClippingRectArgs<Element>) -> Rect {
        let boundary = match args.boundary {
            Boundary::ClippingAncestors => Boundary::ClippingAncestors,
            Boundary::RootViewport => Boundary::RootViewport,
            Boundary::Document => Boundary::Document,
            Boundary::Custom(r) => Boundary::Custom(r),
            Boundary::Element(el) => Boundary::Element(el.id()),
            Boundary::Elements(els) => {
                Boundary::Elements(els.into_iter().map(|e| e.id()).collect())
            }
        };
        get_clipping_rect(args.element, boundary).unwrap_or_default()
    }

    fn get_dimensions(&self, element: &Element) -> Dimensions {
        get_element_dimensions(element).unwrap_or_default()
    }

    fn is_rtl(&self, element: &Element) -> Option<bool> {
        Some(is_rtl(element))
    }

    fn get_scale(&self, element: &Element) -> Option<Coords> {
        Some(get_scale(element))
    }

    fn get_offset_parent(
        &self,
        element: &Element,
    ) -> Option<OwnedElementOrWindow<Element, Window>> {
        get_offset_parent(element).map(OwnedElementOrWindow::Element)
    }

    fn get_document_element(&self, _element: &Element) -> Option<Element> {
        get_document().ok().and_then(|d| d.document_element())
    }

    fn detect_overflow(
        &self,
        state: MiddlewareState<Element, Window>,
        options: DetectOverflowOptions<Element>,
    ) -> SideObject {
        crate::detect_overflow::detect_overflow(state, options)
    }
}

impl Platform<String, ()> for WebFloatingPlatform {
    fn get_element_rects(&self, args: GetElementRectsArgs<String>) -> ElementRects {
        let ref_id = match args.reference {
            ElementOrVirtual::Element(id) => id,
            ElementOrVirtual::VirtualElement(v) => {
                let r = v.get_bounding_client_rect();
                let dim =
                    FloatingPlatform::get_dimensions(self, args.floating).unwrap_or_default();
                return ElementRects {
                    reference: r.into(),
                    floating: Rect::new(0.0, 0.0, dim.width, dim.height),
                };
            }
        };
        FloatingPlatform::get_element_rects(self, ref_id, args.floating).unwrap_or_else(|_| {
            ElementRects {
                reference: Rect::default(),
                floating: Rect::default(),
            }
        })
    }

    fn get_clipping_rect(&self, args: GetClippingRectArgs<String>) -> Rect {
        FloatingPlatform::get_clipping_rect(self, args.element, args.boundary).unwrap_or_default()
    }

    fn get_dimensions(&self, element: &String) -> Dimensions {
        FloatingPlatform::get_dimensions(self, element).unwrap_or_default()
    }

    fn is_rtl(&self, element: &String) -> Option<bool> {
        Some(FloatingPlatform::is_rtl(self, element))
    }
}
