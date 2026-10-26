//! One-shot current State publication custody for finite full-proof production.

use super::*;
use crate::sumeragi::{
    certified_chain::{ChainReadError, CommittedBlock},
    finality::{
        FinalityDecodeWorkspace, NativeFinalityProofBudget, NativeFinalityProofInterval, NativeFinalityProofIntervalError,
        NativeFinalityProofIntervalLimits, NativeFinalityProofSource, build_proof_interval,
    },
    startup::GENESIS_HEIGHT,
};
use iroha_data_model::sumeragi_finality::{MAX_FINALITY_BLOCK_BYTES, SumeragiFinalityProof};
use std::{
    num::{NonZeroU64, NonZeroUsize},
    sync::atomic::AtomicBool,
    time::Instant,
};

/// Target-height increment between attempts to retain optional prefix checkpoints.
const LADDER_STEP: u64 = 64;

/// Original State acquisition or finite historical proof production did not complete.
#[derive(Debug, thiserror::Error)]
pub enum FinalityProofIntervalReadError {
    /// Preserve the physical State writer/reader's original release observation.
    #[error(transparent)]
    StateView(#[from] StateViewError),
    /// Preserve exact native, portable, resource, source and original stop failures.
    #[error(transparent)]
    Proof(#[from] NativeFinalityProofIntervalError),
}

impl State {
    /// Produce finite historical full proofs from the original current State hash generation.
    ///
    /// Configured network/chain and versioned hash custody come from this exact State.
    /// Both generation captures are one-shot; active/changed publication returns the
    /// original Busy release observation. A stable appended tip is allowed if the
    /// captured final hash remains current. No World view or writer fence is retained
    /// during verification or any caller wait. Native prefix checks and a raw-source join
    /// of every frame the walk read precede return; captured hashes alone never certify
    /// authority. The walk resumes from this State's nearest completed prefix below the
    /// first height after rechecking that pinned frame, and retains its own on success.
    ///
    /// The caller retains its actual admitted worker/response/codec owners. The
    /// returned immutable response keeps all destination charges from that same pool.
    /// The native producer prepays its bounded live graph workspace before decoding.
    ///
    /// # Errors
    /// Preserves State Busy, exact original native/portable refusal and stop policy.
    pub fn read_finality_proof_interval(
        &self,
        owner: &NativeFinalityProofBudget,
        limits: &NativeFinalityProofIntervalLimits,
        cancelled: &AtomicBool,
    ) -> Result<NativeFinalityProofInterval, FinalityProofIntervalReadError> {
        self.read_finality_proof_interval_after_read(owner, limits, cancelled, || {})
    }

    fn read_finality_proof_interval_after_read(
        &self,
        owner: &NativeFinalityProofBudget,
        limits: &NativeFinalityProofIntervalLimits,
        cancelled: &AtomicBool,
        after_read: impl FnOnce(),
    ) -> Result<NativeFinalityProofInterval, FinalityProofIntervalReadError> {
        owner.frames().with_deferred_refund_notifications(|_| {
            limits.check(cancelled)?;
            let mut releases = self.block_hashes.reader_release_batch();
            let (hashes, _) = self.try_event_source(&mut releases, || {})?;
            let final_height = limits.final_height();
            let index = usize::try_from(final_height)
                .ok()
                .and_then(|height| height.checked_sub(1))
                .ok_or(NativeFinalityProofIntervalError::Request)?;
            let expected = hashes.get(index).copied();
            let source = NativeFinalityProofSource::new(
                self.chain_id_ref(),
                self.network_id_ref(),
                &hashes,
                &self.kura,
            )
            .with_checkpoints(
                &self.finality_prefix_checkpoints,
                &self.ivm_execution_budget,
            );
            let retained = build_proof_interval(&source, owner, limits, cancelled)?;
            after_read();
            limits.check(cancelled)?;
            #[cfg(not(all(test, sumeragi_core_mutation = "HC214")))]
            {
                let mut current_releases = self.block_hashes.reader_release_batch();
                let (current_hashes, _) = self.try_event_source(&mut current_releases, || {})?;
                if expected.is_none() || current_hashes.get(index).copied() != expected {
                    return Err(NativeFinalityProofIntervalError::Source {
                        height: final_height,
                    }
                    .into());
                }
            }
            #[cfg(all(test, sumeragi_core_mutation = "HC214"))]
            let _ = expected;
            Ok(retained)
        })
    }

    /// Borrowable receipt of a still-current tip, certified through this State's ladder.
    ///
    /// The nearest retained prefix is strictly below `height`; a repeated same-height
    /// request may reverify the final gap. Cold walks try target increments of 64.
    /// Optional cache refusal or an invalid checkpoint can require a longer direct
    /// walk, still subject to the original proof deadline and capacity owners.
    ///
    /// After proof completion, a one-shot State view must still name this height as
    /// its tip. Receipt source bytes and decoder work use the same request's frame
    /// and graph corridors. The move-only returned owner retains that graph funding;
    /// access borrows the receipt. Cloning public data needs separate admission.
    /// A later live-State consumer must still authenticate its own current cut.
    ///
    /// # Errors
    /// Preserves original Busy, native resource/source errors and stop policy. A moved
    /// tip or receipt whose wire differs from the verified proof is a source refusal.
    pub fn read_certified_ladder_receipt(
        &self,
        owner: &NativeFinalityProofBudget,
        height: NonZeroU64,
        maximum_proof_bytes: NonZeroUsize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<impl AsRef<CommittedBlock> + use<>, FinalityProofIntervalReadError> {
        self.read_certified_ladder_receipt_in_steps(
            owner,
            height,
            maximum_proof_bytes,
            deadline,
            cancelled,
            LADDER_STEP,
        )
    }

    fn read_certified_ladder_receipt_in_steps(
        &self,
        owner: &NativeFinalityProofBudget,
        height: NonZeroU64,
        maximum_proof_bytes: NonZeroUsize,
        deadline: Instant,
        cancelled: &AtomicBool,
        step: u64,
    ) -> Result<FundedLadderReceipt, FinalityProofIntervalReadError> {
        let single = |target: NonZeroU64| {
            NativeFinalityProofIntervalLimits::new(
                target,
                target,
                NonZeroUsize::new(MAX_FINALITY_BLOCK_BYTES)
                    .expect("positive canonical block ceiling"),
                maximum_proof_bytes,
                deadline,
            )
        };
        let final_limits = single(height)?;
        final_limits.check(cancelled)?;
        let mut floor = self.ladder_floor(height.get());
        while height.get() - floor > step {
            let target = NonZeroU64::new(floor + step).expect("a step above its floor");
            drop(self.read_finality_proof_interval(owner, &single(target)?, cancelled)?);
            let next = self.ladder_floor(height.get());
            if next < target.get() {
                // Optional retention kept nothing: one direct walk replaces further
                // steps that would each restart below this one.
                break;
            }
            floor = next;
        }
        let interval = self.read_finality_proof_interval(owner, &final_limits, cancelled)?;
        self.finish_certified_ladder_receipt(owner, &interval, &final_limits, cancelled)
    }

    // Final receipt work retains a separate workspace from the same admitted graph
    // corridor. The proof's workspace has already retired; its output stays paid.
    fn finish_certified_ladder_receipt(
        &self,
        owner: &NativeFinalityProofBudget,
        interval: &NativeFinalityProofInterval,
        limits: &NativeFinalityProofIntervalLimits,
        cancelled: &AtomicBool,
    ) -> Result<FundedLadderReceipt, FinalityProofIntervalReadError> {
        limits.check(cancelled)?;
        let proof = interval
            .proof(0)
            .ok_or(NativeFinalityProofIntervalError::Request)?;
        let height = limits.final_height();
        let view = self.try_view_once()?;
        limits.check(cancelled)?;
        // A current-tip consumer must not start an unbounded ancestry suffix when
        // a commit occurred during certification. Its eventual snapshot would refuse.
        if u64::try_from(view.height()).ok() != Some(height) {
            return Err(NativeFinalityProofIntervalError::Source { height }.into());
        }
        let index = usize::try_from(height)
            .ok()
            .and_then(NonZeroUsize::new)
            .ok_or(NativeFinalityProofIntervalError::Request)?;
        let workspace = owner.workspace().map_err(NativeFinalityProofIntervalError::from)?;
        let target = workspace.target_context().map_err(NativeFinalityProofIntervalError::from)?;
        let read_budget = CanonicalHistoryReadBudget::new(owner.frames().clone(), target.clone());
        let receipt = workspace.stage(|| {
            let mut original_refusal = None;
            let mut reads = 0u64;
            let result = read_budget.with(|| {
                view.canonical_history().with_read_budget(&read_budget).executed_receipt(
                    index,
                    |work, wire_bytes| {
                        let refusal = limits.check(cancelled).err().or_else(|| {
                            reads = reads.saturating_add(work);
                            (reads != 1 || wire_bytes > MAX_FINALITY_BLOCK_BYTES as u64)
                                .then_some(NativeFinalityProofIntervalError::Request)
                        });
                        if let Some(original) = refusal {
                            original_refusal = Some(original);
                            // Internal stop token only. Restore the exact original
                            // before exposing any result from this shared reader.
                            return Err(iroha_data_model::query::error::QueryExecutionFail::GasBudgetExceeded.into());
                        }
                        Ok(())
                    },
                )
            });
            if let Some(original) = original_refusal {
                return Err(original);
            }
            let receipt = result.map_err(|error| {
                NativeFinalityProofIntervalError::from(error.map_rejection(|error| {
                    ChainReadError::Malformed { height, reason: error.to_string() }
                }))
            })?;
            limits.check(cancelled)?;
            if !receipt_carries_verified_wire(&receipt, proof) {
                return Err(NativeFinalityProofIntervalError::Source { height });
            }
            limits.check(cancelled)?;
            Ok(receipt)
        })?;
        drop(view);
        Ok(FundedLadderReceipt { receipt, _target: target, _workspace: workspace })
    }

    /// Height of the nearest retained prefix strictly below `height`, else signed genesis.
    fn ladder_floor(&self, height: u64) -> u64 {
        self.ivm_execution_budget
            .with_deferred_refund_notifications(|_| {
                self.finality_prefix_checkpoints
                    .resume_point(height, &self.ivm_execution_budget)
                    .map_or(GENESIS_HEIGHT, |checkpoint| checkpoint.height())
            })
    }
}

// Field order keeps receipt graphs and decoder controls inside their original
// graph reservation through natural drop and unwinding. No owned extraction API.
struct FundedLadderReceipt {
    receipt: CommittedBlock,
    _target: norito::core::DecodeBudgetContext,
    _workspace: FinalityDecodeWorkspace,
}
impl AsRef<CommittedBlock> for FundedLadderReceipt {
    fn as_ref(&self) -> &CommittedBlock {
        &self.receipt
    }
}

/// Whether `receipt` is the frame `proof` verified: the same canonical wire length and digest.
fn receipt_carries_verified_wire(receipt: &CommittedBlock, proof: &SumeragiFinalityProof) -> bool {
    receipt
        .block()
        .canonical_wire_identity()
        .is_ok_and(|(length, digest)| {
            u64::try_from(proof.block_wire.len()) == Ok(length)
                && Hash::new(&proof.block_wire) == digest
        })
}

#[cfg(test)]
mod tests;
