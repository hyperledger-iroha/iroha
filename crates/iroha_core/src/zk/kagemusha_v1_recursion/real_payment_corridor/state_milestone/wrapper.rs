//! Genuine CommitWrapper proof and protocol seed for the next diagnostic graph-closure round.
//!
//! This stage consumes the retained real terminal pair, never dummy active proof material. It
//! reports the full compiled wrapper structure against the incoming State seed without changing
//! the candidate's already-bound wrapper identities. No release is admitted, no Core journal is
//! advanced, and the native monetary gates remain closed.
//! TODO: converge the incoming wrapper structures and re-prove the complete chain under final
//! identities before attempting Core payment finalization or a ReceiveFold handoff.

use super::terminal::{ProvenSenderTerminalV1, instance_digest, terminal_public};
use super::*;
use crate::zk::kagemusha_v1_recursion::{
    deferred_parent::kagemusha_protocol_structure_digest_v1,
    generation::{
        KagemushaCommitWrapperEpGenerationWitnessV1, KagemushaCommitWrapperEqGenerationWitnessV1,
        KagemushaCommitWrapperGenerationWitnessV1, KagemushaGeneratedCommitWrapperArtifactsV1,
        KagemushaLoadedEpCommitWrapperArtifactsV1, KagemushaLoadedEqCommitWrapperArtifactsV1,
        KagemushaTerminalAuthorizationTerminalGenerationPublicV1,
        generate_kagemusha_commit_wrapper_artifacts_v1, prove_kagemusha_commit_wrapper_v1,
    },
    terminal_authorization::{
        KagemushaCommitWrapperEpCircuitV1, KagemushaCommitWrapperEqCircuitV1,
        TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1, public_instance,
    },
};
use halo2_proofs::poly::commitment::Params as _;
use iroha_data_model::kagemusha::{KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1, KagemushaPaymentProofV1};

fn validate_wrapper_projection<F: KagemushaPoseidonFieldV1>(
    terminal_column: &[F],
    wrapper_column: &[F],
    terminal_protocols: [DigestV1; 2],
    wrapper_protocols: [DigestV1; 2],
) -> Result<(), String> {
    ensure(
        terminal_column.len() == TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1
            && wrapper_column.len() == TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1,
        "wrapper projection requires both exact 81-cell columns",
    )?;
    ensure(
        terminal_column[..public_instance::EQ_DEFERRED_AUDIT_LO]
            == wrapper_column[..public_instance::EQ_DEFERRED_AUDIT_LO],
        "wrapper changed the terminal semantic projection",
    )?;
    ensure(
        !terminal_protocols.contains(&[0; 32])
            && !wrapper_protocols.contains(&[0; 32])
            && terminal_protocols[0] != terminal_protocols[1]
            && wrapper_protocols[0] != wrapper_protocols[1]
            && terminal_protocols[0] != wrapper_protocols[0]
            && terminal_protocols[1] != wrapper_protocols[1],
        "terminal and wrapper protocol roles alias or are absent",
    )?;
    for (index, offset) in [
        public_instance::EQ_PROTOCOL_LO,
        public_instance::EP_PROTOCOL_LO,
    ]
    .into_iter()
    .enumerate()
    {
        ensure(
            terminal_column[offset..offset + 2] == digest_limbs::<F>(terminal_protocols[index])
                && wrapper_column[offset..offset + 2]
                    == digest_limbs::<F>(wrapper_protocols[index]),
            "wrapper or nested terminal selects another protocol role",
        )?;
    }
    // Audits and histories belong to their respective proofs. The caller verifies the full
    // columns, independently decides both histories, and checks the actual new history fold.
    Ok(())
}

