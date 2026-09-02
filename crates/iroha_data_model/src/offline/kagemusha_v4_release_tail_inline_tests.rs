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
    lifecycle_enable_witness_wire_fixture_for_staged(&lifecycle_staged_state_fixture())
}

#[cfg(feature = "transparent_api")]
#[expect(
    clippy::too_many_lines,
    reason = "the fixture verifies one complete staged, canary, and four-validator liveness evidence chain"
)]
fn lifecycle_enable_witness_wire_fixture_for_staged(
    staged: &KagemushaV4ReleaseLifecycleStateV1,
) -> KagemushaV4IssuanceEnableWitnessV1 {
    let expected_predecessor_lifecycle = staged
        .exact_bytes_digest()
        .expect("canonical staged lifecycle predecessor");
    let fixture = complete_canary_fixture();
    let receipt_bytes =
        norito::encode_canonical(&fixture.receipt.receipt).expect("canonical stage receipt");
    let authorization_bytes =
        norito::encode_canonical(&fixture.authorization).expect("canonical canary authorization");
    let verified_canary = fixture
        .evidence
        .verify_exact(
            &fixture.evidence_bytes,
            &fixture.authorization,
            &fixture.authorization_bytes,
            &fixture.receipt.expectations,
            &fixture.receipt.receipt,
            &receipt_bytes,
        )
        .expect("fully verified lifecycle canary evidence");
    let canary_finality_proof = fixture
        .evidence
        .body
        .finality_proof_chain
        .last()
        .expect("lifecycle canary finality proof");
    let canary_anchor = KagemushaV4PostCanaryValidatorLivenessCanaryAnchorV1 {
        schema: KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_CANARY_ANCHOR_SCHEMA.to_owned(),
        version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
        activation_finality_receipt: verified_canary.activation_finality_receipt(),
        canary_authorization: verified_canary.authorization_identity(),
        canary_transaction_intent: verified_canary.canary_transaction_intent(),
        canary_transaction_wire: verified_canary.canary_transaction_wire(),
        canary_finalized_height: verified_canary.finalized_height(),
        canary_finalized_block_hash: verified_canary.finalized_block_hash(),
        canary_finalized_block_time_unix_ms: u64::try_from(
            canary_finality_proof
                .block_header
                .creation_time()
                .as_millis(),
        )
        .expect("lifecycle canary block time fits u64"),
    };
    let liveness = super::kagemusha_post_canary_validator_liveness::post_canary_validator_liveness_tests::signed_liveness_evidence_for_fixture(
        &fixture.receipt.expectations,
        &verified_canary,
        &canary_anchor,
        canary_finality_proof,
        &fixture.receipt.issuer,
        &fixture.receipt.validator_keys,
    );

    let witness = KagemushaV4IssuanceEnableWitnessV1 {
        schema: KAGEMUSHA_V4_ISSUANCE_ENABLE_WITNESS_SCHEMA_V1.to_owned(),
        version: KAGEMUSHA_V4_RELEASE_LIFECYCLE_VERSION_V1,
        expected_predecessor_lifecycle,
        transition_id: [0xD1; 32],
        promotion_reservation: fixture.receipt.promotion_reservation,
        stage_expectations: fixture.receipt.expectations_artifact,
        stage_finality_receipt: fixture.receipt.receipt,
        canary_authorization: fixture.authorization,
        canary_evidence: fixture.evidence,
        validator_liveness_evidence: liveness,
    };
    witness.validate().expect("valid lifecycle enable witness");
    verified_lifecycle_liveness(&witness);
    assert_eq!(
        witness.stage_expectations.body.binding,
        staged.promotion_binding
    );
    assert_eq!(
        witness.stage_expectations.body.governance_authority,
        staged.governance_authority,
    );
    assert_eq!(
        witness.stage_expectations.body.validator_seals[0]
            .body
            .runtime_effective_config
            .consensus_sha256()
            .expect("valid lifecycle runtime projection"),
        staged.runtime_effective_config_sha256,
    );
    assert_eq!(
        witness
            .stage_finality_receipt
            .body
            .activation_transaction_intent,
        staged.stage_transaction_intent,
    );
    let stage_finality_proof = witness
        .stage_finality_receipt
        .body
        .finality_proof_chain
        .last()
        .expect("lifecycle stage finality proof");
    assert_eq!(
        stage_finality_proof.finality_artifact.height,
        staged.staged_at_height,
    );
    assert_eq!(
        u64::try_from(
            stage_finality_proof
                .block_header
                .creation_time()
                .as_millis()
        )
        .expect("lifecycle stage block time fits u64"),
        staged.staged_at_unix_ms,
    );
    witness
}

