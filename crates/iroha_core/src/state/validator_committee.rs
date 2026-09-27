//! Authenticated preparation of immutable future validator committees.

use super::{
    BlockHashRead, GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, StateBlock, StateReadOnly,
    StateTransaction, WorldReadOnly, public_lane_validator_record_matches_key,
};
use crate::{
    beacon::{
        GlobalThresholdBeaconSessionBindingV1,
        authenticated_global_threshold_beacon_roster_hash_v1,
        seat_readiness::verify_global_threshold_beacon_seat_readiness_v1,
        validate_global_threshold_beacon_session_v1,
    },
    zk::kagemusha_v1_recursion::{
        verify_kagemusha_mint_finality_candidate_possession_v1,
        verify_kagemusha_mint_finality_seat_readiness_v1,
    },
};
use iroha_data_model::{
    account::AccountId,
    isi::kagemusha_v1::{
        BeaconEpochBindingV1, KagemushaMintFinalityAuthorityGenerationV1,
        KagemushaMintFinalityEpochAuthorizationV1, KagemushaMintFinalityEpochDecisionV1,
    },
    nexus::{
        ValidatorCandidateKeysV1, ValidatorCommitteeOperationV1, ValidatorCommitteeTransitionV1,
    },
};
use iroha_model_base::peer::PeerId;
use iroha_model_base::topology::LaneId;
use mv::storage::StorageReadOnly;

#[cfg(test)]
#[path = "validator_committee/tests.rs"]
pub(crate) mod tests;

/// A requested exit cannot release either a current seat or an immutable future seat.
#[expect(
    single_use_lifetimes,
    reason = "stable Rust requires a named reference lifetime in the iterator item"
)]
pub(crate) fn peer_has_committee_obligation<'a>(
    world: &impl WorldReadOnly,
    current: impl IntoIterator<Item = &'a PeerId>,
    peer: &PeerId,
) -> bool {
    current.into_iter().any(|seat| seat == peer)
        || world
            .validator_committee_transitions()
            .iter()
            .any(|(_, transition)| {
                transition.outcome.is_none()
                    && transition
                        .preparation
                        .roster
                        .iter()
                        .any(|seat| &seat.validator == peer)
            })
}

struct StakingBoundaryUpdates {
    validators: Vec<(
        (LaneId, AccountId),
        iroha_data_model::nexus::PublicLaneValidatorRecord,
    )>,
    shares: Vec<(
        (LaneId, AccountId, AccountId),
        iroha_data_model::nexus::PublicLaneStakeShare,
    )>,
}

/// Compute all custody extensions before publishing any part of a committee decision.
fn prepare_staking_obligations(
    world: &impl WorldReadOnly,
    snapshot: &iroha_data_model::block::consensus_v2::finality::FinalizedNextEpochSnapshot,
) -> Result<StakingBoundaryUpdates, String> {
    let mut updates = StakingBoundaryUpdates {
        validators: Vec::new(),
        shares: Vec::new(),
    };
    if snapshot.mode != iroha_data_model::block::consensus_v2::ConsensusMode::Npos {
        return Ok(updates);
    }
    let parameters = world
        .sumeragi_npos_parameters()
        .ok_or("staking boundary lacks signed NPoS parameters")?;
    let outcome = &snapshot.kagemusha_mint_finality_authorization;
    let mut obligations = std::collections::BTreeMap::<PeerId, u64>::new();
    for seat in &snapshot.roster {
        obligations.insert(seat.validator.clone(), outcome.last_height);
    }
    for preparation in world
        .validator_committee_transitions()
        .iter()
        .filter(|(epoch, transition)| **epoch != outcome.epoch && transition.outcome.is_none())
        .map(|(_, transition)| &transition.preparation)
        .chain(snapshot.committee_preparation.iter())
    {
        for seat in &preparation.roster {
            let through = obligations.entry(seat.validator.clone()).or_default();
            *through = (*through).max(preparation.last_height);
        }
    }
    for (key, record) in world
        .public_lane_validators()
        .iter()
        .filter(|(key, _)| key.0 == LaneId::SINGLE)
    {
        if !public_lane_validator_record_matches_key(key, record) {
            return Err("committee custody has a noncanonical validator owner".to_owned());
        }
        let through = obligations.get(&record.peer_id).copied();
        if let Some(through) = through {
            if record.deactivation_height.is_some_and(|end| end <= through) {
                return Err(
                    "committee decision retains an already ended validator tenure".to_owned(),
                );
            }
            let release = through
                .checked_add(parameters.evidence_horizon_blocks())
                .and_then(|height| height.checked_add(parameters.slashing_delay_blocks()))
                .ok_or("committee custody liability height overflows")?;
            for (share_key, share) in world
                .public_lane_stake_shares()
                .iter()
                .filter(|(share_key, _)| share_key.0 == key.0 && share_key.1 == key.1)
            {
                let mut retained = share.clone();
                for pending in retained.pending_unbonds.values_mut() {
                    pending.slashable_through_height =
                        pending.slashable_through_height.max(through);
                    pending.liability_release_height =
                        pending.liability_release_height.max(release);
                }
                if &retained != share {
                    updates.shares.push((share_key.clone(), retained));
                }
            }
        } else if let Some(requested) = record.election_exit_height {
            if record.deactivation_height.is_none() && requested <= outcome.first_height {
                let mut ended = record.clone();
                ended.deactivation_height = Some(
                    outcome
                        .first_height
                        .max(requested)
                        .max(record.activation_height),
                );
                updates.validators.push((key.clone(), ended));
            }
        }
    }
    Ok(updates)
}

