//! The once-per-block dispatch of the governance block-start pass (`specs/sccp.md` §4.7.2;
//! `specs/parliament_private_ballot_design.md` §12.3 slice 3).
//!
//! The pass runs on the global chain's ordinary carrier only, after S0 and the pause-panel
//! draw. Parliament's items run exactly on a root whose selection installs the anonymous
//! family; on every other root the pass holds SCCP's items alone and no governance-start
//! partition exists. The pass's outputs and `governance_due_applied` stay with the block:
//! the output seal commits the rows ahead of every execution row, and dropping the block
//! discards them with its State.
//!
//! A refusal the node cannot decide defers the block and is never a verdict on it.
//!
//! Only a block whose pass ran Parliament's items executes signerless ballot inputs
//! ([`StateBlock::parliament_pass_ran`]): every other block has no board to decide them.
//!
//! TODO(design §12.3, slices 3 and 4): no real-peer test drives this dispatch. The pass and
//! the ballots it opens windows for are exercised on seeded World rows
//! (`governance::parliament_anonymous::world_pass` and `ballot_input` tests) and never
//! through `integration_tests/tests/`. The four-validator corridor (seal, opening,
//! successor-block clock, ballots and closure with identical committed rows on every peer
//! and across a restart) is still open.

use iroha_data_model::{
    block::consensus::SumeragiRootScope,
    governance::types::{BeaconPulseId, BeaconSessionId},
};
use mv::storage::StorageReadOnly as _;

use super::{StateBlock, StateBlockStartError, StateReadOnly as _, WorldReadOnly as _};
use crate::{
    governance::parliament_anonymous::{
        AnonymousSortitionPulseV1,
        pass::GovernancePassErrorV1,
        world_pass::{
            AnonymousPassWorldErrorV1, GovernanceOwnerErrorV1, ParliamentPassInputsV1,
            run_governance_pass,
        },
    },
    smartcontracts::isi::sccp::governance_clock::SccpPassOwnerV1,
};

