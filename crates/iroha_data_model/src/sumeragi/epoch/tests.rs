//! Real BLS/Pasta epoch codec, identity, and successor controls.

use super::*;
use crate::isi::kagemusha_v1::{
    BeaconEpochBindingV1, InstalledBeaconEpochBindingV1, KagemushaMintFinalityValidatorKeysV1,
};
use iroha_crypto::{Hash, KeyPair};

// Public multiples 1..10 of (-1,2) on y²=x³+5, generated independently with the
// pinned Pasta base fields in vendor/vega-prover/src/provider/pasta.rs. These are
// public test scalars, never signing inputs. The production decoder validates them.
const PALLAS: [[u8; 32]; 10] = [
    hex_literal::hex!("00000000ed302d991bf94c09fc98462200000000000000000000000000000040"),
    hex_literal::hex!("030000b067c50313fcac1144eee2fe0e0000000000000000000000000000001c"),
    hex_literal::hex!("63d232eb3b8af0b75cfcf55ade47f6ff4cdf4e47a7454cb8ed67a9ba6f56e788"),
    hex_literal::hex!("fc86bc8efbbcb878f49427618b6940409b9157e3d777a4c4c0514a8e0d92db18"),
    hex_literal::hex!("d10e70fdf461fb465db10c602adbd7b3fd9fdb0d492d1ecd4cbdffedecaa0ab3"),
    hex_literal::hex!("eb24c6f3d47de736844b67db8f8d3c439fb95c20fb81a91ff0e13ab291630705"),
    hex_literal::hex!("998b9d02ab10540a55a6ec55855c743ee3d8f8b10232bc22cc00abb11438a499"),
    hex_literal::hex!("07ef940d7798553b338b80e8de384cb8b3b6860627530de8c043716fb0ec5d34"),
    hex_literal::hex!("791b2c704a9b71222d23f6992b501fbdce116b05159a325706aec7b17ad2ce8c"),
    hex_literal::hex!("406dd76c6e8e283e2cc28d875a16a525d549807b7bce53fdbbad3784caa8e328"),
];
const VESTA: [[u8; 32]; 10] = [
    hex_literal::hex!("0000000021eb468cdda89409fc98462200000000000000000000000000000040"),
    hex_literal::hex!("03000070de065fede0093144eee2fe0e0000000000000000000000000000001c"),
    hex_literal::hex!("5fce556feb6fee5a15560ddabae10224b026a5d0281af4c613955c39a8797837"),
    hex_literal::hex!("f79037a77e26a2c0794dc326d866c664616499c064073a8f8ebf3080297be5ab"),
    hex_literal::hex!("5480a31defb30ad75ba423b14da36acb46c1cff727575a2a6b5090262da5e823"),
    hex_literal::hex!("fa9553dbbc34b5ca9c03f5ee975bbc66ef7fe6a16e4568779708648c406a8c13"),
    hex_literal::hex!("d9b64d40adcf7b8c3155141bc2e813c9c83d49cc66c199856d118b9530ebccb7"),
    hex_literal::hex!("ab2cecbc329b95461e3993c4ddb07d5132cdafea622f88869dd6af129fce1517"),
    hex_literal::hex!("7acaf6dfb451dbec3b23b191c0418c0dff2aef2717968f45d0687420aa21b511"),
    hex_literal::hex!("5dd951afd934da1f383baff361d8acc11bea9c7e6027a4e0c3fe2eb45d00dc1e"),
];

