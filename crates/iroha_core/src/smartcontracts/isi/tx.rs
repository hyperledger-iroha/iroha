//! Implementations for transaction queries.

use std::{collections::BTreeSet, num::NonZeroUsize};

use eyre::Result;
use iroha_crypto::HashOf;
use iroha_data_model::{
    AccountId,
    block::{BlockHeader, SignedBlock},
    prelude::*,
    query::{
        CommittedTransaction, CommittedTxFilters, dsl::CompoundPredicate,
        error::QueryExecutionFail, json::PredicateJson,
    },
    transaction::signed::TransactionEntrypoint,
};
use iroha_telemetry::metrics;
use nonzero_ext::nonzero;
use norito::json::Value;

use crate::{smartcontracts::ValidQuery, state::StateReadOnly};

fn block_hash_from_value(value: &Value) -> Option<HashOf<BlockHeader>> {
    norito::json::from_value(value.clone()).ok()
}

fn entrypoint_hash_from_value(value: &Value) -> Option<HashOf<TransactionEntrypoint>> {
    norito::json::from_value(value.clone()).ok()
}

fn account_id_from_value(value: &Value) -> Option<AccountId> {
    norito::json::from_value(value.clone()).ok().or_else(|| {
        AccountId::parse_encoded(value.as_str()?)
            .ok()
            .map(iroha_data_model::account::ParsedAccountId::into_account_id)
    })
}

fn timestamp_ms_from_value(value: &Value) -> Option<u64> {
    norito::json::from_value(value.clone()).ok()
}

fn result_status_from_value(value: &Value) -> Option<bool> {
    norito::json::from_value(value.clone()).ok()
}

fn transaction_block_hash_field(field: &str) -> bool {
    matches!(field, "block_hash" | "block" | "block.hash")
}

fn transaction_entrypoint_hash_field(field: &str) -> bool {
    matches!(field, "entrypoint_hash" | "entrypoint.hash")
}

fn transaction_authority_field(field: &str) -> bool {
    matches!(field, "authority" | "authority_id")
}

fn transaction_timestamp_field(field: &str) -> bool {
    matches!(field, "timestamp_ms" | "creation_time_ms")
}

fn transaction_result_status_field(field: &str) -> bool {
    matches!(field, "result_ok" | "result.is_ok")
}

fn intersect_block_candidate_heights(
    selected: &mut Option<BTreeSet<NonZeroUsize>>,
    candidates: BTreeSet<NonZeroUsize>,
) {
    if let Some(selected) = selected {
        selected.retain(|height| candidates.contains(height));
    } else {
        *selected = Some(candidates);
    }
}

fn entrypoint_hash_candidate_heights(
    hash: Option<HashOf<TransactionEntrypoint>>,
    state_ro: &impl StateReadOnly,
) -> Option<BTreeSet<NonZeroUsize>> {
    let Some(hash) = hash else {
        return Some(BTreeSet::new());
    };
    state_ro.kura().get_block_heights_by_entrypoint_hash(hash)
}

fn entrypoint_hash_candidate_heights_from_values(
    values: &[Value],
    state_ro: &impl StateReadOnly,
) -> Option<BTreeSet<NonZeroUsize>> {
    let mut candidates = BTreeSet::new();
    for hash in values.iter().filter_map(entrypoint_hash_from_value) {
        let indexed = state_ro.kura().get_block_heights_by_entrypoint_hash(hash)?;
        candidates.extend(indexed);
    }
    Some(candidates)
}

fn authority_candidate_heights(
    authority: Option<AccountId>,
    state_ro: &impl StateReadOnly,
) -> Option<BTreeSet<NonZeroUsize>> {
    let Some(authority) = authority else {
        return Some(BTreeSet::new());
    };
    state_ro
        .kura()
        .get_block_heights_by_transaction_authority(&authority)
}

fn authority_candidate_heights_from_values(
    values: &[Value],
    state_ro: &impl StateReadOnly,
) -> Option<BTreeSet<NonZeroUsize>> {
    let mut candidates = BTreeSet::new();
    for authority in values.iter().filter_map(account_id_from_value) {
        let indexed = state_ro
            .kura()
            .get_block_heights_by_transaction_authority(&authority)?;
        candidates.extend(indexed);
    }
    Some(candidates)
}

