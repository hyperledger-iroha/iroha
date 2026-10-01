//! Exact tenure retention across lane retirement and peer reuse.
use super::*;
use crate::state::{World, WorldBlock};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    account::AccountId,
    asset::{AssetDefinitionId, AssetId},
    nexus::PublicLaneValidatorStatus,
};
use iroha_model_base::{metadata::Metadata, peer::PeerId};

fn record() -> PublicLaneValidatorRecord {
    let key = KeyPair::from_seed(vec![11; 32], Algorithm::Ed25519);
    let account = AccountId::new(key.public_key().clone());
    PublicLaneValidatorRecord {
        lane_id: LaneId::SINGLE,
        validator: account.clone(),
        peer_id: PeerId::new(key.public_key().clone()),
        stake_account: account,
        total_stake: 100_u32.into(),
        self_stake: 100_u32.into(),
        metadata: Metadata::default(),
        status: PublicLaneValidatorStatus::Active,
        activation_height: 1,
        election_exit_height: None,
        deactivation_height: None,
        last_reward_epoch: None,
    }
}
fn obligation(record: &PublicLaneValidatorRecord, incarnation: u8) -> SumeragiLaneCustody {
    let definition = AssetDefinitionId::derive_from_components(
        iroha_model_base::domain::DomainId::try_new("custody", "test").unwrap(),
        "xor".parse().unwrap(),
    );
    let asset = AssetId::new(definition, record.validator.clone());
    let signers = vec![SumeragiLaneSignerCustody {
        signer: 0,
        binding: SumeragiLaneStakeBinding::from_record(record, &asset).unwrap(),
    }]
    .try_into()
    .unwrap();
    SumeragiLaneCustody {
        lane: LaneId::new(1),
        incarnation: [incarnation; 32],
        instance: [incarnation + 1; 32],
        created_at: 10,
        merged: iroha_data_model::sumeragi_lanes::SumeragiLaneFrontier::default(),
        signer_count: 1,
        signers,
        evidence_horizon: 7,
        slashing_delay: 3,
        retired_at: None,
    }
}
#[test]
fn lane_obligation_never_moves_to_a_later_registration_sharing_key_and_account() {
    let original_world = World::default();
    let mut world = original_world.block();
    let mut original = record();
    world
        .sumeragi_lanes
        .get_mut()
        .custody
        .push(obligation(&original, 1));
    assert!(retains_registration(&world, &original, u64::MAX));
    original.activation_height = 2;
    assert!(!retains_registration(&world, &original, 12));
}
#[test]
fn every_original_incarnation_must_finish_its_delay_before_custody_releases() {
    let original_world = World::default();
    let mut world = original_world.block();
    let original = record();
    let mut first = obligation(&original, 1);
    first.retired_at = Some(20);
    world.sumeragi_lanes.get_mut().custody = vec![first, obligation(&original, 2)];
    assert!(retains_registration(&world, &original, 30));
    world.sumeragi_lanes.get_mut().custody[1].retired_at = Some(25);
    assert!(retains_registration(&world, &original, 34));
    assert!(!retains_registration(&world, &original, 35));
}
#[test]
fn malformed_original_deadline_cannot_release_retained_custody() {
    let original_world = World::default();
    let mut world = original_world.block();
    let original = record();
    let mut invalid = obligation(&original, 1);
    invalid.retired_at = Some(u64::MAX);
    world.sumeragi_lanes.get_mut().custody.push(invalid);
    assert!(retains_registration(&world, &original, u64::MAX));
}

fn parameters(world: &mut WorldBlock<'_>, horizon: u64, delay: u64) {
    use iroha_data_model::parameter::{Parameter, system::SumeragiNposParameters};
    world.parameters.get_mut().set_parameter(Parameter::Custom(
        SumeragiNposParameters {
            evidence_horizon_blocks: horizon,
            slashing_delay_blocks: delay,
            ..SumeragiNposParameters::default()
        }
        .into_custom_parameter(),
    ));
}

fn lane(original: &PublicLaneValidatorRecord) -> SumeragiLaneRecord {
    use iroha_data_model::{
        parameter::system::SumeragiParameters,
        sumeragi_lanes::{SumeragiLaneFrontier, SumeragiLaneMember},
    };
    SumeragiLaneRecord {
        lane: LaneId::new(1),
        dataspace: iroha_model_base::topology::DataSpaceId::new(0),
        incarnation: [1; 32],
        params: SumeragiParameters::default(),
        committee: vec![SumeragiLaneMember {
            peer: original.peer_id.clone(),
            pop: vec![],
        }],
        created_at: 10,
        active_from: 12,
        closing: None,
        anchor_freshness: 4,
        merged: SumeragiLaneFrontier::default(),
        merged_at: 12,
        rescued: 0,
        da_layout: iroha_sumeragi::availability::recommended_data_availability_layout(),
    }
}

