//! Complete typed-output parity and source-aware replay diagnostics.
//!
//! Structural equality is not execution authority. The replay caller separately
//! authenticates exact stored wire, recomputed execution commitment and WSV
//! checkpoint before it may publish its isolated replay result.

use eyre::{Result, WrapErr, eyre};
use iroha_crypto::HashOf;
use iroha_data_model::{
    account::AccountId,
    block::{SignedBlock, execution_output::ExecutionOutputV1},
    transaction::{TransactionEntrypoint, error::TransactionRejectionReason},
};

/// Preserve validation diagnostics even when execution stopped before outputs.
pub(super) fn replay_validation_output_errors(block: &SignedBlock) -> Vec<String> {
    if !block.has_results() {
        return Vec::new();
    }
    block
        .execution_outputs()
        .iter()
        .enumerate()
        .filter_map(|(index, output)| {
            let error = output.result().as_ref().err()?;
            let owner = match output {
                ExecutionOutputV1::Network(row) => format!("network#{}", row.input_index),
                ExecutionOutputV1::Pipeline(row) => format!(
                    "pipeline {:?}/{} trigger={}",
                    row.invocation.event,
                    row.invocation.candidate_index,
                    row.invocation.trigger.trigger_id,
                ),
                ExecutionOutputV1::Time(row) => format!(
                    "time#{} trigger={}",
                    row.invocation.schedule_index, row.invocation.trigger.trigger_id,
                ),
            };
            Some(format!(
                "output#{index} {owner}: {error}; details: {error:?}"
            ))
        })
        .collect()
}

/// Compare complete execution projections after independent replay/finality checks.
pub(super) fn ensure_replayed_results_match_committed(
    height: u64,
    committed: &SignedBlock,
    replayed: &SignedBlock,
) -> Result<()> {
    if !committed.has_results() {
        return Err(eyre!(
            "committed block #{height} does not contain stored execution results"
        ));
    }
    if !replayed.has_results() {
        return Err(eyre!(
            "replayed block #{height} did not produce execution results"
        ));
    }
    for (owner, block) in [("committed", committed), ("replayed", replayed)] {
        if block.header().height().get() != height {
            return Err(eyre!(
                "{owner} block height differs from replay height #{height}"
            ));
        }
        // Includes proposal commitments, complete Network joins, all internal
        // rows, result/receipt/completion shape and FASTPQ vector/key structure.
        block
            .validate_output_merkle_cache()
            .map_err(|error| eyre!(error))
            .wrap_err_with(|| {
                format!("{owner} block #{height} output/source/cache validation failed")
            })?;
    }
    if committed.header() != replayed.header() {
        return Err(eyre!("replayed block #{height} proposal header mismatch"));
    }
    if !committed.signatures().eq(replayed.signatures()) {
        return Err(eyre!(
            "replayed block #{height} proposal signature sequence mismatch"
        ));
    }
    if !committed
        .network_entrypoints()
        .eq(replayed.network_entrypoints())
    {
        return Err(eyre!(
            "replayed block #{height} Network source sequence mismatch: committed_len={} replayed_len={}",
            committed.network_entrypoint_count(),
            replayed.network_entrypoint_count(),
        ));
    }
    let committed_rows = committed.execution_outputs();
    let replayed_rows = replayed.execution_outputs();
    let committed_root = committed.output_merkle_commitment();
    let replayed_root = replayed.output_merkle_commitment();
    if committed_root != replayed_root || committed_rows != replayed_rows {
        let first_mismatch = committed_rows
            .iter()
            .zip(replayed_rows)
            .position(|(left, right)| left != right)
            .or_else(|| {
                (committed_rows.len() != replayed_rows.len())
                    .then_some(committed_rows.len().min(replayed_rows.len()))
            });
        // Borrow only the differing rows; never clone every result to diagnose
        // one mismatch. Full row comparison retains receipts and completions.
        return Err(eyre!(
            "replayed block #{height} typed execution output mismatch: committed_root={committed_root:?} replayed_root={replayed_root:?} committed_len={} replayed_len={} first_mismatch={first_mismatch:?} committed={:?} replayed={:?}",
            committed_rows.len(),
            replayed_rows.len(),
            first_mismatch.and_then(|index| committed_rows.get(index)),
            first_mismatch.and_then(|index| replayed_rows.get(index)),
        ));
    }
    // These fields are outside the output Merkle tree. Exact executed-wire
    // finality authenticates them; the parity seam must compare them as well.
    macro_rules! same_metadata {
        ($projection:ident, $label:literal) => {
            if committed.$projection() != replayed.$projection() {
                return Err(eyre!(
                    concat!("replayed block #{} ", $label, " mismatch"),
                    height
                ));
            }
        };
    }
    same_metadata!(committed_fragment_count, "committed fragment count");
    same_metadata!(fastpq_transcripts, "FASTPQ transcripts");
    same_metadata!(axt_envelopes, "AXT envelopes");
    same_metadata!(axt_policy_snapshot, "AXT policy snapshot");
    same_metadata!(axt_transitioned_dataspaces, "AXT transitioned dataspaces");
    same_metadata!(lane_finality_statements, "lane finality statements");
    Ok(())
}