fn timestamp_candidate_heights(
    timestamp_ms: Option<u64>,
    state_ro: &impl StateReadOnly,
) -> Option<BTreeSet<NonZeroUsize>> {
    let Some(timestamp_ms) = timestamp_ms else {
        return Some(BTreeSet::new());
    };
    state_ro
        .kura()
        .get_block_heights_by_transaction_timestamp_ms(timestamp_ms)
}

fn timestamp_candidate_heights_from_values(
    values: &[Value],
    state_ro: &impl StateReadOnly,
) -> Option<BTreeSet<NonZeroUsize>> {
    let mut candidates = BTreeSet::new();
    for timestamp_ms in values.iter().filter_map(timestamp_ms_from_value) {
        let indexed = state_ro
            .kura()
            .get_block_heights_by_transaction_timestamp_ms(timestamp_ms)?;
        candidates.extend(indexed);
    }
    Some(candidates)
}

fn result_status_candidate_heights(
    result_status: Option<bool>,
    state_ro: &impl StateReadOnly,
) -> Option<BTreeSet<NonZeroUsize>> {
    let Some(result_status) = result_status else {
        return Some(BTreeSet::new());
    };
    state_ro
        .kura()
        .get_block_heights_by_transaction_result_status(result_status)
}

fn result_status_candidate_heights_from_values(
    values: &[Value],
    state_ro: &impl StateReadOnly,
) -> Option<BTreeSet<NonZeroUsize>> {
    let mut candidates = BTreeSet::new();
    for result_status in values.iter().filter_map(result_status_from_value) {
        let indexed = state_ro
            .kura()
            .get_block_heights_by_transaction_result_status(result_status)?;
        candidates.extend(indexed);
    }
    Some(candidates)
}

fn transaction_candidate_block_heights(
    predicate: &PredicateJson,
    state_ro: &impl StateReadOnly,
) -> Option<BTreeSet<NonZeroUsize>> {
    let mut best = None;

    for cond in &predicate.equals {
        if transaction_block_hash_field(&cond.field) {
            intersect_block_candidate_heights(
                &mut best,
                block_hash_from_value(&cond.value)
                    .and_then(|hash| state_ro.kura().get_block_height_by_hash(hash))
                    .into_iter()
                    .collect(),
            );
        }
        if transaction_entrypoint_hash_field(&cond.field)
            && let Some(candidates) =
                entrypoint_hash_candidate_heights(entrypoint_hash_from_value(&cond.value), state_ro)
        {
            intersect_block_candidate_heights(&mut best, candidates);
        }
        if transaction_authority_field(&cond.field)
            && let Some(candidates) =
                authority_candidate_heights(account_id_from_value(&cond.value), state_ro)
        {
            intersect_block_candidate_heights(&mut best, candidates);
        }
        if transaction_timestamp_field(&cond.field)
            && let Some(candidates) =
                timestamp_candidate_heights(timestamp_ms_from_value(&cond.value), state_ro)
        {
            intersect_block_candidate_heights(&mut best, candidates);
        }
        if transaction_result_status_field(&cond.field)
            && let Some(candidates) =
                result_status_candidate_heights(result_status_from_value(&cond.value), state_ro)
        {
            intersect_block_candidate_heights(&mut best, candidates);
        }
    }

    for cond in &predicate.r#in {
        if transaction_block_hash_field(&cond.field) {
            intersect_block_candidate_heights(
                &mut best,
                cond.values
                    .iter()
                    .filter_map(block_hash_from_value)
                    .filter_map(|hash| state_ro.kura().get_block_height_by_hash(hash))
                    .collect(),
            );
        }
        if transaction_entrypoint_hash_field(&cond.field)
            && let Some(candidates) =
                entrypoint_hash_candidate_heights_from_values(&cond.values, state_ro)
        {
            intersect_block_candidate_heights(&mut best, candidates);
        }
        if transaction_authority_field(&cond.field)
            && let Some(candidates) =
                authority_candidate_heights_from_values(&cond.values, state_ro)
        {
            intersect_block_candidate_heights(&mut best, candidates);
        }
        if transaction_timestamp_field(&cond.field)
            && let Some(candidates) =
                timestamp_candidate_heights_from_values(&cond.values, state_ro)
        {
            intersect_block_candidate_heights(&mut best, candidates);
        }
        if transaction_result_status_field(&cond.field)
            && let Some(candidates) =
                result_status_candidate_heights_from_values(&cond.values, state_ro)
        {
            intersect_block_candidate_heights(&mut best, candidates);
        }
    }

    best
}

