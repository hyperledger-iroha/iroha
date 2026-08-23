//! Typed lane-route authority resolution results.

use std::{collections::BTreeSet, num::NonZeroUsize};

use iroha_crypto::Hash;
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    block::consensus::LaneBlockDescriptorV1,
    nexus::{DataSpaceId, LaneId, PublicLaneValidatorStatus},
    peer::PeerId,
};
use iroha_primitives::numeric::Quantity;
use mv::storage::StorageReadOnly;
use thiserror::Error;

use super::{
    ResolvedLaneAuthorityInputs, State, WorldReadOnly, bounded_authority,
    consensus_lane_dataspace_at_height, decode_autoscale_lane_committee,
    lane_claims_autoscale_managed, lane_uses_reserved_autoscale_metadata,
    nexus_active_lane_dataspace_at_height, nexus_lane_active_for_authority,
    nexus_lane_committee_size, nexus_manifest_authority_eligible_lanes_at_height,
    nexus_staking_authority_lane_at_height, peer_has_live_consensus_key,
    public_lane_validator_record_matches_key,
};
use crate::governance::manifest::{GovernanceRules, LaneManifestRegistry};

const TAIRA_RESET11_H4_LANE0_DESCRIPTOR_HASH: [u8; Hash::LENGTH] = [
    0xd4, 0xdf, 0xdd, 0x60, 0x08, 0x4f, 0xa1, 0x90, 0xe7, 0x86, 0xc9, 0x71, 0x73, 0xf8, 0x5d, 0x63,
    0xec, 0xe9, 0xd0, 0xb3, 0x96, 0x7b, 0x36, 0xba, 0x4f, 0x92, 0x69, 0xcf, 0x36, 0x21, 0xc2, 0x3b,
];

const TAIRA_RESET11_H7_LANE2_DESCRIPTOR_HASH: [u8; Hash::LENGTH] = [
    0x43, 0xcd, 0x25, 0xec, 0x05, 0x7a, 0x61, 0x4c, 0x06, 0xbc, 0x45, 0x72, 0xdb, 0xb5, 0x64, 0xbd,
    0xee, 0x13, 0x44, 0x0a, 0xea, 0xe2, 0x4a, 0x9c, 0x8a, 0xcb, 0x23, 0x9a, 0xf6, 0x02, 0x91, 0xf1,
];

fn is_exact_taira_pre_release_projection_gap_identity(
    descriptor_hash: Hash,
    proposal_height: u64,
    lane_id: LaneId,
) -> bool {
    (descriptor_hash == Hash::prehashed(TAIRA_RESET11_H4_LANE0_DESCRIPTOR_HASH)
        && proposal_height == 4
        && lane_id == LaneId::SINGLE)
        || (descriptor_hash == Hash::prehashed(TAIRA_RESET11_H7_LANE2_DESCRIPTOR_HASH)
            && proposal_height == 7
            && lane_id == LaneId::new(2))
}

fn is_exact_taira_pre_release_projection_gap(descriptor: &LaneBlockDescriptorV1) -> bool {
    is_exact_taira_pre_release_projection_gap_identity(
        descriptor.descriptor_hash,
        descriptor.proposal_height,
        descriptor.lane_id,
    ) && descriptor.dataspace_id == DataSpaceId::UNIVERSAL
        && descriptor.previous_lane_block_height == 0
        && descriptor.previous_lane_block_descriptor_hash.is_none()
        && descriptor.lane_block_height == 1
        && descriptor.lane_block_view == 0
}

#[cfg(test)]
mod taira_pre_release_projection_gap_tests {
    use super::*;

    #[test]
    fn accepts_only_the_two_exact_descriptor_identities() {
        let lane0 = Hash::prehashed(TAIRA_RESET11_H4_LANE0_DESCRIPTOR_HASH);
        let lane2 = Hash::prehashed(TAIRA_RESET11_H7_LANE2_DESCRIPTOR_HASH);

        assert!(is_exact_taira_pre_release_projection_gap_identity(
            lane0,
            4,
            LaneId::SINGLE
        ));
        assert!(is_exact_taira_pre_release_projection_gap_identity(
            lane2,
            7,
            LaneId::new(2)
        ));
        assert!(!is_exact_taira_pre_release_projection_gap_identity(
            lane2,
            6,
            LaneId::new(2)
        ));
        assert!(!is_exact_taira_pre_release_projection_gap_identity(
            lane2,
            7,
            LaneId::new(1)
        ));
        assert!(!is_exact_taira_pre_release_projection_gap_identity(
            lane0,
            7,
            LaneId::new(2)
        ));
    }
}

