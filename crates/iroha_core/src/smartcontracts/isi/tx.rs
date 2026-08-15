//! Implementations for transaction queries.
use crate::{smartcontracts::ValidQuery, state::StateReadOnly};
use eyre::Result;
use iroha_crypto::{Hash, HashOf, MerkleTree};
use iroha_data_model::{
    AccountId,
    block::{BlockHeader, CertifiedMergeLedgerReference, SignedBlock},
    merge::MergeLedgerEntry,
    prelude::*,
    query::{
        CertifiedMergeTransactionInclusion, CommittedTransaction, CommittedTxFilters,
        dsl::CompoundPredicate, error::QueryExecutionFail, json::PredicateJson,
    },
    transaction::{TransactionResult, signed::TransactionEntrypoint},
};
use iroha_telemetry::metrics;
use nonzero_ext::nonzero;
use norito::json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroUsize,
    ops::ControlFlow,
};
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
    if let Some(block_hash) = filters.block_eq.as_ref() {
        intersect_block_candidate_heights(
            &mut best,
            state_ro
                .kura()
                .get_block_height_by_hash(*block_hash)
                .into_iter()
                .collect(),
        );
    }
    if !filters.block_in.is_empty() {
        intersect_block_candidate_heights(
            &mut best,
            filters
                .block_in
                .iter()
                .filter_map(|hash| state_ro.kura().get_block_height_by_hash(*hash))
                .collect(),
        );
    }
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
fn transaction_query_plan(
    filter: &CompoundPredicate<CommittedTransaction>,
    state_ro: &impl StateReadOnly,
) -> (Option<PredicateJson>, Option<BTreeSet<NonZeroUsize>>) {
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
    (predicate_json, candidate_heights)
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
fn transaction_filter_applies(
    filter: &CompoundPredicate<CommittedTransaction>,
    predicate_json: Option<&PredicateJson>,
    tx: &CommittedTransaction,
) -> bool {
    predicate_json.map_or_else(
        || filter.applies(tx),
        |predicate| transaction_predicate_json_applies(predicate, tx),
    )
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
                    merge_inclusion: None,
                }
            },
        )
        .collect()
}
fn merge_query_corruption(message: impl std::fmt::Display) -> QueryExecutionFail {
    QueryExecutionFail::Conversion(format!(
        "certified merge transaction history is inconsistent: {message}"
    ))
}
#[cfg(test)]
std::thread_local! {
    static CERTIFIED_MERGE_PROJECTION_CALLS: std::cell::Cell<usize> = const {
        std::cell::Cell::new(0)
    };
}
#[cfg(test)]
/// Reset the thread-local eager merge projection counter used by query gas tests.
pub(crate) fn reset_certified_merge_projection_calls_for_test() {
    CERTIFIED_MERGE_PROJECTION_CALLS.set(0);
}
#[cfg(test)]
/// Return eager merge projection calls observed on the current test thread.
pub(crate) fn certified_merge_projection_calls_for_test() -> usize {
    CERTIFIED_MERGE_PROJECTION_CALLS.get()
}
/// Project the authenticated transaction/result pairs carried by a certified merge entry.
///
/// The compact carrier reference, execution-batch commitments, transcript hashes, and Merkle
/// roots are all revalidated before any transaction is returned.  Consumers outside the query
/// executor use this projection so transaction lifecycle status never trusts an unauthenticated
/// merge sidecar or reimplements only a subset of these checks.
pub fn certified_merge_committed_transactions(
    carrier_hash: HashOf<BlockHeader>,
    reference: &CertifiedMergeLedgerReference,
    entry: &MergeLedgerEntry,
) -> Result<Vec<CommittedTransaction>, QueryExecutionFail> {
    #[cfg(test)]
    CERTIFIED_MERGE_PROJECTION_CALLS.set(CERTIFIED_MERGE_PROJECTION_CALLS.get().saturating_add(1));
    if !reference.matches_entry(entry) {
        return Err(merge_query_corruption(
            "carrier compact reference does not identify its full sidecar",
        ));
    }
    let batch = entry.execution_batch.as_ref().ok_or_else(|| {
        merge_query_corruption("execution carrier references an entry without an execution batch")
    })?;
    if batch.version != 1 || !crate::merge::merge_execution_batch_commitments_match(batch) {
        return Err(merge_query_corruption(
            "merge execution batch commitments are not canonical",
        ));
    }
    let entrypoint_count = usize::try_from(batch.entrypoint_count)
        .map_err(|_| merge_query_corruption("entrypoint count does not fit this platform"))?;
    if entrypoint_count == 0 || u32::try_from(entrypoint_count).is_err() {
        return Err(merge_query_corruption(
            "entrypoint count is outside the supported Merkle proof range",
        ));
    }
    let mut entrypoints = Vec::with_capacity(entrypoint_count);
    let mut results = Vec::with_capacity(entrypoint_count);
    for execution in &batch.lanes {
        let lane_len = execution.entrypoints.len();
        if lane_len == 0
            || execution.entrypoint_hashes.len() != lane_len
            || execution.results.len() != lane_len
            || execution.result_hashes.len() != lane_len
        {
            return Err(merge_query_corruption(
                "lane transcript arrays are empty or not aligned",
            ));
        }
        if execution
            .entrypoints
            .iter()
            .zip(&execution.entrypoint_hashes)
            .any(|(entrypoint, expected)| Hash::from(entrypoint.hash()) != *expected)
            || execution
                .results
                .iter()
                .zip(&execution.result_hashes)
                .any(|(result, expected)| Hash::from(result.hash()) != *expected)
        {
            return Err(merge_query_corruption(
                "lane transcript content differs from its authenticated hashes",
            ));
        }
        entrypoints.extend(execution.entrypoints.iter().cloned());
        results.extend(execution.results.iter().cloned());
    }
    if entrypoints.len() != entrypoint_count || results.len() != entrypoint_count {
        return Err(merge_query_corruption(
            "flattened lane transcript differs from the certified entrypoint count",
        ));
    }
    let entrypoint_hashes = entrypoints
        .iter()
        .map(TransactionEntrypoint::hash)
        .collect::<Vec<_>>();
    let result_hashes = results
        .iter()
        .map(TransactionResult::hash)
        .collect::<Vec<_>>();
    let entrypoint_tree = entrypoint_hashes
        .iter()
        .copied()
        .collect::<MerkleTree<TransactionEntrypoint>>();
    let result_tree = result_hashes
        .iter()
        .copied()
        .collect::<MerkleTree<TransactionResult>>();
    if entrypoint_tree.root() != Some(batch.entrypoint_merkle_root)
        || result_tree.root() != Some(batch.result_merkle_root)
    {
        return Err(merge_query_corruption(
            "reconstructed transaction proof roots differ from the certified batch",
        ));
    }
    let inclusion = CertifiedMergeTransactionInclusion {
        version: 1,
        merge_entry_hash: entry.canonical_hash(),
        merge_epoch_id: entry.epoch_id,
        execution_batch_hash: batch.batch_hash,
        entrypoint_count: batch.entrypoint_count,
        entrypoint_merkle_root: batch.entrypoint_merkle_root,
        result_merkle_root: batch.result_merkle_root,
    };
    let mut committed = Vec::with_capacity(entrypoint_count);
    for index in (0..entrypoint_count).rev() {
        let proof_index = u32::try_from(index)
            .map_err(|_| merge_query_corruption("Merkle proof index exceeds u32"))?;
        let entrypoint_proof = entrypoint_tree.get_proof(proof_index).ok_or_else(|| {
            merge_query_corruption("entrypoint Merkle tree did not yield a required proof")
        })?;
        let result_proof = result_tree.get_proof(proof_index).ok_or_else(|| {
            merge_query_corruption("result Merkle tree did not yield a required proof")
        })?;
        committed.push(CommittedTransaction {
            block_hash: carrier_hash,
            entrypoint_hash: entrypoint_hashes[index],
            entrypoint_proof,
            entrypoint: entrypoints[index].clone(),
            result_hash: result_hashes[index],
            result_proof,
            result: results[index].clone(),
            merge_inclusion: Some(inclusion.clone()),
        });
    }
    Ok(committed)
}
fn certified_merge_projection_work(entry: &MergeLedgerEntry) -> usize {
    let Some(batch) = entry.execution_batch.as_ref() else {
        return 0;
    };
    let declared = usize::try_from(batch.entrypoint_count).unwrap_or(usize::MAX);
    let observed = batch.lanes.iter().fold(0_usize, |total, execution| {
        total.saturating_add(
            execution
                .entrypoints
                .len()
                .max(execution.entrypoint_hashes.len())
                .max(execution.results.len())
                .max(execution.result_hashes.len()),
        )
    });
    declared.max(observed)
}
fn committed_merge_transactions_by_height(
    state_ro: &impl StateReadOnly,
    candidate_heights: Option<&BTreeSet<NonZeroUsize>>,
) -> Result<BTreeMap<NonZeroUsize, Vec<CommittedTransaction>>, QueryExecutionFail> {
    if let Some(candidate_heights) = candidate_heights {
        let mut by_height = BTreeMap::new();
        for &height in candidate_heights {
            let block = state_ro.kura().get_block(height).ok_or_else(|| {
                merge_query_corruption(format!(
                    "indexed canonical block {} body is unavailable",
                    height.get()
                ))
            })?;
            let Some(reference) = block
                .execution_context()
                .and_then(|context| context.merge_entry.as_ref())
            else {
                continue;
            };
            let carrier_height = u64::try_from(height.get()).map_err(|_| {
                merge_query_corruption("indexed carrier height does not fit the canonical range")
            })?;
            let entry = state_ro
                .kura()
                .merge_entry_for_carrier(carrier_height, block.hash())
                .map_err(merge_query_corruption)?
                .ok_or_else(|| {
                    merge_query_corruption(format!(
                        "indexed carrier block {} has no matching durable merge entry",
                        height.get()
                    ))
                })?;
            if entry.execution_batch.is_none() {
                continue;
            }
            let transactions =
                certified_merge_committed_transactions(block.hash(), reference, &entry)?;
            if by_height.insert(height, transactions).is_some() {
                return Err(merge_query_corruption(format!(
                    "indexed carrier height {} was selected more than once",
                    height.get()
                )));
            }
        }
        return Ok(by_height);
    }
    let carried_entries = state_ro
        .kura()
        .committed_merge_execution_entries()
        .map_err(merge_query_corruption)?;
    let mut by_height = BTreeMap::new();
    for (carrier, entry) in carried_entries {
        let height = usize::try_from(carrier.block_height)
            .ok()
            .and_then(NonZeroUsize::new)
            .ok_or_else(|| merge_query_corruption("carrier height is zero or out of range"))?;
        let block = state_ro.kura().get_block(height).ok_or_else(|| {
            merge_query_corruption(format!(
                "canonical carrier block {} body is unavailable",
                carrier.block_height
            ))
        })?;
        if block.hash() != carrier.block_hash {
            return Err(merge_query_corruption(format!(
                "carrier record at height {} does not match the canonical block hash",
                carrier.block_height
            )));
        }
        let reference = block
            .execution_context()
            .and_then(|context| context.merge_entry.as_ref())
            .ok_or_else(|| {
                merge_query_corruption(format!(
                    "carrier block {} has no certified merge reference",
                    carrier.block_height
                ))
            })?;
        let transactions =
            certified_merge_committed_transactions(carrier.block_hash, reference, &entry)?;
        if by_height.insert(height, transactions).is_some() {
            return Err(merge_query_corruption(format!(
                "multiple execution entries claim carrier height {}",
                carrier.block_height
            )));
        }
    }
    Ok(by_height)
}
fn block_committed_transactions_with_merge(
    block: &SignedBlock,
    merge_by_height: &BTreeMap<NonZeroUsize, Vec<CommittedTransaction>>,
) -> Vec<CommittedTransaction> {
    let mut committed = block_committed_transactions(block);
    if let Some(merge) = usize::try_from(block.header().height().get())
        .ok()
        .and_then(NonZeroUsize::new)
        .and_then(|height| merge_by_height.get(&height))
    {
        // Merge execution precedes ordinary block transactions. Both groups are
        // individually reversed so query order remains newest-first.
        committed.extend(merge.iter().cloned());
    }
    committed
}
/// Immutable upper bound for a replayed transaction-history scan.
///
/// Exact stored queries retain this compact chain anchor instead of retaining
/// every projected transaction. A continuation accepts later appended blocks,
/// but it must continue to observe the same canonical prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TransactionHistoryAnchor {
    height: usize,
    tip_hash: Option<HashOf<BlockHeader>>,
}
impl TransactionHistoryAnchor {
    /// Capture the canonical prefix visible to a query view.
    pub(crate) fn capture(state_ro: &impl StateReadOnly) -> Self {
        Self {
            height: state_ro.height(),
            tip_hash: state_ro.latest_block_hash(),
        }
    }
    fn validate(self, state_ro: &impl StateReadOnly) -> Result<(), QueryExecutionFail> {
        let observed_tip = self
            .height
            .checked_sub(1)
            .and_then(|index| state_ro.block_hashes().get(index))
            .copied();
        if observed_tip != self.tip_hash {
            return Err(QueryExecutionFail::Expired);
        }
        Ok(())
    }
}
/// Compact resume position within an anchored transaction-history scan.
///
/// The cursor points just after a projected transaction in one carrier block.
/// Replaying it may re-read that one carrier, but never rescans newer carriers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TransactionHistoryCursor {
    height: usize,
    transaction_offset: usize,
}
/// Visit committed transactions in canonical newest-first query order.
///
/// Unlike [`ValidQuery`] execution, this path is fallible while walking Kura. It resolves and drops
/// one certified merge sidecar at a time, allowing query pagination to retain only its current page
/// (or bounded sorted prefix). `before_project` first receives the ordinary count plus the compact
/// reference's authenticated merge count before the full sidecar is resolved. If decoded transcript
/// arrays exceed that declaration, it receives only the reconciliation delta before merge Merkle
/// trees/proofs or predicates are materialized. The visitor then receives every projected
/// transaction together with whether the query predicate matched it and a compact cursor pointing
/// after it.
///
/// Returns `true` when the selected history was exhausted and `false` when the
/// visitor requested an early stop.
///
/// # Errors
///
/// Returns [`QueryExecutionFail::Expired`] if an anchored canonical prefix is
/// no longer available, or [`QueryExecutionFail::Conversion`] when a touched
/// block, carrier, or certified merge sidecar is inconsistent.
pub(crate) fn visit_committed_transactions(
    state_ro: &impl StateReadOnly,
    filter: &CompoundPredicate<CommittedTransaction>,
    anchor: TransactionHistoryAnchor,
    resume: Option<TransactionHistoryCursor>,
    mut before_project: impl FnMut(u64) -> Result<(), QueryExecutionFail>,
    mut visitor: impl FnMut(
        CommittedTransaction,
        bool,
        TransactionHistoryCursor,
    ) -> Result<ControlFlow<()>, QueryExecutionFail>,
) -> Result<bool, QueryExecutionFail> {
    anchor.validate(state_ro)?;
    let (predicate_json, candidate_heights) = transaction_query_plan(filter, state_ro);
    let indexed = candidate_heights.is_some();
    let maximum_height = resume.map_or(anchor.height, |cursor| cursor.height.min(anchor.height));
    let heights: Box<dyn Iterator<Item = NonZeroUsize>> = match candidate_heights {
        Some(heights) => Box::new(
            heights
                .into_iter()
                .rev()
                .filter(move |height| height.get() <= maximum_height),
        ),
        None => Box::new((1..=maximum_height).rev().filter_map(NonZeroUsize::new)),
    };
    for height in heights {
        let expected_hash = height
            .get()
            .checked_sub(1)
            .and_then(|index| state_ro.block_hashes().get(index))
            .copied()
            .ok_or_else(|| {
                merge_query_corruption(format!(
                    "canonical block hash {} is unavailable in the query anchor",
                    height.get()
                ))
            })?;
        let carrier_height = u64::try_from(height.get()).map_err(|_| {
            merge_query_corruption("carrier height does not fit the canonical range")
        })?;
        // Read only the canonical block and its compact reference first. The
        // full sidecar may be large, so its authenticated declared work must be
        // charged before Kura resolves or decodes it.
        let Some(block) = state_ro.kura().get_block_without_merge_sidecar(height) else {
            if indexed {
                return Err(merge_query_corruption(format!(
                    "canonical block {} body is unavailable",
                    height.get()
                )));
            }
            continue;
        };
        if block.hash() != expected_hash {
            return Err(merge_query_corruption(format!(
                "canonical block {} hash differs from the anchored query view",
                height.get()
            )));
        }
        let reference = block
            .execution_context()
            .and_then(|context| context.merge_entry.as_ref());
        let declared_merge_work = match reference.map(|reference| {
            (
                reference.execution_batch_hash.is_some(),
                reference.entrypoint_count,
            )
        }) {
            None | Some((false, None)) => 0,
            Some((true, Some(count))) if count > 0 => count,
            Some(_) => {
                return Err(merge_query_corruption(format!(
                    "carrier block {} has an inconsistent compact execution count",
                    height.get()
                )));
            }
        };
        let ordinary_work = u64::try_from(block.entrypoint_hashes().count()).unwrap_or(u64::MAX);
        before_project(ordinary_work.saturating_add(declared_merge_work))?;
        // Resolve only after the compact declaration has passed budget
        // admission. This preserves fail-closed sidecar validation without
        // allowing a false predicate to force uncharged sidecar I/O.
        let merge_entry = state_ro
            .kura()
            .merge_entry_for_carrier(carrier_height, expected_hash)
            .map_err(merge_query_corruption)?;
        let merge_projection = match (reference, merge_entry.as_ref()) {
            (None, None) => None,
            (Some(reference), Some(entry)) => Some((reference, entry)),
            (Some(_), None) => {
                return Err(merge_query_corruption(format!(
                    "carrier block {} has no matching durable merge entry",
                    height.get()
                )));
            }
            (None, Some(_)) => {
                return Err(merge_query_corruption(format!(
                    "sparse carrier record at block {} has no compact reference",
                    height.get()
                )));
            }
        };
        let observed_merge_work =
            merge_projection.map_or(0, |(_, entry)| certified_merge_projection_work(entry));
        let declared_merge_work = usize::try_from(declared_merge_work).unwrap_or(usize::MAX);
        let reconciliation_work = observed_merge_work.saturating_sub(declared_merge_work);
        if reconciliation_work > 0 {
            before_project(u64::try_from(reconciliation_work).unwrap_or(u64::MAX))?;
        }
        if merge_projection.is_some_and(|(reference, entry)| !reference.matches_entry(entry)) {
            return Err(merge_query_corruption(format!(
                "carrier block {} compact reference differs from its durable entry",
                height.get()
            )));
        }
        let merge_transactions = merge_projection
            .and_then(|(reference, entry)| {
                entry.execution_batch.as_ref().map(|_| (reference, entry))
            })
            .map(|(reference, entry)| {
                certified_merge_committed_transactions(block.hash(), reference, entry)
            })
            .transpose()?;
        // Merge execution precedes ordinary execution in the same global block,
        // so newest-first query order emits ordinary transactions first.
        let transaction_offset = resume
            .filter(|cursor| cursor.height == height.get())
            .map_or(0, |cursor| cursor.transaction_offset);
        let ordinary_transactions = block_committed_transactions(&block);
        let transaction_count = ordinary_transactions
            .len()
            .saturating_add(merge_transactions.as_ref().map_or(0, std::vec::Vec::len));
        for (index, transaction) in ordinary_transactions
            .into_iter()
            .chain(merge_transactions.into_iter().flatten())
            .enumerate()
            .skip(transaction_offset)
        {
            let matches = transaction_filter_applies(filter, predicate_json.as_ref(), &transaction);
            let next_cursor = if index.saturating_add(1) < transaction_count {
                TransactionHistoryCursor {
                    height: height.get(),
                    transaction_offset: index.saturating_add(1),
                }
            } else {
                TransactionHistoryCursor {
                    height: height.get().saturating_sub(1),
                    transaction_offset: 0,
                }
            };
            if visitor(transaction, matches, next_cursor)?.is_break() {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
/// Visit committed transactions without materializing the complete ledger history.
///
/// `max_carrier_projection_work` is checked independently for each compact carrier before its
/// full merge sidecar is resolved. The visitor can stop as soon as its bounded page or heap is
/// complete; sparse and non-matching history therefore does not consume a global retention
/// budget merely because the chain is old.
///
/// # Errors
///
/// Returns [`QueryExecutionFail::GasBudgetExceeded`] before resolving a carrier whose declared
/// projection exceeds `max_carrier_projection_work`, or propagates durable carrier/sidecar
/// validation failures.
pub fn visit_committed_transactions_bounded(
    state_ro: &impl StateReadOnly,
    filter: CompoundPredicate<CommittedTransaction>,
    max_carrier_projection_work: u64,
    mut visitor: impl FnMut(CommittedTransaction, bool) -> Result<ControlFlow<()>, QueryExecutionFail>,
) -> Result<bool, QueryExecutionFail> {
    if max_carrier_projection_work == 0
        || max_carrier_projection_work > iroha_data_model::query::parameters::MAX_FETCH_SIZE.get()
    {
        return Err(QueryExecutionFail::FetchSizeTooBig);
    }
    visit_committed_transactions(
        state_ro,
        &filter,
        TransactionHistoryAnchor::capture(state_ro),
        None,
        |work| {
            if work > max_carrier_projection_work {
                return Err(QueryExecutionFail::GasBudgetExceeded);
            }
            Ok(())
        },
        |transaction, matches, _| visitor(transaction, matches),
    )
}
/// Collect a small committed-transaction snapshot within explicit retention bounds.
///
/// Carrier work is bounded independently before sidecar resolution. Transactions are visited
/// newest first and only predicate matches are retained. The function rejects a result set that
/// exceeds `max_projected_transactions` instead of returning a silently truncated snapshot, and
/// charges canonical retained bytes before every push.
///
/// # Errors
///
/// Returns [`QueryExecutionFail::GasBudgetExceeded`] before resolving an oversized carrier or
/// retaining bytes beyond `max_retained_bytes`, [`QueryExecutionFail::FetchSizeTooBig`] when the
/// matched result set exceeds the canonical fetch ceiling, or propagates durable validation
/// failures.
pub fn committed_transactions_bounded_snapshot(
    state_ro: &impl StateReadOnly,
    filter: CompoundPredicate<CommittedTransaction>,
    max_projected_transactions: u64,
    max_retained_bytes: u64,
) -> Result<Vec<CommittedTransaction>, QueryExecutionFail> {
    if max_projected_transactions == 0 || max_retained_bytes == 0 {
        return Err(QueryExecutionFail::GasBudgetExceeded);
    }
    if max_projected_transactions > iroha_data_model::query::parameters::MAX_FETCH_SIZE.get() {
        return Err(QueryExecutionFail::FetchSizeTooBig);
    }
    let capacity = usize::try_from(max_projected_transactions).unwrap_or(usize::MAX);
    let mut retained_bytes = 0_u64;
    let mut transactions = Vec::new();
    transactions
        .try_reserve_exact(capacity)
        .map_err(|_| QueryExecutionFail::GasBudgetExceeded)?;
    visit_committed_transactions_bounded(
        state_ro,
        filter,
        max_projected_transactions,
        |transaction, matches| {
            if matches {
                if transactions.len() == capacity {
                    return Err(QueryExecutionFail::FetchSizeTooBig);
                }
                let transaction_bytes =
                    u64::try_from(norito::codec::Encode::encoded_len(&transaction))
                        .unwrap_or(u64::MAX);
                retained_bytes = retained_bytes
                    .checked_add(transaction_bytes)
                    .filter(|bytes| *bytes <= max_retained_bytes)
                    .ok_or(QueryExecutionFail::GasBudgetExceeded)?;
                transactions.push(transaction);
            }
            Ok(ControlFlow::Continue(()))
        },
    )?;
    Ok(transactions)
}
/// Materialize complete canonical transaction history in newest-first order.
///
/// This includes transactions executed by globally ordered certified merge sidecars. The durable
/// sparse carrier index and every full sidecar are revalidated before any history is returned, so
/// callers never receive a cache-truncated or partially fabricated view.
///
/// # Errors
///
/// Returns [`QueryExecutionFail::Conversion`] when durable carrier, block, or
/// sidecar evidence is unavailable, malformed, or mutually inconsistent.
pub fn committed_transactions_snapshot(
    state_ro: &impl StateReadOnly,
) -> Result<Vec<CommittedTransaction>, QueryExecutionFail> {
    let merge_by_height = committed_merge_transactions_by_height(state_ro, None)?;
    Ok(state_ro
        .all_blocks(nonzero!(1_usize))
        .rev()
        .flat_map(|block| block_committed_transactions_with_merge(&block, &merge_by_height))
        .collect())
}
/// Materialize an index-bounded committed-transaction snapshot.
///
/// The filter must resolve through Kura's positive transaction indexes. This
/// keeps app-facing aggregate and projection queries from holding a world-state
/// view while rebuilding complete transaction history. The authoritative
/// predicate is still evaluated against every selected transaction.
///
/// # Errors
///
/// Returns [`QueryExecutionFail::Conversion`] when the filter is not bounded by
/// a complete sparse index, or selected durable evidence is unavailable.
pub fn committed_transactions_indexed_snapshot(
    state_ro: &impl StateReadOnly,
    filter: CompoundPredicate<CommittedTransaction>,
) -> Result<Vec<CommittedTransaction>, QueryExecutionFail> {
    let (_, candidate_heights) = transaction_query_plan(&filter, state_ro);
    if candidate_heights.is_none() {
        return Err(QueryExecutionFail::Conversion(
            "transaction aggregate/select queries require a positive indexed filter".to_owned(),
        ));
    }
    ValidQuery::execute(FindTransactions, filter, state_ro)
        .map(|transactions| transactions.collect())
}
impl ValidQuery for FindTransactions {
    #[metrics(+"find_transactions")]
    fn execute(
        self,
        filter: CompoundPredicate<CommittedTransaction>,
        state_ro: &impl StateReadOnly,
    ) -> Result<impl Iterator<Item = Self::Item>, QueryExecutionFail> {
        let (predicate_json, candidate_heights) = transaction_query_plan(&filter, state_ro);
        // Indexed predicates resolve only the selected sparse carrier entries.
        // Kura's live store and lazy-load paths publish ordinary and merge-sidecar
        // fields under one index lock; startup reconciliation finishes before the
        // Kura handle is returned. Adding every historical carrier here would be
        // both unnecessary and an unbounded pre-pagination amplification vector.
        let merge_by_height =
            committed_merge_transactions_by_height(state_ro, candidate_heights.as_ref())?;
        let iter: Box<dyn Iterator<Item = CommittedTransaction> + '_> =
            if let Some(candidate_heights) = candidate_heights {
                Box::new(
                    candidate_heights
                        .into_iter()
                        .rev()
                        .filter_map(|height| state_ro.kura().get_block(height))
                        .flat_map(move |block| {
                            block_committed_transactions_with_merge(&block, &merge_by_height)
                        }),
                )
            } else {
                Box::new(
                    state_ro
                        .all_blocks(nonzero!(1_usize))
                        // Iterate over blocks in descending order (most recent first).
                        .rev()
                        .flat_map(move |block| {
                            block_committed_transactions_with_merge(&block, &merge_by_height)
                        }),
                )
            };
        Ok(iter.filter(move |tx| transaction_filter_applies(&filter, predicate_json.as_ref(), tx)))
    }
}
#[cfg(test)]
/// Transaction-history regression fixtures and tests.
pub(crate) mod tests {
    use super::*;
    use crate::{
        block::BlockBuilder,
        tx::{AcceptedTransaction, tests::*},
    };
    use iroha_crypto::{Hash, HashOf, KeyPair};
    use iroha_data_model::{
        ValidationFail,
        block::{
            BlockExecutionContextBundle, BlockHeader, SignedBlock,
            consensus::{
                CertPhase, LaneBlockCommitment, LaneBlockDescriptorV1, LaneBlockProposalV1,
                LaneBlockQcV1,
            },
        },
        consensus::VALIDATOR_SET_HASH_VERSION_V1,
        merge::{
            MergeExecutionBatch, MergeLaneExecution, MergeLedgerEntry, MergeQuorumCertificate,
        },
        prelude::{
            AccountId, DataSpaceId, DataTriggerSequence, InstructionBox, LaneId, NetworkId, PeerId,
            TransactionBuilder, TransactionEntrypoint, TransactionResult,
        },
        transaction::error::TransactionRejectionReason,
    };
    use std::{
        num::{NonZeroU64, NonZeroUsize},
        sync::Arc,
        time::Duration,
    };
    fn sample_certified_merge_execution_entry(epoch: u64, result_ok: bool) -> MergeLedgerEntry {
        let network_id = NetworkId::from_genesis_hash(
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"merge-query-network")),
        );
        let entrypoints = (0..2)
            .map(|index| {
                let key_pair = KeyPair::random();
                let authority = AccountId::new(key_pair.public_key().clone());
                let mut builder = TransactionBuilder::new(
                    network_id,
                    authority,
                    iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                )
                .with_instructions::<InstructionBox>([]);
                builder.set_creation_time(Duration::from_millis(
                    epoch.saturating_mul(10).saturating_add(index),
                ));
                TransactionEntrypoint::External(builder.sign(key_pair.private_key()))
            })
            .collect::<Vec<_>>();
        let results = (0..entrypoints.len())
            .map(|index| {
                if result_ok {
                    TransactionResult::from(Ok(DataTriggerSequence::default()))
                } else {
                    TransactionResult::from(Err(TransactionRejectionReason::Validation(
                        ValidationFail::NotPermitted(format!(
                            "merge query rejection {epoch}:{index}"
                        )),
                    )))
                }
            })
            .collect::<Vec<_>>();
        let entrypoint_hashes = entrypoints
            .iter()
            .map(|entrypoint| Hash::from(entrypoint.hash()))
            .collect::<Vec<_>>();
        let result_hashes = results
            .iter()
            .map(|result| Hash::from(result.hash()))
            .collect::<Vec<_>>();
        let validator_set = Vec::<PeerId>::new();
        let lane_incarnation = Hash::new(b"merge-query-lane-incarnation");
        let mut descriptor = LaneBlockDescriptorV1 {
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::UNIVERSAL,
            lane_incarnation,
            proposal_height: 2,
            previous_lane_block_height: 0,
            previous_lane_block_descriptor_hash: None,
            lane_block_height: 1,
            lane_block_view: 0,
            subject_hash: Hash::new(b"merge-query-subject"),
            payload_ownership_hash: Hash::new(b"merge-query-ownership"),
            rbc_instance_hash: Hash::new(b"merge-query-rbc"),
            accepted_candidate_indices: vec![0, 1],
            accepted_transaction_hashes: entrypoint_hashes.clone(),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set: validator_set.clone(),
            validator_count: 0,
            min_quorum: 0,
            qc_mode_tag: "merge-query-test".to_owned(),
            descriptor_hash: Hash::prehashed([0; Hash::LENGTH]),
        };
        descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
        let mut proposal = LaneBlockProposalV1 {
            descriptor,
            proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
            payload_block_hint: None,
        };
        proposal.proposal_hash = proposal.computed_proposal_hash();
        let qc = |phase| LaneBlockQcV1 {
            body: proposal.vote_body(phase),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set: validator_set.clone(),
            signers_bitmap: Vec::new(),
            bls_aggregate_signature: Vec::new(),
            payload_availability_qc: None,
        };
        let prepare_qc = qc(CertPhase::Prepare);
        let commit_qc = qc(CertPhase::Commit);
        let settlement_commitment = LaneBlockCommitment {
            block_height: 1,
            lane_id: LaneId::SINGLE,
            lane_incarnation,
            dataspace_id: DataSpaceId::UNIVERSAL,
            tx_count: 0,
            total_local_amount: "0".parse().expect("valid settlement quantity"),
            total_xor_due: "0".parse().expect("valid settlement quantity"),
            total_xor_after_haircut: "0".parse().expect("valid settlement quantity"),
            total_xor_variance: "0".parse().expect("valid settlement quantity"),
            swap_metadata: None,
            receipts: Vec::new(),
            nexus_fee_receipts: Vec::new(),
            native_amx_receipts: Vec::new(),
        };
        let execution = MergeLaneExecution {
            source_bundle: vec![1],
            source_bundle_hash: Hash::new(b"merge-query-source"),
            proposal: proposal.clone(),
            origin_proposal: proposal,
            prepare_qc,
            commit_qc,
            signer_proofs: Vec::new(),
            autonomous_network_id:
                iroha_data_model::NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
                    iroha_data_model::block::BlockHeader,
                >::from_untyped_unchecked(
                    Hash::new(b"merge-query-genesis")
                )),
            autonomous_epoch: 0,
            autonomous_payload_hash: Hash::new(b"merge-query-payload"),
            entrypoint_hashes,
            entrypoints,
            reservation_keys: vec![vec![1], vec![2]],
            routing_plans: vec![vec![3], vec![4]],
            native_amx_receipts: vec![None, None],
            result_hashes,
            results,
            settlement_hash: iroha_data_model::nexus::compute_settlement_hash(
                &settlement_commitment,
            )
            .expect("test settlement should hash canonically"),
            settlement_commitment,
        };
        let lanes = vec![execution];
        let entrypoint_count = 2;
        let entrypoint_merkle_root = crate::merge::merge_execution_entrypoint_merkle_root(&lanes)
            .expect("non-empty entrypoint tree");
        let result_merkle_root = crate::merge::merge_execution_result_merkle_root(&lanes)
            .expect("non-empty result tree");
        let write_set_root = Hash::new(b"merge-query-write-set");
        let base_state_hash = HashOf::from_untyped_unchecked(Hash::new(b"merge-query-base-state"));
        let mut batch = MergeExecutionBatch {
            version: 1,
            base_state_height: 1,
            base_state_hash,
            application_block_header: BlockHeader::new(
                NonZeroU64::new(2).expect("non-zero height"),
                Some(HashOf::from_untyped_unchecked(Hash::new(
                    b"merge-query-previous-block",
                ))),
                None,
                None,
                1,
                0,
            ),
            execution_root: crate::merge::merge_execution_root(&lanes),
            lanes,
            entrypoint_count,
            entrypoint_merkle_root,
            result_merkle_root,
            application_write_set_root: Hash::new(b"merge-query-application-write-set"),
            write_set_root,
            expected_post_state_hash: crate::merge::merge_expected_post_state_hash(
                1,
                base_state_hash,
                write_set_root,
            ),
            batch_hash: Hash::prehashed([0; Hash::LENGTH]),
        };
        batch.batch_hash = crate::merge::merge_execution_batch_hash(&batch);
        let merge_validators = Vec::<PeerId>::new();
        MergeLedgerEntry {
            version: MergeLedgerEntry::VERSION,
            epoch_id: epoch,
            lane_catalog_hash: Hash::new(b"merge-query-catalog"),
            active_lanes: Vec::new(),
            incarnation_root: Hash::new(b"merge-query-incarnations"),
            activation_root: Hash::new(b"merge-query-activations"),
            lane_snapshots: Vec::new(),
            global_state_root: Hash::new(b"merge-query-global-state"),
            merge_qc: MergeQuorumCertificate::new(
                0,
                epoch,
                2,
                HashOf::from_untyped_unchecked(Hash::new(b"merge-query-previous-block")),
                NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
                    b"merge-query-chain",
                ))),
                VALIDATOR_SET_HASH_VERSION_V1,
                HashOf::new(&merge_validators),
                merge_validators,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Hash::new(b"merge-query-message"),
            ),
            execution_batch: Some(batch),
            lane_drain_certificates: Vec::new(),
            queue_plan_admissions: Vec::new(),
        }
    }
    #[test]
    fn certified_merge_projection_is_reverse_ordered_and_rejects_tampering() {
        let entry = sample_certified_merge_execution_entry(1, true);
        let reference = CertifiedMergeLedgerReference::new(&entry);
        let carrier_hash = HashOf::from_untyped_unchecked(Hash::new(b"merge-query-carrier-block"));
        let committed = certified_merge_committed_transactions(carrier_hash, &reference, &entry)
            .expect("canonical merge projection");
        assert_eq!(committed.len(), 2);
        assert_eq!(committed[0].entrypoint_proof.leaf_index(), 1);
        assert_eq!(committed[1].entrypoint_proof.leaf_index(), 0);
        assert!(
            committed.iter().all(|tx| tx.block_hash == carrier_hash
                && tx.verify_certified_merge_inclusion(&reference))
        );
        let mut wrong_reference = reference.clone();
        wrong_reference.encoded_len = wrong_reference.encoded_len.saturating_add(1);
        assert!(
            certified_merge_committed_transactions(carrier_hash, &wrong_reference, &entry).is_err()
        );
        let mut tampered = entry;
        tampered
            .execution_batch
            .as_mut()
            .expect("execution batch")
            .lanes[0]
            .result_hashes[0] = Hash::new(b"forged-result-hash");
        let tampered_reference = CertifiedMergeLedgerReference::new(&tampered);
        assert!(
            certified_merge_committed_transactions(carrier_hash, &tampered_reference, &tampered)
                .is_err()
        );
    }
    /// Build an empty canonical block for transaction-query fixtures.
    pub(crate) fn empty_query_block(previous: Option<&SignedBlock>) -> SignedBlock {
        let mut block: SignedBlock = BlockBuilder::new(Vec::<AcceptedTransaction<'static>>::new())
            .chain(0, previous)
            .sign(&GENESIS_ACCOUNT.key)
            .unpack(|_| {})
            .into();
        block
            .set_transaction_results(Vec::new(), &[], Vec::new())
            .expect("empty query carrier has an exact empty result set");
        block
    }
    /// Build a two-entry certified merge carrier above `previous`.
    pub(crate) fn certified_query_carrier(
        previous: &SignedBlock,
        epoch: u64,
        result_ok: bool,
    ) -> (Arc<SignedBlock>, MergeLedgerEntry) {
        certified_query_carrier_with_entry(
            previous,
            sample_certified_merge_execution_entry(epoch, result_ok),
        )
    }
    fn certified_query_carrier_with_entry(
        previous: &SignedBlock,
        mut entry: MergeLedgerEntry,
    ) -> (Arc<SignedBlock>, MergeLedgerEntry) {
        let mut block = empty_query_block(Some(previous));
        entry.merge_qc.view = block.header().view_change_index();
        entry.merge_qc.carrier_height = block.header().height().get();
        entry.merge_qc.carrier_parent_hash = block
            .header()
            .prev_block_hash()
            .expect("merge query carrier is not genesis");
        let context = block
            .execution_context()
            .cloned()
            .unwrap_or_else(|| BlockExecutionContextBundle::new(Vec::new()))
            .with_merge_entry(CertifiedMergeLedgerReference::new(&entry));
        block.set_execution_context(Some(context));
        (Arc::new(block), entry)
    }
    /// Canonical merge-carrier history shared by transaction query tests.
    pub(crate) struct MergeQueryFixture {
        /// State sandbox containing the seeded canonical history.
        pub(crate) sandbox: Sandbox,
        /// Carrier hash selected by indexed-filter tests.
        pub(crate) target_block_hash: HashOf<BlockHeader>,
        /// Entrypoint hash selected by indexed-filter tests.
        pub(crate) target_entrypoint_hash: HashOf<TransactionEntrypoint>,
        /// Authority selected by indexed-filter tests.
        pub(crate) target_authority: AccountId,
        /// Creation timestamp selected by indexed-filter tests.
        pub(crate) target_timestamp_ms: u64,
        /// Durable sidecar hash for the selected carrier.
        pub(crate) target_entry_hash: HashOf<MergeLedgerEntry>,
        /// Durable sidecar hash for an older, unselected carrier.
        pub(crate) unrelated_entry_hash: HashOf<MergeLedgerEntry>,
    }
    /// Seed sixteen two-entry merge carriers above an empty genesis block.
    pub(crate) fn merge_query_fixture() -> MergeQueryFixture {
        let mut sandbox = Sandbox::default();
        let genesis = Arc::new(empty_query_block(None));
        sandbox
            .state
            .kura()
            .store_block(Arc::clone(&genesis))
            .expect("store merge query genesis");
        sandbox.state.push_block_hash_for_testing(genesis.hash());
        let target_epoch = 9;
        let mut previous = genesis;
        let mut target = None;
        let mut unrelated_entry_hash = None;
        for epoch in 1..=16 {
            let (carrier, entry) =
                certified_query_carrier(previous.as_ref(), epoch, epoch != target_epoch);
            if epoch == 1 {
                unrelated_entry_hash = Some(entry.canonical_hash());
            }
            if epoch == target_epoch {
                let execution = &entry
                    .execution_batch
                    .as_ref()
                    .expect("query fixture has execution batch")
                    .lanes[0];
                target = Some((
                    carrier.hash(),
                    execution.entrypoints[0].hash(),
                    execution.entrypoints[0]
                        .authority_opt()
                        .expect("external query fixture has authority")
                        .clone(),
                    execution.entrypoints[0]
                        .creation_time_ms()
                        .expect("external query fixture has timestamp"),
                    entry.canonical_hash(),
                ));
            }
            sandbox
                .state
                .kura()
                .store_block_with_merge_entry(Arc::clone(&carrier), &entry)
                .expect("store certified merge query carrier");
            sandbox.state.push_block_hash_for_testing(carrier.hash());
            previous = carrier;
        }
        let (
            target_block_hash,
            target_entrypoint_hash,
            target_authority,
            target_timestamp_ms,
            target_entry_hash,
        ) = target.expect("target merge query carrier was seeded");
        MergeQueryFixture {
            sandbox,
            target_block_hash,
            target_entrypoint_hash,
            target_authority,
            target_timestamp_ms,
            target_entry_hash,
            unrelated_entry_hash: unrelated_entry_hash
                .expect("unrelated merge query carrier was seeded"),
        }
    }
    fn execute_single_carrier_query(
        state_ro: &impl StateReadOnly,
        filter: CompoundPredicate<CommittedTransaction>,
    ) -> Vec<CommittedTransaction> {
        state_ro.kura().reset_merge_query_read_counters_for_test();
        let transactions = ValidQuery::execute(FindTransactions, filter, state_ro)
            .expect("indexed merge transaction query succeeds")
            .collect::<Vec<_>>();
        assert_eq!(
            state_ro.kura().merge_query_read_counters_for_test(),
            (0, 0, 1),
            "indexed query must resolve exactly one sidecar and never snapshot complete history"
        );
        transactions
    }
    #[test]
    fn indexed_merge_queries_resolve_only_selected_carrier_sidecars() {
        let fixture = merge_query_fixture();
        let state_view = fixture.sandbox.state.view();
        let by_block = execute_single_carrier_query(
            &state_view,
            CompoundPredicate::<CommittedTransaction>::build(|p| {
                p.equals("block_hash", fixture.target_block_hash.to_string())
            }),
        );
        assert_eq!(by_block.len(), 2);
        assert!(
            by_block
                .iter()
                .all(|transaction| transaction.block_hash == fixture.target_block_hash)
        );
        let by_hash = execute_single_carrier_query(
            &state_view,
            CompoundPredicate::<CommittedTransaction>::build(|p| {
                p.equals(
                    "entrypoint_hash",
                    fixture.target_entrypoint_hash.to_string(),
                )
            }),
        );
        assert_eq!(by_hash.len(), 1);
        assert_eq!(by_hash[0].entrypoint_hash, fixture.target_entrypoint_hash);
        let by_authority = execute_single_carrier_query(
            &state_view,
            CompoundPredicate::<CommittedTransaction>::build(|p| {
                p.equals("authority", fixture.target_authority.to_string())
            }),
        );
        assert_eq!(by_authority.len(), 1);
        assert_eq!(
            by_authority[0].entrypoint.authority_opt(),
            Some(&fixture.target_authority)
        );
        let by_timestamp = execute_single_carrier_query(
            &state_view,
            CompoundPredicate::<CommittedTransaction>::build(|p| {
                p.equals("timestamp_ms", fixture.target_timestamp_ms)
            }),
        );
        assert_eq!(by_timestamp.len(), 1);
        assert_eq!(
            by_timestamp[0].entrypoint.creation_time_ms(),
            Some(fixture.target_timestamp_ms)
        );
        let by_timestamp_range = execute_single_carrier_query(
            &state_view,
            CompoundPredicate::<CommittedTransaction>::from_filters(CommittedTxFilters {
                ts_ge: Some(fixture.target_timestamp_ms),
                ts_le: Some(fixture.target_timestamp_ms),
                ..CommittedTxFilters::default()
            }),
        );
        assert_eq!(by_timestamp_range.len(), 1);
        let by_result = execute_single_carrier_query(
            &state_view,
            CompoundPredicate::<CommittedTransaction>::build(|p| p.equals("result_ok", false)),
        );
        assert_eq!(by_result.len(), 2);
        assert!(
            by_result
                .iter()
                .all(|transaction| transaction.result.as_ref().is_err())
        );
    }
    #[test]
    fn indexed_snapshot_uses_entrypoint_index_and_rejects_unbounded_filters() {
        let fixture = merge_query_fixture();
        let state_view = fixture.sandbox.state.view();
        state_view.kura().reset_merge_query_read_counters_for_test();
        let selected = committed_transactions_indexed_snapshot(
            &state_view,
            CompoundPredicate::from_filters(CommittedTxFilters {
                entry_eq: Some(fixture.target_entrypoint_hash),
                ..CommittedTxFilters::default()
            }),
        )
        .expect("indexed transaction snapshot");
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].entrypoint_hash, fixture.target_entrypoint_hash);
        assert_eq!(
            state_view.kura().merge_query_read_counters_for_test(),
            (0, 0, 1),
            "indexed materialization must resolve only the selected carrier"
        );
        state_view.kura().reset_merge_query_read_counters_for_test();
        let missing_hash = HashOf::from_untyped_unchecked(Hash::new(b"missing-query-entrypoint"));
        let missing = committed_transactions_indexed_snapshot(
            &state_view,
            CompoundPredicate::from_filters(CommittedTxFilters {
                entry_eq: Some(missing_hash),
                ..CommittedTxFilters::default()
            }),
        )
        .expect("missing indexed transaction snapshot");
        assert!(missing.is_empty());
        assert_eq!(
            state_view.kura().merge_query_read_counters_for_test(),
            (0, 0, 0),
            "a complete sparse-index miss must not read a carrier or merge sidecar"
        );
        let error = committed_transactions_indexed_snapshot(&state_view, CompoundPredicate::PASS)
            .expect_err("unbounded transaction history must be rejected");
        assert!(
            error
                .to_string()
                .contains("require a positive indexed filter")
        );
    }
    #[test]
    fn indexed_merge_query_ignores_unselected_corruption_and_fails_on_selected_corruption() {
        let unrelated = merge_query_fixture();
        unrelated
            .sandbox
            .state
            .kura()
            .remove_merge_entry_payload_for_test(unrelated.unrelated_entry_hash);
        let unrelated_view = unrelated.sandbox.state.view();
        let selected = execute_single_carrier_query(
            &unrelated_view,
            CompoundPredicate::<CommittedTransaction>::build(|p| {
                p.equals(
                    "entrypoint_hash",
                    unrelated.target_entrypoint_hash.to_string(),
                )
            }),
        );
        assert_eq!(selected.len(), 1);
        unrelated_view
            .kura()
            .reset_merge_query_read_counters_for_test();
        assert!(
            ValidQuery::execute(
                FindTransactions,
                CompoundPredicate::<CommittedTransaction>::PASS,
                &unrelated_view,
            )
            .is_err(),
            "unindexed complete history must fail closed on any corrupt sidecar"
        );
        let (_, complete_execution_scans, _) =
            unrelated_view.kura().merge_query_read_counters_for_test();
        assert_eq!(complete_execution_scans, 1);
        let selected = merge_query_fixture();
        selected
            .sandbox
            .state
            .kura()
            .remove_merge_entry_payload_for_test(selected.target_entry_hash);
        let selected_view = selected.sandbox.state.view();
        selected_view
            .kura()
            .reset_merge_query_read_counters_for_test();
        assert!(
            ValidQuery::execute(
                FindTransactions,
                CompoundPredicate::<CommittedTransaction>::build(|p| {
                    p.equals(
                        "entrypoint_hash",
                        selected.target_entrypoint_hash.to_string(),
                    )
                }),
                &selected_view,
            )
            .is_err(),
            "selected corrupt sidecar must fail closed before returning an iterator"
        );
        assert_eq!(
            selected_view.kura().merge_query_read_counters_for_test(),
            (0, 0, 1)
        );
    }
    #[test]
    fn fallible_transaction_visitor_reads_only_carriers_needed_by_bounded_page() {
        let fixture = merge_query_fixture();
        let state_view = fixture.sandbox.state.view();
        state_view.kura().reset_merge_query_read_counters_for_test();
        let mut visited = Vec::new();
        let exhausted = visit_committed_transactions(
            &state_view,
            &CompoundPredicate::PASS,
            TransactionHistoryAnchor::capture(&state_view),
            None,
            |_| Ok(()),
            |transaction, matches, _| {
                assert!(matches);
                visited.push(transaction);
                Ok(if visited.len() == 3 {
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                })
            },
        )
        .expect("bounded fallible transaction scan");
        assert!(!exhausted);
        assert_eq!(visited.len(), 3);
        assert_eq!(
            state_view.kura().merge_query_read_counters_for_test(),
            (0, 0, 2),
            "three newest transactions span exactly two two-entry carriers"
        );
        assert!(visited.iter().all(|transaction| {
            transaction
                .merge_inclusion
                .as_ref()
                .is_some_and(|inclusion| inclusion.version == 1)
        }));
    }
    #[test]
    fn bounded_transaction_snapshot_rejects_count_and_byte_amplification() {
        let fixture = merge_query_fixture();
        let state_view = fixture.sandbox.state.view();
        assert_eq!(
            committed_transactions_bounded_snapshot(
                &state_view,
                CompoundPredicate::PASS,
                1,
                u64::MAX,
            )
            .expect_err("declared carrier work must be charged before projection"),
            QueryExecutionFail::GasBudgetExceeded
        );
        assert_eq!(
            committed_transactions_bounded_snapshot(
                &state_view,
                CompoundPredicate::PASS,
                iroha_data_model::query::parameters::MAX_FETCH_SIZE.get(),
                1,
            )
            .expect_err("retained canonical bytes must be bounded"),
            QueryExecutionFail::GasBudgetExceeded
        );
    }
    #[test]
    fn bounded_transaction_visitor_does_not_charge_chain_age_as_retained_memory() {
        let fixture = merge_query_fixture();
        let state_view = fixture.sandbox.state.view();
        let false_filter = CompoundPredicate::<CommittedTransaction>::build(|prototype| {
            prototype.equals("field_that_does_not_exist", true)
        });
        let mut visited = 0_usize;
        let exhausted =
            visit_committed_transactions_bounded(&state_view, false_filter, 2, |_, matches| {
                assert!(!matches);
                visited = visited.saturating_add(1);
                Ok(ControlFlow::Continue(()))
            })
            .expect("each carrier fits independently within the projection bound");
        assert!(exhausted);
        assert!(visited > 2, "the scan crossed multiple bounded carriers");
    }
    #[test]
    fn transaction_budget_rejects_large_sidecar_before_resolve_or_decode() {
        const LARGE_SOURCE_BUNDLE_BYTES: usize = 8 * 1024 * 1024;
        let mut sandbox = Sandbox::default();
        let genesis = Arc::new(empty_query_block(None));
        sandbox
            .state
            .kura()
            .store_block(Arc::clone(&genesis))
            .expect("store large-sidecar query genesis");
        sandbox.state.push_block_hash_for_testing(genesis.hash());
        let mut entry = sample_certified_merge_execution_entry(1, true);
        let batch = entry
            .execution_batch
            .as_mut()
            .expect("query fixture execution batch");
        let source_bundle = vec![0xA5; LARGE_SOURCE_BUNDLE_BYTES];
        batch.lanes[0].source_bundle_hash = Hash::new_from_chunks(&[
            b"iroha:nexus:autonomous-lane-merge-bundle:v1\0",
            &source_bundle,
        ]);
        batch.lanes[0].source_bundle = source_bundle;
        batch.execution_root = crate::merge::merge_execution_root(&batch.lanes);
        batch.batch_hash = crate::merge::merge_execution_batch_hash(batch);
        let (carrier, entry) = certified_query_carrier_with_entry(&genesis, entry);
        let carrier_hash = carrier.hash();
        sandbox
            .state
            .kura()
            .store_block_with_merge_entry(carrier, &entry)
            .expect("store large certified merge sidecar");
        sandbox.state.push_block_hash_for_testing(carrier_hash);
        let state_view = sandbox.state.view();
        state_view.kura().reset_merge_query_read_counters_for_test();
        reset_certified_merge_projection_calls_for_test();
        let false_filter = CompoundPredicate::<CommittedTransaction>::build(|prototype| {
            prototype.equals("field_that_does_not_exist", true)
        });
        let mut charged = 0_u64;
        let err = visit_committed_transactions(
            &state_view,
            &false_filter,
            TransactionHistoryAnchor::capture(&state_view),
            None,
            |projection_work| {
                charged = charged.saturating_add(projection_work);
                if charged > 1 {
                    Err(QueryExecutionFail::GasBudgetExceeded)
                } else {
                    Ok(())
                }
            },
            |_, _, _| panic!("underfunded query must not project a transaction"),
        )
        .expect_err("declared sidecar work exceeds the one-item budget");
        assert_eq!(err, QueryExecutionFail::GasBudgetExceeded);
        assert_eq!(charged, 2, "compact reference declares both entrypoints");
        assert_eq!(
            state_view.kura().merge_query_read_counters_for_test(),
            (0, 0, 0),
            "budget rejection must happen before indexed sidecar resolution or decode"
        );
        assert_eq!(
            certified_merge_projection_calls_for_test(),
            0,
            "budget rejection must happen before proof reconstruction"
        );
    }
    #[test]
    fn fallible_transaction_visitor_exact_scan_is_point_indexed_and_ordered() {
        let fixture = merge_query_fixture();
        let state_view = fixture.sandbox.state.view();
        let expected = committed_transactions_snapshot(&state_view).expect("eager exact baseline");
        state_view.kura().reset_merge_query_read_counters_for_test();
        let mut visited = Vec::new();
        let exhausted = visit_committed_transactions(
            &state_view,
            &CompoundPredicate::PASS,
            TransactionHistoryAnchor::capture(&state_view),
            None,
            |_| Ok(()),
            |transaction, matches, _| {
                assert!(matches);
                visited.push(transaction);
                Ok(ControlFlow::Continue(()))
            },
        )
        .expect("fallible exact transaction scan");
        assert!(exhausted);
        assert_eq!(visited, expected);
        assert_eq!(
            state_view.kura().merge_query_read_counters_for_test(),
            (0, 0, 16),
            "exact scan should point-resolve each carrier without a complete carrier snapshot"
        );
    }
    #[test]
    fn fallible_transaction_visitor_defers_unreached_corruption_but_exact_fails() {
        let fixture = merge_query_fixture();
        fixture
            .sandbox
            .state
            .kura()
            .remove_merge_entry_payload_for_test(fixture.unrelated_entry_hash);
        let state_view = fixture.sandbox.state.view();
        let mut visited = 0_usize;
        let exhausted = visit_committed_transactions(
            &state_view,
            &CompoundPredicate::PASS,
            TransactionHistoryAnchor::capture(&state_view),
            None,
            |_| Ok(()),
            |_, matches, _| {
                assert!(matches);
                visited += 1;
                Ok(if visited == 2 {
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                })
            },
        )
        .expect("newest bounded page should not touch the corrupt oldest carrier");
        assert!(!exhausted);
        assert_eq!(visited, 2);
        let err = visit_committed_transactions(
            &state_view,
            &CompoundPredicate::PASS,
            TransactionHistoryAnchor::capture(&state_view),
            None,
            |_| Ok(()),
            |_, _, _| Ok(ControlFlow::Continue(())),
        )
        .expect_err("exact scan must fail on selected historical corruption");
        assert!(matches!(err, QueryExecutionFail::Conversion(_)));
    }
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
        sandbox.request_transfer("eve", 2, "eve");
        sandbox.request_transfer("eve", 3, "eve");
        sandbox.request_transfer("eve", 4, "eve");
        sandbox.request_transfer("eve", 5, "eve");
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
        let ordinary = block_committed_transactions(block);
        let mut merge_by_height = BTreeMap::new();
        merge_by_height.insert(
            NonZeroUsize::new(
                usize::try_from(block.header().height().get()).expect("height fits usize"),
            )
            .expect("non-zero block height"),
            vec![ordinary[0].clone()],
        );
        let combined = block_committed_transactions_with_merge(block, &merge_by_height);
        assert_eq!(combined.len(), ordinary.len() + 1);
        assert_eq!(combined.last(), ordinary.first());
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