pub fn fixture(count: usize) -> ValidatorEpochContextV1 {
    let mut pairs = (1..=count)
        .map(|seed| {
            KeyPair::try_from_seed(vec![u8::try_from(seed).unwrap(); 32], Algorithm::BlsNormal)
                .unwrap()
        })
        .collect::<Vec<_>>();
    pairs.sort_by_key(|pair| PeerId::new(pair.public_key().clone()));
    let committee = pairs
        .iter()
        .map(|pair| ValidatorCommitteeMemberV1 {
            validator: PeerId::new(pair.public_key().clone()),
            proof_of_possession: iroha_crypto::bls_normal_pop_prove(pair.private_key()).unwrap(),
        })
        .collect::<Vec<_>>();
    let network_id = NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::new(b"native epoch fixture"),
    ));
    let authority = KagemushaMintFinalityAuthorityGenerationV1 {
        version: 1,
        network_id,
        generation: 0,
        validators: committee
            .iter()
            .enumerate()
            .map(|(index, member)| KagemushaMintFinalityValidatorKeysV1 {
                validator: member.validator.clone(),
                eq_proof_public_key: PALLAS[index],
                ep_proof_public_key: VESTA[index],
            })
            .collect(),
    };
    let authorization = KagemushaMintFinalityEpochAuthorizationV1::genesis(&authority, 10).unwrap();
    let context = ValidatorEpochContextV1 {
        da_layout: iroha_sumeragi::availability::recommended_data_availability_layout(),
        version: 1,
        network_id,
        mode: ConsensusMode::Npos,
        authority,
        authorization,
        committee,
        leader_seed: [0x31; 32],
    };
    context.validate().unwrap();
    context
}

pub fn retained(previous: &ValidatorEpochContextV1) -> ValidatorEpochContextV1 {
    let mut next = previous.clone();
    let authorization = &mut next.authorization;
    authorization.epoch += 1;
    authorization.first_height = previous.authorization.last_height + 1;
    authorization.last_height = authorization.first_height + 9;
    authorization.previous_authorization_id = previous.authorization.authorization_id().unwrap();
    authorization.decision = KagemushaMintFinalityEpochDecisionV1::Retain;
    authorization.beacon = BeaconEpochBindingV1::Installed(InstalledBeaconEpochBindingV1 {
        session_id: [0x41; 32],
        transcript_hash: [0x51; 32],
    });
    next.leader_seed = [0x32; 32];
    next
}

#[test]
fn complete_epoch_round_trips_real_credentials_and_binds_every_field() {
    for count in [4, 7, 10] {
        let context = fixture(count);
        let id = context.context_id().unwrap();
        let bytes = norito::encode_canonical(&context).unwrap();
        let decoded: ValidatorEpochContextV1 = norito::decode_canonical(&bytes).unwrap();
        assert_eq!(decoded, context);
        assert_eq!(decoded.context_id().unwrap(), id);
        let json = norito::json::to_json(&context).unwrap();
        assert_eq!(
            norito::json::from_str::<ValidatorEpochContextV1>(&json).unwrap(),
            context
        );
        let mut changed = context.clone();
        changed.leader_seed[0] ^= 1;
        assert_ne!(changed.context_id().unwrap(), id);
        let mut changed = context.clone();
        changed.mode = ConsensusMode::Permissioned;
        assert_ne!(changed.context_id().unwrap(), id);
        let mut changed = context.clone();
        changed.authorization.last_height += 1;
        assert_ne!(changed.context_id().unwrap(), id);
        let mut changed = context.clone();
        changed.version = 2;
        assert!(changed.validate().is_err());
        let mut changed = context.clone();
        changed.network_id = NetworkId::from_genesis_hash(
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"other network")),
        );
        assert!(changed.validate().is_err());
    }
}

#[test]
fn malformed_actual_points_order_and_proof_vectors_are_refused() {
    let context = fixture(4);
    for mutation in 0..7 {
        let mut bad = context.clone();
        match mutation {
            0 => {
                bad.committee.pop();
            }
            1 => bad.committee.swap(0, 1),
            2 => bad.committee[0].proof_of_possession.clear(),
            3 => bad.committee[0].proof_of_possession[0] ^= 1,
            4 => bad.authority.validators[0].eq_proof_public_key = [0xff; 32],
            5 => bad.authority.validators[0].ep_proof_public_key = [0; 32],
            _ => {
                bad.authority.validators[0].validator =
                    bad.authority.validators[1].validator.clone()
            }
        }
        if let Ok(authority_id) = bad.authority.authority_id() {
            bad.authorization.authority_id = authority_id;
        }
        assert!(bad.validate().is_err(), "mutation {mutation}");
        assert!(bad.context_id().is_err());
    }
    for length in [1, 2] {
        let mut short = context.clone();
        short.authorization.last_height = length;
        assert!(short.validate().is_err());
    }
}

