//! Shared routing registry for queued transactions.
//!
//! Transactions are routed when they enter the queue. The routing decision must
//! survive until the transaction leaves the pipeline so that downstream event
//! emitters (block processing, telemetry, APIs) can expose the correct
//! lane/dataspace metadata. This module provides a global registry keyed by the
//! transaction hash to keep those decisions alive across subsystem boundaries.

use std::{collections::VecDeque, sync::LazyLock};

use dashmap::DashMap;
use iroha_crypto::HashOf;
use iroha_data_model::transaction::SignedTransaction;
use parking_lot::Mutex;

use super::router::{RoutingDecision, RoutingPlan};

#[cfg(test)]
const DEFAULT_MAX_ENTRIES: usize = iroha_config::parameters::defaults::queue::CAPACITY.get();

static ROUTING_LEDGER: LazyLock<RoutingLedgerStore> = LazyLock::new(RoutingLedgerStore::new);

struct RoutingLedgerStore {
    decisions: DashMap<HashOf<SignedTransaction>, RoutingDecision>,
    plans: DashMap<HashOf<SignedTransaction>, RoutingPlan>,
    order: Mutex<VecDeque<HashOf<SignedTransaction>>>,
}

impl RoutingLedgerStore {
    fn new() -> Self {
        Self {
            decisions: DashMap::new(),
            plans: DashMap::new(),
            order: Mutex::new(VecDeque::new()),
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
        let inserted = !self.decisions.contains_key(&hash) && !self.plans.contains_key(&hash);
        self.decisions.insert(hash, decision);
        let mut order = self.order.lock();
        if inserted {
            order.push_back(hash);
        }
        self.evict_over_capacity(&mut order, max_entries, hash);
    }

    fn record_plan_bounded(
        &self,
        hash: HashOf<SignedTransaction>,
        plan: RoutingPlan,
        max_entries: usize,
    ) {
        let max_entries = max_entries.max(1);
        let decision = plan.coordinator_route();
        let inserted = !self.decisions.contains_key(&hash) && !self.plans.contains_key(&hash);
        self.plans.insert(hash, plan);
        self.decisions.insert(hash, decision);
        let mut order = self.order.lock();
        if inserted {
            order.push_back(hash);
        }
        self.evict_over_capacity(&mut order, max_entries, hash);
    }

    fn evict_over_capacity(
        &self,
        order: &mut VecDeque<HashOf<SignedTransaction>>,
        max_entries: usize,
        protected_hash: HashOf<SignedTransaction>,
    ) {
        let mut inspected = 0usize;
        let max_inspect = order.len();
        while (self.plans.len() > max_entries || self.decisions.len() > max_entries)
            && inspected < max_inspect
        {
            let Some(oldest) = order.pop_front() else {
                break;
            };
            inspected = inspected.saturating_add(1);
            if oldest == protected_hash {
                order.push_back(oldest);
                continue;
            }
            self.decisions.remove(&oldest);
            self.plans.remove(&oldest);
        }
    }

    fn take(&self, hash: &HashOf<SignedTransaction>) -> Option<RoutingDecision> {
        let removed = self.decisions.remove(hash).map(|(_, decision)| decision);
        if removed.is_some() && !self.plans.contains_key(hash) {
            self.remove_from_order(hash);
        }
        removed
    }

    fn take_plan(&self, hash: &HashOf<SignedTransaction>) -> Option<RoutingPlan> {
        let removed = self.plans.remove(hash).map(|(_, plan)| plan);
        if removed.is_some() && !self.decisions.contains_key(hash) {
            self.remove_from_order(hash);
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
        if self
            .decisions
            .remove_if(hash, |_, current| *current == expected)
            .is_some()
            && !self.plans.contains_key(hash)
        {
            self.remove_from_order(hash);
        }
    }

    fn discard_plan_if_matches(&self, hash: &HashOf<SignedTransaction>, expected: &RoutingPlan) {
        let expected_digest = expected.digest();
        if self
            .plans
            .remove_if(hash, |_, current| current.digest() == expected_digest)
            .is_some()
            && !self.decisions.contains_key(hash)
        {
            self.remove_from_order(hash);
        }
        self.discard_if_matches(hash, expected.coordinator_route());
    }

    fn remove_from_order(&self, hash: &HashOf<SignedTransaction>) {
        let mut order = self.order.lock();
        order.retain(|entry| entry != hash);
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
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::{DataSpaceId, nexus::LaneId, transaction::SignedTransaction};

    use super::*;

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
}
