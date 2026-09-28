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

use super::{commitment, height::SccpHeightInputsV1, params, prune, roster, store, subjects};
use crate::{
    block::BlockValidationError,
    state::{StateBlock, StateReadOnly, StateStorageAdmissionError, WorldReadOnly},
};
use iroha_data_model::{block::BlockHeader, sccp::attestation::SccpAttestationSubjectV1};

/// Run the SCCP post-execution step of block `header` (§4.5).
///
/// `height_inputs` are the authenticated consensus inputs of the block's height: from the
/// Sumeragi core's lag-2 schedule on the production path, from the frozen v2 height context
/// on the v2 path, and for a v2 signed genesis from the height-one context
/// `build_genesis_height_context` derives from the signed genesis and its staged state. They
/// are `None` only for component fixtures that execute without consensus and for a height
/// whose inputs could not be derived; SCCP then commits leaves and history but derives no
/// roster and writes no subject (fail closed).
///
/// The steps run in one transaction, in §4.5 order: (a) commit the block's leaves and append
/// the history leaf; (b) at rotation heights apply key promotion and the roster rule; (c)
/// write the subject when the block has messages, is a boundary, or created a generation;
/// (d) record stalled handoffs; (e) prune within the per-block budget.
///
/// # Errors
///
/// Fails the block on an execution invariant violation (for example non-dense leaf indices)
/// or when local storage refuses the hook's transaction.
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
    let Some(parameters) = store::parameters::get(&state_block.world).clone() else {
        return Ok(());
    };
    let network_id = *state_block.network_id().as_bytes();
    let height = header.height().get();
    let now_ms = header.creation_time_ms;
    let mut stx = state_block
        .try_transaction()
        .map_err(BlockValidationError::StateStorageAdmission)?;

    let committed = commitment::commit_block(&mut stx, height)?;
    let step = match height_inputs {
        Some(inputs) => Some(roster::apply_roster_rule(
            &mut stx,
            &network_id,
            &parameters,
            height,
            now_ms,
            inputs,
        )?),
        None => None,
    };

    if let (Some(inputs), Some(step)) = (height_inputs, step) {
        let required = committed.is_some() || step.boundary || step.created_digest.is_some();
        let signer = roster::generation_for_height(&*stx.world, height).and_then(|generation| {
            store::rosters::get(&*stx.world, &generation).map(|roster| (generation, roster.digest))
        });
        if let (true, Some((generation, roster_digest))) = (required, signer) {
            let (history_root, history_size) = commitment::history_root_and_size(&*stx.world);
            subjects::write_subject(
                &mut stx,
                SccpAttestationSubjectV1 {
                    height,
                    epoch: inputs.epoch,
                    timestamp_ms: now_ms,
                    sccp_root: committed.map_or([0; 32], |commitment| commitment.root),
                    message_count: committed.map_or(0, |commitment| commitment.message_count),
                    history_root,
                    history_size,
                    generation,
                    roster_digest,
                    next_roster_digest: step.created_digest.unwrap_or([0; 32]),
                },
            )?;
        }
    }

    subjects::record_stalled_handoffs(&mut stx, parameters.attestation_stall_ms, now_ms)?;
    prune::prune(&mut stx, now_ms, prune::MAX_PRUNE_DELETIONS_PER_BLOCK);
    stx.apply();
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
    roster::heartbeat_due(world, candidate_header.creation_time_ms)
        .is_some_and(|generation| *store::heartbeat_marker::get(world) != Some(generation))
}

/// Apply SCCP block-start work of `header` (the heartbeat marker, §4.3.2).
///
/// The marker fires at most once per generation, which makes the heartbeat block
/// deterministic start-of-block work even on an idle Taira.
///
/// # Errors
///
/// Propagates local storage admission failures of the block-start transaction.
pub fn apply_block_start(
    state_block: &mut StateBlock<'_>,
    header: &BlockHeader,
) -> Result<(), StateStorageAdmissionError> {
    if !heartbeat_start_work_pending(&state_block.world, header) {
        return Ok(());
    }
    let Some(generation) = roster::heartbeat_due(&state_block.world, header.creation_time_ms)
    else {
        return Ok(());
    };
    let mut stx = state_block.try_transaction()?;
    store::heartbeat_marker::set(&mut stx, Some(generation));
    stx.apply();
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
    fn keyless_or_empty_rosters_fail_closed_without_failing_the_block() {
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
        finalize_block(&mut block, &candidate, Some(&inputs)).expect("empty roster fails closed");
        finalize_block(&mut block, &candidate, None).expect("no inputs: no roster work");
        assert!(store::rosters::is_empty(&block.world));
        assert_eq!(observed::take(), vec![(2, Some(inputs)), (2, None)]);
    }

    #[test]
    fn genesis_creates_generation_one_and_the_heartbeat_fires_once() {
        use crate::smartcontracts::isi::sccp::test_support::peer;
        use iroha_data_model::sccp::keys::{SccpBridgeKeyStateV1, SccpBridgeKeyV1};
        use std::num::NonZeroU64;

        let state = blank_state();
        let genesis = header(1);
        let mut block = state.block(genesis);
        {
            let mut stx = block.transaction();
            store::parameters::set(&mut stx, Some(SccpParametersV1::taira_default()));
            for seed in 1..=4_u8 {
                let key = SccpBridgeKeyV1 {
                    public_key: [2; 33],
                    address: [seed * 16; 20],
                    activation_epoch: 0,
                    registered_at_height: 1,
                    faulted: false,
                };
                store::bridge_keys::insert(
                    &mut stx,
                    peer(seed),
                    SccpBridgeKeyStateV1 {
                        active: Some(key),
                        ..SccpBridgeKeyStateV1::default()
                    },
                )
                .expect("key");
            }
            stx.apply();
        }
        let inputs = SccpHeightInputsV1 {
            mode: iroha_data_model::parameter::system::ConsensusMode::Npos,
            height: 1,
            epoch: 0,
            epoch_end_height: 1_000_000,
            roster: (1..=4).map(peer).collect(),
            next_roster: None,
        };
        finalize_block(&mut block, &genesis, Some(&inputs)).expect("genesis hook");
        let (generation, first) = roster::current(&block.world).expect("generation 1");
        assert_eq!(generation, 1);
        assert!(!first.is_inert());
        assert!(
            !subjects::exists(&block.world, 1),
            "no messages, no boundary"
        );

        let max_age = SccpParametersV1::taira_default().roster_max_age_ms;
        let early = BlockHeader::new(
            NonZeroU64::new(2).expect("nonzero"),
            None,
            None,
            first.valid_from_ms + max_age - 1,
            0,
        );
        assert!(!heartbeat_start_work_pending(&block.world, &early));
        let due = BlockHeader::new(
            NonZeroU64::new(2).expect("nonzero"),
            None,
            None,
            first.valid_from_ms + max_age,
            0,
        );
        assert!(heartbeat_start_work_pending(&block.world, &due));
        apply_block_start(&mut block, &due).expect("marker");
        assert_eq!(*store::heartbeat_marker::get(&block.world), Some(1));
        assert!(
            !heartbeat_start_work_pending(&block.world, &due),
            "the marker fires once per generation"
        );
    }
}