/// Exact lane/dataspace route whose consensus authority is being resolved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LaneAuthorityRoute {
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
}

impl LaneAuthorityRoute {
    /// Construct an exact lane/dataspace authority route.
    #[must_use]
    pub const fn new(lane_id: LaneId, dataspace_id: DataSpaceId) -> Self {
        Self {
            lane_id,
            dataspace_id,
        }
    }

    /// Lane bound to this route.
    #[must_use]
    pub const fn lane_id(self) -> LaneId {
        self.lane_id
    }

    /// Dataspace bound to this route.
    #[must_use]
    pub const fn dataspace_id(self) -> DataSpaceId {
        self.dataspace_id
    }
}

/// Canonical exact `3f+1` authority resolved for one route and height.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaneAuthorityCommittee {
    route: LaneAuthorityRoute,
    authority_height: u64,
    fault_tolerance: u32,
    validators: Vec<PeerId>,
}

impl LaneAuthorityCommittee {
    /// Construct a committee after exact route, geometry, and source validation.
    pub(super) fn new(
        route: LaneAuthorityRoute,
        authority_height: u64,
        fault_tolerance: u32,
        validators: Vec<PeerId>,
    ) -> Self {
        Self {
            route,
            authority_height,
            fault_tolerance,
            validators,
        }
    }

    /// Exact route authorized by this committee.
    #[must_use]
    pub const fn route(&self) -> LaneAuthorityRoute {
        self.route
    }

    /// Consensus height whose route, epoch seed, and live pool were resolved.
    #[must_use]
    pub const fn authority_height(&self) -> u64 {
        self.authority_height
    }

    /// Dataspace fault tolerance used to derive the exact committee size.
    #[must_use]
    pub const fn fault_tolerance(&self) -> u32 {
        self.fault_tolerance
    }

    /// Canonically ordered exact `3f+1` validators.
    #[must_use]
    pub fn validators(&self) -> &[PeerId] {
        &self.validators
    }

    /// Consume this authority and return its canonical validators.
    #[must_use]
    pub fn into_validators(self) -> Vec<PeerId> {
        self.validators
    }
}

/// Fail-closed error while resolving canonical lane authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum LaneAuthorityError {
    /// The requested dataspace is absent from the committed catalog.
    #[error("lane authority dataspace {dataspace_id} is not configured")]
    UnknownDataspace {
        /// Missing dataspace identifier.
        dataspace_id: DataSpaceId,
    },
    /// The lane is not active on the requested dataspace at this height.
    #[error(
        "lane {lane_id} is not active on dataspace {dataspace_id} at authority height {authority_height}"
    )]
    InactiveRoute {
        /// Requested lane.
        lane_id: LaneId,
        /// Requested dataspace.
        dataspace_id: DataSpaceId,
        /// Requested consensus height.
        authority_height: u64,
    },
    /// The dataspace's configured `f` cannot form a protocol-bounded `3f+1` committee.
    #[error(
        "dataspace {dataspace_id} fault tolerance {fault_tolerance} does not define a valid bounded 3f+1 lane committee"
    )]
    InvalidGeometry {
        /// Dataspace whose geometry is invalid.
        dataspace_id: DataSpaceId,
        /// Configured Byzantine fault tolerance.
        fault_tolerance: u32,
    },
    /// Manifest/staking authority inputs are inconsistent with the exact route.
    #[error(
        "lane {lane_id} dataspace {dataspace_id} has inconsistent manifest or staking authority at height {authority_height}"
    )]
    InvalidAuthoritySource {
        /// Requested lane.
        lane_id: LaneId,
        /// Requested dataspace.
        dataspace_id: DataSpaceId,
        /// Requested consensus height.
        authority_height: u64,
    },
    /// The live canonical pool cannot fill the exact `3f+1` committee.
    #[error(
        "lane {lane_id} dataspace {dataspace_id} requires {required} validators at height {authority_height}, but its canonical pool has {actual}"
    )]
    UndersizedPool {
        /// Requested lane.
        lane_id: LaneId,
        /// Requested dataspace.
        dataspace_id: DataSpaceId,
        /// Requested consensus height.
        authority_height: u64,
        /// Exact `3f+1` size.
        required: usize,
        /// Unique validators available from the canonical source.
        actual: usize,
    },
    /// An immutable autoscale pin does not have the dataspace's exact `3f+1` shape.
    #[error(
        "autoscale lane {lane_id} dataspace {dataspace_id} pins {actual} validators, expected exactly {required}"
    )]
    InvalidAutoscalePin {
        /// Autoscale lane.
        lane_id: LaneId,
        /// Bound dataspace.
        dataspace_id: DataSpaceId,
        /// Exact `3f+1` size.
        required: usize,
        /// Pinned validator count.
        actual: usize,
    },
    /// Canonical peer encoding failed while deriving the seeded committee order.
    #[error("failed to encode a lane authority peer canonically")]
    CanonicalPeerEncoding,
}

