//! Same-body active Mint State qualification with genuine current proofs and complete histories.
//!
//! Known-public mathematical originals install no release, FI/clock/DATA authority or Native
//! owner. The full-cycle caller must supply the final held Wrapper descriptor and later require
//! that the genuinely generated Send Wrapper retains it. No descriptor is accepted from a DTO.
use super::super::*;
use super::*;
use crate::kagemusha_v1_recursion::KagemushaReplayInsertWitnessV1;
use crate::kagemusha_v1_recursion::{
    composite::RecursiveStateConstructionV1,
    ordinary_guard_circuit::OrdinaryGuardWitnessV1,
    real_handoff_qualification_tests::{
        StateKeys,
        real_payment_corridor::{
            OrdinaryProvenMintSourceForTestingV1, prove_ordinary_neutral_mint_source_for_testing_v1,
        },
        terminally_verify_state_proof,
    },
};
use crate::kagemusha_v1_state::DigestV1;
use crate::kagemusha_v1_state::{
    CreditIdV1, KagemushaStateV1, KagemushaTransitionKindV1,
    OrdinaryConsumedCreditsForQualificationV1, ordinary_incoming_preview_for_qualification_v1,
};

/// Retained genuine common family and its actually funded predecessor for the next Send step.
/// Private mathematical fixture data cannot become an authenticated Native financial owner.
pub(in super::super) struct OrdinaryFundedStateForTestingV1 {
    pub(in super::super) bootstrap:
        super::super::ordinary_active_state_bootstrap::OrdinaryBootstrapStateForTestingV1,
    pub(in super::super) state: KagemushaStateV1,
    pub(in super::super) generated: KagemushaGeneratedRecursiveStateProofV1,
    pub(in super::super) public_original: Vec<u8>,
    // Hold the exact consumed-credit replay index for the funded fixture lifetime.
    pub(in super::super) _consumed_credits: OrdinaryConsumedCreditsForQualificationV1,
    pub(in super::super) mint_source: ProvenMintForTesting,
}

pub(in super::super) struct ProvenMintForTesting {
    pub(in super::super) pair: super::super::ordinary_mint_generation::GeneratedOrdinaryMintPairV1,
    pub(in super::super) authorization: KagemushaOrdinaryMintAuthorizationV1,
    pub(in super::super) source: OrdinaryProvenMintSourceForTestingV1,
}

fn prove_mint_source(
    f: &MintOriginals,
    hash_eq: KagemushaLoadedEqMintHashArtifactsV1,
    hash_ep: KagemushaLoadedEpMintHashArtifactsV1,
) -> ProvenMintForTesting {
    let seed = KagemushaRecoverySeedV1::from_unsealed([44; 32]).unwrap();
    let mut pair = super::super::ordinary_mint_generation::generate_ordinary_mint_pair_v1(
        OrdinaryMintWitnessV1 {
            statement: &f.statement,
            approval: &f.approval,
            credential: &f.enrollment.credential,
            previous_app_attest_counter: f.enrollment.previous_counter,
            integrity_lease: None,
            financial_secret: &[0x41; 32],
            credit_opening: &f.opening,
            encrypted_credit: &f.encrypted_credit,
        },
        f.enrollment.state.device_policy_binding.hardware_policy_id,
        &f.enrollment.issuer_table,
        &seed,
    )
    .expect("actual ordinary113 consuming-resource-preflight keys and both original proofs");
    let authorization = KagemushaOrdinaryMintAuthorizationV1 {
        version: 1,
        statement: f.statement.clone(),
        approval: f.approval.clone(),
        proof: KagemushaOrdinaryMintPairedProofV1 {
            version: 1,
            eq_protocol_digest: pair.eq.protocol_digest,
            ep_protocol_digest: pair.ep.protocol_digest,
            statement_digest: f.statement.binding_digest().unwrap(),
            approval_original_digest: f.approval.binding_digest().unwrap(),
            eq_proof: pair.eq.proof.clone(),
            ep_proof: pair.ep.proof.clone(),
            eq_history: pair.eq.history.as_bytes().to_vec(),
            ep_history: pair.ep.history.as_bytes().to_vec(),
        },
    };
    authorization.validate_shape().unwrap();
    let eq = canonical_kagemusha_eq_parameters_v1();
    let ep = canonical_kagemusha_ep_parameters_v1();
    let params_eq = crate::kagemusha_v1_recursion::ordinary_mint_circuit::KagemushaOrdinaryMintCircuitParamsV1 {
        base: pair.eq.base_params.clone(),
        provider_policy_root: f.enrollment.state.device_policy_binding.hardware_policy_id,
        issuer_table: f.enrollment.issuer_table.clone(),
    };
    let params_ep = crate::kagemusha_v1_recursion::ordinary_mint_circuit::KagemushaOrdinaryMintCircuitParamsV1 {
        base: pair.ep.base_params.clone(),
        ..params_eq.clone()
    };
    let eq_vk = VerifyingKey::<EqAffine>::read::<
        _,
        crate::kagemusha_v1_recursion::ordinary_mint_circuit::KagemushaOrdinaryMintEqCircuitV1,
    >(
        &mut Cursor::new(pair.eq.verifying_key.as_ref()),
        SerdeFormat::Processed,
        params_eq,
    )
    .unwrap();
    let ep_vk = VerifyingKey::<EpAffine>::read::<
        _,
        crate::kagemusha_v1_recursion::ordinary_mint_circuit::KagemushaOrdinaryMintEpCircuitV1,
    >(
        &mut Cursor::new(pair.ep.verifying_key.as_ref()),
        SerdeFormat::Processed,
        params_ep,
    )
    .unwrap();
    // These seeds are disabled Bootstrap parser operands in the maintained neutral helper.
    // Its bounded topology discovery replaces them before the active authority proof executes.
    let eq_seed = compile(&eq, &eq_vk, snark_verifier::system::halo2::Config::ipa()
        .with_num_instance(vec![crate::kagemusha_v1_recursion::mint_authority::KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]));
    let ep_seed = compile(&ep, &ep_vk, snark_verifier::system::halo2::Config::ipa()
        .with_num_instance(vec![crate::kagemusha_v1_recursion::mint_authority::KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]));
    drop(eq_vk);
    drop(ep_vk);
    // Retain original proofs/protocols; serialized PKs are no longer needed for this source.
    pair.eq.proving_key = Arc::from([]);
    pair.ep.proving_key = Arc::from([]);
    pair.eq.parameters = Arc::from([]);
    pair.ep.parameters = Arc::from([]);
    halo2_proofs::release_allocator_slack();
    let source = prove_ordinary_neutral_mint_source_for_testing_v1(
        &authorization,
        &f.encrypted_credit,
        hash_eq,
        hash_ep,
        eq_seed,
        ep_seed,
    );
    super::active_state::require_actual_mint_source(&eq, &ep, &authorization, &source);
    ProvenMintForTesting {
        pair,
        authorization,
        source,
    }
}

