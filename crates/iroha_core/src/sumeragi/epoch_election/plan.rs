//! Borrowed original-prestate decision, before funded boundary payload materialization.
//!
//! This is deliberately not the finalizer capability: its references prevent transactions
//! from mutating the source while it is being read. The block owner must finish materializing
//! its immutable funded guard before releasing these borrows and beginning execution.

use super::*;
use iroha_data_model::{
    isi::kagemusha_v1::{
        KagemushaMintFinalityAuthorityGenerationV1, KagemushaMintFinalityEpochAuthorizationV1,
        KagemushaMintFinalityEpochDecisionV1,
    },
    nexus::ValidatorCommitteeTransitionV1,
};

/// A complete checked boundary decision that still borrows its selecting prestate.
pub(super) struct BoundaryInputs<'a> {
    pub(super) current: &'a ValidatorEpochContextV1,
    pub(super) selection_anchor: iroha_crypto::HashOf<iroha_data_model::block::BlockHeader>,
    pub(super) authorization: KagemushaMintFinalityEpochAuthorizationV1,
    pub(super) authority: &'a KagemushaMintFinalityAuthorityGenerationV1,
    pub(super) committee: &'a [iroha_data_model::sumeragi::epoch::ValidatorCommitteeMemberV1],
    pub(super) completed: Option<&'a ValidatorCommitteeTransitionV1>,
    pub(super) entropy: BoundaryEntropy,
    pub(super) future: SelectedCommittee<'a>,
    pub(super) policy: &'a ValidatorElectionPolicyV1,
    pub(super) future_first: u64,
    pub(super) future_last: u64,
    pub(super) next_params: crate::sumeragi::schedule::ChainParamsRecord,
    pub(super) after_next_params: crate::sumeragi::schedule::ChainParamsRecord,
}

