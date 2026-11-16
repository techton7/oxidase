//! Platform-agnostic single-threaded `requestAnimationFrame` batching registry.
//!
//! Coalesces concurrent `request_next_frame` one-shots and `start_frame_loop` subscribers
//! into a single frame step so every callback in the same frame observes an identical,
//! hitch-clamped [`FrameInfo`] delta, and resets frame timing cleanly upon entering idle.

#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::Duration;

use super::types::FrameInfo;

pub(crate) struct LoopSubscriber {
    pub(crate) active: Rc<Cell<bool>>,
    pub(crate) callback: Rc<RefCell<Box<dyn FnMut(FrameInfo)>>>,
}

pub(crate) struct OneShotRequest {
    pub(crate) active: Rc<Cell<bool>>,
    pub(crate) callback: Rc<RefCell<Option<Box<dyn FnOnce(FrameInfo)>>>>,
}

pub(crate) struct PreparedRafStep {
    pub(crate) info: FrameInfo,
    pub(crate) one_shots: BTreeMap<u64, OneShotRequest>,
    pub(crate) loops: Vec<(Rc<Cell<bool>>, Rc<RefCell<Box<dyn FnMut(FrameInfo)>>>)>,
}

#[derive(Default)]
pub(crate) struct RafFrameRegistry {
    next_id: u64,
    last_raf_ms: Option<f64>,
    scheduled_raf_id: Option<i32>,
    loop_subscribers: BTreeMap<u64, LoopSubscriber>,
    one_shot_requests: BTreeMap<u64, OneShotRequest>,
}

impl RafFrameRegistry {
    pub(crate) fn has_pending_frames(&self) -> bool {
        !self.one_shot_requests.is_empty()
            || self.loop_subscribers.values().any(|s| s.active.get())
    }

    pub(crate) fn scheduled_raf_id(&self) -> Option<i32> {
        self.scheduled_raf_id
    }

    pub(crate) fn set_scheduled_raf_id(&mut self, raf_id: Option<i32>) {
        self.scheduled_raf_id = raf_id;
    }

    #[cfg(test)]
    pub(crate) fn last_raf_ms(&self) -> Option<f64> {
        self.last_raf_ms
    }

    pub(crate) fn register_loop(
        &mut self,
        on_frame: impl FnMut(FrameInfo) + 'static,
    ) -> (u64, Rc<Cell<bool>>) {
        let id = self.next_id;
        self.next_id += 1;

        let active = Rc::new(Cell::new(true));
        let callback = Rc::new(RefCell::new(Box::new(on_frame) as Box<dyn FnMut(FrameInfo)>));

        self.loop_subscribers.insert(
            id,
            LoopSubscriber {
                active: active.clone(),
                callback,
            },
        );

        (id, active)
    }

    pub(crate) fn register_one_shot(
        &mut self,
        on_frame: impl FnOnce(FrameInfo) + 'static,
    ) -> (
        u64,
        Rc<Cell<bool>>,
        Rc<RefCell<Option<Box<dyn FnOnce(FrameInfo)>>>>,
    ) {
        let id = self.next_id;
        self.next_id += 1;

        let active = Rc::new(Cell::new(true));
        let callback = Rc::new(RefCell::new(Some(
            Box::new(on_frame) as Box<dyn FnOnce(FrameInfo)>
        )));

        self.one_shot_requests.insert(
            id,
            OneShotRequest {
                active: active.clone(),
                callback: callback.clone(),
            },
        );

        (id, active, callback)
    }

    pub(crate) fn cancel_loop(&mut self, id: u64) -> (Option<LoopSubscriber>, Option<i32>) {
        let removed = self.loop_subscribers.remove(&id);
        let cancel_raf_id = if removed.is_some() && !self.has_pending_frames() {
            self.last_raf_ms = None;
            self.scheduled_raf_id.take()
        } else {
            None
        };
        (removed, cancel_raf_id)
    }