/// Same active Mint body using the caller's immutable already generated State keys.
pub(in super::super) fn prove_funded_ordinary_state_with_held_keys_for_testing_v1(
    apple: bool,
    release: DigestV1,
    vk: DigestV1,
    manifest: DigestV1,
    hash_eq: KagemushaLoadedEqMintHashArtifactsV1,
    hash_ep: KagemushaLoadedEpMintHashArtifactsV1,
    wrapper_eq: &PlonkProtocol<EqAffine>,
    wrapper_ep: &PlonkProtocol<EpAffine>,
    held_keys: Option<Arc<StateKeys>>,
) -> OrdinaryFundedStateForTestingV1 {
    let eq = canonical_kagemusha_eq_parameters_v1();
    let ep = canonical_kagemusha_ep_parameters_v1();
    let seed = KagemushaRecoverySeedV1::from_unsealed([44; 32]).unwrap();
    let suite = hash_eq.suite_id;
    let account = super::super::ordinary_qualification_wallet_account_v1(62);
    let seed_f = fixture_for_active_state(apple, release, suite, vk, manifest, [47; 32]);
    let seed_mint = prove_mint_source(&seed_f, hash_eq, hash_ep);
    let bootstrap = super::super::ordinary_active_state_bootstrap::generate_ordinary_bootstrap_state_with_held_keys_for_testing_v1(
        apple, &account, release, vk, manifest,
        &seed_mint.source.hash_eq, &seed_mint.source.hash_ep,
        wrapper_eq, wrapper_ep,
        &seed_mint.pair.eq.protocol, &seed_mint.pair.ep.protocol,
        &seed_mint.source.eq_protocol, &seed_mint.source.ep_protocol, held_keys,
    );
    let expected_mint_protocols = [
        seed_mint.pair.eq.protocol_digest,
        seed_mint.pair.ep.protocol_digest,
    ];
    let expected_authority_protocols = [
        native_parent_protocol_digest_v1(&seed_mint.source.eq_protocol, KagemushaPastaParityV1::Eq)
            .unwrap(),
        native_parent_protocol_digest_v1(&seed_mint.source.ep_protocol, KagemushaPastaParityV1::Ep)
            .unwrap(),
    ];
    let mut f = fixture_for_active_state(
        apple,
        release,
        suite,
        vk,
        manifest,
        Sha256::digest(&bootstrap.public_original).into(),
    );
    // Apple progression is original Bootstrap17→pre-debit18→fresh incoming W2 19.
    // This is a mathematical witness, never a claim that Core transports a private floor.
    if apple {
        f.enrollment.previous_counter = Some(17);
        f.approval.evidence = sign_message_with_counter(
            &f.approval.challenge.canonical_signing_bytes().unwrap(),
            true,
            18,
        );
        verify_platform(&f.approval, &f.enrollment.credential, Some(17));
    }
    assert_eq!(f.enrollment.state, bootstrap.state);
    assert_eq!(f.enrollment.credential, bootstrap.credential);
    let ProvenMintForTesting {
        source: seed_source,
        pair: old_pair,
        authorization: old_authorization,
    } = seed_mint;
    let OrdinaryProvenMintSourceForTestingV1 {
        hash_eq, hash_ep, ..
    } = seed_source;
    drop(old_pair);
    drop(old_authorization);
    halo2_proofs::release_allocator_slack();
    let mint = prove_mint_source(&f, hash_eq, hash_ep);
    assert_eq!(
        [mint.pair.eq.protocol_digest, mint.pair.ep.protocol_digest],
        expected_mint_protocols,
        "Mint original/parent SHA replacement must preserve the held fixed family"
    );
    assert_eq!(
        [
            native_parent_protocol_digest_v1(&mint.source.eq_protocol, KagemushaPastaParityV1::Eq)
                .unwrap(),
            native_parent_protocol_digest_v1(&mint.source.ep_protocol, KagemushaPastaParityV1::Ep)
                .unwrap(),
        ],
        expected_authority_protocols,
        "active neutral source must retain the Bootstrap held family"
    );
    let request = KagemushaOrdinaryTopUpRequestV1 {
        version: 1,
        authorization: mint.authorization.clone(),
        encrypted_credit: mint.source.credit.encrypted_credit.clone(),
    };
    let c = &bootstrap.credential;
    let context = &mint.authorization.statement.context;
    let reservation = KagemushaOrdinaryIncomingReservationV1 {
        selection: KagemushaOrdinaryIncomingSelectionV1 {
            version: 1,
            lineage: context.lineage.clone(),
            operation_id: context.operation_id,
            predecessor: context.predecessor,
            source: KagemushaOrdinaryIncomingSourceSelectionV1::Mint {
                topup_request_original_sha256: Sha256::digest(request.canonical_bytes().unwrap())
                    .into(),
            },
            credit_id: context.credit_id().unwrap(),
            amount: context.amount,
            scale: context.lineage.owner.runtime.scale,
            recipient_app_credential_digest: c.canonical_digest().unwrap(),
            financial_control_original_sha256: context.financial_control_original_sha256,
            clock_context_digest: context.clock_context.binding_digest().unwrap(),
        },
        source_proof_original_sha256: Sha256::digest(
            norito::encode_canonical(&mint.source.credit).unwrap(),
        )
        .into(),
        // External finalized Node-original authentication is a separate closed source capability.
        // This fixture deliberately installs no such capability or DATA effect.
        finalized_source_original_sha256: Sha256::digest(
            b"known-public mathematical finalized source data",
        )
        .into(),
        source_semantic_digest: mint.source.credit.statement.canonical_digest().unwrap(),
    };
    reservation
        .selection
        .validate_against_topup(&request)
        .unwrap();
    let mut consumed = OrdinaryConsumedCreditsForQualificationV1::empty();
    assert_eq!(consumed.root(), bootstrap.state.consumed_credit_root);
    let replay = consumed
        .preview(
            CreditIdV1(reservation.selection.credit_id),
            reservation.digest().unwrap(),
        )
        .unwrap();
    let fresh_clock = KagemushaOrdinaryCashClockContextV1 {
        version: 1,
        request_nonce: [70; 32],
        signed_observations_original_digest: [71; 32],
        lower_at_ms: 1500,
        upper_at_ms: 1501,
    };
    let mut artifacts = crate::kagemusha_v1_recursion::tests::artifacts();
    artifacts.release_id = release;
    artifacts.artifact_manifest_digest = manifest;
    artifacts.eq_protocol_digest =
        native_parent_protocol_digest_v1(&bootstrap.keys.eq_protocol, KagemushaPastaParityV1::Eq)
            .unwrap();
    artifacts.ep_protocol_digest =
        native_parent_protocol_digest_v1(&bootstrap.keys.ep_protocol, KagemushaPastaParityV1::Ep)
            .unwrap();
    artifacts.guard_bundle_eq_protocol_digest =
        native_parent_protocol_digest_v1(&bootstrap.guard_eq_protocol, KagemushaPastaParityV1::Eq)
            .unwrap();
    artifacts.guard_bundle_ep_protocol_digest =
        native_parent_protocol_digest_v1(&bootstrap.guard_ep_protocol, KagemushaPastaParityV1::Ep)
            .unwrap();
    artifacts.mint_authorization_eq_protocol_digest = mint.pair.eq.protocol_digest;
    artifacts.mint_authorization_ep_protocol_digest = mint.pair.ep.protocol_digest;
    artifacts.mint_finality_eq_protocol_digest = mint.source.credit.proof.eq_protocol_digest;
    artifacts.mint_finality_ep_protocol_digest = mint.source.credit.proof.ep_protocol_digest;
    artifacts.commit_wrapper_eq_protocol_digest =
        native_parent_protocol_digest_v1(wrapper_eq, KagemushaPastaParityV1::Eq).unwrap();
    artifacts.commit_wrapper_ep_protocol_digest =
        native_parent_protocol_digest_v1(wrapper_ep, KagemushaPastaParityV1::Ep).unwrap();
    artifacts.canonical_empty_effect_digest = f.enrollment.relation.canonical_empty_effect_digest;
    let preview = ordinary_incoming_preview_for_qualification_v1(
        &bootstrap.state,
        &reservation,
        KagemushaTransitionKindV1::MintFold,
        mint.source.credit.finality_proof_binding_digest,
        mint.source
            .credit
            .statement
            .lifecycle
            .canonical_digest()
            .unwrap(),
        &replay,
        [72; 32],
        0,
        [73; 32],
        &fresh_clock,
        [74; 32],
        artifacts,
    )
    .unwrap();
    let mut subject = f.enrollment.approval.challenge.subject.clone();
    subject.operation_kind = KagemushaOperationKindV1::MintFold;
    subject.transition_statement_digest = preview.statement.digest().unwrap();
    subject.candidate_envelope_digest = [0; 32];
    subject.terminal_body_commitment = [0; 32];
    subject.secure_index_before = bootstrap.state.secure_index;
    subject.secure_index_after = preview.successor.secure_index;
    let issued = fresh_clock.upper_at_ms;
    let challenge = KagemushaAppOperationApprovalChallengeV1 {
        version: 1,
        purpose: KagemushaAppOperationApprovalPurposeV1::PrepareTransition,
        operation_id: preview.preparation.operation_id,
        nonce: preview.preparation.nonce,
        account_binding: c.subject.account_binding,
        authority_policy_digest: c.subject.app_authority_policy_digest,
        attested_key_id: c.subject.attested_key_id,
        enrollment_digest: c.canonical_digest().unwrap(),
        subject_signing_digest: Sha256::digest(subject.canonical_prepare_signing_bytes().unwrap())
            .into(),
        normalized_guard_digest: preview.normalized.canonical_digest().unwrap(),
        issued_at_ms: issued,
        expires_at_ms: issued
            .checked_add(crate::kagemusha_v1_state::ORDINARY_PREPARATION_LIFETIME_MS)
            .unwrap()
            .min(c.subject.expires_at_ms),
        subject,
    };
    let approval = KagemushaAppOperationApprovalV1 {
        evidence: sign_message_with_counter(
            &challenge.canonical_signing_bytes().unwrap(),
            apple,
            19,
        ),
        challenge,
    };
    let previous_counter = apple.then_some(18);
    let mut guard_relation = f.enrollment.relation.clone();
    guard_relation.statement = preview.normalized.clone();
    guard_relation.validate().unwrap();
    let guard = super::super::ordinary_guard_generation::generate_ordinary_guard_pair_v1(
        OrdinaryGuardWitnessV1 {
            relation: &guard_relation,
            credential: c,
            approval: &approval,
            previous_app_attest_counter: previous_counter,
            integrity_lease: None,
            incoming_terminal_body: None,
        },
        bootstrap.state.device_policy_binding.hardware_policy_id,
        &bootstrap.issuer_table,
        &seed,
    )
    .expect("fresh incoming W2 full fixed-topology Guard proof under actual original bindings");
    assert_eq!(
        guard.eq.protocol_digest,
        native_parent_protocol_digest_v1(&bootstrap.guard_eq_protocol, KagemushaPastaParityV1::Eq)
            .unwrap()
    );
    assert_eq!(
        guard.ep.protocol_digest,
        native_parent_protocol_digest_v1(&bootstrap.guard_ep_protocol, KagemushaPastaParityV1::Ep)
            .unwrap()
    );
    let eq_zero = initial_kagemusha_eq_accumulator_v1(&eq).unwrap();
    let ep_zero = initial_kagemusha_ep_accumulator_v1(&ep).unwrap();
    let eq_fold_padding = KagemushaEqFoldProofV1::try_from_bytes(&dummy_fold_proof_bytes(
        EqAffine::generator().to_bytes().as_ref(),
    ))
    .unwrap();
    let ep_fold_padding = KagemushaEpFoldProofV1::try_from_bytes(&dummy_fold_proof_bytes(
        EpAffine::generator().to_bytes().as_ref(),
    ))
    .unwrap();
    let eq_incoming_padding = dummy_ordinary_proof_bytes(
        wrapper_eq,
        EqAffine::generator().to_bytes().as_ref(),
        KagemushaPastaParityV1::Eq,
    )
    .unwrap();
    let ep_incoming_padding = dummy_ordinary_proof_bytes(
        wrapper_ep,
        EpAffine::generator().to_bytes().as_ref(),
        KagemushaPastaParityV1::Ep,
    )
    .unwrap();
    let eq_wrapper_padding_column =
        inactive_wrapper_column::<Fp>(wrapper_eq, wrapper_ep, eq_zero.as_bytes());
    let ep_wrapper_padding_column =
        inactive_wrapper_column::<Fq>(wrapper_eq, wrapper_ep, ep_zero.as_bytes());
    let eq_inner_column = vec![bootstrap.generated.eq_public_instances.clone()];
    let ep_inner_column = vec![bootstrap.generated.ep_public_instances.clone()];
    let eq_outer_column = vec![bootstrap.generated.eq_transport_public_instances.clone()];
    let ep_outer_column = vec![bootstrap.generated.ep_transport_public_instances.clone()];
    let eq_inner_current = KagemushaEqAccumulatorV1::from_native(
        &verify_eq_succinct_protocol(
            &eq,
            &bootstrap.keys.eq_protocol,
            &bootstrap.generated.eq_inner_proof,
            &eq_inner_column[0],
        )
        .unwrap(),
    )
    .unwrap();
    let ep_inner_current = KagemushaEpAccumulatorV1::from_native(
        &verify_ep_succinct_protocol(
            &ep,
            &bootstrap.keys.ep_protocol,
            &bootstrap.generated.ep_inner_proof,
            &ep_inner_column[0],
        )
        .unwrap(),
    )
    .unwrap();
    let eq_parent_complete = fold_kagemusha_eq_accumulators_v1(
        &eq,
        &eq_inner_current,
        &bootstrap.generated.eq_history,
        &seed,
    )
    .unwrap();
    let ep_parent_complete = fold_kagemusha_ep_accumulators_v1(
        &ep,
        &ep_inner_current,
        &bootstrap.generated.ep_history,
        &seed,
    )
    .unwrap();
    let eq_outer_history =
        KagemushaEqAccumulatorV1::try_from_bytes(&bootstrap.generated.proof.eq_history).unwrap();
    let ep_outer_history =
        KagemushaEpAccumulatorV1::try_from_bytes(&bootstrap.generated.proof.ep_history).unwrap();
    let eq_outer_current = KagemushaEqAccumulatorV1::from_native(
        &verify_eq_succinct_protocol(
            &eq,
            &bootstrap.keys.eq_transport_protocol,
            &bootstrap.generated.proof.eq_proof,
            &eq_outer_column[0],
        )
        .unwrap(),
    )
    .unwrap();
    let ep_outer_current = KagemushaEpAccumulatorV1::from_native(
        &verify_ep_succinct_protocol(
            &ep,
            &bootstrap.keys.ep_transport_protocol,
            &bootstrap.generated.proof.ep_proof,
            &ep_outer_column[0],
        )
        .unwrap(),
    )
    .unwrap();
    let eq_outer_complete =
        fold_kagemusha_eq_accumulators_v1(&eq, &eq_outer_current, &eq_outer_history, &seed)
            .unwrap();
    let ep_outer_complete =
        fold_kagemusha_ep_accumulators_v1(&ep, &ep_outer_current, &ep_outer_history, &seed)
            .unwrap();
    let eq_outer_merge = fold_kagemusha_eq_accumulators_v1(
        &eq,
        eq_parent_complete.successor(),
        eq_outer_complete.successor(),
        &seed,
    )
    .unwrap();
    let ep_outer_merge = fold_kagemusha_ep_accumulators_v1(
        &ep,
        ep_parent_complete.successor(),
        ep_outer_complete.successor(),
        &seed,
    )
    .unwrap();
    let eq_guard_current = KagemushaEqAccumulatorV1::from_native(
        &verify_eq_succinct_protocol(
            &eq,
            &guard.eq.protocol,
            &guard.eq.proof,
            &guard.eq.instances,
        )
        .unwrap(),
    )
    .unwrap();
    let ep_guard_current = KagemushaEpAccumulatorV1::from_native(
        &verify_ep_succinct_protocol(
            &ep,
            &guard.ep.protocol,
            &guard.ep.proof,
            &guard.ep.instances,
        )
        .unwrap(),
    )
    .unwrap();
    let eq_guard_complete =
        fold_kagemusha_eq_accumulators_v1(&eq, &eq_guard_current, &guard.eq.history, &seed)
            .unwrap();
    let ep_guard_complete =
        fold_kagemusha_ep_accumulators_v1(&ep, &ep_guard_current, &guard.ep.history, &seed)
            .unwrap();
    let eq_guard_merge = fold_kagemusha_eq_accumulators_v1(
        &eq,
        eq_outer_merge.successor(),
        eq_guard_complete.successor(),
        &seed,
    )
    .unwrap();
    let ep_guard_merge = fold_kagemusha_ep_accumulators_v1(
        &ep,
        ep_outer_merge.successor(),
        ep_guard_complete.successor(),
        &seed,
    )
    .unwrap();
    let eq_auth_current = KagemushaEqAccumulatorV1::from_native(
        &verify_eq_succinct_protocol(
            &eq,
            &mint.pair.eq.protocol,
            &mint.authorization.proof.eq_proof,
            &mint.pair.eq.instances,
        )
        .unwrap(),
    )
    .unwrap();
    let ep_auth_current = KagemushaEpAccumulatorV1::from_native(
        &verify_ep_succinct_protocol(
            &ep,
            &mint.pair.ep.protocol,
            &mint.authorization.proof.ep_proof,
            &mint.pair.ep.instances,
        )
        .unwrap(),
    )
    .unwrap();
    let eq_auth_complete =
        fold_kagemusha_eq_accumulators_v1(&eq, &eq_auth_current, &mint.pair.eq.history, &seed)
            .unwrap();
    let ep_auth_complete =
        fold_kagemusha_ep_accumulators_v1(&ep, &ep_auth_current, &mint.pair.ep.history, &seed)
            .unwrap();
    let eq_auth_merge = fold_kagemusha_eq_accumulators_v1(
        &eq,
        eq_guard_merge.successor(),
        eq_auth_complete.successor(),
        &seed,
    )
    .unwrap();
    let ep_auth_merge = fold_kagemusha_ep_accumulators_v1(
        &ep,
        ep_guard_merge.successor(),
        ep_auth_complete.successor(),
        &seed,
    )
    .unwrap();
    let eq_mint_current = KagemushaEqAccumulatorV1::from_native(
        &verify_eq_succinct_protocol(
            &eq,
            &mint.source.eq_protocol,
            &mint.source.credit.proof.eq_proof,
            &mint.source.eq_instances,
        )
        .unwrap(),
    )
    .unwrap();
    let ep_mint_current = KagemushaEpAccumulatorV1::from_native(
        &verify_ep_succinct_protocol(
            &ep,
            &mint.source.ep_protocol,
            &mint.source.credit.proof.ep_proof,
            &mint.source.ep_instances,
        )
        .unwrap(),
    )
    .unwrap();
    let eq_mint_complete =
        fold_kagemusha_eq_accumulators_v1(&eq, &eq_mint_current, &mint.source.eq_history, &seed)
            .unwrap();
    let ep_mint_complete =
        fold_kagemusha_ep_accumulators_v1(&ep, &ep_mint_current, &mint.source.ep_history, &seed)
            .unwrap();
    let eq_mint_merge = fold_kagemusha_eq_accumulators_v1(
        &eq,
        eq_auth_merge.successor(),
        eq_mint_complete.successor(),
        &seed,
    )
    .unwrap();
    let ep_mint_merge = fold_kagemusha_ep_accumulators_v1(
        &ep,
        ep_auth_merge.successor(),
        ep_mint_complete.successor(),
        &seed,
    )
    .unwrap();
    let (inactive_authorization, _) =
        crate::kagemusha_v1_recursion::generation::production_prover::ordinary_bootstrap_padding_for_testing(
            &bootstrap.state,
            &account,
            manifest,
            [75; 32],
            &mint.pair.eq.protocol,
            &mint.pair.ep.protocol,
            &mint.source.eq_protocol,
            &mint.source.ep_protocol,
            &eq_zero,
            &ep_zero,
        )
        .unwrap();
    let eq_mint_columns = vec![mint.source.eq_instances.clone()];
    let ep_mint_columns = vec![mint.source.ep_instances.clone()];
    let eq_auth_columns = vec![mint.pair.eq.instances.clone()];
    let ep_auth_columns = vec![mint.pair.ep.instances.clone()];
    let eq_wrapper_columns = vec![eq_wrapper_padding_column];
    let ep_wrapper_columns = vec![ep_wrapper_padding_column];
    let t = &preview.statement;
    let mut witness = KagemushaRecursiveStateGenerationWitnessV1 {
        hash_claim: None,
        state: KagemushaStateRelationWitnessV1 {
            operation: KagemushaOperationV1::MintFold,
            predecessor: Some(bootstrap.state.clone()),
            successor: preview.successor.clone(),
            amount: t.amount,
            journal_revision_before: t.journal_revision_before,
            journal_revision_after: t.journal_revision_after,
            transition_effect_digest: t.effect_digest,
            mint_finality_semantic_digest: t.mint_finality_semantic_digest,
            mint_finality_proof_binding_digest: t.mint_finality_proof_binding_digest,
            peer_credit_id: t.peer_credit_id,
            recipient_encryption_key_binding: t.recipient_encryption_key_binding,
            receive_credit: None,
            receive_credit_binding_digest: t.receive_credit_binding_digest,
            lifecycle_binding_digest: t.lifecycle_binding_digest,
            prepared_transition_binding_digest: [0; 32],
            prepared_intent: None,
            transport_semantic_digest: preview.transport_semantic_digest,
            guard_statement_digest: preview.normalized.canonical_digest().unwrap(),
            eq_protocol_digest: native_parent_protocol_digest_v1(
                &bootstrap.keys.eq_protocol,
                KagemushaPastaParityV1::Eq,
            )
            .unwrap(),
            ep_protocol_digest: native_parent_protocol_digest_v1(
                &bootstrap.keys.ep_protocol,
                KagemushaPastaParityV1::Ep,
            )
            .unwrap(),
            guard_eq_protocol_digest: guard.eq.protocol_digest,
            guard_ep_protocol_digest: guard.ep.protocol_digest,
            mint_eq_protocol_digest: mint.source.credit.proof.eq_protocol_digest,
            mint_ep_protocol_digest: mint.source.credit.proof.ep_protocol_digest,
            mint_authorization_eq_protocol_digest: mint.pair.eq.protocol_digest,
            mint_authorization_ep_protocol_digest: mint.pair.ep.protocol_digest,
            commit_wrapper_eq_protocol_digest: native_parent_protocol_digest_v1(
                wrapper_eq,
                KagemushaPastaParityV1::Eq,
            )
            .unwrap(),
            commit_wrapper_ep_protocol_digest: native_parent_protocol_digest_v1(
                wrapper_ep,
                KagemushaPastaParityV1::Ep,
            )
            .unwrap(),
            guard_eq_credential_audit: bootstrap.keys.eq_protocol_digest,
            guard_ep_credential_audit: bootstrap.keys.ep_protocol_digest,
            eq_deferred_audit: [1; 32],
            ep_deferred_audit: [2; 32],
            replay_insert: Some(KagemushaReplayInsertWitnessV1::from(&replay)),
        },
        mint_fold_opening: None,
        mint_authorization: &inactive_authorization,
        mint_credit: &mint.source.credit,
        guard_relation,
        hardware_selection: None,
        ordinary_selection: Some(KagemushaOrdinaryAppRecursiveSelectionWitnessV1 {
            credential: c,
            approval: &approval,
            integrity_lease: None,
            previous_app_attest_counter: previous_counter,
            prepared: None,
            outer_parent: Some(KagemushaOrdinaryRecursiveOuterParentWitnessV1 {
                public_original: Some(&bootstrap.public_original),
                eq_protocol: &bootstrap.keys.eq_transport_protocol,
                ep_protocol: &bootstrap.keys.ep_transport_protocol,
                eq_instances: &eq_outer_column,
                ep_instances: &ep_outer_column,
                eq_proof: &bootstrap.generated.proof.eq_proof,
                ep_proof: &bootstrap.generated.proof.ep_proof,
                eq_history: &eq_outer_history,
                ep_history: &ep_outer_history,
                eq_history_fold: eq_outer_complete.proof(),
                ep_history_fold: ep_outer_complete.proof(),
                eq_merge_fold: eq_outer_merge.proof(),
                ep_merge_fold: ep_outer_merge.proof(),
            }),
            incoming_mint: Some(KagemushaOrdinaryRecursiveMintIncomingOpeningV1 {
                authorization: &mint.authorization,
                reservation: &reservation,
                preparation: &preview.preparation,
                credit_opening: &f.opening,
            }),
            incoming_receive: None,
        }),
        eq_parent_protocol: &bootstrap.keys.eq_protocol,
        ep_parent_protocol: &bootstrap.keys.ep_protocol,
        eq_parent_instances: &eq_inner_column,
        ep_parent_instances: &ep_inner_column,
        eq_parent_proof: &bootstrap.generated.eq_inner_proof,
        ep_parent_proof: &bootstrap.generated.ep_inner_proof,
        eq_predecessor_history: &bootstrap.generated.eq_history,
        ep_predecessor_history: &bootstrap.generated.ep_history,
        eq_parent_fold_proof: eq_parent_complete.proof(),
        ep_parent_fold_proof: ep_parent_complete.proof(),
        eq_incoming_protocol: wrapper_eq,
        ep_incoming_protocol: wrapper_ep,
        eq_incoming_credits: [KagemushaRecursiveIncomingEqGenerationWitnessV1 {
            instances: &eq_wrapper_columns,
            proof: &eq_incoming_padding,
            history: &eq_zero,
            history_fold_proof: &eq_fold_padding,
            merge_fold_proof: &eq_fold_padding,
        }],
        ep_incoming_credits: [KagemushaRecursiveIncomingEpGenerationWitnessV1 {
            instances: &ep_wrapper_columns,
            proof: &ep_incoming_padding,
            history: &ep_zero,
            history_fold_proof: &ep_fold_padding,
            merge_fold_proof: &ep_fold_padding,
        }],
        eq_successor_history: eq_mint_merge.successor(),
        ep_successor_history: ep_mint_merge.successor(),
        eq_guard_protocol: &guard.eq.protocol,
        ep_guard_protocol: &guard.ep.protocol,
        eq_guard_proof: &guard.eq.proof,
        ep_guard_proof: &guard.ep.proof,
        eq_guard_history: &guard.eq.history,
        ep_guard_history: &guard.ep.history,
        eq_guard_history_fold_proof: eq_guard_complete.proof(),
        ep_guard_history_fold_proof: ep_guard_complete.proof(),
        eq_guard_merge_fold_proof: eq_guard_merge.proof(),
        ep_guard_merge_fold_proof: ep_guard_merge.proof(),
        eq_mint_authorization_protocol: &mint.pair.eq.protocol,
        ep_mint_authorization_protocol: &mint.pair.ep.protocol,
        eq_mint_authorization_instances: &eq_auth_columns,
        ep_mint_authorization_instances: &ep_auth_columns,
        eq_mint_authorization_proof: &mint.authorization.proof.eq_proof,
        ep_mint_authorization_proof: &mint.authorization.proof.ep_proof,
        eq_mint_authorization_history: &mint.pair.eq.history,
        ep_mint_authorization_history: &mint.pair.ep.history,
        eq_mint_authorization_history_fold_proof: eq_auth_complete.proof(),
        ep_mint_authorization_history_fold_proof: ep_auth_complete.proof(),
        eq_mint_authorization_merge_fold_proof: eq_auth_merge.proof(),
        ep_mint_authorization_merge_fold_proof: ep_auth_merge.proof(),
        eq_mint_protocol: &mint.source.eq_protocol,
        ep_mint_protocol: &mint.source.ep_protocol,
        eq_mint_instances: &eq_mint_columns,
        ep_mint_instances: &ep_mint_columns,
        eq_mint_proof: &mint.source.credit.proof.eq_proof,
        ep_mint_proof: &mint.source.credit.proof.ep_proof,
        eq_mint_history: &mint.source.eq_history,
        ep_mint_history: &mint.source.ep_history,
        eq_mint_history_fold_proof: eq_mint_complete.proof(),
        ep_mint_history_fold_proof: ep_mint_complete.proof(),
        eq_mint_merge_fold_proof: eq_mint_merge.proof(),
        ep_mint_merge_fold_proof: ep_mint_merge.proof(),
    };
    witness.state.validate().unwrap();
    let construction = RecursiveStateConstructionV1::OrdinaryMintIncomingQualification;
    let claim = prove_kagemusha_recursive_state_hash_claim_v1_with_construction(
        &mint.source.hash_eq,
        &mint.source.hash_ep,
        witness.clone(),
        &seed,
        construction,
    )
    .expect("actual active Mint full ordered SHA queue and all current/history proofs");
    let eq_hash_merge = fold_kagemusha_eq_accumulators_v1(
        &eq,
        eq_mint_merge.successor(),
        &claim.eq_complete_history,
        &seed,
    )
    .unwrap();
    let ep_hash_merge = fold_kagemusha_ep_accumulators_v1(
        &ep,
        ep_mint_merge.successor(),
        &claim.ep_complete_history,
        &seed,
    )
    .unwrap();
    witness.hash_claim = Some(
        claim
            .consumer_witness(
                &mint.source.hash_eq,
                &mint.source.hash_ep,
                eq_hash_merge.proof(),
                ep_hash_merge.proof(),
            )
            .unwrap(),
    );
    witness.eq_successor_history = eq_hash_merge.successor();
    witness.ep_successor_history = ep_hash_merge.successor();
    assert!(
        prove_kagemusha_recursive_state_v1(
            &bootstrap.keys.eq,
            &bootstrap.keys.ep,
            witness.clone(),
            &seed
        )
        .is_err(),
        "shipping refusal remains until complete genuine graph qualification"
    );
    let (_, _, eq_audit, ep_audit) =
        build_recursive_generation_pair_v1(&eq, &ep, witness.clone(), construction).unwrap();
    witness.state.eq_deferred_audit = eq_audit;
    witness.state.ep_deferred_audit = ep_audit;
    let generated = prove_kagemusha_recursive_state_v1_with_construction(
        &bootstrap.keys.eq,
        &bootstrap.keys.ep,
        witness.clone(),
        &seed,
        construction,
    )
    .expect("genuine active Mint full State proofs under the same retained zero-State PK family");
    terminally_verify_state_proof(&bootstrap.keys, &generated);
    require_public_mutations_rejected(&bootstrap.keys, &generated);
    let mut changed_parent = bootstrap.public_original.clone();
    let middle = changed_parent.len() / 2;
    changed_parent[middle] ^= 1;
    let mut substituted = witness.clone();
    let outer = substituted
        .ordinary_selection
        .as_ref()
        .unwrap()
        .outer_parent
        .unwrap();
    substituted
        .ordinary_selection
        .as_mut()
        .unwrap()
        .outer_parent = Some(KagemushaOrdinaryRecursiveOuterParentWitnessV1 {
        public_original: Some(&changed_parent),
        ..outer
    });
    require_full_relation_rejection(&eq, &ep, substituted, construction);
    let mut changed_reservation = reservation.clone();
    changed_reservation.source_proof_original_sha256[0] ^= 1;
    let mut substituted = witness.clone();
    let opening = substituted
        .ordinary_selection
        .as_ref()
        .unwrap()
        .incoming_mint
        .unwrap();
    substituted
        .ordinary_selection
        .as_mut()
        .unwrap()
        .incoming_mint = Some(KagemushaOrdinaryRecursiveMintIncomingOpeningV1 {
        reservation: &changed_reservation,
        ..opening
    });
    require_full_relation_rejection(&eq, &ep, substituted, construction);
    let width =
        crate::kagemusha_v1_recursion::state_relation::RECURSIVE_SEMANTIC_PUBLIC_INSTANCE_COUNT;
    let public_original = crate::kagemusha_v1_recursion::KagemushaOrdinaryLineageStateOriginalV1 {
        version: 1,
        projection:
            crate::kagemusha_v1_recursion::KagemushaOrdinaryLineageStateProjectionV1::from_fields(
                generated.eq_transport_public_instances[..width].to_vec(),
                generated.ep_transport_public_instances[..width].to_vec(),
            )
            .unwrap(),
        proof: generated.proof.clone(),
    }
    .canonical_bytes()
    .unwrap();
    assert_eq!(preview.successor.balance, context.amount);
    assert_eq!(preview.successor.logical_sequence, 1);
    assert_eq!(preview.successor.secure_index, 1);
    consumed.install(&replay).unwrap();
    assert_eq!(consumed.root(), preview.successor.consumed_credit_root);
    drop(witness);
    OrdinaryFundedStateForTestingV1 {
        bootstrap,
        state: preview.successor,
        generated,
        public_original,
        _consumed_credits: consumed,
        mint_source: mint,
    }
}