fn add_obligations(
    obligations: &mut std::collections::BTreeMap<PeerId, u64>,
    peers: impl IntoIterator<Item = PeerId>,
    through: u64,
) {
    for peer in peers {
        let retained = obligations.entry(peer).or_default();
        *retained = (*retained).max(through);
    }
}

/// Snapshot metadata cannot shorten stake liability proved by retained committee history.
fn validate_retained_staking_obligations(
    world: &impl WorldReadOnly,
    historical: &std::collections::BTreeMap<PeerId, u64>,
    live: &std::collections::BTreeMap<PeerId, u64>,
) -> Result<(), String> {
    let mut covered = std::collections::BTreeSet::new();
    for (key, record) in world
        .public_lane_validators()
        .iter()
        .filter(|(key, _)| key.0 == LaneId::SINGLE)
    {
        if !public_lane_validator_record_matches_key(key, record) {
            return Err(
                "retained committee custody has a substituted validator identity".to_owned(),
            );
        }
        if let Some(through) = live.get(&record.peer_id) {
            if !covered.insert(record.peer_id.clone())
                || record
                    .deactivation_height
                    .is_some_and(|end| end <= *through)
            {
                return Err(
                    "snapshot revokes or duplicates a retained committee obligation".to_owned(),
                );
            }
        }
        let Some(through) = historical
            .get(&record.peer_id)
            .copied()
            .filter(|height| *height >= record.activation_height)
        else {
            continue;
        };
        for (_, share) in world
            .public_lane_stake_shares()
            .iter()
            .filter(|(share_key, _)| share_key.0 == key.0 && share_key.1 == key.1)
        {
            if share.pending_unbonds.values().any(|pending| {
                pending.slashable_through_height < through
                    || pending.liability_release_height < pending.slashable_through_height
            }) {
                return Err("snapshot shortens pending stake liability below authenticated committee service".to_owned());
            }
        }
    }
    if live.keys().any(|peer| !covered.contains(peer)) {
        return Err("snapshot omits the owner of a current or frozen committee seat".to_owned());
    }
    Ok(())
}

/// Check every persisted publication and preparation, including exact storage identities.
pub(crate) fn validate_persisted_progress(world: &impl WorldReadOnly) -> Result<(), String> {
    let mut eq_keys = std::collections::BTreeSet::new();
    let mut ep_keys = std::collections::BTreeSet::new();
    for (key, publication) in world.validator_candidate_keys().iter() {
        verify_candidate(publication)?;
        if *key
            != ValidatorCandidateKeysV1::key_id(
                publication.network_id,
                publication.generation,
                &publication.keys.validator,
            )
            || !eq_keys.insert(publication.keys.eq_proof_public_key)
            || !ep_keys.insert(publication.keys.ep_proof_public_key)
        {
            return Err(
                "candidate snapshot changes identity or duplicates signing keys".to_owned(),
            );
        }
    }
    for (epoch, transition) in world.validator_committee_transitions().iter() {
        if *epoch != transition.preparation.target_epoch {
            return Err("committee snapshot key differs from its exact target epoch".to_owned());
        }
        verify_progress(world, transition)?;
    }
    Ok(())
}

/// Read one exact authenticated artifact at the snapshot's committed chain cut.
fn committed_artifact(
    kura: &crate::kura::Kura,
    network: iroha_data_model::NetworkId,
    hashes: &[iroha_crypto::HashOf<iroha_data_model::block::BlockHeader>],
    height: u64,
) -> Result<iroha_data_model::block::consensus_v2::finality::V2FinalityArtifact, String> {
    let index = usize::try_from(height)
        .ok()
        .and_then(|height| height.checked_sub(1))
        .ok_or("invalid committee finality height")?;
    let expected = hashes
        .get(index)
        .ok_or("committee finality exceeds the committed cut")?;
    let artifact = kura
        .v2_finality_artifact(height)
        .map_err(|error| error.to_string())?
        .ok_or("committee snapshot requires retained latest and epoch-boundary finality")?;
    if artifact.height != height
        || artifact.block_hash != *expected
        || artifact.height_context.network_id != network
    {
        return Err("committee finality differs from the exact committed chain cut".to_owned());
    }
    Ok(artifact)
}

