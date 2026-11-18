//! Modal external DOM subtree shielding (`aria-hidden="true"` and `inert=""`).
//!
//! Suppresses background sibling subtrees outside active modal overlays across Web (`wasm32`)
//! and Blitz Native (`feature = "native"`), preserving `[aria-live]` regions (such as toast
//! viewports), skipping non-rendered metadata tags, and reference-counting stacked modals
//! without clobbering pre-existing consumer `aria-hidden` or `inert` attributes.

use std::cell::RefCell;

#[cfg_attr(
    all(not(target_arch = "wasm32"), not(feature = "native")),
    allow(dead_code)
)]
const SKIPPED_ELEMENTS: [&str; 6] = ["link", "meta", "noscript", "script", "style", "template"];

#[inline]
#[cfg_attr(
    all(not(target_arch = "wasm32"), not(feature = "native")),
    allow(dead_code)
)]
fn is_skipped_tag(local_name: &str) -> bool {
    SKIPPED_ELEMENTS
        .iter()
        .any(|skipped| local_name.eq_ignore_ascii_case(skipped))
}

/// Attribute application policy for [`shield_others_with_mode`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ModalShieldMode {
    /// Applies both `aria-hidden="true"` and `inert=""` (recommended modal default).
    #[default]
    Suppress,
    /// Applies `aria-hidden="true"` only.
    AriaHiddenOnly,
    /// Applies `inert=""` only.
    InertOnly,
}

impl ModalShieldMode {
    #[inline]
    pub const fn controls_aria_hidden(self) -> bool {
        matches!(self, Self::Suppress | Self::AriaHiddenOnly)
    }

    #[inline]
    pub const fn controls_inert(self) -> bool {
        matches!(self, Self::Suppress | Self::InertOnly)
    }
}

/// RAII undo token restoring shielded external DOM subtrees on [`ModalShieldGuard::release`] or drop.
pub struct ModalShieldGuard {
    cleanup: RefCell<Option<Box<dyn FnOnce()>>>,
}

impl ModalShieldGuard {
    /// Creates a no-op guard that performs no DOM mutations on release or drop.
    pub fn noop() -> Self {
        Self {
            cleanup: RefCell::new(None),
        }
    }

    #[cfg_attr(
        all(not(target_arch = "wasm32"), not(feature = "native")),
        allow(dead_code)
    )]
    pub(crate) fn with_cleanup(cleanup: impl FnOnce() + 'static) -> Self {
        Self {
            cleanup: RefCell::new(Some(Box::new(cleanup))),
        }
    }

    /// Explicitly restores shielded DOM attributes immediately. Idempotent if called before drop.
    pub fn release(&self) {
        let cb = self.cleanup.try_borrow_mut().ok().and_then(|mut opt| opt.take());
        if let Some(cleanup) = cb {
            cleanup();
        }
    }

    /// Returns `true` if this guard has already released its shield (or was created as `noop`).
    pub fn is_released(&self) -> bool {
        self.cleanup
            .try_borrow()
            .map(|opt| opt.is_none())
            .unwrap_or(true)
    }
}

impl Drop for ModalShieldGuard {
    fn drop(&mut self) {
        self.release();
    }
}

impl Default for ModalShieldGuard {
    fn default() -> Self {
        Self::noop()
    }
}

impl std::fmt::Debug for ModalShieldGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModalShieldGuard")
            .field("released", &self.is_released())
            .finish()
    }
}

/// Suppresses all DOM subtrees outside `keep_ids` (and `[aria-live]` regions) with both
/// `aria-hidden="true"` and `inert=""`.
pub fn suppress_others(keep_ids: &[&str]) -> ModalShieldGuard {
    shield_others_with_mode(keep_ids, ModalShieldMode::Suppress)
}

/// Marks all DOM subtrees outside `keep_ids` (and `[aria-live]` regions) with `aria-hidden="true"` only.
pub fn hide_others(keep_ids: &[&str]) -> ModalShieldGuard {
    shield_others_with_mode(keep_ids, ModalShieldMode::AriaHiddenOnly)
}

/// Marks all DOM subtrees outside `keep_ids` (and `[aria-live]` regions) with `inert=""` only.
pub fn inert_others(keep_ids: &[&str]) -> ModalShieldGuard {
    shield_others_with_mode(keep_ids, ModalShieldMode::InertOnly)
}