/// Snapshot the bounded authority inputs for one active lane and height.
pub(super) fn inputs_from_nexus(
    lane_id: LaneId,
    validator_mode: iroha_config::parameters::actual::LaneValidatorMode,
    nexus: &iroha_config::parameters::actual::Nexus,
    block_height: u64,
) -> Option<ResolvedLaneAuthorityInputs> {
    let dataspace_id = State::nexus_authoritative_lane_dataspace(lane_id, nexus)?;
    nexus_lane_active_for_authority(lane_id, dataspace_id, nexus, block_height).then_some(())?;
    let lane = nexus
        .lane_catalog
        .lanes()
        .iter()
        .find(|lane| lane.id == lane_id)?;
    // Snapshot only the protocol-bounded authority material. Cloning the
    // full lane would duplicate arbitrary dashboard metadata on every
    // routed query even though authority resolution never reads it.
    let autoscale_validator_set = lane_claims_autoscale_managed(lane).then(|| {
        decode_autoscale_lane_committee(lane)
            .ok()
            .flatten()
            .map_or_else(Vec::new, |committee| committee.validator_set)
    });
    let manifest_authority_lanes = nexus_manifest_authority_eligible_lanes_at_height(
        lane_id,
        dataspace_id,
        nexus,
        block_height,
    );
    let staking_authority_lanes = if lane_uses_reserved_autoscale_metadata(lane) {
        matches!(
            validator_mode,
            iroha_config::parameters::actual::LaneValidatorMode::StakeElected
        )
        .then_some(BTreeSet::from([lane_id]))
        .unwrap_or_default()
    } else {
        nexus
            .lane_catalog
            .lanes()
            .iter()
            .filter(|candidate| !lane_uses_reserved_autoscale_metadata(candidate))
            .filter(|candidate| {
                nexus_active_lane_dataspace_at_height(candidate.id, nexus, block_height)
                    == Some(dataspace_id)
            })
            .filter(|candidate| {
                matches!(
                    nexus
                        .staking
                        .validator_mode(candidate.id, &nexus.lane_catalog),
                    iroha_config::parameters::actual::LaneValidatorMode::StakeElected
                )
            })
            .map(|candidate| candidate.id)
            .collect()
    };
    let staking_owner_lane = if lane_uses_reserved_autoscale_metadata(lane) {
        Some(lane_id)
    } else {
        nexus_staking_authority_lane_at_height(lane_id, nexus, block_height)
    };
    Some(ResolvedLaneAuthorityInputs {
        bounded: bounded_authority::LaneAuthorityInputs {
            dataspace_id,
            autoscale_validator_set,
            validator_mode,
            minimum_stake: nexus.staking.min_validator_stake.clone(),
            max_validators: nexus.staking.max_validators.get(),
        },
        manifest_authority_lanes,
        staking_authority_lanes,
        staking_owner_lane,
    })
}