/// Match the currently effective authorization to the exact persisted signing session.
fn validate_current_beacon(
    world: &impl WorldReadOnly,
    authority: &KagemushaMintFinalityAuthorityGenerationV1,
    authorization: &KagemushaMintFinalityEpochAuthorizationV1,
    committed_height: u64,
) -> Result<(), String> {
    authorization
        .validate_against_authority(authority)
        .map_err(|error| error.to_string())?;
    let active = world.active_global_beacon_key_session();
    let (session_id, transcript_hash, latest_activation) = match authorization.beacon {
        BeaconEpochBindingV1::Installed(binding) => {
            if active != Some(binding.session_id) {
                return Err(
                    "committee authorization differs from the active beacon pointer".to_owned(),
                );
            }
            (
                binding.session_id,
                Some(binding.transcript_hash),
                authorization.first_height,
            )
        }
        BeaconEpochBindingV1::Bootstrap => {
            let Some(session_id) = active else {
                if world.global_beacon_key_sessions().iter().next().is_some() {
                    return Err(
                        "bootstrap has a finalized session without its activation".to_owned()
                    );
                }
                return Ok(());
            };
            (
                session_id,
                None,
                committed_height
                    .checked_add(1)
                    .ok_or("bootstrap height overflows")?,
            )
        }
    };
    let record = world
        .global_beacon_key_sessions()
        .get(&session_id)
        .ok_or("committee authorization lacks its exact beacon session")?;
    record.validate().map_err(|error| error.to_string())?;
    if record.session.network_id != authority.network_id
        || transcript_hash.is_some_and(|hash| hash != record.session.transcript_hash)
        || record
            .activated_at_height
            .is_none_or(|height| height > latest_activation)
        || record.retired_at_height.is_some()
        || record.session.adaptive_dkg.finalized_at_height > committed_height
        || (authorization.decision == KagemushaMintFinalityEpochDecisionV1::Activate
            && record.activated_at_height != Some(authorization.first_height))
    {
        return Err("committee authorization differs from the active beacon lifecycle".to_owned());
    }
    let peers = authority
        .validators
        .iter()
        .map(|keys| keys.validator.clone())
        .collect::<Vec<_>>();
    authenticated_global_threshold_beacon_roster_hash_v1(&record.session, &peers)
        .map_err(|error| error.to_string())?;
    Ok(())
}

