//! Bounded process records whose physical health outlives policy and borrowers.

use mv::allocation::{AllocationReservation, ChargedBuffer, ChargedShared};
use std::{
    alloc::Layout,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
    },
};

/// Nonblocking discovery pass admission with an explicit public clock input.
pub(super) struct DiscoveryGate(Mutex<Option<std::time::Instant>>);
impl DiscoveryGate {
    pub(super) const fn new() -> Self {
        Self(Mutex::new(None))
    }
    pub(super) fn try_enter(
        &self,
        now: std::time::Instant,
        retry: std::time::Duration,
    ) -> Option<std::sync::MutexGuard<'_, Option<std::time::Instant>>> {
        let mut last = match self.0.try_lock() {
            Ok(last) => last,
            Err(std::sync::TryLockError::WouldBlock) => return None,
            Err(std::sync::TryLockError::Poisoned(error)) => {
                // This scalar timestamp is assigned before callbacks. Every
                // value is valid; recovering it must preserve the cooldown.
                self.0.clear_poison();
                error.into_inner()
            }
        };
        if last.is_some_and(|prior| now.saturating_duration_since(prior) < retry) {
            return None;
        }
        *last = Some(now);
        Some(last)
    }
    pub(super) fn restart(&self) {
        let mut last = match self.0.try_lock() {
            Ok(last) => last,
            Err(std::sync::TryLockError::WouldBlock) => return,
            Err(std::sync::TryLockError::Poisoned(error)) => {
                self.0.clear_poison();
                error.into_inner()
            }
        };
        // This explicit restart intentionally clears only the retry timestamp;
        // poison recovery itself never resets a cursor or physical health.
        *last = None;
    }
}

/// Process-lived public scheduling position, independent of device health.
/// Only one new-work pass holds a family cursor; competing readers may still use
/// cached profiles. Neither an expired budget nor a busy cursor advances it.
pub(super) struct FairProgress(Mutex<usize>);
pub(super) struct FairPass<'a> {
    next: std::sync::MutexGuard<'a, usize>,
    count: usize,
    start: usize,
    started: std::time::Instant,
    budget: std::time::Duration,
}
impl FairProgress {
    pub(super) const fn new() -> Self {
        Self(Mutex::new(0))
    }
    pub(super) fn try_pass(
        &self,
        count: usize,
        started: std::time::Instant,
        budget: std::time::Duration,
    ) -> Option<FairPass<'_>> {
        if count == 0 {
            return None;
        }
        let next = match self.0.try_lock() {
            Ok(next) => next,
            Err(std::sync::TryLockError::WouldBlock) => return None,
            Err(std::sync::TryLockError::Poisoned(error)) => {
                // begin_attempt stores the next scalar index before callbacks.
                // Keep that progress after a caught unwind; coupled registry,
                // profile and physical-health state is never recovered here.
                self.0.clear_poison();
                error.into_inner()
            }
        };
        let start = *next % count;
        Some(FairPass {
            next,
            count,
            start,
            started,
            budget,
        })
    }
}
impl FairPass<'_> {
    pub(super) fn start(&self) -> usize {
        self.start
    }
    /// Record progress before native work, including slow failure or unwinding.
    /// An unvisited candidate receives no attempt and no retry penalty.
    pub(super) fn begin_attempt(
        &mut self,
        index: usize,
        now: std::time::Instant,
    ) -> Option<std::time::Instant> {
        if index >= self.count || now.saturating_duration_since(self.started) >= self.budget {
            return None;
        }
        *self.next = if index + 1 == self.count {
            0
        } else {
            index + 1
        };
        Some(self.started)
    }
}

/// Physical health is independent of configuration epochs and pipeline handles.
pub(super) struct DeviceHealth {
    identity: u64,
    quarantined: AtomicBool,
    uncertain: AtomicBool,
    completions: [AtomicU64; super::MetalKernel::ALL.len()],
}
impl DeviceHealth {
    pub(super) fn identity(&self) -> u64 {
        self.identity
    }
    pub(super) fn record_completion(&self, kernel: usize) {
        if let Some(count) = self.completions.get(kernel) {
            let _ = count.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
                Some(n.saturating_add(1))
            });
        }
    }
    #[cfg(test)]
    pub(super) fn completions(&self, kernel: usize) -> u64 {
        self.completions
            .get(kernel)
            .map_or(0, |n| n.load(Ordering::Relaxed))
    }

    pub(super) fn usable(&self) -> bool {
        !self.quarantined.load(Ordering::Acquire)
    }
    pub(super) fn quarantine(&self, uncertain: bool) {
        if uncertain {
            self.uncertain.store(true, Ordering::Release);
        }
        self.quarantined.store(true, Ordering::Release);
    }
}

