// Focused V4 artifact layout and release-lifecycle wire regressions.

#[test]
fn v4_profiles_bind_exact_four_role_inventory_and_inline_params() {
    let manifest = manifest();
    manifest.validate().expect("valid four-role V4 manifest");
    assert_eq!(KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4.len(), 8);
    for profile in &manifest.profiles {
        assert_eq!(profile.artifacts.len(), 4);
        profile
            .circuit_params
            .validate()
            .expect("valid inline circuit parameters");
        assert_eq!(
            profile
                .artifacts
                .iter()
                .map(|artifact| artifact.kind)
                .collect::<Vec<_>>(),
            vec![
                KagemushaPastaCycleArtifactKindV4::ParamsIpa,
                KagemushaPastaCycleArtifactKindV4::ProvingKey,
                KagemushaPastaCycleArtifactKindV4::VerifyingKey,
                KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
            ]
        );
        assert_eq!(
            profile
                .bootstrap_artifact()
                .expect("bootstrap descriptor")
                .kind,
            KagemushaPastaCycleArtifactKindV4::BootstrapWitness
        );
    }
    let mut tampered = manifest.clone();
    tampered.profiles[0].circuit_params.num_fixed = 0;
    assert!(tampered.validate().is_err());
    let mut separate_params_file = manifest.clone();
    let mut rejected_artifact = separate_params_file.profiles[0].artifacts[0].clone();
    rejected_artifact.file_name = ["step-eq.circuit-", "params.krv4"].concat();
    rejected_artifact.sha256 = digest(b"rejected separate circuit parameters frame");
    rejected_artifact.payload_sha256 = digest(b"rejected separate circuit parameters");
    separate_params_file.profiles[0]
        .artifacts
        .insert(1, rejected_artifact);
    assert!(
        separate_params_file.validate().is_err(),
        "a separate circuit-parameter file must not extend the exact inventory"
    );
    let mut reordered = manifest;
    reordered.profiles[0].artifacts.swap(1, 2);
    assert!(reordered.validate().is_err());
}
include!("kagemusha_release_validation_inline_tests.rs");
include!("kagemusha_promotion_receipt_inline_tests.rs");
#[cfg(feature = "transparent_api")]
pub(super) fn lifecycle_enable_witness_wire_fixture() -> KagemushaV4IssuanceEnableWitnessV1 {
    let fixture = complete_canary_fixture();
    let binding = fixture.receipt.expectations.binding().clone();
    let receipt_bytes =
        norito::encode_canonical(&fixture.receipt.receipt).expect("canonical stage receipt");
    let authorization_bytes =
        norito::encode_canonical(&fixture.authorization).expect("canonical canary authorization");
    let canary_body = &fixture.evidence.body;

    let mut liveness =
        super::kagemusha_post_canary_validator_liveness::tests::signed_liveness_evidence_fixture();
    let mut challenge_body = liveness.body.challenge.body.clone();
    challenge_body.binding = binding;
    challenge_body.canary_anchor = KagemushaV4PostCanaryValidatorLivenessCanaryAnchorV1 {
        schema: KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_CANARY_ANCHOR_SCHEMA.to_owned(),
        version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
        activation_finality_receipt: exact_receipt_bytes(&receipt_bytes),
        canary_authorization: exact_receipt_bytes(&authorization_bytes),
        canary_transaction_intent: canary_body.canary_transaction_intent,
        canary_transaction_wire: canary_body.canary_transaction_wire,
        canary_finalized_height: canary_body.finalized_height,
        canary_finalized_block_hash: canary_body.finalized_block_hash,
        canary_finalized_block_time_unix_ms: 1_700_000_002_000,
    };
    challenge_body.issuer = fixture.receipt.issuer.public_key().clone();
    challenge_body.issued_at_unix_ms = 1_700_000_002_001;
    challenge_body.expires_at_unix_ms = challenge_body.issued_at_unix_ms
        + KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_MAX_INTERVAL_MS;
    let challenge = KagemushaV4PostCanaryValidatorLivenessChallengeV1::try_sign(
        challenge_body,
        &fixture.receipt.issuer,
    )
    .expect("sign cross-bound lifecycle liveness challenge");
    liveness.body.endpoint_challenge = challenge
        .endpoint_challenge()
        .expect("derive cross-bound lifecycle endpoint challenge");
    liveness.body.challenge = challenge;
    liveness.signature = SignatureOf::try_from_hash(
        fixture.receipt.issuer.private_key(),
        liveness.body.signing_hash(),
    )
    .expect("sign cross-bound lifecycle liveness evidence");

    let witness = KagemushaV4IssuanceEnableWitnessV1 {
        schema: KAGEMUSHA_V4_ISSUANCE_ENABLE_WITNESS_SCHEMA_V1.to_owned(),
        version: KAGEMUSHA_V4_RELEASE_LIFECYCLE_VERSION_V1,
        expected_predecessor_lifecycle: exact_receipt_bytes(
            b"canonical staged lifecycle predecessor",
        ),
        transition_id: [0xD1; 32],
        promotion_reservation: fixture.receipt.promotion_reservation,
        stage_expectations: fixture.receipt.expectations_artifact,
        stage_finality_receipt: fixture.receipt.receipt,
        canary_authorization: fixture.authorization,
        canary_evidence: fixture.evidence,
        validator_liveness_evidence: liveness,
    };
    witness.validate().expect("valid lifecycle enable witness");
    witness
}
#[cfg(feature = "transparent_api")]
fn lifecycle_staged_state_fixture() -> KagemushaV4ReleaseLifecycleStateV1 {
    let fixture = complete_receipt_fixture(true);
    let (activation, _, _) = valid_release_activation_fixture();
    let release_record_norito = norito::encode_canonical(&activation.release_record)
        .expect("canonical lifecycle release record");
    KagemushaV4ReleaseLifecycleStateV1 {
        schema: KAGEMUSHA_V4_RELEASE_LIFECYCLE_STATE_SCHEMA_V1.to_owned(),
        version: KAGEMUSHA_V4_RELEASE_LIFECYCLE_VERSION_V1,
        promotion_binding: fixture.expectations.binding().clone(),
        artifact_binding: KagemushaRecursiveSpendArtifactBindingV4 {
            version: KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
            generation: activation.release_record.manifest.generation.clone(),
            manifest_sha256: fixture.expectations.binding().manifest_sha256,
        },
        governance_authority: fixture
            .expectations_artifact
            .body
            .governance_authority
            .clone(),
        stage_transaction_intent: fixture.approved_transaction.hash(),
        staged_at_height: 2,
        staged_at_unix_ms: 1_700_000_000_000,
        release_record_norito: exact_receipt_bytes(&release_record_norito),
        runtime_effective_config_sha256: fixture.expectations_artifact.body.validator_seals[0]
            .body
            .runtime_effective_config
            .consensus_sha256()
            .expect("valid runtime projection digest"),
        device_attestation_policy: fixture
            .promotion_reservation
            .body
            .device_attestation_policy
            .clone(),
        step_eq_verifier_key_id: activation.step_eq_verifier_key_id,
        step_ep_verifier_key_id: activation.step_ep_verifier_key_id,
        verifier_version: activation.step_eq_verifier_record.version,
        phase: KagemushaV4ReleaseLifecyclePhaseV1::Staged,
    }
}
#[cfg(feature = "transparent_api")]
#[test]
fn release_lifecycle_state_binds_the_promoted_release_record() {
    let staged = lifecycle_staged_state_fixture();
    assert_eq!(
        staged.promotion_binding.release_record_sha256, staged.release_record_norito.sha256,
        "the retained record must carry the promoted release-record digest",
    );
    assert_ne!(
        staged.promotion_binding.release_record_sha256,
        staged.promotion_binding.release_policy_source.sha256,
        "the fixture must distinguish the release record from its policy source",
    );
    staged.validate().expect("valid release-record binding");

    let mut changed_release_record = staged;
    changed_release_record.release_record_norito.sha256[0] ^= 0xFF;
    assert_eq!(
        changed_release_record.validate(),
        Err(KagemushaV4ReleaseLifecycleValidationError::InvalidField(
            "lifecycle.release_record_identity",
        )),
        "a retained record digest outside the signed promotion must fail closed",
    );
}
#[cfg(feature = "transparent_api")]
fn validated_staged_lifecycle_fixture() -> (
    KagemushaV4ReleaseLifecycleStateV1,
    KagemushaExactBytesDigestV1,
) {
    let staged = lifecycle_staged_state_fixture();
    staged.validate().expect("valid staged lifecycle");
    assert!(!staged.issuance_enabled());

    let mut changed_policy = staged.clone();
    changed_policy
        .device_attestation_policy
        .revoked_certificate_tbs_sha256
        .push(vec![0xA5; 32]);
    assert!(
        changed_policy.validate().is_err(),
        "the retained redemption policy must match its signed promotion identity"
    );

    let staged_id = staged.exact_bytes_digest().expect("staged state identity");
    let staged_bytes = norito::encode_canonical(&staged).expect("canonical staged state");
    assert_eq!(
        KagemushaV4ReleaseLifecycleStateV1::decode_canonical(&staged_bytes)
            .expect("decode staged state"),
        staged
    );
    let mut trailing = staged_bytes;
    trailing.push(0xA5);
    assert!(KagemushaV4ReleaseLifecycleStateV1::decode_canonical(&trailing).is_err());
    (staged, staged_id)
}

