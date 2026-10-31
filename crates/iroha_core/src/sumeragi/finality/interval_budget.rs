//! One request's prepaid, finite live native-verifier working set.

use iroha_allocation::{AllocationBudget, AllocationCharge};
use norito::{
    DecodeLimits,
    core::{DecodeBudgetContext, PreparedDecodeWorkspace},
};
use std::{alloc::Layout, cell::RefCell};

use crate::execution_attempt::ExecutionAttemptError;
use crate::sumeragi::certified_chain::ChainReadError;

/// Original physical corridors supplied by the integrating request admission.
///
/// These pools must be disjoint parts of the same retained request reservation.
/// The source/output pool remains charged by actual native destinations. The graph
/// pool prepays the complete verifier working set before any source is decoded.
#[derive(Clone)]
pub struct NativeFinalityProofBudget {
    frames: AllocationBudget,
    graphs: AllocationBudget,
    limits: DecodeLimits,
}

impl NativeFinalityProofBudget {
    /// Retain the caller's already admitted source and graph corridors.
    pub fn new(frames: AllocationBudget, graphs: AllocationBudget, limits: DecodeLimits) -> Self {
        Self {
            frames,
            graphs,
            limits,
        }
    }

    pub(crate) fn frames(&self) -> &AllocationBudget {
        &self.frames
    }

    pub(crate) fn workspace(
        &self,
    ) -> Result<FinalityDecodeWorkspace, ExecutionAttemptError<ChainReadError>> {
        FinalityDecodeWorkspace::new(&self.graphs, self.limits)
    }
}

// The bodyless query retains at most seven bounded metadata generations:
// signed-genesis epoch, held target ScheduleOutcome, current gap ScheduleOutcome,
// old/new three-slot prefix schedules and old/new query authority. The first five
// retain the intrinsic result decoder's conservative ceiling. The two authorities
// instead retain the closed maximum native committee/credential backing; their
// actual owned material extent is checked after all original native validation.
// Inline validation snapshots/cursor fields belong to the separate exact frame pool.
const RESULT_METADATA_GENERATIONS: usize = 5;
const AUTHORITY_GENERATIONS: usize = 2;

pub(crate) fn metadata_generation_bytes() -> usize {
    norito::canonical_decode_limits(iroha_data_model::sumeragi_finality::MAX_RESULT_BODY_BYTES)
        .max_total_allocated_bytes()
}

fn metadata_bytes() -> Option<usize> {
    metadata_generation_bytes()
        .checked_mul(RESULT_METADATA_GENERATIONS)?
        .checked_add(
            crate::sumeragi::certified_chain::native_finality_authority_bytes()?
                .checked_mul(AUTHORITY_GENERATIONS)?,
        )
}

// Norito's JSON unknown-field error owns one key outside decoder accounting.
// Signed genesis metadata is capped by MAX_JSON_BYTES; that error is projected
// directly without formatting. Other native diagnostics can format a bounded
// peer/public key (at most 8,258 payload bytes), with overlapping text and growth
// covered by 128 KiB. These diagnostics are dropped before this workspace; no
// dynamic reason escapes the interval's heapless native-refusal projection.
const DIAGNOSTIC_BYTES: usize = iroha_primitives::json::MAX_JSON_BYTES + 128 * 1024;

/// Smallest graph corridor which can admit this closed native query owner.
/// Source/output frames have a separate finite corridor; the remaining byte is
/// only a nonzero transient allowance, not a promise to accept any block size.
pub fn native_finality_graph_floor_bytes() -> Option<usize> {
    PreparedDecodeWorkspace::allocation_layouts()
        .iter()
        .try_fold(0usize, |sum, layout| sum.checked_add(layout.size()))?
        .checked_add(DecodeBudgetContext::allocation_layout().size())?
        .checked_add(DIAGNOSTIC_BYTES)?
        .checked_add(metadata_bytes()?)?
        .checked_add(1)
}

fn capacity() -> ExecutionAttemptError<ChainReadError> {
    ExecutionAttemptError::Deferred(ivm::error::ExecutionDeferral::ActiveMemoryCapacity.into())
}

pub struct FinalityDecodeWorkspace {
    counters: RefCell<PreparedDecodeWorkspace>,
    budget: AllocationBudget,
    limits: DecodeLimits,
    // All caller-owned cursor/receipt graphs must die before this original charge.
    _graphs: AllocationCharge,
}

