//! Blitz Native platform adapter backed by `blitz-dom`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use blitz_dom::{BaseDocument, NodeId, QualName, local_name, ns};

use crate::detect_overflow::DetectOverflowOptions;
use crate::error::{Error, Result};
use crate::geometry::{Coords, Dimensions, ElementOrVirtual, ElementRects, Rect, SideObject};
use crate::types::{
    Boundary, FloatingPlatform, GetClippingRectArgs, GetElementRectsArgs, MiddlewareState,
    Platform,
};

static GENERATED_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Measures the bounding client rectangle of a native node in viewport coordinates.
pub fn measure_node_rect(base: &BaseDocument, node_id: NodeId) -> Option<Rect> {
    let node = base.get_node(node_id)?;
    let layout = node.final_layout();
    let width = layout.size.width as f64;
    let height = layout.size.height as f64;

    let mut x = layout.location.x as f64;
    let mut y = layout.location.y as f64;

    let root_id = base.root_node().id;
    let mut current_parent = node.parent;
    while let Some(parent_id) = current_parent {
        if parent_id == root_id {
            break;
        }
        if let Some(parent_node) = base.get_node(parent_id) {
            let parent_layout = parent_node.final_layout();
            x += parent_layout.location.x as f64 - parent_node.scroll_offset().x;
            y += parent_layout.location.y as f64 - parent_node.scroll_offset().y;
            current_parent = parent_node.parent;
        } else {
            break;
        }
    }

    let vp_scroll = base.viewport_scroll();
    x -= vp_scroll.x;
    y -= vp_scroll.y;

    Some(Rect::new(x, y, width, height))
}

/// Returns the layout dimensions (width, height) of a native node.
pub fn get_node_dimensions(base: &BaseDocument, node_id: NodeId) -> Option<Dimensions> {
    let node = base.get_node(node_id)?;
    let layout = node.final_layout();
    Some(Dimensions {
        width: layout.size.width as f64,
        height: layout.size.height as f64,
    })
}

/// Checks if a native node has right-to-left text direction.
pub fn is_node_rtl(base: &BaseDocument, node_id: NodeId) -> bool {
    let Some(node) = base.get_node(node_id) else {
        return false;
    };
    if let Some(styles) = node.primary_styles() {
        let dir = format!("{:?}", styles.clone_direction());
        dir.eq_ignore_ascii_case("rtl")
    } else {
        false
    }
}

/// Checks if a native node's computed styles specify clipping or scrollable overflow.
pub fn is_overflow_node(base: &BaseDocument, node_id: NodeId) -> bool {
    let Some(node) = base.get_node(node_id) else {
        return false;
    };
    if let Some(styles) = node.primary_styles() {
        let ox = format!("{:?}", styles.clone_overflow_x());
        let oy = format!("{:?}", styles.clone_overflow_y());
        ox != "Visible" || oy != "Visible"
    } else {
        false
    }
}