fn fund(world: &mut WorldBlock<'_>, original: &PublicLaneValidatorRecord) {
    let asset = AssetId::new(
        AssetDefinitionId::derive_from_components(
            iroha_model_base::domain::DomainId::try_new("custody", "test").unwrap(),
            "xor".parse().unwrap(),
        ),
        original.validator.clone(),
    );
    let key = (original.lane_id, original.validator.clone());
    world
        .public_lane_validators
        .insert(key.clone(), original.clone());
    world
        .public_lane_stake_custody
        .insert(key, (asset, 100_u32.into()));
}

#[test]
fn original_signer_binding_requires_positive_custody_and_survives_policy_member_order() {
    let original_world = World::default();
    let mut world = original_world.block();
    let original = record();
    let mut lane = lane(&original);
    let other = KeyPair::from_seed(vec![9; 32], Algorithm::Ed25519);
    lane.committee
        .push(iroha_data_model::sumeragi_lanes::SumeragiLaneMember {
            peer: PeerId::new(other.public_key().clone()),
            pop: vec![],
        });
    lane.committee.sort_by(|a, b| b.peer.cmp(&a.peer));
    assert!(
        pin_signers(&world, &Nexus::default(), &lane, true)
            .unwrap()
            .as_slice()
            .is_empty()
    );
    fund(&mut world, &original);
    let bindings = pin_signers(&world, &Nexus::default(), &lane, true).unwrap();
    assert_eq!(bindings.as_slice().len(), 1);
    let expected = u32::from(
        lane.committee
            .iter()
            .any(|member| member.peer < original.peer_id),
    );
    assert_eq!(bindings.as_slice()[0].signer, expected);
    // The physical lane has no independently selected mutable staking owner: fixed
    // committee reuse of a global key alone must not manufacture financial authority.
    assert!(
        pin_signers(&world, &Nexus::default(), &lane, false)
            .unwrap()
            .as_slice()
            .is_empty()
    );
    world
        .public_lane_stake_custody
        .get_mut(&(original.lane_id, original.validator.clone()))
        .unwrap()
        .1 = 0_u32.into();
    assert!(
        pin_signers(&world, &Nexus::default(), &lane, true)
            .unwrap()
            .as_slice()
            .is_empty()
    );
}

#[test]
fn retirement_marks_exact_boundary_and_policy_extension_precedes_withdrawal() {
    let original_world = World::default();
    let mut world = original_world.block();
    let original = record();
    parameters(&mut world, 7, 3);
    let mut state = SumeragiLaneState::default();
    let mut lane = lane(&original);
    lane.closing = Some(15); // 15 + 4 + 1 = 20.
    state.lanes.push(lane);
    state.custody.push(obligation(&original, 1));
    prepare_retirement(&mut state, &world, 19).unwrap();
    assert_eq!(state.custody[0].retired_at, None);
    prepare_retirement(&mut state, &world, 20).unwrap();
    assert_eq!(state.custody[0].retired_at, Some(20));
    world.sumeragi_lanes.get_mut().custody = state.custody.clone();
    assert!(!retains_registration(&world, &original, 30));
    parameters(&mut world, 8, 4);
    assert!(
        retains_registration(&world, &original, 30),
        "same-block extension must already retain custody"
    );
    state.lanes.clear();
    prepare_retirement(&mut state, &world, 30).unwrap();
    assert_eq!(state.custody[0].release_height().unwrap(), Some(32));
    parameters(&mut world, 1, 1);
    prepare_retirement(&mut state, &world, 31).unwrap();
    assert_eq!(
        state.custody[0].release_height().unwrap(),
        Some(32),
        "policy cannot shorten existing liability"
    );
    prepare_retirement(&mut state, &world, 32).unwrap();
    assert!(state.custody.is_empty());
}

#[test]
fn creation_pins_once_and_capacity_is_reclaimed_only_after_retirement_delay() {
    use iroha_crypto::{Hash, HashOf};
    let original_world = World::default();
    let mut world = original_world.block();
    parameters(&mut world, 7, 3);
    let original = record();
    let network = iroha_data_model::NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
        Hash::prehashed([3; 32]),
    ));
    let mut state = SumeragiLaneState {
        lanes: vec![lane(&original)],
        ..SumeragiLaneState::default()
    };
    pin_created(
        &mut state,
        &world,
        &Nexus::default(),
        &network,
        "custody-test",
        None,
        10,
    )
    .unwrap();
    assert_eq!(state.custody.len(), 1);
    assert!(state.custody[0].signers.as_slice().is_empty());
    assert_eq!(
        creation_capacity(&state, &world),
        MAX_LANE_CUSTODY_OBLIGATIONS - 1
    );
    assert!(
        pin_created(
            &mut state,
            &world,
            &Nexus::default(),
            &network,
            "custody-test",
            None,
            10
        )
        .is_err()
    );
    fund(&mut world, &original);
    pin_created(
        &mut state,
        &world,
        &Nexus::default(),
        &network,
        "custody-test",
        None,
        11,
    )
    .unwrap();
    assert!(
        state.custody[0].signers.as_slice().is_empty(),
        "later funds cannot rewrite the creation-time forensic decision"
    );
    state.custody[0].retired_at = Some(20);
    state.lanes.clear();
    prepare_retirement(&mut state, &world, 29).unwrap();
    assert_eq!(
        creation_capacity(&state, &world),
        MAX_LANE_CUSTODY_OBLIGATIONS - 1
    );
    prepare_retirement(&mut state, &world, 30).unwrap();
    assert_eq!(
        creation_capacity(&state, &world),
        MAX_LANE_CUSTODY_OBLIGATIONS
    );
}