impl FinalityDecodeWorkspace {
    fn new(
        budget: &AllocationBudget,
        original: DecodeLimits,
    ) -> Result<Self, ExecutionAttemptError<ChainReadError>> {
        let controls = PreparedDecodeWorkspace::allocation_layouts()
            .iter()
            .map(Layout::size)
            .sum::<usize>()
            .checked_add(DecodeBudgetContext::allocation_layout().size())
            .ok_or_else(capacity)?;
        let stage = budget
            .limit_bytes()
            .checked_sub(controls)
            .and_then(|bytes| bytes.checked_sub(DIAGNOSTIC_BYTES))
            .and_then(|bytes| bytes.checked_sub(metadata_bytes()?))
            .filter(|bytes| *bytes > 0)
            .ok_or_else(capacity)?;
        let graph_bytes = stage
            .checked_add(metadata_bytes().ok_or_else(capacity)?)
            .and_then(|bytes| bytes.checked_add(DIAGNOSTIC_BYTES))
            .ok_or_else(capacity)?;
        let layout = Layout::array::<u8>(
            graph_bytes
                .checked_add(controls)
                .and_then(|bytes| {
                    bytes.checked_sub(DecodeBudgetContext::allocation_layout().size())
                })
                .ok_or_else(capacity)?,
        )
        .map_err(|_| capacity())?;
        let mut reservation = budget.try_reserve(layout).map_err(|_| capacity())?;
        let counters = PreparedDecodeWorkspace::from_reservation(budget, &mut reservation)
            .map_err(|_| capacity())?;
        let charge = reservation
            .try_split(Layout::array::<u8>(graph_bytes).map_err(|_| capacity())?)
            .map_err(|_| capacity())?;
        Ok(Self {
            counters: RefCell::new(counters),
            budget: budget.clone(),
            limits: DecodeLimits::new(
                original.max_sequence_elements(),
                original.max_field_bytes(),
                stage.min(original.max_total_elements()),
                stage.min(original.max_total_allocated_bytes()),
                original.max_nesting_depth(),
            ),
            _graphs: charge,
        })
    }

    pub(crate) fn target_context(
        &self,
    ) -> Result<DecodeBudgetContext, ExecutionAttemptError<ChainReadError>> {
        DecodeBudgetContext::try_new_owned(self.limits, &self.budget).map_err(|_| capacity())
    }

    #[cfg(test)]
    pub(crate) fn note_target_cost(&self, context: &DecodeBudgetContext) {
        cost_observation::record("target", context.consumed_allocated_bytes());
    }

    /// Fresh genesis checks precede target C1-C5. While that first decoded target
    /// body is still alive, preserve its entire consumed original corridor as a
    /// conservative retained upper bound; never reset its original counter.
    pub(crate) fn bootstrap<R, E>(
        &self,
        target: &DecodeBudgetContext,
        work: impl FnOnce() -> Result<R, E>,
    ) -> Result<R, E>
    where
        E: From<ExecutionAttemptError<ChainReadError>>,
    {
        let retained =
            usize::try_from(target.consumed_allocated_bytes()).map_err(|_| E::from(capacity()))?;
        let remaining = self
            .limits
            .max_total_allocated_bytes()
            .checked_sub(retained)
            .ok_or_else(|| E::from(capacity()))?;
        let limits = DecodeLimits::new(
            self.limits.max_sequence_elements(),
            self.limits.max_field_bytes(),
            self.limits.max_total_elements(),
            remaining,
            self.limits.max_nesting_depth(),
        );
        self.counters
            .borrow_mut()
            .with_limits(limits, limits, work)
            .map_err(|_| E::from(capacity()))?
    }

    /// Reuse only this workspace's prepaid stage controls. Any truly inherited
    /// caller limit still composes normally; no original outer counter is reset.
    pub(crate) fn stage<R, E>(&self, work: impl FnOnce() -> Result<R, E>) -> Result<R, E>
    where
        E: From<ExecutionAttemptError<ChainReadError>>,
    {
        #[cfg(test)]
        let work = || cost_observation::stage(work);
        self.counters
            .borrow_mut()
            .with_limits(self.limits, self.limits, work)
            .map_err(|_| E::from(capacity()))?
    }
}

// Test instrumentation observes the original decoder's actual admissions. It
// grants no production allocation or certificate authority and is inactive for
// every test except the explicit native maximum-frame cost fixture.
#[cfg(test)]
pub(super) mod cost_observation {
    use super::*;