pub(super) type HealthLease = ChargedShared<DeviceHealth>;
pub(super) type DeviceLease<T> = ChargedShared<DeviceRecord<T>>;

pub(super) struct DeviceRecord<T> {
    health: HealthLease,
    phase: AtomicU8,
    value: OnceLock<T>,
}
impl<T> DeviceRecord<T> {
    pub(super) fn health(&self) -> HealthLease {
        self.health.clone()
    }
    pub(super) fn value(&self) -> Option<&T> {
        (self.phase.load(Ordering::Acquire) == 2 && self.health.usable())
            .then(|| self.value.get())
            .flatten()
    }
    /// One initializer owns native setup; concurrent and recursive callers decline.
    pub(super) fn initialize(&self, initialize: impl FnOnce() -> Option<T>) -> bool {
        if !self.health.usable() {
            return false;
        }
        if self.phase.load(Ordering::Acquire) == 2 {
            return true;
        }
        if self
            .phase
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return false;
        }
        struct Guard<'a>(&'a DeviceHealth, &'a AtomicU8, bool);
        impl Drop for Guard<'_> {
            fn drop(&mut self) {
                if !self.2 {
                    self.0.quarantine(false);
                    self.1.store(3, Ordering::Release);
                }
            }
        }
        let mut guard = Guard(&self.health, &self.phase, false);
        let Some(value) = initialize() else {
            self.phase
                .store(if self.health.usable() { 0 } else { 3 }, Ordering::Release);
            guard.2 = true;
            return false;
        };
        if !self.health.usable() || self.value.set(value).is_err() {
            return false;
        }
        self.phase.store(2, Ordering::Release);
        guard.2 = true;
        self.health.usable()
    }
}