#[test]
fn pending_original_evidence_delays_reclamation_without_native_height_arithmetic() {
    use iroha_crypto::Hash;
    use iroha_data_model::block::consensus::{
        Evidence, EvidenceAttribution, EvidencePenaltyStatus, EvidenceRecord,
    };
    let original_world = World::default();
    let mut world = original_world.block();
    parameters(&mut world, 7, 3);
    let original = record();
    let mut row = obligation(&original, 1);
    row.retired_at = Some(20);
    let mut state = SumeragiLaneState {
        custody: vec![row.clone()],
        ..SumeragiLaneState::default()
    };
    // Predicate-only retained-table fixture. Empty bytes grant no evidence authority and
    // are never submitted to admission; this test exercises lifetime dispatch only.
    let key = Hash::new(b"original pending custody fixture");
    world.consensus_evidence.insert(
        key,
        EvidenceRecord {
            evidence: Evidence { native: vec![] },
            attribution: EvidenceAttribution {
                scope: iroha_data_model::block::consensus::EvidenceScope::Root,
                instance: row.instance,
                height: u64::MAX,
                epoch: 0,
                context_id: [3; 32],
                authority_generation: [3; 32],
                offenders: vec![],
                safety_violation: false,
            },
            recorded_at_height: 27,
            recorded_at_view: 0,
            recorded_at_ms: 0,
            penalty_status: EvidencePenaltyStatus::Pending,
        },
    );
    world.sumeragi_lanes.get_mut().custody = state.custody.clone();
    assert!(retains_registration(&world, &original, 30));
    prepare_retirement(&mut state, &world, 30).unwrap();
    assert_eq!(state.custody.len(), 1);
    world
        .consensus_evidence
        .get_mut(&key)
        .unwrap()
        .penalty_status = EvidencePenaltyStatus::Applied { height: 30 };
    prepare_retirement(&mut state, &world, 30).unwrap();
    assert_eq!(
        state.custody.len(),
        1,
        "terminal report retains reauthentication provenance"
    );
    assert!(
        !retains_registration(&world, &original, 30),
        "completed penalty permits release"
    );
    world.consensus_evidence.remove(key);
    prepare_retirement(&mut state, &world, 31).unwrap();
    assert!(state.custody.is_empty());
}

#[test]
fn retirement_keeps_the_final_same_carrier_merge_after_the_live_record_is_removed() {
    use iroha_data_model::sumeragi_lanes::SumeragiLaneFrontier;
    let original_world = World::default();
    let mut world = original_world.block();
    parameters(&mut world, 7, 3);
    let original = record();
    let mut live = lane(&original);
    live.closing = Some(15); // A=4: exact retirement at global height 20.
    let final_merge = SumeragiLaneFrontier {
        height: 900,
        block_hash: [0xA1; 32],
        result: [0xB1; 32],
    };
    live.merged = final_merge;
    live.merged_at = 20;
    let mut state = SumeragiLaneState {
        lanes: vec![live],
        custody: vec![obligation(&original, 1)],
        ..SumeragiLaneState::default()
    };
    prepare_retirement(&mut state, &world, 20).unwrap();
    assert_eq!(state.custody[0].retired_at, Some(20));
    assert_eq!(state.custody[0].merged, final_merge);
    state.lanes.clear();
    prepare_retirement(&mut state, &world, 27).unwrap();
    assert_eq!(state.custody[0].merged, final_merge);
    assert!(state.custody[0].admits_at(27).unwrap());
    assert!(state.custody[0].covers_native_subject(901).unwrap());
    assert!(!state.custody[0].covers_native_subject(902).unwrap());
}

#[test]
fn an_existing_frontier_cannot_regress_or_switch_hash_or_result_at_the_same_native_height() {
    use iroha_data_model::sumeragi_lanes::SumeragiLaneFrontier;
    let original_world = World::default();
    let mut world = original_world.block();
    parameters(&mut world, 7, 3);
    let original = record();
    let frontier = SumeragiLaneFrontier {
        height: 9,
        block_hash: [1; 32],
        result: [2; 32],
    };
    let mut row = obligation(&original, 1);
    row.merged = frontier;
    for replacement in [
        SumeragiLaneFrontier {
            height: 8,
            ..frontier
        },
        SumeragiLaneFrontier {
            block_hash: [3; 32],
            ..frontier
        },
        SumeragiLaneFrontier {
            result: [3; 32],
            ..frontier
        },
    ] {
        let mut live = lane(&original);
        live.merged = replacement;
        let mut state = SumeragiLaneState {
            lanes: vec![live],
            custody: vec![row.clone()],
            ..SumeragiLaneState::default()
        };
        assert_eq!(
            prepare_retirement(&mut state, &world, 20),
            Err(CustodyViolation::Frontier)
        );
        assert_eq!(state.custody[0].merged, frontier);
    }
}