/// Shields all DOM subtrees outside `keep_ids` (and `[aria-live]` regions) according to `mode`.
pub fn shield_others_with_mode(keep_ids: &[&str], mode: ModalShieldMode) -> ModalShieldGuard {
    #[cfg(target_arch = "wasm32")]
    {
        return wasm_impl::shield_others_web(keep_ids, mode);
    }
    #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
    {
        return native_impl::shield_others_native(keep_ids, mode);
    }
    #[cfg(all(not(target_arch = "wasm32"), not(feature = "native")))]
    {
        let _ = (keep_ids, mode);
        ModalShieldGuard::noop()
    }
}

#[cfg(any(
    target_arch = "wasm32",
    all(not(target_arch = "wasm32"), feature = "native")
))]
#[derive(Default)]
struct ShieldNodeCounter {
    aria_hidden_count: usize,
    uncontrolled_aria_hidden: bool,
    inert_count: usize,
    uncontrolled_inert: bool,
}

#[cfg(target_arch = "wasm32")]
mod wasm_impl {
    use super::{
        ModalShieldGuard, ModalShieldMode, ShieldNodeCounter, is_skipped_tag,
    };
    use std::cell::RefCell;
    use std::collections::{HashMap, HashSet};
    use wasm_bindgen::JsCast;
    use wasm_bindgen::JsValue;

    struct WebModalShieldState {
        next_key: u64,
        element_to_key: js_sys::Map,
        key_to_element: HashMap<u64, web_sys::Element>,
        counters: HashMap<u64, ShieldNodeCounter>,
    }

    impl Default for WebModalShieldState {
        fn default() -> Self {
            Self {
                next_key: 1,
                element_to_key: js_sys::Map::new(),
                key_to_element: HashMap::new(),
                counters: HashMap::new(),
            }
        }
    }

    impl WebModalShieldState {
        fn get_or_assign_key(&mut self, el: &web_sys::Element) -> u64 {
            let js_ref: &JsValue = el.as_ref();
            let existing = self.element_to_key.get(js_ref);
            if let Some(num) = existing.as_f64() {
                return num as u64;
            }
            let key = self.next_key;
            self.next_key = self.next_key.wrapping_add(1);
            self.element_to_key.set(js_ref, &JsValue::from_f64(key as f64));
            self.key_to_element.insert(key, el.clone());
            key
        }

        fn remove_key_if_unused(&mut self, key: u64) {
            let empty = self
                .counters
                .get(&key)
                .map(|c| c.aria_hidden_count == 0 && c.inert_count == 0)
                .unwrap_or(true);
            if empty {
                self.counters.remove(&key);
                if let Some(el) = self.key_to_element.remove(&key) {
                    let js_ref: &JsValue = el.as_ref();
                    self.element_to_key.delete(js_ref);
                }
            }
        }
    }

    std::thread_local! {
        static MODAL_SHIELD_STATE: RefCell<WebModalShieldState> = RefCell::new(WebModalShieldState::default());
    }