/// Read all boundary decisions from B−1 once; later B transactions cannot change this result.
/// No credential renewal is needed to retain the current generation. Invalid public proof or
/// custody-ledger corruption refuses the source, while incomplete legitimate target readiness
/// produces an exact cancellation and unchanged-generation retention.
pub(super) fn boundary_inputs<'a>(
    world: &'a impl WorldReadOnly,
    hashes: &(impl BlockHashRead + ?Sized),
    current: &'a ValidatorEpochContextV1,
    policy: &'a ValidatorElectionPolicyV1,
    height: u64,
    original_budget: &AllocationBudget,
) -> Result<Option<BoundaryInputs<'a>>, BoundaryCaptureError> {
    current.validate()?;
    if height != current.authorization.last_height {
        return Ok(None);
    }
    if current.mode != iroha_data_model::parameter::system::ConsensusMode::Npos {
        return Err("a finite boundary requires the authenticated NPoS policy".into());
    }
    let parameters = world
        .sumeragi_npos_parameters()
        .ok_or("boundary lacks signed NPoS parameters")?;
    policy.validate()?;
    if policy.xor_asset_definition_id != parameters.xor_asset_definition_id
        || policy.min_self_bond != parameters.min_self_bond
        || policy.min_nomination_bond != parameters.min_nomination_bond
        || policy.max_validators != parameters.max_validators
        || policy.epoch_length_blocks != parameters.epoch_length_blocks.get()
    {
        return Err("future election policy is not the original signed prestate policy".into());
    }
    let entropy = authenticated_boundary_entropy(world, hashes, current, height)?;
    let anchor_index = height
        .checked_sub(2)
        .and_then(|index| usize::try_from(index).ok())
        .ok_or("boundary anchor index overflows")?;
    let selection_anchor = *hashes
        .hash_at(anchor_index)
        .ok_or("boundary committed anchor is absent")?;
    let next_epoch = current
        .authorization
        .epoch
        .checked_add(1)
        .ok_or("next epoch overflows")?;
    let next_first = height
        .checked_add(1)
        .ok_or("boundary successor height overflows")?;
    let schedule = world.consensus_schedule();
    if schedule.tip().and_then(|tip| tip.checked_add(1)) != Some(height)
        || schedule
            .ready(height)
            .map_err(|error| error.to_string())?
            .epoch
            != *current
    {
        return Err("boundary context is not the exact retained prestate authority".into());
    }
    let next_slot = schedule
        .get(next_first)
        .ok_or("boundary successor parameters are absent")?;
    if !matches!(
        next_slot,
        crate::sumeragi::schedule::ScheduledSlot::PendingBoundary { .. }
    ) {
        return Err("boundary successor does not retain the exact application barrier".into());
    }
    let next_params = *next_slot.params();
    let after_next_params = crate::sumeragi::schedule::ChainParamsRecord::from_parameters(
        world.parameters().sumeragi(),
    );
    let source = CheckedElectionView::new(world, policy, original_budget)?;
    let completed = world.validator_committee_transitions().get(&next_epoch);
    let (last_height, decision, transition_id, authority, committee, beacon) =
        if let Some(transition) = completed {
            if transition.outcome.is_some() {
                return Err("boundary attempt is already terminal".into());
            }
            transition
                .preparation
                .validate_against_preparing_authorization(&current.authorization)?;
            crate::state::validator_committee::verify_progress(world, transition)?;
            let preparation = &transition.preparation;
            let ready = transition.credentials.is_some()
                && transition.readiness.len() == preparation.committee.len()
                && preparation.committee.iter().all(|seat| {
                    source
                        .ready_under(
                            &preparation.eligibility,
                            &seat.validator,
                            preparation.first_height,
                            preparation.last_height,
                        )
                        .is_some()
                });
            let id = preparation.transition_id()?;
            if ready {
                let credentials = transition
                    .credentials
                    .as_ref()
                    .ok_or("ready committee lost credentials")?;
                let target = world
                    .global_beacon_key_sessions()
                    .get(&credentials.beacon.session_id)
                    .ok_or("ready beacon session is absent")?;
                if target.activated_at_height.is_some() || target.retired_at_height.is_some() {
                    return Err("target beacon session was already consumed".into());
                }
                (
                    preparation.last_height,
                    KagemushaMintFinalityEpochDecisionV1::Activate,
                    id,
                    &credentials.authority,
                    preparation.committee.as_slice(),
                    credentials.beacon,
                )
            } else {
                (
                    preparation.last_height,
                    KagemushaMintFinalityEpochDecisionV1::RetainAndCancel,
                    id,
                    &current.authority,
                    current.committee.as_slice(),
                    entropy.beacon,
                )
            }
        } else {
            (
                height
                    .checked_add(policy.epoch_length_blocks)
                    .ok_or("retained epoch end overflows")?,
                KagemushaMintFinalityEpochDecisionV1::Retain,
                [0; 32],
                &current.authority,
                current.committee.as_slice(),
                entropy.beacon,
            )
        };
    if decision != KagemushaMintFinalityEpochDecisionV1::Activate {
        let incumbent = world
            .global_beacon_key_sessions()
            .get(&entropy.beacon.session_id)
            .ok_or("retained beacon is absent")?;
        if !incumbent.is_active_at(next_first) {
            return Err("retained beacon does not cover the next epoch".into());
        }
    }
    let authorization = KagemushaMintFinalityEpochAuthorizationV1 {
        version: 1,
        network_id: current.network_id,
        epoch: next_epoch,
        first_height: next_first,
        last_height,
        authority_generation: authority.generation,
        authority_id: authority
            .authority_id()
            .map_err(|error| error.to_string())?,
        beacon: BeaconEpochBindingV1::Installed(beacon),
        previous_authorization_id: current
            .authorization
            .authorization_id()
            .map_err(|error| error.to_string())?,
        transition_id,
        decision,
    };
    authorization
        .validate_successor(&current.authorization)
        .map_err(|error| error.to_string())?;
    authorization
        .validate_against_authority(authority)
        .map_err(|error| error.to_string())?;
    let future_first = last_height
        .checked_add(1)
        .ok_or("future activation overflows")?;
    let future_last = future_first
        .checked_add(policy.epoch_length_blocks - 1)
        .ok_or("future epoch end overflows")?;
    let future_epoch = next_epoch.checked_add(1).ok_or("future epoch overflows")?;
    if world
        .validator_committee_transitions()
        .get(&future_epoch)
        .is_some()
    {
        return Err("future attempt cannot be overwritten or rerolled".into());
    }
    let future = source.select(
        current.network_id,
        current.authorization.epoch,
        entropy.election_seed,
        future_first,
        future_last,
    )?;
    Ok(Some(BoundaryInputs {
        current,
        selection_anchor,
        authorization,
        authority,
        committee,
        completed,
        entropy,
        future,
        policy,
        future_first,
        future_last,
        next_params,
        after_next_params,
    }))
}
