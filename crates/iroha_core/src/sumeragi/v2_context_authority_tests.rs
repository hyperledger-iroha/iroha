//! Explicit genesis/retention authorization and authenticated installed-beacon boundaries.

use super::*;
use crate::{beacon, state::World};
use iroha_crypto::{HashOf, KeyPair};
use iroha_data_model::block::BlockHeader;

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
    let (_, _, authority) = fixture();
    let genesis = genesis_mint_finality_authorization(&authority, 9).unwrap();
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
    let retained =
        retained_mint_finality_authorization(&genesis, &authority, installed, 19).unwrap();
    let again = retained_mint_finality_authorization(&retained, &authority, installed, 29).unwrap();
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
    assert!(genesis_mint_finality_authorization(&authority, 0).is_err());
    let mut relabeled = authority.clone();
    relabeled.generation = 1;
    assert!(genesis_mint_finality_authorization(&relabeled, 9).is_err());
    assert!(retained_mint_finality_authorization(&retained, &relabeled, installed, 29).is_err());
    assert!(retained_mint_finality_authorization(&retained, &authority, installed, 19).is_err());
    let different = InstalledBeaconEpochBindingV1 {
        session_id: [0xB1; 32],
        ..installed
    };
    assert!(retained_mint_finality_authorization(&retained, &authority, different, 29).is_err());
}

#[test]
fn retention_beacon_requires_canonical_active_exact_incumbent_transcript() {
    let (network, roster, _) = fixture();
    let peers = roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    let mut valid = beacon::tests::finalized_key_session_fixture_for_context_v1(
        network,
        [0xC1; 32],
        beacon::global_threshold_beacon_roster_hash_v1(&peers),
    );
    let committed = valid.session.adaptive_dkg.finalized_at_height;
    valid.activate(committed).unwrap();
    let boundary = committed + 1;
    let successor = boundary + 1;
    for case in 0..10 {
        let world = World::new();
        let mut record = valid.clone();
        let mut pointer_key = GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY;
        let mut selected = valid.session.session_id;
        match case {
            1 => pointer_key = GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY + 1,
            2 => selected = [0xD1; 32],
            3 => record.activated_at_height = Some(successor),
            4 => record.retire(boundary).unwrap(),
            5 => record.session.transcript_hash[0] ^= 1,
            6 => {
                record.session.network_id = NetworkId::from_genesis_hash(
                    HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xD2; 32])),
                )
            }
            8 => record.activated_at_height = None,
            9 => record.session.committee_size += 1,
            _ => (),
        }
        if case != 7 {
            let mut block = world.block();
            block
                .global_beacon_key_sessions
                .insert(valid.session.session_id, record);
            block
                .global_beacon_active_session
                .insert(pointer_key, selected);
            block.commit();
        }
        let result =
            retained_mint_finality_beacon(&world.view(), &network, boundary, successor, &roster);
        if case == 0 {
            assert_eq!(
                result,
                Ok(InstalledBeaconEpochBindingV1 {
                    session_id: valid.session.session_id,
                    transcript_hash: valid.session.transcript_hash
                })
            );
            let mut wrong_roster = roster.clone();
            wrong_roster.swap(0, 1);
            assert!(
                retained_mint_finality_beacon(
                    &world.view(),
                    &network,
                    boundary,
                    successor,
                    &wrong_roster
                )
                .is_err()
            );
        } else {
            assert!(
                result.is_err(),
                "invalid installed beacon case {case} admitted"
            );
        }
    }
}
