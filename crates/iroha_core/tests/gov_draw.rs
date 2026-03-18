//! Governance VRF draw test: members + alternates ordering.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(feature = "bls")]

use iroha_core::governance::{
    draw::{Draw, run_draw},
    parliament::{CandidateRef, CandidateVariant, INPUT_DOMAIN, build_input, compute_seed},
};
use iroha_crypto::{
    Algorithm, BlsNormal, BlsSmall, KeyGenOption, KeyPair,
    vrf::{VrfProof, prove_normal_with_chain, prove_small_with_chain},
};
use iroha_data_model::ChainId;
use iroha_test_samples::{ALICE_ID, BOB_ID, CARPENTER_ID};

fn make_candidate(
    account: &iroha_data_model::account::AccountId,
    chain_id: &ChainId,
    seed: &[u8; 64],
    variant: CandidateVariant,
    key_seed: u8,
) -> (Vec<u8>, Vec<u8>, [u8; 32]) {
    let input = build_input(seed, account);
    let chain_bytes = chain_id.as_str().as_bytes();
    match variant {
        CandidateVariant::Normal => {
            let (pk_raw, sk) = BlsNormal::keypair(KeyGenOption::UseSeed(vec![key_seed; 4]));
            let (vrf_output, proof) = prove_normal_with_chain(&sk, chain_bytes, &input);
            let key_pair = KeyPair::from((pk_raw, sk));
            let (public_key, _) = key_pair.into_parts();
            let (algo, pk_payload) = public_key.to_bytes();
            assert_eq!(algo, Algorithm::BlsNormal);
            let proof_bytes = match proof {
                VrfProof::SigInG2(bytes) => bytes.to_vec(),
                _ => unreachable!("normal variant emits G2 proofs"),
            };
            (pk_payload.to_vec(), proof_bytes, vrf_output.0)
        }
        CandidateVariant::Small => {
            let (pk_raw, sk) = BlsSmall::keypair(KeyGenOption::UseSeed(vec![key_seed; 4]));
            let (vrf_output, proof) = prove_small_with_chain(&sk, chain_bytes, &input);
            let key_pair = KeyPair::from((pk_raw, sk));
            let (public_key, _) = key_pair.into_parts();
            let (algo, pk_payload) = public_key.to_bytes();
            assert_eq!(algo, Algorithm::BlsSmall);
            let proof_bytes = match proof {
                VrfProof::SigInG1(bytes) => bytes.to_vec(),
                _ => unreachable!("small variant emits G1 proofs"),
            };
            (pk_payload.to_vec(), proof_bytes, vrf_output.0)
        }
    }
}

#[test]
fn vrf_draw_returns_members_and_alternates() {
    // Synthetic candidates with fixed proofs to avoid heavy VRF key generation;
    // proofs lengths must match variant expectations to be considered verified.
    let chain: ChainId = "chain-demo".into();
    let beacon = [0xAB; 32];
    let epoch = 1u64;
    let seed = compute_seed(&chain, epoch, &beacon);

    // CandidateNormal expects proof len 96; CandidateSmall expects len 48.
    let alice = ALICE_ID.clone();
    let bob = BOB_ID.clone();
    let carol = CARPENTER_ID.clone();
    let (alice_pk, alice_proof, alice_out) =
        make_candidate(&alice, &chain, &seed, CandidateVariant::Normal, 11);
    let (bob_pk, bob_proof, bob_out) =
        make_candidate(&bob, &chain, &seed, CandidateVariant::Normal, 22);
    let (carol_pk, carol_proof, carol_out) =
        make_candidate(&carol, &chain, &seed, CandidateVariant::Small, 33);

    let candidates = vec![
        CandidateRef {
            account_id: &alice,
            variant: CandidateVariant::Normal,
            public_key: &alice_pk,
            proof: &alice_proof,
        },
        CandidateRef {
            account_id: &bob,
            variant: CandidateVariant::Normal,
            public_key: &bob_pk,
            proof: &bob_proof,
        },
        CandidateRef {
            account_id: &carol,
            variant: CandidateVariant::Small,
            public_key: &carol_pk,
            proof: &carol_proof,
        },
    ];

    let Draw {
        members,
        alternates,
        verified,
    } = run_draw(&chain, epoch, &beacon, candidates, 2, 1);

    let mut alice_proof_arr = [0u8; 96];
    alice_proof_arr.copy_from_slice(&alice_proof);
    let mut bob_proof_arr = [0u8; 96];
    bob_proof_arr.copy_from_slice(&bob_proof);
    let mut carol_proof_arr = [0u8; 48];
    carol_proof_arr.copy_from_slice(&carol_proof);
    assert!(
        iroha_crypto::vrf::verify_normal_with_chain(
            &iroha_crypto::BlsNormal::parse_public_key(&alice_pk).unwrap(),
            chain.as_str().as_bytes(),
            &build_input(&seed, &alice),
            &VrfProof::SigInG2(alice_proof_arr)
        )
        .is_some()
    );
    assert!(
        iroha_crypto::vrf::verify_normal_with_chain(
            &iroha_crypto::BlsNormal::parse_public_key(&bob_pk).unwrap(),
            chain.as_str().as_bytes(),
            &build_input(&seed, &bob),
            &VrfProof::SigInG2(bob_proof_arr)
        )
        .is_some()
    );
    assert!(
        iroha_crypto::vrf::verify_small_with_chain(
            &iroha_crypto::BlsSmall::parse_public_key(&carol_pk).unwrap(),
            chain.as_str().as_bytes(),
            &build_input(&seed, &carol),
            &VrfProof::SigInG1(carol_proof_arr)
        )
        .is_some()
    );
    let mut expected = [
        (alice_out, alice.clone()),
        (bob_out, bob.clone()),
        (carol_out, carol.clone()),
    ];
    expected.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));

    assert_eq!(members.len(), 2);
    assert_eq!(alternates.len(), 1);
    assert_eq!(verified, 3);
    // Deterministic order: descending output, tie-break on account id.
    assert_eq!(members[0], expected[0].1);
    assert_eq!(members[1], expected[1].1);
    assert_eq!(alternates[0], expected[2].1);
    // Ensure domain constant is wired so build_input is linked
    assert!(!INPUT_DOMAIN.is_empty());
}