/// Authenticate both presence and absence at every retained epoch boundary.
pub(crate) fn validate_committed_progress(
    world: &impl WorldReadOnly,
    network: iroha_data_model::NetworkId,
    hashes: &[iroha_crypto::HashOf<iroha_data_model::block::BlockHeader>],
    kura: &crate::kura::Kura,
) -> Result<(), String> {
    validate_persisted_progress(world)?;
    for (_, candidate) in world.validator_candidate_keys().iter() {
        if candidate.network_id != network {
            return Err("candidate snapshot names another network".to_owned());
        }
    }
    let height = u64::try_from(hashes.len()).map_err(|_| "committed height overflows")?;
    if height == 0 {
        if world.validator_candidate_keys().iter().next().is_some()
            || world
                .validator_committee_transitions()
                .iter()
                .next()
                .is_some()
            || world.global_beacon_key_sessions().iter().next().is_some()
            || world.active_global_beacon_key_session().is_some()
        {
            return Err("uncommitted State invents committee or beacon progress".to_owned());
        }
        return Ok(());
    }
    let mut cursor = committed_artifact(kura, network, hashes, height)?;
    let (authority, authorization) = match &cursor.height_context.next_epoch_snapshot {
        Some(snapshot) => (
            &snapshot.kagemusha_mint_finality_authority,
            &snapshot.kagemusha_mint_finality_authorization,
        ),
        None => (
            &cursor.height_context.kagemusha_mint_finality_authority,
            &cursor.height_context.kagemusha_mint_finality_authorization,
        ),
    };
    validate_current_beacon(world, authority, authorization, height)?;
    let mut historical_obligations = std::collections::BTreeMap::new();
    let mut live_obligations = std::collections::BTreeMap::new();
    if cursor.height_context.mode == iroha_data_model::block::consensus_v2::ConsensusMode::Npos {
        add_obligations(
            &mut live_obligations,
            authority
                .validators
                .iter()
                .map(|keys| keys.validator.clone()),
            authorization.last_height,
        );
        for (_, transition) in world
            .validator_committee_transitions()
            .iter()
            .filter(|(_, transition)| transition.outcome.is_none())
        {
            add_obligations(
                &mut live_obligations,
                transition
                    .preparation
                    .roster
                    .iter()
                    .map(|seat| seat.validator.clone()),
                transition.preparation.last_height,
            );
        }
        historical_obligations.clone_from(&live_obligations);
    }
    let mut observed = std::collections::BTreeSet::new();
    loop {
        let context = &cursor.height_context;
        if context.mode == iroha_data_model::block::consensus_v2::ConsensusMode::Npos {
            add_obligations(
                &mut historical_obligations,
                context.roster.iter().map(|seat| seat.validator.clone()),
                context.kagemusha_mint_finality_authorization.last_height,
            );
        }
        if let Some(snapshot) = &context.next_epoch_snapshot {
            let outcome = &snapshot.kagemusha_mint_finality_authorization;
            outcome
                .validate_successor(&context.kagemusha_mint_finality_authorization)
                .map_err(|error| error.to_string())?;
            if let Some(preparation) = &snapshot.committee_preparation {
                let transition = world
                    .validator_committee_transitions()
                    .get(&preparation.target_epoch)
                    .ok_or("committee snapshot omits an incumbent-certified preparation")?;
                if &transition.preparation != preparation
                    || preparation.selection_height != cursor.height
                    || cursor.subject.parent_block_hash != Some(preparation.selection_anchor)
                    || !observed.insert(preparation.target_epoch)
                {
                    return Err(
                        "committee snapshot changes or duplicates a certified preparation"
                            .to_owned(),
                    );
                }
                preparation.validate_against_preparing_authorization(outcome)?;
                add_obligations(
                    &mut historical_obligations,
                    preparation.roster.iter().map(|seat| seat.validator.clone()),
                    preparation.last_height,
                );
                if preparation.first_height - 1 > height && transition.outcome.is_some() {
                    return Err("committee snapshot invents a future terminal decision".to_owned());
                }
            }
            match outcome.decision {
                KagemushaMintFinalityEpochDecisionV1::Activate
                | KagemushaMintFinalityEpochDecisionV1::RetainAndCancel => {
                    let transition = world
                        .validator_committee_transitions()
                        .get(&outcome.epoch)
                        .ok_or("committee snapshot omits a certified terminal attempt")?;
                    if transition.outcome.as_ref() != Some(outcome) {
                        return Err("committee snapshot terminal decision differs from authenticated finality".to_owned());
                    }
                    if outcome.decision == KagemushaMintFinalityEpochDecisionV1::Activate {
                        let BeaconEpochBindingV1::Installed(previous) =
                            context.kagemusha_mint_finality_authorization.beacon
                        else {
                            return Err("activation finality lacks an installed incumbent beacon"
                                .to_owned());
                        };
                        let BeaconEpochBindingV1::Installed(next) = outcome.beacon else {
                            return Err("activation finality lacks its target beacon".to_owned());
                        };
                        let old = world
                            .global_beacon_key_sessions()
                            .get(&previous.session_id)
                            .ok_or("activation snapshot omits incumbent beacon history")?;
                        let new = world
                            .global_beacon_key_sessions()
                            .get(&next.session_id)
                            .ok_or("activation snapshot omits target beacon history")?;
                        if old.session.transcript_hash != previous.transcript_hash
                            || old.retired_at_height != Some(outcome.first_height)
                            || new.session.transcript_hash != next.transcript_hash
                            || new.activated_at_height != Some(outcome.first_height)
                        {
                            return Err(
                                "beacon lifecycle does not match the certified activation cut"
                                    .to_owned(),
                            );
                        }
                    }
                }
                KagemushaMintFinalityEpochDecisionV1::Retain => {
                    if world
                        .validator_committee_transitions()
                        .get(&outcome.epoch)
                        .is_some()
                    {
                        return Err(
                            "retention finality omitted cancellation of a frozen attempt"
                                .to_owned(),
                        );
                    }
                }
                KagemushaMintFinalityEpochDecisionV1::Genesis => {
                    return Err("epoch boundary resets genesis authorization".to_owned());
                }
            }
        }
        let authorization = context.kagemusha_mint_finality_authorization;
        if authorization.first_height == 1 {
            if authorization.decision != KagemushaMintFinalityEpochDecisionV1::Genesis {
                return Err(
                    "boundary history does not terminate at genesis authorization".to_owned(),
                );
            }
            break;
        }
        let previous_height = authorization
            .first_height
            .checked_sub(1)
            .filter(|previous| *previous < cursor.height)
            .ok_or("epoch authorization does not advance its boundary height")?;
        let previous = committed_artifact(kura, network, hashes, previous_height)?;
        if previous
            .height_context
            .next_epoch_snapshot
            .as_ref()
            .map(|snapshot| &snapshot.kagemusha_mint_finality_authorization)
            != Some(&authorization)
        {
            return Err("epoch boundary history changes the following authorization".to_owned());
        }
        cursor = previous;
    }
    if observed.len() != world.validator_committee_transitions().iter().count() {
        return Err("committee snapshot contains an uncertified preparation".to_owned());
    }
    validate_retained_staking_obligations(world, &historical_obligations, &live_obligations)
}

/// Resolve current signing authority exclusively from authenticated finalized history.
pub(crate) fn current_authority(
    state: &impl StateReadOnly,
) -> Result<
    (
        KagemushaMintFinalityAuthorityGenerationV1,
        KagemushaMintFinalityEpochAuthorizationV1,
    ),
    String,
