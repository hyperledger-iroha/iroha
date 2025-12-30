//! Shared routing registry for queued transactions.
//!
//! Transactions are routed when they enter the queue. The routing decision must
//! survive until the transaction leaves the pipeline so that downstream event
//! emitters (block processing, telemetry, APIs) can expose the correct
//! lane/dataspace metadata. This module provides a global registry keyed by the
//! transaction hash to keep those decisions alive across subsystem boundaries.

use std::sync::LazyLock;

use dashmap::DashMap;
use iroha_crypto::HashOf;
use iroha_data_model::transaction::SignedTransaction;

use super::router::RoutingDecision;

/// Global routing registry indexed by `HashOf<SignedTransaction>`.
static ROUTING_REGISTRY: LazyLock<DashMap<HashOf<SignedTransaction>, RoutingDecision>> =
    LazyLock::new(DashMap::new);

/// Store (or replace) the routing decision for the given transaction hash.
pub fn record(hash: HashOf<SignedTransaction>, decision: RoutingDecision) {
    ROUTING_REGISTRY.insert(hash, decision);
}

/// Remove and return the routing decision for `hash`, if present.
pub fn take(hash: &HashOf<SignedTransaction>) -> Option<RoutingDecision> {
    ROUTING_REGISTRY.remove(hash).map(|(_, decision)| decision)
}

/// Retrieve the routing decision for `hash` without removing it.
#[cfg_attr(not(feature = "telemetry"), allow(dead_code))]
pub fn get(hash: &HashOf<SignedTransaction>) -> Option<RoutingDecision> {
    ROUTING_REGISTRY.get(hash).map(|entry| *entry.value())
}

/// Delete the routing decision for `hash` if it matches `expected`.
///
/// This is useful for cleanup paths where the queue can still observe the
/// cached decision (for example, when a transaction expires) but we do not want
/// to accidentally clear entries already consumed by downstream stages.
pub fn discard_if_matches(hash: &HashOf<SignedTransaction>, expected: RoutingDecision) {
    ROUTING_REGISTRY.remove_if(hash, |_, current| *current == expected);
}
