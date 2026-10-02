//! Full mathematical ordinary zero-Bootstrap qualification with genuine keys/proofs.
//!
//! No signed release, FI certificate, accepting verifier or Native owner is manufactured.
//! Inactive predecessor/incoming/mint slots are exact parser operands with selector zero.
//! Guard, ordered SHA, State carrier, reciprocal audits and transport decisions all execute.

use super::super::{
    DigestV1,
    composite::{
        KagemushaRecursiveStateEpCircuitV1, KagemushaRecursiveStateEqCircuitV1,
        RecursiveStateConstructionV1,
    },
    mint_authority::KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1,
    ordinary_guard_circuit::OrdinaryGuardWitnessV1,
    ordinary_mint_circuit::ORDINARY_MINT_PUBLIC_INSTANCE_COUNT_V1,
    ordinary_state_reserved::kagemusha_ordinary_state_reserved_guard_positions_v1,
    terminal_authorization::{
        TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1, public_instance as incoming_slot,
    },
};
use super::*;
use halo2_base::gates::{
    GateInstructions as _, RangeInstructions as _, circuit::builder::BaseCircuitBuilder,
};
use halo2_proofs::dev::MockProver;

#[path = "ordinary_zero_bootstrap_fixture.rs"]
mod originals;

fn mathematical_protocol<C>(parameters: &ParamsIPA<C>, width: usize) -> PlonkProtocol<C>
where
    C: CurveAffineExt,
    C::Base: halo2_base::utils::BigPrimeField,
    C::ScalarExt: crate::kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
{
    let mut builder = BaseCircuitBuilder::<C::ScalarExt>::new(false)
        .use_k(16)
        .use_instance_columns(1);
    let gate = builder.range_chip();
    let cells = (0..width)
        .map(|i| {
            let value = C::ScalarExt::from(i as u64 + 1);
            let cell = builder.main(0).load_witness(value);
            gate.gate().assert_is_const(builder.main(0), &cell, &value);
            cell
        })
        .collect();
    builder.assigned_instances = vec![cells];
    builder.calculate_params(Some(9));
    let vk = keygen_vk(parameters, &builder).unwrap();
    compile(
        parameters,
        &vk,
        snark_verifier::system::halo2::Config::ipa().with_num_instance(vec![width]),
    )
}

fn column<F: crate::kagemusha_v1_poseidon::KagemushaPoseidonFieldV1>(
    width: usize,
    history: &[u8],
) -> Vec<Vec<F>> {
    let mut cells = vec![F::ZERO; width];
    let offset = width - history.len() / 16;
    for (cell, bytes) in cells[offset..].iter_mut().zip(history.chunks_exact(16)) {
        *cell =
            crate::kagemusha_v1_poseidon::from_u128(u128::from_le_bytes(bytes.try_into().unwrap()));
    }
    vec![cells]
}

fn incoming_column<F: crate::kagemusha_v1_poseidon::KagemushaPoseidonFieldV1>(
    eq: DigestV1,
    ep: DigestV1,
    history: &[u8],
) -> Vec<Vec<F>> {
    let mut cells = column::<F>(TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1, history);
    let eq = crate::kagemusha_v1_poseidon::digest_limbs::<F>(eq);
    let ep = crate::kagemusha_v1_poseidon::digest_limbs::<F>(ep);
    cells[0][incoming_slot::EQ_PROTOCOL_LO..incoming_slot::EQ_PROTOCOL_LO + 2].copy_from_slice(&eq);
    cells[0][incoming_slot::EP_PROTOCOL_LO..incoming_slot::EP_PROTOCOL_LO + 2].copy_from_slice(&ep);
    cells
}

fn canonical_sha<T: norito::NoritoSerialize>(domain: &[u8], value: &T) -> DigestV1 {
    let bytes = norito::encode_canonical(value).unwrap();
    let mut hash = Sha256::new();
    hash.update((domain.len() as u64).to_be_bytes());
    hash.update(domain);
    hash.update((bytes.len() as u64).to_be_bytes());
    hash.update(bytes);
    hash.finalize().into()
}