> {
    let height = u64::try_from(state.height()).map_err(|_| "committed height overflows")?;
    let artifact = state
        .kura()
        .v2_finality_artifact(height)
        .map_err(|error| error.to_string())?
        .ok_or("validator preparation requires authenticated incumbent finality")?;
    if artifact.height != height
        || artifact.height_context.network_id != *state.network_id()
        || state.latest_block_hash() != Some(artifact.block_hash)
    {
        return Err("incumbent finality differs from the exact committed State anchor".to_owned());
    }
    let context = artifact.height_context;
    Ok(match context.next_epoch_snapshot {
        Some(snapshot) => (
            snapshot.kagemusha_mint_finality_authority,
            snapshot.kagemusha_mint_finality_authorization,
        ),
        None => (
            context.kagemusha_mint_finality_authority,
            context.kagemusha_mint_finality_authorization,
        ),
    })
}

/// Admit a finalized public transcript without giving it independent rotation authority.
/// The returned flag permits next-height activation only for the genesis bootstrap.
pub(crate) fn validate_beacon_finalization(
    state: &StateTransaction<'_, '_>,
    record: &crate::beacon::FinalizedGlobalThresholdBeaconKeySessionRecordV1,
    authorizing_roster: &[PeerId],
) -> Result<bool, String> {
    let (authority, authorization) = current_authority(state)?;
    validate_beacon_preparation(
        &state.world,
        state.block_height(),
        &authority,
        &authorization,
        record,
        authorizing_roster,
    )
}

fn validate_beacon_preparation(
    world: &impl WorldReadOnly,
    height: u64,
    authority: &KagemushaMintFinalityAuthorityGenerationV1,
    authorization: &KagemushaMintFinalityEpochAuthorizationV1,
    record: &crate::beacon::FinalizedGlobalThresholdBeaconKeySessionRecordV1,
    authorizing_roster: &[PeerId],
) -> Result<bool, String> {
    authorization
        .validate_against_authority(authority)
        .map_err(|error| error.to_string())?;
    record.validate().map_err(|error| error.to_string())?;
    if !(authorization.first_height..=authorization.last_height).contains(&height)
        || record.session.network_id != authority.network_id
        || record.activated_at_height.is_some()
        || record.retired_at_height.is_some()
        || record.session.adaptive_dkg.finalized_at_height > height
        || !authority
            .validators
            .iter()
            .map(|keys| &keys.validator)
            .eq(authorizing_roster.iter())
    {
        return Err(
            "beacon finalization differs from its authenticated execution context".to_owned(),
        );
    }
    let active = world.active_global_beacon_key_session();
    if active.is_none() {
        if authority.generation != 0 || authorization.beacon != BeaconEpochBindingV1::Bootstrap {
            return Err(
                "only the authenticated genesis authority may bootstrap a beacon".to_owned(),
            );
        }
        authenticated_global_threshold_beacon_roster_hash_v1(&record.session, authorizing_roster)
            .map_err(|error| error.to_string())?;
        return Ok(true);
    }
    let BeaconEpochBindingV1::Installed(incumbent) = authorization.beacon else {
        return Err("a pending beacon requires the installed incumbent authorization".to_owned());
    };
    let target_epoch = authorization
        .epoch
        .checked_add(1)
        .ok_or("target epoch overflows")?;
    let transition = world
        .validator_committee_transitions()
        .get(&target_epoch)
        .ok_or("beacon finalization requires an authenticated frozen committee")?;
    transition.validate()?;
    let preparation = &transition.preparation;
    preparation.validate_against_preparing_authorization(authorization)?;
    let current = world
        .global_beacon_key_sessions()
        .get(&incumbent.session_id)
        .ok_or("incumbent beacon public state is absent")?;
    if active != Some(incumbent.session_id)
        || current.session.transcript_hash != incumbent.transcript_hash
        || !current.is_active_at(height)
        || transition.outcome.is_some()
        || height >= authorization.last_height
        || record.session.session_id != preparation.beacon_session_id()?
        || record.session.adaptive_dkg.session.attempt_id != preparation.transition_id()?
        || record.session.adaptive_dkg.session.authority_generation
            != preparation.authority_generation
        || record.session.adaptive_dkg.session.start_height <= preparation.selection_height
        || record.session.adaptive_dkg.finalized_at_height >= preparation.first_height - 1
    {
        return Err(
            "beacon finalization does not belong to the live preparation attempt".to_owned(),
        );
    }
    let target_roster = preparation
        .roster
        .iter()
        .map(|seat| seat.validator.clone())
        .collect::<Vec<_>>();
    authenticated_global_threshold_beacon_roster_hash_v1(&record.session, &target_roster)
        .map_err(|error| error.to_string())?;
    Ok(false)
}