/// Run the governance block-start pass of `state` once, with `before_enactments` between
/// G2c and G3. A block that is not one of the global chain runs `before_enactments` alone.
///
/// # Errors
/// The refusal of the pass or of `before_enactments`. A local refusal is a deferral
/// ([`StateBlockStartError::Storage`] or [`StateBlockStartError::ExecutionDeferred`]).
pub(super) fn run<E: std::fmt::Debug>(
    state: &mut StateBlock<'_>,
    before_enactments: impl FnOnce(&mut StateBlock<'_>) -> Result<(), StateBlockStartError<E>>,
) -> Result<(), StateBlockStartError<E>> {
    if state.sumeragi_schedule.original_root_scope() != Some(SumeragiRootScope::Global) {
        return before_enactments(state);
    }
    run_on_global_chain(state, before_enactments)
}

/// [`run`] for a block of the global chain: the root's selection decides whether
/// Parliament's items exist.
///
/// # Errors
/// As [`run`].
pub(crate) fn run_on_global_chain<E: std::fmt::Debug>(
    state: &mut StateBlock<'_>,
    before_enactments: impl FnOnce(&mut StateBlock<'_>) -> Result<(), StateBlockStartError<E>>,
) -> Result<(), StateBlockStartError<E>> {
    // SPEC: §13.4 HC803 — Parliament's items run only where the selection installs them.
    let installed = crate::runtime::parliament_anonymous_installed(state.world.runtime_selection())
        .map_err(StateBlockStartError::Policy)?
        || cfg!(all(test, sumeragi_core_mutation = "HC803"));
    let parliament = installed.then(|| {
        let pulses = committed_pulses(state);
        ParliamentPassInputsV1 {
            pulse_at: move |slot: u64| pulses.iter().find(|pulse| pulse.height == slot).copied(),
            parent_time_ms: state
                .native_execution_tip()
                .map(|tip| tip.creation_time_ms()),
        }
    });
    let mut sccp = SccpPassOwnerV1::<E>::default();
    let report = run_governance_pass(state, parliament, &mut sccp, before_enactments)
        .map_err(start_error)?;
    state.governance_due_applied = report.governance_due_applied();
    if installed {
        state.governance_start_outputs = Some(report.rows);
    }
    Ok(())
}

/// The authenticated native pulse of every slot an active attempt awaits at or below this
/// block's height: the pulse this block carries at its own height, which entered World in
/// the pristine stage, or one an earlier block committed. Each is verified again against
/// the key session retained for its own height; a slot whose pulse is absent or does not
/// verify has none, and its generation keeps waiting.
fn committed_pulses(state: &StateBlock<'_>) -> Vec<AnonymousSortitionPulseV1> {
    let height = state._curr_block.height().get();
    let network_id = *state.network_id();
    let session = BeaconSessionId::for_network_v1(&network_id);
    let mut slots: Vec<u64> = state
        .world
        .parliament_anonymous_due
        .iter()
        .filter_map(|(id, _)| state.world.parliament_anonymous_attempts.get(id))
        .flat_map(|attempt| attempt.awaiting_slots())
        .filter(|slot| *slot <= height)
        .collect();
    slots.sort_unstable();
    slots.dedup();
    slots
        .into_iter()
        .filter_map(|slot| {
            let pulse_id = state
                .world
                .global_beacon_pulse_slots
                .get(&(session, slot))?;
            let pulse = state.world.global_beacon_pulses.get(pulse_id)?;
            if pulse.pulse_id != *pulse_id || pulse.network_id != network_id || pulse.height != slot
            {
                return None;
            }
            let output =
                crate::beacon::verified_persisted_global_threshold_beacon_governance_seed_v1(
                    &state.world,
                    &network_id,
                    pulse,
                    slot,
                )
                .ok()?;
            Some(AnonymousSortitionPulseV1 {
                beacon_session_id: session,
                height: slot,
                pulse_id: BeaconPulseId::new(*pulse_id),
                output,
            })
        })
        .collect()
}

/// The block-start refusal of a pass that did not complete.
fn start_error<E: std::fmt::Debug>(
    error: GovernancePassErrorV1<GovernanceOwnerErrorV1<StateBlockStartError<E>>>,
) -> StateBlockStartError<E> {
    match error {
        GovernancePassErrorV1::Owner(GovernanceOwnerErrorV1::Foreign(error)) => error,
        GovernancePassErrorV1::Owner(GovernanceOwnerErrorV1::Parliament(error)) => {
            parliament_error(error)
        }
        GovernancePassErrorV1::Policy(reason) => StateBlockStartError::Policy(reason.into()),
        cursor => StateBlockStartError::Policy(format!("governance block-start pass: {cursor:?}")),
    }
}

/// The block-start refusal of a Parliament item. A refusal the node cannot decide (World
/// storage admission, its proof profile, a pinned frame, the verifier backend or an
/// allocation) defers the block; any other refusal of a listed item abandons it.
pub(crate) fn parliament_error<E: std::fmt::Debug>(
    error: AnonymousPassWorldErrorV1,
) -> StateBlockStartError<E> {
    use ivm::error::ExecutionDeferral;

    // SPEC: §13.4 HC805 — a local refusal defers the block and is never a verdict.
    let local = error.is_local() && !cfg!(all(test, sumeragi_core_mutation = "HC805"));
    match error {
        AnonymousPassWorldErrorV1::Storage(error) if local => StateBlockStartError::Storage(error),
        AnonymousPassWorldErrorV1::Output(_) if local => {
            StateBlockStartError::ExecutionDeferred(ExecutionDeferral::AllocationUnavailable.into())
        }
        _ if local => StateBlockStartError::ExecutionDeferred(
            ExecutionDeferral::VerifierArtifactsUnavailable.into(),
        ),
        error => StateBlockStartError::Policy(format!("governance block-start pass: {error}")),
    }
}

impl StateBlock<'_> {
    /// `governance_due_applied` of this block's pass (`specs/sccp.md` §4.7.2, K2): the
    /// prelude established a casting clock or at least one G1–G3 item was processed.
    pub(crate) fn governance_due_applied(&self) -> bool {
        self.governance_due_applied
    }

    /// Whether this block's pass ran Parliament's items: the block belongs to the global
    /// chain's ordinary carrier and its root's selection installs the anonymous family. Only
    /// such a block has ballot boards, so only it executes anonymous-ballot inputs
    /// (`specs/parliament_private_ballot_design.md` §3.6 and §12.3 slice 4).
    pub(crate) fn parliament_pass_ran(&self) -> bool {
        self.governance_start_outputs.is_some()
    }

    /// The governance block-start rows this block's pass emitted, in emission order; `None`
    /// when the block reserves no governance-start partition.
    #[cfg(test)]
    pub(crate) fn governance_start_rows(
        &self,
    ) -> Option<&[iroha_data_model::block::execution_output::ExecutionOutputV1]> {
        self.governance_start_outputs
            .as_ref()
            .map(iroha_data_model::block::output_budget::GovernanceStartOutputBudgetV1::rows)
    }
}

#[cfg(test)]
mod tests {
    use iroha_data_model::{
        block::output_budget::GovernanceStartOutputBudgetErrorV1,
        governance::anonymous_ballot::AnonymousBallotCodecErrorV1,
    };

    use super::*;
    use crate::smartcontracts::isi::sccp::test_support::{blank_state, header};

    #[test]
    fn a_local_refusal_defers_the_block_and_a_verdict_abandons_it() {
        for local in [
            AnonymousPassWorldErrorV1::Output(GovernanceStartOutputBudgetErrorV1::Allocation),
            AnonymousPassWorldErrorV1::Receipt(AnonymousBallotCodecErrorV1::FrameLayout(
                "a pinned frame the node cannot produce",
            )),
        ] {
            assert!(local.is_local());
            assert!(
                matches!(
                    parliament_error::<()>(local),
                    StateBlockStartError::ExecutionDeferred(_)
                ),
                "a local refusal of the governance pass defers the block and is never a verdict"
            );
        }
        for verdict in [
            AnonymousPassWorldErrorV1::World("tables that disagree"),
            AnonymousPassWorldErrorV1::Policy("a committed parameter that does not decode"),
            AnonymousPassWorldErrorV1::Output(GovernanceStartOutputBudgetErrorV1::Refused(
                "rows beyond the checked capacity",
            )),
        ] {
            assert!(!verdict.is_local());
            assert!(matches!(
                parliament_error::<()>(verdict),
                StateBlockStartError::Policy(_)
            ));
        }
    }

    #[test]
    fn a_block_outside_the_global_chain_runs_the_baseline_enactments_alone() {
        // A block without a native schedule request has no root scope: no pass, no
        // partition, and the step before the enactments still runs exactly once.
        let state = blank_state();
        let mut block = state.block(header(2));
        let mut calls = 0;
        run::<()>(&mut block, |_| {
            calls += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(calls, 1);
        assert!(!block.governance_due_applied());
        assert!(block.governance_start_rows().is_none());
        assert!(
            !block.parliament_pass_ran(),
            "a block outside the global chain has no board to decide a ballot against"
        );
    }

    #[test]
    fn the_global_chain_of_a_root_without_the_family_reserves_no_partition() {
        let state = blank_state();
        let mut block = state.block(header(2));
        let mut calls = 0;
        run_on_global_chain::<()>(&mut block, |_| {
            calls += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(calls, 1);
        assert!(!block.governance_due_applied());
        assert!(
            block.governance_start_rows().is_none(),
            "a root whose selection does not install the anonymous family reserves no governance-start partition"
        );
        assert!(!block.parliament_pass_ran());
    }
}
