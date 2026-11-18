//! Read a complete ballot interval from the original signed genesis and its certified successors.

use crate::{
    corpus::closing_ms_v1,
    replay::{
        FullVerificationRequestV1, FullVerificationV1, RecordedBallotResultV1, ReplayBallotV1,
        ReplayBlockV1, ReplaySourceErrorV1, verify_full_v1,
    },
};
use iroha_data_model::{
    block::{
        consensus::SumeragiRootScope,
        execution_output::{
            ExecutionOutputV1, TransactionRejectionRef, TransactionResultRef,
            governance_start_row_count_v1,
        },
    },
    governance::{
        GovernanceAnchorV1,
        anonymous_ballot::{
            GovernanceBlockStartEventV1 as Event, GovernanceOutputAnchorV1 as Anchor,
        },
    },
    sumeragi_finality::{
        FinalityReadError, SumeragiFinalityProof, SumeragiFinalityVerifier, VerifiedSumeragiBlock,
    },
    transaction::{TransactionEntrypoint, TransactionResult, error::TransactionRejectionReason},
};

/// Why the authenticated reader could not supply the complete interval to the ballot reducer.
#[derive(Debug, thiserror::Error)]
pub enum FinalizedIntervalErrorV1 {
    /// A requested full certificate and its execution were unavailable.
    #[error(transparent)]
    Source(#[from] ReplaySourceErrorV1),
    /// The original verifier rejected the next proof or could not complete its checks.
    #[error(transparent)]
    Finality(#[from] FinalityReadError),
    /// Authenticated inputs, results or governance rows disagree with the requested evidence.
    #[error("ballot history at height {height}: {reason}")]
    Evidence {
        /// Height of the first inconsistent or missing row.
        height: u64,
        /// The failed source or anchor check.
        reason: &'static str,
    },
    /// Local storage for one block's ballots could not be reserved.
    #[error(transparent)]
    Allocation(#[from] std::collections::TryReserveError),
}

fn evidence(height: u64, reason: &'static str) -> FinalizedIntervalErrorV1 {
    FinalizedIntervalErrorV1::Evidence { height, reason }
}

fn governance(
    block: &VerifiedSumeragiBlock,
    request: &FullVerificationRequestV1,
    seal: GovernanceAnchorV1,
) -> Result<(), FinalizedIntervalErrorV1> {
    let height = block.height();
    let time = u64::try_from(block.header().creation_time().as_millis())
        .map_err(|_| evidence(height, "block timestamp overflow"))?;
    let context = &request.context;
    let anchors = request.anchors;
    let mut seen = [false; 4];
    for output in block.block().execution_outputs() {
        let ExecutionOutputV1::GovernanceBlockStart(row) = output else {
            break;
        };
        let (index, same) = match &row.event {
            Event::SealedCredentialListPublished(event)
                if event.body_context == *context.body_context() =>
            {
                (
                    0,
                    event == &request.seal
                        && row.anchor == Anchor::Pass(seal)
                        && time == seal.timestamp_ms,
                )
            }
            Event::CastingOpened(event)
                if event.scope == *context.scope()
                    || event.body_context == *context.body_context() =>
            {
                (
                    1,
                    event.scope == *context.scope()
                        && event.body_context == *context.body_context()
                        && event.body_instance_id == *context.body_instance_id()
                        && event.policy_hash == *context.policy_hash()
                        && seal
                            .timestamp_ms
                            .checked_add(context.policy().binding.vote_notice_ms)
                            .is_some_and(|earliest| earliest <= event.opening_at_ms)
                        && event.opening_at_ms <= time
                        && row.anchor == Anchor::Pass(anchors.opening)
                        && time == anchors.opening.timestamp_ms,
                )
            }
            Event::CastingClockEstablished(event) if event.scope == *context.scope() => (
                2,
                event.casting_window_ms == context.policy().binding.casting_window_ms
                    && closing_ms_v1(time, event.casting_window_ms) == Some(event.closing_ms)
                    && row.anchor
                        == Anchor::CastingClockPrelude {
                            height,
                            timestamp_ms: time,
                        }
                    && time == anchors.casting_clock_timestamp_ms,
            ),
            Event::BallotClosed(record) if record.scope == *context.scope() => (
                3,
                record == &request.closure_record
                    && row.anchor == Anchor::Pass(anchors.closure)
                    && time == anchors.closure.timestamp_ms,
            ),
            _ => continue,
        };
        if !same || seen[index] {
            return Err(evidence(
                height,
                "inconsistent or repeated ballot governance event",
            ));
        }
        seen[index] = true;
    }
    let heights = [
        seal.height,
        anchors.opening.height,
        anchors.casting_clock_height,
        anchors.closure.height,
    ];
    if heights.map(|at| at == height) != seen {
        return Err(evidence(
            height,
            "certified governance events differ from the claimed anchors",
        ));
    }
    Ok(())
}

fn replay_block(block: &VerifiedSumeragiBlock) -> Result<ReplayBlockV1, FinalizedIntervalErrorV1> {
    // The finality verifier authenticates the full executed wire and independently checks
    // both counted Merkle commitments, including their complete input/output leaf lists.
    let height = block.height();
    let signed = block.block();
    let outputs = signed.execution_outputs();
    let network_start = governance_start_row_count_v1(outputs);
    let mut ballots = Vec::new();
    for (index, input) in signed.network_entrypoints().enumerate() {
        let TransactionEntrypoint::AnonymousBallot(carrier) = input else {
            continue;
        };
        let input_index =
            u32::try_from(index).map_err(|_| evidence(height, "ballot input index overflow"))?;
        let position = network_start
            .checked_add(index)
            .ok_or_else(|| evidence(height, "ballot output index overflow"))?;
        let row = match outputs.get(position) {
            Some(ExecutionOutputV1::Network(row)) => row,
            Some(ExecutionOutputV1::NetworkWithLifecycle(row))
                if row.trigger_lifecycle.effects().is_empty() =>
            {
                &row.execution
            }
            _ => return Err(evidence(height, "ballot has no ordinary Network output")),
        };
        if row.input_index != input_index || !row.completions.is_empty() {
            return Err(evidence(
                height,
                "ballot source index or trigger completions differ",
            ));
        }
        let (recorded, expected) = match TransactionResultRef::from(&row.result).as_ref() {
            Ok(_) => (RecordedBallotResultV1::Accepted, Ok(Vec::new())),
            Err(TransactionRejectionRef::Baseline(
                TransactionRejectionReason::AnonymousBallot(reason),
            )) => (
                RecordedBallotResultV1::Rejected(*reason),
                Err(TransactionRejectionReason::AnonymousBallot(*reason)),
            ),
            _ => return Err(evidence(height, "unexpected ballot rejection family")),
        };
        if row.result != TransactionResult::new(expected) {
            return Err(evidence(height, "ballot result carries unrelated effects"));
        }
        let mut envelope = Vec::new();
        envelope.try_reserve_exact(carrier.as_bytes().len())?;
        envelope.extend_from_slice(carrier.as_bytes());
        ballots.try_reserve(1)?;
        ballots.push(ReplayBallotV1 {
            input_index,
            envelope,
            recorded,
        });
    }
    Ok(ReplayBlockV1 {
        height,
        timestamp_ms: u64::try_from(block.header().creation_time().as_millis())
            .map_err(|_| evidence(height, "block timestamp overflow"))?,
        ballots,
    })
}

/// Read and verify the target seal, opening, casting clock, closure and every ballot in the interval.
///
/// Supply an original independently selected global genesis verifier, optionally already advanced
/// through a prefix ending before the seal. The reader requests every remaining height including
/// blocks without ballots and consumes the verifier. It retains the current block, the verifier's
/// three recent decisions and the ballot reducer's bounded corpus. The caller supplies independently
/// checked other-scope contexts and the attempt facts used by the ballot reducer.
///
/// # Errors
/// Preserves certificate/availability refusal and source failures, and reports a wrong root,
/// noncontiguous origin, missing anchor, substituted result or additional execution effects.
pub fn verify_finalized_ballot_interval_v1(
    mut verifier: SumeragiFinalityVerifier,
    request: &FullVerificationRequestV1,
    seal: GovernanceAnchorV1,
    mut fetch: impl FnMut(u64) -> Result<SumeragiFinalityProof, ReplaySourceErrorV1>,
) -> Result<FullVerificationV1, FinalizedIntervalErrorV1> {
    let opening = request.anchors.opening;
    let clock = request.anchors.casting_clock_height;
    let closure = request.anchors.closure;
    let prefix = verifier.authenticated_height().unwrap_or(0);
    if verifier.root_scope() != SumeragiRootScope::Global
        || verifier.initial_epoch().network_id != *request.context.network_id()
        || prefix >= seal.height
        || seal.height < 2
        || (seal.height, seal.position) >= (opening.height, opening.position)
        || opening.height.checked_add(1) != Some(clock)
        || clock > closure.height
    {
        return Err(evidence(
            seal.height,
            "wrong selected root, prefix or interval anchors",
        ));
    }
    verifier.retain_recent_decisions(3);
    let mut next = |height| -> Result<VerifiedSumeragiBlock, FinalizedIntervalErrorV1> {
        let proof = fetch(height)?;
        let block = verifier.verify(&proof)?;
        verifier.retain_recent_decisions(3);
        governance(&block, request, seal)?;
        Ok(block)
    };
    for height in prefix + 1..opening.height {
        next(height)?;
    }
    let mut read_error = None;
    let blocks = (opening.height..=closure.height).map_while(|height| {
        match next(height).and_then(|block| replay_block(&block)) {
            Ok(block) => Some(Ok(block)),
            Err(error) => {
                read_error = Some(error);
                None
            }
        }
    });
    let verdict = verify_full_v1(request, blocks);
    read_error.map_or(Ok(verdict), Err)
}

#[cfg(test)]
#[path = "finalized_interval_tests.rs"]
mod tests;
