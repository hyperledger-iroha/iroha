//! Internal terminal proofs from the genuine Core State diagnostic's retained sender material.
//!
//! Core's persisted candidate identifies the State transport role, including its protocol IDs
//! and deferred audits. The private State carrier has different values in those cells and is
//! independently checked, never relabeled as transport. Test-provider secrets and a structurally
//! sealed certificate are diagnostic inputs, not evidence of an OEM atomic hardware commit.
//! TODO: close the actual State/TerminalAuthorization/CommitWrapper key graph before producing
//! Payment, installing the sender successor, or attempting a qualified ReceiveFold handoff.

use super::*;
use crate::zk::{
    kagemusha_v1_recursion::{
        generation::{
            KagemushaCommitEvidenceOpeningGenerationV1,
            KagemushaGeneratedTerminalAuthorizationArtifactsV1,
            KagemushaGeneratedTerminalAuthorizationProofV1,
            KagemushaLoadedEpTerminalAuthorizationArtifactsV1,
            KagemushaLoadedEqTerminalAuthorizationArtifactsV1,
            KagemushaTerminalAuthorizationEpGenerationWitnessV1,
            KagemushaTerminalAuthorizationEqGenerationWitnessV1,
            KagemushaTerminalAuthorizationGenerationWitnessV1,
            KagemushaTerminalAuthorizationHashClaimGenerationWitnessV1,
            KagemushaTerminalAuthorizationHashClaimParityWitnessV1,
            KagemushaTerminalAuthorizationPrivateGenerationWitnessV1,
            KagemushaTerminalAuthorizationTerminalGenerationPublicV1,
            KagemushaTerminalSendGenerationWitnessV1,
            generate_kagemusha_terminal_authorization_artifacts_v1,
            prove_kagemusha_terminal_authorization_hash_claim_v1,
            prove_kagemusha_terminal_authorization_v1,
        },
        state_relation,
        terminal_authorization::{
            KagemushaTerminalAuthorizationEpCircuitV1, KagemushaTerminalAuthorizationEqCircuitV1,
            KagemushaTerminalAuthorizationPrivateTransitionV1,
            KagemushaTerminalAuthorizationPublicInputsV1, KagemushaTerminalSendPrivateV1,
            TERMINAL_AUTHORIZATION_ENABLED_PROFILE_SLOTS_V1,
            TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1, TerminalSemanticPlanInputsV1,
            TerminalSemanticPlanParityV1, canonical_outbox_reservation_commitment_v1,
            canonical_prepared_transition_binding_digest_v1,
            canonical_terminal_authorization_candidate_digest_v1,
            canonical_terminal_commit_binding_digest_v1, plan_terminal_semantic_sha_v1,
            public_instance,
        },
    },
    kagemusha_v1_state::{
        CommittedOutgoingCandidateV1, PersistedOutgoingCandidateV1, PreparedOutgoingCandidateV1,
    },
};
use halo2_base::utils::fe_to_biguint;
use halo2_proofs::poly::commitment::Params as _;
use iroha_data_model::kagemusha::{KagemushaCommitCertificateV1, kagemusha_ciphertext_digest_v1};

pub(super) fn terminal_public(
    public: &KagemushaTerminalAuthorizationTerminalGenerationPublicV1,
    audits: [DigestV1; 2],
    protocols: [DigestV1; 2],
) -> KagemushaTerminalAuthorizationPublicInputsV1 {
    KagemushaTerminalAuthorizationPublicInputsV1::from_lifecycle(
        &public.lifecycle,
        public.semantic_digest,
        public.candidate_envelope_digest,
        public.commit_certificate_digest,
        public.transition_nullifier,
        public.request_digest,
        public.receiver_binding_digest,
        public.ciphertext_commitment,
        public.amount,
        public.terminal_output_binding,
        audits[0],
        audits[1],
        protocols[0],
        protocols[1],
    )
    .expect("exact Core terminal public projection")
}

fn generation_private(
    private: &KagemushaTerminalAuthorizationPrivateTransitionV1,
) -> KagemushaTerminalAuthorizationPrivateGenerationWitnessV1 {
    let opening = private.commit_evidence_opening;
    KagemushaTerminalAuthorizationPrivateGenerationWitnessV1 {
        lifecycle: private.lifecycle.clone(),
        predecessor: private.predecessor.clone(),
        successor: private.successor.clone(),
        outbox_reservation: private.outbox_reservation,
        commit_certificate: private.commit_certificate.clone(),
        commit_evidence_opening: KagemushaCommitEvidenceOpeningGenerationV1 {
            opening: opening.opening,
            trusted_commit_time_ms: opening.trusted_commit_time_ms,
            lease_id: opening.lease_id,
            lease_valid_from_ms: opening.lease_valid_from_ms,
            lease_expires_at_ms: opening.lease_expires_at_ms,
        },
        one_use_hardware_authorization: private.one_use_hardware_authorization,
        terminal_payload_digest: private.terminal_payload_digest,
        send: private
            .send
            .as_ref()
            .map(|send| KagemushaTerminalSendGenerationWitnessV1 {
                request: send.request.clone(),
                output: send.output.clone(),
                encrypted_credit_digest: send.encrypted_credit_digest,
            }),
        journal_revision_before: private.journal_revision_before,
        journal_revision_after: private.journal_revision_after,
        authorization_counter_before: private.authorization_counter_before,
        authorization_counter_after: private.authorization_counter_after,
        hardware_profile: private.hardware_profile.clone(),
        hardware_credential: private.hardware_credential.clone(),
    }
}