#[test]
fn epoch_retention_keeps_generation_and_original_proofs_but_changes_context() {
    let initial = fixture(4);
    let next = retained(&initial);
    next.validate_successor(&initial).unwrap();
    assert_eq!(next.authority, initial.authority);
    assert_eq!(next.committee, initial.committee);
    assert_ne!(next.context_id().unwrap(), initial.context_id().unwrap());
    let mut skipped = next.clone();
    skipped.authorization.first_height += 1;
    assert!(skipped.validate_successor(&initial).is_err());
    let mut relabeled = next.clone();
    relabeled.authority.generation += 1;
    relabeled.authorization.authority_generation = relabeled.authority.generation;
    relabeled.authorization.authority_id = relabeled.authority.authority_id().unwrap();
    assert!(relabeled.validate_successor(&initial).is_err());
    let mut changed = next.clone();
    changed.authority.validators[0].eq_proof_public_key = PALLAS[7];
    changed.authorization.authority_id = changed.authority.authority_id().unwrap();
    assert!(changed.validate_successor(&initial).is_err());
}

#[test]
fn boundary_binds_exact_incumbent_cut_and_contiguous_successor() {
    let current = fixture(4);
    let boundary = ValidatorEpochBoundaryV1 {
        version: 1,
        height: 10,
        predecessor_context_id: current.context_id().unwrap(),
        selection_anchor: HashOf::from_untyped_unchecked(Hash::new(
            b"actual prestate cut supplied externally",
        )),
        next: retained(&current),
        preparation: None,
    };
    boundary.validate_against(&current).unwrap();
    let bytes = norito::encode_canonical(&boundary).unwrap();
    let decoded: ValidatorEpochBoundaryV1 = norito::decode_canonical(&bytes).unwrap();
    assert_eq!(decoded, boundary);
    decoded.validate_against(&current).unwrap();
    for mutation in 0..3 {
        let mut bad = boundary.clone();
        match mutation {
            0 => bad.height -= 1,
            1 => bad.predecessor_context_id[0] ^= 1,
            _ => bad.next.authorization.first_height += 1,
        }
        assert!(bad.validate_against(&current).is_err());
    }
}

#[test]
fn uniform_rank_binds_network_epochs_seed_and_peer_without_stake_weight() {
    let context = fixture(4);
    let peer = &context.committee[0].validator;
    let rank = validator_seat_rank(context.network_id, 0, 2, [7; 32], peer).unwrap();
    let bytes = norito::encode_canonical(peer).unwrap();
    let independent = Hash::new_from_chunks(&[
        b"iroha:validator-seat:v1",
        &[0],
        context.network_id.as_bytes(),
        &0_u64.to_le_bytes(),
        &2_u64.to_le_bytes(),
        &[7; 32],
        &bytes,
    ]);
    assert_eq!(rank, <[u8; 32]>::from(independent));
    assert_ne!(
        rank,
        validator_seat_rank(context.network_id, 1, 3, [7; 32], peer).unwrap()
    );
    assert_ne!(
        rank,
        validator_seat_rank(context.network_id, 0, 2, [8; 32], peer).unwrap()
    );
    assert_ne!(
        rank,
        validator_seat_rank(
            context.network_id,
            0,
            2,
            [7; 32],
            &context.committee[1].validator
        )
        .unwrap()
    );
    assert!(validator_seat_rank(context.network_id, 0, 1, [7; 32], peer).is_err());
    assert!(validator_seat_rank(context.network_id, 0, 2, [0; 32], peer).is_err());
}

#[test]
fn signed_availability_layout_binds_epoch_and_cannot_change_at_retention() {
    let original = fixture(4);
    let mut changed = original.clone();
    changed.da_layout.chunk_size_bytes /= 2;
    changed.validate().unwrap();
    assert_ne!(
        original.context_id().unwrap(),
        changed.context_id().unwrap()
    );
    let mut next = retained(&original);
    next.validate_successor(&original).unwrap();
    next.da_layout = changed.da_layout;
    assert!(next.validate_successor(&original).is_err());
    changed.da_layout.parity_shards = 0;
    assert!(changed.validate().is_err());
    assert!(crate::sumeragi_finality::core_epoch(&changed).is_err());
}