#[test]
fn wrapper_projection_preflight_binds_semantics_and_distinct_roles_in_both_fields() {
    fn check<F: KagemushaPoseidonFieldV1>() {
        // Exact-column comparison only: these synthetic cells carry no proof or wallet state.
        let terminal_protocols = [
            digest(b"terminal-eq-role", 1),
            digest(b"terminal-ep-role", 1),
        ];
        let wrapper_protocols = [digest(b"wrapper-eq-role", 1), digest(b"wrapper-ep-role", 1)];
        let mut terminal = vec![F::ZERO; TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1];
        let mut wrapper = terminal.clone();
        for (index, offset) in [
            public_instance::EQ_PROTOCOL_LO,
            public_instance::EP_PROTOCOL_LO,
        ]
        .into_iter()
        .enumerate()
        {
            terminal[offset..offset + 2]
                .copy_from_slice(&digest_limbs::<F>(terminal_protocols[index]));
            wrapper[offset..offset + 2]
                .copy_from_slice(&digest_limbs::<F>(wrapper_protocols[index]));
        }
        validate_wrapper_projection(&terminal, &wrapper, terminal_protocols, wrapper_protocols)
            .unwrap();
        assert!(
            validate_wrapper_projection(
                &terminal,
                &wrapper[..80],
                terminal_protocols,
                wrapper_protocols
            )
            .is_err()
        );
        assert!(
            validate_wrapper_projection(
                &terminal[..80],
                &wrapper,
                terminal_protocols,
                wrapper_protocols
            )
            .is_err()
        );
        let mut extended = wrapper.clone();
        extended.push(F::ZERO);
        assert!(
            validate_wrapper_projection(
                &terminal,
                &extended,
                terminal_protocols,
                wrapper_protocols
            )
            .is_err()
        );
        assert!(
            validate_wrapper_projection(&terminal, &wrapper, wrapper_protocols, terminal_protocols)
                .is_err()
        );
        assert!(
            validate_wrapper_projection(
                &terminal,
                &terminal,
                terminal_protocols,
                terminal_protocols
            )
            .is_err()
        );
        for offset in 0..public_instance::EQ_DEFERRED_AUDIT_LO {
            let mut changed = wrapper.clone();
            changed[offset] += F::ONE;
            assert!(
                validate_wrapper_projection(
                    &terminal,
                    &changed,
                    terminal_protocols,
                    wrapper_protocols
                )
                .is_err(),
                "wrapper changed semantic row {offset}"
            );
        }
        for offset in public_instance::EQ_PROTOCOL_LO..public_instance::HISTORY_START {
            let mut changed = wrapper.clone();
            changed[offset] += F::ONE;
            assert!(
                validate_wrapper_projection(
                    &terminal,
                    &changed,
                    terminal_protocols,
                    wrapper_protocols
                )
                .is_err()
            );
            let mut changed = terminal.clone();
            changed[offset] += F::ONE;
            assert!(
                validate_wrapper_projection(
                    &changed,
                    &wrapper,
                    terminal_protocols,
                    wrapper_protocols
                )
                .is_err()
            );
        }
    }
    check::<Fp>();
    check::<Fq>();
}

