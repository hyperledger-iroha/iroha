//! SCCP block hooks (`specs/sccp.md` §4.3.2, §4.5). Owner: ws30.
//!
//! * [`finalize_block`] runs in `ValidBlock::finalize_owned_execution_metadata` after every
//!   transaction of block `h` and before the output seal, with the consensus inputs of `h`
//!   ([`SccpHeightInputsV1`]): it commits the block's leaves, appends the history leaf, applies
//!   bridge-key promotion and the roster rule at rotation heights, writes the attestation
//!   subject and prunes.
//! * [`heartbeat_start_work_pending`] tells `State::deterministic_start_work_pending` that the
//!   candidate block must be produced even with an empty queue because its creation time
//!   crosses the current generation's heartbeat.
//! * [`apply_block_start`] runs at block start, next to the due Parliament certificates and
//!   before the block's transactions, and writes the heartbeat marker.
//!
//! With SCCP absent every hook is a no-op.

use super::{height::SccpHeightInputsV1, params};
use crate::{
    block::BlockValidationError,
    state::{StateBlock, StateStorageAdmissionError, WorldReadOnly},
};
use iroha_data_model::block::BlockHeader;

/// Run the SCCP post-execution step of block `header` (§4.5).
///
/// `height_inputs` are the authenticated consensus inputs of the block's height: from the
/// Sumeragi core's lag-2 schedule on the production path, from the frozen v2 height context
/// on the v2 path, and for a v2 signed genesis from the height-one context
/// `build_genesis_height_context` derives from the signed genesis and its staged state. They
/// are `None` only for component fixtures that execute without consensus and for a height
/// whose inputs could not be derived; SCCP then fails closed for roster derivation.
///
/// # Errors
///
/// Fails the block on an execution invariant violation (for example non-dense leaf indices).
pub fn finalize_block(
    state_block: &mut StateBlock<'_>,
    header: &BlockHeader,
    height_inputs: Option<&SccpHeightInputsV1>,
) -> Result<(), BlockValidationError> {
    if !params::exists(&state_block.world) {
        return Ok(());
    }
    #[cfg(test)]
    observed::record(header, height_inputs);
    let _ = (header, height_inputs);
    // TODO(ws30): commitments, history, promotion, roster rule, subjects and pruning (§4.5).
    Ok(())
}

/// Test observation of the inputs [`finalize_block`] received, per executing thread.
///
/// Block validation runs the post-execution finalizer on the validating thread, so a test that
/// validates a block observes exactly the calls of its own blocks.
#[cfg(test)]
pub(crate) mod observed {
    use super::{BlockHeader, SccpHeightInputsV1};
    use std::cell::RefCell;

    std::thread_local! {
        static CALLS: RefCell<Vec<(u64, Option<SccpHeightInputsV1>)>> =
            const { RefCell::new(Vec::new()) };
    }

    pub(super) fn record(header: &BlockHeader, inputs: Option<&SccpHeightInputsV1>) {
        CALLS.with(|calls| {
            calls
                .borrow_mut()
                .push((header.height().get(), inputs.cloned()));
        });
    }

    /// Take the `(height, inputs)` of every SCCP finalizer call on this thread so far.
    pub(crate) fn take() -> Vec<(u64, Option<SccpHeightInputsV1>)> {
        CALLS.with(|calls| core::mem::take(&mut *calls.borrow_mut()))
    }
}

/// Return whether block start of `candidate_header` has SCCP work: its creation time reaches
/// the heartbeat of the current generation and the heartbeat marker has not fired for it
/// (§4.3.2). `world` is the committed parent state.
#[must_use]
pub fn heartbeat_start_work_pending(
    world: &(impl WorldReadOnly + ?Sized),
    candidate_header: &BlockHeader,
) -> bool {
    if !params::exists(world) {
        return false;
    }
    let _ = candidate_header;
    // TODO(ws30): `creation_time_ms − valid_from_ms(g) ≥ roster_max_age_ms` and marker ≠ g.
    false
}

/// Apply SCCP block-start work of `header` (the heartbeat marker, §4.3.2).
///
/// # Errors
///
/// Propagates local storage admission failures of the block-start transaction.
pub fn apply_block_start(
    state_block: &mut StateBlock<'_>,
    header: &BlockHeader,
) -> Result<(), StateStorageAdmissionError> {
    if !params::exists(&state_block.world) {
        return Ok(());
    }
    let _ = header;
    // TODO(ws30): write `sccp_heartbeat_marker = Some(g)` when the heartbeat fires.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::{
        store,
        test_support::{blank_state, header},
    };
    use iroha_data_model::sccp::params::SccpParametersV1;

    #[test]
    fn hooks_are_no_ops_without_sccp() {
        let state = blank_state();
        let candidate = header(2);
        assert!(!heartbeat_start_work_pending(
            &state.world_view(),
            &candidate
        ));
        let mut block = state.block(candidate);
        apply_block_start(&mut block, &candidate).expect("no-op block start");
        let _ = observed::take();
        finalize_block(&mut block, &candidate, None).expect("no-op finalizer");
        assert!(
            observed::take().is_empty(),
            "the finalizer returns before reading inputs without SCCP"
        );
        assert!(store::block_commitments::is_empty(&block.world));
        assert_eq!(*store::heartbeat_marker::get(&block.world), None);
    }

    #[test]
    fn skeleton_hooks_are_neutral_with_sccp_present() {
        let state = blank_state();
        let candidate = header(2);
        let mut block = state.block(candidate);
        {
            let mut stx = block.transaction();
            store::parameters::set(&mut stx, Some(SccpParametersV1::taira_default()));
            stx.apply();
        }
        assert!(!heartbeat_start_work_pending(&block.world, &candidate));
        apply_block_start(&mut block, &candidate).expect("no-op block start");
        let _ = observed::take();
        let inputs = SccpHeightInputsV1 {
            mode: iroha_data_model::parameter::system::ConsensusMode::Npos,
            height: 2,
            epoch: 0,
            epoch_end_height: 2,
            roster: Vec::new(),
            next_roster: Some(Vec::new()),
        };
        finalize_block(&mut block, &candidate, Some(&inputs)).expect("no-op finalizer");
        finalize_block(&mut block, &candidate, None).expect("no-op finalizer");
        assert_eq!(observed::take(), vec![(2, Some(inputs)), (2, None)]);
    }
}
