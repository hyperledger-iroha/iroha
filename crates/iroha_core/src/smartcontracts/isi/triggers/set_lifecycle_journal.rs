//! Transaction-local lifecycle capture at the authoritative trigger-store seams.
//!
//! The owner is embedded in `SetTransaction`, and drops with the enclosing
//! State transaction unless complete custody is transferred to an output row.
//! Frames and identifiers retain their original physical charges; effects are
//! never coalesced. State enables capture only for its committed runtime selection.

use iroha_allocation::{AllocationBudget, AllocationCharge, AllocationReservation, ChargedBuffer};
use iroha_data_model::block::execution_output::{
    RetainedActionFrameBytesV1, TriggerLifecycleActionCodecV1, TriggerLifecycleOriginV1 as Origin,
    TriggerLifecyclePolicyV1 as Policy, TriggerLifecycleRetryStateV1, TriggerLifecycleStateKindV1,
    lifecycle_hash_frame, lifecycle_unavailable_frame,
};

use super::*;

/// Explicit admission fixed by the selected runtime; never a node-local switch.
#[derive(Clone, Copy)]
pub(crate) struct LifecycleJournalLimits {
    pub(crate) effects: usize,
    pub(crate) total_frame_bytes: usize,
    pub(crate) scratch_bytes: usize,
    pub(crate) encode: norito::core::EncodeWorkLimits,
}

impl LifecycleJournalLimits {
    /// Immutable successor limits used by State, preflight and production-envelope tests.
    pub(crate) fn selected(maximum: u64) -> Self {
        Self {
            effects: 4096,
            total_frame_bytes: usize::try_from(maximum)
                .unwrap_or(usize::MAX)
                .min(16 * 1024 * 1024),
            scratch_bytes: 4 * 1024 * 1024,
            encode: norito::core::EncodeWorkLimits {
                max_frame_bytes: 16 * 1024 * 1024,
                // These are cumulative root limits, not a per-effect multiplier.
                max_byte_visits: 1024 * 1024 * 1024,
                max_operations: 16 * 1024 * 1024,
                max_depth: 128,
            },
        }
    }
}

/// Deterministic root exhaustion is distinct from node-local admission failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LifecycleJournalFailure {
    /// The root exceeded a committed trace/work bound and must be rejected.
    OutputLimit(&'static str),
    /// The original physical pool or allocator refused admitted storage.
    Allocation(&'static str),
    /// An invariant prevented authoritative capture.
    Refused(&'static str),
}

impl LifecycleJournalFailure {
    pub(crate) fn reason(self) -> &'static str {
        match self {
            Self::OutputLimit(reason) | Self::Allocation(reason) | Self::Refused(reason) => reason,
        }
    }

    fn codec(error: norito::core::BoundedEncodeWorkError) -> Self {
        use norito::core::{BoundedEncodeWorkError, EncodeWorkRefusal};
        match error {
            BoundedEncodeWorkError::Refused(
                EncodeWorkRefusal::ByteVisits
                | EncodeWorkRefusal::Operations
                | EncodeWorkRefusal::Depth
                | EncodeWorkRefusal::FrameBytes,
            ) => Self::OutputLimit("lifecycle codec work limit exceeded"),
            BoundedEncodeWorkError::Codec(error) => Self::decode(error),
            BoundedEncodeWorkError::Refused(EncodeWorkRefusal::Source) => {
                Self::OutputLimit("lifecycle codec source refused")
            }
            BoundedEncodeWorkError::Refused(EncodeWorkRefusal::NestedScope) => {
                Self::Refused("lifecycle codec scope already borrowed")
            }
        }
    }

    fn decode(error: norito::Error) -> Self {
        match error.decode_resource_error() {
            Some(norito::core::DecodeResourceError::AllocationFailed { .. }) => {
                Self::Allocation("lifecycle codec allocation refused")
            }
            Some(_) => Self::OutputLimit("lifecycle codec scratch limit exceeded"),
            None => Self::OutputLimit("lifecycle codec source refused"),
        }
    }
}

/// A name copied only after its exact optional backing layout was reserved.
pub(crate) struct RetainedTriggerId {
    pub(crate) id: TriggerId,
    // Field order releases the name allocation before returning its credits.
    pub(crate) _charge: Option<AllocationCharge>,
}

impl RetainedTriggerId {
    fn new(id: &TriggerId, pool: &AllocationBudget) -> Result<Self, LifecycleJournalFailure> {
        let charge = id
            .name()
            .admission_clone_layout()
            .map(|layout| {
                let mut reservation = pool.try_reserve(layout).map_err(|_| {
                    LifecycleJournalFailure::Allocation("lifecycle identifier admission refused")
                })?;
                reservation.try_split(layout).map_err(|_| {
                    LifecycleJournalFailure::Refused("lifecycle identifier reservation lost")
                })
            })
            .transpose()?;
        let name = id.name().try_clone_for_admission().map_err(|_| {
            LifecycleJournalFailure::Allocation("lifecycle identifier allocation refused")
        })?;
        Ok(Self {
            id: TriggerId::new(name),
            _charge: charge,
        })
    }
}

/// A complete store snapshot without a cloned Action/loaded executable graph.
pub(crate) struct LifecycleSnapshot {
    pub(crate) kind: TriggerLifecycleStateKindV1,
    pub(crate) registered_at_height: Option<u64>,
    pub(crate) action_hash: Option<Hash>,
    pub(crate) action: RetainedActionFrameBytesV1,
    pub(crate) codec: TriggerLifecycleActionCodecV1,
    pub(crate) retry_state: Option<TriggerLifecycleRetryStateV1>,
}

pub(crate) enum LifecycleChange {
    Insert(LifecycleSnapshot),
    Update(LifecycleSnapshot, LifecycleSnapshot),
    Remove(LifecycleSnapshot),
    Invoke(LifecycleSnapshot, u32),
}

pub(crate) struct LifecycleEffect {
    pub(crate) ordinal: u32,
    pub(crate) trigger_id: RetainedTriggerId,
    pub(crate) origin: Origin,
    pub(crate) change: LifecycleChange,
}

/// Fixed, prepaid control storage with separately owned frame and name backing.
#[derive(Clone, Copy)]
pub(super) struct LifecycleWorkBudget {
    byte_visits: u64,
    operations: u64,
}
impl LifecycleWorkBudget {
    pub(super) fn block() -> Self {
        Self::new(1024 * 1024 * 1024, 16 * 1024 * 1024)
    }

    fn new(byte_visits: u64, operations: u64) -> Self {
        Self {
            byte_visits,
            operations,
        }
    }
    #[cfg(test)]
    fn remaining_byte_visits(&self) -> u64 {
        self.byte_visits
    }
    fn with_snapshot<R>(
        &mut self,
        limits: norito::core::EncodeWorkLimits,
        operation: impl FnOnce() -> R,
    ) -> Result<R, norito::core::BoundedEncodeWorkError> {
        // Each scope fits usize on every supported target; the cumulative
        // envelope is explicitly u64, so pointer width cannot alter consensus.
        let bytes = self.byte_visits.min(limits.max_byte_visits as u64) as usize;
        let operations = self.operations.min(limits.max_operations as u64) as usize;
        let mut scope = norito::core::EncodeWorkBudget::new(bytes, operations);
        let result = norito::core::with_cumulative_encode_work(&mut scope, operation);
        self.byte_visits -= (bytes - scope.remaining_byte_visits()) as u64;
        self.operations -= (operations - scope.remaining_operations()) as u64;
        result
    }
}

/// Exact journal counters for disposable read-only diagnostics. These counters
/// measure capture, not transaction gas, final row encoding or allocation work.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LifecycleCaptureMetrics {
    pub(crate) byte_visits: u64,
    pub(crate) operations: u64,
    pub(crate) effects: usize,
    pub(crate) frame_bytes: usize,
    pub(crate) unavailable_states: usize,
}