// Exact generated bytes are decoded only inside this private diagnostic. No authenticated
// artifact-set loader or release catalog is synthesized from these fixture metadata fields.
fn diagnostic_wrapper_keys(
    funded: &RealFundedPrerequisite,
    terminal: &ProvenSenderTerminalV1,
    generated: KagemushaGeneratedCommitWrapperArtifactsV1,
) -> (
    KagemushaLoadedEqCommitWrapperArtifactsV1,
    KagemushaLoadedEpCommitWrapperArtifactsV1,
) {
    let mut eq_parameters = Vec::new();
    funded.eq.write(&mut eq_parameters).unwrap();
    assert_eq!(eq_parameters.as_slice(), generated.eq_parameters.as_ref());
    let mut ep_parameters = Vec::new();
    funded.ep.write(&mut ep_parameters).unwrap();
    assert_eq!(ep_parameters.as_slice(), generated.ep_parameters.as_ref());
    assert_eq!(
        generated.enabled_hardware_profiles,
        terminal.eq_keys.enabled_hardware_profiles
    );
    assert_eq!(
        generated.enabled_hardware_profiles,
        terminal.ep_keys.enabled_hardware_profiles
    );
    macro_rules! load {
        ($curve:ty, $circuit:ty, $loaded:ident, $params:expr, $nested:expr, $pk:expr, $vk:expr,
         $layout:expr, $digest:expr, $nested_digest:expr, $parity:expr) => {{
            let mut vk_cursor = Cursor::new($vk.as_ref());
            let verifying_key = VerifyingKey::<$curve>::read::<_, $circuit>(
                &mut vk_cursor,
                SerdeFormat::Processed,
                $layout.clone(),
            )
            .expect("decode exact generated wrapper VK");
            assert_eq!(usize::try_from(vk_cursor.position()).unwrap(), $vk.len());
            let mut pk_cursor = Cursor::new($pk.as_ref());
            let proving_key = ProvingKey::<$curve>::read::<_, $circuit>(
                &mut pk_cursor,
                SerdeFormat::Processed,
                $layout.clone(),
            )
            .expect("decode exact generated wrapper PK");
            assert_eq!(usize::try_from(pk_cursor.position()).unwrap(), $pk.len());
            assert_eq!(
                proving_key
                    .get_vk()
                    .to_bytes(SerdeFormat::Processed)
                    .as_slice(),
                $vk.as_ref()
            );
            assert_ne!(
                $vk.as_ref(),
                $nested
                    .verifying_key
                    .to_bytes(SerdeFormat::Processed)
                    .as_slice()
            );
            assert_eq!($nested_digest, $nested.protocol_digest);
            assert_ne!($digest, $nested_digest);
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
            let profile = ordinary_ipa_proof_profile_v1(&protocol).unwrap();
            assert!(profile.byte_len <= KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1);
            $loaded {
                parameters: (*$params).clone(),
                proving_key,
                verifying_key,
                circuit_params: $layout.clone(),
                protocol_digest: $digest,
                terminal_authorization_protocol_digest: $nested_digest,
                release_id: $nested.release_id,
                profile_digest: $nested.profile_digest,
                artifact_manifest_digest: $nested.artifact_manifest_digest,
                suite_id: $nested.suite_id,
                vk_digest: $nested.vk_digest,
                enabled_hardware_profiles: generated.enabled_hardware_profiles,
            }
        }};
    }
    let eq = load!(
        EqAffine,
        KagemushaCommitWrapperEqCircuitV1,
        KagemushaLoadedEqCommitWrapperArtifactsV1,
        &funded.eq,
        &terminal.eq_keys,
        generated.eq_proving_key,
        generated.eq_verifying_key,
        generated.eq_circuit_params,
        generated.eq_protocol_digest,
        generated.terminal_authorization_eq_protocol_digest,
        KagemushaPastaParityV1::Eq
    );
    let ep = load!(
        EpAffine,
        KagemushaCommitWrapperEpCircuitV1,
        KagemushaLoadedEpCommitWrapperArtifactsV1,
        &funded.ep,
        &terminal.ep_keys,
        generated.ep_proving_key,
        generated.ep_verifying_key,
        generated.ep_circuit_params,
        generated.ep_protocol_digest,
        generated.terminal_authorization_ep_protocol_digest,
        KagemushaPastaParityV1::Ep
    );
    (eq, ep)
}