/// Resolve the unique manifest authority source for one exact lane route.
pub(super) fn manifest_rules_for_lane<'a>(
    lane_id: LaneId,
    manifest_registry: &'a LaneManifestRegistry,
    inputs: &ResolvedLaneAuthorityInputs,
) -> Result<Option<&'a GovernanceRules>, ()> {
    manifest_registry
        .dataspace_authority_rules_for_lanes(
            lane_id,
            inputs.bounded.dataspace_id,
            &inputs.manifest_authority_lanes,
        )
        .map_err(|_| ())
}

/// Resolve one physical dataspace's live stake projection.
///
/// Lane-keyed stake rows are canonical storage projections of dataspace-wide
/// authority. Empty sibling projections are ignored, while more than one
/// non-empty projection fails closed instead of retaining duplicate state
/// that a later lane-local mutation can split. Autoscale rows stay exact-lane;
/// their immutable peer committee is resolved before this helper.
fn live_dataspace_stake_candidates(
    world: &impl WorldReadOnly,
    inputs: &ResolvedLaneAuthorityInputs,
    block_height: u64,
) -> Option<Vec<(AccountId, PeerId, Quantity)>> {
    live_stake_candidates_for_lanes(
        world,
        &inputs.staking_authority_lanes,
        inputs.staking_owner_lane,
        &inputs.bounded.minimum_stake,
        inputs.bounded.max_validators,
        block_height,
    )
}

/// Resolve one canonical live stake projection across dataspace sibling lanes.
pub(super) fn live_stake_candidates_for_lanes(
    world: &impl WorldReadOnly,
    source_lanes: &BTreeSet<LaneId>,
    canonical_owner: Option<LaneId>,
    minimum_stake: &Quantity,
    configured_limit: u32,
    block_height: u64,
) -> Option<Vec<(AccountId, PeerId, Quantity)>> {
    let limit = bounded_authority::staking_validator_limit_from(configured_limit)?;
    let mut selected = Vec::with_capacity(limit);
    let mut selected_projection = None;
    for (key, record) in world.public_lane_validators().iter() {
        if !source_lanes.contains(&key.0)
            || !public_lane_validator_record_matches_key(key, record)
            || !matches!(record.status, PublicLaneValidatorStatus::Active)
            || !crate::smartcontracts::isi::staking::meets_min_stake(
                &record.self_stake,
                minimum_stake,
            )
            .unwrap_or(false)
            || !world.peers().iter().any(|peer| peer == &record.peer_id)
            || !peer_has_live_consensus_key(world, &record.peer_id, block_height)
        {
            continue;
        }
        if selected_projection.is_some_and(|lane| lane != key.0) {
            return None;
        }
        selected_projection = Some(key.0);
        bounded_authority::insert_unique_by(
            &mut selected,
            record,
            limit,
            |lhs, rhs| lhs.validator == rhs.validator,
            |lhs, rhs| {
                rhs.total_stake
                    .cmp(&lhs.total_stake)
                    .then_with(|| lhs.validator.cmp(&rhs.validator))
                    .then_with(|| lhs.peer_id.cmp(&rhs.peer_id))
            },
        );
    }
    if selected_projection.is_some() && selected_projection != canonical_owner {
        return None;
    }
    Some(
        selected
            .into_iter()
            .map(|record| {
                (
                    record.validator.clone(),
                    record.peer_id.clone(),
                    record.total_stake.clone(),
                )
            })
            .collect(),
    )
}

/// Project stake-ranked candidates into a unique peer pool without reordering them.
pub(super) fn ranked_stake_peer_pool(
    candidates: Vec<(AccountId, PeerId, Quantity)>,
) -> Vec<PeerId> {
    let mut peers = Vec::with_capacity(candidates.len());
    for (_, peer_id, _) in candidates {
        if !peers.contains(&peer_id) {
            peers.push(peer_id);
        }
    }
    peers
}