    pub(super) fn shield_others_web(keep_ids: &[&str], mode: ModalShieldMode) -> ModalShieldGuard {
        let Some(win) = web_sys::window() else {
            return ModalShieldGuard::noop();
        };
        let Some(doc) = win.document() else {
            return ModalShieldGuard::noop();
        };
        let Some(body_html) = doc.body() else {
            return ModalShieldGuard::noop();
        };
        let body: web_sys::Element = body_html.into();

        let mut target_elements: Vec<web_sys::Element> = Vec::new();
        for &id in keep_ids {
            if id.is_empty() {
                continue;
            }
            if let Some(el) = doc.get_element_by_id(id)
                && body.contains(Some(&el))
                && el != body
                && !target_elements.iter().any(|existing| existing == &el)
            {
                target_elements.push(el);
            }
        }

        if target_elements.is_empty() {
            return ModalShieldGuard::noop();
        }

        if let Ok(live_nodes) = body.query_selector_all("[aria-live]") {
            for idx in 0..live_nodes.length() {
                if let Some(node) = live_nodes.item(idx)
                    && let Ok(el) = node.dyn_into::<web_sys::Element>()
                    && body.contains(Some(&el))
                    && el != body
                    && !target_elements.iter().any(|existing| existing == &el)
                {
                    target_elements.push(el);
                }
            }
        }

        let elements_to_keep_map = js_sys::Map::new();
        let elements_to_stop_map = js_sys::Map::new();
        let js_true = JsValue::from_bool(true);

        for target in &target_elements {
            elements_to_stop_map.set(target.as_ref(), &js_true);
            let mut cur_opt: Option<web_sys::Element> = Some(target.clone());
            while let Some(cur) = cur_opt {
                if elements_to_keep_map.has(cur.as_ref()) {
                    break;
                }
                elements_to_keep_map.set(cur.as_ref(), &js_true);
                if cur == body {
                    break;
                }
                cur_opt = cur.parent_element();
            }
        }

        let mut shielded_keys: Vec<u64> = Vec::new();
        let mut seen_keys: HashSet<u64> = HashSet::new();

        fn walk(
            parent: &web_sys::Element,
            elements_to_keep_map: &js_sys::Map,
            elements_to_stop_map: &js_sys::Map,
            mode: ModalShieldMode,
            shielded_keys: &mut Vec<u64>,
            seen_keys: &mut HashSet<u64>,
        ) {
            if elements_to_stop_map.has(parent.as_ref()) {
                return;
            }
            let children = parent.children();
            for idx in 0..children.length() {
                let Some(child) = children.item(idx) else {
                    continue;
                };
                if is_skipped_tag(&child.tag_name()) {
                    continue;
                }
                if elements_to_keep_map.has(child.as_ref()) {
                    walk(
                        &child,
                        elements_to_keep_map,
                        elements_to_stop_map,
                        mode,
                        shielded_keys,
                        seen_keys,
                    );
                } else {
                    MODAL_SHIELD_STATE.with(|cell| {
                        let mut state = cell.borrow_mut();
                        let key = state.get_or_assign_key(&child);
                        if !seen_keys.insert(key) {
                            return;
                        }
                        let counter = state.counters.entry(key).or_default();

                        if mode.controls_aria_hidden() {
                            if counter.aria_hidden_count == 0 {
                                let attr = child.get_attribute("aria-hidden");
                                let already_hidden =
                                    matches!(attr.as_deref(), Some(v) if v != "false");
                                counter.uncontrolled_aria_hidden = already_hidden;
                                if !already_hidden {
                                    let _ = child.set_attribute("aria-hidden", "true");
                                }
                                let _ = child.set_attribute("data-oxidase-aria-hidden", "true");
                            }
                            counter.aria_hidden_count += 1;
                        }

                        if mode.controls_inert() {
                            if counter.inert_count == 0 {
                                let already_inert = child.has_attribute("inert");
                                counter.uncontrolled_inert = already_inert;
                                if !already_inert {
                                    let _ = child.set_attribute("inert", "");
                                }
                                let _ = child.set_attribute("data-oxidase-inert", "true");
                            }
                            counter.inert_count += 1;
                        }

                        shielded_keys.push(key);
                    });
                }
            }
        }

        walk(
            &body,
            &elements_to_keep_map,
            &elements_to_stop_map,
            mode,
            &mut shielded_keys,
            &mut seen_keys,
        );

        if shielded_keys.is_empty() {
            return ModalShieldGuard::noop();
        }

        ModalShieldGuard::with_cleanup(move || {
            MODAL_SHIELD_STATE.with(|cell| {
                let mut state = cell.borrow_mut();
                for key in shielded_keys {
                    let el_opt = state.key_to_element.get(&key).cloned();
                    if let Some(counter) = state.counters.get_mut(&key) {
                        if mode.controls_aria_hidden() && counter.aria_hidden_count > 0 {
                            counter.aria_hidden_count -= 1;
                            if counter.aria_hidden_count == 0 {
                                if let Some(ref el) = el_opt {
                                    if !counter.uncontrolled_aria_hidden {
                                        let _ = el.remove_attribute("aria-hidden");
                                    }
                                    let _ = el.remove_attribute("data-oxidase-aria-hidden");
                                }
                                counter.uncontrolled_aria_hidden = false;
                            }
                        }

                        if mode.controls_inert() && counter.inert_count > 0 {
                            counter.inert_count -= 1;
                            if counter.inert_count == 0 {
                                if let Some(ref el) = el_opt {
                                    if !counter.uncontrolled_inert {
                                        let _ = el.remove_attribute("inert");
                                    }
                                    let _ = el.remove_attribute("data-oxidase-inert");
                                }
                                counter.uncontrolled_inert = false;
                            }
                        }
                    }
                    state.remove_key_if_unused(key);
                }
            });
        })
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
mod native_impl {
    use super::{
        ModalShieldGuard, ModalShieldMode, ShieldNodeCounter, is_skipped_tag,
    };
    use blitz_dom::{BaseDocument, LocalName, NodeId, QualName, ns};
    use std::cell::RefCell;
    use std::collections::{HashMap, HashSet};
    use std::rc::{Rc, Weak};

    type NativeNodeKey = (usize, NodeId);

    struct NativeShieldEntry {
        doc: Weak<RefCell<BaseDocument>>,
        counter: ShieldNodeCounter,
    }

    std::thread_local! {
        static MODAL_SHIELD_STATE: RefCell<HashMap<NativeNodeKey, NativeShieldEntry>> =
            RefCell::new(HashMap::new());
    }

    fn lookup_node_id_in_base(base: &BaseDocument, id: &str) -> Option<NodeId> {
        base.get_element_by_id(id)
            .or_else(|| base.query_selector(&format!("#{id}")).ok().flatten())
            .or_else(|| {
                let mut stack = vec![base.root_node().id];
                while let Some(nid) = stack.pop() {
                    if let Some(node) = base.get_node(nid) {
                        if let Some(el) = node.element_data()
                            && el.id.as_deref() == Some(id)
                        {
                            return Some(nid);
                        }
                        if node.attr(blitz_dom::local_name!("id")) == Some(id) {
                            return Some(nid);
                        }
                        stack.extend(node.children.iter().rev().copied());
                    }
                }
                None
            })
    }

    fn resolve_shield_root_id(base: &BaseDocument) -> NodeId {
        if let Some(body) = base.find_body_node() {
            return body.id;
        }
        if let Some(root_el) = base.try_root_element() {
            return root_el.id;
        }
        base.root_node().id
    }

    fn is_strict_descendant(base: &BaseDocument, ancestor_id: NodeId, candidate_id: NodeId) -> bool {
        if candidate_id == ancestor_id {
            return false;
        }
        let mut cur = base.get_node(candidate_id).and_then(|n| n.parent);
        while let Some(pid) = cur {
            if pid == ancestor_id {
                return true;
            }
            cur = base.get_node(pid).and_then(|n| n.parent);
        }
        false
    }

    fn collect_aria_live_descendants(base: &BaseDocument, root_id: NodeId, out: &mut Vec<NodeId>) {
        let mut stack: Vec<NodeId> = Vec::new();
        if let Some(root_node) = base.get_node(root_id) {
            stack.extend(root_node.children.iter().rev().copied());
        }
        while let Some(nid) = stack.pop() {
            let Some(node) = base.get_node(nid) else {
                continue;
            };
            if let Some(el) = node.element_data()
                && el
                    .attrs
                    .iter()
                    .any(|a| (&*a.name.local).eq_ignore_ascii_case("aria-live"))
                && !out.contains(&nid)
            {
                out.push(nid);
            }
            stack.extend(node.children.iter().rev().copied());
        }
    }

    fn get_empty_ns_attr(base: &BaseDocument, nid: NodeId, name: &str) -> Option<String> {
        let node = base.get_node(nid)?;
        let elem = node.element_data()?;
        elem.attrs
            .iter()
            .find(|a| (a.name.ns == ns!() || a.name.ns.is_empty()) && &*a.name.local == name)
            .map(|a| a.value.to_string())
    }

    fn has_empty_ns_attr(base: &BaseDocument, nid: NodeId, name: &str) -> bool {
        get_empty_ns_attr(base, nid, name).is_some()
    }

    fn apply_attr_mutations(
        base_rc: &Rc<RefCell<BaseDocument>>,
        set_ops: &[(NodeId, &'static str, &'static str)],
        remove_ops: &[(NodeId, &'static str)],
    ) {
        if set_ops.is_empty() && remove_ops.is_empty() {
            return;
        }
        let apply = |base: &mut BaseDocument| {
            let mut mutator = base.mutate();
            for &(nid, name, val) in set_ops {
                if mutator.doc.get_node(nid).is_some() {
                    let qname = QualName::new(None, ns!(), LocalName::from(name));
                    mutator.set_attribute(nid, qname, val);
                }
            }
            for &(nid, name) in remove_ops {
                if mutator.doc.get_node(nid).is_some() {
                    let qname = QualName::new(None, ns!(), LocalName::from(name));
                    mutator.clear_attribute(nid, qname);
                }
            }
        };

        if let Ok(mut base) = base_rc.try_borrow_mut() {
            apply(&mut base);
        } else {
            let doc_clone = base_rc.clone();
            let set_owned = set_ops.to_vec();
            let remove_owned = remove_ops.to_vec();
            let _ = crate::internal::safe_spawn(async move {
                if let Ok(mut base) = doc_clone.try_borrow_mut() {
                    let mut mutator = base.mutate();
                    for (nid, name, val) in set_owned {
                        if mutator.doc.get_node(nid).is_some() {
                            let qname = QualName::new(None, ns!(), LocalName::from(name));
                            mutator.set_attribute(nid, qname, val);
                        }
                    }
                    for (nid, name) in remove_owned {
                        if mutator.doc.get_node(nid).is_some() {
                            let qname = QualName::new(None, ns!(), LocalName::from(name));
                            mutator.clear_attribute(nid, qname);
                        }
                    }
                }
            });
        }
    }

    pub(super) fn shield_others_native(
        keep_ids: &[&str],
        mode: ModalShieldMode,
    ) -> ModalShieldGuard {
        let Some(doc) = crate::dom::Document::current() else {
            return ModalShieldGuard::noop();
        };
        let Some(base_rc) = doc.get_base() else {
            return ModalShieldGuard::noop();
        };
        let doc_ptr = Rc::as_ptr(&base_rc) as usize;

        let (shielded_nodes, set_ops) = {
            let Ok(base) = base_rc.try_borrow() else {
                return ModalShieldGuard::noop();
            };

            let body_id = resolve_shield_root_id(&base);

            let mut targets: Vec<NodeId> = Vec::new();
            for &id in keep_ids {
                if id.is_empty() {
                    continue;
                }
                if let Some(nid) = lookup_node_id_in_base(&base, id)
                    && is_strict_descendant(&base, body_id, nid)
                    && !targets.contains(&nid)
                {
                    targets.push(nid);
                }
            }

            if targets.is_empty() {
                return ModalShieldGuard::noop();
            }

            collect_aria_live_descendants(&base, body_id, &mut targets);

            let elements_to_stop: HashSet<NodeId> = targets.iter().copied().collect();
            let mut elements_to_keep: HashSet<NodeId> = HashSet::new();

            for &target_id in &targets {
                let mut cur_opt = Some(target_id);
                while let Some(cur) = cur_opt {
                    if !elements_to_keep.insert(cur) {
                        break;
                    }
                    if cur == body_id {
                        break;
                    }
                    cur_opt = base.get_node(cur).and_then(|n| n.parent);
                }
            }

            let mut candidate_nodes: Vec<NodeId> = Vec::new();
            let mut seen: HashSet<NodeId> = HashSet::new();

            fn walk_native(
                base: &BaseDocument,
                parent_id: NodeId,
                elements_to_keep: &HashSet<NodeId>,
                elements_to_stop: &HashSet<NodeId>,
                candidates: &mut Vec<NodeId>,
                seen: &mut HashSet<NodeId>,
            ) {
                if elements_to_stop.contains(&parent_id) {
                    return;
                }
                let Some(parent_node) = base.get_node(parent_id) else {
                    return;
                };
                for &child_id in &parent_node.children {
                    let Some(child_node) = base.get_node(child_id) else {
                        continue;
                    };
                    let Some(elem) = child_node.element_data() else {
                        continue;
                    };
                    if is_skipped_tag(&elem.name.local) {
                        continue;
                    }
                    if elements_to_keep.contains(&child_id) {
                        walk_native(
                            base,
                            child_id,
                            elements_to_keep,
                            elements_to_stop,
                            candidates,
                            seen,
                        );
                    } else if seen.insert(child_id) {
                        candidates.push(child_id);
                    }
                }
            }

            walk_native(
                &base,
                body_id,
                &elements_to_keep,
                &elements_to_stop,
                &mut candidate_nodes,
                &mut seen,
            );

            if candidate_nodes.is_empty() {
                return ModalShieldGuard::noop();
            }

            let mut set_ops: Vec<(NodeId, &'static str, &'static str)> = Vec::new();

            MODAL_SHIELD_STATE.with(|cell| {
                let mut state = cell.borrow_mut();
                state.retain(|_, entry| entry.doc.strong_count() > 0);

                for &nid in &candidate_nodes {
                    let key = (doc_ptr, nid);
                    let entry = state.entry(key).or_insert_with(|| NativeShieldEntry {
                        doc: Rc::downgrade(&base_rc),
                        counter: ShieldNodeCounter::default(),
                    });

                    if mode.controls_aria_hidden() {
                        if entry.counter.aria_hidden_count == 0 {
                            let attr = get_empty_ns_attr(&base, nid, "aria-hidden");
                            let already_hidden =
                                matches!(attr.as_deref(), Some(v) if v != "false");
                            entry.counter.uncontrolled_aria_hidden = already_hidden;
                            if !already_hidden {
                                set_ops.push((nid, "aria-hidden", "true"));
                            }
                            set_ops.push((nid, "data-oxidase-aria-hidden", "true"));
                        }
                        entry.counter.aria_hidden_count += 1;
                    }

                    if mode.controls_inert() {
                        if entry.counter.inert_count == 0 {
                            let already_inert = has_empty_ns_attr(&base, nid, "inert");
                            entry.counter.uncontrolled_inert = already_inert;
                            if !already_inert {
                                set_ops.push((nid, "inert", ""));
                            }
                            set_ops.push((nid, "data-oxidase-inert", "true"));
                        }
                        entry.counter.inert_count += 1;
                    }
                }
            });

            (candidate_nodes, set_ops)
        };

        apply_attr_mutations(&base_rc, &set_ops, &[]);

        let weak_doc = Rc::downgrade(&base_rc);
        ModalShieldGuard::with_cleanup(move || {
            let mut remove_ops: Vec<(NodeId, &'static str)> = Vec::new();
            MODAL_SHIELD_STATE.with(|cell| {
                let mut state = cell.borrow_mut();
                for &nid in &shielded_nodes {
                    let key = (doc_ptr, nid);
                    let mut should_remove_entry = false;
                    if let Some(entry) = state.get_mut(&key) {
                        if mode.controls_aria_hidden() && entry.counter.aria_hidden_count > 0 {
                            entry.counter.aria_hidden_count -= 1;
                            if entry.counter.aria_hidden_count == 0 {
                                if !entry.counter.uncontrolled_aria_hidden {
                                    remove_ops.push((nid, "aria-hidden"));
                                }
                                remove_ops.push((nid, "data-oxidase-aria-hidden"));
                                entry.counter.uncontrolled_aria_hidden = false;
                            }
                        }

                        if mode.controls_inert() && entry.counter.inert_count > 0 {
                            entry.counter.inert_count -= 1;
                            if entry.counter.inert_count == 0 {
                                if !entry.counter.uncontrolled_inert {
                                    remove_ops.push((nid, "inert"));
                                }
                                remove_ops.push((nid, "data-oxidase-inert"));
                                entry.counter.uncontrolled_inert = false;
                            }
                        }

                        should_remove_entry = entry.counter.aria_hidden_count == 0
                            && entry.counter.inert_count == 0;
                    }
                    if should_remove_entry {
                        state.remove(&key);
                    }
                }
            });

            if let Some(base_rc) = weak_doc.upgrade() {
                apply_attr_mutations(&base_rc, &[], &remove_ops);
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modal_shield_mode_flags_and_noop_guard() {
        assert_eq!(ModalShieldMode::default(), ModalShieldMode::Suppress);
        assert!(ModalShieldMode::Suppress.controls_aria_hidden());
        assert!(ModalShieldMode::Suppress.controls_inert());
        assert!(ModalShieldMode::AriaHiddenOnly.controls_aria_hidden());
        assert!(!ModalShieldMode::AriaHiddenOnly.controls_inert());
        assert!(!ModalShieldMode::InertOnly.controls_aria_hidden());
        assert!(ModalShieldMode::InertOnly.controls_inert());

        assert!(is_skipped_tag("script"));
        assert!(is_skipped_tag("STYLE"));
        assert!(is_skipped_tag("template"));
        assert!(is_skipped_tag("noscript"));
        assert!(is_skipped_tag("link"));
        assert!(is_skipped_tag("meta"));
        assert!(!is_skipped_tag("div"));

        let guard = suppress_others(&["missing-modal"]);
        assert!(guard.is_released());
        guard.release();
        assert!(format!("{guard:?}").contains("ModalShieldGuard"));
    }
}