impl StateBlock<'_> {
    /// Stage the complete certified boundary effect in the original execution overlay.
    /// Publication of this overlay is still gated by the current committee's exact finality.
    pub(crate) fn finalize_validator_committee_boundary(
        &mut self,
        context: &iroha_data_model::block::consensus_v2::HeightContext,
    ) -> Result<(), String> {
        context.validate().map_err(|error| error.to_string())?;
        let Some(snapshot) = &context.next_epoch_snapshot else {
            return Ok(());
        };
        if context.height != self._curr_block.height().get()
            || context.network_id != self.network_id
        {
            return Err("committee boundary belongs to another execution context".to_owned());
        }
        let outcome = &snapshot.kagemusha_mint_finality_authorization;
        let mut completed = None;
        let mut beacon_rotation = None;
        match outcome.decision {
            KagemushaMintFinalityEpochDecisionV1::Activate
            | KagemushaMintFinalityEpochDecisionV1::RetainAndCancel => {
                let mut transition = self
                    .world
                    .validator_committee_transitions
                    .get(&snapshot.epoch)
                    .cloned()
                    .ok_or("boundary decision lacks its frozen committee attempt")?;
                if transition.outcome.is_some() {
                    return Err("committee attempt is already terminal".to_owned());
                }
                transition
                    .preparation
                    .validate_against_preparing_authorization(
                        &context.kagemusha_mint_finality_authorization,
                    )?;
                verify_progress(&self.world, &transition)?;
                transition.outcome = Some(*outcome);
                transition.validate()?;
                if outcome.decision == KagemushaMintFinalityEpochDecisionV1::Activate {
                    let credentials = transition
                        .credentials
                        .as_ref()
                        .ok_or("activated committee lacks prepared credentials")?;
                    if credentials.authority != snapshot.kagemusha_mint_finality_authority
                        || transition.preparation.roster != snapshot.roster
                        || transition.preparation.validator_set_pops != snapshot.validator_set_pops
                    {
                        return Err(
                            "activation substitutes the frozen committee or its exact credentials"
                                .to_owned(),
                        );
                    }
                    let BeaconEpochBindingV1::Installed(previous) =
                        context.kagemusha_mint_finality_authorization.beacon
                    else {
                        return Err(
                            "committee activation requires an installed incumbent beacon"
                                .to_owned(),
                        );
                    };
                    if self.world.active_global_beacon_key_session() != Some(previous.session_id) {
                        return Err(
                            "committee activation lost the exact incumbent beacon".to_owned()
                        );
                    }
                    let mut old = self
                        .world
                        .global_beacon_key_sessions
                        .get(&previous.session_id)
                        .cloned()
                        .ok_or("incumbent beacon session is absent")?;
                    let mut next = self
                        .world
                        .global_beacon_key_sessions
                        .get(&credentials.beacon.session_id)
                        .cloned()
                        .ok_or("target beacon session is absent")?;
                    if old.session.transcript_hash != previous.transcript_hash
                        || next.activated_at_height.is_some()
                        || next.retired_at_height.is_some()
                    {
                        return Err(
                            "committee activation beacon credentials have changed".to_owned()
                        );
                    }
                    old.retire(outcome.first_height)
                        .map_err(|error| error.to_string())?;
                    next.activate(outcome.first_height)
                        .map_err(|error| error.to_string())?;
                    beacon_rotation = Some((old, next));
                }
                completed = Some(transition);
            }
            KagemushaMintFinalityEpochDecisionV1::Retain => {
                if self
                    .world
                    .validator_committee_transitions
                    .get(&snapshot.epoch)
                    .is_some()
                {
                    return Err("retention must cancel the exact frozen attempt".to_owned());
                }
            }
            KagemushaMintFinalityEpochDecisionV1::Genesis => {
                return Err("boundary cannot reset scheduling authorization".to_owned());
            }
        }
        let future = snapshot
            .committee_preparation
            .as_ref()
            .map(
                |preparation| -> Result<ValidatorCommitteeTransitionV1, String> {
                    preparation.validate_against_preparing_authorization(outcome)?;
                    let anchor_index = context
                        .height
                        .checked_sub(2)
                        .and_then(|n| usize::try_from(n).ok())
                        .ok_or("future committee has no committed pre-boundary anchor")?;
                    if self.block_hashes().hash_at(anchor_index)
                        != Some(&preparation.selection_anchor)
                        || self
                            .world
                            .validator_committee_transitions
                            .get(&preparation.target_epoch)
                            .is_some()
                    {
                        return Err(
                            "future committee attempts cannot be replaced or reanchored".to_owned()
                        );
                    }
                    let transition = ValidatorCommitteeTransitionV1 {
                        preparation: preparation.clone(),
                        credentials: None,
                        readiness: Vec::new(),
                        outcome: None,
                    };
                    verify_progress(&self.world, &transition)?;
                    Ok(transition)
                },
            )
            .transpose()?;
        let staking = prepare_staking_obligations(&self.world, snapshot)?;
        // All checks precede these writes; the carrier publishes membership, Pasta authorization,
        // the beacon pointer and this same World journal only after exact incumbent finality.
        if let Some((old, next)) = beacon_rotation {
            let next_id = next.session.session_id;
            self.world
                .global_beacon_key_sessions
                .insert(old.session.session_id, old);
            self.world.global_beacon_key_sessions.insert(next_id, next);
            self.world
                .global_beacon_active_session
                .insert(GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, next_id);
        }
        if let Some(transition) = completed {
            self.world
                .validator_committee_transitions
                .insert(transition.preparation.target_epoch, transition);
        }
        if let Some(transition) = future {
            self.world
                .validator_committee_transitions
                .insert(transition.preparation.target_epoch, transition);
        }
        for (key, record) in staking.validators {
            self.world.public_lane_validators.insert(key, record);
        }
        for (key, share) in staking.shares {
            self.world.public_lane_stake_shares.insert(key, share);
        }
        Ok(())
    }
}

