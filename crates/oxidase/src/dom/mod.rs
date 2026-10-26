//! Pure-Rust cross-platform DOM abstraction layer.
//!
//! Provides a unified, synchronous DOM access API across Web (`wasm32`)
//! and Native (`blitz-dom`).

#[cfg(target_arch = "wasm32")]
#[path = "web.rs"]
mod backend;

#[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
#[path = "native.rs"]
mod backend;

#[cfg(all(not(target_arch = "wasm32"), not(feature = "native")))]
#[path = "mock.rs"]
mod backend;

pub mod element;
pub use element::*;

pub mod observer;
pub use observer::*;

pub use backend::Document;

#[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
pub use backend::extract_base_doc;

/// Returns the ambient active `Document`.
pub fn document() -> Document {
    Document::current().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_document_current() {
        // On non-wasm host without active mock, current() returns None
        assert!(Document::current().is_none());
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
    #[test]
    fn test_native_document_resolution() {
        use blitz_dom::{BaseDocument, DocumentConfig};
        use std::cell::RefCell;
        use std::rc::Rc;

        // Initially None
        assert!(Document::current().is_none());

        // Create a real Blitz BaseDocument handle
        let base_doc = Rc::new(RefCell::new(BaseDocument::new(DocumentConfig::default())));
        let native_doc = Document::from_base(base_doc.clone());
        assert!(native_doc.is_authoritative());

        let unattached_doc = Document::from_unattached_base(base_doc.clone());
        assert!(!unattached_doc.is_authoritative());
        assert!(Document::default().is_authoritative());

        // Test with_current scope
        Document::with_current(native_doc.clone(), || {
            let cur = Document::current().expect("Document::current() must be Some within with_current");
            assert_eq!(cur.base().borrow().id(), base_doc.borrow().id());
        });

        // After scope, must be None again
        assert!(Document::current().is_none());

        // Test set_current
        Document::set_current(Some(native_doc));
        assert!(Document::current().is_some());
        Document::set_current(None);
        assert!(Document::current().is_none());
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
    #[test]
    fn test_native_document_query_descendant_order() {
        use blitz_dom::{BaseDocument, DocumentConfig};
        use std::cell::RefCell;
        use std::rc::Rc;

        let base_doc = Rc::new(RefCell::new(BaseDocument::new(DocumentConfig::default())));
        let native_doc = Document::from_base(base_doc.clone());

        let qual_name = |name: &'static str| blitz_dom::QualName {
            prefix: None,
            ns: blitz_dom::ns!(html),
            local: blitz_dom::LocalName::from(name),
        };
        let attr = |name: &'static str, value: &str| blitz_dom::Attribute {
            name: qual_name(name),
            value: value.to_string(),
        };

        {
            let mut base = base_doc.borrow_mut();
            let root_id = base.root_node().id;
            let mut mutator = base.mutate();
            let content = mutator.create_element(qual_name("div"), vec![attr("id", "content")]);
            let g1 = mutator.create_element(qual_name("div"), vec![attr("id", "g1")]);
            let g1_a = mutator.create_element(qual_name("div"), vec![attr("id", "g1-a")]);
            let g1_b = mutator.create_element(qual_name("div"), vec![attr("id", "g1-b")]);
            let g2 = mutator.create_element(qual_name("div"), vec![attr("id", "g2")]);
            let g2_c = mutator.create_element(qual_name("div"), vec![attr("id", "g2-c")]);
            let g2_d = mutator.create_element(qual_name("div"), vec![attr("id", "g2-d")]);

            mutator.append_children(g1, &[g1_a, g1_b]);
            mutator.append_children(g2, &[g2_c, g2_d]);
            mutator.append_children(content, &[g1, g2]);
            mutator.append_children(root_id, &[content]);
        }

        let order = native_doc
            .query_descendant_order("content", &["g2-d", "g1-b", "g2-c", "g1-a"])
            .expect("query_descendant_order should succeed");
        assert_eq!(order, vec!["g1-a", "g1-b", "g2-c", "g2-d"]);

        // Non-existent root returns ElementNotFound
        let missing = native_doc.query_descendant_order("nonexistent", &["g1-a"]);
        assert!(matches!(missing, Err(crate::error::HostError::ElementNotFound(_))));
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
    #[test]
    fn test_native_sync_animation_observers_non_destructive() {
        use blitz_dom::{BaseDocument, DocumentConfig, QueuedAnimationEvent};
        use blitz_traits::events::{BlitzAnimationEvent, DomEventData};
        use crate::dom::observer::{observe_transition_lifecycle, TransitionLifecycleEvent};
        use std::cell::RefCell;
        use std::rc::Rc;

        let base_doc = Rc::new(RefCell::new(BaseDocument::new(DocumentConfig::default())));
        let native_doc = Document::from_base(base_doc.clone());

        let target_node_id = {
            let mut base = base_doc.borrow_mut();
            let root_id = base.root_node().id;
            let mut mutator = base.mutate();
            let el = mutator.create_element(
                blitz_dom::QualName {
                    prefix: None,
                    ns: blitz_dom::ns!(html),
                    local: blitz_dom::LocalName::from("div"),
                },
                vec![blitz_dom::Attribute {
                    name: blitz_dom::QualName {
                        prefix: None,
                        ns: blitz_dom::ns!(html),
                        local: blitz_dom::LocalName::from("id"),
                    },
                    value: "anim-box".to_string(),
                }],
            );
            mutator.append_children(root_id, &[el]);
            el
        };

        let events_log = Rc::new(RefCell::new(Vec::new()));
        let el_clone = events_log.clone();
        let _guard = observe_transition_lifecycle("anim-box", 0, move |ev| {
            el_clone.borrow_mut().push(ev);
        })
        .expect("observe_transition_lifecycle must succeed");

        // Push AnimationEnd event to Blitz's pending queue
        base_doc.borrow_mut().pending_animation_events.push(QueuedAnimationEvent {
            node_id: target_node_id,
            event: DomEventData::AnimationEnd(BlitzAnimationEvent {
                animation_name: "fade-in".to_string(),
                elapsed_time: 1.0,
                pseudo_element: blitz_traits::SmolStr::default(),
            }),
        });

        // Sync from index 0
        native_doc.sync_animation_observers(0);

        // Verification 1: Callback received AnimationEnd
        assert_eq!(
            *events_log.borrow(),
            vec![TransitionLifecycleEvent::AnimationEnd {
                animation_name: "fade-in".to_string(),
            }]
        );

        // Verification 2: Non-destructive - queue is NOT drained or cleared
        assert_eq!(base_doc.borrow().pending_animation_events.len(), 1);

        // Re-register observer for subsequent event (transition observers are one-shot)
        let el_clone2 = events_log.clone();
        let _guard2 = observe_transition_lifecycle("anim-box", 0, move |ev| {
            el_clone2.borrow_mut().push(ev);
        })
        .expect("observe_transition_lifecycle must succeed");

        // Sync with prev_len = 1 (no new events)
        native_doc.sync_animation_observers(1);
        assert_eq!(events_log.borrow().len(), 1);

        // Push AnimationCancel event
        base_doc.borrow_mut().pending_animation_events.push(QueuedAnimationEvent {
            node_id: target_node_id,
            event: DomEventData::AnimationCancel(BlitzAnimationEvent {
                animation_name: "slide-out".to_string(),
                elapsed_time: 0.5,
                pseudo_element: blitz_traits::SmolStr::default(),
            }),
        });

        // Sync delta from prev_len = 1
        native_doc.sync_animation_observers(1);

        // Verification 3: Received AnimationCancel
        assert_eq!(
            *events_log.borrow(),
            vec![
                TransitionLifecycleEvent::AnimationEnd {
                    animation_name: "fade-in".to_string(),
                },
                TransitionLifecycleEvent::AnimationCancel {
                    animation_name: "slide-out".to_string(),
                },
            ]
        );

        // Verification 4: Both events remain preserved in pending_animation_events for flush_animation_events()
        assert_eq!(base_doc.borrow().pending_animation_events.len(), 2);
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
    #[test]
    fn test_native_sync_transition_observers_non_destructive() {
        use blitz_dom::{BaseDocument, DocumentConfig, QueuedAnimationEvent};
        use blitz_traits::events::{BlitzTransitionEvent, DomEventData};
        use crate::dom::observer::{observe_transition_lifecycle, TransitionLifecycleEvent};
        use std::cell::RefCell;
        use std::rc::Rc;

        let base_doc = Rc::new(RefCell::new(BaseDocument::new(DocumentConfig::default())));
        let native_doc = Document::from_base(base_doc.clone());

        let target_node_id = {
            let mut base = base_doc.borrow_mut();
            let root_id = base.root_node().id;
            let mut mutator = base.mutate();
            let el = mutator.create_element(
                blitz_dom::QualName {
                    prefix: None,
                    ns: blitz_dom::ns!(html),
                    local: blitz_dom::LocalName::from("div"),
                },
                vec![blitz_dom::Attribute {
                    name: blitz_dom::QualName {
                        prefix: None,
                        ns: blitz_dom::ns!(html),
                        local: blitz_dom::LocalName::from("id"),
                    },
                    value: "trans-box".to_string(),
                }],
            );
            mutator.append_children(root_id, &[el]);
            el
        };

        let events_log = Rc::new(RefCell::new(Vec::new()));
        let el_clone = events_log.clone();
        let _guard = observe_transition_lifecycle("trans-box", 0, move |ev| {
            el_clone.borrow_mut().push(ev);
        })
        .expect("observe_transition_lifecycle must succeed");

        // Push TransitionEnd event to Blitz's pending queue
        base_doc.borrow_mut().pending_animation_events.push(QueuedAnimationEvent {
            node_id: target_node_id,
            event: DomEventData::TransitionEnd(BlitzTransitionEvent {
                property_name: "opacity".to_string(),
                elapsed_time: 0.1,
                pseudo_element: blitz_traits::SmolStr::default(),
            }),
        });

        // Sync from index 0
        native_doc.sync_animation_observers(0);

        // Verification 1: Callback received TransitionEnd
        assert_eq!(
            *events_log.borrow(),
            vec![TransitionLifecycleEvent::TransitionEnd {
                property_name: "opacity".to_string(),
            }]
        );

        // Verification 2: Non-destructive - queue is NOT drained or cleared
        assert_eq!(base_doc.borrow().pending_animation_events.len(), 1);

        // Re-register observer for subsequent event (transition observers are one-shot)
        let el_clone2 = events_log.clone();
        let _guard2 = observe_transition_lifecycle("trans-box", 0, move |ev| {
            el_clone2.borrow_mut().push(ev);
        })
        .expect("observe_transition_lifecycle must succeed");

        // Sync with prev_len = 1 (no new events)
        native_doc.sync_animation_observers(1);
        assert_eq!(events_log.borrow().len(), 1);

        // Push TransitionCancel event
        base_doc.borrow_mut().pending_animation_events.push(QueuedAnimationEvent {
            node_id: target_node_id,
            event: DomEventData::TransitionCancel(BlitzTransitionEvent {
                property_name: "transform".to_string(),
                elapsed_time: 0.05,
                pseudo_element: blitz_traits::SmolStr::default(),
            }),
        });

        // Sync delta from prev_len = 1
        native_doc.sync_animation_observers(1);

        // Verification 3: Received TransitionCancel
        assert_eq!(
            *events_log.borrow(),
            vec![
                TransitionLifecycleEvent::TransitionEnd {
                    property_name: "opacity".to_string(),
                },
                TransitionLifecycleEvent::TransitionCancel {
                    property_name: "transform".to_string(),
                },
            ]
        );

        // Verification 4: Both events remain preserved in pending_animation_events for flush_animation_events()
        assert_eq!(base_doc.borrow().pending_animation_events.len(), 2);
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
    #[test]
    fn test_native_sync_animation_observers_borrow_safety() {
        use blitz_dom::{BaseDocument, DocumentConfig, QueuedAnimationEvent};
        use blitz_traits::events::{BlitzAnimationEvent, DomEventData};
        use crate::dom::observer::observe_transition_lifecycle;
        use std::cell::RefCell;
        use std::rc::Rc;

        let base_doc = Rc::new(RefCell::new(BaseDocument::new(DocumentConfig::default())));
        let native_doc = Document::from_base(base_doc.clone());

        let target_node_id = {
            let mut base = base_doc.borrow_mut();
            let root_id = base.root_node().id;
            let mut mutator = base.mutate();
            let el = mutator.create_element(
                blitz_dom::QualName {
                    prefix: None,
                    ns: blitz_dom::ns!(html),
                    local: blitz_dom::LocalName::from("div"),
                },
                vec![blitz_dom::Attribute {
                    name: blitz_dom::QualName {
                        prefix: None,
                        ns: blitz_dom::ns!(html),
                        local: blitz_dom::LocalName::from("id"),
                    },
                    value: "borrow-box".to_string(),
                }],
            );
            mutator.append_children(root_id, &[el]);
            el
        };

        let reentrancy_called = Rc::new(RefCell::new(false));
        let rc_clone = reentrancy_called.clone();
        let doc_clone = native_doc.clone();

        let _guard = observe_transition_lifecycle("borrow-box", 0, move |_| {
            // Re-entrant mutable borrow on base document inside observer callback
            doc_clone.resolve(0.0);
            *rc_clone.borrow_mut() = true;
        })
        .expect("observe_transition_lifecycle must succeed");

        base_doc.borrow_mut().pending_animation_events.push(QueuedAnimationEvent {
            node_id: target_node_id,
            event: DomEventData::AnimationEnd(BlitzAnimationEvent {
                animation_name: "reentrant-anim".to_string(),
                elapsed_time: 1.0,
                pseudo_element: blitz_traits::SmolStr::default(),
            }),
        });

        // If the base document borrow were held during observer callback execution,
        // this call would panic with BorrowMutError. It must succeed safely.
        native_doc.sync_animation_observers(0);

        assert!(*reentrancy_called.borrow(), "Re-entrant callback must be invoked safely");
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
    #[test]
    fn test_native_sync_animation_observers_all() {
        use blitz_dom::{BaseDocument, DocumentConfig, QueuedAnimationEvent};
        use blitz_traits::events::{BlitzAnimationEvent, DomEventData};
        use crate::dom::observer::{observe_transition_lifecycle, TransitionLifecycleEvent};
        use std::cell::RefCell;
        use std::rc::Rc;

        let base_doc = Rc::new(RefCell::new(BaseDocument::new(DocumentConfig::default())));
        let native_doc = Document::from_base(base_doc.clone());

        let target_node_id = {
            let mut base = base_doc.borrow_mut();
            let root_id = base.root_node().id;
            let mut mutator = base.mutate();
            let el = mutator.create_element(
                blitz_dom::QualName {
                    prefix: None,
                    ns: blitz_dom::ns!(html),
                    local: blitz_dom::LocalName::from("div"),
                },
                vec![blitz_dom::Attribute {
                    name: blitz_dom::QualName {
                        prefix: None,
                        ns: blitz_dom::ns!(html),
                        local: blitz_dom::LocalName::from("id"),
                    },
                    value: "box-all".to_string(),
                }],
            );
            mutator.append_children(root_id, &[el]);
            el
        };

        let events_log = Rc::new(RefCell::new(Vec::new()));
        let el_clone = events_log.clone();
        let _guard = observe_transition_lifecycle("box-all", 0, move |ev| {
            el_clone.borrow_mut().push(ev);
        })
        .expect("observe_transition_lifecycle must succeed");

        base_doc.borrow_mut().pending_animation_events.push(QueuedAnimationEvent {
            node_id: target_node_id,
            event: DomEventData::AnimationEnd(BlitzAnimationEvent {
                animation_name: "zoom-in".to_string(),
                elapsed_time: 0.25,
                pseudo_element: blitz_traits::SmolStr::default(),
            }),
        });

        native_doc.sync_animation_observers_all();

        assert_eq!(
            *events_log.borrow(),
            vec![TransitionLifecycleEvent::AnimationEnd {
                animation_name: "zoom-in".to_string(),
            }]
        );
        assert_eq!(base_doc.borrow().pending_animation_events.len(), 1);
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
    #[test]
    fn test_native_document_bind_live_base_in_place_upgrade() {
        use blitz_dom::{BaseDocument, DocumentConfig};
        use std::cell::RefCell;
        use std::rc::Rc;

        let ambient_base = Rc::new(RefCell::new(BaseDocument::new(DocumentConfig::default())));
        let doc = Document::from_unattached_base(ambient_base.clone());
        assert!(!doc.is_authoritative());

        let doc_clone = doc.clone();
        assert!(!doc_clone.is_authoritative());

        let live_base = Rc::new(RefCell::new(BaseDocument::new(DocumentConfig::default())));
        assert_ne!(ambient_base.borrow().id(), live_base.borrow().id());

        doc.bind_live_base(live_base.clone());

        assert!(doc.is_authoritative());
        assert!(doc_clone.is_authoritative());
        assert_eq!(doc_clone.base().borrow().id(), live_base.borrow().id());
        assert_eq!(doc.base().borrow().id(), live_base.borrow().id());
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
    #[test]
    fn test_native_document_pump_hosted_observers() {
        use blitz_dom::{BaseDocument, DocumentConfig, QueuedAnimationEvent};
        use blitz_traits::events::{BlitzAnimationEvent, DomEventData};
        use crate::dom::observer::{observe_transition_lifecycle, TransitionLifecycleEvent};
        use std::cell::RefCell;
        use std::rc::Rc;

        let base_doc = Rc::new(RefCell::new(BaseDocument::new(DocumentConfig::default())));
        let native_doc = Document::from_base(base_doc.clone());

        let target_node_id = {
            let mut base = base_doc.borrow_mut();
            let root_id = base.root_node().id;
            let mut mutator = base.mutate();
            let el = mutator.create_element(
                blitz_dom::QualName {
                    prefix: None,
                    ns: blitz_dom::ns!(html),
                    local: blitz_dom::LocalName::from("div"),
                },
                vec![blitz_dom::Attribute {
                    name: blitz_dom::QualName {
                        prefix: None,
                        ns: blitz_dom::ns!(html),
                        local: blitz_dom::LocalName::from("id"),
                    },
                    value: "pump-box".to_string(),
                }],
            );
            mutator.append_children(root_id, &[el]);
            el
        };

        let events_log = Rc::new(RefCell::new(Vec::new()));
        let el_clone = events_log.clone();
        let _guard = observe_transition_lifecycle("pump-box", 0, move |ev| {
            el_clone.borrow_mut().push(ev);
        })
        .expect("observe_transition_lifecycle must succeed");

        // External resolve / event queueing
        base_doc.borrow_mut().pending_animation_events.push(QueuedAnimationEvent {
            node_id: target_node_id,
            event: DomEventData::AnimationEnd(BlitzAnimationEvent {
                animation_name: "pump-anim-1".to_string(),
                elapsed_time: 0.1,
                pseudo_element: blitz_traits::SmolStr::default(),
            }),
        });

        // Pump observers
        native_doc.pump_hosted_observers();

        assert_eq!(
            *events_log.borrow(),
            vec![TransitionLifecycleEvent::AnimationEnd {
                animation_name: "pump-anim-1".to_string(),
            }]
        );
        // Non-destructive: queue is not drained
        assert_eq!(base_doc.borrow().pending_animation_events.len(), 1);

        // Subsequent pump without new events should be idempotent (no duplicated callbacks)
        native_doc.pump_hosted_observers();
        assert_eq!(events_log.borrow().len(), 1);

        // Re-register observer for second event
        let el_clone2 = events_log.clone();
        let _guard2 = observe_transition_lifecycle("pump-box", 0, move |ev| {
            el_clone2.borrow_mut().push(ev);
        })
        .expect("observe_transition_lifecycle must succeed");

        // Second external event queued
        base_doc.borrow_mut().pending_animation_events.push(QueuedAnimationEvent {
            node_id: target_node_id,
            event: DomEventData::AnimationEnd(BlitzAnimationEvent {
                animation_name: "pump-anim-2".to_string(),
                elapsed_time: 0.2,
                pseudo_element: blitz_traits::SmolStr::default(),
            }),
        });

        // Pump again
        native_doc.pump_hosted_observers();
        assert_eq!(
            *events_log.borrow(),
            vec![
                TransitionLifecycleEvent::AnimationEnd {
                    animation_name: "pump-anim-1".to_string(),
                },
                TransitionLifecycleEvent::AnimationEnd {
                    animation_name: "pump-anim-2".to_string(),
                },
            ]
        );
        assert_eq!(base_doc.borrow().pending_animation_events.len(), 2);

        // Drain queue externally (simulating flush_animation_events)
        base_doc.borrow_mut().pending_animation_events.clear();
        assert_eq!(base_doc.borrow().pending_animation_events.len(), 0);

        // Pumping after drain resets cursor cleanly without panic
        native_doc.pump_hosted_observers();
        assert_eq!(events_log.borrow().len(), 2);

        // Re-register observer for third event after drain
        let el_clone3 = events_log.clone();
        let _guard3 = observe_transition_lifecycle("pump-box", 0, move |ev| {
            el_clone3.borrow_mut().push(ev);
        })
        .expect("observe_transition_lifecycle must succeed");

        base_doc.borrow_mut().pending_animation_events.push(QueuedAnimationEvent {
            node_id: target_node_id,
            event: DomEventData::AnimationEnd(BlitzAnimationEvent {
                animation_name: "pump-anim-3".to_string(),
                elapsed_time: 0.3,
                pseudo_element: blitz_traits::SmolStr::default(),
            }),
        });

        native_doc.pump_hosted_observers();
        assert_eq!(
            *events_log.borrow(),
            vec![
                TransitionLifecycleEvent::AnimationEnd {
                    animation_name: "pump-anim-1".to_string(),
                },
                TransitionLifecycleEvent::AnimationEnd {
                    animation_name: "pump-anim-2".to_string(),
                },
                TransitionLifecycleEvent::AnimationEnd {
                    animation_name: "pump-anim-3".to_string(),
                },
            ]
        );
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
    #[test]
    fn test_native_document_is_animating() {
        use blitz_dom::{BaseDocument, DocumentConfig};
        use std::cell::RefCell;
        use std::rc::Rc;

        let base_doc = Rc::new(RefCell::new(BaseDocument::new(DocumentConfig::default())));
        let native_doc = Document::from_base(base_doc.clone());
        assert!(!native_doc.is_animating());
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
    #[test]
    fn test_element_attribute_empty_namespace_parity() {
        use blitz_dom::{BaseDocument, DocumentConfig, QualName, local_name, ns};
        use std::cell::RefCell;
        use std::rc::Rc;

        let base_doc = Rc::new(RefCell::new(BaseDocument::new(DocumentConfig::default())));
        let native_doc = Document::from_base(base_doc.clone());

        let target_node_id = {
            let mut base = base_doc.borrow_mut();
            let root_id = base.root_node().id;
            let mut mutator = base.mutate();
            let el = mutator.create_element(
                QualName::new(None, ns!(html), local_name!("div")),
                vec![blitz_dom::Attribute {
                    name: QualName::new(None, ns!(html), local_name!("id")),
                    value: "attr-test-box".to_string(),
                }],
            );
            mutator.append_children(root_id, &[el]);
            el
        };

        let el = native_doc
            .element_by_id("attr-test-box")
            .expect("element attr-test-box must exist");

        // Initial state
        assert!(!el.has_attribute("data-state"));
        assert_eq!(el.get_attribute("data-state"), None);

        // 1. set_attribute with non-namespaced attribute
        el.set_attribute("data-state", "open");
        assert!(el.has_attribute("data-state"));
        assert_eq!(el.get_attribute("data-state"), Some("open".to_string()));

        // Inspect raw Blitz DOM attributes to assert strict empty namespace parity ns!()
        {
            let base = base_doc.borrow();
            let node = base.get_node(target_node_id).expect("node must exist");
            let elem_data = node.element_data().expect("must be element");
            let matching_attrs: Vec<_> = elem_data
                .attrs
                .iter()
                .filter(|a| &*a.name.local == "data-state")
                .collect();
            assert_eq!(matching_attrs.len(), 1, "Must have exactly one data-state attribute");
            assert_eq!(
                matching_attrs[0].name.ns,
                blitz_dom::ns!(),
                "Attribute namespace must strictly match empty namespace ns!()"
            );
            assert_eq!(matching_attrs[0].value, "open");
        }

        // 2. update attribute value
        el.set_attribute("data-state", "closed");
        assert!(el.has_attribute("data-state"));
        assert_eq!(el.get_attribute("data-state"), Some("closed".to_string()));

        // Assert zero duplicate entries and updated value in raw DOM
        {
            let base = base_doc.borrow();
            let node = base.get_node(target_node_id).expect("node must exist");
            let elem_data = node.element_data().expect("must be element");
            let matching_attrs: Vec<_> = elem_data
                .attrs
                .iter()
                .filter(|a| &*a.name.local == "data-state")
                .collect();
            assert_eq!(
                matching_attrs.len(),
                1,
                "Must not create duplicate attributes across updates"
            );
            assert_eq!(matching_attrs[0].name.ns, blitz_dom::ns!());
            assert_eq!(matching_attrs[0].value, "closed");
        }

        // 3. remove_attribute
        el.remove_attribute("data-state");
        assert!(!el.has_attribute("data-state"));
        assert_eq!(el.get_attribute("data-state"), None);

        // Assert attribute is completely gone from raw DOM
        {
            let base = base_doc.borrow();
            let node = base.get_node(target_node_id).expect("node must exist");
            let elem_data = node.element_data().expect("must be element");
            assert!(
                !elem_data.attrs.iter().any(|a| &*a.name.local == "data-state"),
                "Attribute must be removed from raw element attrs"
            );
        }
    }
}