/// Resolve validator accounts for test-only lane authority diagnostics.
#[cfg(test)]
pub(super) fn validator_accounts_with_inputs(
    world: &impl WorldReadOnly,
    lane_id: LaneId,
    manifest_registry: &LaneManifestRegistry,
    inputs: &ResolvedLaneAuthorityInputs,
    block_height: u64,
) -> Vec<AccountId> {
    let rules = match manifest_rules_for_lane(lane_id, manifest_registry, inputs) {
        Ok(rules) => rules,
        Err(()) => return Vec::new(),
    };
    if let Some(rules) = rules {
        if !rules.validator_bindings.is_empty() {
            let bindings = bounded_authority::live_manifest_validator_bindings(
                world,
                &rules.validator_bindings,
                &rules.validators,
                block_height,
            );
            return bindings
                .into_iter()
                .map(|binding| binding.validator)
                .collect();
        }
        return bounded_authority::live_manifest_validator_account_peers(
            world,
            &rules.validators,
            block_height,
        )
        .into_iter()
        .map(|(validator, _)| validator)
        .collect();
    }
    if inputs.staking_authority_lanes.len() == 1
        && inputs.staking_authority_lanes.contains(&lane_id)
        && inputs.staking_owner_lane == Some(lane_id)
        && matches!(
            inputs.bounded.validator_mode,
            iroha_config::parameters::actual::LaneValidatorMode::StakeElected
        )
    {
        return bounded_authority::stake_elected_validator_accounts(
            world,
            lane_id,
            &inputs.bounded.minimum_stake,
            inputs.bounded.max_validators,
            block_height,
        );
    }
    live_dataspace_stake_candidates(world, inputs, block_height)
        .unwrap_or_default()
        .into_iter()
        .map(|(account, _, _)| account)
        .collect()
}

/// Resolve the canonical live manifest/stake pool or immutable autoscale pin.
pub(super) fn peer_pool_with_inputs(
    world: &impl WorldReadOnly,
    lane_id: LaneId,
    manifest_registry: &LaneManifestRegistry,
    inputs: &ResolvedLaneAuthorityInputs,
    block_height: u64,
) -> Result<Vec<PeerId>, ()> {
    if let Some(validator_set) = &inputs.bounded.autoscale_validator_set {
        // An autoscale lane has one immutable committee for its full
        // incarnation. Never apply current-world peer/key/manifest or
        // topology filters here: delayed QCs from this pinned authority
        // must remain verifiable after roster churn.
        return Ok(validator_set.clone());
    }
    let rules = manifest_rules_for_lane(lane_id, manifest_registry, inputs)?;
    if let Some(rules) = rules {
        if !rules.validator_bindings.is_empty() {
            let bindings = bounded_authority::live_manifest_validator_bindings(
                world,
                &rules.validator_bindings,
                &rules.validators,
                block_height,
            );
            return Ok(bindings
                .into_iter()
                .map(|binding| binding.peer_id)
                .collect());
        }
        return Ok(bounded_authority::live_manifest_validator_account_peers(
            world,
            &rules.validators,
            block_height,
        )
        .into_iter()
        .map(|(_, peer)| peer)
        .collect());
    }
    if inputs.staking_authority_lanes.len() == 1
        && inputs.staking_authority_lanes.contains(&lane_id)
        && inputs.staking_owner_lane == Some(lane_id)
        && matches!(
            inputs.bounded.validator_mode,
            iroha_config::parameters::actual::LaneValidatorMode::StakeElected
        )
    {
        return Ok(bounded_authority::stake_elected_peer_ids(
            world,
            lane_id,
            &inputs.bounded.minimum_stake,
            inputs.bounded.max_validators,
            block_height,
        ));
    }
    let Some(candidates) = live_dataspace_stake_candidates(world, inputs, block_height) else {
        return Err(());
    };
    Ok(ranked_stake_peer_pool(candidates))
}