fn transaction_filter_candidate_block_heights(
    filters: &CommittedTxFilters,
    state_ro: &impl StateReadOnly,
) -> Option<BTreeSet<NonZeroUsize>> {
    let mut best = None;

    if let Some(entrypoint_hash) = filters.entry_eq.as_ref()
        && let Some(candidates) = state_ro
            .kura()
            .get_block_heights_by_entrypoint_hash(*entrypoint_hash)
    {
        intersect_block_candidate_heights(&mut best, candidates);
    }

    if !filters.entry_in.is_empty() {
        let mut candidates = BTreeSet::new();
        for entrypoint_hash in &filters.entry_in {
            let indexed = state_ro
                .kura()
                .get_block_heights_by_entrypoint_hash(*entrypoint_hash)?;
            candidates.extend(indexed);
        }
        intersect_block_candidate_heights(&mut best, candidates);
    }

    if let Some(authority) = filters.authority_eq.as_ref()
        && let Some(candidates) = state_ro
            .kura()
            .get_block_heights_by_transaction_authority(authority)
    {
        intersect_block_candidate_heights(&mut best, candidates);
    }

    if !filters.authority_in.is_empty() {
        let mut candidates = BTreeSet::new();
        for authority in &filters.authority_in {
            let indexed = state_ro
                .kura()
                .get_block_heights_by_transaction_authority(authority)?;
            candidates.extend(indexed);
        }
        intersect_block_candidate_heights(&mut best, candidates);
    }

    if filters.ts_ge.is_some() || filters.ts_le.is_some() {
        let candidates = state_ro
            .kura()
            .get_block_heights_by_transaction_timestamp_range(filters.ts_ge, filters.ts_le)?;
        intersect_block_candidate_heights(&mut best, candidates);
    }

    if let Some(result_ok) = filters.result_ok
        && let Some(candidates) = state_ro
            .kura()
            .get_block_heights_by_transaction_result_status(result_ok)
    {
        intersect_block_candidate_heights(&mut best, candidates);
    }

    if !filters.result_ok_in.is_empty() {
        let mut candidates = BTreeSet::new();
        for result_ok in filters.result_ok_in.iter().copied() {
            let indexed = state_ro
                .kura()
                .get_block_heights_by_transaction_result_status(result_ok)?;
            candidates.extend(indexed);
        }
        intersect_block_candidate_heights(&mut best, candidates);
    }

    best
}

fn predicate_value_at_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    if path.is_empty() {
        return None;
    }
    let mut current = value;
    for segment in path.split('.') {
        if segment.is_empty() {
            return None;
        }
        match current {
            Value::Object(map) => current = map.get(segment)?,
            _ => return None,
        }
    }
    Some(current)
}

fn transaction_field_equals(
    tx: &CommittedTransaction,
    field: &str,
    expected: &Value,
    tx_value: Option<&Value>,
) -> bool {
    if transaction_block_hash_field(field) {
        return block_hash_from_value(expected).is_some_and(|hash| tx.block_hash == hash);
    }
    if transaction_entrypoint_hash_field(field) {
        return entrypoint_hash_from_value(expected).is_some_and(|hash| tx.entrypoint_hash == hash);
    }
    if transaction_authority_field(field) {
        return account_id_from_value(expected)
            .is_some_and(|authority| tx.entrypoint.authority_opt() == Some(&authority));
    }
    if transaction_timestamp_field(field) {
        return timestamp_ms_from_value(expected)
            .is_some_and(|timestamp_ms| tx.entrypoint.creation_time_ms() == Some(timestamp_ms));
    }
    if transaction_result_status_field(field) {
        return result_status_from_value(expected)
            .is_some_and(|result_status| tx.result.as_ref().is_ok() == result_status);
    }
    tx_value.and_then(|value| predicate_value_at_path(value, field)) == Some(expected)
}

