//! Cross-host document body scroll-lock coordination with non-destructive CSSOM snapshot/restore,
//! reference-counted lock IDs, configurable `pointer-events` blocking, and optional delayed restoration.

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    time::Duration,
};

const MANAGED_PROPERTIES: [&str; 7] = [
    "padding-right",
    "margin-right",
    "overflow",
    "overflow-x",
    "overflow-y",
    "pointer-events",
    "--scrollbar-width",
];

/// Options controlling document body scroll-lock behavior for an individual lock acquisition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScrollLockOptions {
    /// When `true` (default), sets `pointer-events: none` on `<body>` while this lock is active,
    /// restoring `pointer-events` synchronously as soon as the last pointer-blocking lock releases.
    pub prevent_body_pointer_events: bool,
}

impl Default for ScrollLockOptions {
    fn default() -> Self {
        Self {
            prevent_body_pointer_events: true,
        }
    }
}

#[cfg_attr(
    all(not(target_arch = "wasm32"), not(feature = "native")),
    allow(dead_code)
)]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct StylePropertySnapshot {
    value: String,
    priority: String,
}

#[cfg_attr(
    all(not(target_arch = "wasm32"), not(feature = "native")),
    allow(dead_code)
)]
struct ScrollLockState {
    locks: HashMap<String, ScrollLockOptions>,
    is_locked: bool,
    is_pointer_events_locked: bool,
    had_initial_style_attribute: bool,
    initial_properties: Option<HashMap<&'static str, StylePropertySnapshot>>,
    modified_properties: HashSet<&'static str>,
    cleanup_generation: u64,
    #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
    locked_native_body: Option<(
        std::rc::Weak<RefCell<blitz_dom::BaseDocument>>,
        blitz_dom::NodeId,
    )>,
    #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
    pending_restore_guard: Option<crate::frame::FrameLoopGuard>,
}

impl Default for ScrollLockState {
    fn default() -> Self {
        Self {
            locks: HashMap::new(),
            is_locked: false,
            is_pointer_events_locked: false,
            had_initial_style_attribute: false,
            initial_properties: None,
            modified_properties: HashSet::new(),
            cleanup_generation: 0,
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            locked_native_body: None,
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            pending_restore_guard: None,
        }
    }
}

impl ScrollLockState {
    fn any_pointer_blocking(&self) -> bool {
        self.locks
            .values()
            .any(|opts| opts.prevent_body_pointer_events)
    }
}

thread_local! {
    static STATE: RefCell<ScrollLockState> = RefCell::new(ScrollLockState::default());
}

/// Returns whether a document scroll lock is currently active on this thread (including pending `restore_delay`).
pub fn is_scroll_locked() -> bool {
    STATE.with(|state| {
        let state = state.borrow();
        state.is_locked || !state.locks.is_empty()
    })
}

/// Returns whether `<body>` `pointer-events` is currently locked (`pointer-events: none`) by any active scroll lock.
pub fn is_body_pointer_events_locked() -> bool {
    STATE.with(|state| {
        let state = state.borrow();
        state.is_pointer_events_locked || state.any_pointer_blocking()
    })
}

/// Acquires a named scroll lock with [`ScrollLockOptions::default`] (`prevent_body_pointer_events: true`).
pub fn acquire_scroll_lock(lock_id: &str) {
    acquire_scroll_lock_with_options(lock_id, ScrollLockOptions::default());
}

/// Acquires (or updates) a named scroll lock on the active host document with explicit [`ScrollLockOptions`].
pub fn acquire_scroll_lock_with_options(lock_id: &str, options: ScrollLockOptions) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.cleanup_generation = state.cleanup_generation.wrapping_add(1);
        #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
        {
            state.pending_restore_guard = None;
        }
        state.locks.insert(lock_id.to_string(), options);
        let should_block_pointer = state.any_pointer_blocking();

        #[cfg(target_arch = "wasm32")]
        wasm_host::apply_lock(&mut state, should_block_pointer);

        #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
        native_host::apply_lock(&mut state, should_block_pointer);

        state.is_locked = true;
        state.is_pointer_events_locked = should_block_pointer;
    });
}