/// Resolve an exact deterministic `3f+1` route committee from immutable state inputs.
pub(super) fn resolve_from_sources(
    world: &impl WorldReadOnly,
    network_id: &NetworkId,
    route: LaneAuthorityRoute,
    manifest_registry: &LaneManifestRegistry,
    nexus: &iroha_config::parameters::actual::Nexus,
    authority_height: u64,
) -> Result<LaneAuthorityCommittee, LaneAuthorityError> {
    let dataspace = nexus.dataspace_catalog.by_id(route.dataspace_id()).ok_or(
        LaneAuthorityError::UnknownDataspace {
            dataspace_id: route.dataspace_id(),
        },
    )?;
    if consensus_lane_dataspace_at_height(route.lane_id(), nexus, authority_height)
        != Some(route.dataspace_id())
    {
        return Err(LaneAuthorityError::InactiveRoute {
            lane_id: route.lane_id(),
            dataspace_id: route.dataspace_id(),
            authority_height,
        });
    }
    let fault_tolerance = dataspace.fault_tolerance;
    let required = nexus_lane_committee_size(nexus, route.dataspace_id()).ok_or(
        LaneAuthorityError::InvalidGeometry {
            dataspace_id: route.dataspace_id(),
            fault_tolerance,
        },
    )?;
    let validator_mode = nexus
        .staking
        .validator_mode(route.lane_id(), &nexus.lane_catalog);
    let inputs = inputs_from_nexus(route.lane_id(), validator_mode, nexus, authority_height)
        .filter(|inputs| inputs.bounded.dataspace_id == route.dataspace_id())
        .ok_or(LaneAuthorityError::InvalidAuthoritySource {
            lane_id: route.lane_id(),
            dataspace_id: route.dataspace_id(),
            authority_height,
        })?;
    let is_autoscale_pin = inputs.bounded.autoscale_validator_set.is_some();
    let pool = peer_pool_with_inputs(
        world,
        route.lane_id(),
        manifest_registry,
        &inputs,
        authority_height,
    )
    .map_err(|()| LaneAuthorityError::InvalidAuthoritySource {
        lane_id: route.lane_id(),
        dataspace_id: route.dataspace_id(),
        authority_height,
    })?;
    let unique_count = pool.iter().collect::<BTreeSet<_>>().len();
    if unique_count != pool.len() {
        return Err(LaneAuthorityError::InvalidAuthoritySource {
            lane_id: route.lane_id(),
            dataspace_id: route.dataspace_id(),
            authority_height,
        });
    }
    if is_autoscale_pin {
        if pool.len() != required || pool.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(LaneAuthorityError::InvalidAutoscalePin {
                lane_id: route.lane_id(),
                dataspace_id: route.dataspace_id(),
                required,
                actual: pool.len(),
            });
        }
        return Ok(LaneAuthorityCommittee::new(
            route,
            authority_height,
            fault_tolerance,
            pool,
        ));
    }
    if pool.len() < required {
        return Err(LaneAuthorityError::UndersizedPool {
            lane_id: route.lane_id(),
            dataspace_id: route.dataspace_id(),
            authority_height,
            required,
            actual: pool.len(),
        });
    }
    let seed = State::lane_relay_committee_seed_from_sources(
        world,
        network_id,
        route.dataspace_id(),
        route.lane_id(),
        authority_height,
    );
    let mut validators = State::lane_relay_committee_from_pool(&pool, required, seed)
        .map_err(|_| LaneAuthorityError::CanonicalPeerEncoding)?;
    if validators.len() != required {
        return Err(LaneAuthorityError::UndersizedPool {
            lane_id: route.lane_id(),
            dataspace_id: route.dataspace_id(),
            authority_height,
            required,
            actual: validators.len(),
        });
    }
    validators.sort();
    Ok(LaneAuthorityCommittee::new(
        route,
        authority_height,
        fault_tolerance,
        validators,
    ))
}