#[cfg(feature = "transparent_api")]
fn validated_enabled_lifecycle_fixture(
    staged: &KagemushaV4ReleaseLifecycleStateV1,
    staged_id: KagemushaExactBytesDigestV1,
) -> (KagemushaV4ReleaseEnabledV1, KagemushaExactBytesDigestV1) {
    let witness = lifecycle_enable_witness_wire_fixture();
    let witness_norito = norito::encode_canonical(&witness).expect("canonical enable witness");
    assert_eq!(
        KagemushaV4IssuanceEnableWitnessV1::decode_canonical(&witness_norito)
            .expect("decode enable witness"),
        witness
    );
    let liveness_norito = norito::encode_canonical(&witness.validator_liveness_evidence)
        .expect("canonical liveness evidence");
    let canary = &witness.canary_evidence.body;
    let liveness = &witness.validator_liveness_evidence.body;
    let enabled = KagemushaV4ReleaseEnabledV1 {
        expected_staged_lifecycle: staged_id,
        transition_id: witness.transition_id,
        enable_witness_norito: exact_receipt_bytes(&witness_norito),
        enable_transaction_intent: HashOf::from_untyped_unchecked(Hash::new(
            b"lifecycle enable transaction",
        )),
        enabled_at_height: canary.finalized_height + 1,
        enabled_at_unix_ms: 1_700_000_003_000,
        validator_liveness_evidence: exact_receipt_bytes(&liveness_norito),
        canary_transaction_intent: canary.canary_transaction_intent,
        canary_finalized_height: canary.finalized_height,
        canary_finalized_block_hash: canary.finalized_block_hash,
        endpoint_challenge: liveness.endpoint_challenge,
        validator_ids: std::array::from_fn(|index| {
            liveness.challenge.body.targets[index].validator_id.clone()
        }),
        observed_tip_heights: [canary.finalized_height; KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT],
        highest_observed_tip_height: canary.finalized_height,
    };
    let mut enabled_state = staged.clone();
    enabled_state.phase = KagemushaV4ReleaseLifecyclePhaseV1::Enabled(Box::new(enabled.clone()));
    enabled_state.validate().expect("valid enabled lifecycle");
    assert!(enabled_state.issuance_enabled());
    let enabled_bytes = norito::encode_canonical(&enabled_state).expect("canonical enabled state");
    assert_eq!(
        KagemushaV4ReleaseLifecycleStateV1::decode_canonical(&enabled_bytes)
            .expect("decode boxed enabled state"),
        enabled_state,
    );
    let enabled_id = enabled_state
        .exact_bytes_digest()
        .expect("enabled state identity");
    (enabled, enabled_id)
}