/// Releases a named scroll lock.
///
/// - `pointer-events` on `<body>` is restored **immediately and synchronously** as soon as the last
///   pointer-blocking lock is released (before `restore_delay`).
/// - `overflow` and scrollbar compensation are restored immediately when `restore_delay` is `None` / `Some(0)`
///   (or outside an active Dioxus runtime), or after `restore_delay` milliseconds when scheduled inside an
///   active Dioxus runtime (guarded by `cleanup_generation`).
pub fn release_scroll_lock(lock_id: &str, restore_delay: Option<u64>) {
    let (should_restore_body, token) = STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.locks.remove(lock_id);

        let still_blocking_pointer = state.any_pointer_blocking();
        if !still_blocking_pointer && state.is_pointer_events_locked {
            state.is_pointer_events_locked = false;

            #[cfg(target_arch = "wasm32")]
            wasm_host::restore_pointer_events(&mut state);

            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            native_host::restore_pointer_events(&mut state);
        }

        if !state.locks.is_empty() || !state.is_locked {
            return (false, 0);
        }
        state.cleanup_generation = state.cleanup_generation.wrapping_add(1);
        (true, state.cleanup_generation)
    });

    if !should_restore_body {
        return;
    }

    let do_restore = std::rc::Rc::new(move || {
        STATE.with(|state| {
            let mut state = state.borrow_mut();
            if state.cleanup_generation != token || !state.locks.is_empty() || !state.is_locked {
                return;
            }
            state.is_locked = false;
            state.is_pointer_events_locked = false;
            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            {
                state.pending_restore_guard = None;
            }

            #[cfg(target_arch = "wasm32")]
            wasm_host::reset_body_style(&mut state);

            #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
            native_host::reset_body_style(&mut state);

            #[cfg(all(not(target_arch = "wasm32"), not(feature = "native")))]
            {
                state.initial_properties = None;
                state.modified_properties.clear();
                state.had_initial_style_attribute = false;
            }
        });
    });

    let delay_ms = restore_delay.unwrap_or(0);
    if delay_ms > 0 && dioxus::core::Runtime::try_current().is_some() {
        #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
        let mut scheduled_any = false;
        #[cfg(not(all(not(target_arch = "wasm32"), feature = "native")))]
        let scheduled_any = false;

        #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
        {
            let target_delay = Duration::from_millis(delay_ms);
            let mut elapsed = Duration::ZERO;
            let mut fired = false;
            let do_restore_frame = std::rc::Rc::clone(&do_restore);
            if let Ok(guard) = crate::frame::start_frame_loop(move |info| {
                if fired {
                    return;
                }
                elapsed = elapsed.saturating_add(info.delta);
                if elapsed >= target_delay {
                    fired = true;
                    do_restore_frame();
                }
            }) {
                STATE.with(|s| {
                    if let Ok(mut st) = s.try_borrow_mut() {
                        if st.cleanup_generation == token && st.is_locked {
                            st.pending_restore_guard = Some(guard);
                        }
                    }
                });
                scheduled_any = true;
            }
        }

        let delay = futures_timer::Delay::new(Duration::from_millis(delay_ms));
        let do_restore_async = std::rc::Rc::clone(&do_restore);
        let spawned = crate::internal::safe_spawn(async move {
            delay.await;
            do_restore_async();
        });
        if spawned.is_none() && !scheduled_any {
            do_restore();
        }
    } else {
        do_restore();
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm_host {
    use super::{MANAGED_PROPERTIES, ScrollLockState, StylePropertySnapshot};
    use std::collections::HashMap;

    fn parse_px_int(raw: &str) -> i32 {
        let trimmed = raw.trim().trim_end_matches("px").trim();
        trimmed
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
            .map(|v| v as i32)
            .unwrap_or(0)
    }

    fn set_managed_property(
        state: &mut ScrollLockState,
        style: &web_sys::CssStyleDeclaration,
        prop: &'static str,
        value: &str,
    ) {
        let _ = style.set_property(prop, value);
        state.modified_properties.insert(prop);
        if prop == "overflow" {
            state.modified_properties.insert("overflow-x");
            state.modified_properties.insert("overflow-y");
        }
    }

    fn restore_single_property(
        state: &ScrollLockState,
        style: &web_sys::CssStyleDeclaration,
        prop: &'static str,
    ) {
        if let Some(snapshot) = state
            .initial_properties
            .as_ref()
            .and_then(|map| map.get(prop))
            && !snapshot.value.is_empty()
        {
            let _ = style.set_property_with_priority(prop, &snapshot.value, &snapshot.priority);
        } else {
            let _ = style.remove_property(prop);
        }
    }

    pub(super) fn apply_lock(state: &mut ScrollLockState, should_block_pointer: bool) {
        let Some(window) = web_sys::window() else {
            return;
        };
        let Some(document) = window.document() else {
            return;
        };
        let Some(body) = document.body() else {
            return;
        };
        let body_style = body.style();

        let is_first_capture = state.initial_properties.is_none();
        if is_first_capture {
            state.had_initial_style_attribute = body.has_attribute("style");
            let mut map = HashMap::with_capacity(MANAGED_PROPERTIES.len());
            for prop in MANAGED_PROPERTIES {
                let value = body_style.get_property_value(prop).unwrap_or_default();
                let priority = body_style.get_property_priority(prop);
                map.insert(prop, StylePropertySnapshot { value, priority });
            }
            state.initial_properties = Some(map);

            let doc_elem = document.document_element();
            let html_style = doc_elem
                .as_ref()
                .and_then(|el| window.get_computed_style(el).ok().flatten());
            let computed_body_style = window.get_computed_style(&body).ok().flatten();

            let has_stable_gutter = html_style
                .and_then(|s| s.get_property_value("scrollbar-gutter").ok())
                .unwrap_or_default()
                .contains("stable")
                || computed_body_style
                    .as_ref()
                    .and_then(|s| s.get_property_value("scrollbar-gutter").ok())
                    .unwrap_or_default()
                    .contains("stable");

            let window_inner_width = window
                .inner_width()
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0) as i32;
            let doc_client_width = doc_elem.map(|el| el.client_width()).unwrap_or(0);
            let scrollbar_width = window_inner_width - doc_client_width;

            if scrollbar_width > 0 && !has_stable_gutter {
                let base_padding = computed_body_style
                    .as_ref()
                    .and_then(|s| s.get_property_value("padding-right").ok())
                    .map(|v| parse_px_int(&v))
                    .unwrap_or(0);
                let base_margin = computed_body_style
                    .as_ref()
                    .and_then(|s| s.get_property_value("margin-right").ok())
                    .map(|v| parse_px_int(&v))
                    .unwrap_or(0);

                set_managed_property(
                    state,
                    &body_style,
                    "padding-right",
                    &format!("{}px", base_padding + scrollbar_width),
                );
                set_managed_property(
                    state,
                    &body_style,
                    "margin-right",
                    &format!("{base_margin}px"),
                );
                set_managed_property(
                    state,
                    &body_style,
                    "--scrollbar-width",
                    &format!("{scrollbar_width}px"),
                );
            }
        }

        set_managed_property(state, &body_style, "overflow", "hidden");

        if should_block_pointer {
            set_managed_property(state, &body_style, "pointer-events", "none");
        } else if state.modified_properties.remove("pointer-events") {
            restore_single_property(state, &body_style, "pointer-events");
        }
    }

    pub(super) fn restore_pointer_events(state: &mut ScrollLockState) {
        if !state.modified_properties.remove("pointer-events") {
            return;
        }
        let Some(window) = web_sys::window() else {
            return;
        };
        let Some(document) = window.document() else {
            return;
        };
        let Some(body) = document.body() else {
            return;
        };
        let body_style = body.style();
        restore_single_property(state, &body_style, "pointer-events");
        if !state.had_initial_style_attribute && body_style.length() == 0 {
            let _ = body.remove_attribute("style");
        }
    }

    pub(super) fn reset_body_style(state: &mut ScrollLockState) {
        if let Some(window) = web_sys::window()
            && let Some(document) = window.document()
            && let Some(body) = document.body()
        {
            let body_style = body.style();
            for prop in MANAGED_PROPERTIES {
                if !state.modified_properties.contains(prop) {
                    continue;
                }
                restore_single_property(state, &body_style, prop);
            }
            if !state.had_initial_style_attribute && body_style.length() == 0 {
                let _ = body.remove_attribute("style");
            }
        }
        state.initial_properties = None;
        state.modified_properties.clear();
        state.had_initial_style_attribute = false;
    }
}

