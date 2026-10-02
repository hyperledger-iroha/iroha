//! Active ordinary financial graph qualification over actual generated proof originals.
//! This module uses only known-public mathematical fixture signers. It installs no release,
//! Native wallet, trusted clock, FI/Node debit or global DATA authorization.
use super::*;
use crate::kagemusha_v1_recursion::mint_authority::KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1;
use crate::kagemusha_v1_recursion::real_handoff_qualification_tests::real_payment_corridor::{
    OrdinaryProvenMintSourceForTestingV1, ordinary_zero_bootstrap_hash_keys_for_testing,
    prove_ordinary_neutral_mint_source_for_testing_v1,
    run_ordinary_zero_bootstrap_qualification_worker,
};
use crate::kagemusha_v1_state::DigestV1;
use ff::Field as _;

#[test]
#[ignore = "genuine Mint113 and neutral MintAuthority Bootstrap/finalized keys/proofs; owned direct-libtest CPU/RSS guard and exclusive maintained worker required"]
fn ordinary_mint113_neutral_finality_generated_keys_both_parities_bind_full_credit_original() {
    run_ordinary_zero_bootstrap_qualification_worker(qualify_mint_source);
}

fn qualify_mint_source() {
    let release = [41; 32];
    let vk = [42; 32];
    let manifest = [43; 32];
    let (hash_eq, hash_ep) = ordinary_zero_bootstrap_hash_keys_for_testing(release, vk, manifest);
    let f = fixture_for_active_state(false, release, hash_eq.suite_id, vk, manifest, [47; 32]);
    let eq = canonical_kagemusha_eq_parameters_v1();
    let ep = canonical_kagemusha_ep_parameters_v1();
    let seed = KagemushaRecoverySeedV1::from_unsealed([44; 32]).unwrap();
    let witness = OrdinaryMintWitnessV1 {
        statement: &f.statement,
        approval: &f.approval,
        credential: &f.enrollment.credential,
        previous_app_attest_counter: f.enrollment.previous_counter,
        integrity_lease: None,
        financial_secret: &[0x41; 32],
        credit_opening: &f.opening,
        encrypted_credit: &f.encrypted_credit,
    };
    let generated = super::super::ordinary_mint_generation::generate_ordinary_mint_pair_v1(
        witness,
        f.enrollment.state.device_policy_binding.hardware_policy_id,
        &f.enrollment.issuer_table,
        &seed,
    )
    .unwrap();
    let authorization = KagemushaOrdinaryMintAuthorizationV1 {
        version: 1,
        statement: f.statement.clone(),
        approval: f.approval.clone(),
        proof: KagemushaOrdinaryMintPairedProofV1 {
            version: 1,
            eq_protocol_digest: generated.eq.protocol_digest,
            ep_protocol_digest: generated.ep.protocol_digest,
            statement_digest: f.statement.binding_digest().unwrap(),
            approval_original_digest: f.approval.binding_digest().unwrap(),
            eq_proof: generated.eq.proof.clone(),
            ep_proof: generated.ep.proof.clone(),
            eq_history: generated.eq.history.as_bytes().to_vec(),
            ep_history: generated.ep.history.as_bytes().to_vec(),
        },
    };
    authorization.validate_shape().unwrap();
    assert!(decide_eq(
        &eq,
        &generated.eq.protocol,
        &generated.eq.proof,
        &generated.eq.instances
    ));
    assert!(decide_ep(
        &ep,
        &generated.ep.protocol,
        &generated.ep.proof,
        &generated.ep.instances
    ));
    // Shape seeds are used only with the disabled neutral Bootstrap parent. The existing
    // bounded key convergence replaces them before either active finality proof is produced.
    let eq_seed = compile(&eq, &VerifyingKey::<EqAffine>::read::<_,
        crate::kagemusha_v1_recursion::ordinary_mint_circuit::KagemushaOrdinaryMintEqCircuitV1>(
        &mut Cursor::new(generated.eq.verifying_key.as_ref()), SerdeFormat::Processed,
        crate::kagemusha_v1_recursion::ordinary_mint_circuit::KagemushaOrdinaryMintCircuitParamsV1 {
            base: generated.eq.base_params.clone(),
            provider_policy_root: f.enrollment.state.device_policy_binding.hardware_policy_id,
            issuer_table: f.enrollment.issuer_table.clone(),
        }).unwrap(),
        snark_verifier::system::halo2::Config::ipa().with_num_instance(vec![KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]));
    let ep_seed = compile(&ep, &VerifyingKey::<EpAffine>::read::<_,
        crate::kagemusha_v1_recursion::ordinary_mint_circuit::KagemushaOrdinaryMintEpCircuitV1>(
        &mut Cursor::new(generated.ep.verifying_key.as_ref()), SerdeFormat::Processed,
        crate::kagemusha_v1_recursion::ordinary_mint_circuit::KagemushaOrdinaryMintCircuitParamsV1 {
            base: generated.ep.base_params.clone(),
            provider_policy_root: f.enrollment.state.device_policy_binding.hardware_policy_id,
            issuer_table: f.enrollment.issuer_table.clone(),
        }).unwrap(),
        snark_verifier::system::halo2::Config::ipa().with_num_instance(vec![KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]));
    drop(generated);
    halo2_proofs::release_allocator_slack();
    let source = prove_ordinary_neutral_mint_source_for_testing_v1(
        &authorization,
        &f.encrypted_credit,
        hash_eq,
        hash_ep,
        eq_seed,
        ep_seed,
    );
    require_actual_mint_source(&eq, &ep, &authorization, &source);
    let mut original = norito::encode_canonical(&source.credit).unwrap();
    let digest: DigestV1 = Sha256::digest(&original).into();
    let midpoint = original.len() / 2;
    original[midpoint] ^= 1;
    assert_ne!(digest, <DigestV1>::from(Sha256::digest(&original)));
    for slot in [0, 1, 2, 9, 18, 21, 23, 25] {
        let mut eq_column = source.eq_instances.clone();
        eq_column[slot] += Fp::ONE;
        assert!(!decide_eq(
            &eq,
            &source.eq_protocol,
            &source.credit.proof.eq_proof,
            &eq_column
        ));
        let mut ep_column = source.ep_instances.clone();
        ep_column[slot] += Fq::ONE;
        assert!(!decide_ep(
            &ep,
            &source.ep_protocol,
            &source.credit.proof.ep_proof,
            &ep_column
        ));
    }
    let mut eq_history = source.credit.proof.eq_history.clone();
    eq_history[0] ^= 1;
    let mut eq_column = source.eq_instances.clone();
    let offset = eq_column.len() - eq_history.len() / 16;
    for (cell, limb) in eq_column[offset..]
        .iter_mut()
        .zip(eq_history.chunks_exact(16))
    {
        *cell =
            crate::kagemusha_v1_poseidon::from_u128(u128::from_le_bytes(limb.try_into().unwrap()));
    }
    assert!(!decide_eq(
        &eq,
        &source.eq_protocol,
        &source.credit.proof.eq_proof,
        &eq_column
    ));
    assert!(!decide_ep(
        &ep,
        &source.ep_protocol,
        &source.credit.proof.eq_proof,
        &source.ep_instances
    ));
}