#[cfg(feature = "transparent_api")]
fn verified_lifecycle_liveness(
    witness: &KagemushaV4IssuanceEnableWitnessV1,
) -> KagemushaV4VerifiedPostCanaryValidatorLivenessEvidenceV1 {
    let reservation_bytes = norito::encode_canonical(&witness.promotion_reservation)
        .expect("canonical lifecycle promotion reservation");
    let expectations_bytes = norito::encode_canonical(&witness.stage_expectations)
        .expect("canonical lifecycle stage expectations");
    let receipt_bytes = norito::encode_canonical(&witness.stage_finality_receipt)
        .expect("canonical lifecycle stage receipt");
    let authorization_bytes = norito::encode_canonical(&witness.canary_authorization)
        .expect("canonical lifecycle canary authorization");
    let canary_evidence_bytes = norito::encode_canonical(&witness.canary_evidence)
        .expect("canonical lifecycle canary evidence");
    let liveness_bytes = norito::encode_canonical(&witness.validator_liveness_evidence)
        .expect("canonical lifecycle liveness evidence");
    let expectations = witness
        .stage_expectations
        .verify_exact(
            &expectations_bytes,
            &witness.promotion_reservation.body.promotion_controller,
            &reservation_bytes,
        )
        .expect("verify lifecycle stage expectations");
    witness
        .stage_finality_receipt
        .verify(&expectations)
        .expect("verify lifecycle stage receipt");
    let verified_canary = witness
        .canary_evidence
        .verify_exact(
            &canary_evidence_bytes,
            &witness.canary_authorization,
            &authorization_bytes,
            &expectations,
            &witness.stage_finality_receipt,
            &receipt_bytes,
        )
        .expect("verify lifecycle canary evidence");
    let canary_finality_proof = witness
        .canary_evidence
        .body
        .finality_proof_chain
        .last()
        .expect("lifecycle canary finality proof");
    witness
        .validator_liveness_evidence
        .verify_exact(
            &liveness_bytes,
            &expectations,
            &verified_canary,
            &witness
                .validator_liveness_evidence
                .body
                .challenge
                .body
                .canary_anchor,
            canary_finality_proof,
        )
        .expect("verify complete lifecycle liveness evidence")
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
        staged_at_unix_ms: 1_699_999_999_000,
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
    assert_invalid_lifecycle_state(&changed_release_record, "lifecycle.release_record_identity");
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
    assert_invalid_lifecycle_state(
        &changed_policy,
        "lifecycle.device_attestation_policy_identity",
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
    assert_eq!(
        KagemushaV4ReleaseLifecycleStateV1::decode_canonical(&trailing),
        Err(KagemushaV4ReleaseLifecycleValidationError::Decode),
    );
    (staged, staged_id)
}

#[cfg(feature = "transparent_api")]
fn validated_enabled_lifecycle_fixture(
    staged: &KagemushaV4ReleaseLifecycleStateV1,
    staged_id: KagemushaExactBytesDigestV1,
) -> (
    KagemushaV4ReleaseEnabledV1,
    KagemushaExactBytesDigestV1,
    KagemushaV4IssuanceEnableWitnessV1,
) {
    let witness = lifecycle_enable_witness_wire_fixture_for_staged(staged);
    assert_eq!(witness.expected_predecessor_lifecycle, staged_id);
    let verified_liveness = verified_lifecycle_liveness(&witness);
    let witness_norito = norito::encode_canonical(&witness).expect("canonical enable witness");
    assert_eq!(
        KagemushaV4IssuanceEnableWitnessV1::decode_canonical(&witness_norito)
            .expect("decode enable witness"),
        witness
    );
    let liveness_norito = norito::encode_canonical(&witness.validator_liveness_evidence)
        .expect("canonical liveness evidence");
    let enabled = KagemushaV4ReleaseEnabledV1 {
        expected_staged_lifecycle: staged_id,
        transition_id: witness.transition_id,
        enable_witness_norito: exact_receipt_bytes(&witness_norito),
        enable_transaction_intent: HashOf::from_untyped_unchecked(Hash::new(
            b"lifecycle enable transaction",
        )),
        enabled_at_height: verified_liveness.highest_observed_tip_height() + 1,
        enabled_at_unix_ms: 1_700_000_003_000,
        validator_liveness_evidence: exact_receipt_bytes(&liveness_norito),
        canary_transaction_intent: verified_liveness.canary_transaction_intent(),
        canary_finalized_height: verified_liveness.canary_finalized_height(),
        canary_finalized_block_hash: verified_liveness.canary_finalized_block_hash(),
        endpoint_challenge: verified_liveness.endpoint_challenge(),
        validator_ids: verified_liveness.validator_ids().clone(),
        observed_tip_heights: *verified_liveness.observed_tip_heights(),
        highest_observed_tip_height: verified_liveness.highest_observed_tip_height(),
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
    (enabled, enabled_id, witness)
}

#[cfg(feature = "transparent_api")]
fn assert_invalid_lifecycle_state(state: &KagemushaV4ReleaseLifecycleStateV1, field: &'static str) {
    let expected = KagemushaV4ReleaseLifecycleValidationError::InvalidField(field);
    assert_eq!(state.validate(), Err(expected));
    let bytes = norito::encode_canonical(state).expect("encode invalid lifecycle fixture");
    assert_eq!(
        KagemushaV4ReleaseLifecycleStateV1::decode_canonical(&bytes),
        Err(expected),
        "canonical decoding must reapply lifecycle validation",
    );
}

#[cfg(feature = "transparent_api")]
fn assert_invalid_enable_witness(
    witness: &KagemushaV4IssuanceEnableWitnessV1,
    field: &'static str,
) {
    let expected = KagemushaV4ReleaseLifecycleValidationError::InvalidField(field);
    assert_eq!(witness.validate(), Err(expected));
    let bytes = norito::encode_canonical(witness).expect("encode invalid enable witness fixture");
    assert_eq!(
        KagemushaV4IssuanceEnableWitnessV1::decode_canonical(&bytes),
        Err(expected),
        "canonical decoding must reapply enable-witness validation",
    );
}

#[cfg(feature = "transparent_api")]
fn cancelled_lifecycle_state(
    staged: &KagemushaV4ReleaseLifecycleStateV1,
    staged_id: KagemushaExactBytesDigestV1,
) -> KagemushaV4ReleaseLifecycleStateV1 {
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
}

#[cfg(feature = "transparent_api")]
fn assert_cancelled_lifecycle_state(
    staged: &KagemushaV4ReleaseLifecycleStateV1,
    staged_id: KagemushaExactBytesDigestV1,
) {
    let cancelled_state = cancelled_lifecycle_state(staged, staged_id);
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
fn deactivated_lifecycle_state(
    staged: &KagemushaV4ReleaseLifecycleStateV1,
    enabled: KagemushaV4ReleaseEnabledV1,
    enabled_id: KagemushaExactBytesDigestV1,
) -> KagemushaV4ReleaseLifecycleStateV1 {
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
}

#[cfg(feature = "transparent_api")]
fn assert_deactivated_lifecycle_state(
    staged: &KagemushaV4ReleaseLifecycleStateV1,
    staged_id: KagemushaExactBytesDigestV1,
    enabled: KagemushaV4ReleaseEnabledV1,
    enabled_id: KagemushaExactBytesDigestV1,
) {
    let mut deactivated_state = deactivated_lifecycle_state(staged, enabled, enabled_id);
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
    assert_invalid_lifecycle_state(&deactivated_state, "lifecycle.deactivated_predecessor");
}

#[cfg(feature = "transparent_api")]
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the predecessor and replay-id matrix asserts exact canonical error precedence for every phase"
)]
fn release_lifecycle_state_enforces_exact_predecessors_and_transition_ids() {
    let (staged, staged_id) = validated_staged_lifecycle_fixture();
    let (enabled, enabled_id, witness) = validated_enabled_lifecycle_fixture(&staged, staged_id);
    assert_cancelled_lifecycle_state(&staged, staged_id);
    assert_deactivated_lifecycle_state(&staged, staged_id, enabled.clone(), enabled_id);
    let zero_predecessor = KagemushaExactBytesDigestV1 {
        byte_len: 0,
        sha256: [0; 32],
    };

    let mut zero_witness_predecessor = witness.clone();
    zero_witness_predecessor.expected_predecessor_lifecycle = zero_predecessor;
    assert_invalid_enable_witness(
        &zero_witness_predecessor,
        "enable_witness.expected_predecessor_lifecycle",
    );

    let mut zero_witness_transition_id = witness;
    zero_witness_transition_id.transition_id = [0; 32];
    assert_invalid_enable_witness(&zero_witness_transition_id, "enable_witness.transition_id");

    let mut enabled_state = staged.clone();
    enabled_state.phase = KagemushaV4ReleaseLifecyclePhaseV1::Enabled(Box::new(enabled.clone()));

    let mut wrong_enabled_predecessor = enabled_state.clone();
    let KagemushaV4ReleaseLifecyclePhaseV1::Enabled(wrong_enabled) =
        &mut wrong_enabled_predecessor.phase
    else {
        unreachable!("fixture phase is enabled")
    };
    wrong_enabled.expected_staged_lifecycle =
        exact_receipt_bytes(b"wrong enabled staged predecessor");
    assert_invalid_lifecycle_state(&wrong_enabled_predecessor, "lifecycle.enabled_transition");

    let mut zero_enabled_predecessor = enabled_state.clone();
    let KagemushaV4ReleaseLifecyclePhaseV1::Enabled(zero_enabled) =
        &mut zero_enabled_predecessor.phase
    else {
        unreachable!("fixture phase is enabled")
    };
    zero_enabled.expected_staged_lifecycle = zero_predecessor;
    assert_invalid_lifecycle_state(
        &zero_enabled_predecessor,
        "enabled.expected_staged_lifecycle",
    );

    let mut zero_enabled_transition_id = enabled_state;
    let KagemushaV4ReleaseLifecyclePhaseV1::Enabled(zero_enabled) =
        &mut zero_enabled_transition_id.phase
    else {
        unreachable!("fixture phase is enabled")
    };
    zero_enabled.transition_id = [0; 32];
    assert_invalid_lifecycle_state(&zero_enabled_transition_id, "enabled.transition_id");

    let mut wrong_cancelled_predecessor = cancelled_lifecycle_state(&staged, staged_id);
    let KagemushaV4ReleaseLifecyclePhaseV1::Cancelled(cancelled) =
        &mut wrong_cancelled_predecessor.phase
    else {
        unreachable!("fixture phase is cancelled")
    };
    cancelled.cancellation.expected_predecessor_lifecycle =
        exact_receipt_bytes(b"wrong cancelled staged predecessor");
    assert_invalid_lifecycle_state(
        &wrong_cancelled_predecessor,
        "lifecycle.cancelled_transition",
    );

    let mut zero_cancelled_predecessor = cancelled_lifecycle_state(&staged, staged_id);
    let KagemushaV4ReleaseLifecyclePhaseV1::Cancelled(cancelled) =
        &mut zero_cancelled_predecessor.phase
    else {
        unreachable!("fixture phase is cancelled")
    };
    cancelled.cancellation.expected_predecessor_lifecycle = zero_predecessor;
    assert_invalid_lifecycle_state(&zero_cancelled_predecessor, "cancellation");

    let mut zero_cancelled_transition_id = cancelled_lifecycle_state(&staged, staged_id);
    let KagemushaV4ReleaseLifecyclePhaseV1::Cancelled(cancelled) =
        &mut zero_cancelled_transition_id.phase
    else {
        unreachable!("fixture phase is cancelled")
    };
    cancelled.cancellation.transition_id = [0; 32];
    assert_invalid_lifecycle_state(&zero_cancelled_transition_id, "cancellation");

    let deactivated_state = deactivated_lifecycle_state(&staged, enabled, enabled_id);
    let mut wrong_deactivated_staged_predecessor = deactivated_state.clone();
    let KagemushaV4ReleaseLifecyclePhaseV1::Deactivated(deactivated) =
        &mut wrong_deactivated_staged_predecessor.phase
    else {
        unreachable!("fixture phase is deactivated")
    };
    deactivated.enabled.expected_staged_lifecycle =
        exact_receipt_bytes(b"wrong deactivated staged predecessor");
    assert_invalid_lifecycle_state(
        &wrong_deactivated_staged_predecessor,
        "lifecycle.deactivated_transition",
    );

    let mut zero_deactivated_staged_predecessor = deactivated_state.clone();
    let KagemushaV4ReleaseLifecyclePhaseV1::Deactivated(deactivated) =
        &mut zero_deactivated_staged_predecessor.phase
    else {
        unreachable!("fixture phase is deactivated")
    };
    deactivated.enabled.expected_staged_lifecycle = zero_predecessor;
    assert_invalid_lifecycle_state(
        &zero_deactivated_staged_predecessor,
        "enabled.expected_staged_lifecycle",
    );

    let mut zero_deactivated_predecessor = deactivated_state.clone();
    let KagemushaV4ReleaseLifecyclePhaseV1::Deactivated(deactivated) =
        &mut zero_deactivated_predecessor.phase
    else {
        unreachable!("fixture phase is deactivated")
    };
    deactivated.deactivation.expected_predecessor_lifecycle = zero_predecessor;
    assert_invalid_lifecycle_state(&zero_deactivated_predecessor, "deactivation");

    let mut zero_embedded_enabled_transition_id = deactivated_state.clone();
    let KagemushaV4ReleaseLifecyclePhaseV1::Deactivated(deactivated) =
        &mut zero_embedded_enabled_transition_id.phase
    else {
        unreachable!("fixture phase is deactivated")
    };
    deactivated.enabled.transition_id = [0; 32];
    assert_invalid_lifecycle_state(
        &zero_embedded_enabled_transition_id,
        "enabled.transition_id",
    );

    let mut zero_deactivation_transition_id = deactivated_state.clone();
    let KagemushaV4ReleaseLifecyclePhaseV1::Deactivated(deactivated) =
        &mut zero_deactivation_transition_id.phase
    else {
        unreachable!("fixture phase is deactivated")
    };
    deactivated.deactivation.transition_id = [0; 32];
    assert_invalid_lifecycle_state(&zero_deactivation_transition_id, "deactivation");

    let mut reused_deactivation_transition_id = deactivated_state;
    let KagemushaV4ReleaseLifecyclePhaseV1::Deactivated(deactivated) =
        &mut reused_deactivation_transition_id.phase
    else {
        unreachable!("fixture phase is deactivated")
    };
    deactivated.deactivation.transition_id = deactivated.enabled.transition_id;
    assert_invalid_lifecycle_state(&reused_deactivation_transition_id, "deactivated");
}

#[cfg(feature = "transparent_api")]
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the chronology matrix keeps every strict lifecycle boundary and canonical error field together"
)]
fn release_lifecycle_state_requires_strict_phase_chronology() {
    let (staged, staged_id) = validated_staged_lifecycle_fixture();
    let (enabled, enabled_id, _) = validated_enabled_lifecycle_fixture(&staged, staged_id);

    let mut zero_staged_height = staged.clone();
    zero_staged_height.staged_at_height = 0;
    assert_invalid_lifecycle_state(&zero_staged_height, "lifecycle");

    let mut zero_staged_time = staged.clone();
    zero_staged_time.staged_at_unix_ms = 0;
    assert_invalid_lifecycle_state(&zero_staged_time, "lifecycle");

    let mut enabled_state = staged.clone();
    enabled_state.phase = KagemushaV4ReleaseLifecyclePhaseV1::Enabled(Box::new(enabled.clone()));
    enabled_state.validate().expect("valid enabled chronology");

    let mut zero_canary_height = enabled_state.clone();
    let KagemushaV4ReleaseLifecyclePhaseV1::Enabled(zero_canary_height_enabled) =
        &mut zero_canary_height.phase
    else {
        unreachable!("fixture phase is enabled")
    };
    zero_canary_height_enabled.canary_finalized_height = 0;
    assert_invalid_lifecycle_state(&zero_canary_height, "enabled");

    let mut zero_enabled_height = enabled_state.clone();
    let KagemushaV4ReleaseLifecyclePhaseV1::Enabled(zero_enabled_height_enabled) =
        &mut zero_enabled_height.phase
    else {
        unreachable!("fixture phase is enabled")
    };
    zero_enabled_height_enabled.enabled_at_height = 0;
    assert_invalid_lifecycle_state(&zero_enabled_height, "enabled");

    let mut zero_enabled_time = enabled_state.clone();
    let KagemushaV4ReleaseLifecyclePhaseV1::Enabled(zero_enabled_time_enabled) =
        &mut zero_enabled_time.phase
    else {
        unreachable!("fixture phase is enabled")
    };
    zero_enabled_time_enabled.enabled_at_unix_ms = 0;
    assert_invalid_lifecycle_state(&zero_enabled_time, "enabled");

    let mut equal_canary_height = enabled_state.clone();
    let KagemushaV4ReleaseLifecyclePhaseV1::Enabled(equal_canary_height_enabled) =
        &mut equal_canary_height.phase
    else {
        unreachable!("fixture phase is enabled")
    };
    equal_canary_height_enabled.canary_finalized_height = staged.staged_at_height;
    assert_invalid_lifecycle_state(&equal_canary_height, "lifecycle.enabled_transition");

    let mut equal_enabled_height = enabled_state.clone();
    let KagemushaV4ReleaseLifecyclePhaseV1::Enabled(equal_enabled_height_enabled) =
        &mut equal_enabled_height.phase
    else {
        unreachable!("fixture phase is enabled")
    };
    equal_enabled_height_enabled.enabled_at_height =
        equal_enabled_height_enabled.canary_finalized_height;
    assert_invalid_lifecycle_state(&equal_enabled_height, "enabled");

    let mut equal_enabled_time = enabled_state;
    let KagemushaV4ReleaseLifecyclePhaseV1::Enabled(equal_enabled_time_enabled) =
        &mut equal_enabled_time.phase
    else {
        unreachable!("fixture phase is enabled")
    };
    equal_enabled_time_enabled.enabled_at_unix_ms = staged.staged_at_unix_ms;
    assert_invalid_lifecycle_state(&equal_enabled_time, "lifecycle.enabled_transition");

    let mut zero_cancelled_time = cancelled_lifecycle_state(&staged, staged_id);
    let KagemushaV4ReleaseLifecyclePhaseV1::Cancelled(zero_cancelled_time_cancelled) =
        &mut zero_cancelled_time.phase
    else {
        unreachable!("fixture phase is cancelled")
    };
    zero_cancelled_time_cancelled.cancelled_at_unix_ms = 0;
    assert_invalid_lifecycle_state(&zero_cancelled_time, "cancelled");

    let mut zero_cancelled_height = cancelled_lifecycle_state(&staged, staged_id);
    let KagemushaV4ReleaseLifecyclePhaseV1::Cancelled(zero_cancelled_height_cancelled) =
        &mut zero_cancelled_height.phase
    else {
        unreachable!("fixture phase is cancelled")
    };
    zero_cancelled_height_cancelled.cancelled_at_height = 0;
    assert_invalid_lifecycle_state(&zero_cancelled_height, "cancelled");

    let mut equal_cancelled_time = cancelled_lifecycle_state(&staged, staged_id);
    let KagemushaV4ReleaseLifecyclePhaseV1::Cancelled(equal_cancelled_time_cancelled) =
        &mut equal_cancelled_time.phase
    else {
        unreachable!("fixture phase is cancelled")
    };
    equal_cancelled_time_cancelled.cancelled_at_unix_ms = staged.staged_at_unix_ms;
    assert_invalid_lifecycle_state(&equal_cancelled_time, "lifecycle.cancelled_transition");

    let mut equal_cancelled_height = cancelled_lifecycle_state(&staged, staged_id);
    let KagemushaV4ReleaseLifecyclePhaseV1::Cancelled(equal_cancelled_height_cancelled) =
        &mut equal_cancelled_height.phase
    else {
        unreachable!("fixture phase is cancelled")
    };
    equal_cancelled_height_cancelled.cancelled_at_height = staged.staged_at_height;
    assert_invalid_lifecycle_state(&equal_cancelled_height, "lifecycle.cancelled_transition");

    let deactivated_state = deactivated_lifecycle_state(&staged, enabled.clone(), enabled_id);
    deactivated_state
        .validate()
        .expect("valid deactivated chronology");

    let mut zero_nested_canary_height = deactivated_state.clone();
    let KagemushaV4ReleaseLifecyclePhaseV1::Deactivated(zero_canary_height_deactivated) =
        &mut zero_nested_canary_height.phase
    else {
        unreachable!("fixture phase is deactivated")
    };
    zero_canary_height_deactivated
        .enabled
        .canary_finalized_height = 0;
    assert_invalid_lifecycle_state(&zero_nested_canary_height, "enabled");

    let mut zero_nested_enabled_height = deactivated_state.clone();
    let KagemushaV4ReleaseLifecyclePhaseV1::Deactivated(zero_enabled_height_deactivated) =
        &mut zero_nested_enabled_height.phase
    else {
        unreachable!("fixture phase is deactivated")
    };
    zero_enabled_height_deactivated.enabled.enabled_at_height = 0;
    assert_invalid_lifecycle_state(&zero_nested_enabled_height, "enabled");

    let mut zero_nested_enabled_time = deactivated_state.clone();
    let KagemushaV4ReleaseLifecyclePhaseV1::Deactivated(zero_enabled_time_deactivated) =
        &mut zero_nested_enabled_time.phase
    else {
        unreachable!("fixture phase is deactivated")
    };
    zero_enabled_time_deactivated.enabled.enabled_at_unix_ms = 0;
    assert_invalid_lifecycle_state(&zero_nested_enabled_time, "enabled");

    let mut zero_deactivated_time = deactivated_state.clone();
    let KagemushaV4ReleaseLifecyclePhaseV1::Deactivated(zero_deactivated_time_deactivated) =
        &mut zero_deactivated_time.phase
    else {
        unreachable!("fixture phase is deactivated")
    };
    zero_deactivated_time_deactivated.deactivated_at_unix_ms = 0;
    assert_invalid_lifecycle_state(&zero_deactivated_time, "deactivated");

    let mut zero_deactivated_height = deactivated_state.clone();
    let KagemushaV4ReleaseLifecyclePhaseV1::Deactivated(zero_deactivated_height_deactivated) =
        &mut zero_deactivated_height.phase
    else {
        unreachable!("fixture phase is deactivated")
    };
    zero_deactivated_height_deactivated.deactivated_at_height = 0;
    assert_invalid_lifecycle_state(&zero_deactivated_height, "deactivated");

    let mut equal_deactivated_canary_height = deactivated_state.clone();
    let KagemushaV4ReleaseLifecyclePhaseV1::Deactivated(equal_canary_height_deactivated) =
        &mut equal_deactivated_canary_height.phase
    else {
        unreachable!("fixture phase is deactivated")
    };
    equal_canary_height_deactivated
        .enabled
        .canary_finalized_height = staged.staged_at_height;
    assert_invalid_lifecycle_state(
        &equal_deactivated_canary_height,
        "lifecycle.deactivated_transition",
    );

    let mut equal_nested_enabled_height = deactivated_state.clone();
    let KagemushaV4ReleaseLifecyclePhaseV1::Deactivated(equal_enabled_height_deactivated) =
        &mut equal_nested_enabled_height.phase
    else {
        unreachable!("fixture phase is deactivated")
    };
    equal_enabled_height_deactivated.enabled.enabled_at_height = equal_enabled_height_deactivated
        .enabled
        .canary_finalized_height;
    assert_invalid_lifecycle_state(&equal_nested_enabled_height, "enabled");

    let mut equal_nested_enabled_time = deactivated_state.clone();
    let KagemushaV4ReleaseLifecyclePhaseV1::Deactivated(equal_enabled_time_deactivated) =
        &mut equal_nested_enabled_time.phase
    else {
        unreachable!("fixture phase is deactivated")
    };
    equal_enabled_time_deactivated.enabled.enabled_at_unix_ms = staged.staged_at_unix_ms;
    assert_invalid_lifecycle_state(
        &equal_nested_enabled_time,
        "lifecycle.deactivated_transition",
    );

    let mut equal_deactivated_time = deactivated_state;
    let KagemushaV4ReleaseLifecyclePhaseV1::Deactivated(equal_deactivated_time_deactivated) =
        &mut equal_deactivated_time.phase
    else {
        unreachable!("fixture phase is deactivated")
    };
    equal_deactivated_time_deactivated.deactivated_at_unix_ms = equal_deactivated_time_deactivated
        .enabled
        .enabled_at_unix_ms;
    assert_invalid_lifecycle_state(&equal_deactivated_time, "deactivated");

    let mut equal_deactivated_height = deactivated_lifecycle_state(&staged, enabled, enabled_id);
    let KagemushaV4ReleaseLifecyclePhaseV1::Deactivated(equal_deactivated_height_deactivated) =
        &mut equal_deactivated_height.phase
    else {
        unreachable!("fixture phase is deactivated")
    };
    equal_deactivated_height_deactivated.deactivated_at_height =
        equal_deactivated_height_deactivated
            .enabled
            .enabled_at_height;
    assert_invalid_lifecycle_state(&equal_deactivated_height, "deactivated");
}