/// Returns the root viewport rectangle in CSS pixels.
pub fn get_root_viewport_rect(base: &BaseDocument) -> Rect {
    let vp = base.viewport();
    let scale = (vp.hidpi_scale * vp.zoom).max(0.001) as f64;
    let width = vp.window_size.0 as f64 / scale;
    let height = vp.window_size.1 as f64 / scale;
    Rect::new(0.0, 0.0, width, height)
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

/// Collects all scrollable/clipping ancestor `NodeId`s in traversal order.
pub fn get_node_overflow_ancestors(base: &BaseDocument, node_id: NodeId) -> Vec<NodeId> {
    let chain = base.node_chain(node_id);
    let root_id = base.root_node().id;
    let mut ancestors = Vec::new();
    for &anc_id in chain.iter().skip(1) {
        if anc_id == root_id {
            break;
        }
        if is_overflow_node(base, anc_id) {
            ancestors.push(anc_id);
        }
    }
    ancestors
}

/// Calculates the clipping boundary rectangle for a native node under boundary constraints.
pub fn get_clipping_rect_for_node(
    base: &BaseDocument,
    node_id: NodeId,
    boundary: Boundary<String>,
) -> Result<Rect> {
    get_clipping_rect_for_node_with_resolver(base, node_id, boundary, |id| base.get_element_by_id(id))
}

/// Calculates the clipping boundary rectangle for a native node resolving element IDs via a custom resolver.
pub fn get_clipping_rect_for_node_with_resolver<F>(
    base: &BaseDocument,
    node_id: NodeId,
    boundary: Boundary<String>,
    resolve_id: F,
) -> Result<Rect>
where
    F: Fn(&str) -> Option<NodeId>,
{
    let vp_rect = get_root_viewport_rect(base);
    match boundary {
        Boundary::RootViewport | Boundary::Document => Ok(vp_rect),
        Boundary::Custom(rect) => Ok(rect),
        Boundary::Element(id) => {
            let target_node = resolve_id(&id)
                .ok_or_else(|| Error::ElementNotFound(id))?;
            let rect = measure_node_rect(base, target_node).ok_or_else(|| {
                Error::Unsupported(format!("Could not measure element {:?}", target_node))
            })?;
            Ok(intersect_rect(&vp_rect, &rect))
        }
        Boundary::Elements(ids) => {
            let mut current = vp_rect;
            for id in ids {
                let target_node = resolve_id(&id)
                    .ok_or_else(|| Error::ElementNotFound(id))?;
                let rect = measure_node_rect(base, target_node).ok_or_else(|| {
                    Error::Unsupported(format!("Could not measure element {:?}", target_node))
                })?;
                current = intersect_rect(&current, &rect);
            }
            Ok(current)
        }
        Boundary::ClippingAncestors => {
            let ancestors = get_node_overflow_ancestors(base, node_id);
            let mut current = vp_rect;
            for anc_id in ancestors {
                if let Some(rect) = measure_node_rect(base, anc_id) {
                    current = intersect_rect(&current, &rect);
                }
            }
            Ok(current)
        }
    }
}

/// Native Blitz platform adapter for Floating UI backed by `blitz_dom::BaseDocument`.
///
/// Implements both the host-neutral [`FloatingPlatform`] substrate trait and the
/// generic [`Platform<NodeId, ()>`][Platform] and [`Platform<String, ()>`][Platform]
/// traits for native Blitz environments.
#[derive(Clone)]
pub struct NativeFloatingPlatform {
    doc: Rc<RefCell<BaseDocument>>,
    generated_ids: Rc<RefCell<HashMap<NodeId, String>>>,
    generated_nodes: Rc<RefCell<HashMap<String, NodeId>>>,
}

impl std::fmt::Debug for NativeFloatingPlatform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("NativeFloatingPlatform");
        if let Ok(base) = self.doc.try_borrow() {
            s.field("document_id", &base.id());
        } else {
            s.field("borrowed", &true);
        }
        s.finish()
    }
}