/// Published fresh-block capture limits, shared with the diagnostic report.
pub(crate) fn lifecycle_block_work_limits() -> (u64, u64) {
    let budget = LifecycleWorkBudget::block();
    (budget.byte_visits, budget.operations)
}

pub(crate) struct LifecycleJournal {
    pub(crate) effects: ChargedBuffer<LifecycleEffect>,
    pub(crate) pool: AllocationBudget,
    limits: LifecycleJournalLimits,
    applying_height: u64,
    frame_bytes: usize,
    scratch: AllocationReservation,
    codec: norito::core::DecodeBudgetContext,
    remaining_scan_steps: usize,
    encode_work: LifecycleWorkBudget,
    verified_ivm: ChargedBuffer<invocation_identity::VerifiedIvm>,
    origin: Origin,
    refusal: Option<LifecycleJournalFailure>,
    failed_callback: bool,
    last_callback: Option<u32>,
}

impl LifecycleJournal {
    fn new(
        pool: AllocationBudget,
        limits: LifecycleJournalLimits,
        applying_height: u64,
    ) -> Result<Self, &'static str> {
        if u32::try_from(limits.effects).is_err() {
            return Err("lifecycle effect capacity exceeds u32");
        }
        // Mutable Action fields are freshly hashed under one finite root
        // allowance. Only fully verified immutable IVM rows are reusable.
        let verification_capacity = limits
            .effects
            .checked_mul(2)
            .and_then(|n| n.checked_add(2))
            .ok_or("lifecycle verification capacity overflow")?;
        let verified_ivm = ChargedBuffer::new(verification_capacity, &pool)
            .map_err(|_| "lifecycle IVM verification admission refused")?;
        let effects = ChargedBuffer::new(limits.effects, &pool)
            .map_err(|_| "lifecycle journal admission refused")?;
        let scratch = pool
            .try_reserve_bytes(limits.scratch_bytes)
            .map_err(|_| "lifecycle scratch admission refused")?;
        let codec = norito::core::DecodeBudgetContext::try_new_owned(
            norito::core::DecodeLimits::new(
                limits.encode.max_operations,
                limits.encode.max_frame_bytes,
                limits.encode.max_operations,
                limits.scratch_bytes,
                limits.encode.max_depth,
            ),
            &pool,
        )
        .map_err(|_| "lifecycle codec control admission refused")?;
        Ok(Self {
            effects,
            pool,
            limits,
            applying_height,
            frame_bytes: 0,
            scratch,
            codec,
            remaining_scan_steps: limits.encode.max_operations,
            encode_work: LifecycleWorkBudget::new(
                limits.encode.max_byte_visits as u64,
                limits.encode.max_operations as u64,
            ),
            verified_ivm,
            origin: Origin::Derived {
                callback_index: None,
            },
            refusal: None,
            failed_callback: false,
            last_callback: None,
        })
    }

    fn charge_cache_work(&mut self, bytes: usize) -> Result<(), LifecycleJournalFailure> {
        let refusal =
            LifecycleJournalFailure::OutputLimit("lifecycle verification index work exhausted");
        self.encode_work.byte_visits = self
            .encode_work
            .byte_visits
            .checked_sub(bytes as u64)
            .ok_or(refusal)?;
        self.encode_work.operations = self.encode_work.operations.checked_sub(1).ok_or(refusal)?;
        Ok(())
    }

    fn remember_ivm(
        &mut self,
        verified: invocation_identity::VerifiedIvm,
    ) -> Result<(), LifecycleJournalFailure> {
        let comparisons = self
            .verified_ivm
            .as_slice()
            .len()
            .checked_ilog2()
            .unwrap_or(0) as usize
            + 1;
        self.charge_cache_work(comparisons * Hash::LENGTH)?;
        let index = match self
            .verified_ivm
            .as_slice()
            .binary_search_by_key(&verified.blob, |entry| entry.blob)
        {
            Ok(index) => {
                return if self.verified_ivm.as_slice()[index] == verified {
                    Ok(())
                } else {
                    Err(LifecycleJournalFailure::OutputLimit(
                        "lifecycle IVM verification changed without invalidation",
                    ))
                };
            }
            Err(index) => index,
        };
        if self.verified_ivm.as_slice().len() == self.verified_ivm.capacity() {
            return Err(LifecycleJournalFailure::OutputLimit(
                "lifecycle IVM verification capacity exhausted",
            ));
        }
        let bytes = (self.verified_ivm.as_slice().len() - index + 1)
            .checked_mul(72)
            .ok_or(LifecycleJournalFailure::OutputLimit(
                "lifecycle verification index length overflow",
            ))?;
        self.charge_cache_work(bytes)?;
        self.verified_ivm.push_reserved(verified);
        self.verified_ivm.as_mut_slice()[index..].rotate_right(1);
        Ok(())
    }

    /// Account cache invalidation against the same nonrefunded block allowance.
    pub(super) fn forget_ivm_in_block(
        &mut self,
        blob: &HashOf<IvmBytecode>,
        block: &std::cell::Cell<LifecycleWorkBudget>,
    ) {
        let block_before = block.get();
        let root_before = self.encode_work;
        self.encode_work = LifecycleWorkBudget::new(
            root_before.byte_visits.min(block_before.byte_visits),
            root_before.operations.min(block_before.operations),
        );
        let before = self.encode_work;
        self.forget_ivm(blob);
        let bytes = before.byte_visits - self.encode_work.byte_visits;
        let operations = before.operations - self.encode_work.operations;
        self.encode_work = LifecycleWorkBudget::new(
            root_before.byte_visits - bytes,
            root_before.operations - operations,
        );
        block.set(LifecycleWorkBudget::new(
            block_before.byte_visits - bytes,
            block_before.operations - operations,
        ));
    }

    /// Entry-lifetime invalidation at the private authoritative contract-store seams.
    /// Invalidation always happens, even after work exhaustion, and refunds no work.
    pub(super) fn forget_ivm(&mut self, blob: &HashOf<IvmBytecode>) {
        if self.verified_ivm.as_slice().is_empty() {
            return;
        }
        let comparisons = self
            .verified_ivm
            .as_slice()
            .len()
            .checked_ilog2()
            .unwrap_or(0) as usize
            + 1;
        if self.charge_cache_work(comparisons * Hash::LENGTH).is_err() {
            self.verified_ivm.truncate(0);
            return;
        }
        let Ok(index) = self
            .verified_ivm
            .as_slice()
            .binary_search_by_key(blob, |entry| entry.blob)
        else {
            return;
        };
        let length = self.verified_ivm.as_slice().len();
        let bytes = (length - index) * 72;
        if self.charge_cache_work(bytes).is_err() {
            // VerifiedIvm contains only Copy values: clearing this optimization
            // is constant-time and never rejects a mandatory failure policy.
            // Future capture must verify afresh under the remaining allowance.
            self.verified_ivm.truncate(0);
            return;
        }
        self.verified_ivm.as_mut_slice()[index..].rotate_left(1);
        self.verified_ivm.truncate(length - 1);
    }

    fn refuse(&mut self, reason: &'static str) {
        self.fail(LifecycleJournalFailure::Refused(reason));
    }

    fn fail(&mut self, failure: LifecycleJournalFailure) {
        // A real local refusal must never become a deterministic root verdict.
        if self.refusal.is_none()
            || matches!(
                (self.refusal, failure),
                (
                    Some(LifecycleJournalFailure::OutputLimit(_)),
                    LifecycleJournalFailure::Refused(_) | LifecycleJournalFailure::Allocation(_)
                )
            )
        {
            self.refusal = Some(failure);
        }
    }

    fn charge_frame(
        &mut self,
        snapshot: &LifecycleSnapshot,
    ) -> Result<(), LifecycleJournalFailure> {
        let total = self
            .frame_bytes
            .checked_add(snapshot.action.backing_bytes())
            .ok_or(LifecycleJournalFailure::OutputLimit(
                "lifecycle aggregate frame length overflow",
            ))?;
        if total > self.limits.total_frame_bytes {
            return Err(LifecycleJournalFailure::OutputLimit(
                "lifecycle aggregate frame limit exceeded",
            ));
        }
        self.frame_bytes = total;
        Ok(())
    }

    fn record(&mut self, id: &TriggerId, change: LifecycleChange) {
        if self.refusal.is_some() || self.failed_callback {
            return;
        }
        let result = (|| {
            if self.effects.as_slice().len() == self.effects.capacity() {
                return Err(LifecycleJournalFailure::OutputLimit(
                    "lifecycle effect limit exceeded",
                ));
            }
            // A repair removes only the stray typed row. It may coexist with
            // a witnessed live row of another kind, whose continuity survives.
            if let LifecycleChange::Remove(before) = &change
                && norito::canonical_frame_payload::<u8>(before.action.as_bytes(), 0)
                    .is_ok_and(|payload| payload == [4])
            {
                let mut ordinary_seen = false;
                for effect in self
                    .effects
                    .as_slice()
                    .iter()
                    .rev()
                    .filter(|effect| effect.trigger_id.id == *id)
                {
                    if let LifecycleChange::Remove(prior) = &effect.change {
                        if prior.kind == before.kind {
                            return Err(LifecycleJournalFailure::OutputLimit(
                                "lifecycle stray repair repeats a witnessed store removal",
                            ));
                        }
                        if norito::canonical_frame_payload::<u8>(prior.action.as_bytes(), 0)
                            .is_ok_and(|payload| payload == [4])
                        {
                            continue;
                        }
                    }
                    if !ordinary_seen {
                        let live = match &effect.change {
                            LifecycleChange::Insert(state)
                            | LifecycleChange::Invoke(state, _)
                            | LifecycleChange::Update(_, state) => Some(state),
                            LifecycleChange::Remove(_) => None,
                        };
                        if live.is_some_and(|state| state.kind == before.kind) {
                            return Err(LifecycleJournalFailure::OutputLimit(
                                "lifecycle stray repair contradicts a witnessed live store kind",
                            ));
                        }
                        ordinary_seen = true;
                    }
                    if matches!(effect.change, LifecycleChange::Insert(_)) {
                        break;
                    }
                }
            }
            // Count every physical snapshot, including an equal-state update.
            match &change {
                LifecycleChange::Insert(s)
                | LifecycleChange::Remove(s)
                | LifecycleChange::Invoke(s, _) => self.charge_frame(s)?,
                LifecycleChange::Update(before, after) => {
                    self.charge_frame(before)?;
                    self.charge_frame(after)?;
                }
            }
            let trigger_id = RetainedTriggerId::new(id, &self.pool)?;
            let ordinal = u32::try_from(self.effects.as_slice().len()).map_err(|_| {
                LifecycleJournalFailure::OutputLimit("lifecycle ordinal exceeds u32")
            })?;
            self.effects.push_reserved(LifecycleEffect {
                ordinal,
                trigger_id,
                origin: self.origin,
                change,
            });
            Ok(())
        })();
        if let Err(reason) = result {
            self.fail(reason);
        }
    }
}