#[cfg(feature = "transparent_api")]
fn assert_cancelled_lifecycle_state(
    staged: &KagemushaV4ReleaseLifecycleStateV1,
    staged_id: KagemushaExactBytesDigestV1,
) {
    let mut cancelled_state = staged.clone();
    cancelled_state.phase =
        KagemushaV4ReleaseLifecyclePhaseV1::Cancelled(Box::new(KagemushaV4ReleaseCancelledV1 {
            cancellation: KagemushaV4ReleaseCancellationV1 {
                schema: KAGEMUSHA_V4_RELEASE_CANCELLATION_SCHEMA_V1.to_owned(),
                version: KAGEMUSHA_V4_RELEASE_LIFECYCLE_VERSION_V1,
                promotion_id: staged.promotion_binding.promotion_id,
                manifest_sha256: staged.promotion_binding.manifest_sha256,
                expected_predecessor_lifecycle: staged_id,
                transition_id: [0xD2; 32],
                reason: KagemushaV4ReleaseLifecycleReasonV1::GovernanceCancelled,
                evidence: None,
            },
            cancellation_transaction_intent: HashOf::from_untyped_unchecked(Hash::new(
                b"lifecycle cancellation transaction",
            )),
            cancelled_at_height: staged.staged_at_height + 1,
            cancelled_at_unix_ms: staged.staged_at_unix_ms + 1,
        }));
    cancelled_state
        .validate()
        .expect("valid terminal cancellation");
    assert!(!cancelled_state.issuance_enabled());
    let cancelled_bytes =
        norito::encode_canonical(&cancelled_state).expect("canonical cancelled state");
    assert_eq!(
        KagemushaV4ReleaseLifecycleStateV1::decode_canonical(&cancelled_bytes)
            .expect("decode boxed cancelled state"),
        cancelled_state,
    );
}

