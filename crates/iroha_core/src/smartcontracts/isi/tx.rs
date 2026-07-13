//! Implementations for transaction queries.

use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroUsize,
};

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

fn certified_merge_committed_transactions(
    carrier_hash: HashOf<BlockHeader>,
    reference: &CertifiedMergeLedgerReference,
    entry: &MergeLedgerEntry,
) -> Result<Vec<CommittedTransaction>, QueryExecutionFail> {
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

fn committed_merge_transactions_by_height(
    state_ro: &impl StateReadOnly,
) -> Result<BTreeMap<NonZeroUsize, Vec<CommittedTransaction>>, QueryExecutionFail> {
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

/// Materialize complete canonical transaction history in newest-first order.
///
/// This includes transactions executed by globally ordered certified merge
/// sidecars. The durable sparse carrier index and every full sidecar are
/// revalidated before any history is returned, so callers never receive a
/// cache-truncated or partially fabricated view.
///
/// # Errors
///
/// Returns [`QueryExecutionFail::Conversion`] when durable carrier, block, or
/// sidecar evidence is unavailable, malformed, or mutually inconsistent.
pub fn committed_transactions_snapshot(
    state_ro: &impl StateReadOnly,
) -> Result<Vec<CommittedTransaction>, QueryExecutionFail> {
    let merge_by_height = committed_merge_transactions_by_height(state_ro)?;
    Ok(state_ro
        .all_blocks(nonzero!(1_usize))
        .rev()
        .flat_map(|block| block_committed_transactions_with_merge(&block, &merge_by_height))
        .collect())
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

        let merge_by_height = committed_merge_transactions_by_height(state_ro)?;
        if let Some(candidate_heights) = candidate_heights.as_mut() {
            // The ordinary block index and sparse merge-carrier index are
            // published by separate durable steps. Always union certified
            // carrier heights before applying the predicate so a concurrent
            // index refresh cannot create a false-negative query result.
            candidate_heights.extend(merge_by_height.keys().copied());
        }

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
    use std::num::{NonZeroU64, NonZeroUsize};

    use iroha_crypto::{Hash, HashOf, KeyPair};
    use iroha_data_model::{
        block::{
            BlockHeader,
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
            AccountId, ChainId, DataSpaceId, DataTriggerSequence, InstructionBox, LaneId, PeerId,
            TransactionBuilder, TransactionEntrypoint, TransactionResult,
        },
    };

    use super::*;
    use crate::tx::tests::*;

    fn sample_certified_merge_execution_entry() -> MergeLedgerEntry {
        let chain_id: ChainId = "merge-query-projection".parse().expect("chain id");
        let entrypoints = (0..2)
            .map(|_| {
                let key_pair = KeyPair::random();
                let authority = AccountId::new(key_pair.public_key().clone());
                TransactionEntrypoint::External(
                    TransactionBuilder::new(chain_id.clone(), authority)
                        .with_instructions::<InstructionBox>([])
                        .sign(key_pair.private_key()),
                )
            })
            .collect::<Vec<_>>();
        let results = (0..entrypoints.len())
            .map(|_| TransactionResult::from(Ok(DataTriggerSequence::default())))
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
            autonomous_chain_id_hash: Hash::new(b"merge-query-chain"),
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
            epoch_id: 1,
            lane_catalog_hash: Hash::new(b"merge-query-catalog"),
            active_lanes: Vec::new(),
            incarnation_root: Hash::new(b"merge-query-incarnations"),
            activation_root: Hash::new(b"merge-query-activations"),
            lane_snapshots: Vec::new(),
            global_state_root: Hash::new(b"merge-query-global-state"),
            merge_qc: MergeQuorumCertificate::new(
                0,
                1,
                2,
                HashOf::from_untyped_unchecked(Hash::new(b"merge-query-previous-block")),
                Hash::new(b"merge-query-chain"),
                VALIDATOR_SET_HASH_VERSION_V1,
                HashOf::new(&merge_validators),
                merge_validators,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Hash::new(b"merge-query-message"),
            ),
            execution_batch: Some(batch),
        }
    }

    #[test]
    fn certified_merge_projection_is_reverse_ordered_and_rejects_tampering() {
        let entry = sample_certified_merge_execution_entry();
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