fn owns_validator(world: &impl WorldReadOnly, owner: &AccountId, peer: &PeerId) -> bool {
    world.public_lane_validators().iter().any(|(key, record)| {
        public_lane_validator_record_matches_key(key, record)
            && &record.validator == owner
            && &record.peer_id == peer
            && !record.total_stake.is_zero()
    })
}

/// Verify the full candidate proof before accepting or restoring a publication.
pub(crate) fn verify_candidate(candidate: &ValidatorCandidateKeysV1) -> Result<(), String> {
    candidate.validate()?;
    candidate
        .peer_signature
        .verify(
            candidate.keys.validator.public_key(),
            &candidate.authorization(),
        )
        .map_err(|error| error.to_string())?;
    verify_kagemusha_mint_finality_candidate_possession_v1(
        candidate.network_id,
        candidate.generation,
        &candidate.keys,
        &candidate.possession,
    )
    .map_err(|error| error.to_string())
}

/// Reverify prepared keys and every recorded actual-custody proof from public state.
pub(crate) fn verify_progress(
    world: &impl WorldReadOnly,
    transition: &ValidatorCommitteeTransitionV1,
) -> Result<(), String> {
    transition.validate()?;
    iroha_data_model::block::consensus_v2::finality::verify_validator_power_roster_pops(
        &transition.preparation.roster,
        &transition.preparation.validator_set_pops,
    )
    .map_err(|error| error.to_string())?;
    let Some(credentials) = &transition.credentials else {
        return Ok(());
    };
    for keys in &credentials.authority.validators {
        let published = world
            .validator_candidate_keys()
            .get(&ValidatorCandidateKeysV1::key_id(
                credentials.authority.network_id,
                credentials.authority.generation,
                &keys.validator,
            ))
            .ok_or("prepared authority lacks a candidate key publication")?;
        if published.network_id != credentials.authority.network_id || &published.keys != keys {
            return Err("prepared authority substitutes a candidate publication".to_owned());
        }
        verify_candidate(published)?;
    }
    let record = world
        .global_beacon_key_sessions()
        .get(&credentials.beacon.session_id)
        .ok_or("prepared beacon transcript has not been finalized")?;
    record.validate().map_err(|error| error.to_string())?;
    let session = &record.session;
    let preparation = &transition.preparation;
    if session.network_id != preparation.network_id
        || session.session_id != preparation.beacon_session_id()?
        || session.adaptive_dkg.session.attempt_id != preparation.transition_id()?
        || session.adaptive_dkg.session.authority_generation != preparation.authority_generation
        || session.transcript_hash != credentials.beacon.transcript_hash
        || session.adaptive_dkg.session.start_height <= preparation.selection_height
        || session.adaptive_dkg.finalized_at_height >= preparation.first_height - 1
    {
        return Err(
            "beacon transcript is outside its exact preparation attempt or cutoff".to_owned(),
        );
    }
    let peers = preparation
        .roster
        .iter()
        .map(|voter| voter.validator.clone())
        .collect::<Vec<_>>();
    let roster_hash = authenticated_global_threshold_beacon_roster_hash_v1(session, &peers)
        .map_err(|error| error.to_string())?;
    let validated = validate_global_threshold_beacon_session_v1(
        session.clone(),
        &GlobalThresholdBeaconSessionBindingV1 {
            network_id: preparation.network_id,
            session_id: credentials.beacon.session_id,
            roster_hash,
            transcript_hash: credentials.beacon.transcript_hash,
        },
    )
    .map_err(|error| error.to_string())?;
    for readiness in &transition.readiness {
        let context = transition.readiness_context(readiness.validator_index)?;
        verify_kagemusha_mint_finality_seat_readiness_v1(
            &credentials.authority,
            &context,
            &readiness.pasta,
        )
        .map_err(|error| error.to_string())?;
        verify_global_threshold_beacon_seat_readiness_v1(
            &validated,
            &credentials.authority,
            &context,
            &readiness.beacon,
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}

impl StateTransaction<'_, '_> {
    /// Consume a signed preparation command after exact owner, attempt and proof checks.
    pub(crate) fn apply_validator_committee_operation(
        &mut self,
        owner: &AccountId,
        operation: ValidatorCommitteeOperationV1,
    ) -> Result<(), String> {
        let (incumbent, authorization) = current_authority(self)?;
        let next_generation = incumbent
            .generation
            .checked_add(1)
            .ok_or("authority generation overflow")?;
        match operation {
            ValidatorCommitteeOperationV1::PublishCandidate(candidate) => {
                verify_candidate(&candidate)?;
                if candidate.network_id != self.network_id
                    || candidate.generation != next_generation
                    || !owns_validator(&self.world, owner, &candidate.keys.validator)
                {
                    return Err("candidate publication lacks the exact current owner, network or next generation".to_owned());
                }
                let key = ValidatorCandidateKeysV1::key_id(
                    candidate.network_id,
                    candidate.generation,
                    &candidate.keys.validator,
                );
                if self.world.validator_candidate_keys.get(&key).is_some()
                    || self
                        .world
                        .validator_candidate_keys
                        .iter()
                        .any(|(_, published)| {
                            published.keys.eq_proof_public_key == candidate.keys.eq_proof_public_key
                                || published.keys.ep_proof_public_key
                                    == candidate.keys.ep_proof_public_key
                        })
                    || incumbent.validators.iter().any(|keys| {
                        keys.eq_proof_public_key == candidate.keys.eq_proof_public_key
                            || keys.ep_proof_public_key == candidate.keys.ep_proof_public_key
                    })
                {
                    return Err(
                        "candidate key publication replays or duplicates an existing key"
                            .to_owned(),
                    );
                }
                self.world.validator_candidate_keys.insert(key, candidate);
            }
            ValidatorCommitteeOperationV1::PrepareCredentials(command) => {
                let mut transition = self.pending_committee_transition(
                    command.target_epoch,
                    command.transition_id,
                    &authorization,
                )?;
                if transition.credentials.is_some() || !transition.readiness.is_empty() {
                    return Err(
                        "prepared credentials cannot be replaced within a frozen attempt"
                            .to_owned(),
                    );
                }
                if !transition
                    .preparation
                    .roster
                    .iter()
                    .any(|voter| owns_validator(&self.world, owner, &voter.validator))
                {
                    return Err(
                        "preparing credentials requires an exact target validator owner".to_owned(),
                    );
                }
                let beacon = self
                    .world
                    .global_beacon_key_sessions
                    .get(&command.credentials.beacon.session_id)
                    .ok_or("target beacon session is absent")?;
                if beacon.activated_at_height.is_some() || beacon.retired_at_height.is_some() {
                    return Err("target beacon session has already been consumed".to_owned());
                }
                transition.credentials = Some(command.credentials);
                verify_progress(&self.world, &transition)?;
                self.world
                    .validator_committee_transitions
                    .insert(command.target_epoch, transition);
            }
            ValidatorCommitteeOperationV1::AdmitSeat(command) => {
                let mut transition = self.pending_committee_transition(
                    command.target_epoch,
                    command.transition_id,
                    &authorization,
                )?;
                let index = usize::try_from(command.readiness.validator_index)
                    .map_err(|_| "invalid target seat")?;
                let seat = transition
                    .preparation
                    .roster
                    .get(index)
                    .ok_or("invalid target seat")?;
                if !owns_validator(&self.world, owner, &seat.validator) {
                    return Err(
                        "seat readiness requires that exact target validator owner".to_owned()
                    );
                }
                let insertion = transition
                    .readiness
                    .binary_search_by_key(&command.readiness.validator_index, |ready| {
                        ready.validator_index
                    })
                    .err()
                    .ok_or("target seat readiness is already recorded")?;
                transition.readiness.insert(insertion, command.readiness);
                verify_progress(&self.world, &transition)?;
                self.world
                    .validator_committee_transitions
                    .insert(command.target_epoch, transition);
            }
        }
        Ok(())
    }

    fn pending_committee_transition(
        &self,
        target_epoch: u64,
        transition_id: [u8; 32],
        authorization: &KagemushaMintFinalityEpochAuthorizationV1,
    ) -> Result<ValidatorCommitteeTransitionV1, String> {
        let transition = self
            .world
            .validator_committee_transitions
            .get(&target_epoch)
            .ok_or("validator committee attempt has not been frozen")?;
        transition
            .preparation
            .validate_against_preparing_authorization(authorization)?;
        if transition.preparation.transition_id()? != transition_id
            || transition.outcome.is_some()
            || self.block_height() >= authorization.last_height
            || self.block_height() < authorization.first_height
        {
            return Err(
                "committee attempt is terminal, replayed or outside its preparation cutoff"
                    .to_owned(),
            );
        }
        Ok(transition.clone())
    }
}