/// Recover a lane descriptor's immutable committee from the globally
/// finalized block that authenticated its proposal-height ownership.
///
/// This is deliberately separate from live authority resolution: current
/// manifest and stake records cannot reconstruct a historical committee
/// after churn.
pub(super) fn authenticated_committee_for_descriptor(
    state: &State,
    descriptor: &LaneBlockDescriptorV1,
) -> Result<Vec<PeerId>, &'static str> {
    if descriptor.computed_descriptor_hash() != descriptor.descriptor_hash {
        return Err("lane descriptor hash is not canonical");
    }
    let height = usize::try_from(descriptor.proposal_height)
        .ok()
        .and_then(NonZeroUsize::new)
        .ok_or("lane proposal height does not fit committed block storage")?;
    let block = state
        .block_by_height(height)
        .ok_or("lane proposal-height block is absent or unauthenticated")?;
    let (finalized_header, finality) = state
        .kura
        .v2_finality_artifact_with_header(descriptor.proposal_height)
        .map_err(|_| "lane proposal-height finality cannot be verified")?
        .ok_or("lane proposal-height V2 finality is absent")?;
    if finalized_header != block.header()
        || finality.block_hash != block.hash()
        || finality.height_context.height != descriptor.proposal_height
        || finality.height_context.network_id != state.network_id
    {
        return Err("lane proposal-height block and V2 finality disagree");
    }
    let ownerships = block
        .execution_context()
        .ok_or("lane proposal-height block has no execution context")?
        .lane_payload_ownerships
        .iter()
        .filter(|ownership| {
            ownership.proposal_height == descriptor.proposal_height
                && ownership.lane_id == descriptor.lane_id
                && ownership.dataspace_id == descriptor.dataspace_id
                && ownership.lane_incarnation == descriptor.lane_incarnation
                && ownership.lane_block_height == descriptor.lane_block_height
                && ownership.lane_block_view == descriptor.lane_block_view
                && ownership.subject_hash == descriptor.subject_hash
                && ownership.payload_ownership_hash == descriptor.payload_ownership_hash
                && ownership.rbc_instance_hash == descriptor.rbc_instance_hash
                && ownership.accepted_candidate_indices == descriptor.accepted_candidate_indices
                && ownership.accepted_transaction_hashes == descriptor.accepted_transaction_hashes
                && ownership.previous_lane_block_height == descriptor.previous_lane_block_height
                && ownership.previous_lane_block_descriptor_hash
                    == descriptor.previous_lane_block_descriptor_hash
                && ownership.lane_block_descriptor_hash == Some(descriptor.descriptor_hash)
                && ownership.lane_block_descriptor_validator_set == descriptor.validator_set
                && ownership.lane_block_descriptor_validator_count == descriptor.validator_count
                && ownership.lane_block_descriptor_min_quorum == descriptor.min_quorum
                && ownership.qc_mode_tag == descriptor.qc_mode_tag
                && ownership.validate_replay_material().is_ok()
        });
    let mut ownerships = ownerships.take(2);
    let Some(ownership) = ownerships.next() else {
        // Reset-11 was produced before global blocks persisted the complete
        // lane-payload ownership projection. Its two certified lane blocks
        // were nevertheless authorized with the producer's canonical
        // proposal-height committees. Retain that exact, narrow recovery
        // corridor only for the two authenticated descriptors emitted by the
        // reset producer. The live State height may already expose a staged
        // successor while this source is selected, so it is not a stable
        // discriminator here; the immutable descriptor and proposal-height
        // finality are.
        if crate::sumeragi::v2_context::uses_pre_release_taira_nexus_projection(&state.network_id)
            && is_exact_taira_pre_release_projection_gap(descriptor)
        {
            let committee = finality
                .height_context
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect::<Vec<_>>();
            if !committee.is_empty()
                && finality
                    .height_context
                    .roster
                    .iter()
                    .all(|entry| entry.power == 1)
                && descriptor.validator_set == committee
                && descriptor.validator_set_hash == descriptor.computed_validator_set_hash()
                && usize::try_from(descriptor.validator_count).ok() == Some(committee.len())
                && finality.height_context.quorum.min_signers == descriptor.min_quorum
                && finality.height_context.quorum.total_power
                    == u64::from(descriptor.validator_count)
            {
                return Ok(committee);
            }
            return Err("reset-11 lane descriptor differs from its canonical committee");
        }
        return Err("finalized proposal-height block does not bind this lane descriptor");
    };
    if ownerships.next().is_some() {
        return Err("finalized proposal-height block ambiguously binds this lane descriptor");
    }
    Ok(ownership.lane_block_descriptor_validator_set.clone())
}
