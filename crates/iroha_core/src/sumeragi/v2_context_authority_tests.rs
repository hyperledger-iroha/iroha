//! Explicit genesis/retention authorization and authenticated installed-beacon boundaries.

use super::*;
use crate::{
    beacon,
    kura::Kura,
    query::store::LiveQueryStore,
    state::{BlockHashes, State, World},
};
use iroha_crypto::{HashOf, KeyPair};
use iroha_data_model::{
    block::BlockHeader,
    consensus::{
        ConsensusKeyId, ConsensusKeyRecord, ConsensusKeyRole, ConsensusKeyStatus,
        GlobalThresholdBeaconChainAnchorV1,
    },
};
use iroha_model_base::chain::ChainId;

fn fixture() -> (
    NetworkId,
    Vec<wire::ValidatorPower>,
    KagemushaMintFinalityAuthorityGenerationV1,
) {
    let network = NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::prehashed([0xA9; 32]),
    ));
    let mut roster = (1u8..=4)
        .map(|seed| wire::ValidatorPower {
            validator: PeerId::new(
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .unwrap()
                    .public_key()
                    .clone(),
            ),
            power: 1,
        })
        .collect::<Vec<_>>();
    roster.sort();
    let authority = crate::kagemusha_v1_test_fixtures::mint_finality_authority(network, 0, &roster);
    (network, roster, authority)
}

#[test]
fn genesis_and_repeated_retention_advance_epoch_without_relabeling_keys() {
    let (_, roster, authority) = fixture();
    let genesis = KagemushaMintFinalityEpochAuthorizationV1::genesis(&authority, 9).unwrap();
    assert_eq!(
        genesis.decision,
        KagemushaMintFinalityEpochDecisionV1::Genesis
    );
    assert_eq!(genesis.beacon, BeaconEpochBindingV1::Bootstrap);
    assert_eq!(
        (
            genesis.epoch,
            genesis.authority_generation,
            genesis.first_height,
            genesis.last_height
        ),
        (0, 0, 1, 9)
    );
    let installed = InstalledBeaconEpochBindingV1 {
        session_id: [0xA1; 32],
        transcript_hash: [0xA2; 32],
    };
    let mut election = FrozenElectionInputs {
        epoch: genesis.epoch,
        kagemusha_mint_finality_authorization: genesis,
        kagemusha_mint_finality_authority: authority.clone(),
        epoch_end_height: genesis.last_height,
        mode: wire::ConsensusMode::Permissioned,
        roster,
        leader_seed: [0xA3; 32],
    };
    let retained = retained_epoch_authorization(&election, 19, installed).unwrap();
    election.epoch = retained.epoch;
    election.kagemusha_mint_finality_authorization = retained;
    election.epoch_end_height = retained.last_height;
    let again = retained_epoch_authorization(&election, 29, installed).unwrap();
    for (previous, next, epoch, first, last) in [
        (&genesis, &retained, 1, 10, 19),
        (&retained, &again, 2, 20, 29),
    ] {
        assert_eq!(
            (next.epoch, next.first_height, next.last_height),
            (epoch, first, last)
        );
        assert_eq!(next.authority_generation, 0);
        assert_eq!(next.authority_id, authority.authority_id().unwrap());
        assert_eq!(
            next.previous_authorization_id,
            previous.authorization_id().unwrap()
        );
        assert_eq!(next.transition_id, [0; 32]);
        assert_eq!(next.decision, KagemushaMintFinalityEpochDecisionV1::Retain);
        next.validate_successor(previous).unwrap();
    }
    assert!(KagemushaMintFinalityEpochAuthorizationV1::genesis(&authority, 0).is_err());
    let mut relabeled = authority.clone();
    relabeled.generation = 1;
    assert!(KagemushaMintFinalityEpochAuthorizationV1::genesis(&relabeled, 9).is_err());
    let mut relabeled_election = election.clone();
    relabeled_election.kagemusha_mint_finality_authority = relabeled;
    assert!(retained_epoch_authorization(&relabeled_election, 29, installed).is_err());
    assert!(retained_epoch_authorization(&election, 19, installed).is_err());
    let different = InstalledBeaconEpochBindingV1 {
        session_id: [0xB1; 32],
        ..installed
    };
    assert!(retained_epoch_authorization(&election, 29, different).is_err());
}