#[derive(Debug)]
struct SignedSourceDiagnostic<'a> {
    input_index: u32,
    entrypoint_hash: HashOf<TransactionEntrypoint>,
    execution_call_hash: HashOf<TransactionEntrypoint>,
    authority: &'a AccountId,
    rejection: Option<&'a TransactionRejectionReason>,
}

fn replayed_signed_sources(block: &SignedBlock) -> Result<Vec<SignedSourceDiagnostic<'_>>> {
    block
        .validate_output_merkle_cache()
        .map_err(|error| eyre!(error))
        .wrap_err("cannot summarize malformed replayed outputs")?;
    block
        .network_entrypoints()
        .enumerate()
        .filter_map(|(index, input)| {
            let signed = match input {
                TransactionEntrypoint::External(signed) => signed,
                TransactionEntrypoint::SealedReveal(reveal) => reveal.signed_transaction(),
                // Preserve the existing signed-transaction summary subset. This
                // source still participates in complete validation and parity.
                TransactionEntrypoint::SealedCommitment(_) => return None,
            };
            Some((|| {
                let input_index =
                    u32::try_from(index).wrap_err("replay Network index exceeds u32")?;
                let (_, output) = block.network_output_at(input_index).ok_or_else(|| {
                    eyre!("replay Network source #{input_index} has no exact output join")
                })?;
                Ok(SignedSourceDiagnostic {
                    input_index,
                    entrypoint_hash: input.hash(),
                    execution_call_hash: input.execution_call_hash(),
                    authority: signed.authority(),
                    rejection: output.result.as_ref().err(),
                })
            })())
        })
        .collect()
}

/// Log signed Network sources using explicit joins; internal rows are not transactions.
pub(super) fn log_replayed_signed_sources(height: u64, block: &SignedBlock) -> Result<()> {
    let sources = replayed_signed_sources(block)?;
    let tx_count = sources.len();
    let rejected: Vec<_> = sources
        .iter()
        .filter_map(|source| source.rejection.map(|error| (source.input_index, error)))
        .collect();
    let tx_hashes: Vec<_> = sources
        .iter()
        .map(|source| {
            (
                source.entrypoint_hash,
                source.execution_call_hash,
                source.authority,
            )
        })
        .collect();
    iroha_logger::info!(
        height,
        txs = tx_count,
        rejected = rejected.len(),
        ?tx_hashes,
        "replayed block from Kura"
    );
    if !rejected.is_empty() {
        iroha_logger::warn!(
            height,
            txs = tx_count,
            rejected = rejected.len(),
            ?rejected,
            "transaction replays were rejected while rebuilding state"
        );
    }
    Ok(())
}

#[cfg(test)]
#[path = "replay_outputs_tests.rs"]
mod tests;