fn transaction_field_in(
    tx: &CommittedTransaction,
    field: &str,
    expected_values: &[Value],
    tx_value: Option<&Value>,
) -> bool {
    expected_values
        .iter()
        .any(|expected| transaction_field_equals(tx, field, expected, tx_value))
}

fn transaction_field_exists(
    tx: &CommittedTransaction,
    field: &str,
    tx_value: Option<&Value>,
) -> bool {
    if transaction_block_hash_field(field)
        || transaction_entrypoint_hash_field(field)
        || transaction_result_status_field(field)
    {
        return true;
    }
    if transaction_authority_field(field) {
        return tx.entrypoint.authority_opt().is_some();
    }
    if transaction_timestamp_field(field) {
        return tx.entrypoint.creation_time_ms().is_some();
    }
    tx_value
        .and_then(|value| predicate_value_at_path(value, field))
        .is_some_and(|actual| !actual.is_null())
}

fn transaction_predicate_json_applies(
    predicate: &PredicateJson,
    tx: &CommittedTransaction,
) -> bool {
    let tx_value = norito::json::to_value(tx).ok();
    let tx_value = tx_value.as_ref();

    for cond in &predicate.equals {
        if !transaction_field_equals(tx, &cond.field, &cond.value, tx_value) {
            return false;
        }
    }
    for cond in &predicate.r#in {
        if !transaction_field_in(tx, &cond.field, &cond.values, tx_value) {
            return false;
        }
    }
    for field in &predicate.exists {
        if !transaction_field_exists(tx, field, tx_value) {
            return false;
        }
    }
    true
}

fn block_committed_transactions(block: &SignedBlock) -> Vec<CommittedTransaction> {
    let block_hash = block.hash();

    let entrypoint_hashes = block.entrypoint_hashes().rev();
    let entrypoint_proofs = block.entrypoint_proofs().rev();
    let entrypoints = block.entrypoints_cloned().rev();
    let result_hashes = block.result_hashes().rev();
    let result_proofs = block.result_proofs().rev();
    let results = block.results().cloned().rev();

    entrypoint_hashes
        .zip(entrypoint_proofs)
        .zip(entrypoints)
        .zip(result_hashes)
        .zip(result_proofs)
        .zip(results)
        .map(
            |(
                ((((entrypoint_hash, entrypoint_proof), entrypoint), result_hash), result_proof),
                result,
            )| {
                CommittedTransaction {
                    block_hash,
                    entrypoint_hash,
                    entrypoint_proof,
                    entrypoint,
                    result_hash,
                    result_proof,
                    result,
                }
            },
        )
        .collect()
}

impl ValidQuery for FindTransactions {
    #[metrics(+"find_transactions")]
    fn execute(
        self,
        filter: CompoundPredicate<CommittedTransaction>,
        state_ro: &impl StateReadOnly,
    ) -> Result<impl Iterator<Item = Self::Item>, QueryExecutionFail> {
        let predicate_json = filter
            .json_payload()
            .and_then(|raw| norito::json::from_str::<PredicateJson>(raw).ok());

        let mut candidate_heights = None;
        if let Some(filters) = filter.committed_tx_filters()
            && let Some(candidates) = transaction_filter_candidate_block_heights(&filters, state_ro)
        {
            intersect_block_candidate_heights(&mut candidate_heights, candidates);
        }
        if let Some(candidates) = predicate_json
            .as_ref()
            .and_then(|predicate| transaction_candidate_block_heights(predicate, state_ro))
        {
            intersect_block_candidate_heights(&mut candidate_heights, candidates);
        }

        let iter: Box<dyn Iterator<Item = CommittedTransaction> + '_> =
            if let Some(candidate_heights) = candidate_heights {
                Box::new(
                    candidate_heights
                        .into_iter()
                        .rev()
                        .filter_map(|height| state_ro.kura().get_block(height))
                        .flat_map(|block| block_committed_transactions(&block)),
                )
            } else {
                Box::new(
                    state_ro
                        .all_blocks(nonzero!(1_usize))
                        // Iterate over blocks in descending order (most recent first).
                        .rev()
                        .flat_map(|block| block_committed_transactions(&block)),
                )
            };

        Ok(iter.filter(move |tx| {
            predicate_json.as_ref().map_or_else(
                || filter.applies(tx),
                |predicate| transaction_predicate_json_applies(predicate, tx),
            )
        }))
    }
}