/// Records are charged before construction and never discarded to clear health.
pub(super) struct DeviceRegistry<T> {
    records: OnceLock<Mutex<ChargedBuffer<DeviceLease<T>>>>,
}
impl<T> DeviceRegistry<T> {
    pub(super) const fn new() -> Self {
        Self {
            records: OnceLock::new(),
        }
    }
    pub(super) fn prepare(
        &self,
        capacity: usize,
        reserve: impl FnOnce(Layout) -> Option<AllocationReservation>,
    ) -> bool {
        if self.records.get().is_some() {
            return true;
        }
        if capacity == 0 {
            return false;
        }
        let Ok(layout) = Layout::array::<DeviceLease<T>>(capacity) else {
            return false;
        };
        let Some(mut credit) = reserve(layout) else {
            return false;
        };
        let Ok(records) = ChargedBuffer::from_reservation(capacity, &mut credit) else {
            return false;
        };
        // A concurrent winner remains authoritative; loser allocations refund
        // outside every registry lock and may invoke reentrant capacity wakes.
        match self.records.set(Mutex::new(records)) {
            Ok(()) => true,
            Err(unpublished) => {
                drop(unpublished);
                true
            }
        }
    }
    /// Repeated or reordered discovery always returns the original physical record.
    pub(super) fn observe(
        &self,
        identity: u64,
        cap: usize,
        reserve: impl FnOnce(usize) -> Option<AllocationReservation>,
    ) -> Option<DeviceLease<T>> {
        {
            let records = self.records.get()?.try_lock().ok()?;
            if let Some(index) = records
                .as_slice()
                .iter()
                .position(|r| r.health.identity == identity)
            {
                return (index < cap).then(|| records.as_slice()[index].clone());
            }
            if records.as_slice().len() >= cap.min(records.capacity()) {
                return None;
            }
        }
        // Admission and construction can refund on failure. No registry writer
        // remains held while acquiring credit, allocating or dropping a loser.
        let bytes = ChargedShared::<DeviceHealth>::allocation_layout()
            .size()
            .checked_add(ChargedShared::<DeviceRecord<T>>::allocation_layout().size())?;
        let mut credit = reserve(bytes)?;
        let health = ChargedShared::from_reservation(
            DeviceHealth {
                identity,
                quarantined: AtomicBool::new(false),
                uncertain: AtomicBool::new(false),
                completions: std::array::from_fn(|_| AtomicU64::new(0)),
            },
            &mut credit,
        )
        .ok()?;
        let record = ChargedShared::from_reservation(
            DeviceRecord {
                health,
                phase: AtomicU8::new(0),
                value: OnceLock::new(),
            },
            &mut credit,
        )
        .ok()?;
        let selected = {
            let mut records = self.records.get()?.try_lock().ok()?;
            if let Some(index) = records
                .as_slice()
                .iter()
                .position(|r| r.health.identity == identity)
            {
                (index < cap).then(|| records.as_slice()[index].clone())
            } else if records.as_slice().len() < cap.min(records.capacity()) {
                records.push_reserved(record.clone());
                Some(record.clone())
            } else {
                None
            }
        };
        drop(record);
        selected
    }
    pub(super) fn record(&self, index: usize, cap: usize) -> Option<DeviceLease<T>> {
        if index >= cap {
            return None;
        }
        self.records
            .get()?
            .try_lock()
            .ok()?
            .as_slice()
            .get(index)
            .cloned()
    }
    /// The operator cap counts healthy initialized/in-progress devices, while
    /// physical records (including failed identities) retain separate inventory credit.
    pub(super) fn eligible(
        &self,
        lease: &DeviceLease<T>,
        physical_cap: usize,
        active_cap: usize,
    ) -> bool {
        if active_cap == 0 || !lease.health.usable() {
            return false;
        }
        let Some(records) = self.records.get().and_then(|r| r.try_lock().ok()) else {
            return false;
        };
        let mut running = 0usize;
        let mut pending_target = false;
        for record in records.as_slice().iter().take(physical_cap) {
            if !record.health.usable() {
                continue;
            }
            let initialized = matches!(record.phase.load(Ordering::Acquire), 1 | 2);
            if ChargedShared::ptr_eq(record, lease) {
                if initialized {
                    return running < active_cap;
                }
                pending_target = true;
            }
            if initialized {
                running += 1;
            }
        }
        pending_target && running < active_cap
    }
    /// Continue the bounded production discovery schedule from its retained cursor.
    pub(super) fn qualify_fair(
        &self,
        pass: &mut FairPass<'_>,
        physical_cap: usize,
        active_cap: usize,
        now: impl Fn() -> std::time::Instant,
        mut initialize: impl FnMut(&DeviceLease<T>) -> Option<T>,
    ) {
        for offset in 0..pass.count {
            let index = (pass.start() + offset) % pass.count;
            let Some(record) = self.record(index, physical_cap) else {
                continue;
            };
            if record.value().is_some() || !self.eligible(&record, physical_cap, active_cap) {
                continue;
            }
            if pass.begin_attempt(index, now()).is_none() {
                break;
            }
            record.initialize(|| initialize(&record));
        }
    }

    /// Only a complete bounded inventory can establish physical disappearance.
    /// Health remains sticky if the same registry identity later reappears.
    pub(super) fn reconcile_presence(
        &self,
        physical_cap: usize,
        complete: bool,
        present: impl Fn(u64) -> bool,
    ) {
        if !complete {
            return;
        }
        for index in 0..self.len().min(physical_cap) {
            let Some(record) = self.record(index, physical_cap) else {
                continue;
            };
            if !present(record.health.identity) {
                record.health.quarantine(false);
            }
        }
    }
    pub(super) fn select_costed(
        &self,
        physical_cap: usize,
        active_cap: usize,
        start: usize,
        mut score: impl FnMut(usize, &DeviceLease<T>, &T) -> Option<u64>,
    ) -> Option<DeviceLease<T>> {
        let mut best: Option<(u64, u64, DeviceLease<T>)> = None;
        let count = self.len().min(physical_cap);
        for offset in 0..count {
            let index = (start % count + offset) % count;
            let Some(lease) = self.record(index, physical_cap) else {
                continue;
            };
            if !self.eligible(&lease, physical_cap, active_cap) {
                continue;
            }
            let Some(value) = lease.value() else {
                continue;
            };
            let Some(cost) = score(index, &lease, value) else {
                continue;
            };
            if !self.eligible(&lease, physical_cap, active_cap) {
                continue;
            }
            let identity = lease.health.identity;
            if best
                .as_ref()
                .is_none_or(|(prior, id, _)| (cost, identity) < (*prior, *id))
            {
                best = Some((cost, identity, lease));
            }
        }
        best.map(|(_, _, lease)| lease)
    }
    pub(super) fn all_quarantined(&self, cap: usize) -> bool {
        let Some(records) = self.records.get().and_then(|r| r.try_lock().ok()) else {
            return false;
        };
        let count = records.as_slice().len().min(cap);
        count > 0
            && records.as_slice()[..count]
                .iter()
                .all(|record| !record.health.usable())
    }
    pub(super) fn len(&self) -> usize {
        self.records
            .get()
            .and_then(|r| r.try_lock().ok())
            .map_or(0, |r| r.as_slice().len())
    }
}

