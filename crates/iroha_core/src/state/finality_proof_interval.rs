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

// Park on a refused capture's own release within the request's deadline and stop.
// A one-shot read, or a wait that outlasts the deadline, keeps the original refusal.
fn wait_for_release(
    waiter: Option<&mut StateViewWaiter>,
    release: iroha_allocation::release::ReleaseWait,
    limits: &NativeFinalityProofIntervalLimits,
    cancelled: &AtomicBool,
) -> Result<(), FinalityProofIntervalReadError> {
    let Some(waiter) = waiter else {
        return Err(StateViewError::Busy(release).into());
    };
    if waiter.wait(&release, limits.deadline(), cancelled) {
        return Ok(());
    }
    match limits.check(cancelled) {
        Err(NativeFinalityProofIntervalError::Cancelled) => {
            Err(NativeFinalityProofIntervalError::Cancelled.into())
        }
        _ => Err(StateViewError::Busy(release).into()),
    }
}

impl State {
    /// Heights between the verified prefixes a climbing read or the startup ladder retains.
    pub const FINALITY_LADDER_STEP: u64 = LADDER_STEP;

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
    /// A first height more than one ladder step above that prefix is approached in
    /// retained steps first. Each step is the same walk with its own one-shot captures,
    /// so a deadline, stop or Busy refusal keeps every completed step for the next read.
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
        self.read_finality_proof_interval_climbing(owner, limits, cancelled, None)
    }

    /// Produce the same interval, waiting out busy State captures within its limits.
    ///
    /// A busy capture before the walk parks this worker on `waiter` until its exact
    /// publisher or reader releases, then captures again. A busy recheck after the walk
    /// waits the same way and repeats only the recheck: the verified interval keeps its
    /// destination charges and is never walked twice. Every wait shares the limits'
    /// absolute deadline and `cancelled`; whoever sets `cancelled` must also unpark this
    /// worker. No State view or reader notice is held while it is parked. Every ladder
    /// step below the first height waits the same way.
    ///
    /// # Errors
    /// As [`Self::read_finality_proof_interval`], except that State Busy, with its
    /// original release observation, is returned only once the deadline has passed.
    pub fn read_finality_proof_interval_waiting(
        &self,
        owner: &NativeFinalityProofBudget,
        limits: &NativeFinalityProofIntervalLimits,
        cancelled: &AtomicBool,
        waiter: &mut StateViewWaiter,
    ) -> Result<NativeFinalityProofInterval, FinalityProofIntervalReadError> {
        self.read_finality_proof_interval_climbing(owner, limits, cancelled, Some(waiter))
    }

    fn read_finality_proof_interval_climbing(
        &self,
        owner: &NativeFinalityProofBudget,
        limits: &NativeFinalityProofIntervalLimits,
        cancelled: &AtomicBool,
        mut waiter: Option<&mut StateViewWaiter>,
    ) -> Result<NativeFinalityProofInterval, FinalityProofIntervalReadError> {
        // Climb only toward committed history; a later height still refuses at once.
        if u64::try_from(self.committed_height()).is_ok_and(|tip| limits.final_height() <= tip) {
            self.climb_finality_ladder(
                owner,
                limits,
                cancelled,
                LADDER_STEP,
                waiter.as_deref_mut(),
                || {},
            )?;
        }
        self.read_finality_proof_interval_after_read(owner, limits, cancelled, waiter, || {})
    }

    // Retain verified prefixes `step` apart until one lies within `step` below the
    // first height. Each step's proof is discarded; its retained prefix is the progress.
    fn climb_finality_ladder(
        &self,
        owner: &NativeFinalityProofBudget,
        limits: &NativeFinalityProofIntervalLimits,
        cancelled: &AtomicBool,
        step: u64,
        mut waiter: Option<&mut StateViewWaiter>,
        mut after_step: impl FnMut(),
    ) -> Result<(), FinalityProofIntervalReadError> {
        let first = limits.first_height();
        let mut floor = self.finality_ladder_floor(first);
        while first - floor > step {
            let target = NonZeroU64::new(floor + step).expect("a step above its floor");
            drop(self.read_finality_proof_interval_after_read(
                owner,
                &limits.step_to(target),
                cancelled,
                waiter.as_deref_mut(),
                &mut after_step,
            )?);
            let next = self.finality_ladder_floor(first);
            if next < target.get() {
                // Optional retention kept nothing: one direct walk replaces further
                // steps that would each restart below this one.
                break;
            }
            floor = next;
        }
        Ok(())
    }

    fn read_finality_proof_interval_after_read(
        &self,
        owner: &NativeFinalityProofBudget,
        limits: &NativeFinalityProofIntervalLimits,
        cancelled: &AtomicBool,
        mut waiter: Option<&mut StateViewWaiter>,
        after_read: impl FnOnce(),
    ) -> Result<NativeFinalityProofInterval, FinalityProofIntervalReadError> {
        let mut after_read = Some(after_read);
        let (retained, index, expected, mut current) = loop {
            let walked = owner.frames().with_deferred_refund_notifications(
                |_| -> Result<_, FinalityProofIntervalReadError> {
                    limits.check(cancelled)?;
                    // Deliver this capture's reader notice before the walk, as the recheck
                    // does: a reader that met this acquisition retries now, not after it.
                    let (hashes, _) = {
                        let mut releases = self.block_hashes.reader_release_batch();
                        self.try_event_source(&mut releases, || {})?
                    };
                    let index = usize::try_from(limits.final_height())
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
                    if let Some(after_read) = after_read.take() {
                        after_read();
                    }
                    limits.check(cancelled)?;
                    let current = self.interval_tip_is_current(index, expected, limits);
                    Ok((retained, index, expected, current))
                },
            );
            match walked {
                Err(FinalityProofIntervalReadError::StateView(StateViewError::Busy(release))) => {
                    wait_for_release(waiter.as_deref_mut(), release, limits, cancelled)?;
                }
                walked => break walked?,
            }
        };
        // Only the recheck repeats; the verified interval is never walked again.
        loop {
            match current {
                Err(FinalityProofIntervalReadError::StateView(StateViewError::Busy(release))) => {
                    wait_for_release(waiter.as_deref_mut(), release, limits, cancelled)?;
                    current = owner.frames().with_deferred_refund_notifications(|_| {
                        limits.check(cancelled)?;
                        self.interval_tip_is_current(index, expected, limits)
                    });
                }
                current => {
                    current?;
                    return Ok(retained);
                }
            }
        }
    }

    // A stable appended tip is allowed only while the captured final hash is still
    // this State's hash at that height. Another frame there is a source refusal.
    fn interval_tip_is_current(
        &self,
        index: usize,
        expected: Option<HashOf<BlockHeader>>,
        limits: &NativeFinalityProofIntervalLimits,
    ) -> Result<(), FinalityProofIntervalReadError> {
        #[cfg(not(all(test, sumeragi_core_mutation = "HC214")))]
        {
            let mut current_releases = self.block_hashes.reader_release_batch();
            let (current_hashes, _) = self.try_event_source(&mut current_releases, || {})?;
            if expected.is_none() || current_hashes.get(index).copied() != expected {
                return Err(NativeFinalityProofIntervalError::Source {
                    height: limits.final_height(),
                }
                .into());
            }
        }
        #[cfg(all(test, sumeragi_core_mutation = "HC214"))]
        let _ = (index, expected, limits);
        Ok(())
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
        let final_limits = NativeFinalityProofIntervalLimits::new(
            height,
            height,
            NonZeroUsize::new(MAX_FINALITY_BLOCK_BYTES).expect("positive canonical block ceiling"),
            maximum_proof_bytes,
            deadline,
        )?;
        final_limits.check(cancelled)?;
        self.climb_finality_ladder(owner, &final_limits, cancelled, step, None, || {})?;
        let interval = self.read_finality_proof_interval_after_read(
            owner,
            &final_limits,
            cancelled,
            None,
            || {},
        )?;
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
    ///
    /// A read of `height` verifies every frame above this floor. Retained prefixes are
    /// process-local, so a restarted node's floor is signed genesis until reads or the
    /// startup ladder retain new ones. The floor grants no authority by itself.
    pub fn finality_ladder_floor(&self, height: u64) -> u64 {
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