#[cfg(test)]
mod tests {
    use iroha_data_model::prelude::{TransactionEntrypoint, TransactionResult};

    use crate::tx::tests::*;

    /// Verifies that all per-field iterators over a committed block are consistent.
    #[tokio::test]
    async fn block_iterators_are_consistent() {
        let mut sandbox = Sandbox::default()
            .with_data_trigger_transfer("bob", 40, "carol")
            .with_time_trigger_transfer_labeled("alice", 1, "alice", 0)
            .with_time_trigger_transfer_labeled("alice", 1, "alice", 1)
            .with_time_trigger_transfer_labeled("alice", 1, "alice", 2)
            .with_time_trigger_transfer("carol", 30, "dave")
            .with_data_trigger_transfer("dave", 20, "eve");
        sandbox.request_transfer("alice", 50, "bob");
        sandbox.request_transfer("eve", 1, "eve");
        sandbox.request_transfer("eve", 1, "eve");
        sandbox.request_transfer("eve", 1, "eve");
        sandbox.request_transfer("eve", 1, "eve");
        sandbox.request_transfer("eve", 1, "eve");
        let mut block = sandbox.block();
        block.assert_balances([
            ("alice", 60),
            ("bob", 10),
            ("carol", 10),
            ("dave", 10),
            ("eve", 10),
        ]);
        let (_events, committed_block) = block.apply();
        block.assert_balances([
            ("alice", 10),
            ("bob", 20),
            ("carol", 20),
            ("dave", 20),
            ("eve", 30),
        ]);
        let block = committed_block.as_ref();

        // All entrypoint-related iterators yield the same number of elements.
        assert_eq!(10, block.entrypoint_hashes().len());
        assert_eq!(10, block.entrypoint_proofs().len());
        assert_eq!(10, block.entrypoints_cloned().len());
        assert_eq!(10, block.result_hashes().len());
        assert_eq!(10, block.result_proofs().len());
        assert_eq!(10, block.results().len());
        assert_eq!(6, block.external_transactions().len());
        assert_eq!(4, block.time_triggers().len());

        // Hashes of entrypoints and results match their respective contents.
        assert_eq!(
            block.entrypoint_hashes().collect::<Vec<_>>(),
            block
                .entrypoints_cloned()
                .map(|e| e.hash())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            block.result_hashes().collect::<Vec<_>>(),
            block
                .results()
                .map(TransactionResult::hash)
                .collect::<Vec<_>>()
        );

        // External and time-triggered entrypoints are merged correctly into a unified view.
        assert_eq!(
            block.entrypoints_cloned().collect::<Vec<_>>(),
            block
                .external_transactions()
                .cloned()
                .map(TransactionEntrypoint::from)
                .chain(
                    block
                        .time_triggers()
                        .cloned()
                        .map(TransactionEntrypoint::from)
                )
                .collect::<Vec<_>>()
        );

        // The order and content of the first and last transactions are as expected.
        // Ensure the first merged entrypoint matches the first external transaction and
        // the last entrypoint matches the last time-triggered entry.
        assert_eq!(
            block.entrypoints_cloned().next(),
            block
                .external_transactions()
                .cloned()
                .map(TransactionEntrypoint::from)
                .next()
        );
        assert_eq!(
            block.entrypoints_cloned().next_back(),
            block
                .time_triggers()
                .cloned()
                .map(TransactionEntrypoint::from)
                .next_back()
        );

        // Results remain aligned with entrypoints across the merged view.
        assert_eq!(
            block.results().next().map(TransactionResult::hash),
            block.result_hashes().next()
        );
        assert_eq!(
            block.results().last().map(TransactionResult::hash),
            block.result_hashes().last()
        );
    }
}