/// Lossless CSS inline `style` declaration list parser and serializer used by Blitz Native
/// (and unit-tested across all targets).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct InlineStyleDeclarations {
    entries: Vec<(String, String, String)>,
}

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
impl InlineStyleDeclarations {
    pub(crate) fn parse(raw_css: &str) -> Self {
        let mut entries: Vec<(String, String, String)> = Vec::new();
        for decl in split_css_declarations(raw_css) {
            let trimmed = decl.trim();
            if trimmed.is_empty() {
                continue;
            }
            let Some((raw_name, raw_val)) = trimmed.split_once(':') else {
                continue;
            };
            let prop_name = normalize_prop_name(raw_name);
            if prop_name.is_empty() {
                continue;
            }
            let (value, priority) = split_value_and_priority(raw_val);
            if value.is_empty() {
                continue;
            }

            if let Some(existing_idx) = entries.iter().position(|(n, _, _)| n == &prop_name) {
                let existing_important = entries[existing_idx].2 == "important";
                let new_important = priority == "important";
                if new_important || !existing_important {
                    entries.remove(existing_idx);
                    entries.push((prop_name, value, priority));
                }
            } else {
                entries.push((prop_name, value, priority));
            }
        }
        Self { entries }
    }

    pub(crate) fn get_property_value(&self, name: &str) -> String {
        let norm = normalize_prop_name(name);
        self.entries
            .iter()
            .find(|(n, _, _)| n == &norm)
            .map(|(_, v, _)| v.clone())
            .unwrap_or_default()
    }