#[cfg(feature = "transparent_api")]
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the intent matrix covers every nonzero and pairwise-distinct retained lifecycle identity"
)]
fn release_lifecycle_state_requires_nonzero_distinct_transaction_intents() {
    let (staged, staged_id) = validated_staged_lifecycle_fixture();
    let (enabled, enabled_id, _) = validated_enabled_lifecycle_fixture(&staged, staged_id);

    let zero_intent: HashOf<SignedTransaction> =
        HashOf::from_untyped_unchecked(Hash::prehashed([0; Hash::LENGTH]));

    let mut zero_stage = staged.clone();
    zero_stage.stage_transaction_intent = zero_intent;
    assert_invalid_lifecycle_state(&zero_stage, "lifecycle");

    let mut enabled_state = staged.clone();
    enabled_state.phase = KagemushaV4ReleaseLifecyclePhaseV1::Enabled(Box::new(enabled.clone()));

    let mut zero_enable = enabled_state.clone();
    let KagemushaV4ReleaseLifecyclePhaseV1::Enabled(zero_enable_enabled) = &mut zero_enable.phase
    else {
        unreachable!("fixture phase is enabled")
    };
    zero_enable_enabled.enable_transaction_intent = zero_intent;
    assert_invalid_lifecycle_state(&zero_enable, "enabled");

    let mut zero_canary = enabled_state.clone();
    let KagemushaV4ReleaseLifecyclePhaseV1::Enabled(zero_canary_enabled) = &mut zero_canary.phase
    else {
        unreachable!("fixture phase is enabled")
    };
    zero_canary_enabled.canary_transaction_intent = zero_intent;
    assert_invalid_lifecycle_state(&zero_canary, "enabled");

    let mut enable_reuses_stage = enabled_state.clone();
    let KagemushaV4ReleaseLifecyclePhaseV1::Enabled(enable_reuses_stage_enabled) =
        &mut enable_reuses_stage.phase
    else {
        unreachable!("fixture phase is enabled")
    };
    enable_reuses_stage_enabled.enable_transaction_intent = staged.stage_transaction_intent;
    assert_invalid_lifecycle_state(&enable_reuses_stage, "lifecycle.enabled_transition");

    let mut canary_reuses_stage = enabled_state.clone();
    let KagemushaV4ReleaseLifecyclePhaseV1::Enabled(canary_reuses_stage_enabled) =
        &mut canary_reuses_stage.phase
    else {
        unreachable!("fixture phase is enabled")
    };
    canary_reuses_stage_enabled.canary_transaction_intent = staged.stage_transaction_intent;
    assert_invalid_lifecycle_state(&canary_reuses_stage, "lifecycle.enabled_transition");

    let mut enable_reuses_canary = enabled_state;
    let KagemushaV4ReleaseLifecyclePhaseV1::Enabled(enable_reuses_canary_enabled) =
        &mut enable_reuses_canary.phase
    else {
        unreachable!("fixture phase is enabled")
    };
    enable_reuses_canary_enabled.enable_transaction_intent =
        enable_reuses_canary_enabled.canary_transaction_intent;
    assert_invalid_lifecycle_state(&enable_reuses_canary, "enabled");

    let mut cancellation_reuses_stage = cancelled_lifecycle_state(&staged, staged_id);
    let KagemushaV4ReleaseLifecyclePhaseV1::Cancelled(cancellation_reuses_stage_cancelled) =
        &mut cancellation_reuses_stage.phase
    else {
        unreachable!("fixture phase is cancelled")
    };
    cancellation_reuses_stage_cancelled.cancellation_transaction_intent =
        staged.stage_transaction_intent;
    assert_invalid_lifecycle_state(&cancellation_reuses_stage, "lifecycle.cancelled_transition");

    let mut zero_cancellation = cancelled_lifecycle_state(&staged, staged_id);
    let KagemushaV4ReleaseLifecyclePhaseV1::Cancelled(zero_cancellation_cancelled) =
        &mut zero_cancellation.phase
    else {
        unreachable!("fixture phase is cancelled")
    };
    zero_cancellation_cancelled.cancellation_transaction_intent = zero_intent;
    assert_invalid_lifecycle_state(&zero_cancellation, "cancelled");

    let deactivated_state = deactivated_lifecycle_state(&staged, enabled, enabled_id);

    let mut nested_zero_enable = deactivated_state.clone();
    let KagemushaV4ReleaseLifecyclePhaseV1::Deactivated(nested_zero_enable_deactivated) =
        &mut nested_zero_enable.phase
    else {
        unreachable!("fixture phase is deactivated")
    };
    nested_zero_enable_deactivated
        .enabled
        .enable_transaction_intent = zero_intent;
    assert_invalid_lifecycle_state(&nested_zero_enable, "enabled");

    let mut nested_zero_canary = deactivated_state.clone();
    let KagemushaV4ReleaseLifecyclePhaseV1::Deactivated(nested_zero_canary_deactivated) =
        &mut nested_zero_canary.phase
    else {
        unreachable!("fixture phase is deactivated")
    };
    nested_zero_canary_deactivated
        .enabled
        .canary_transaction_intent = zero_intent;
    assert_invalid_lifecycle_state(&nested_zero_canary, "enabled");

    let mut zero_deactivation = deactivated_state.clone();
    let KagemushaV4ReleaseLifecyclePhaseV1::Deactivated(zero_deactivation_deactivated) =
        &mut zero_deactivation.phase
    else {
        unreachable!("fixture phase is deactivated")
    };
    zero_deactivation_deactivated.deactivation_transaction_intent = zero_intent;
    assert_invalid_lifecycle_state(&zero_deactivation, "deactivated");

    let mut nested_enable_reuses_stage = deactivated_state.clone();
    let KagemushaV4ReleaseLifecyclePhaseV1::Deactivated(nested_enable_reuses_stage_deactivated) =
        &mut nested_enable_reuses_stage.phase
    else {
        unreachable!("fixture phase is deactivated")
    };
    nested_enable_reuses_stage_deactivated
        .enabled
        .enable_transaction_intent = staged.stage_transaction_intent;
    assert_invalid_lifecycle_state(
        &nested_enable_reuses_stage,
        "lifecycle.deactivated_transition",
    );

    let mut nested_canary_reuses_stage = deactivated_state.clone();
    let KagemushaV4ReleaseLifecyclePhaseV1::Deactivated(nested_canary_reuses_stage_deactivated) =
        &mut nested_canary_reuses_stage.phase
    else {
        unreachable!("fixture phase is deactivated")
    };
    nested_canary_reuses_stage_deactivated
        .enabled
        .canary_transaction_intent = staged.stage_transaction_intent;
    assert_invalid_lifecycle_state(
        &nested_canary_reuses_stage,
        "lifecycle.deactivated_transition",
    );

    let mut nested_enable_reuses_canary = deactivated_state.clone();
    let KagemushaV4ReleaseLifecyclePhaseV1::Deactivated(nested_enable_reuses_canary_deactivated) =
        &mut nested_enable_reuses_canary.phase
    else {
        unreachable!("fixture phase is deactivated")
    };
    nested_enable_reuses_canary_deactivated
        .enabled
        .enable_transaction_intent = nested_enable_reuses_canary_deactivated
        .enabled
        .canary_transaction_intent;
    assert_invalid_lifecycle_state(&nested_enable_reuses_canary, "enabled");

    let mut deactivation_reuses_stage = deactivated_state.clone();
    let KagemushaV4ReleaseLifecyclePhaseV1::Deactivated(deactivation_reuses_stage_deactivated) =
        &mut deactivation_reuses_stage.phase
    else {
        unreachable!("fixture phase is deactivated")
    };
    deactivation_reuses_stage_deactivated.deactivation_transaction_intent =
        staged.stage_transaction_intent;
    assert_invalid_lifecycle_state(
        &deactivation_reuses_stage,
        "lifecycle.deactivated_transition",
    );

    let mut deactivation_reuses_enable = deactivated_state.clone();
    let KagemushaV4ReleaseLifecyclePhaseV1::Deactivated(deactivation_reuses_enable_deactivated) =
        &mut deactivation_reuses_enable.phase
    else {
        unreachable!("fixture phase is deactivated")
    };
    deactivation_reuses_enable_deactivated.deactivation_transaction_intent =
        deactivation_reuses_enable_deactivated
            .enabled
            .enable_transaction_intent;
    assert_invalid_lifecycle_state(&deactivation_reuses_enable, "deactivated");

    let mut deactivation_reuses_canary = deactivated_state;
    let KagemushaV4ReleaseLifecyclePhaseV1::Deactivated(deactivation_reuses_canary_deactivated) =
        &mut deactivation_reuses_canary.phase
    else {
        unreachable!("fixture phase is deactivated")
    };
    deactivation_reuses_canary_deactivated.deactivation_transaction_intent =
        deactivation_reuses_canary_deactivated
            .enabled
            .canary_transaction_intent;
    assert_invalid_lifecycle_state(&deactivation_reuses_canary, "deactivated");
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