fn candidate_matches_core<F: KagemushaPoseidonFieldV1>(
    expected_semantic: &[F],
    actual: &[F],
    actual_protocol_digest: DigestV1,
    parity: KagemushaPastaParityV1,
) -> Result<(), String> {
    ensure(
        expected_semantic.len() == state_relation::PUBLIC_INSTANCE_COUNT
            && actual.len() == RECURSIVE_PUBLIC_INSTANCE_COUNT,
        "candidate is not the exact State transport column",
    )?;
    ensure(
        actual[..state_relation::PUBLIC_INSTANCE_COUNT] == *expected_semantic,
        "candidate semantic column differs from Core persistence",
    )?;
    let offset = match parity {
        KagemushaPastaParityV1::Eq => state_relation::public_instance::EQ_PROTOCOL_LO,
        KagemushaPastaParityV1::Ep => state_relation::public_instance::EP_PROTOCOL_LO,
    };
    ensure(
        actual[offset..offset + 2] == digest_limbs::<F>(actual_protocol_digest),
        "candidate proof selects another protocol role",
    )
}

pub(super) fn instance_digest<F: KagemushaPoseidonFieldV1>(
    column: &[F],
    offset: usize,
) -> DigestV1 {
    let mut digest = [0; 32];
    for (index, value) in column[offset..offset + 2].iter().enumerate() {
        let bytes = fe_to_biguint(value).to_bytes_le();
        assert!(bytes.len() <= 16, "public digest limb is an exact u128");
        digest[index * 16..index * 16 + bytes.len()].copy_from_slice(&bytes);
    }
    digest
}

#[test]
fn terminal_candidate_preflight_requires_exact_role_and_semantic_cells_in_both_fields() {
    fn check<F: KagemushaPoseidonFieldV1>(parity: KagemushaPastaParityV1) {
        // Only an exact-column comparison fixture: these cells carry no proof, balance, or
        // hardware authority. The separately ignored test exercises actual generated proofs.
        let eq_protocol = digest(b"terminal-role-preflight-eq", 1);
        let ep_protocol = digest(b"terminal-role-preflight-ep", 1);
        let selected = match parity {
            KagemushaPastaParityV1::Eq => eq_protocol,
            KagemushaPastaParityV1::Ep => ep_protocol,
        };
        let mut semantic = vec![F::ZERO; state_relation::PUBLIC_INSTANCE_COUNT];
        for (offset, protocol) in [
            (state_relation::public_instance::EQ_PROTOCOL_LO, eq_protocol),
            (state_relation::public_instance::EP_PROTOCOL_LO, ep_protocol),
        ] {
            semantic[offset..offset + 2].copy_from_slice(&digest_limbs::<F>(protocol));
            assert_eq!(instance_digest(&semantic, offset), protocol);
        }
        let mut complete = semantic.clone();
        complete.resize(RECURSIVE_PUBLIC_INSTANCE_COUNT, F::ZERO);
        candidate_matches_core(&semantic, &complete, selected, parity).unwrap();
        assert!(
            candidate_matches_core(&semantic, &complete[..complete.len() - 1], selected, parity)
                .is_err()
        );
        let mut extended = complete.clone();
        extended.push(F::ZERO);
        assert!(candidate_matches_core(&semantic, &extended, selected, parity).is_err());
        assert!(
            candidate_matches_core(&semantic, &complete, digest(b"other-role", 1), parity).is_err()
        );
        for offset in 0..state_relation::PUBLIC_INSTANCE_COUNT {
            let mut changed = complete.clone();
            changed[offset] += F::ONE;
            assert!(
                candidate_matches_core(&semantic, &changed, selected, parity).is_err(),
                "detached candidate semantic row {offset}"
            );
        }
    }
    check::<Fp>(KagemushaPastaParityV1::Eq);
    check::<Fq>(KagemushaPastaParityV1::Ep);
}