/// Return actual proved wrapper protocols as the next nonauthorizing graph-construction seed.
pub(super) fn prove_sender_wrapper(
    funded: &RealFundedPrerequisite,
    artifacts: KagemushaRecursionArtifactsV1,
    incoming_seed: &IncomingStateProofMaterial,
    terminal: ProvenSenderTerminalV1,
) -> (PlonkProtocol<EqAffine>, PlonkProtocol<EpAffine>) {
    let started = std::time::Instant::now();
    let original_core = terminal
        .committed
        .public_output()
        .expect("retained exact Core candidate");
    let original_public = KagemushaTerminalAuthorizationTerminalGenerationPublicV1 {
        lifecycle: original_core.lifecycle,
        semantic_digest: original_core.semantic_digest,
        candidate_envelope_digest: original_core.candidate_envelope_digest,
        commit_certificate_digest: original_core.commit_certificate_digest,
        transition_nullifier: original_core.transition_nullifier,
        request_digest: original_core.request_digest,
        receiver_binding_digest: original_core.receiver_binding_digest,
        ciphertext_commitment: original_core.ciphertext_commitment,
        amount: original_core.amount,
        terminal_output_binding: original_core.terminal_output_binding,
    };
    assert_eq!(terminal.public, original_public);
    assert_eq!(terminal.eq_keys.release_id, artifacts.release_id);
    assert_eq!(terminal.ep_keys.release_id, artifacts.release_id);
    assert_eq!(terminal.eq_keys.profile_digest, artifacts.profile_digest);
    assert_eq!(terminal.ep_keys.profile_digest, artifacts.profile_digest);
    assert_eq!(
        terminal.eq_keys.artifact_manifest_digest,
        artifacts.artifact_manifest_digest
    );
    assert_eq!(
        terminal.ep_keys.artifact_manifest_digest,
        artifacts.artifact_manifest_digest
    );
    let terminal_protocols = [
        terminal.eq_keys.protocol_digest,
        terminal.ep_keys.protocol_digest,
    ];
    let eq_terminal_protocol = compile(
        &funded.eq,
        &terminal.eq_keys.verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
    );
    let ep_terminal_protocol = compile(
        &funded.ep,
        &terminal.ep_keys.verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
    );
    assert_eq!(
        native_parent_protocol_digest_v1(&eq_terminal_protocol, KagemushaPastaParityV1::Eq)
            .unwrap(),
        terminal_protocols[0]
    );
    assert_eq!(
        native_parent_protocol_digest_v1(&ep_terminal_protocol, KagemushaPastaParityV1::Ep)
            .unwrap(),
        terminal_protocols[1]
    );
    let terminal_audits = [
        instance_digest(
            &terminal.proof.eq_public_instances,
            public_instance::EQ_DEFERRED_AUDIT_LO,
        ),
        instance_digest(
            &terminal.proof.eq_public_instances,
            public_instance::EP_DEFERRED_AUDIT_LO,
        ),
    ];
    assert_eq!(
        terminal_audits,
        [
            instance_digest(
                &terminal.proof.ep_public_instances,
                public_instance::EQ_DEFERRED_AUDIT_LO
            ),
            instance_digest(
                &terminal.proof.ep_public_instances,
                public_instance::EP_DEFERRED_AUDIT_LO
            )
        ]
    );
    let exact_terminal_public =
        terminal_public(&terminal.public, terminal_audits, terminal_protocols);
    let mut eq_expected = exact_terminal_public.public_prefix::<Fp>().unwrap();
    eq_expected.extend(history_values::<Fp>(terminal.proof.eq_history.as_bytes()));
    let mut ep_expected = exact_terminal_public.public_prefix::<Fq>().unwrap();
    ep_expected.extend(history_values::<Fq>(terminal.proof.ep_history.as_bytes()));
    assert_eq!(eq_expected, terminal.proof.eq_public_instances);
    assert_eq!(ep_expected, terminal.proof.ep_public_instances);
    let eq_terminal_current = decide_eq(
        &funded.eq,
        &eq_terminal_protocol,
        &terminal.proof.eq_proof,
        &eq_expected,
        &terminal.proof.eq_history,
    )
    .expect("retained real Eq terminal proof and history");
    let ep_terminal_current = decide_ep(
        &funded.ep,
        &ep_terminal_protocol,
        &terminal.proof.ep_proof,
        &ep_expected,
        &terminal.proof.ep_history,
    )
    .expect("retained real Ep terminal proof and history");
    assert_eq!(eq_terminal_current, terminal.proof.eq_current_accumulator);
    assert_eq!(ep_terminal_current, terminal.proof.ep_current_accumulator);
    let seed = test_only_recovery_seed();
    // The preceding stage's candidate/Guard merge is the terminal proof's PRIOR history. Its
    // current equation must be folded with that prior history once more for CommitWrapper.
    let eq_terminal_fold = fold_kagemusha_eq_accumulators_v1(
        &funded.eq,
        &eq_terminal_current,
        &terminal.proof.eq_history,
        &seed,
    )
    .expect("complete exact Eq terminal history");
    let ep_terminal_fold = fold_kagemusha_ep_accumulators_v1(
        &funded.ep,
        &ep_terminal_current,
        &terminal.proof.ep_history,
        &seed,
    )
    .expect("complete exact Ep terminal history");
    decide_kagemusha_eq_accumulator_v1(&funded.eq, eq_terminal_fold.successor()).unwrap();
    decide_kagemusha_ep_accumulator_v1(&funded.ep, ep_terminal_fold.successor()).unwrap();
    let eq_terminal_columns = [eq_expected];
    let ep_terminal_columns = [ep_expected];
    let witness = KagemushaCommitWrapperGenerationWitnessV1 {
        public: terminal.public.clone(),
        enabled_hardware_profiles: terminal.eq_keys.enabled_hardware_profiles,
        eq: KagemushaCommitWrapperEqGenerationWitnessV1 {
            terminal_authorization_protocol: &eq_terminal_protocol,
            terminal_authorization_instances: &eq_terminal_columns,
            terminal_authorization_proof: &terminal.proof.eq_proof,
            terminal_authorization_history: &terminal.proof.eq_history,
            terminal_authorization_history_fold_proof: eq_terminal_fold.proof(),
            successor_history: eq_terminal_fold.successor(),
        },
        ep: KagemushaCommitWrapperEpGenerationWitnessV1 {
            terminal_authorization_protocol: &ep_terminal_protocol,
            terminal_authorization_instances: &ep_terminal_columns,
            terminal_authorization_proof: &terminal.proof.ep_proof,
            terminal_authorization_history: &terminal.proof.ep_history,
            terminal_authorization_history_fold_proof: ep_terminal_fold.proof(),
            successor_history: ep_terminal_fold.successor(),
        },
    };
    eprintln!(
        "KAGEMUSHA wrapper diagnostic: retained terminal pair and both new folds decided; generating dedicated wrapper keys with unchanged transport size gates"
    );
    let generated = generate_kagemusha_commit_wrapper_artifacts_v1(witness.clone())
        .expect("actual wrapper key generation, transport capacity, and final-ID VK stability");
    let (eq_keys, ep_keys) = diagnostic_wrapper_keys(funded, &terminal, generated);
    let eq_wrapper_protocol = compile(
        &funded.eq,
        &eq_keys.verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
    );
    let ep_wrapper_protocol = compile(
        &funded.ep,
        &ep_keys.verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
    );
    let generated = prove_kagemusha_commit_wrapper_v1(&eq_keys, &ep_keys, witness, &seed)
        .expect("actual paired CommitWrapper proof over exact terminal pair");
    assert!(
        generated.clone().into_redemption().is_err(),
        "a proved SendSplit cannot change terminal operation"
    );
    // Conversion exposes the already proved bounded bytes, not a finalized Core payment.
    let generated_payment = generated
        .into_payment()
        .expect("exact SendSplit proof wire family");
    let encoded = norito::encode_canonical(&generated_payment.proof).unwrap();
    let decoded = KagemushaPaymentProofV1::decode_canonical_exact_against(
        &encoded,
        terminal.public.semantic_digest,
        terminal.public.candidate_envelope_digest,
        terminal.public.commit_certificate_digest,
    )
    .expect("exact bounded wrapper wire round trip before native decisions");
    assert_eq!(decoded, generated_payment.proof);
    let mut trailing = encoded.clone();
    trailing.push(0);
    assert!(
        KagemushaPaymentProofV1::decode_canonical_exact_against(
            &trailing,
            terminal.public.semantic_digest,
            terminal.public.candidate_envelope_digest,
            terminal.public.commit_certificate_digest,
        )
        .is_err()
    );
    let wire = &decoded;
    assert_eq!(wire.semantic_digest, terminal.public.semantic_digest);
    assert_eq!(
        wire.candidate_envelope_digest,
        terminal.public.candidate_envelope_digest
    );
    assert_eq!(
        wire.commit_certificate_digest,
        terminal.public.commit_certificate_digest
    );
    let eq_history = KagemushaEqAccumulatorV1::try_from_bytes(&wire.eq_history).unwrap();
    let ep_history = KagemushaEpAccumulatorV1::try_from_bytes(&wire.ep_history).unwrap();
    assert_eq!(&eq_history, eq_terminal_fold.successor());
    assert_eq!(&ep_history, ep_terminal_fold.successor());
    let wrapper_protocols = [eq_keys.protocol_digest, ep_keys.protocol_digest];
    assert_eq!(
        wrapper_protocols,
        [wire.eq_protocol_digest, wire.ep_protocol_digest]
    );
    let exact_wrapper_public = terminal_public(
        &terminal.public,
        [wire.eq_deferred_audit, wire.ep_deferred_audit],
        wrapper_protocols,
    );
    macro_rules! verify {
        ($field:ty, $params:expr, $protocol:expr, $nested:expr, $column:expr, $bytes:expr, $history:expr, $current:expr, $decide:ident) => {{
            validate_wrapper_projection::<$field>(
                $nested,
                $column,
                terminal_protocols,
                wrapper_protocols,
            )
            .unwrap();
            let mut expected = exact_wrapper_public.public_prefix::<$field>().unwrap();
            expected.extend(history_values::<$field>($history.as_bytes()));
            assert_eq!($column, &expected);
            assert_eq!(
                $decide($params, $protocol, $bytes, $column, $history).unwrap(),
                *$current
            );
            for offset in [
                public_instance::SEMANTIC_LO,
                public_instance::CANDIDATE_LO,
                public_instance::COMMIT_CERTIFICATE_LO,
                public_instance::AMOUNT,
                public_instance::OUTPUT_BINDING_LO,
                public_instance::EQ_DEFERRED_AUDIT_LO,
                public_instance::EP_DEFERRED_AUDIT_LO,
                public_instance::EQ_PROTOCOL_LO,
                public_instance::EP_PROTOCOL_LO,
                public_instance::HISTORY_START,
            ] {
                let mut changed = $column.to_vec();
                changed[offset] += <$field>::ONE;
                assert!(
                    $decide($params, $protocol, $bytes, &changed, $history).is_err(),
                    "actual wrapper proof accepted changed public row {offset}"
                );
            }
        }};
    }
    verify!(
        Fp,
        &funded.eq,
        &eq_wrapper_protocol,
        &terminal.proof.eq_public_instances,
        &generated_payment.eq_public_instances,
        &wire.eq_proof,
        &eq_history,
        &generated_payment.eq_current_accumulator,
        decide_eq
    );
    verify!(
        Fq,
        &funded.ep,
        &ep_wrapper_protocol,
        &terminal.proof.ep_public_instances,
        &generated_payment.ep_public_instances,
        &wire.ep_proof,
        &ep_history,
        &generated_payment.ep_current_accumulator,
        decide_ep
    );
    // Keep the real bytes while selecting the other role's verifier: no role can be relabeled.
    assert!(
        decide_eq(
            &funded.eq,
            &eq_terminal_protocol,
            &wire.eq_proof,
            &generated_payment.eq_public_instances,
            &eq_history
        )
        .is_err()
    );
    assert!(
        decide_ep(
            &funded.ep,
            &ep_terminal_protocol,
            &wire.ep_proof,
            &generated_payment.ep_public_instances,
            &ep_history
        )
        .is_err()
    );
    assert!(encoded.len() <= KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1);
    assert!(wire.eq_proof.len() <= KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1);
    assert!(wire.ep_proof.len() <= KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1);

    // Observing equal structures is necessary but insufficient: the old State candidate and
    // every non-Bootstrap predecessor still bind the old full wrapper protocol identities.
    // Neither the seed protocols nor any existing State/public proof field is replaced here.
    let eq_seed_structure = kagemusha_protocol_structure_digest_v1(
        &incoming_seed.eq_protocol,
        KagemushaPastaParityV1::Eq,
    )
    .unwrap();
    let ep_seed_structure = kagemusha_protocol_structure_digest_v1(
        &incoming_seed.ep_protocol,
        KagemushaPastaParityV1::Ep,
    )
    .unwrap();
    let eq_wrapper_structure =
        kagemusha_protocol_structure_digest_v1(&eq_wrapper_protocol, KagemushaPastaParityV1::Eq)
            .unwrap();
    let ep_wrapper_structure =
        kagemusha_protocol_structure_digest_v1(&ep_wrapper_protocol, KagemushaPastaParityV1::Ep)
            .unwrap();
    let eq_seed_identity =
        native_parent_protocol_digest_v1(&incoming_seed.eq_protocol, KagemushaPastaParityV1::Eq)
            .unwrap();
    let ep_seed_identity =
        native_parent_protocol_digest_v1(&incoming_seed.ep_protocol, KagemushaPastaParityV1::Ep)
            .unwrap();
    assert_eq!(
        [eq_seed_identity, ep_seed_identity],
        [
            artifacts.commit_wrapper_eq_protocol_digest,
            artifacts.commit_wrapper_ep_protocol_digest
        ]
    );
    let retained_state = terminal.committed.candidate.recovery_view().unwrap();
    let original_state_proof = retained_state.candidate_proof;
    assert_eq!(
        *retained_state.candidate_envelope_digest,
        terminal.public.candidate_envelope_digest
    );
    assert_eq!(
        original_state_proof.semantic_digest,
        terminal.public.semantic_digest
    );
    eprintln!(
        "KAGEMUSHA diagnostic wrapper structure: Eq seed={} actual={} match={}; Ep seed={} actual={} match={}; full identities match={}; final graph re-proving has not run",
        hex::encode(eq_seed_structure),
        hex::encode(eq_wrapper_structure),
        eq_seed_structure == eq_wrapper_structure,
        hex::encode(ep_seed_structure),
        hex::encode(ep_wrapper_structure),
        ep_seed_structure == ep_wrapper_structure,
        [eq_seed_identity, ep_seed_identity] == wrapper_protocols
    );
    eprintln!(
        "KAGEMUSHA diagnostic actual paired CommitWrapper verified and decided in {:?}; proof bytes Eq={} Ep={} pair={}; no Core journal commit, admitted release, physical finality, or handoff qualification",
        started.elapsed(),
        wire.eq_proof.len(),
        wire.ep_proof.len(),
        encoded.len()
    );
    (eq_wrapper_protocol, ep_wrapper_protocol)
}