/// Pin physical state and health together, restoring nested/unwinding callers.
pub(super) fn with_device_binding<T, R>(
    state: &std::cell::RefCell<Option<DeviceLease<T>>>,
    health: &std::cell::RefCell<Option<HealthLease>>,
    lease: &DeviceLease<T>,
    call: impl FnOnce() -> R,
) -> R {
    struct Restore<'a, T> {
        state: &'a std::cell::RefCell<Option<DeviceLease<T>>>,
        health: &'a std::cell::RefCell<Option<HealthLease>>,
        old_state: Option<DeviceLease<T>>,
        old_health: Option<HealthLease>,
    }
    impl<T> Drop for Restore<'_, T> {
        fn drop(&mut self) {
            self.state.replace(self.old_state.take());
            self.health.replace(self.old_health.take());
        }
    }
    let _restore = Restore {
        state,
        health,
        old_state: state.replace(Some(lease.clone())),
        old_health: health.replace(Some(lease.health())),
    };
    call()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mv::allocation::AllocationBudget;
    fn setup() -> (DeviceRegistry<u32>, AllocationBudget) {
        let registry = DeviceRegistry::new();
        let budget = AllocationBudget::new(16384);
        assert!(registry.prepare(3, |layout| budget.try_reserve(layout).ok()));
        (registry, budget)
    }
    #[test]
    fn discovery_retry_and_concurrent_passes_are_bounded_without_health_reset() {
        let gate = DiscoveryGate::new();
        let now = std::time::Instant::now();
        let retry = std::time::Duration::from_secs(30);
        let pass = gate.try_enter(now, retry).unwrap();
        assert!(
            gate.try_enter(now + retry, retry).is_none(),
            "recursive pass must decline"
        );
        std::thread::scope(|scope| {
            let gate = &gate;
            for _ in 0..8 {
                scope.spawn(move || assert!(gate.try_enter(now + retry, retry).is_none()));
            }
        });
        gate.restart(); // A restart cannot steal an in-progress pass.
        drop(pass);
        assert!(gate.try_enter(now + retry / 2, retry).is_none());
        drop(
            gate.try_enter(now + retry, retry)
                .expect("retry becomes available"),
        );
        gate.restart();
        assert!(gate.try_enter(now + retry, retry).is_some());
    }

    #[test]
    fn repeated_slow_unquarantined_first_failure_cannot_starve_discovery_at_cap_one() {
        use std::{
            cell::Cell,
            time::{Duration, Instant},
        };
        let (registry, budget) = setup();
        let first = registry
            .observe(11, 3, |n| budget.try_reserve_bytes(n).ok())
            .unwrap();
        let second = registry
            .observe(22, 3, |n| budget.try_reserve_bytes(n).ok())
            .unwrap();
        let progress = FairProgress::new();
        let base = Instant::now();
        let window = Duration::from_secs(1);
        let slow = Cell::new(0);
        let later = Cell::new(0);
        for turn in 0..3 {
            let now = base + Duration::from_secs(turn * 30);
            let clock = Cell::new(now);
            let mut pass = progress.try_pass(2, now, window).unwrap();
            registry.qualify_fair(
                &mut pass,
                3,
                1,
                || clock.get(),
                |record| {
                    if record.health().identity() == 11 {
                        slow.set(slow.get() + 1);
                        clock.set(clock.get() + window);
                        None
                    } else {
                        later.set(later.get() + 1);
                        (later.get() >= 2).then_some(9)
                    }
                },
            );
        }
        assert_eq!(
            slow.get(),
            2,
            "first device repeatedly consumes the entire pass"
        );
        assert_eq!(
            later.get(),
            2,
            "later device keeps its next turn and eventually qualifies"
        );
        assert!(
            first.health().usable(),
            "transient failure was never relabelled quarantine"
        );
        assert_eq!(second.value(), Some(&9));
        assert_eq!(
            registry
                .select_costed(3, 1, 0, |_, _, n| Some(u64::from(*n)))
                .unwrap()
                .health()
                .identity(),
            22
        );
    }

    #[test]
    fn fair_cursor_advances_only_for_attempts_and_survives_busy_expired_and_resized_passes() {
        use std::time::{Duration, Instant};
        let progress = FairProgress::new();
        let now = Instant::now();
        let budget = Duration::from_secs(1);
        assert!(progress.try_pass(0, now, budget).is_none());
        {
            let mut pass = progress.try_pass(3, now, budget).unwrap();
            assert_eq!(pass.start(), 0);
            assert!(progress.try_pass(3, now, budget).is_none());
            assert!(pass.begin_attempt(0, now).is_some());
            assert!(pass.begin_attempt(1, now + budget).is_none());
        }
        {
            let pass = progress.try_pass(3, now + budget, budget).unwrap();
            assert_eq!(pass.start(), 1);
        }
        {
            let mut pass = progress.try_pass(1, now + budget, budget).unwrap();
            assert_eq!(pass.start(), 0);
            assert!(pass.begin_attempt(0, now + budget).is_some());
        }
        assert_eq!(
            progress.try_pass(4, now + budget, budget).unwrap().start(),
            0
        );
    }

    #[test]
    fn discovery_scheduler_unwind_preserves_cooldown_cursor_and_later_device_progress() {
        use std::{
            panic::{AssertUnwindSafe, catch_unwind},
            time::{Duration, Instant},
        };
        let (registry, budget) = setup();
        let first = registry
            .observe(11, 3, |n| budget.try_reserve_bytes(n).ok())
            .unwrap();
        let second = registry
            .observe(22, 3, |n| budget.try_reserve_bytes(n).ok())
            .unwrap();
        let gate = DiscoveryGate::new();
        let progress = FairProgress::new();
        let now = Instant::now();
        let retry = Duration::from_secs(30);
        let window = Duration::from_secs(1);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let _discovery = gate.try_enter(now, retry).unwrap();
                let mut pass = progress.try_pass(2, now, window).unwrap();
                registry.qualify_fair(
                    &mut pass,
                    3,
                    1,
                    || now,
                    |record| {
                        assert_eq!(record.health().identity(), 11);
                        panic!("initializer unwinds through both actual scheduling guards")
                    },
                );
            }))
            .is_err()
        );
        assert!(gate.0.is_poisoned());
        assert!(progress.0.is_poisoned());
        assert!(!first.health().usable());
        assert!(second.health().usable());
        assert!(
            gate.try_enter(now + retry / 2, retry).is_none(),
            "recovery preserves prior cooldown"
        );
        let later = now + retry;
        {
            let _discovery = gate.try_enter(later, retry).expect("scalar gate recovered");
            assert!(
                gate.try_enter(later + retry, retry).is_none(),
                "WouldBlock remains nonblocking"
            );
            let mut pass = progress
                .try_pass(2, later, window)
                .expect("scalar cursor recovered");
            assert_eq!(
                pass.start(),
                1,
                "unwind does not reset progress to first device"
            );
            assert!(
                progress.try_pass(2, later, window).is_none(),
                "active recovered cursor still declines contenders"
            );
            registry.qualify_fair(
                &mut pass,
                3,
                1,
                || later,
                |record| {
                    assert_eq!(record.health().identity(), 22);
                    Some(9)
                },
            );
        }
        assert_eq!(second.value(), Some(&9));
        assert!(!first.health().usable());
        assert!(gate.try_enter(later + retry / 2, retry).is_none());
        // Explicit restart also recovers its scalar guard without changing health.
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let _discovery = gate.try_enter(later + retry, retry).unwrap();
                panic!("restart control")
            }))
            .is_err()
        );
        gate.restart();
        assert!(gate.try_enter(later + retry, retry).is_some());
        assert!(!first.health().usable());
        assert_eq!(second.value(), Some(&9));
    }

    #[test]
    fn independent_records_preserve_quarantine_and_original_identity() {
        let (registry, budget) = setup();
        let first = registry
            .observe(11, 3, |n| budget.try_reserve_bytes(n).ok())
            .unwrap();
        assert!(!first.initialize(|| {
            first.health().quarantine(false);
            None
        }));
        let second = registry
            .observe(22, 3, |n| budget.try_reserve_bytes(n).ok())
            .unwrap();
        assert!(second.initialize(|| Some(9)));
        assert_eq!(second.value(), Some(&9));
        let rediscovered = registry
            .observe(11, 3, |_| panic!("existing identity needs no new credit"))
            .unwrap();
        assert!(ChargedShared::ptr_eq(&first, &rediscovered));
        assert!(!rediscovered.initialize(|| panic!("quarantine cannot be requalified")));
        assert!(second.health().usable());
        assert!(registry.all_quarantined(1));
        assert!(!registry.all_quarantined(2));
        assert!(!registry.all_quarantined(0));
        assert_eq!(second.health().identity(), 22);
        second.health().record_completion(0);
        assert_eq!(second.health().completions(0), 1);
        assert_eq!(first.health().completions(0), 0);
    }
    #[test]
    fn caps_and_pressure_preserve_borrowers_and_final_owner_credit() {
        let (registry, budget) = setup();
        let first = registry
            .observe(11, 1, |n| budget.try_reserve_bytes(n).ok())
            .unwrap();
        assert!(first.initialize(|| Some(7)));
        let charged = budget.reserved_bytes();
        assert!(
            registry
                .observe(22, 1, |_| panic!("cap admission precedes allocation"))
                .is_none()
        );
        assert!(registry.record(0, 0).is_none());
        assert_eq!(first.value(), Some(&7));
        budget.set_limit_bytes(0);
        assert!(
            registry
                .observe(22, 3, |n| budget.try_reserve_bytes(n).ok())
                .is_none()
        );
        assert_eq!(budget.reserved_bytes(), charged);
        drop(registry);
        assert!(budget.reserved_bytes() > 0);
        assert_eq!(first.value(), Some(&7));
        drop(first);
        assert_eq!(budget.reserved_bytes(), 0);
    }
    #[test]
    fn reentrant_and_concurrent_initialization_never_wait_or_publish_partial_state() {
        let (registry, budget) = setup();
        let record = registry
            .observe(11, 3, |n| budget.try_reserve_bytes(n).ok())
            .unwrap();
        assert!(record.initialize(|| {
            assert!(!record.initialize(|| panic!("reentry")));
            assert!(record.value().is_none());
            Some(8)
        }));
        std::thread::scope(|s| {
            for _ in 0..8 {
                let record = record.clone();
                s.spawn(move || {
                    assert!(record.initialize(|| panic!("already initialized")));
                    assert_eq!(record.value(), Some(&8));
                });
            }
        });
    }
    #[test]
    fn concurrent_unfinished_initialization_declines_without_waiting() {
        let (registry, budget) = setup();
        let record = registry
            .observe(11, 3, |n| budget.try_reserve_bytes(n).ok())
            .unwrap();
        let (entered_tx, entered_rx) = std::sync::mpsc::sync_channel(0);
        let (release_tx, release_rx) = std::sync::mpsc::sync_channel(0);
        std::thread::scope(|scope| {
            let copy = record.clone();
            let initializer = scope.spawn(move || {
                copy.initialize(|| {
                    entered_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                    Some(9)
                })
            });
            entered_rx.recv().unwrap();
            assert!(!record.initialize(|| panic!("another initializer is active")));
            assert!(record.value().is_none());
            release_tx.send(()).unwrap();
            assert!(initializer.join().unwrap());
        });
        assert_eq!(record.value(), Some(&9));
    }

    #[test]
    fn initializer_unwind_quarantines_only_its_original_record() {
        let (registry, budget) = setup();
        let record = registry
            .observe(11, 3, |n| budget.try_reserve_bytes(n).ok())
            .unwrap();
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                || record.initialize(|| panic!("failure"))
            ))
            .is_err()
        );
        assert!(!record.health().usable());
        let other = registry
            .observe(22, 3, |n| budget.try_reserve_bytes(n).ok())
            .unwrap();
        assert!(other.initialize(|| Some(5)));
    }
    #[test]
    fn quarantine_during_initialization_wins_and_uncertainty_is_sticky() {
        let (registry, budget) = setup();
        let record = registry
            .observe(11, 3, |n| budget.try_reserve_bytes(n).ok())
            .unwrap();
        assert!(!record.initialize(|| {
            record.health().quarantine(true);
            Some(8)
        }));
        assert!(record.value().is_none());
        assert!(record.health.uncertain.load(Ordering::Acquire));
        record.health().quarantine(false);
        assert!(record.health.uncertain.load(Ordering::Acquire));
        assert_eq!(registry.len(), 1);
    }
    #[test]
    fn public_cost_selection_keeps_distinct_devices_caps_and_quarantine() {
        let (registry, budget) = setup();
        let a = registry
            .observe(22, 3, |n| budget.try_reserve_bytes(n).ok())
            .unwrap();
        let b = registry
            .observe(11, 3, |n| budget.try_reserve_bytes(n).ok())
            .unwrap();
        assert!(a.initialize(|| Some(100)));
        assert!(b.initialize(|| Some(20)));
        let fast = registry
            .select_costed(3, 3, 0, |_, _, cost| Some(u64::from(*cost)))
            .unwrap();
        assert_eq!(fast.health().identity(), 11);
        let tied = registry.select_costed(3, 3, 0, |_, _, _| Some(9)).unwrap();
        assert_eq!(tied.health().identity(), 11);
        assert!(
            registry
                .select_costed(3, 0, 0, |_, _, _| panic!(
                    "zero cap must not measure or dispatch"
                ))
                .is_none()
        );
        assert!(
            registry
                .observe(11, 1, |_| panic!("cap shrink cannot allocate"))
                .is_none()
        );
        let capped = registry
            .select_costed(3, 1, 0, |_, _, cost| Some(u64::from(*cost)))
            .unwrap();
        assert_eq!(capped.health().identity(), 22);
        b.health().quarantine(false);
        let fallback = registry
            .select_costed(3, 3, 0, |_, _, cost| Some(u64::from(*cost)))
            .unwrap();
        assert_eq!(fallback.health().identity(), 22);
        assert!(
            registry
                .select_costed(3, 3, 0, |_, record, _| {
                    record.health().quarantine(false);
                    Some(1)
                })
                .is_none()
        );
    }
    #[test]
    fn failed_first_device_does_not_consume_the_only_active_slot() {
        let (registry, budget) = setup();
        let first = registry
            .observe(11, 3, |n| budget.try_reserve_bytes(n).ok())
            .unwrap();
        let second = registry
            .observe(22, 3, |n| budget.try_reserve_bytes(n).ok())
            .unwrap();
        assert!(registry.eligible(&first, 3, 1));
        assert!(!first.initialize(|| {
            first.health().quarantine(false);
            None
        }));
        assert!(registry.eligible(&second, 3, 1));
        assert!(second.initialize(|| Some(9)));
        let selected = registry
            .select_costed(3, 1, 0, |_, _, n| Some(u64::from(*n)))
            .unwrap();
        assert_eq!(selected.health().identity(), 22);
        let third = registry
            .observe(33, 3, |n| budget.try_reserve_bytes(n).ok())
            .unwrap();
        assert!(!registry.eligible(&third, 3, 1));
        second.health().quarantine(true);
        assert!(registry.eligible(&third, 3, 1));
        assert!(third.initialize(|| Some(7)));
        assert_eq!(
            registry
                .select_costed(3, 1, 0, |_, _, n| Some(u64::from(*n)))
                .unwrap()
                .health()
                .identity(),
            33
        );
        assert!(!registry.eligible(&third, 3, 0));
        assert!(!registry.eligible(&first, 3, 3));
    }

    #[test]
    fn complete_discovery_releases_absent_device_slot_without_forgetting_health() {
        let (registry, budget) = setup();
        let first = registry
            .observe(11, 3, |n| budget.try_reserve_bytes(n).ok())
            .unwrap();
        let second = registry
            .observe(22, 3, |n| budget.try_reserve_bytes(n).ok())
            .unwrap();
        assert!(first.initialize(|| Some(10)));
        assert!(!registry.eligible(&second, 3, 1));
        registry.reconcile_presence(3, false, |_| {
            panic!("partial inventory cannot establish absence")
        });
        assert!(first.health().usable());
        registry.reconcile_presence(3, true, |id| {
            assert!(
                registry.records.get().unwrap().try_lock().is_ok(),
                "inventory callback runs outside registry writer"
            );
            id == 22
        });
        assert!(!first.health().usable());
        assert!(registry.eligible(&second, 3, 1));
        assert!(second.initialize(|| Some(20)));
        assert_eq!(
            registry
                .select_costed(3, 1, 0, |_, _, n| Some(u64::from(*n)))
                .unwrap()
                .health()
                .identity(),
            22
        );
        registry.reconcile_presence(3, true, |_| true);
        let returned = registry
            .observe(11, 3, |_| {
                panic!("returning physical identity retains original owner")
            })
            .unwrap();
        assert!(ChargedShared::ptr_eq(&first, &returned));
        assert!(!returned.health().usable());
    }

    #[test]
    fn losing_record_refunds_after_registry_unlock_and_reentrant_observation() {
        use std::{
            future::Future,
            pin::Pin,
            sync::Arc,
            task::{Context, Wake, Waker},
        };
        struct Reenter {
            registry: Arc<DeviceRegistry<u32>>,
            calls: AtomicU64,
        }
        impl Wake for Reenter {
            fn wake(self: Arc<Self>) {
                let guard = self
                    .registry
                    .records
                    .get()
                    .unwrap()
                    .try_lock()
                    .expect("refund runs outside registry writer");
                assert_eq!(guard.as_slice().len(), 1);
                drop(guard);
                assert!(self.registry.record(0, 3).is_some());
                self.calls.fetch_add(1, Ordering::SeqCst);
            }
        }
        let (registry, budget) = setup();
        let registry = Arc::new(registry);
        let wake = Arc::new(Reenter {
            registry: registry.clone(),
            calls: AtomicU64::new(0),
        });
        let waker = Waker::from(wake.clone());
        let mut waiter = None;
        let record = registry
            .observe(11, 3, |bytes| {
                let winner = registry
                    .observe(11, 3, |n| budget.try_reserve_bytes(n).ok())
                    .unwrap();
                assert!(winner.initialize(|| Some(9)));
                let mv::allocation::AllocationRefusal::Capacity { release, .. } =
                    budget.try_reserve_bytes(budget.limit_bytes()).unwrap_err()
                else {
                    panic!("occupied pool")
                };
                let mut future = release.wait_for_release();
                assert!(
                    Pin::new(&mut future)
                        .poll(&mut Context::from_waker(&waker))
                        .is_pending()
                );
                waiter = Some(future);
                budget.try_reserve_bytes(bytes).ok()
            })
            .unwrap();
        assert_eq!(record.value(), Some(&9));
        assert_eq!(registry.len(), 1);
        assert!(wake.calls.load(Ordering::SeqCst) > 0);
        drop(waiter);
    }

    #[test]
    fn selected_state_and_health_restore_together_after_nested_unwind() {
        let (registry, budget) = setup();
        let a = registry
            .observe(11, 3, |n| budget.try_reserve_bytes(n).ok())
            .unwrap();
        let b = registry
            .observe(22, 3, |n| budget.try_reserve_bytes(n).ok())
            .unwrap();
        let state = std::cell::RefCell::new(None);
        let health = std::cell::RefCell::new(None);
        with_device_binding(&state, &health, &a, || {
            assert_eq!(health.borrow().as_ref().unwrap().identity(), 11);
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| with_device_binding(
                    &state,
                    &health,
                    &b,
                    || {
                        assert_eq!(health.borrow().as_ref().unwrap().identity(), 22);
                        panic!("nested")
                    }
                )))
                .is_err()
            );
            assert!(ChargedShared::ptr_eq(state.borrow().as_ref().unwrap(), &a));
            assert_eq!(health.borrow().as_ref().unwrap().identity(), 11);
        });
        assert!(state.borrow().is_none());
        assert!(health.borrow().is_none());
    }
}