fn exact_queue(
    mut witness: KagemushaRecursiveStateGenerationWitnessV1<'_>,
) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
    witness.hash_claim = None;
    witness.state.eq_protocol_digest =
        native_parent_protocol_digest_v1(witness.eq_parent_protocol, KagemushaPastaParityV1::Eq)
            .unwrap();
    witness.state.ep_protocol_digest =
        native_parent_protocol_digest_v1(witness.ep_parent_protocol, KagemushaPastaParityV1::Ep)
            .unwrap();
    let eq_incoming = witness
        .eq_incoming_credits
        .map(KagemushaRecursiveIncomingEqGenerationWitnessV1::into_composite);
    let ep_incoming = witness
        .ep_incoming_credits
        .map(KagemushaRecursiveIncomingEpGenerationWitnessV1::into_composite);
    match super::super::composite::build_recursive_state_pair_impl_v1(
        &canonical_kagemusha_eq_parameters_v1(),
        &canonical_kagemusha_ep_parameters_v1(),
        witness.into_recursive(&eq_incoming, &ep_incoming),
        true,
        RecursiveStateConstructionV1::OrdinaryZeroBootstrapQualification,
    )
    .unwrap()
    {
        super::super::composite::RecursiveStateBuildV1::Messages(eq, ep) => (eq, ep),
        _ => panic!("exact SHA discovery cannot return an unauthenticated circuit"),
    }
}

#[test]
#[ignore = "machine-scale genuine Guard, complete SHA, State keys/proofs; run exclusively under the maintained resource guard"]
fn ordinary_zero_bootstrap_generated_keys_both_parities_bind_originals_purpose_and_reserved_positions()
 {
    super::super::real_handoff_qualification_tests::real_payment_corridor::run_ordinary_zero_bootstrap_qualification_worker(qualify);
}