    pub(crate) fn get_property_priority(&self, name: &str) -> String {
        let norm = normalize_prop_name(name);
        self.entries
            .iter()
            .find(|(n, _, _)| n == &norm)
            .map(|(_, _, p)| p.clone())
            .unwrap_or_default()
    }

    pub(crate) fn set_property_with_priority(&mut self, name: &str, value: &str, priority: &str) {
        let norm = normalize_prop_name(name);
        let val_trimmed = value.trim();
        if val_trimmed.is_empty() {
            self.remove_property(&norm);
            return;
        }
        let prio = if priority.trim().eq_ignore_ascii_case("important") {
            "important".to_string()
        } else {
            String::new()
        };

        if norm == "overflow" {
            self.entries
                .retain(|(n, _, _)| n != "overflow-x" && n != "overflow-y");
        }

        if let Some(existing) = self.entries.iter_mut().find(|(n, _, _)| n == &norm) {
            existing.1 = val_trimmed.to_string();
            existing.2 = prio;
        } else {
            self.entries.push((norm, val_trimmed.to_string(), prio));
        }
    }

    pub(crate) fn remove_property(&mut self, name: &str) {
        let norm = normalize_prop_name(name);
        self.entries.retain(|(n, _, _)| n != &norm);
    }

    #[cfg_attr(
        all(target_arch = "wasm32", not(test)),
        allow(dead_code)
    )]
    #[cfg_attr(
        all(not(target_arch = "wasm32"), not(feature = "native")),
        allow(dead_code)
    )]
    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn serialize(&self) -> String {
        let mut out = String::new();
        for (idx, (name, value, priority)) in self.entries.iter().enumerate() {
            if idx > 0 {
                out.push(' ');
            }
            out.push_str(name);
            out.push_str(": ");
            out.push_str(value);
            if priority == "important" {
                out.push_str(" !important");
            }
            out.push(';');
        }
        out
    }
}