impl SetTransaction<'_> {
    /// The output producer captures its source before opening the business overlay.
    pub(crate) fn lifecycle_source_snapshot(
        &mut self,
        id: &TriggerId,
    ) -> Option<LifecycleSnapshot> {
        self.lifecycle_snapshot(id)
    }

    /// Move an untouched source journal/cache into its immediately following root.
    pub(crate) fn adopt_lifecycle_source_journal(
        &mut self,
        journal: LifecycleJournal,
    ) -> Result<(), &'static str> {
        if self.lifecycle_journal.is_some()
            || self.lifecycle_sealed.is_some()
            || !journal.effects.as_slice().is_empty()
            || journal.refusal.is_some()
            || journal.failed_callback
        {
            return Err("lifecycle source custody is not pristine");
        }
        self.lifecycle_journal = Some(journal);
        self.lifecycle_requires_transfer = true;
        Ok(())
    }

    /// State arms capture only for an authenticated successor runtime selection.
    pub(crate) fn enable_selected_lifecycle_journal(
        &mut self,
        pool: AllocationBudget,
        limits: LifecycleJournalLimits,
        applying_height: u64,
    ) -> Result<(), &'static str> {
        if self.lifecycle_journal.is_some() || self.lifecycle_sealed.is_some() {
            return Err("lifecycle journal already enabled");
        }
        self.lifecycle_journal = Some(LifecycleJournal::new(pool, limits, applying_height)?);
        self.lifecycle_requires_transfer = true;
        Ok(())
    }

    /// Storage-seam tests may observe capture without constructing an output row.
    #[cfg(test)]
    pub(crate) fn enable_lifecycle_journal(
        &mut self,
        pool: AllocationBudget,
        limits: LifecycleJournalLimits,
    ) -> Result<(), &'static str> {
        self.enable_selected_lifecycle_journal(pool, limits, u64::MAX)?;
        self.lifecycle_requires_transfer = false;
        Ok(())
    }

    pub(crate) fn lifecycle_allows_apply(&self) -> bool {
        (!self.lifecycle_requires_transfer || self.lifecycle_sealed == Some(false))
            && self.lifecycle_sealed != Some(true)
            && self
                .lifecycle_journal
                .as_ref()
                .is_none_or(|j| j.refusal.is_none() && !j.failed_callback)
    }

    pub(crate) fn lifecycle_failure(&self) -> Option<LifecycleJournalFailure> {
        if self.lifecycle_sealed == Some(true) {
            return Some(LifecycleJournalFailure::Refused(
                "lifecycle mutation after output transfer",
            ));
        }
        self.lifecycle_journal.as_ref().and_then(|j| j.refusal)
    }

    /// Move the original charged owners exactly once, closing the mutation seam.
    pub(crate) fn take_lifecycle_journal(
        &mut self,
    ) -> Result<Option<LifecycleJournal>, LifecycleJournalFailure> {
        if let Some(failure) = self.lifecycle_failure() {
            return Err(failure);
        }
        if self
            .lifecycle_journal
            .as_ref()
            .is_some_and(|j| j.failed_callback)
        {
            return Err(LifecycleJournalFailure::Refused(
                "failed callback business journal cannot publish",
            ));
        }
        if self.lifecycle_sealed.is_some() {
            return Err(LifecycleJournalFailure::Refused(
                "lifecycle journal already transferred",
            ));
        }
        let journal = self.lifecycle_journal.take();
        if journal.is_some() {
            self.lifecycle_sealed = Some(false);
        }
        Ok(journal)
    }

    /// Reject this root for a deterministic trace bound without poisoning its block.
    pub(crate) fn limit_lifecycle(&mut self, reason: &'static str) {
        if let Some(sealed) = &mut self.lifecycle_sealed {
            // A caller touching consumed capture violated ownership regardless
            // of whether the operation also exhausted a deterministic limit.
            *sealed = true;
        }
        if let Some(journal) = &mut self.lifecycle_journal {
            journal.fail(LifecycleJournalFailure::OutputLimit(reason));
        }
    }

    pub(crate) fn refuse_lifecycle(&mut self, reason: &'static str) {
        if let Some(sealed) = &mut self.lifecycle_sealed {
            *sealed = true;
        }
        if let Some(journal) = &mut self.lifecycle_journal {
            journal.refuse(reason);
        }
    }

    pub(crate) fn lifecycle_enabled(&self) -> bool {
        self.lifecycle_journal.is_some() || self.lifecycle_sealed.is_some()
    }

    pub(crate) fn lifecycle_origin(&self) -> Option<Origin> {
        self.lifecycle_journal.as_ref().map(|j| j.origin)
    }

    pub(crate) fn set_lifecycle_origin(&mut self, origin: Origin) {
        if let Some(j) = &mut self.lifecycle_journal {
            j.origin = origin;
        }
    }

    pub(crate) fn lifecycle_last_callback(&self) -> Option<u32> {
        self.lifecycle_journal
            .as_ref()
            .and_then(|j| j.last_callback)
    }

    /// Read actual production counters without transferring custody or applying State.
    pub(crate) fn lifecycle_capture_metrics(&self) -> Option<LifecycleCaptureMetrics> {
        self.lifecycle_journal.as_ref().map(|journal| {
            let unavailable = |state: &LifecycleSnapshot| {
                usize::from(
                    norito::canonical_frame_payload::<u8>(state.action.as_bytes(), 0).is_ok(),
                )
            };
            let unavailable_states = journal
                .effects
                .as_slice()
                .iter()
                .map(|effect| match &effect.change {
                    LifecycleChange::Insert(state)
                    | LifecycleChange::Remove(state)
                    | LifecycleChange::Invoke(state, _) => unavailable(state),
                    LifecycleChange::Update(before, after) => {
                        unavailable(before) + unavailable(after)
                    }
                })
                .sum();
            LifecycleCaptureMetrics {
                byte_visits: journal.limits.encode.max_byte_visits as u64
                    - journal.encode_work.byte_visits,
                operations: journal.limits.encode.max_operations as u64
                    - journal.encode_work.operations,
                effects: journal.effects.as_slice().len(),
                frame_bytes: journal.frame_bytes,
                unavailable_states,
            }
        })
    }

    pub(crate) fn lifecycle_checkpoint(&self) -> Option<(usize, usize)> {
        self.lifecycle_journal
            .as_ref()
            .map(|j| (j.effects.as_slice().len(), j.frame_bytes))
    }

    pub(crate) fn lifecycle_finish_callback(
        &mut self,
        checkpoint: Option<(usize, usize)>,
        index: u32,
        success: bool,
    ) {
        if let Some(j) = &mut self.lifecycle_journal {
            if !success {
                j.failed_callback = true;
                if let Some((length, bytes)) = checkpoint {
                    j.effects.truncate(length);
                    j.frame_bytes = bytes;
                }
            }
            // A failure never clears a resource refusal, even after truncation.
            j.last_callback = success.then_some(index);
        }
    }

    pub(crate) fn lifecycle_invoke(&mut self, id: &TriggerId, index: u32) {
        if let Some(snapshot) = self.lifecycle_snapshot(id) {
            // Authentication historically accepts a wider source language than
            // bounded capture. Never publish a successful Invoke without its
            // exact use-time identity: reject this disposable root deterministically.
            if norito::canonical_frame_payload::<u8>(snapshot.action.as_bytes(), 0).is_ok() {
                if let Some(journal) = self.lifecycle_journal.as_mut() {
                    journal.fail(LifecycleJournalFailure::OutputLimit(
                        "invoked lifecycle source is unavailable",
                    ));
                }
                return;
            }
            self.lifecycle_record(id, LifecycleChange::Invoke(snapshot, index));
        } else if let Some(j) = &mut self.lifecycle_journal
            && j.refusal.is_none()
            && !j.failed_callback
        {
            j.fail(LifecycleJournalFailure::OutputLimit(
                "invoked lifecycle entry is absent",
            ));
        }
    }

    pub(super) fn lifecycle_record(&mut self, id: &TriggerId, change: LifecycleChange) {
        if let Some(j) = &mut self.lifecycle_journal {
            j.record(id, change);
        }
    }

    pub(super) fn lifecycle_inserted(&mut self, id: &TriggerId) {
        if let Some(after) = self.lifecycle_snapshot(id) {
            self.lifecycle_record(id, LifecycleChange::Insert(after));
        }
    }

    pub(super) fn lifecycle_updated(&mut self, id: &TriggerId, before: Option<LifecycleSnapshot>) {
        self.lifecycle_update_result(id, before, true);
    }

    pub(super) fn lifecycle_update_result(
        &mut self,
        id: &TriggerId,
        before: Option<LifecycleSnapshot>,
        success: bool,
    ) {
        if let Some(before) = before {
            if let Some(after) = self.lifecycle_snapshot(id) {
                // A failed mutator may have changed storage before returning its
                // error. Capture those actual effects if a caller retains them.
                // A failed operation with unchanged state is not a successful touch.
                if success
                    || before.action.as_bytes() != after.action.as_bytes()
                    || before.retry_state != after.retry_state
                    || before.action_hash != after.action_hash
                    || before.registered_at_height != after.registered_at_height
                    || before.kind != after.kind
                {
                    self.lifecycle_record(id, LifecycleChange::Update(before, after));
                }
            }
        }
    }

    pub(super) fn lifecycle_removed(&mut self, id: &TriggerId, before: Option<LifecycleSnapshot>) {
        if let Some(before) = before {
            self.lifecycle_record(id, LifecycleChange::Remove(before));
        }
    }

    pub(super) fn lifecycle_snapshot(&mut self, id: &TriggerId) -> Option<LifecycleSnapshot> {
        if let Some(sealed) = &mut self.lifecycle_sealed {
            *sealed = true;
            return None;
        }
        let j = self.lifecycle_journal.as_ref()?;
        if j.refusal.is_some() || j.failed_callback {
            return None;
        }
        // Absent removals/lookups have no effect and need no capture admission.
        self.ids.get(id)?;
        if j.effects.as_slice().len() >= j.limits.effects {
            self.lifecycle_journal
                .as_mut()
                .expect("enabled journal")
                .fail(LifecycleJournalFailure::OutputLimit(
                    "lifecycle effect limit exceeded",
                ));
            return None;
        }
        let pool = j.pool.clone();
        // Source hashing work is independent of fixed-size retained evidence.
        // record() charges every commitment; the pool owns its backing first.
        let limits = j.limits.encode;
        let applying_height = j.applying_height;
        let mut captured = None;
        let block_before = self.lifecycle_block_work.get();
        let root_before = j.encode_work;
        let mut encode_work = LifecycleWorkBudget::new(
            root_before.byte_visits.min(block_before.byte_visits),
            root_before.operations.min(block_before.operations),
        );
        let admitted_before = encode_work;
        let admitted = encode_work.with_snapshot(limits, || {
            norito::core::with_prepaid_validation(&j.scratch, &j.codec, || {
                captured = Some(capture_snapshot(
                    self,
                    id,
                    &pool,
                    limits,
                    applying_height,
                    j.verified_ivm.as_slice(),
                ));
                Ok(())
            })
        });
        self.lifecycle_journal
            .as_mut()
            .expect("enabled capture")
            .encode_work = encode_work;
        let result = match admitted {
            Ok(Ok(())) => captured.expect("admitted capture ran"),
            Ok(Err(error)) => Err(LifecycleJournalFailure::decode(error)),
            Err(error) => Err(LifecycleJournalFailure::codec(error)),
        };
        let result = result.and_then(|candidate| {
            candidate
                .map(|(snapshot, verified)| {
                    if let Some(verified) = verified {
                        self.lifecycle_journal
                            .as_mut()
                            .expect("enabled capture")
                            .remember_ivm(verified)?;
                    }
                    Ok(snapshot)
                })
                .transpose()
        });
        let remaining = self
            .lifecycle_journal
            .as_ref()
            .expect("enabled capture")
            .encode_work;
        let bytes = admitted_before
            .byte_visits
            .saturating_sub(remaining.byte_visits);
        let operations = admitted_before
            .operations
            .saturating_sub(remaining.operations);
        self.lifecycle_block_work.set(LifecycleWorkBudget::new(
            block_before.byte_visits.saturating_sub(bytes),
            block_before.operations.saturating_sub(operations),
        ));
        self.lifecycle_journal
            .as_mut()
            .expect("enabled capture")
            .encode_work = LifecycleWorkBudget::new(
            root_before.byte_visits.saturating_sub(bytes),
            root_before.operations.saturating_sub(operations),
        );
        self.finish_lifecycle_snapshot(id, result)
    }

    fn finish_lifecycle_snapshot(
        &mut self,
        id: &TriggerId,
        result: Result<Option<LifecycleSnapshot>, LifecycleJournalFailure>,
    ) -> Option<LifecycleSnapshot> {
        // Genuine failure policy has a fixed reserved effect envelope. If its
        // source exceeds deterministic hashing work, retain an explicit opaque
        // state marker rather than dropping the retry/quarantine transition.
        // This never authorizes a verifier claim about the affected trigger.
        if matches!(result, Err(LifecycleJournalFailure::OutputLimit(_)))
            && matches!(
                self.lifecycle_origin(),
                Some(Origin::InvocationPolicy {
                    policy: Policy::AdvanceTimeRetry
                        | Policy::ExhaustTimeRetry
                        | Policy::QuarantinePipeline,
                    ..
                })
            )
        {
            let kind = match self.ids.get(id)? {
                TriggeringEventType::Data => TriggerLifecycleStateKindV1::Data,
                TriggeringEventType::Pipeline => TriggerLifecycleStateKindV1::Pipeline,
                TriggeringEventType::Time => TriggerLifecycleStateKindV1::Time,
                TriggeringEventType::ExecuteTrigger => TriggerLifecycleStateKindV1::ByCall,
            };
            return self.lifecycle_unavailable_snapshot(kind, 5);
        }
        match result {
            Ok(snapshot) => snapshot,
            Err(reason) => {
                self.lifecycle_journal
                    .as_mut()
                    .expect("enabled journal")
                    .fail(reason);
                None
            }
        }
    }

    fn lifecycle_unavailable_snapshot(
        &mut self,
        kind: TriggerLifecycleStateKindV1,
        reason: u8,
    ) -> Option<LifecycleSnapshot> {
        let journal = self.lifecycle_journal.as_mut()?;
        if journal.refusal.is_some() || journal.failed_callback {
            return None;
        }
        match unavailable(kind, reason, &journal.pool) {
            Ok(snapshot) => Some(snapshot),
            Err(error) => {
                journal.fail(error);
                None
            }
        }
    }

    /// Retire depleted entries in the original Data/Pipeline/Time/ByCall order.
    /// The ordinary disabled path continues using its existing bulk sweep.
    pub(super) fn lifecycle_remove_depleted(&mut self) -> Vec<TriggerId> {
        // The original bulk sweep visits each entry once per call. Its work
        // allowance is per sweep, never accumulated over unrelated callbacks.
        if let Some(journal) = &mut self.lifecycle_journal {
            journal.remaining_scan_steps = journal.limits.encode.max_operations;
        }
        let original = self.lifecycle_origin();
        if let Some(Origin::InvocationPolicy { callback_index, .. }) = original {
            self.set_lifecycle_origin(Origin::InvocationPolicy {
                callback_index,
                policy: Policy::RemoveDepleted,
            });
        }
        let mut removed = Vec::new();
        // The return value is the existing storage API's cleanup list. Capture
        // retains its own separately charged identifiers, never these clones.
        macro_rules! sweep {
            ($store:ident, $active:ident, $kind:ident, $state_kind:ident) => {{
                // Match remove_zeros: visit each typed store once and retain its
                // complete cleanup list even after capture has become impossible.
                let to_remove: Vec<_> = self.$store.iter().filter_map(|(id, action)| {
                    if let Some(journal) = &mut self.lifecycle_journal {
                        if journal.remaining_scan_steps == 0 {
                            journal.fail(LifecycleJournalFailure::OutputLimit("lifecycle sweep work exhausted"));
                        } else {
                            journal.remaining_scan_steps -= 1;
                        }
                    }
                    action.repeats.is_depleted().then(|| id.clone())
                }).collect();
                for id in to_remove {
                    let registered = self.ids.get(&id).copied();
                    let exact = registered == Some(TriggeringEventType::$kind);
                    // Only an existing authoritative typed row survives a
                    // stray repair. A dangling registry entry is itself a
                    // removal, before the separate typed-store repair below.
                    let preserved_action = (!exact).then(|| {
                        self.inspect_by_id(&id, |action| match action.executable() {
                            ExecutableRef::Ivm(blob) => Some(*blob),
                            _ => None,
                        })
                    }).flatten();
                    let preserves_registration = preserved_action.is_some();
                    let removed_dangling_registration = registered.is_some()
                        && !exact && !preserves_registration && self.remove(&id);
                    let before = if !exact {
                        self.lifecycle_unavailable_snapshot(TriggerLifecycleStateKindV1::$state_kind, 4)
                    } else {
                        self.lifecycle_snapshot(&id)
                    };
                    // A stale row in this store must not delete the live id in
                    // another store, its generation, or its permissions.
                    let removed_id = removed_dangling_registration
                        || (!preserves_registration && self.ids.remove(id.clone()).is_some());
                    self.$active.remove(id.clone());
                    // A desynced duplicate may never have acquired a counted
                    // artifact reference. Preserve the same-ID authoritative
                    // row's last reference without scanning unrelated entries
                    // or pretending to reconstruct all corrupted refcounts.
                    let preserved_blob = preserved_action.flatten();
                    let protects_last_live_reference = self.$store.get(&id)
                        .and_then(LoadedAction::extract_blob_hash)
                        .is_some_and(|blob| Some(blob) == preserved_blob
                            && self.contracts.get(&blob).is_some_and(|entry| entry.count == NonZeroU64::MIN));
                    let removed_trigger = if protects_last_live_reference {
                        self.$store.remove(id.clone()).is_some()
                    } else {
                        Self::remove_from(&mut self.contracts, &mut self.$store, id.clone(), self.lifecycle_journal.as_mut().map(|j| (j, self.lifecycle_block_work)))
                    };
                    if !removed_id || !removed_trigger {
                        warn!(trigger_id = %id, removed_id, removed_trigger,
                            "`Set` trigger collections out of sync while removing depleted trigger");
                    }
                    if !preserves_registration {
                        self.registration_generations.remove(&id);
                        self.data_trigger_eligibility_generations.remove(&id);
                        self.data_trigger_index.remove(&id);
                    } else if TriggeringEventType::$kind == TriggeringEventType::Data {
                        self.data_trigger_eligibility_generations.remove(&id);
                        self.data_trigger_index.remove(&id);
                    }
                    self.lifecycle_removed(&id, before);
                    if !preserves_registration {
                        removed.push(id);
                    }
                }
            }};
        }
        sweep!(data_triggers, active_data_trigger_ids, Data, Data);
        sweep!(
            pipeline_triggers,
            active_pipeline_trigger_ids,
            Pipeline,
            Pipeline
        );
        sweep!(time_triggers, active_time_trigger_ids, Time, Time);
        sweep!(
            by_call_triggers,
            active_by_call_trigger_ids,
            ExecuteTrigger,
            ByCall
        );
        if let Some(origin) = original {
            self.set_lifecycle_origin(origin);
        }
        removed
    }

    #[cfg(test)]
    pub(crate) fn lifecycle_effects(&self) -> &[LifecycleEffect] {
        self.lifecycle_journal
            .as_ref()
            .map_or(&[], |j| j.effects.as_slice())
    }
}