fn qualify() {
    let release_id = [41; 32];
    let vk_digest = [42; 32];
    let manifest = [43; 32];
    let (hash_eq, hash_ep) = super::super::real_handoff_qualification_tests::real_payment_corridor::ordinary_zero_bootstrap_hash_keys_for_testing(release_id, vk_digest, manifest);
    let eq_params = canonical_kagemusha_eq_parameters_v1();
    let ep_params = canonical_kagemusha_ep_parameters_v1();
    let seed = KagemushaRecoverySeedV1::from_unsealed([44; 32]).unwrap();
    for apple in [false, true] {
        let f = originals::fixture(apple, release_id, hash_eq.suite_id, vk_digest);
        f.state.validate().unwrap();
        f.relation.validate().unwrap();
        let guard = super::ordinary_guard_generation::generate_ordinary_guard_pair_v1(
            OrdinaryGuardWitnessV1 {
                relation: &f.relation,
                credential: &f.credential,
                approval: &f.approval,
                previous_app_attest_counter: f.previous_counter,
                integrity_lease: None,
            },
            f.state.device_policy_binding.hardware_policy_id,
            &f.issuer_table,
            &seed,
        )
        .expect("genuine whole ordinary Guard PK/VK and original proofs in both fields");
        let eq_zero = initial_kagemusha_eq_accumulator_v1(&eq_params).unwrap();
        let ep_zero = initial_kagemusha_ep_accumulator_v1(&ep_params).unwrap();
        let eq_parent = mathematical_protocol(&eq_params, recursive_public_instance_count());
        let ep_parent = mathematical_protocol(&ep_params, recursive_public_instance_count());
        let eq_incoming =
            mathematical_protocol(&eq_params, TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1);
        let ep_incoming =
            mathematical_protocol(&ep_params, TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1);
        let eq_authorization =
            mathematical_protocol(&eq_params, ORDINARY_MINT_PUBLIC_INSTANCE_COUNT_V1);
        let ep_authorization =
            mathematical_protocol(&ep_params, ORDINARY_MINT_PUBLIC_INSTANCE_COUNT_V1);
        let eq_mint = mathematical_protocol(
            &eq_params,
            KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1,
        );
        let ep_mint = mathematical_protocol(
            &ep_params,
            KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1,
        );
        let recipient = iroha_data_model::account::AccountId::new(
            iroha_crypto::KeyPair::from_seed(vec![45; 32], iroha_crypto::Algorithm::Ed25519)
                .public_key()
                .clone(),
        );
        let (authorization, credit) =
            super::production_prover::ordinary_bootstrap_padding_for_testing(
                &f.state,
                &recipient,
                manifest,
                [46; 32],
                &eq_authorization,
                &ep_authorization,
                &eq_mint,
                &ep_mint,
                &eq_zero,
                &ep_zero,
            )
            .expect("same production zero-Bootstrap parser constructor");
        let eq_authorization_instances =
            column::<Fp>(ORDINARY_MINT_PUBLIC_INSTANCE_COUNT_V1, eq_zero.as_bytes());
        let ep_authorization_instances =
            column::<Fq>(ORDINARY_MINT_PUBLIC_INSTANCE_COUNT_V1, ep_zero.as_bytes());
        let request = super::super::KagemushaMintFinalityHelperVerificationRequestV1 {
            eq_protocol_digest: credit.proof.eq_protocol_digest,
            ep_protocol_digest: credit.proof.ep_protocol_digest,
            statement: &credit.statement,
            semantic_digest: credit.proof.semantic_digest,
            proof: &credit.proof,
            finality_certificate_binding: credit.finality_certificate_binding,
            finality_authority_head: credit.finality_authority_head,
            finality_genesis_authorization_id: credit.finality_genesis_authorization_id,
            finality_proof_binding_digest: credit.finality_proof_binding_digest,
            artifact_manifest_digest: manifest,
        };
        let eq_mint_instances = vec![
            super::super::native_backend::mint_public_instances::<Fp>(&request, eq_zero.as_bytes())
                .unwrap(),
        ];
        let ep_mint_instances = vec![
            super::super::native_backend::mint_public_instances::<Fq>(&request, ep_zero.as_bytes())
                .unwrap(),
        ];
        let eq_current = KagemushaEqAccumulatorV1::from_native(
            &verify_eq_succinct_protocol(
                &eq_params,
                &guard.eq.protocol,
                &guard.eq.proof,
                &guard.eq.instances,
            )
            .unwrap(),
        )
        .unwrap();
        let ep_current = KagemushaEpAccumulatorV1::from_native(
            &verify_ep_succinct_protocol(
                &ep_params,
                &guard.ep.protocol,
                &guard.ep.proof,
                &guard.ep.instances,
            )
            .unwrap(),
        )
        .unwrap();
        let eq_complete =
            fold_kagemusha_eq_accumulators_v1(&eq_params, &eq_current, &guard.eq.history, &seed)
                .unwrap();
        let ep_complete =
            fold_kagemusha_ep_accumulators_v1(&ep_params, &ep_current, &guard.ep.history, &seed)
                .unwrap();
        let eq_merge =
            fold_kagemusha_eq_accumulators_v1(&eq_params, &eq_zero, eq_complete.successor(), &seed)
                .unwrap();
        let ep_merge =
            fold_kagemusha_ep_accumulators_v1(&ep_params, &ep_zero, ep_complete.successor(), &seed)
                .unwrap();
        let eq_fold = KagemushaEqFoldProofV1::try_from_bytes(&dummy_fold_proof_bytes(
            EqAffine::generator().to_bytes().as_ref(),
        ))
        .unwrap();
        let ep_fold = KagemushaEpFoldProofV1::try_from_bytes(&dummy_fold_proof_bytes(
            EpAffine::generator().to_bytes().as_ref(),
        ))
        .unwrap();
        let eq_parent_proof = dummy_ordinary_proof_bytes(
            &eq_parent,
            EqAffine::generator().to_bytes().as_ref(),
            KagemushaPastaParityV1::Eq,
        )
        .unwrap();
        let ep_parent_proof = dummy_ordinary_proof_bytes(
            &ep_parent,
            EpAffine::generator().to_bytes().as_ref(),
            KagemushaPastaParityV1::Ep,
        )
        .unwrap();
        let eq_incoming_proof = dummy_ordinary_proof_bytes(
            &eq_incoming,
            EqAffine::generator().to_bytes().as_ref(),
            KagemushaPastaParityV1::Eq,
        )
        .unwrap();
        let ep_incoming_proof = dummy_ordinary_proof_bytes(
            &ep_incoming,
            EpAffine::generator().to_bytes().as_ref(),
            KagemushaPastaParityV1::Ep,
        )
        .unwrap();
        let eq_parent_instances =
            column::<Fp>(recursive_public_instance_count(), eq_zero.as_bytes());
        let ep_parent_instances =
            column::<Fq>(recursive_public_instance_count(), ep_zero.as_bytes());
        let protocol_eq =
            |p| native_parent_protocol_digest_v1(p, KagemushaPastaParityV1::Eq).unwrap();
        let protocol_ep =
            |p| native_parent_protocol_digest_v1(p, KagemushaPastaParityV1::Ep).unwrap();
        let incoming_eq_digest = protocol_eq(&eq_incoming);
        let incoming_ep_digest = protocol_ep(&ep_incoming);
        let eq_incoming_instances =
            incoming_column::<Fp>(incoming_eq_digest, incoming_ep_digest, eq_zero.as_bytes());
        let ep_incoming_instances =
            incoming_column::<Fq>(incoming_eq_digest, incoming_ep_digest, ep_zero.as_bytes());
        let (reserved_eq, reserved_ep) = kagemusha_ordinary_state_reserved_guard_positions_v1();
        let relation = KagemushaStateRelationWitnessV1 {
            operation: KagemushaOperationV1::Bootstrap,
            predecessor: None,
            successor: f.state.clone(),
            amount: 0,
            journal_revision_before: 0,
            journal_revision_after: 0,
            transition_effect_digest: f.relation.statement.transition_effect_digest,
            mint_finality_semantic_digest: [0; 32],
            mint_finality_proof_binding_digest: [0; 32],
            peer_credit_id: [0; 32],
            recipient_encryption_key_binding: [0; 32],
            receive_credit: None,
            receive_credit_binding_digest: [0; 32],
            lifecycle_binding_digest: f.relation.statement.lifecycle_binding_digest,
            prepared_transition_binding_digest: [0; 32],
            prepared_intent: None,
            transport_semantic_digest: canonical_sha(
                b"iroha:kagemusha:v1:transport-statement\0",
                &f.relation.statement_digest(),
            ),
            guard_statement_digest: f.relation.statement_digest(),
            eq_protocol_digest: protocol_eq(&eq_parent),
            ep_protocol_digest: protocol_ep(&ep_parent),
            guard_eq_protocol_digest: guard.eq.protocol_digest,
            guard_ep_protocol_digest: guard.ep.protocol_digest,
            mint_eq_protocol_digest: protocol_eq(&eq_mint),
            mint_ep_protocol_digest: protocol_ep(&ep_mint),
            mint_authorization_eq_protocol_digest: protocol_eq(&eq_authorization),
            mint_authorization_ep_protocol_digest: protocol_ep(&ep_authorization),
            commit_wrapper_eq_protocol_digest: incoming_eq_digest,
            commit_wrapper_ep_protocol_digest: incoming_ep_digest,
            guard_eq_credential_audit: reserved_eq,
            guard_ep_credential_audit: reserved_ep,
            eq_deferred_audit: [1; 32],
            ep_deferred_audit: [2; 32],
            replay_insert: None,
        };
        relation.validate().unwrap();
        let mut witness = KagemushaRecursiveStateGenerationWitnessV1 {
            hash_claim: None,
            state: relation,
            mint_fold_opening: None,
            mint_authorization: &authorization,
            mint_credit: &credit,
            guard_relation: f.relation.clone(),
            hardware_selection: None,
            ordinary_selection: Some(KagemushaOrdinaryAppRecursiveSelectionWitnessV1 {
                credential: &f.credential,
                approval: &f.approval,
                integrity_lease: None,
                previous_app_attest_counter: f.previous_counter,
                prepared: None,
                incoming_mint: None,
            }),
            eq_parent_protocol: &eq_parent,
            ep_parent_protocol: &ep_parent,
            eq_parent_instances: &eq_parent_instances,
            ep_parent_instances: &ep_parent_instances,
            eq_parent_proof: &eq_parent_proof,
            ep_parent_proof: &ep_parent_proof,
            eq_predecessor_history: &eq_zero,
            ep_predecessor_history: &ep_zero,
            eq_parent_fold_proof: &eq_fold,
            ep_parent_fold_proof: &ep_fold,
            eq_incoming_protocol: &eq_incoming,
            ep_incoming_protocol: &ep_incoming,
            eq_incoming_credits: [KagemushaRecursiveIncomingEqGenerationWitnessV1 {
                instances: &eq_incoming_instances,
                proof: &eq_incoming_proof,
                history: &eq_zero,
                history_fold_proof: &eq_fold,
                merge_fold_proof: &eq_fold,
            }],
            ep_incoming_credits: [KagemushaRecursiveIncomingEpGenerationWitnessV1 {
                instances: &ep_incoming_instances,
                proof: &ep_incoming_proof,
                history: &ep_zero,
                history_fold_proof: &ep_fold,
                merge_fold_proof: &ep_fold,
            }],
            eq_successor_history: eq_merge.successor(),
            ep_successor_history: ep_merge.successor(),
            eq_guard_protocol: &guard.eq.protocol,
            ep_guard_protocol: &guard.ep.protocol,
            eq_guard_proof: &guard.eq.proof,
            ep_guard_proof: &guard.ep.proof,
            eq_guard_history: &guard.eq.history,
            ep_guard_history: &guard.ep.history,
            eq_guard_history_fold_proof: eq_complete.proof(),
            ep_guard_history_fold_proof: ep_complete.proof(),
            eq_guard_merge_fold_proof: eq_merge.proof(),
            ep_guard_merge_fold_proof: ep_merge.proof(),
            eq_mint_authorization_protocol: &eq_authorization,
            ep_mint_authorization_protocol: &ep_authorization,
            eq_mint_authorization_instances: &eq_authorization_instances,
            ep_mint_authorization_instances: &ep_authorization_instances,
            eq_mint_authorization_proof: &authorization.proof.eq_proof,
            ep_mint_authorization_proof: &authorization.proof.ep_proof,
            eq_mint_authorization_history: &eq_zero,
            ep_mint_authorization_history: &ep_zero,
            eq_mint_authorization_history_fold_proof: &eq_fold,
            ep_mint_authorization_history_fold_proof: &ep_fold,
            eq_mint_authorization_merge_fold_proof: &eq_fold,
            ep_mint_authorization_merge_fold_proof: &ep_fold,
            eq_mint_protocol: &eq_mint,
            ep_mint_protocol: &ep_mint,
            eq_mint_instances: &eq_mint_instances,
            ep_mint_instances: &ep_mint_instances,
            eq_mint_proof: &credit.proof.eq_proof,
            ep_mint_proof: &credit.proof.ep_proof,
            eq_mint_history: &eq_zero,
            ep_mint_history: &ep_zero,
            eq_mint_history_fold_proof: &eq_fold,
            ep_mint_history_fold_proof: &ep_fold,
            eq_mint_merge_fold_proof: &eq_fold,
            ep_mint_merge_fold_proof: &ep_fold,
        };
        let construction = RecursiveStateConstructionV1::OrdinaryZeroBootstrapQualification;
        let initial_queue = exact_queue(witness.clone());
        let claim = prove_kagemusha_recursive_state_hash_claim_v1_with_construction(
            &hash_eq,
            &hash_ep,
            witness.clone(),
            &seed,
            construction,
        )
        .expect("actual complete ordered SHA proof for the exact ordinary State queue");
        let eq_hash_merge = fold_kagemusha_eq_accumulators_v1(
            &eq_params,
            witness.eq_successor_history,
            &claim.eq_complete_history,
            &seed,
        )
        .unwrap();
        let ep_hash_merge = fold_kagemusha_ep_accumulators_v1(
            &ep_params,
            witness.ep_successor_history,
            &claim.ep_complete_history,
            &seed,
        )
        .unwrap();
        witness.hash_claim = Some(
            claim
                .consumer_witness(
                    &hash_eq,
                    &hash_ep,
                    eq_hash_merge.proof(),
                    ep_hash_merge.proof(),
                )
                .unwrap(),
        );
        witness.eq_successor_history = eq_hash_merge.successor();
        witness.ep_successor_history = ep_hash_merge.successor();
        assert!(
            generate_kagemusha_recursive_state_artifacts_v1(witness.clone(), &seed).is_err(),
            "public State generator remains closed"
        );
        let generated = generate_kagemusha_recursive_state_artifacts_v1_with_construction(witness.clone(), &seed, construction)
            .expect("actual generated ordinary State carrier + transport PK/VK and native terminal decisions in BOTH fields");
        let eq_vk = VerifyingKey::<EqAffine>::read::<_, KagemushaRecursiveStateEqCircuitV1>(
            &mut Cursor::new(generated.inner_eq.verifying_key.as_ref()),
            SerdeFormat::Processed,
            generated.inner_eq_circuit_params.clone(),
        )
        .unwrap();
        let ep_vk = VerifyingKey::<EpAffine>::read::<_, KagemushaRecursiveStateEpCircuitV1>(
            &mut Cursor::new(generated.inner_ep.verifying_key.as_ref()),
            SerdeFormat::Processed,
            generated.inner_ep_circuit_params.clone(),
        )
        .unwrap();
        let eq_inner = compile(
            &eq_params,
            &eq_vk,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![recursive_public_instance_count()]),
        );
        let ep_inner = compile(
            &ep_params,
            &ep_vk,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![recursive_public_instance_count()]),
        );
        assert_eq!(protocol_eq(&eq_inner), generated.inner_eq_protocol_digest);
        assert_eq!(protocol_ep(&ep_inner), generated.inner_ep_protocol_digest);
        let eq_inner_padding = dummy_ordinary_proof_bytes(
            &eq_inner,
            EqAffine::generator().to_bytes().as_ref(),
            KagemushaPastaParityV1::Eq,
        )
        .unwrap();
        let ep_inner_padding = dummy_ordinary_proof_bytes(
            &ep_inner,
            EpAffine::generator().to_bytes().as_ref(),
            KagemushaPastaParityV1::Ep,
        )
        .unwrap();
        witness.eq_parent_protocol = &eq_inner;
        witness.ep_parent_protocol = &ep_inner;
        witness.eq_parent_proof = &eq_inner_padding;
        witness.ep_parent_proof = &ep_inner_padding;
        witness.state.eq_protocol_digest = generated.inner_eq_protocol_digest;
        witness.state.ep_protocol_digest = generated.inner_ep_protocol_digest;
        // Signed originals and public protocol identities are immutable. If converged
        // parent parser structure changes a SHA operand, retain a newly proved complete
        // claim and new merge from the original Guard-complete history, never stale SHA.
        let final_queue = exact_queue(witness.clone());
        let replanned = if final_queue != initial_queue {
            Some(
                prove_kagemusha_recursive_state_hash_claim_v1_with_construction(
                    &hash_eq,
                    &hash_ep,
                    witness.clone(),
                    &seed,
                    construction,
                )
                .expect("converged State parser requires its actual complete SHA claim"),
            )
        } else {
            None
        };
        let refreshed_eq_merge = replanned.as_ref().map(|claim| {
            fold_kagemusha_eq_accumulators_v1(
                &eq_params,
                eq_merge.successor(),
                &claim.eq_complete_history,
                &seed,
            )
            .unwrap()
        });
        let refreshed_ep_merge = replanned.as_ref().map(|claim| {
            fold_kagemusha_ep_accumulators_v1(
                &ep_params,
                ep_merge.successor(),
                &claim.ep_complete_history,
                &seed,
            )
            .unwrap()
        });
        if let (Some(claim), Some(eq), Some(ep)) =
            (&replanned, &refreshed_eq_merge, &refreshed_ep_merge)
        {
            witness.hash_claim = Some(
                claim
                    .consumer_witness(&hash_eq, &hash_ep, eq.proof(), ep.proof())
                    .unwrap(),
            );
            witness.eq_successor_history = eq.successor();
            witness.ep_successor_history = ep.successor();
        }
        let (_, _, eq_audit, ep_audit) = build_recursive_generation_pair_v1(
            &eq_params,
            &ep_params,
            witness.clone(),
            construction,
        )
        .unwrap();
        witness.state.eq_deferred_audit = eq_audit;
        witness.state.ep_deferred_audit = ep_audit;
        for mutation in 0..7 {
            let mut bad = witness.clone();
            let mut substituted_credential = f.credential.clone();
            let mut substituted_approval = f.approval.clone();
            match mutation {
                0 => {}
                1 => {
                    substituted_credential.subject.enrollment_id[0] ^= 1;
                    originals::resign_credential(&mut substituted_credential);
                    bad.ordinary_selection.as_mut().unwrap().credential = &substituted_credential;
                }
                2 => {
                    substituted_approval.challenge.nonce[0] ^= 1;
                    substituted_approval.evidence =
                        originals::sign_approval(&substituted_approval.challenge, apple);
                    bad.ordinary_selection.as_mut().unwrap().approval = &substituted_approval;
                }
                3 => {
                    substituted_approval.challenge.purpose = iroha_data_model::kagemusha::KagemushaAppOperationApprovalPurposeV1::PrepareTransition;
                    substituted_approval.evidence =
                        originals::sign_approval(&substituted_approval.challenge, apple);
                    bad.ordinary_selection.as_mut().unwrap().approval = &substituted_approval;
                }
                4 => bad.state.guard_eq_credential_audit[0] ^= 1,
                5 => bad.state.guard_ep_credential_audit[16] ^= 1,
                6 => bad.state.guard_eq_protocol_digest[0] ^= 1,
                _ => unreachable!(),
            }
            let built = build_recursive_generation_pair_v1(
                &eq_params,
                &ep_params,
                bad.clone(),
                construction,
            );
            let (eq, ep, _, _) = match built {
                Ok(circuits) => circuits,
                Err(error) => {
                    assert_ne!(mutation, 0, "valid complete State construction: {error}");
                    eprintln!(
                        "complete paired State rejected mutation {mutation} at exact claim/shape construction: {error}"
                    );
                    continue;
                }
            };
            let eq_instances =
                recursive_public_instances::<Fp>(&bad.state, bad.eq_successor_history.as_bytes())
                    .unwrap();
            let ep_instances =
                recursive_public_instances::<Fq>(&bad.state, bad.ep_successor_history.as_bytes())
                    .unwrap();
            assert_eq!(
                MockProver::run(16, &eq, vec![eq_instances])
                    .unwrap()
                    .verify()
                    .is_ok(),
                mutation == 0,
                "whole Eq State mutation {mutation}, Apple={apple}"
            );
            drop(eq);
            halo2_proofs::release_allocator_slack();
            assert_eq!(
                MockProver::run(16, &ep, vec![ep_instances])
                    .unwrap()
                    .verify()
                    .is_ok(),
                mutation == 0,
                "whole Ep State mutation {mutation}, Apple={apple}"
            );
            drop(ep);
            halo2_proofs::release_allocator_slack();
        }
    }
}