#[test]
fn retention_beacon_requires_canonical_active_exact_incumbent_transcript() {
    const BOUNDARY: u64 = 7;
    const SUCCESSOR: u64 = BOUNDARY + 1;
    let (network, roster, authority) = fixture();
    let peers = roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    let anchor = GlobalThresholdBeaconChainAnchorV1 {
        height: BOUNDARY - 2,
        block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xC1; 32])),
    };
    let (valid, mut pulses) =
        beacon::signed_pulses_fixture_for_roster_and_anchors(network, &peers, &[anchor]);
    let pulse = pulses.pop().expect("one real signed pre-boundary pulse");
    let link = validate_persisted_global_threshold_beacon_pulse_v1(&pulse).unwrap();
    assert!(valid.is_active_at(pulse.height));
    assert!(valid.is_active_at(SUCCESSOR));
    assert_eq!(pulse.height, BOUNDARY - 1);
    let election =
        FrozenElectionInputs {
            epoch: 0,
            kagemusha_mint_finality_authorization:
                KagemushaMintFinalityEpochAuthorizationV1::genesis(&authority, BOUNDARY).unwrap(),
            kagemusha_mint_finality_authority: authority,
            epoch_end_height: BOUNDARY,
            mode: wire::ConsensusMode::Permissioned,
            roster,
            leader_seed: [0xC2; 32],
        };
    for case in 0..12 {
        let mut world = World::new();
        for seed in 1_u8..=4 {
            let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal).unwrap();
            let id = ConsensusKeyId::new(ConsensusKeyRole::Validator, format!("retained{seed}"));
            let record = ConsensusKeyRecord {
                id: id.clone(),
                public_key: key.public_key().clone(),
                pop: Some(iroha_crypto::bls_normal_pop_prove(key.private_key()).unwrap()),
                activation_height: 0,
                expiry_height: None,
                replaces: None,
                status: ConsensusKeyStatus::Active,
            };
            world
                .consensus_keys_by_pk
                .insert(record.public_key.to_string(), vec![id.clone()]);
            world.consensus_keys.insert(id, record);
        }
        let mut record = valid.clone();
        let mut pointer_key = GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY;
        let mut selected = valid.session.session_id;
        let mut candidate_pulse = pulse.clone();
        match case {
            1 => pointer_key = GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY + 1,
            2 => selected = [0xD1; 32],
            3 => record.activated_at_height = Some(SUCCESSOR),
            4 => record.retire(BOUNDARY).unwrap(),
            5 => record.session.transcript_hash[0] ^= 1,
            6 => {
                record.session.network_id = NetworkId::from_genesis_hash(
                    HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xD2; 32])),
                )
            }
            8 => record.activated_at_height = None,
            9 => record.session.committee_size += 1,
            11 => candidate_pulse.signature[0] ^= 1,
            _ => (),
        }
        {
            let mut block = world.block();
            if case != 7 {
                block
                    .global_beacon_key_sessions
                    .insert(valid.session.session_id, record);
                block
                    .global_beacon_active_session
                    .insert(pointer_key, selected);
            }
            if case != 10 {
                block
                    .global_beacon_pulses
                    .insert(candidate_pulse.pulse_id, candidate_pulse);
            }
            block
                .global_beacon_latest_pulse
                .insert(GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, link);
            block.commit();
        }
        let mut state = State::new_with_chain_and_network_id_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            ChainId::from("retained-authority-exact-beacon"),
            network,
        );
        let mut hashes = vec![
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"retained authority fixture chain"
            ),);
            usize::try_from(pulse.height).unwrap()
        ];
        hashes[usize::try_from(anchor.height - 1).unwrap()] = anchor.block_hash;
        state.block_hashes = BlockHashes::new(hashes);
        let view = state.view();
        let result = finalized_next_epoch_snapshot(&view, &network, BOUNDARY, &election);
        if case == 0 {
            let snapshot = result
                .expect("authenticated incumbent")
                .expect("exact boundary");
            assert_eq!(
                snapshot.kagemusha_mint_finality_authorization.beacon,
                BeaconEpochBindingV1::Installed(InstalledBeaconEpochBindingV1 {
                    session_id: valid.session.session_id,
                    transcript_hash: valid.session.transcript_hash
                })
            );
            assert_eq!(
                snapshot.kagemusha_mint_finality_authorization.first_height,
                SUCCESSOR
            );
            assert_eq!(snapshot.epoch_end_height, u64::MAX);
            assert_eq!(snapshot.roster, election.roster);
            let mut wrong_roster = election.clone();
            wrong_roster.roster.swap(0, 1);
            assert!(
                finalized_next_epoch_snapshot(&view, &network, BOUNDARY, &wrong_roster).is_err()
            );
        } else {
            assert!(
                result.is_err(),
                "invalid installed beacon case {case} admitted"
            );
        }
    }
}