impl NativeFloatingPlatform {
    /// Creates a new NativeFloatingPlatform wrapping an `Rc<RefCell<BaseDocument>>`.
    pub fn new(doc: Rc<RefCell<BaseDocument>>) -> Self {
        Self {
            doc,
            generated_ids: Rc::new(RefCell::new(HashMap::new())),
            generated_nodes: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    /// Creates a new NativeFloatingPlatform from an owned `BaseDocument`.
    pub fn from_base(doc: BaseDocument) -> Self {
        Self::new(Rc::new(RefCell::new(doc)))
    }

    /// Access the underlying `Rc<RefCell<BaseDocument>>`.
    pub fn document(&self) -> &Rc<RefCell<BaseDocument>> {
        &self.doc
    }

    /// Resolves an element's `NodeId` by explicit ID or generated ancestor ID.
    pub fn get_element_by_id(&self, id: &str) -> Option<NodeId> {
        let base = self.doc.borrow();
        if let Some(node_id) = base.get_element_by_id(id) {
            return Some(node_id);
        }
        self.generated_nodes.borrow().get(id).copied()
    }

    /// Measures element rects given native `NodeId`s.
    pub fn get_element_rects_by_node_id(
        &self,
        reference: NodeId,
        floating: NodeId,
    ) -> Result<ElementRects> {
        let base = self.doc.borrow();
        let ref_rect = measure_node_rect(&base, reference).ok_or_else(|| {
            Error::Unsupported(format!("Could not measure reference node {:?}", reference))
        })?;
        let floating_dim = get_node_dimensions(&base, floating).ok_or_else(|| {
            Error::Unsupported(format!(
                "Could not get dimensions for floating node {:?}",
                floating
            ))
        })?;

        Ok(ElementRects {
            reference: ref_rect,
            floating: Rect::new(0.0, 0.0, floating_dim.width, floating_dim.height),
        })
    }

    /// Returns layout dimensions for a native node.
    pub fn get_dimensions_by_node_id(&self, element: NodeId) -> Result<Dimensions> {
        let base = self.doc.borrow();
        get_node_dimensions(&base, element).ok_or_else(|| {
            Error::Unsupported(format!("Could not get dimensions for node {:?}", element))
        })
    }

    /// Returns the scale factor for a native node (normalized to 1.0 per Phase 6 ledger).
    pub fn get_scale_by_node_id(&self, _element: NodeId) -> Coords {
        Coords::new(1.0)
    }

    /// Checks if a native node has right-to-left text direction.
    pub fn is_rtl_by_node_id(&self, element: NodeId) -> bool {
        let base = self.doc.borrow();
        is_node_rtl(&base, element)
    }

    /// Collects clipping ancestor `NodeId`s for a native node.
    pub fn get_overflow_ancestors_by_node_id(&self, element: NodeId) -> Vec<NodeId> {
        let base = self.doc.borrow();
        get_node_overflow_ancestors(&base, element)
    }

    /// Measures bounding client rects for reference and floating elements by ID.
    pub fn get_element_rects(&self, reference_id: &str, floating_id: &str) -> Result<ElementRects> {
        let ref_node = self
            .get_element_by_id(reference_id)
            .ok_or_else(|| Error::ElementNotFound(reference_id.to_string()))?;
        let floating_node = self
            .get_element_by_id(floating_id)
            .ok_or_else(|| Error::ElementNotFound(floating_id.to_string()))?;
        self.get_element_rects_by_node_id(ref_node, floating_node)
    }

    /// Computes the visible clipping boundary rect for an element constrained by boundary rules.
    pub fn get_clipping_rect(&self, element_id: &str, boundary: Boundary<String>) -> Result<Rect> {
        let node_id = self
            .get_element_by_id(element_id)
            .ok_or_else(|| Error::ElementNotFound(element_id.to_string()))?;
        let base = self.doc.borrow();
        get_clipping_rect_for_node_with_resolver(
            &base,
            node_id,
            boundary,
            |id| self.get_element_by_id(id),
        )
    }

    /// Returns the layout dimensions (width, height) of an element by ID.
    pub fn get_dimensions(&self, element_id: &str) -> Result<Dimensions> {
        let node_id = self
            .get_element_by_id(element_id)
            .ok_or_else(|| Error::ElementNotFound(element_id.to_string()))?;
        self.get_dimensions_by_node_id(node_id)
    }

    /// Checks if the element's computed text direction is right-to-left.
    pub fn is_rtl(&self, element_id: &str) -> bool {
        let Some(node_id) = self.get_element_by_id(element_id) else {
            return false;
        };
        self.is_rtl_by_node_id(node_id)
    }

    /// Traverses the element's ancestor tree, collecting all scrollable/clipping ancestor element IDs.
    pub fn get_overflow_ancestors(&self, element_id: &str) -> Result<Vec<String>> {
        let (_node_id, anc_nodes) = {
            let base = self.doc.borrow();
            let node_id = self
                .get_element_by_id(element_id)
                .ok_or_else(|| Error::ElementNotFound(element_id.to_string()))?;
            let anc_nodes = get_node_overflow_ancestors(&base, node_id);
            (node_id, anc_nodes)
        };

        let mut ids = Vec::new();
        for &nid in &anc_nodes {
            let explicit_id = {
                let base = self.doc.borrow();
                base.get_node(nid)
                    .and_then(|n| n.element_data())
                    .and_then(|el| el.id.as_ref())
                    .filter(|id| !id.is_empty())
                    .map(|id| id.to_string())
            };

            let id = if let Some(explicit) = explicit_id {
                explicit
            } else {
                let mut gen_ids = self.generated_ids.borrow_mut();
                if let Some(existing) = gen_ids.get(&nid) {
                    existing.clone()
                } else {
                    let gen_id = format!(
                        "ox-oa-{}",
                        GENERATED_ID_COUNTER.fetch_add(1, Ordering::Relaxed)
                    );
                    gen_ids.insert(nid, gen_id.clone());
                    self.generated_nodes.borrow_mut().insert(gen_id.clone(), nid);

                    let mut base = self.doc.borrow_mut();
                    base.mutate().set_attribute(
                        nid,
                        QualName::new(None, ns!(), local_name!("id")),
                        &gen_id,
                    );

                    gen_id
                }
            };
            ids.push(id);
        }
        Ok(ids)
    }
}

impl FloatingPlatform for NativeFloatingPlatform {
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

impl Platform<NodeId, ()> for NativeFloatingPlatform {
    fn get_element_rects(&self, args: GetElementRectsArgs<NodeId>) -> ElementRects {
        let ref_node = match args.reference {
            ElementOrVirtual::Element(nid) => nid,
            ElementOrVirtual::VirtualElement(v) => {
                let r = v.get_bounding_client_rect();
                let dim = self
                    .get_dimensions_by_node_id(*args.floating)
                    .unwrap_or_default();
                return ElementRects {
                    reference: r.into(),
                    floating: Rect::new(0.0, 0.0, dim.width, dim.height),
                };
            }
        };
        self.get_element_rects_by_node_id(*ref_node, *args.floating)
            .unwrap_or_else(|_| ElementRects {
                reference: Rect::default(),
                floating: Rect::default(),
            })
    }

    fn get_clipping_rect(&self, args: GetClippingRectArgs<NodeId>) -> Rect {
        let boundary = match args.boundary {
            Boundary::ClippingAncestors => Boundary::ClippingAncestors,
            Boundary::RootViewport => Boundary::RootViewport,
            Boundary::Document => Boundary::Document,
            Boundary::Custom(r) => Boundary::Custom(r),
            Boundary::Element(nid) => {
                let base = self.doc.borrow();
                let id = base
                    .get_node(nid)
                    .and_then(|n| n.element_data())
                    .and_then(|e| e.id.as_ref().map(|id| id.to_string()))
                    .unwrap_or_default();
                Boundary::Element(id)
            }
            Boundary::Elements(nids) => {
                let base = self.doc.borrow();
                let ids = nids
                    .into_iter()
                    .filter_map(|nid| {
                        base.get_node(nid)
                            .and_then(|n| n.element_data())
                            .and_then(|e| e.id.as_ref().map(|id| id.to_string()))
                    })
                    .collect();
                Boundary::Elements(ids)
            }
        };
        let base = self.doc.borrow();
        get_clipping_rect_for_node(&base, *args.element, boundary).unwrap_or_default()
    }

    fn get_dimensions(&self, element: &NodeId) -> Dimensions {
        self.get_dimensions_by_node_id(*element).unwrap_or_default()
    }

    fn is_rtl(&self, element: &NodeId) -> Option<bool> {
        Some(self.is_rtl_by_node_id(*element))
    }

    fn get_scale(&self, element: &NodeId) -> Option<Coords> {
        Some(self.get_scale_by_node_id(*element))
    }

    fn detect_overflow(
        &self,
        state: MiddlewareState<NodeId, ()>,
        options: DetectOverflowOptions<NodeId>,
    ) -> SideObject {
        crate::detect_overflow::detect_overflow(state, options)
    }
}

impl Platform<String, ()> for NativeFloatingPlatform {
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

#[cfg(test)]
mod tests {
    use super::*;
    use blitz_dom::DocumentConfig;

    #[test]
    fn test_native_platform_missing_elements() {
        let doc = BaseDocument::new(DocumentConfig::default());
        let platform = NativeFloatingPlatform::from_base(doc);

        assert!(matches!(
            platform.get_dimensions("nonexistent"),
            Err(Error::ElementNotFound(_))
        ));
        assert!(matches!(
            platform.get_element_rects("ref", "float"),
            Err(Error::ElementNotFound(_))
        ));
        assert!(!platform.is_rtl("nonexistent"));
        assert!(matches!(
            platform.get_overflow_ancestors("nonexistent"),
            Err(Error::ElementNotFound(_))
        ));
        assert!(matches!(
            platform.get_clipping_rect("nonexistent", Boundary::RootViewport),
            Err(Error::ElementNotFound(_))
        ));
    }

    #[test]
    fn test_native_platform_viewport_geometry() {
        let doc = BaseDocument::new(DocumentConfig::default());
        let platform = NativeFloatingPlatform::from_base(doc);

        let vp = get_root_viewport_rect(&platform.document().borrow());
        assert_eq!(vp.x, 0.0);
        assert_eq!(vp.y, 0.0);
        assert!(vp.width >= 0.0);
        assert!(vp.height >= 0.0);
    }

    #[test]
    fn test_native_platform_scale_and_rtl() {
        let doc = BaseDocument::new(DocumentConfig::default());
        let root_id = doc.root_node().id;
        let platform = NativeFloatingPlatform::from_base(doc);

        // Scale is normalized to 1.0 per Phase 6 ledger
        assert_eq!(platform.get_scale_by_node_id(root_id), Coords::new(1.0));
        assert!(!platform.is_rtl_by_node_id(root_id));
    }

    #[test]
    fn test_native_platform_generated_ancestor_ids_and_resolution() {
        use blitz_dom::{Attribute, QualName, local_name, ns};

        let mut doc = BaseDocument::new(DocumentConfig::default());
        let root_id = doc.root_node().id;

        // Container without ID, styled with overflow: scroll
        let container_id = {
            let mut mutator = doc.mutate();
            let cid = mutator.create_element(
                QualName::new(None, ns!(), local_name!("div")),
                vec![Attribute {
                    name: QualName::new(None, ns!(), local_name!("style")),
                    value: "width: 300px; height: 300px; overflow: scroll; position: relative;".into(),
                }],
            );
            mutator.append_children(root_id, &[cid]);
            cid
        };

        // Target with ID inside container
        let _target_id = {
            let mut mutator = doc.mutate();
            let tid = mutator.create_element(
                QualName::new(None, ns!(), local_name!("button")),
                vec![
                    Attribute {
                        name: QualName::new(None, ns!(), local_name!("id")),
                        value: "my-target".into(),
                    },
                    Attribute {
                        name: QualName::new(None, ns!(), local_name!("style")),
                        value: "width: 100px; height: 30px;".into(),
                    },
                ],
            );
            mutator.append_children(container_id, &[tid]);
            tid
        };

        // Resolve style & layout
        doc.resolve(0.0);

        let platform = NativeFloatingPlatform::from_base(doc);

        let ancestors = platform
            .get_overflow_ancestors("my-target")
            .expect("must resolve overflow ancestors");
        assert_eq!(ancestors.len(), 1, "Ancestor with overflow: scroll must be detected");
        let gen_id = &ancestors[0];
        assert!(gen_id.starts_with("ox-oa-"), "Generated ID must follow ox-oa-N pattern: {}", gen_id);

        // Verification of seamless resolution:
        // 1. get_element_by_id
        assert_eq!(platform.get_element_by_id(gen_id), Some(container_id));

        // 2. get_dimensions
        let dim = platform.get_dimensions(gen_id).expect("must resolve dimensions for generated ID");
        assert!(dim.width >= 0.0);
        assert!(dim.height >= 0.0);

        // 3. get_clipping_rect
        let clip = platform
            .get_clipping_rect("my-target", Boundary::Element(gen_id.clone()))
            .expect("must resolve clipping rect with generated boundary ID");
        assert!(clip.width >= 0.0);

        // 4. Stable ID parity: calling get_overflow_ancestors again returns the exact same ID
        let ancestors_again = platform
            .get_overflow_ancestors("my-target")
            .expect("must resolve overflow ancestors again");
        assert_eq!(ancestors_again[0], *gen_id, "Generated ID must be stable across queries");
    }
}