pub(super) fn require_actual_mint_source(
    eq: &ParamsIPA<EqAffine>,
    ep: &ParamsIPA<EpAffine>,
    authorization: &KagemushaOrdinaryMintAuthorizationV1,
    source: &OrdinaryProvenMintSourceForTestingV1,
) {
    assert_eq!(
        source.credit.statement,
        authorization.finalized_credit_statement(1020).unwrap()
    );
    assert_eq!(
        source.credit.proof.eq_history.as_slice(),
        source.eq_history.as_bytes().as_slice()
    );
    assert_eq!(
        source.credit.proof.ep_history.as_slice(),
        source.ep_history.as_bytes().as_slice()
    );
    assert_eq!(
        source.credit.statement.mint_authorization_digest,
        authorization.binding_digest().unwrap()
    );
    assert_eq!(
        source.credit.proof.eq_protocol_digest,
        native_parent_protocol_digest_v1(&source.eq_protocol, KagemushaPastaParityV1::Eq).unwrap()
    );
    assert_eq!(
        source.credit.proof.ep_protocol_digest,
        native_parent_protocol_digest_v1(&source.ep_protocol, KagemushaPastaParityV1::Ep).unwrap()
    );
    assert!(decide_eq(
        eq,
        &source.eq_protocol,
        &source.credit.proof.eq_proof,
        &source.eq_instances
    ));
    assert!(decide_ep(
        ep,
        &source.ep_protocol,
        &source.credit.proof.ep_proof,
        &source.ep_instances
    ));
    decide_kagemusha_eq_accumulator_v1(eq, &source.eq_history).unwrap();
    decide_kagemusha_ep_accumulator_v1(ep, &source.ep_history).unwrap();
}