fn compact_bytes(
    bytes: &[u8],
    pool: &AllocationBudget,
) -> Result<RetainedActionFrameBytesV1, LifecycleJournalFailure> {
    RetainedActionFrameBytesV1::try_copy_compact(bytes, pool)
        .map_err(|_| LifecycleJournalFailure::Allocation("lifecycle commitment allocation refused"))
}

fn unavailable(
    kind: TriggerLifecycleStateKindV1,
    reason: u8,
    pool: &AllocationBudget,
) -> Result<LifecycleSnapshot, LifecycleJournalFailure> {
    Ok(LifecycleSnapshot {
        kind,
        registered_at_height: None,
        action_hash: None,
        retry_state: None,
        codec: TriggerLifecycleActionCodecV1::NoritoCanonicalV1,
        action: compact_bytes(
            &lifecycle_unavailable_frame(reason).map_err(LifecycleJournalFailure::decode)?,
            pool,
        )?,
    })
}

fn capture_snapshot(
    store: &SetTransaction<'_>,
    id: &TriggerId,
    pool: &AllocationBudget,
    limits: norito::core::EncodeWorkLimits,
    applying_height: u64,
    verified: &[invocation_identity::VerifiedIvm],
) -> Result<
    Option<(LifecycleSnapshot, Option<invocation_identity::VerifiedIvm>)>,
    LifecycleJournalFailure,