// This decoder is private to the test module. It checks actual generated key bytes and protocols
// but does not create an authenticated catalog, register a release, or call a native admission API.
fn diagnostic_terminal_keys(
    funded: &RealFundedPrerequisite,
    artifacts: KagemushaRecursionArtifactsV1,
    generated: KagemushaGeneratedTerminalAuthorizationArtifactsV1,
) -> (
    KagemushaLoadedEqTerminalAuthorizationArtifactsV1,
    KagemushaLoadedEpTerminalAuthorizationArtifactsV1,
) {
    let mut eq_parameters = Vec::new();
    funded
        .eq
        .write(&mut eq_parameters)
        .expect("original Eq parameters");
    assert_eq!(eq_parameters.as_slice(), generated.eq_parameters.as_ref());
    let mut ep_parameters = Vec::new();
    funded
        .ep
        .write(&mut ep_parameters)
        .expect("original Ep parameters");
    assert_eq!(ep_parameters.as_slice(), generated.ep_parameters.as_ref());
    let lifecycle = &funded.material.authorization_relation.statement.context;
    macro_rules! load {
        ($curve:ty, $circuit:ty, $loaded:ident, $params:expr, $pk:expr, $vk:expr,
         $layout:expr, $digest:expr, $parity:expr) => {{
            let mut vk_cursor = Cursor::new($vk.as_ref());
            let verifying_key = VerifyingKey::<$curve>::read::<_, $circuit>(
                &mut vk_cursor,
                SerdeFormat::Processed,
                $layout.clone(),
            )
            .expect("decode exact generated terminal VK");
            assert_eq!(usize::try_from(vk_cursor.position()).unwrap(), $vk.len());
            let mut pk_cursor = Cursor::new($pk.as_ref());
            let proving_key = ProvingKey::<$curve>::read_structured_v1_checked::<_, $circuit>(
                &mut pk_cursor,
                KAGEMUSHA_RECURSION_IPA_K_V1,
                u64::try_from($pk.len()).expect("generated PK length fits u64"),
                $layout.clone(),
            )
            .expect("decode exact generated terminal PK");
            assert_eq!(usize::try_from(pk_cursor.position()).unwrap(), $pk.len());
            assert_generated_structured_proving_key_bytes_v1(&proving_key, $pk.as_ref());
            assert_eq!(
                proving_key
                    .get_vk()
                    .to_bytes(SerdeFormat::Processed)
                    .as_slice(),
                $vk.as_ref()
            );
            let protocol = compile(
                $params,
                &verifying_key,
                snark_verifier::system::halo2::Config::ipa()
                    .with_num_instance(vec![TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
            );
            assert_eq!(
                native_parent_protocol_digest_v1(&protocol, $parity).unwrap(),
                $digest
            );
            $loaded {
                parameters: (*$params).clone(),
                proving_key,
                verifying_key,
                circuit_params: $layout.clone(),
                protocol_digest: $digest,
                release_id: artifacts.release_id,
                profile_digest: artifacts.profile_digest,
                artifact_manifest_digest: artifacts.artifact_manifest_digest,
                suite_id: lifecycle.suite_id,
                vk_digest: lifecycle.vk_digest,
                eq_claim_protocol_digest: artifacts.mint_hash_claim_eq_protocol_digest,
                ep_claim_protocol_digest: artifacts.mint_hash_claim_ep_protocol_digest,
                eq_shard_protocol_digest: artifacts.mint_hash_shard_eq_protocol_digest,
                ep_shard_protocol_digest: artifacts.mint_hash_shard_ep_protocol_digest,
                enabled_hardware_profiles: generated.enabled_hardware_profiles,
            }
        }};
    }
    let eq = load!(
        EqAffine,
        KagemushaTerminalAuthorizationEqCircuitV1,
        KagemushaLoadedEqTerminalAuthorizationArtifactsV1,
        &funded.eq,
        generated.eq_proving_key,
        generated.eq_verifying_key,
        generated.eq_circuit_params,
        generated.eq_protocol_digest,
        KagemushaPastaParityV1::Eq
    );
    let ep = load!(
        EpAffine,
        KagemushaTerminalAuthorizationEpCircuitV1,
        KagemushaLoadedEpTerminalAuthorizationArtifactsV1,
        &funded.ep,
        generated.ep_proving_key,
        generated.ep_verifying_key,
        generated.ep_circuit_params,
        generated.ep_protocol_digest,
        KagemushaPastaParityV1::Ep
    );
    (eq, ep)
}

/// Retained genuine proof material for the next private diagnostic stage, never a Core capability.
pub(super) struct ProvenSenderTerminalV1 {
    pub(super) public: KagemushaTerminalAuthorizationTerminalGenerationPublicV1,
    pub(super) eq_keys: KagemushaLoadedEqTerminalAuthorizationArtifactsV1,
    pub(super) ep_keys: KagemushaLoadedEpTerminalAuthorizationArtifactsV1,
    pub(super) proof: KagemushaGeneratedTerminalAuthorizationProofV1,
    pub(super) committed: CommittedOutgoingCandidateV1,
}

pub(super) fn prove_sender_terminal(
    funded: &RealFundedPrerequisite,
    state_keys: &StateKeys,
    guard_keys: &mut Option<GuardKeys>,
    verifier: &DiagnosticVerifier<'_>,
    artifacts: KagemushaRecursionArtifactsV1,
    candidate: PreparedOutgoingCandidateV1,
    send: &KagemushaGeneratedRecursiveStateProofV1,
    send_guard: &GuardProof,
    preparation: &SendSplitPreparationV1,
    openings: &DiagnosticSenderOpeningsV1,
) -> ProvenSenderTerminalV1 {
    let started = std::time::Instant::now();
    openings
        .validate_preparation(preparation)
        .expect("original one-use sender preparation");
    // The existing helper rechecks the INNER equations and both actual transport proofs. Also
    // decide the retained INNER claims directly and reproduce their original deterministic folds.
    terminally_verify_state_proof(state_keys, send);
    let seed = test_only_recovery_seed();
    assert_eq!(
        &send.eq_public_instances[state_relation::PUBLIC_INSTANCE_COUNT..],
        history_values::<Fp>(send.eq_history.as_bytes())
    );
    assert_eq!(
        &send.ep_public_instances[state_relation::PUBLIC_INSTANCE_COUNT..],
        history_values::<Fq>(send.ep_history.as_bytes())
    );
    for inner in [&send.eq_current_accumulator, &send.eq_history] {
        decide_kagemusha_eq_accumulator_v1(&funded.eq, inner).unwrap();
    }
    for inner in [&send.ep_current_accumulator, &send.ep_history] {
        decide_kagemusha_ep_accumulator_v1(&funded.ep, inner).unwrap();
    }
    let eq_retained_inner_fold = fold_kagemusha_eq_accumulators_v1(
        &funded.eq,
        &send.eq_current_accumulator,
        &send.eq_history,
        &seed,
    )
    .unwrap();
    let ep_retained_inner_fold = fold_kagemusha_ep_accumulators_v1(
        &funded.ep,
        &send.ep_current_accumulator,
        &send.ep_history,
        &seed,
    )
    .unwrap();
    assert_eq!(
        eq_retained_inner_fold.successor().as_bytes().as_slice(),
        send.proof.eq_history.as_slice()
    );
    assert_eq!(
        ep_retained_inner_fold.successor().as_bytes().as_slice(),
        send.proof.ep_history.as_slice()
    );
    let core_public = candidate
        .candidate_public_inputs(artifacts, &send.proof)
        .unwrap();
    let eq_expected = core_public.public_instances::<Fp>().unwrap();
    let ep_expected = core_public.public_instances::<Fq>().unwrap();
    // StateKeys.eq_protocol/ep_protocol name INNER keys; select their explicit transport
    // counterparts here. GeneratedState's top-level instances/current/history are also INNER;
    // the transport columns and send.proof bytes/history below are the exact Core candidate.
    let eq_candidate_transport_protocol_digest = native_parent_protocol_digest_v1(
        &state_keys.eq_transport_protocol,
        KagemushaPastaParityV1::Eq,
    )
    .unwrap();
    let ep_candidate_transport_protocol_digest = native_parent_protocol_digest_v1(
        &state_keys.ep_transport_protocol,
        KagemushaPastaParityV1::Ep,
    )
    .unwrap();
    macro_rules! check_role {
        ($field:ty, $expected:expr, $transport:expr, $inner:expr, $digest:expr, $inner_protocol:expr, $parity:expr) => {{
            candidate_matches_core::<$field>($expected, $transport, $digest, $parity)
                .expect("exact persisted Core transport projection");
            assert!(candidate_matches_core::<$field>($expected, $inner, $digest, $parity).is_err());
            let inner_digest = native_parent_protocol_digest_v1($inner_protocol, $parity).unwrap();
            assert_ne!(inner_digest, $digest);
            assert!(
                candidate_matches_core::<$field>($expected, $transport, inner_digest, $parity)
                    .is_err()
            );
            let mut changed = $transport.clone();
            changed[state_relation::public_instance::AMOUNT] += <$field>::ONE;
            assert!(
                candidate_matches_core::<$field>($expected, &changed, $digest, $parity).is_err()
            );
        }};
    }
    check_role!(
        Fp,
        &eq_expected,
        &send.eq_transport_public_instances,
        &send.eq_public_instances,
        eq_candidate_transport_protocol_digest,
        &state_keys.eq_protocol,
        KagemushaPastaParityV1::Eq
    );
    check_role!(
        Fq,
        &ep_expected,
        &send.ep_transport_public_instances,
        &send.ep_public_instances,
        ep_candidate_transport_protocol_digest,
        &state_keys.ep_protocol,
        KagemushaPastaParityV1::Ep
    );

    let persisted = PersistedOutgoingCandidateV1::verify_and_persist_send(
        candidate.clone(),
        send.proof.clone(),
        artifacts,
        verifier,
    )
    .expect("persist only the genuinely verified Core SendSplit candidate");
    let body = persisted
        .hardware_terminal_body()
        .expect("Core derives exact self-free terminal body");
    assert_eq!(body.commit_evidence, openings.commit_evidence().unwrap());
    assert_eq!(
        body.private_successor_commitment,
        candidate.successor_state.state_commitment
    );
    for candidate_digest in [
        canonical_terminal_authorization_candidate_digest_v1(&[eq_expected]).unwrap(),
        canonical_terminal_authorization_candidate_digest_v1(&[ep_expected]).unwrap(),
    ] {
        assert_eq!(candidate_digest, body.candidate_envelope_digest);
    }
    // A test-only structural certificate over the exact Core body supplies circuit inputs. No
    // callback claims OEM finality, and no state-machine commit or monetary finalizer is invoked.
    let certificate = KagemushaCommitCertificateV1 {
        version: body.version,
        certificate_id: [0; 32],
        candidate_envelope_digest: body.candidate_envelope_digest,
        lifecycle_binding_digest: body.lifecycle_binding_digest,
        transition_nullifier: body.transition_nullifier,
        outbox_reservation_commitment: body.outbox_reservation_commitment,
        commit_evidence: body.commit_evidence,
        hardware_profile_id: body.hardware_profile_id,
        policy_epoch: body.policy_epoch,
        hardware_terminal_commitment: [0; 32],
    }
    .seal_with_terminal_body(&body)
    .expect("seal exact diagnostic terminal transcript");
    let mut changed_certificate = certificate.clone();
    changed_certificate.candidate_envelope_digest[0] ^= 1;
    changed_certificate = changed_certificate.seal_certificate_id().unwrap();
    assert!(
        CommittedOutgoingCandidateV1::from_hardware_commit(persisted.clone(), changed_certificate)
            .is_err()
    );
    let committed =
        CommittedOutgoingCandidateV1::from_hardware_commit(persisted, certificate.clone())
            .expect("Core checks exact candidate/body/certificate correspondence");
    let output = committed
        .public_output()
        .expect("Core derives terminal public output");
    let public = KagemushaTerminalAuthorizationTerminalGenerationPublicV1 {
        lifecycle: output.lifecycle,
        semantic_digest: output.semantic_digest,
        candidate_envelope_digest: output.candidate_envelope_digest,
        commit_certificate_digest: output.commit_certificate_digest,
        transition_nullifier: output.transition_nullifier,
        request_digest: output.request_digest,
        receiver_binding_digest: output.receiver_binding_digest,
        ciphertext_commitment: output.ciphertext_commitment,
        amount: output.amount,
        terminal_output_binding: output.terminal_output_binding,
    };
    let PreparedOutgoingRecoveryViewV1::Send {
        request,
        output,
        encrypted_credit,
        ..
    } = candidate.recovery_view()
    else {
        panic!("retain exact Core SendSplit recovery projection");
    };
    let private = KagemushaTerminalAuthorizationPrivateTransitionV1 {
        lifecycle: public.lifecycle.clone(),
        predecessor: candidate.predecessor_state.clone(),
        successor: candidate.successor_state.clone(),
        outbox_reservation: candidate.outbox_reservation,
        commit_certificate: certificate,
        commit_evidence_opening: openings.commit_evidence_opening,
        one_use_hardware_authorization: openings.one_use_hardware_authorization,
        terminal_payload_digest: public.semantic_digest,
        send: Some(KagemushaTerminalSendPrivateV1 {
            request: request.clone(),
            output: output.clone(),
            encrypted_credit_digest: kagemusha_ciphertext_digest_v1(encrypted_credit),
        }),
        journal_revision_before: openings.journal_revision_before,
        journal_revision_after: openings.journal_revision_after,
        authorization_counter_before: openings.authorization_counter_before,
        authorization_counter_after: openings.authorization_counter_after,
        hardware_profile: funded.material.hardware_profile.clone(),
        hardware_credential: funded.material.hardware_credential.clone(),
    };
    // Setup placeholders are used solely for host validation and the no-cycle terminal binding;
    // generation derives actual reciprocal audits and actual PK/VK identities before proving.
    let setup_public = terminal_public(
        &public,
        [encode_pasta(Fp::from(3)), encode_pasta(Fq::from(4))],
        [encode_pasta(Fp::from(1)), encode_pasta(Fq::from(2))],
    );
    private
        .validate_against(&setup_public)
        .expect("original credential/profile/time/private sender bindings");
    let mut changed_private = private.clone();
    changed_private
        .commit_evidence_opening
        .trusted_commit_time_ms += 1;
    assert!(changed_private.validate_against(&setup_public).is_err());
    changed_private = private.clone();
    changed_private.authorization_counter_before += 1;
    changed_private.authorization_counter_after += 1;
    assert!(changed_private.validate_against(&setup_public).is_err());
    changed_private = private.clone();
    changed_private.terminal_payload_digest[0] ^= 1;
    assert!(changed_private.validate_against(&setup_public).is_err());

    let prepared_authorization = openings.prepared_authorization_digest();
    let prepared_transition = canonical_prepared_transition_binding_digest_v1(
        setup_public.lifecycle_binding_digest,
        public.request_digest,
        output.sender_before_commitment,
        output.sender_after_commitment,
        public.amount,
        canonical_outbox_reservation_commitment_v1(private.outbox_reservation).unwrap(),
        prepared_authorization,
    );
    let mut relation = send_guard.relation.clone();
    assert_eq!(
        relation.statement.prepared_transition_binding_digest,
        prepared_transition
    );
    relation.statement.sender_one_time_authorization_digest = prepared_authorization;
    relation.statement.terminal_commit_binding_digest =
        canonical_terminal_commit_binding_digest_v1(
            &setup_public,
            &private,
            prepared_transition,
            prepared_authorization,
            relation.statement.transition_intent_digest,
            relation.statement.transition_effect_digest,
            relation.statement.recovery_record_digest,
            relation.statement.durable_inbox_effect_digest,
            relation.statement.durable_outbox_effect_digest,
        )
        .unwrap();
    relation
        .validate()
        .expect("postcommit Guard retains original sender context");
    assert_eq!(
        relation.credential_digests(),
        send_guard.relation.credential_digests()
    );
    assert_eq!(
        relation.predecessor_credential.credential_issuance_digest,
        private.hardware_credential.credential_id
    );
    assert_eq!(
        relation.successor_credential.credential_issuance_digest,
        private.hardware_credential.credential_id
    );
    let original_guard_protocols = {
        let keys = guard_keys.as_ref().expect("original Guard keys");
        [keys.eq_protocol_digest, keys.ep_protocol_digest]
    };
    let terminal_guard = prove_guard(
        &funded.eq,
        &funded.ep,
        &funded.credential_keys,
        guard_keys,
        relation,
        &funded.credential,
        &funded.credential,
    );
    let guard_keys = guard_keys.as_ref().expect("same postcommit Guard keys");
    assert_eq!(
        original_guard_protocols,
        [guard_keys.eq_protocol_digest, guard_keys.ep_protocol_digest]
    );
    assert_eq!(
        guard_keys.provider_policy_root,
        funded
            .material
            .platform_credential
            .statement
            .hardware_policy_id
    );
    let eq_guard_column = guard_public_instances::<Fp>(
        &terminal_guard.relation,
        terminal_guard.eq_credential_audit,
        terminal_guard.ep_credential_audit,
        terminal_guard.eq_history.as_bytes(),
    );
    let ep_guard_column = guard_public_instances::<Fq>(
        &terminal_guard.relation,
        terminal_guard.eq_credential_audit,
        terminal_guard.ep_credential_audit,
        terminal_guard.ep_history.as_bytes(),
    );
    assert_eq!(
        decide_eq(
            &funded.eq,
            &guard_keys.eq_protocol,
            &terminal_guard.eq_proof,
            &eq_guard_column,
            &terminal_guard.eq_history
        )
        .unwrap(),
        terminal_guard.eq_current
    );
    assert_eq!(
        decide_ep(
            &funded.ep,
            &guard_keys.ep_protocol,
            &terminal_guard.ep_proof,
            &ep_guard_column,
            &terminal_guard.ep_history
        )
        .unwrap(),
        terminal_guard.ep_current
    );

    let eq_candidate_transport_history =
        KagemushaEqAccumulatorV1::try_from_bytes(&send.proof.eq_history).unwrap();
    let ep_candidate_transport_history =
        KagemushaEpAccumulatorV1::try_from_bytes(&send.proof.ep_history).unwrap();
    let eq_candidate_transport_current = decide_eq(
        &funded.eq,
        &state_keys.eq_transport_protocol,
        &send.proof.eq_proof,
        &send.eq_transport_public_instances,
        &eq_candidate_transport_history,
    )
    .unwrap();
    let ep_candidate_transport_current = decide_ep(
        &funded.ep,
        &state_keys.ep_transport_protocol,
        &send.proof.ep_proof,
        &send.ep_transport_public_instances,
        &ep_candidate_transport_history,
    )
    .unwrap();
    // These are new folds of TRANSPORT current + TRANSPORTED history. State generation's
    // existing folds completed INNER current + INNER history and cannot substitute here.
    let eq_candidate_fold = fold_kagemusha_eq_accumulators_v1(
        &funded.eq,
        &eq_candidate_transport_current,
        &eq_candidate_transport_history,
        &seed,
    )
    .unwrap();
    let ep_candidate_fold = fold_kagemusha_ep_accumulators_v1(
        &funded.ep,
        &ep_candidate_transport_current,
        &ep_candidate_transport_history,
        &seed,
    )
    .unwrap();
    let eq_guard_fold = fold_kagemusha_eq_accumulators_v1(
        &funded.eq,
        &terminal_guard.eq_current,
        &terminal_guard.eq_history,
        &seed,
    )
    .unwrap();
    let ep_guard_fold = fold_kagemusha_ep_accumulators_v1(
        &funded.ep,
        &terminal_guard.ep_current,
        &terminal_guard.ep_history,
        &seed,
    )
    .unwrap();
    let eq_merge = fold_kagemusha_eq_accumulators_v1(
        &funded.eq,
        eq_candidate_fold.successor(),
        eq_guard_fold.successor(),
        &seed,
    )
    .unwrap();
    let ep_merge = fold_kagemusha_ep_accumulators_v1(
        &funded.ep,
        ep_candidate_fold.successor(),
        ep_guard_fold.successor(),
        &seed,
    )
    .unwrap();
    for history in [
        eq_candidate_fold.successor(),
        eq_guard_fold.successor(),
        eq_merge.successor(),
    ] {
        decide_kagemusha_eq_accumulator_v1(&funded.eq, history).unwrap();
    }
    for history in [
        ep_candidate_fold.successor(),
        ep_guard_fold.successor(),
        ep_merge.successor(),
    ] {
        decide_kagemusha_ep_accumulator_v1(&funded.ep, history).unwrap();
    }
    let eq_candidates = [send.eq_transport_public_instances.clone()];
    let ep_candidates = [send.ep_transport_public_instances.clone()];
    let eq_guards = [eq_guard_column];
    let ep_guards = [ep_guard_column];
    let mut enabled = [[0; 32]; TERMINAL_AUTHORIZATION_ENABLED_PROFILE_SLOTS_V1];
    enabled[0] = funded.material.hardware_profile.hardware_profile_id;
    let semantic_plan = plan_terminal_semantic_sha_v1(TerminalSemanticPlanInputsV1 {
        public: &setup_public,
        private_transition: &private,
        terminal_guard_relation: &terminal_guard.relation,
        enabled_hardware_profiles: &enabled,
        eq: TerminalSemanticPlanParityV1 {
            candidate_protocol: &state_keys.eq_transport_protocol,
            candidate_instances: &eq_candidates,
            terminal_guard_protocol: &guard_keys.eq_protocol,
            terminal_guard_instances: &eq_guards,
            successor_history: eq_merge.successor().as_bytes(),
        },
        ep: TerminalSemanticPlanParityV1 {
            candidate_protocol: &state_keys.ep_transport_protocol,
            candidate_instances: &ep_candidates,
            terminal_guard_protocol: &guard_keys.ep_protocol,
            terminal_guard_instances: &ep_guards,
            successor_history: ep_merge.successor().as_bytes(),
        },
    })
    .expect("capture complete original Terminal semantic SHA queues without proof verification");
    assert_eq!(semantic_plan.eq_messages.len(), 26);
    assert_eq!(semantic_plan.ep_messages.len(), 26);
    assert_eq!(semantic_plan.job_block_counts.len(), 26);
    for messages in [&semantic_plan.eq_messages, &semantic_plan.ep_messages] {
        assert!(
            messages
                .last()
                .expect("candidate is the final Terminal hash")
                .starts_with(b"iroha:kagemusha:v1:terminal-authorization-candidate\0")
        );
    }
    // Retain only ephemeral planning messages. The typed producer below proves this complete
    // queue, and Terminal verifies that claim against its original assigned semantic cells.
    eprintln!(
        "KAGEMUSHA terminal diagnostic: complete paired SHA queue has {} jobs and {} compression blocks per parity",
        semantic_plan.job_block_counts.len(),
        semantic_plan
            .job_block_counts
            .iter()
            .map(|count| u64::from(*count))
            .sum::<u64>()
    );
    drop(semantic_plan);
    let generation_private = generation_private(&private);
    let hash_claim = prove_kagemusha_terminal_authorization_hash_claim_v1(
        &funded.hash_eq,
        &funded.hash_ep,
        KagemushaTerminalAuthorizationHashClaimGenerationWitnessV1 {
            public: &public,
            private_transition: &generation_private,
            terminal_guard_relation: &terminal_guard.relation,
            enabled_hardware_profiles: &enabled,
            eq: KagemushaTerminalAuthorizationHashClaimParityWitnessV1 {
                candidate_protocol: &state_keys.eq_transport_protocol,
                candidate_instances: &eq_candidates,
                terminal_guard_protocol: &guard_keys.eq_protocol,
                terminal_guard_instances: &eq_guards,
            },
            ep: KagemushaTerminalAuthorizationHashClaimParityWitnessV1 {
                candidate_protocol: &state_keys.ep_transport_protocol,
                candidate_instances: &ep_candidates,
                terminal_guard_protocol: &guard_keys.ep_protocol,
                terminal_guard_instances: &ep_guards,
            },
        },
        &seed,
    )
    .expect("genuine complete Terminal SHA claim under the existing authenticated helper suite");
    let eq_claim_merge = fold_kagemusha_eq_accumulators_v1(
        &funded.eq,
        eq_merge.successor(),
        &hash_claim.eq_complete_history,
        &seed,
    )
    .expect("retain candidate, Guard and complete Eq SHA ancestry");
    let ep_claim_merge = fold_kagemusha_ep_accumulators_v1(
        &funded.ep,
        ep_merge.successor(),
        &hash_claim.ep_complete_history,
        &seed,
    )
    .expect("retain candidate, Guard and complete Ep SHA ancestry");
    decide_kagemusha_eq_accumulator_v1(&funded.eq, eq_claim_merge.successor()).unwrap();
    decide_kagemusha_ep_accumulator_v1(&funded.ep, ep_claim_merge.successor()).unwrap();
    let hash_claim_witness = hash_claim
        .consumer_witness(
            &funded.hash_eq,
            &funded.hash_ep,
            eq_claim_merge.proof(),
            ep_claim_merge.proof(),
        )
        .expect("exact loaded claim protocols and original complete history folds");
    let witness = KagemushaTerminalAuthorizationGenerationWitnessV1 {
        public: public.clone(),
        private_transition: generation_private,
        terminal_guard_relation: terminal_guard.relation.clone(),
        enabled_hardware_profiles: enabled,
        hash_claim: hash_claim_witness,
        eq: KagemushaTerminalAuthorizationEqGenerationWitnessV1 {
            candidate_protocol: &state_keys.eq_transport_protocol,
            candidate_instances: &eq_candidates,
            candidate_proof: &send.proof.eq_proof,
            candidate_history: &eq_candidate_transport_history,
            candidate_history_fold_proof: eq_candidate_fold.proof(),
            terminal_guard_protocol: &guard_keys.eq_protocol,
            terminal_guard_instances: &eq_guards,
            terminal_guard_proof: &terminal_guard.eq_proof,
            terminal_guard_history: &terminal_guard.eq_history,
            terminal_guard_history_fold_proof: eq_guard_fold.proof(),
            merge_fold_proof: eq_merge.proof(),
            successor_history: eq_claim_merge.successor(),
        },
        ep: KagemushaTerminalAuthorizationEpGenerationWitnessV1 {
            candidate_protocol: &state_keys.ep_transport_protocol,
            candidate_instances: &ep_candidates,
            candidate_proof: &send.proof.ep_proof,
            candidate_history: &ep_candidate_transport_history,
            candidate_history_fold_proof: ep_candidate_fold.proof(),
            terminal_guard_protocol: &guard_keys.ep_protocol,
            terminal_guard_instances: &ep_guards,
            terminal_guard_proof: &terminal_guard.ep_proof,
            terminal_guard_history: &terminal_guard.ep_history,
            terminal_guard_history_fold_proof: ep_guard_fold.proof(),
            merge_fold_proof: ep_merge.proof(),
            successor_history: ep_claim_merge.successor(),
        },
    };
    eprintln!(
        "KAGEMUSHA terminal diagnostic: actual transport candidate, terminal Guard44, complete SHA claim and all history folds verified; generating dedicated terminal keys"
    );
    let generated = generate_kagemusha_terminal_authorization_artifacts_v1(witness.clone())
        .expect("actual terminal key generation/profile capacity and key stability");
    let (eq_keys, ep_keys) = diagnostic_terminal_keys(funded, artifacts, generated);
    let proof = prove_kagemusha_terminal_authorization_v1(&eq_keys, &ep_keys, witness, &seed)
        .expect("actual paired TerminalAuthorization proof over retained Core candidate");
    assert_eq!(&proof.eq_history, eq_claim_merge.successor());
    assert_eq!(&proof.ep_history, ep_claim_merge.successor());
    let eq_audits = [
        instance_digest(
            &proof.eq_public_instances,
            public_instance::EQ_DEFERRED_AUDIT_LO,
        ),
        instance_digest(
            &proof.eq_public_instances,
            public_instance::EP_DEFERRED_AUDIT_LO,
        ),
    ];
    let ep_audits = [
        instance_digest(
            &proof.ep_public_instances,
            public_instance::EQ_DEFERRED_AUDIT_LO,
        ),
        instance_digest(
            &proof.ep_public_instances,
            public_instance::EP_DEFERRED_AUDIT_LO,
        ),
    ];
    assert_eq!(eq_audits, ep_audits);
    let exact_public = terminal_public(
        &public,
        eq_audits,
        [eq_keys.protocol_digest, ep_keys.protocol_digest],
    );
    macro_rules! verify_terminal {
        ($field:ty, $keys:expr, $column:expr, $bytes:expr, $history:expr, $current:expr, $decide:ident) => {{
            let protocol = compile(
                &$keys.parameters,
                &$keys.verifying_key,
                snark_verifier::system::halo2::Config::ipa()
                    .with_num_instance(vec![TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
            );
            let mut expected = exact_public.public_prefix::<$field>().unwrap();
            expected.extend(history_values::<$field>($history.as_bytes()));
            assert_eq!(&$column, &expected);
            assert_eq!(
                $decide(&$keys.parameters, &protocol, &$bytes, &$column, &$history).unwrap(),
                $current
            );
            // Preserve real proof bytes while substituting candidate/certificate/value, both
            // reciprocal audits, either protocol identity, and an exact transported history limb.
            for offset in [
                public_instance::CANDIDATE_LO,
                public_instance::COMMIT_CERTIFICATE_LO,
                public_instance::AMOUNT,
                public_instance::EQ_DEFERRED_AUDIT_LO,
                public_instance::EP_DEFERRED_AUDIT_LO,
                public_instance::EQ_PROTOCOL_LO,
                public_instance::EP_PROTOCOL_LO,
                public_instance::HISTORY_START,
            ] {
                let mut changed = $column.clone();
                changed[offset] += <$field>::ONE;
                assert!(
                    $decide(&$keys.parameters, &protocol, &$bytes, &changed, &$history).is_err(),
                    "real terminal proof accepted substituted public row {offset}"
                );
            }
        }};
    }
    verify_terminal!(
        Fp,
        eq_keys,
        proof.eq_public_instances,
        proof.eq_proof,
        proof.eq_history,
        proof.eq_current_accumulator,
        decide_eq
    );
    verify_terminal!(
        Fq,
        ep_keys,
        proof.ep_public_instances,
        proof.ep_proof,
        proof.ep_history,
        proof.ep_current_accumulator,
        decide_ep
    );
    openings
        .validate_preparation(preparation)
        .expect("original private sender opening remains unchanged");
    eprintln!(
        "KAGEMUSHA diagnostic actual paired TerminalAuthorization verified and decided in {:?}; no CommitWrapper, Payment, hardware finality, or handoff qualification",
        started.elapsed()
    );
    ProvenSenderTerminalV1 {
        public,
        eq_keys,
        ep_keys,
        proof,
        committed,
    }
}

#[path = "terminal_semantic_fixture.rs"]
mod terminal_semantic_fixture;
