//! Selector integration: uses governance config to size members/alternates.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(feature = "bls")]

use iroha_core::governance::{
    parliament::{CandidateRef, CandidateVariant},
    selector::select_parliament,
};
use iroha_data_model::ChainId;
use iroha_test_samples::gen_account_in;

fn make_candidate_material(
    account: iroha_data_model::account::AccountId,
    chain: &ChainId,
    seed: &[u8; 64],
    key_seed: u8,
) -> (iroha_data_model::account::AccountId, Vec<u8>, Vec<u8>) {
    use iroha_core::governance::parliament::build_input;
    use iroha_crypto::{
        Algorithm, BlsNormal, KeyGenOption, KeyPair,
        vrf::{VrfProof, prove_normal_with_chain},
    };

    let input = build_input(seed, &account);
    let (pk_raw, sk) = BlsNormal::keypair(KeyGenOption::UseSeed(vec![key_seed; 4]));
    let (_vrf_output, proof) = prove_normal_with_chain(&sk, chain.as_str().as_bytes(), &input);
    let key_pair = KeyPair::from((pk_raw, sk));
    let (public_key, _) = key_pair.into_parts();
    let (algo, pk_payload) = public_key.to_bytes();
    assert_eq!(algo, Algorithm::BlsNormal);
    let proof_bytes = match proof {
        VrfProof::SigInG2(arr) => arr.to_vec(),
        VrfProof::SigInG1(arr) => arr.to_vec(),
    };
    (account, pk_payload.to_vec(), proof_bytes)
}

#[test]
fn selector_respects_config_sizes() {
    let chain: ChainId = "chain-demo".into();
    let beacon = [0xCD; 32];
    let epoch = 7u64;
    let (alice, _) = gen_account_in("wonderland");
    let (bob, _) = gen_account_in("wonderland");
    let (carol, _) = gen_account_in("wonderland");

    let seed = iroha_core::governance::parliament::compute_seed(&chain, epoch, &beacon);
    let materials = [
        make_candidate_material(alice, &chain, &seed, 1),
        make_candidate_material(bob, &chain, &seed, 2),
        make_candidate_material(carol, &chain, &seed, 3),
    ];
    let candidates = materials
        .iter()
        .map(|(account, public_key, proof)| CandidateRef {
            account_id: account,
            variant: CandidateVariant::Normal,
            public_key,
            proof,
        });

    let cfg = iroha_config::parameters::actual::Governance {
        parliament_committee_size: 2,
        parliament_alternate_size: Some(1),
        ..Default::default()
    };
    let draw = select_parliament(&cfg, &chain, epoch, &beacon, candidates);
    assert_eq!(draw.members.len(), 2);
    assert_eq!(draw.alternates.len(), 1);
    assert_eq!(draw.verified, 3);
}