#[cfg(feature = "transparent_api")]
fn assert_deactivated_lifecycle_state(
    staged: &KagemushaV4ReleaseLifecycleStateV1,
    staged_id: KagemushaExactBytesDigestV1,
    enabled: KagemushaV4ReleaseEnabledV1,
    enabled_id: KagemushaExactBytesDigestV1,
) {
    let canary_finalized_height = enabled.canary_finalized_height;
    let mut deactivated_state = staged.clone();
    deactivated_state.phase = KagemushaV4ReleaseLifecyclePhaseV1::Deactivated(Box::new(
        KagemushaV4ReleaseDeactivatedV1 {
            enabled,
            deactivation: KagemushaV4ReleaseDeactivationV1 {
                schema: KAGEMUSHA_V4_RELEASE_DEACTIVATION_SCHEMA_V1.to_owned(),
                version: KAGEMUSHA_V4_RELEASE_LIFECYCLE_VERSION_V1,
                promotion_id: staged.promotion_binding.promotion_id,
                manifest_sha256: staged.promotion_binding.manifest_sha256,
                expected_predecessor_lifecycle: enabled_id,
                transition_id: [0xD3; 32],
                reason: KagemushaV4ReleaseLifecycleReasonV1::EmergencyDeactivation,
                evidence: Some(exact_receipt_bytes(b"deactivation evidence")),
            },
            deactivation_transaction_intent: HashOf::from_untyped_unchecked(Hash::new(
                b"lifecycle deactivation transaction",
            )),
            deactivated_at_height: canary_finalized_height + 2,
            deactivated_at_unix_ms: 1_700_000_004_000,
        },
    ));
    deactivated_state
        .validate()
        .expect("valid terminal deactivation");
    assert!(!deactivated_state.issuance_enabled());
    assert_eq!(
        deactivated_state.device_attestation_policy, staged.device_attestation_policy,
        "deactivation must retain the exact policy required for full redemption",
    );
    let deactivated_bytes =
        norito::encode_canonical(&deactivated_state).expect("canonical deactivated state");
    assert_eq!(
        KagemushaV4ReleaseLifecycleStateV1::decode_canonical(&deactivated_bytes)
            .expect("decode boxed deactivated state"),
        deactivated_state,
    );

    let KagemushaV4ReleaseLifecyclePhaseV1::Deactivated(ref mut deactivated) =
        deactivated_state.phase
    else {
        unreachable!("fixture phase is deactivated")
    };
    deactivated.deactivation.expected_predecessor_lifecycle = staged_id;
    assert!(
        deactivated_state.validate().is_err(),
        "deactivation cannot name the staged state instead of the exact enabled predecessor"
    );
}