> {
    let Some(kind) = store.ids.get(id).copied() else {
        return Ok(None);
    };
    macro_rules! capture {
        ($field:ident, $kind:ident, $hash_kind:expr, $internal:expr) => {{
            let kind = TriggerLifecycleStateKindV1::$kind;
            let Some(action) = store.$field.get(id) else {
                return unavailable(kind, 2, pool).map(|state| Some((state, None)));
            };
            let height = action
                .metadata
                .get(super::super::TRIGGER_REGISTERED_BLOCK_HEIGHT_METADATA_KEY)
                .map(|value| {
                    let text = value.get();
                    if text.is_empty()
                        || text.len() > 20
                        || !text.bytes().all(|byte| byte.is_ascii_digit())
                        || (text.len() > 1 && text.starts_with('0'))
                    {
                        return None;
                    }
                    text.parse::<u64>().ok()
                });
            if height == Some(None)
                || ($internal && height.is_none())
                || height
                    .flatten()
                    .is_some_and(|height| height > applying_height)
                || action
                    .retry_state
                    .is_some_and(|retry| retry.retries_used == 0)
                || (kind != TriggerLifecycleStateKindV1::Time && action.retry_state.is_some())
            {
                return unavailable(kind, 1, pool).map(|state| Some((state, None)));
            }
            if let ExecutableRef::Ivm(blob) = &action.executable
                && store.get_original_contract(blob).is_none()
            {
                return unavailable(kind, 3, pool).map(|state| Some((state, None)));
            }
            let hash = norito::core::with_bounded_encode_work(limits, || {
                invocation_identity::lifecycle_action_hash(
                    store, action, $hash_kind, verified, None,
                )
            });
            let (hash, fresh, _) = match hash {
                Ok(hash) => hash,
                Err(norito::core::BoundedEncodeWorkError::Refused(
                    norito::core::EncodeWorkRefusal::Source,
                )) => return unavailable(kind, 3, pool).map(|state| Some((state, None))),
                Err(error) => return Err(LifecycleJournalFailure::codec(error)),
            };
            Ok(Some((
                LifecycleSnapshot {
                    kind,
                    registered_at_height: height.flatten(),
                    action_hash: $internal.then_some(hash),
                    codec: TriggerLifecycleActionCodecV1::NoritoCanonicalV1,
                    action: compact_bytes(
                        &lifecycle_hash_frame(hash).map_err(LifecycleJournalFailure::decode)?,
                        pool,
                    )?,
                    retry_state: action
                        .retry_state
                        .map(|retry| TriggerLifecycleRetryStateV1 {
                            attempt: retry.retries_used,
                            next_time_ms: retry.next_retry_at_ms,
                        }),
                },
                fresh,
            )))
        }};
    }
    match kind {
        TriggeringEventType::Data => capture!(data_triggers, Data, 2, false),
        TriggeringEventType::Pipeline => capture!(pipeline_triggers, Pipeline, 0, true),
        TriggeringEventType::Time => capture!(time_triggers, Time, 1, true),
        TriggeringEventType::ExecuteTrigger => capture!(by_call_triggers, ByCall, 3, false),
    }
}

#[cfg(test)]
#[path = "set_lifecycle_journal_tests.rs"]
mod tests;