    pub(crate) fn cancel_one_shot(&mut self, id: u64) -> (Option<OneShotRequest>, Option<i32>) {
        let removed = self.one_shot_requests.remove(&id);
        let cancel_raf_id = if removed.is_some() && !self.has_pending_frames() {
            self.last_raf_ms = None;
            self.scheduled_raf_id.take()
        } else {
            None
        };
        (removed, cancel_raf_id)
    }

    pub(crate) fn prepare_step(&mut self, timestamp_ms: f64) -> Option<PreparedRafStep> {
        self.scheduled_raf_id = None;
        if !self.has_pending_frames() {
            self.last_raf_ms = None;
            return None;
        }

        let dt_ms = match self.last_raf_ms {
            Some(prev) => (timestamp_ms - prev).clamp(0.0, 100.0),
            None => 16.666_667,
        };
        self.last_raf_ms = Some(timestamp_ms);

        let info = FrameInfo {
            now: Duration::from_secs_f64(timestamp_ms.max(0.0) / 1000.0),
            delta: Duration::from_secs_f64(dt_ms / 1000.0),
        };

        let one_shots = std::mem::take(&mut self.one_shot_requests);
        let loops = self
            .loop_subscribers
            .values()
            .filter(|sub| sub.active.get())
            .map(|sub| (sub.active.clone(), sub.callback.clone()))
            .collect();

        Some(PreparedRafStep {
            info,
            one_shots,
            loops,
        })
    }

    pub(crate) fn finish_step(&mut self) -> (Vec<LoopSubscriber>, Option<i32>) {
        let inactive_ids: Vec<u64> = self
            .loop_subscribers
            .iter()
            .filter_map(|(&id, sub)| (!sub.active.get()).then_some(id))
            .collect();
        let mut removed = Vec::with_capacity(inactive_ids.len());
        for id in inactive_ids {
            if let Some(sub) = self.loop_subscribers.remove(&id) {
                removed.push(sub);
            }
        }

        let cancel_raf_id = if !self.has_pending_frames() {
            self.last_raf_ms = None;
            self.scheduled_raf_id.take()
        } else {
            None
        };

        (removed, cancel_raf_id)
    }

