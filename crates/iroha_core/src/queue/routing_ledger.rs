//! Shared routing registry for queued transactions.
//!
//! Transactions are routed when they enter the queue. The routing decision must
//! survive until the transaction leaves the pipeline so that downstream event
//! emitters (block processing, telemetry, APIs) can expose the correct
//! lane/dataspace metadata. This module provides a global registry keyed by the
//! transaction hash to keep those decisions alive across subsystem boundaries.
use super::router::{RoutingDecision, RoutingPlan};
use dashmap::DashMap;
use iroha_crypto::HashOf;
use iroha_data_model::transaction::SignedTransaction;
use parking_lot::Mutex;
use std::{
    collections::{HashMap, VecDeque},
    sync::LazyLock,
};
#[cfg(test)]
const DEFAULT_MAX_ENTRIES: usize = iroha_config::parameters::defaults::queue::CAPACITY.get();
static ROUTING_LEDGER: LazyLock<RoutingLedgerStore> = LazyLock::new(RoutingLedgerStore::new);
struct RoutingLedgerStore {
    decisions: DashMap<HashOf<SignedTransaction>, RoutingDecision>,
    plans: DashMap<HashOf<SignedTransaction>, RoutingPlan>,
    order: Mutex<RoutingOrder>,
}
#[derive(Default)]
struct RoutingOrder {
    entries: VecDeque<(HashOf<SignedTransaction>, u64)>,
    live_generations: HashMap<HashOf<SignedTransaction>, u64>,
    next_generation: u64,
}
impl RoutingOrder {
    fn insert(&mut self, hash: HashOf<SignedTransaction>) {
        if self.live_generations.contains_key(&hash) {
            return;
        }
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        let generation = self.next_generation;
        self.live_generations.insert(hash, generation);
        self.entries.push_back((hash, generation));
    }
    fn remove(&mut self, hash: &HashOf<SignedTransaction>) {
        self.live_generations.remove(hash);
    }
    fn is_live(&self, hash: &HashOf<SignedTransaction>, generation: u64) -> bool {
        self.live_generations.get(hash) == Some(&generation)
    }
    fn compact_if_needed(&mut self, max_entries: usize) {
        let compact_threshold = max_entries.saturating_mul(2).max(2);
        if self.entries.len() <= compact_threshold {
            return;
        }
        self.entries
            .retain(|(hash, generation)| self.live_generations.get(hash) == Some(generation));
    }
}
impl RoutingLedgerStore {
    fn new() -> Self {
        Self {
            decisions: DashMap::new(),
            plans: DashMap::new(),
            order: Mutex::new(RoutingOrder::default()),
        }
    }
    #[cfg(test)]
    fn record_bounded(
        &self,
        hash: HashOf<SignedTransaction>,
        decision: RoutingDecision,
        max_entries: usize,
    ) {
        let max_entries = max_entries.max(1);
        let mut order = self.order.lock();
        order.insert(hash);
        self.decisions.insert(hash, decision);
        self.evict_over_capacity(&mut order, max_entries, hash);
        order.compact_if_needed(max_entries);
    }
    fn record_plan_bounded(
        &self,
        hash: HashOf<SignedTransaction>,
        plan: RoutingPlan,
        max_entries: usize,
    ) {
        let max_entries = max_entries.max(1);
        let decision = plan.coordinator_route();
        let mut order = self.order.lock();
        order.insert(hash);
        self.plans.insert(hash, plan);
        self.decisions.insert(hash, decision);
        self.evict_over_capacity(&mut order, max_entries, hash);
        order.compact_if_needed(max_entries);
    }
    fn evict_over_capacity(
        &self,
        order: &mut RoutingOrder,
        max_entries: usize,
        protected_hash: HashOf<SignedTransaction>,
    ) {
        let mut inspected = 0usize;
        let max_inspect = order.entries.len();
        while (self.plans.len() > max_entries || self.decisions.len() > max_entries)
            && inspected < max_inspect
        {
            let Some((oldest, generation)) = order.entries.pop_front() else {
                break;
            };
            inspected = inspected.saturating_add(1);
            if !order.is_live(&oldest, generation) {
                continue;
            }
            if oldest == protected_hash {
                order.entries.push_back((oldest, generation));
                continue;
            }
            order.remove(&oldest);
            self.decisions.remove(&oldest);
            self.plans.remove(&oldest);
        }
    }
    fn take(&self, hash: &HashOf<SignedTransaction>) -> Option<RoutingDecision> {
        let mut order = self.order.lock();
        let removed = self.decisions.remove(hash).map(|(_, decision)| decision);
        if removed.is_some() && !self.plans.contains_key(hash) {
            order.remove(hash);
        }
        removed
    }
    fn take_plan(&self, hash: &HashOf<SignedTransaction>) -> Option<RoutingPlan> {
        let mut order = self.order.lock();
        let removed = self.plans.remove(hash).map(|(_, plan)| plan);
        if removed.is_some() && !self.decisions.contains_key(hash) {
            order.remove(hash);
        }
        removed
    }
    fn take_route(&self, hash: &HashOf<SignedTransaction>) -> Option<RoutingDecision> {
        let mut order = self.order.lock();
        if let Some((_, plan)) = self.plans.remove(hash) {
            let route = plan.coordinator_route();
            self.decisions.remove(hash);
            order.remove(hash);
            return Some(route);
        }
        let removed = self.decisions.remove(hash).map(|(_, decision)| decision);
        if removed.is_some() {
            order.remove(hash);
        }
        removed
    }
    #[cfg(test)]
    fn get(&self, hash: &HashOf<SignedTransaction>) -> Option<RoutingDecision> {
        self.decisions.get(hash).map(|entry| *entry.value())
    }
    fn get_plan(&self, hash: &HashOf<SignedTransaction>) -> Option<RoutingPlan> {
        self.plans.get(hash).map(|entry| entry.value().clone())
    }
    fn discard_if_matches(&self, hash: &HashOf<SignedTransaction>, expected: RoutingDecision) {
        let mut order = self.order.lock();
        if self
            .decisions
            .remove_if(hash, |_, current| *current == expected)
            .is_some()
            && !self.plans.contains_key(hash)
        {
            order.remove(hash);
        }
    }
    fn discard_plan_if_matches(&self, hash: &HashOf<SignedTransaction>, expected: &RoutingPlan) {
        let mut order = self.order.lock();
        let expected_digest = expected.digest();
        if self
            .plans
            .remove_if(hash, |_, current| current.digest() == expected_digest)
            .is_some()
        {
            self.decisions.remove(hash);
            order.remove(hash);
        }
    }
    #[cfg(test)]
    fn len_for_tests(&self) -> usize {
        self.decisions.len().max(self.plans.len())
    }
}
/// Store (or replace) the routing decision for the given transaction hash.
#[cfg(test)]
pub fn record(hash: HashOf<SignedTransaction>, decision: RoutingDecision) {
    record_bounded(hash, decision, DEFAULT_MAX_ENTRIES);
}
/// Store (or replace) the routing decision and evict stale records above `max_entries`.
#[cfg(test)]
pub fn record_bounded(
    hash: HashOf<SignedTransaction>,
    decision: RoutingDecision,
    max_entries: usize,
) {
    ROUTING_LEDGER.record_bounded(hash, decision, max_entries);
}
/// Store (or replace) the full routing plan and evict stale records above `max_entries`.
pub fn record_plan_bounded(hash: HashOf<SignedTransaction>, plan: RoutingPlan, max_entries: usize) {
    ROUTING_LEDGER.record_plan_bounded(hash, plan, max_entries);
}
/// Remove and return the routing decision for `hash`, if present.
pub fn take(hash: &HashOf<SignedTransaction>) -> Option<RoutingDecision> {
    ROUTING_LEDGER.take(hash)
}
/// Remove and return the full routing plan for `hash`, if present.
pub fn take_plan(hash: &HashOf<SignedTransaction>) -> Option<RoutingPlan> {
    ROUTING_LEDGER.take_plan(hash)
}
/// Remove and return the authoritative coordinator route for `hash`, if present.
///
/// Full routing plans are preferred over legacy single-route decisions because
/// they are the queue's digest-checked routing artifact. When a plan exists,
/// any shadow legacy decision for the same transaction is cleared as well.
pub fn take_route(hash: &HashOf<SignedTransaction>) -> Option<RoutingDecision> {
    ROUTING_LEDGER.take_route(hash)
}
/// Retrieve the routing decision for `hash` without removing it.
#[cfg(test)]
pub fn get(hash: &HashOf<SignedTransaction>) -> Option<RoutingDecision> {
    ROUTING_LEDGER.get(hash)
}
/// Retrieve the full routing plan for `hash` without removing it.
#[cfg_attr(not(feature = "telemetry"), allow(dead_code))]
pub fn get_plan(hash: &HashOf<SignedTransaction>) -> Option<RoutingPlan> {
    ROUTING_LEDGER.get_plan(hash)
}
/// Delete the routing decision for `hash` if it matches `expected`.
///
/// This is useful for cleanup paths where the queue can still observe the
/// cached decision (for example, when a transaction expires) but we do not want
/// to accidentally clear entries already consumed by downstream stages.
pub fn discard_if_matches(hash: &HashOf<SignedTransaction>, expected: RoutingDecision) {
    ROUTING_LEDGER.discard_if_matches(hash, expected);
}
/// Delete the routing plan for `hash` if it has the expected digest.
pub fn discard_plan_if_matches(hash: &HashOf<SignedTransaction>, expected: &RoutingPlan) {
    ROUTING_LEDGER.discard_plan_if_matches(hash, expected);
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::{DataSpaceId, nexus::LaneId, transaction::SignedTransaction};
    fn tx_hash(byte: u8) -> HashOf<SignedTransaction> {
        HashOf::from_untyped_unchecked(Hash::new([byte; Hash::LENGTH]))
    }
    fn plan(lane: u32) -> RoutingPlan {
        RoutingPlan::single(RoutingDecision::new(
            LaneId::new(lane),
            DataSpaceId::UNIVERSAL,
        ))
    }
    #[test]
    fn bounded_recording_evicts_oldest_entries() {
        let ledger = RoutingLedgerStore::new();
        let first = tx_hash(1);
        let second = tx_hash(2);
        let third = tx_hash(3);
        ledger.record_plan_bounded(first, plan(1), 2);
        ledger.record_plan_bounded(second, plan(2), 2);
        ledger.record_plan_bounded(third, plan(3), 2);
        assert_eq!(ledger.len_for_tests(), 2);
        assert!(
            ledger.get_plan(&first).is_none(),
            "oldest entry must be evicted"
        );
        assert_eq!(
            ledger
                .get_plan(&second)
                .map(|plan| plan.coordinator_route().lane_id),
            Some(LaneId::new(2))
        );
        assert_eq!(
            ledger
                .get_plan(&third)
                .map(|plan| plan.coordinator_route().lane_id),
            Some(LaneId::new(3))
        );
    }
    #[test]
    fn take_removes_ordered_entry_from_bounded_ledger() {
        let ledger = RoutingLedgerStore::new();
        let first = tx_hash(11);
        let second = tx_hash(12);
        let third = tx_hash(13);
        ledger.record_plan_bounded(first, plan(1), 2);
        ledger.record_plan_bounded(second, plan(2), 2);
        assert!(ledger.take_plan(&first).is_some());
        let _ = ledger.take(&first);
        ledger.record_plan_bounded(third, plan(3), 2);
        assert_eq!(ledger.len_for_tests(), 2);
        assert!(ledger.get_plan(&second).is_some());
        assert!(ledger.get_plan(&third).is_some());
    }
    #[test]
    fn take_route_prefers_full_plan_over_divergent_legacy_decision() {
        let ledger = RoutingLedgerStore::new();
        let hash = tx_hash(21);
        let plan = plan(7);
        let plan_route = plan.coordinator_route();
        let stale_decision = RoutingDecision::new(LaneId::new(99), DataSpaceId::new(99));
        ledger.record_plan_bounded(hash, plan, 2);
        ledger.decisions.insert(hash, stale_decision);
        assert_eq!(ledger.take_route(&hash), Some(plan_route));
        assert!(ledger.get_plan(&hash).is_none());
        assert!(ledger.get(&hash).is_none());
    }
    #[test]
    fn discard_plan_if_matches_removes_divergent_legacy_decision() {
        let ledger = RoutingLedgerStore::new();
        let hash = tx_hash(22);
        let plan = plan(8);
        let stale_decision = RoutingDecision::new(LaneId::new(77), DataSpaceId::new(77));
        ledger.record_plan_bounded(hash, plan.clone(), 2);
        ledger.decisions.insert(hash, stale_decision);
        ledger.discard_plan_if_matches(&hash, &plan);
        assert!(ledger.get_plan(&hash).is_none());
        assert!(ledger.get(&hash).is_none());
        assert_eq!(ledger.len_for_tests(), 0);
    }
    #[test]
    fn removals_are_lazy_but_order_storage_stays_bounded() {
        let ledger = RoutingLedgerStore::new();
        for byte in 1_u8..=64 {
            let hash = tx_hash(byte);
            ledger.record_plan_bounded(hash, plan(u32::from(byte)), 2);
            assert!(ledger.take_route(&hash).is_some());
        }
        assert_eq!(ledger.len_for_tests(), 0);
        assert!(
            ledger.order.lock().entries.len() <= 4,
            "amortized compaction must bound lazy FIFO tombstones"
        );
    }
    #[test]
    fn stale_order_generation_cannot_evict_reinserted_hash() {
        let ledger = RoutingLedgerStore::new();
        let reinserted = tx_hash(31);
        ledger.record_plan_bounded(reinserted, plan(1), 2);
        assert!(ledger.take_route(&reinserted).is_some());
        let genuinely_oldest = tx_hash(32);
        ledger.record_plan_bounded(genuinely_oldest, plan(2), 2);
        ledger.record_plan_bounded(reinserted, plan(3), 2);
        ledger.record_plan_bounded(tx_hash(33), plan(4), 2);
        assert!(
            ledger.get_plan(&genuinely_oldest).is_none(),
            "the genuinely oldest live generation should be evicted at capacity"
        );
        assert!(
            ledger.get_plan(&reinserted).is_some(),
            "a stale generation must not evict the reinserted live record"
        );
        assert!(ledger.get_plan(&tx_hash(33)).is_some());
    }
}