pub(in super::super) fn inactive_wrapper_column<
    F: crate::kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
>(
    eq: &PlonkProtocol<EqAffine>,
    ep: &PlonkProtocol<EpAffine>,
    history: &[u8; crate::kagemusha_v1_recursion::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
) -> Vec<F> {
    let mut values = vec![F::ZERO; 83];
    for (offset, value) in [
        (
            45,
            native_parent_protocol_digest_v1(eq, KagemushaPastaParityV1::Eq).unwrap(),
        ),
        (
            47,
            native_parent_protocol_digest_v1(ep, KagemushaPastaParityV1::Ep).unwrap(),
        ),
    ] {
        values[offset..offset + 2]
            .copy_from_slice(&crate::kagemusha_v1_poseidon::digest_limbs::<F>(value));
    }
    for (cell, bytes) in values[49..].iter_mut().zip(history.chunks_exact(16)) {
        *cell =
            crate::kagemusha_v1_poseidon::from_u128(u128::from_le_bytes(bytes.try_into().unwrap()));
    }
    values
}

pub(in super::super) fn require_public_mutations_rejected(
    keys: &StateKeys,
    generated: &KagemushaGeneratedRecursiveStateProofV1,
) {
    let eq = canonical_kagemusha_eq_parameters_v1();
    let ep = canonical_kagemusha_ep_parameters_v1();
    for index in [0, 1, 20, 30, 64, 91, 92] {
        let mut changed_eq = generated.eq_transport_public_instances.clone();
        changed_eq[index] += Fp::ONE;
        assert!(!decide_eq(
            &eq,
            &keys.eq_transport_protocol,
            &generated.proof.eq_proof,
            &changed_eq
        ));
        let mut changed_ep = generated.ep_transport_public_instances.clone();
        changed_ep[index] += Fq::ONE;
        assert!(!decide_ep(
            &ep,
            &keys.ep_transport_protocol,
            &generated.proof.ep_proof,
            &changed_ep
        ));
    }
}
pub(in super::super) fn require_full_relation_rejection(
    eq: &ParamsIPA<EqAffine>,
    ep: &ParamsIPA<EpAffine>,
    witness: KagemushaRecursiveStateGenerationWitnessV1<'_>,
    construction: RecursiveStateConstructionV1,
) {
    match build_recursive_generation_pair_v1(eq, ep, witness, construction) {
        Err(_) => {}
        Ok((eq_circuit, ep_circuit, _, _)) => {
            let eq_column = eq_circuit.public_instances_for_testing();
            let ep_column = ep_circuit.public_instances_for_testing();
            assert!(
                halo2_proofs::dev::MockProver::run(eq.k(), &eq_circuit, eq_column)
                    .unwrap()
                    .verify()
                    .is_err()
            );
            assert!(
                halo2_proofs::dev::MockProver::run(ep.k(), &ep_circuit, ep_column)
                    .unwrap()
                    .verify()
                    .is_err()
            );
        }
    }
}