fn normalize_prop_name(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.starts_with("--") {
        trimmed.to_string()
    } else {
        trimmed.to_ascii_lowercase()
    }
}

fn split_value_and_priority(raw_val: &str) -> (String, String) {
    let trimmed = raw_val.trim();
    if trimmed.len() >= 10 {
        let tail_start = trimmed.len() - 9;
        if trimmed[tail_start..].eq_ignore_ascii_case("important") {
            let before_important = trimmed[..tail_start].trim_end();
            if let Some(before_bang) = before_important.strip_suffix('!') {
                return (before_bang.trim().to_string(), "important".to_string());
            }
        }
    }
    (trimmed.to_string(), String::new())
}

fn split_css_declarations(raw_css: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut paren_depth: usize = 0;
    let bytes = raw_css.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\\' && (in_single_quote || in_double_quote) {
            i += 2;
            continue;
        }
        match b {
            b'\'' if !in_double_quote => in_single_quote = !in_single_quote,
            b'"' if !in_single_quote => in_double_quote = !in_double_quote,
            b'(' if !in_single_quote && !in_double_quote => {
                paren_depth = paren_depth.saturating_add(1);
            }
            b')' if !in_single_quote && !in_double_quote => {
                paren_depth = paren_depth.saturating_sub(1);
            }
            b';' if !in_single_quote && !in_double_quote && paren_depth == 0 => {
                parts.push(&raw_css[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    if start <= raw_css.len() {
        parts.push(&raw_css[start..]);
    }
    parts
}

#[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
mod native_host {
    use super::{
        InlineStyleDeclarations, MANAGED_PROPERTIES, ScrollLockState, StylePropertySnapshot,
    };
    use blitz_dom::{BaseDocument, NodeId};
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;

    fn read_body_style_attr(base_rc: &Rc<RefCell<BaseDocument>>, body_id: NodeId) -> Option<String> {
        let el = crate::dom::Element::from_native(base_rc.clone(), body_id);
        el.get_attribute("style").or_else(|| {
            let base = base_rc.try_borrow().ok()?;
            let node = base.get_node(body_id)?;
            node.attr(blitz_dom::local_name!("style"))
                .map(|s| s.to_string())
        })
    }

    fn write_body_style_decls(
        base_rc: &Rc<RefCell<BaseDocument>>,
        body_id: NodeId,
        decls: &InlineStyleDeclarations,
        had_initial_style_attribute: bool,
    ) {
        let el = crate::dom::Element::from_native(base_rc.clone(), body_id);
        if decls.is_empty() && !had_initial_style_attribute {
            el.remove_attribute("style");
            if let Ok(mut base) = base_rc.try_borrow_mut()
                && base.get_node(body_id).is_some()
            {
                let qname = blitz_dom::QualName::new(
                    None,
                    blitz_dom::ns!(html),
                    blitz_dom::local_name!("style"),
                );
                base.mutate().clear_attribute(body_id, qname);
            }
        } else {
            let serialized = decls.serialize();
            el.set_attribute("style", &serialized);
        }
    }

    fn set_managed_property(
        state: &mut ScrollLockState,
        decls: &mut InlineStyleDeclarations,
        prop: &'static str,
        value: &str,
    ) {
        decls.set_property_with_priority(prop, value, "");
        state.modified_properties.insert(prop);
        if prop == "overflow" {
            state.modified_properties.insert("overflow-x");
            state.modified_properties.insert("overflow-y");
        }
    }

    fn restore_single_property(
        state: &ScrollLockState,
        decls: &mut InlineStyleDeclarations,
        prop: &'static str,
    ) {
        if let Some(snapshot) = state
            .initial_properties
            .as_ref()
            .and_then(|map| map.get(prop))
            && !snapshot.value.is_empty()
        {
            decls.set_property_with_priority(prop, &snapshot.value, &snapshot.priority);
        } else {
            decls.remove_property(prop);
        }
    }

    fn resolve_active_or_locked_body(
        state: &mut ScrollLockState,
    ) -> Option<(Rc<RefCell<BaseDocument>>, NodeId)> {
        if let Some(doc) = crate::dom::Document::current()
            && let Some(base_rc) = doc.get_base()
        {
            let body_opt = base_rc
                .try_borrow()
                .ok()
                .and_then(|base| base.find_body_node().map(|n| n.id));
            if let Some(body_id) = body_opt {
                if let Some((prev_weak, prev_body_id)) = &state.locked_native_body {
                    let switched_doc = match prev_weak.upgrade() {
                        Some(prev_rc) => !Rc::ptr_eq(&prev_rc, &base_rc) || *prev_body_id != body_id,
                        None => true,
                    };
                    if switched_doc {
                        if let Some(prev_rc) = prev_weak.upgrade() {
                            reset_body_on_target(state, &prev_rc, *prev_body_id);
                        } else {
                            state.initial_properties = None;
                            state.modified_properties.clear();
                            state.had_initial_style_attribute = false;
                        }
                        state.locked_native_body = None;
                    }
                }
                return Some((base_rc, body_id));
            }
        }

        if let Some((weak, body_id)) = &state.locked_native_body
            && let Some(base_rc) = weak.upgrade()
        {
            return Some((base_rc, *body_id));
        }

        None
    }

    fn reset_body_on_target(
        state: &mut ScrollLockState,
        base_rc: &Rc<RefCell<BaseDocument>>,
        body_id: NodeId,
    ) {
        let current_raw = read_body_style_attr(base_rc, body_id).unwrap_or_default();
        let mut decls = InlineStyleDeclarations::parse(&current_raw);
        for prop in MANAGED_PROPERTIES {
            if !state.modified_properties.contains(prop) {
                continue;
            }
            restore_single_property(state, &mut decls, prop);
        }
        write_body_style_decls(base_rc, body_id, &decls, state.had_initial_style_attribute);
        state.initial_properties = None;
        state.modified_properties.clear();
        state.had_initial_style_attribute = false;
    }

    pub(super) fn apply_lock(state: &mut ScrollLockState, should_block_pointer: bool) {
        let Some((base_rc, body_id)) = resolve_active_or_locked_body(state) else {
            return;
        };

        let raw_style = read_body_style_attr(&base_rc, body_id);
        let mut decls = InlineStyleDeclarations::parse(raw_style.as_deref().unwrap_or(""));

        if state.initial_properties.is_none() {
            state.had_initial_style_attribute = raw_style.is_some();
            let mut map = HashMap::with_capacity(MANAGED_PROPERTIES.len());
            for prop in MANAGED_PROPERTIES {
                map.insert(
                    prop,
                    StylePropertySnapshot {
                        value: decls.get_property_value(prop),
                        priority: decls.get_property_priority(prop),
                    },
                );
            }
            state.initial_properties = Some(map);
            state.locked_native_body = Some((Rc::downgrade(&base_rc), body_id));
        }

        set_managed_property(state, &mut decls, "overflow", "hidden");

        if should_block_pointer {
            set_managed_property(state, &mut decls, "pointer-events", "none");
        } else if state.modified_properties.remove("pointer-events") {
            restore_single_property(state, &mut decls, "pointer-events");
        }

        write_body_style_decls(&base_rc, body_id, &decls, state.had_initial_style_attribute);
    }

    pub(super) fn restore_pointer_events(state: &mut ScrollLockState) {
        if !state.modified_properties.remove("pointer-events") {
            return;
        }
        let Some((weak, body_id)) = state.locked_native_body.clone() else {
            return;
        };
        let Some(base_rc) = weak.upgrade() else {
            return;
        };
        let raw_style = read_body_style_attr(&base_rc, body_id).unwrap_or_default();
        let mut decls = InlineStyleDeclarations::parse(&raw_style);
        restore_single_property(state, &mut decls, "pointer-events");
        write_body_style_decls(&base_rc, body_id, &decls, state.had_initial_style_attribute);
    }

    pub(super) fn reset_body_style(state: &mut ScrollLockState) {
        if let Some((weak, body_id)) = state.locked_native_body.take()
            && let Some(base_rc) = weak.upgrade()
        {
            reset_body_on_target(state, &base_rc, body_id);
        } else {
            state.initial_properties = None;
            state.modified_properties.clear();
            state.had_initial_style_attribute = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inline_style_declarations_roundtrip_and_priority() {
        let mut decls = InlineStyleDeclarations::parse(
            "overflow-y: scroll; margin-right: 12px !important; color: rgb(1, 2, 3); --scrollbar-width: 8px",
        );
        assert_eq!(decls.get_property_value("overflow"), "");
        assert_eq!(decls.get_property_value("overflow-y"), "scroll");
        assert_eq!(decls.get_property_priority("overflow-y"), "");
        assert_eq!(decls.get_property_value("margin-right"), "12px");
        assert_eq!(decls.get_property_priority("margin-right"), "important");
        assert_eq!(decls.get_property_value("color"), "rgb(1, 2, 3)");
        assert_eq!(decls.get_property_value("--scrollbar-width"), "8px");

        // Setting overflow shorthand clears inline overflow-x/y while active
        decls.set_property_with_priority("overflow", "hidden", "");
        assert_eq!(decls.get_property_value("overflow"), "hidden");
        assert_eq!(decls.get_property_value("overflow-y"), "");

        // Restoring in MANAGED_PROPERTIES order restores overflow-y: scroll and margin-right: 12px !important
        decls.remove_property("overflow");
        decls.remove_property("overflow-x");
        decls.set_property_with_priority("overflow-y", "scroll", "");
        assert_eq!(decls.get_property_value("overflow"), "");
        assert_eq!(decls.get_property_value("overflow-y"), "scroll");
        assert_eq!(decls.get_property_value("margin-right"), "12px");
        assert_eq!(decls.get_property_priority("margin-right"), "important");
        assert_eq!(decls.get_property_value("color"), "rgb(1, 2, 3)");
        assert!(decls.serialize().contains("margin-right: 12px !important;"));
        assert!(decls.serialize().contains("overflow-y: scroll;"));
    }

    #[test]
    fn test_scroll_lock_options_pointer_events_state() {
        release_scroll_lock("opt-a", None);
        release_scroll_lock("opt-b", None);
        assert!(!is_scroll_locked());
        assert!(!is_body_pointer_events_locked());

        acquire_scroll_lock_with_options(
            "opt-a",
            ScrollLockOptions {
                prevent_body_pointer_events: false,
            },
        );
        assert!(is_scroll_locked());
        assert!(!is_body_pointer_events_locked());

        acquire_scroll_lock_with_options(
            "opt-b",
            ScrollLockOptions {
                prevent_body_pointer_events: true,
            },
        );
        assert!(is_scroll_locked());
        assert!(is_body_pointer_events_locked());

        release_scroll_lock("opt-b", None);
        assert!(is_scroll_locked());
        assert!(
            !is_body_pointer_events_locked(),
            "Releasing last pointer-blocking lock must unlock pointer-events while opt-a holds scroll lock"
        );

        release_scroll_lock("opt-a", None);
        assert!(!is_scroll_locked());
        assert!(!is_body_pointer_events_locked());
    }
}