    thread_local! {
        static RECORDS: RefCell<Option<Vec<(&'static str, u64)>>> = const { RefCell::new(None) };
    }

    pub(crate) fn record(kind: &'static str, bytes: u64) {
        RECORDS.with_borrow_mut(|records| {
            if let Some(records) = records {
                records.push((kind, bytes));
            }
        });
    }

    pub(super) fn stage<R>(work: impl FnOnce() -> R) -> R {
        if !RECORDS.with_borrow(Option::is_some) {
            return work();
        }
        let counter = DecodeBudgetContext::new(DecodeLimits::new(
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
        ));
        let result = counter.with(work);
        record("stage", counter.consumed_allocated_bytes());
        result
    }

    pub(crate) fn measure<R>(work: impl FnOnce() -> R) -> (R, Vec<(&'static str, u64)>) {
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                RECORDS.with_borrow_mut(|records| *records = None);
            }
        }
        RECORDS.with_borrow_mut(|records| {
            assert!(records.is_none(), "native cost observations cannot nest");
            // Reserve instrumentation before an inner live-allocation census;
            // the maximum-frame fixture records fewer than 64 observations.
            *records = Some(Vec::with_capacity(64));
        });
        let _reset = Reset;
        let result = work();
        let records = RECORDS.with_borrow_mut(|records| records.take().unwrap());
        (result, records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner() -> (AllocationBudget, FinalityDecodeWorkspace) {
        let pool = AllocationBudget::new(DIAGNOSTIC_BYTES + metadata_bytes().unwrap() + 16_384);
        let workspace = FinalityDecodeWorkspace::new(
            &pool,
            DecodeLimits::new(16_384, 16_384, 16_384, 16_384, 32),
        )
        .unwrap();
        (pool, workspace)
    }

    #[test]
    fn finality_workspace_retains_funding_across_bounded_repeated_phases() {
        let (pool, workspace) = owner();
        let held = pool.reserved_bytes();
        assert!(held > pool.limit_bytes() / 2);
        for _ in 0..100 {
            workspace
                .stage(|| {
                    norito::core::reserve_decode_allocation(
                        workspace.limits.max_total_allocated_bytes(),
                    )
                    .map_err(|error| {
                        let _ = error;
                        capacity()
                    })
                })
                .unwrap();
            assert_eq!(pool.reserved_bytes(), held);
        }
        assert!(
            workspace
                .stage(|| norito::core::reserve_decode_allocation(
                    workspace.limits.max_total_allocated_bytes() + 1
                )
                .map_err(|_| capacity()))
                .is_err()
        );
        assert_eq!(pool.reserved_bytes(), held);
        drop(workspace);
        assert_eq!(pool.reserved_bytes(), 0);
    }

    #[test]
    fn finality_workspace_bootstrap_keeps_original_target_overlap_and_counter() {
        let (_pool, workspace) = owner();
        let target = workspace.target_context().unwrap();
        let maximum = workspace.limits.max_total_allocated_bytes();
        let retained = maximum / 2;
        target
            .with(|| norito::core::reserve_decode_allocation(retained))
            .unwrap();
        workspace
            .bootstrap(&target, || {
                norito::core::reserve_decode_allocation(maximum - retained).map_err(|_| capacity())
            })
            .unwrap();
        assert!(
            workspace
                .bootstrap(&target, || norito::core::reserve_decode_allocation(
                    maximum - retained + 1
                )
                .map_err(|_| capacity()))
                .is_err()
        );
        assert_eq!(target.consumed_allocated_bytes(), retained as u64);
        target
            .with(|| norito::core::reserve_decode_allocation(maximum - retained))
            .unwrap();
        assert!(
            target
                .with(|| norito::core::reserve_decode_allocation(1))
                .is_err()
        );
    }

    #[test]
    fn finality_workspace_never_resets_target_or_inherited_counter() {
        let (pool, workspace) = owner();
        let target = workspace.target_context().unwrap();
        let maximum = workspace.limits.max_total_allocated_bytes();
        target
            .with(|| norito::core::reserve_decode_allocation(maximum))
            .unwrap();
        workspace
            .stage(|| Ok::<_, ExecutionAttemptError<ChainReadError>>(()))
            .unwrap();
        assert!(
            target
                .with(|| norito::core::reserve_decode_allocation(1))
                .is_err()
        );
        let inherited = DecodeBudgetContext::new(DecodeLimits::new(16, 16, 16, 16, 16));
        inherited
            .with(|| {
                workspace
                    .stage(|| norito::core::reserve_decode_allocation(16).map_err(|_| capacity()))
            })
            .unwrap();
        assert!(
            inherited
                .with(|| workspace
                    .stage(|| norito::core::reserve_decode_allocation(1).map_err(|_| capacity())))
                .is_err()
        );
        assert_eq!(inherited.consumed_allocated_bytes(), 16);
        drop(target);
        drop(workspace);
        assert_eq!(pool.reserved_bytes(), 0);
    }
}