#[cfg(feature = "transparent_api")]
#[test]
fn release_lifecycle_state_enforces_exact_predecessors_and_terminal_phases() {
    let (staged, staged_id) = validated_staged_lifecycle_fixture();
    let (enabled, enabled_id) = validated_enabled_lifecycle_fixture(&staged, staged_id);
    assert_cancelled_lifecycle_state(&staged, staged_id);
    assert_deactivated_lifecycle_state(&staged, staged_id, enabled, enabled_id);
}
#[test]
fn v4_artifact_contract_source_guard_is_exhaustive() {
    fn canonical_index(kind: KagemushaPastaCycleArtifactKindV4) -> usize {
        match kind {
            KagemushaPastaCycleArtifactKindV4::ParamsIpa => 0,
            KagemushaPastaCycleArtifactKindV4::ProvingKey => 1,
            KagemushaPastaCycleArtifactKindV4::VerifyingKey => 2,
            KagemushaPastaCycleArtifactKindV4::BootstrapWitness => 3,
        }
    }
    let kinds = [
        KagemushaPastaCycleArtifactKindV4::ParamsIpa,
        KagemushaPastaCycleArtifactKindV4::ProvingKey,
        KagemushaPastaCycleArtifactKindV4::VerifyingKey,
        KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
    ];
    assert_eq!(kinds.map(canonical_index), [0, 1, 2, 3]);
    assert_eq!(KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4.len(), 8);
    assert!(
        manifest()
            .profiles
            .iter()
            .all(|profile| profile.artifacts.len() == 4)
    );
    let source = concat!(
        include_str!("mod.rs"),
        include_str!("kagemusha_model.rs"),
        include_str!("kagemusha_release_verifier.rs")
    );
    assert!(source.contains("KagemushaPastaCycleFramedArtifactHeaderV4"));
    for forbidden in [
        concat!("Circuit", "Params,"),
        concat!("CIRCUIT_", "PARAMS_FILE_NAME_V4"),
        concat!("circuit-", "params.krv4"),
    ] {
        assert!(
            !source.contains(forbidden),
            "rejected V4 artifact contract marker is present: {forbidden}"
        );
    }
}
#[test]
fn v4_envelope_uses_verifying_key_role_at_canonical_index() {
    let manifest = manifest();
    manifest.validate().expect("valid V4 manifest");
    let [vesta_profile, pallas_profile] = manifest.profiles.as_slice() else {
        panic!("test manifest must have Eq/Ep profiles");
    };
    let mut state_limbs = vec![0; KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V5];
    state_limbs[0] = KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V5;
    let mut envelope = KagemushaPastaCycleProofEnvelopeV4 {
        version: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V4,
        proof_backend: manifest.proof_backend.clone(),
        transcript_profile: manifest.transcript_profile.clone(),
        step_eq_circuit_id: vesta_profile.circuit_id.clone(),
        step_ep_circuit_id: pallas_profile.circuit_id.clone(),
        artifact_generation: manifest.generation.clone(),
        manifest_sha256: digest(&norito::encode_canonical(&manifest).expect("canonical manifest")),
        step_eq_parameter_generation: vesta_profile.parameter_generation.clone(),
        step_ep_parameter_generation: pallas_profile.parameter_generation.clone(),
        step_eq_circuit_params_sha256: vesta_profile
            .circuit_params
            .sha256()
            .expect("Eq params identity"),
        step_ep_circuit_params_sha256: pallas_profile
            .circuit_params
            .sha256()
            .expect("Ep params identity"),
        step_eq_verifier_key_sha256: vesta_profile.artifacts[2].payload_sha256,
        step_ep_verifier_key_sha256: pallas_profile.artifacts[2].payload_sha256,
        state_boundary: KagemushaRecursiveSpendStateBoundaryV5 {
            layout_version: KAGEMUSHA_RECURSIVE_SPEND_STATE_BOUNDARY_VERSION_V5,
            state_limbs,
        },
        proof: ProofBox::new(manifest.proof_backend.clone(), vec![0xA5]),
    };
    envelope
        .validate_against_manifest(&manifest)
        .expect("V4 envelope binds verifying-key role at index two");
    envelope.step_eq_verifier_key_sha256 = vesta_profile.artifacts[1].payload_sha256;
    assert!(envelope.validate_against_manifest(&manifest).is_err());
}