    pub(crate) fn dispatch_step(registry: &RefCell<Self>, timestamp_ms: f64) -> Option<i32> {
        let prepared = registry.borrow_mut().prepare_step(timestamp_ms);
        let Some(prepared) = prepared else {
            return None;
        };

        for (_, req) in prepared.one_shots {
            if req.active.get() {
                let cb_opt = req.callback.borrow_mut().take();
                if let Some(cb) = cb_opt {
                    cb(prepared.info);
                }
            }
        }

        for (active, cb) in prepared.loops {
            if active.get() {
                cb.borrow_mut()(prepared.info);
            }
        }

        let (removed_loops, cancel_raf_id) = registry.borrow_mut().finish_step();
        drop(removed_loops);
        cancel_raf_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::types::{FrameLoopGuard, FrameRequestGuard};

    fn register_test_one_shot(
        registry: &Rc<RefCell<RafFrameRegistry>>,
        on_frame: impl FnOnce(FrameInfo) + 'static,
    ) -> FrameRequestGuard {
        let (id, active, callback) = registry.borrow_mut().register_one_shot(on_frame);
        let reg = registry.clone();
        FrameRequestGuard::new(move || {
            active.set(false);
            if let Ok(mut cb) = callback.try_borrow_mut() {
                let _ = cb.take();
            }
            let removed = reg
                .try_borrow_mut()
                .ok()
                .map(|mut r| r.cancel_one_shot(id));
            drop(removed);
        })
    }

    fn register_test_loop(
        registry: &Rc<RefCell<RafFrameRegistry>>,
        on_frame: impl FnMut(FrameInfo) + 'static,
    ) -> FrameLoopGuard {
        let (id, active) = registry.borrow_mut().register_loop(on_frame);
        let reg = registry.clone();
        FrameLoopGuard::new(move || {
            active.set(false);
            let removed = reg.try_borrow_mut().ok().map(|mut r| r.cancel_loop(id));
            drop(removed);
        })
    }

    #[test]
    fn concurrent_one_shot_waiters_receive_identical_delta_across_frames() {
        let registry = Rc::new(RefCell::new(RafFrameRegistry::default()));
        let frame1_records: Rc<RefCell<Vec<(usize, FrameInfo)>>> = Rc::new(RefCell::new(Vec::new()));
        let frame2_records: Rc<RefCell<Vec<(usize, FrameInfo)>>> = Rc::new(RefCell::new(Vec::new()));
        let frame2_guards: Rc<RefCell<Vec<FrameRequestGuard>>> = Rc::new(RefCell::new(Vec::new()));

        let mut frame1_guards = Vec::new();
        for idx in 0..4 {
            let reg = registry.clone();
            let f1 = frame1_records.clone();
            let f2 = frame2_records.clone();
            let g2 = frame2_guards.clone();
            frame1_guards.push(register_test_one_shot(&registry, move |info| {
                f1.borrow_mut().push((idx, info));
                let guard = register_test_one_shot(&reg, move |info2| {
                    f2.borrow_mut().push((idx, info2));
                });
                g2.borrow_mut().push(guard);
            }));
        }

        // Frame 1 at t1 = 1000.0ms: initial baseline ~16.666_667ms for ALL 4 waiters
        RafFrameRegistry::dispatch_step(&registry, 1000.0);

        let expected_first_delta = Duration::from_secs_f64(16.666_667 / 1000.0);
        let f1 = frame1_records.borrow();
        assert_eq!(f1.len(), 4);
        for (i, &(idx, info)) in f1.iter().enumerate() {
            assert_eq!(idx, i);
            assert_eq!(info.now, Duration::from_millis(1000));
            assert_eq!(
                info.delta, expected_first_delta,
                "waiter {idx} on frame 1 must receive exact initial baseline delta (never 0)"
            );
            assert_ne!(info.delta, Duration::ZERO);
        }
        drop(f1);

        // Frame 2 at t2 = 1024.0ms: exact t2 - t1 = 24.0ms for ALL 4 waiters
        RafFrameRegistry::dispatch_step(&registry, 1024.0);

        let expected_second_delta = Duration::from_secs_f64(24.0 / 1000.0);
        let f2 = frame2_records.borrow();
        assert_eq!(f2.len(), 4);
        for (i, &(idx, info)) in f2.iter().enumerate() {
            assert_eq!(idx, i);
            assert_eq!(info.now, Duration::from_millis(1024));
            assert_eq!(
                info.delta, expected_second_delta,
                "waiter {idx} on frame 2 must receive exact t2 - t1 delta (never 0)"
            );
            assert_ne!(info.delta, Duration::ZERO);
        }
    }

    #[test]
    fn idle_reset_clears_last_raf_ms_and_hitch_clamping_caps_at_100ms() {
        let registry = Rc::new(RefCell::new(RafFrameRegistry::default()));

        // 1. Single one-shot runs at t1 = 1000.0ms, then registry goes idle
        let seen = Rc::new(RefCell::new(Vec::new()));
        let s1 = seen.clone();
        let _g1 = register_test_one_shot(&registry, move |info| {
            s1.borrow_mut().push(info);
        });

        RafFrameRegistry::dispatch_step(&registry, 1000.0);
        assert!(!registry.borrow().has_pending_frames());
        assert_eq!(
            registry.borrow().last_raf_ms(),
            None,
            "entering idle after draining one-shots must reset last_raf_ms to None"
        );

        // After a 15-second idle gap (t2 = 16_000.0ms), next request must NOT see 15s or 100ms spike
        let s2 = seen.clone();
        let _g2 = register_test_one_shot(&registry, move |info| {
            s2.borrow_mut().push(info);
        });
        RafFrameRegistry::dispatch_step(&registry, 16_000.0);
        let baseline = Duration::from_secs_f64(16.666_667 / 1000.0);
        assert_eq!(seen.borrow()[1].delta, baseline);
        assert_eq!(seen.borrow()[1].now, Duration::from_millis(16_000));

        // 2. Active loop hitch clamping: 5s gap while active is clamped to 100ms
        let loop_deltas = Rc::new(RefCell::new(Vec::new()));
        let ld = loop_deltas.clone();
        let loop_guard = register_test_loop(&registry, move |info| {
            ld.borrow_mut().push(info.delta);
        });

        RafFrameRegistry::dispatch_step(&registry, 20_000.0); // initial frame -> baseline
        RafFrameRegistry::dispatch_step(&registry, 20_020.0); // normal frame -> 20ms
        RafFrameRegistry::dispatch_step(&registry, 25_020.0); // 5,000ms hitch -> clamped to 100ms
        RafFrameRegistry::dispatch_step(&registry, 25_010.0); // negative timestamp skew -> clamped to 0ms

        let recorded = loop_deltas.borrow().clone();
        assert_eq!(recorded.len(), 4);
        assert_eq!(recorded[0], baseline);
        assert_eq!(recorded[1], Duration::from_secs_f64(20.0 / 1000.0));
        assert_eq!(recorded[2], Duration::from_millis(100));
        assert_eq!(recorded[3], Duration::ZERO);

        drop(loop_guard);
        assert!(!registry.borrow().has_pending_frames());
        assert_eq!(registry.borrow().last_raf_ms(), None);
    }

    #[test]
    fn reentrant_request_next_frame_executes_on_next_frame_without_panic() {
        let registry = Rc::new(RefCell::new(RafFrameRegistry::default()));
        let events = Rc::new(RefCell::new(Vec::new()));
        let held_inner_guard = Rc::new(RefCell::new(None));

        let reg_clone = registry.clone();
        let events_outer = events.clone();
        let inner_slot = held_inner_guard.clone();

        let _outer_guard = register_test_one_shot(&registry, move |info1| {
            events_outer.borrow_mut().push(("outer", info1));
            let events_inner = events_outer.clone();
            let g = register_test_one_shot(&reg_clone, move |info2| {
                events_inner.borrow_mut().push(("inner", info2));
            });
            *inner_slot.borrow_mut() = Some(g);
        });

        // Frame 1 executes ONLY "outer"; "inner" is queued for Frame 2
        RafFrameRegistry::dispatch_step(&registry, 500.0);
        assert_eq!(events.borrow().len(), 1);
        assert_eq!(events.borrow()[0].0, "outer");
        assert!(registry.borrow().has_pending_frames());
        assert_eq!(registry.borrow().last_raf_ms(), Some(500.0));

        // Frame 2 executes "inner" with delta = 16ms
        RafFrameRegistry::dispatch_step(&registry, 516.0);
        assert_eq!(events.borrow().len(), 2);
        assert_eq!(events.borrow()[1].0, "inner");
        assert_eq!(events.borrow()[1].1.delta, Duration::from_secs_f64(16.0 / 1000.0));
        assert!(!registry.borrow().has_pending_frames());
        assert_eq!(registry.borrow().last_raf_ms(), None);
    }

    #[test]
    fn guard_cancellation_when_empty_resets_pending_and_last_raf_ms() {
        let registry = Rc::new(RefCell::new(RafFrameRegistry::default()));

        let loop_guard = register_test_loop(&registry, |_info| {});
        RafFrameRegistry::dispatch_step(&registry, 2000.0);
        assert!(registry.borrow().has_pending_frames());
        assert_eq!(registry.borrow().last_raf_ms(), Some(2000.0));

        registry.borrow_mut().set_scheduled_raf_id(Some(77));
        let one_shot_guard = register_test_one_shot(&registry, |_info| {});

        // Cancelling loop while one-shot is still pending keeps state active
        loop_guard.cancel();
        assert!(registry.borrow().has_pending_frames());
        assert_eq!(registry.borrow().last_raf_ms(), Some(2000.0));
        assert_eq!(registry.borrow().scheduled_raf_id(), Some(77));

        // Cancelling the last pending guard resets has_pending_frames, last_raf_ms, and scheduled_raf_id
        one_shot_guard.cancel();
        assert!(!registry.borrow().has_pending_frames());
        assert_eq!(registry.borrow().last_raf_ms(), None);
        assert_eq!(registry.borrow().scheduled_raf_id(), None);
    }
}
