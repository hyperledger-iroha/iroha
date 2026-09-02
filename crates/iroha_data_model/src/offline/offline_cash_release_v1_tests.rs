use super::*;
use crate::offline::{
    OFFLINE_CASH_HARDWARE_REQUIRED_CAPABILITIES_V1, OfflineCashDevicePublicKeyV1,
    OfflineCashHardwarePlatformClassV1, OfflineCashHardwareProfileV1,
};
use iroha_crypto::{Algorithm, KeyPair};
use p256::ecdsa::SigningKey;

const STATE_EQ_PROTOCOL_DIGEST: [u8; 32] = [0x31; 32];
const STATE_EP_PROTOCOL_DIGEST: [u8; 32] = [0x32; 32];
const WRAPPER_EQ_PROTOCOL_DIGEST: [u8; 32] = [0x33; 32];
const WRAPPER_EP_PROTOCOL_DIGEST: [u8; 32] = [0x34; 32];
const MINT_AUTHORIZATION_EQ_PROTOCOL_DIGEST: [u8; 32] = [0x3B; 32];
const MINT_AUTHORIZATION_EP_PROTOCOL_DIGEST: [u8; 32] = [0x3C; 32];
const MINT_EQ_PROTOCOL_DIGEST: [u8; 32] = [0x35; 32];
const MINT_EP_PROTOCOL_DIGEST: [u8; 32] = [0x36; 32];
const CREDENTIAL_EQ_PROTOCOL_DIGEST: [u8; 32] = [0x37; 32];
const CREDENTIAL_EP_PROTOCOL_DIGEST: [u8; 32] = [0x38; 32];
const GUARD_EQ_PROTOCOL_DIGEST: [u8; 32] = [0x39; 32];
const GUARD_EP_PROTOCOL_DIGEST: [u8; 32] = [0x3A; 32];

fn helper_protocols() -> Vec<OfflineCashHelperProtocolV1> {
    vec![
        OfflineCashHelperProtocolV1 {
            helper: OfflineCashQualifiedHelperCircuitV1::MintAuthorization,
            eq_protocol_digest: MINT_AUTHORIZATION_EQ_PROTOCOL_DIGEST,
            ep_protocol_digest: MINT_AUTHORIZATION_EP_PROTOCOL_DIGEST,
        },
        OfflineCashHelperProtocolV1 {
            helper: OfflineCashQualifiedHelperCircuitV1::MintCredit,
            eq_protocol_digest: MINT_EQ_PROTOCOL_DIGEST,
            ep_protocol_digest: MINT_EP_PROTOCOL_DIGEST,
        },
        OfflineCashHelperProtocolV1 {
            helper: OfflineCashQualifiedHelperCircuitV1::PlatformCredential,
            eq_protocol_digest: CREDENTIAL_EQ_PROTOCOL_DIGEST,
            ep_protocol_digest: CREDENTIAL_EP_PROTOCOL_DIGEST,
        },
        OfflineCashHelperProtocolV1 {
            helper: OfflineCashQualifiedHelperCircuitV1::GuardBundle,
            eq_protocol_digest: GUARD_EQ_PROTOCOL_DIGEST,
            ep_protocol_digest: GUARD_EP_PROTOCOL_DIGEST,
        },
    ]
}

fn evidence(seed: u8) -> OfflineCashEvidenceFileV1 {
    let seed = if seed == 0 { u8::MAX } else { seed };
    OfflineCashEvidenceFileV1 {
        sha256: [seed; 32],
        byte_len: 1_000 + u64::from(seed),
    }
}

fn artifacts() -> Vec<OfflineCashArtifactBindingV1> {
    OfflineCashArtifactRoleV1::ALL
        .iter()
        .copied()
        .enumerate()
        .map(|(index, role)| OfflineCashArtifactBindingV1 {
            role,
            sha256: [u8::try_from(index + 1).expect("small role index"); 32],
            byte_len: if role.is_params() {
                OFFLINE_CASH_PARAMS_BYTES_V1
            } else if role.is_state_pk() {
                32 * 1024 * 1024
            } else if role.is_helper_pk() {
                16 * 1024 * 1024
            } else {
                32 * 1024
            },
        })
        .collect()
}

fn artifact(
    artifacts: &[OfflineCashArtifactBindingV1],
    role: OfflineCashArtifactRoleV1,
) -> OfflineCashArtifactBindingV1 {
    *artifacts
        .iter()
        .find(|artifact| artifact.role == role)
        .expect("fixture contains every artifact role")
}

fn device_public_key(seed: u8) -> OfflineCashDevicePublicKeyV1 {
    let signing_key = SigningKey::from_bytes((&[seed; 32]).into()).expect("P-256 signing key");
    let encoded = signing_key.verifying_key().to_encoded_point(false);
    OfflineCashDevicePublicKeyV1::from_sec1_bytes(encoded.as_bytes()).expect("device public key")
}

fn hardware_profile(
    seed: u8,
    suite_id: [u8; 32],
    qualification_report_digest: [u8; 32],
) -> OfflineCashHardwareProfileV1 {
    OfflineCashHardwareProfileV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        protocol_version: OFFLINE_CASH_WIRE_VERSION_V1,
        hardware_profile_id: [0; 32],
        provider_id: [seed; 32],
        platform_class: OfflineCashHardwarePlatformClassV1::DedicatedSecureElement,
        product_class_digest: [seed.wrapping_add(1); 32],
        firmware_policy_digest: [seed.wrapping_add(2); 32],
        enrollment_attestation_verifier_digest: [seed.wrapping_add(3); 32],
        attestation_trust_roots_digest: [seed.wrapping_add(4); 32],
        allowed_suite_commitment: offline_cash_suite_commitment_v1(suite_id),
        policy_epoch: u64::from(seed),
        governance_credential_public_key: device_public_key(seed.wrapping_add(5)),
        capability_mask: OFFLINE_CASH_HARDWARE_REQUIRED_CAPABILITIES_V1,
        qualification_report_digest,
        valid_from_ms: 1,
        expires_at_ms: 100_000,
    }
    .seal_hardware_profile_id()
    .expect("hardware profile identity")
}

fn enabled_profile(seed: u8, vk_digest: [u8; 32]) -> OfflineCashEnabledProfileV1 {
    let suite_id = [seed.wrapping_add(0x10); 32];
    let qualification_report = evidence(seed.wrapping_add(0x20));
    let hardware_profile = hardware_profile(seed, suite_id, qualification_report.sha256);
    OfflineCashEnabledProfileV1 {
        hardware_profile,
        hardware_profile_id: hardware_profile.hardware_profile_id,
        suite_id,
        vk_digest,
        qualification_digest: [0; 32],
        policy_epoch: u64::from(seed),
        qualification_report,
    }
}

fn profile_qualification(
    profile: OfflineCashEnabledProfileV1,
    artifacts: &[OfflineCashArtifactBindingV1],
    helper_protocols: &[OfflineCashHelperProtocolV1],
    report_seed: u8,
) -> OfflineCashProfileQualificationV1 {
    let relations = OfflineCashQualifiedRelationV1::ALL
        .iter()
        .copied()
        .enumerate()
        .map(|(index, relation)| {
            let (eq_role, ep_role) = relation.expected_vk_roles();
            let (eq_protocol_digest, ep_protocol_digest) =
                if relation.uses_commit_wrapper_protocol() {
                    (WRAPPER_EQ_PROTOCOL_DIGEST, WRAPPER_EP_PROTOCOL_DIGEST)
                } else {
                    (STATE_EQ_PROTOCOL_DIGEST, STATE_EP_PROTOCOL_DIGEST)
                };
            OfflineCashRelationQualificationV1 {
                relation,
                eq_protocol_digest,
                ep_protocol_digest,
                eq_verifying_key: artifact(artifacts, eq_role),
                ep_verifying_key: artifact(artifacts, ep_role),
                eq_circuit_rows: 64_000,
                ep_circuit_rows: 64_000,
                complete_proof_bytes: 6_000,
                prove_p95_ms: 9_000,
                verify_p95_ms: 900,
                process_rss_bytes: 120 * 1024 * 1024,
                operation_energy_millijoules: 10_000,
                report: evidence(
                    report_seed.wrapping_add(u8::try_from(index).expect("small relation index")),
                ),
            }
        })
        .collect();
    let helper_circuits = helper_protocols
        .iter()
        .copied()
        .enumerate()
        .map(|(index, protocol)| {
            let (eq_role, ep_role) = protocol.helper.expected_vk_roles();
            OfflineCashHelperQualificationV1 {
                helper: protocol.helper,
                eq_protocol_digest: protocol.eq_protocol_digest,
                ep_protocol_digest: protocol.ep_protocol_digest,
                eq_verifying_key: artifact(artifacts, eq_role),
                ep_verifying_key: artifact(artifacts, ep_role),
                eq_circuit_rows: 32_000,
                ep_circuit_rows: 32_000,
                complete_proof_bytes: 4_000,
                prove_p95_ms: 8_000,
                verify_p95_ms: 800,
                process_rss_bytes: 110 * 1024 * 1024,
                operation_energy_millijoules: 8_000,
                report: evidence(
                    report_seed.wrapping_add(9 + u8::try_from(index).expect("small helper index")),
                ),
            }
        })
        .collect();
    let receive_fold_occupancies = (1_u8..=OFFLINE_CASH_RECEIVE_FOLD_BATCH_WIDTH_V1)
        .map(|occupancy| OfflineCashReceiveFoldOccupancyV1 {
            occupancy,
            complete_proof_bytes: 6_000,
            report: evidence(report_seed.wrapping_add(0x10).wrapping_add(occupancy)),
        })
        .collect();
    let recursive_depths = [8_u32, 64, 1_024, 2_048]
        .into_iter()
        .enumerate()
        .map(|(index, depth)| OfflineCashRecursiveDepthQualificationV1 {
            depth,
            verified_handoffs: depth,
            complete_proof_bytes: 6_000,
            raw_session_bytes: 9_000,
            text_session_bytes: 12_000,
            report: evidence(
                report_seed.wrapping_add(0x30 + u8::try_from(index).expect("small depth index")),
            ),
        })
        .collect();
    let acceptance_cases = OfflineCashAcceptanceCaseV1::ALL
        .iter()
        .copied()
        .enumerate()
        .map(|(index, case)| OfflineCashAcceptanceCaseEvidenceV1 {
            case,
            validator_count: if matches!(
                case,
                OfflineCashAcceptanceCaseV1::FourPeerActivationRestartReplay
            ) {
                OFFLINE_CASH_VALIDATOR_COUNT_V1
            } else {
                0
            },
            report: evidence(
                report_seed
                    .wrapping_add(0x60)
                    .wrapping_add(u8::try_from(index).expect("small acceptance-case index")),
            ),
        })
        .collect();
    OfflineCashProfileQualificationV1 {
        profile,
        relations,
        helper_circuits,
        receive_fold_occupancies,
        recursive_depths,
        aggregate_balance: OfflineCashAggregateBalanceQualificationV1 {
            independent_payments: OFFLINE_CASH_MIN_QUALIFIED_AGGREGATED_CREDITS_V1,
            folded_credits: OFFLINE_CASH_MIN_QUALIFIED_AGGREGATED_CREDITS_V1,
            spend_payments: 1,
            report: evidence(report_seed.wrapping_add(0x40)),
        },
        thermal: OfflineCashThermalQualificationV1 {
            folded_credits: OFFLINE_CASH_MIN_THERMAL_FOLDED_CREDITS_V1,
            fold_p95_ms: 9_500,
            process_rss_bytes: 120 * 1024 * 1024,
            operation_energy_millijoules: 11_000,
            report: evidence(report_seed.wrapping_add(0x41)),
        },
        envelope: OfflineCashEnvelopeQualificationV1 {
            raw_session_bytes: 9_000,
            text_session_bytes: 12_000,
            handoff_p95_ms: 29_000,
            report: evidence(report_seed.wrapping_add(0x42)),
        },
        acceptance_cases,
    }
    .seal_qualification_digest()
    .expect("profile qualification digest")
}

fn receipt(artifacts: &[OfflineCashArtifactBindingV1]) -> OfflineCashInternalValidationReceiptV1 {
    let artifact_set_digest =
        offline_cash_artifact_set_digest_v1(artifacts).expect("artifact digest");
    let helper_protocols = helper_protocols();
    let vk_digest = offline_cash_vk_set_digest_v1(
        artifacts,
        STATE_EQ_PROTOCOL_DIGEST,
        STATE_EP_PROTOCOL_DIGEST,
        WRAPPER_EQ_PROTOCOL_DIGEST,
        WRAPPER_EP_PROTOCOL_DIGEST,
        &helper_protocols,
    )
    .expect("VK-set digest");
    let mut profile_qualifications = vec![
        profile_qualification(
            enabled_profile(0x41, vk_digest),
            artifacts,
            &helper_protocols,
            0x61,
        ),
        profile_qualification(
            enabled_profile(0x42, vk_digest),
            artifacts,
            &helper_protocols,
            0xA1,
        ),
    ];
    profile_qualifications.sort_by_key(|qualification| qualification.profile.hardware_profile_id);
    let enabled_profiles: Vec<_> = profile_qualifications
        .iter()
        .map(|qualification| qualification.profile)
        .collect();
    let hardware_policy_digest =
        offline_cash_hardware_policy_digest_v1(&enabled_profiles).expect("hardware policy digest");
    let circuit_shape_report = evidence(5);
    let profile_digest = offline_cash_release_profile_digest_v1(
        circuit_shape_report,
        STATE_EQ_PROTOCOL_DIGEST,
        STATE_EP_PROTOCOL_DIGEST,
        WRAPPER_EQ_PROTOCOL_DIGEST,
        WRAPPER_EP_PROTOCOL_DIGEST,
        &helper_protocols,
    )
    .expect("release profile digest");
    OfflineCashInternalValidationReceiptV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        source_tree_digest: [1; 32],
        cargo_lock_digest: [2; 32],
        profile_digest,
        eq_protocol_digest: STATE_EQ_PROTOCOL_DIGEST,
        ep_protocol_digest: STATE_EP_PROTOCOL_DIGEST,
        commit_wrapper_eq_protocol_digest: WRAPPER_EQ_PROTOCOL_DIGEST,
        commit_wrapper_ep_protocol_digest: WRAPPER_EP_PROTOCOL_DIGEST,
        artifact_set_digest,
        hardware_policy_digest,
        evidence_closure: OfflineCashEvidenceClosureV1 {
            evidence_manifest: evidence(0xE1),
            observer_policy: evidence(0xE2),
            verification_records_digest: [0xE3; 32],
            candidate_context_digest: [0xE4; 32],
            verification_record_count: 128,
            total_evidence_bytes: 400 * 1024 * 1024,
            total_transcript_bytes: 1024 * 1024,
            total_command_input_bytes: 2 * 1024 * 1024 * 1024,
            total_observed_duration_ms: 60_000,
            total_observed_cpu_ms: 30_000,
        },
        circuit_shape_report,
        security_review_report: evidence(6),
        kat_report: evidence(7),
        fuzz_report: evidence(8),
        resource_report: evidence(9),
        profile_qualifications,
        helper_protocols,
        reproducible_builds: vec![
            OfflineCashReproducibleBuildV1 {
                builder_id: [0xD1; 32],
                artifact_set_digest,
                report: evidence(0xD3),
            },
            OfflineCashReproducibleBuildV1 {
                builder_id: [0xD2; 32],
                artifact_set_digest,
                report: evidence(0xD4),
            },
        ],
        fuzz_cases: OFFLINE_CASH_MIN_FUZZ_CASES_V1,
    }
}

fn receipt_with_profile_count(
    artifacts: &[OfflineCashArtifactBindingV1],
    profile_count: usize,
) -> OfflineCashInternalValidationReceiptV1 {
    assert!((1..=OFFLINE_CASH_RELEASE_MAX_ENABLED_PROFILES_V1).contains(&profile_count));
    let mut receipt = receipt(artifacts);
    let vk_digest = receipt.profile_qualifications[0].profile.vk_digest;
    let helper_protocols = receipt.helper_protocols.clone();
    receipt.profile_qualifications = (0..profile_count)
        .map(|index| {
            let seed = u8::try_from(index + 1).expect("profile bound fits u8");
            profile_qualification(
                enabled_profile(seed, vk_digest),
                artifacts,
                &helper_protocols,
                seed.wrapping_add(0x80),
            )
        })
        .collect();
    receipt
        .profile_qualifications
        .sort_by_key(|qualification| qualification.profile.hardware_profile_id);
    let enabled_profiles: Vec<_> = receipt
        .profile_qualifications
        .iter()
        .map(|qualification| qualification.profile)
        .collect();
    receipt.hardware_policy_digest =
        offline_cash_hardware_policy_digest_v1(&enabled_profiles).expect("bounded hardware policy");
    receipt
}

fn reseal_profile_qualification(
    receipt: &mut OfflineCashInternalValidationReceiptV1,
    index: usize,
) {
    receipt.profile_qualifications[index] = receipt.profile_qualifications[index]
        .clone()
        .seal_qualification_digest()
        .expect("reseal profile qualification");
    let enabled_profiles: Vec<_> = receipt
        .profile_qualifications
        .iter()
        .map(|qualification| qualification.profile)
        .collect();
    receipt.hardware_policy_digest =
        offline_cash_hardware_policy_digest_v1(&enabled_profiles).expect("reseal hardware policy");
}

fn manifest(
    artifacts: Vec<OfflineCashArtifactBindingV1>,
    receipt: &OfflineCashInternalValidationReceiptV1,
) -> OfflineCashReleaseManifestV1 {
    OfflineCashReleaseManifestV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        release_id: [0; 32],
        source_tree_digest: receipt.source_tree_digest,
        cargo_lock_digest: receipt.cargo_lock_digest,
        profile_digest: receipt.profile_digest,
        eq_protocol_digest: receipt.eq_protocol_digest,
        ep_protocol_digest: receipt.ep_protocol_digest,
        commit_wrapper_eq_protocol_digest: receipt.commit_wrapper_eq_protocol_digest,
        commit_wrapper_ep_protocol_digest: receipt.commit_wrapper_ep_protocol_digest,
        hardware_policy_digest: receipt.hardware_policy_digest,
        validation_receipt_digest: receipt.canonical_digest().expect("receipt digest"),
        halo2_k: OFFLINE_CASH_HALO2_K_V1,
        helper_protocols: receipt.helper_protocols.clone(),
        enabled_profiles: receipt
            .profile_qualifications
            .iter()
            .map(|qualification| qualification.profile)
            .collect(),
        artifacts,
    }
    .seal()
    .expect("seal manifest")
}

fn authority_keys() -> Vec<KeyPair> {
    let mut keys = Vec::from(
        [0x41_u8, 0x42, 0x43].map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)),
    );
    keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    keys
}

fn authority_policy(keys: &[KeyPair], threshold: u16) -> OfflineCashReleaseAuthorityPolicyV1 {
    OfflineCashReleaseAuthorityPolicyV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        authority_set_id: [0x40; 32],
        threshold,
        authorized_signers: keys.iter().map(|key| key.public_key().clone()).collect(),
    }
}

fn release_attestation(
    manifest: &OfflineCashReleaseManifestV1,
    receipt: &OfflineCashInternalValidationReceiptV1,
    policy: &OfflineCashReleaseAuthorityPolicyV1,
    signing_keys: &[KeyPair],
) -> OfflineCashReleaseAttestationV1 {
    let subject = manifest
        .release_attestation_subject(receipt, policy)
        .expect("release attestation subject");
    let payload = subject.approval_payload();
    OfflineCashReleaseAttestationV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        subject,
        approvals: signing_keys
            .iter()
            .map(|key| OfflineCashReleaseApprovalV1 {
                public_key: key.public_key().clone(),
                signature: SignatureOf::try_new(key.private_key(), &payload)
                    .expect("release approval signature"),
            })
            .collect(),
    }
}

#[test]
fn authenticates_complete_typed_evidence_release() {
    let artifacts = artifacts();
    let receipt = receipt(&artifacts);
    let expected_vk_digest = offline_cash_vk_set_digest_v1(
        &artifacts,
        receipt.eq_protocol_digest,
        receipt.ep_protocol_digest,
        receipt.commit_wrapper_eq_protocol_digest,
        receipt.commit_wrapper_ep_protocol_digest,
        &receipt.helper_protocols,
    )
    .expect("expected VK set");
    let manifest = manifest(artifacts, &receipt);
    let receipt_bytes = norito::encode_canonical(&receipt).expect("encode receipt");
    assert!(receipt_bytes.len() <= OFFLINE_CASH_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1);
    let decoded_receipt =
        OfflineCashInternalValidationReceiptV1::decode_canonical_exact(&receipt_bytes)
            .expect("decode receipt");
    assert_eq!(decoded_receipt, receipt);

    let manifest_bytes = norito::encode_canonical(&manifest).expect("encode manifest");
    let decoded_manifest = OfflineCashReleaseManifestV1::decode_canonical_exact(&manifest_bytes)
        .expect("decode manifest");
    assert_eq!(decoded_manifest, manifest);

    let keys = authority_keys();
    let policy = authority_policy(&keys, 2);
    let attestation = release_attestation(&manifest, &receipt, &policy, &keys[..2]);
    let authenticated = decoded_manifest
        .authenticate(&decoded_receipt, &policy, &attestation)
        .expect("authenticate");
    assert_eq!(authenticated.release_id(), decoded_manifest.release_id);
    assert_eq!(authenticated.approved_signers().len(), 2);
    assert_eq!(authenticated.enabled_profiles().len(), 2);
    let first_profile = authenticated.enabled_profiles()[0];
    assert_eq!(
        authenticated
            .enabled_profile(first_profile.hardware_profile_id)
            .expect("enabled profile"),
        &first_profile
    );
    assert!(authenticated.enabled_profile([0xFF; 32]).is_none());
    assert_eq!(authenticated.vk_set_digest(), expected_vk_digest);
    assert_eq!(
        authenticated.hardware_policy_digest(),
        receipt.hardware_policy_digest
    );
    assert_eq!(
        authenticated.commit_wrapper_eq_protocol_digest(),
        WRAPPER_EQ_PROTOCOL_DIGEST
    );
    assert_eq!(
        authenticated.commit_wrapper_ep_protocol_digest(),
        WRAPPER_EP_PROTOCOL_DIGEST
    );
    assert_eq!(authenticated.helper_protocols(), receipt.helper_protocols);
    let mint_authorization = authenticated
        .helper_protocol(OfflineCashQualifiedHelperCircuitV1::MintAuthorization)
        .expect("mint-authorization helper protocol");
    assert_eq!(
        mint_authorization.helper,
        OfflineCashQualifiedHelperCircuitV1::MintAuthorization
    );
    assert!(
        authenticated
            .helper_protocols()
            .iter()
            .all(|protocol| authenticated.helper_protocol(protocol.helper) == Some(protocol))
    );
    assert_eq!(
        authenticated
            .artifact(OfflineCashArtifactRoleV1::CommitWrapperVkEp)
            .role,
        OfflineCashArtifactRoleV1::CommitWrapperVkEp
    );
}

#[test]
fn vk_qualification_and_hardware_policy_digests_bind_exact_content() {
    let artifacts = artifacts();
    let base = receipt(&artifacts);
    let expected_vk_digest = offline_cash_vk_set_digest_v1(
        &artifacts,
        base.eq_protocol_digest,
        base.ep_protocol_digest,
        base.commit_wrapper_eq_protocol_digest,
        base.commit_wrapper_ep_protocol_digest,
        &base.helper_protocols,
    )
    .expect("VK-set digest");
    assert!(
        base.profile_qualifications
            .iter()
            .all(|qualification| qualification.profile.vk_digest == expected_vk_digest)
    );
    for qualification in &base.profile_qualifications {
        assert_eq!(
            qualification.profile.qualification_digest,
            qualification
                .expected_qualification_digest()
                .expect("qualification digest")
        );
    }

    let mut changed_vk_artifact = artifacts.clone();
    changed_vk_artifact
        .iter_mut()
        .find(|artifact| artifact.role == OfflineCashArtifactRoleV1::StateVkEq)
        .expect("state Eq VK")
        .sha256 = [0xF0; 32];
    assert_ne!(
        offline_cash_vk_set_digest_v1(
            &changed_vk_artifact,
            base.eq_protocol_digest,
            base.ep_protocol_digest,
            base.commit_wrapper_eq_protocol_digest,
            base.commit_wrapper_ep_protocol_digest,
            &base.helper_protocols,
        )
        .expect("changed VK-set digest"),
        expected_vk_digest
    );

    let mut duplicate_any_artifact = artifacts.clone();
    let params_digest =
        artifact(&duplicate_any_artifact, OfflineCashArtifactRoleV1::ParamsEq).sha256;
    duplicate_any_artifact
        .iter_mut()
        .find(|artifact| artifact.role == OfflineCashArtifactRoleV1::MintCreditPkEq)
        .expect("mint-credit proving key")
        .sha256 = params_digest;
    assert_eq!(
        offline_cash_artifact_set_digest_v1(&duplicate_any_artifact),
        Err(OfflineCashReleaseErrorV1::InvalidArtifactSet)
    );

    let mut inconsistent_receipt_vk = base.clone();
    inconsistent_receipt_vk.profile_qualifications[0]
        .profile
        .vk_digest = [0xF1; 32];
    reseal_profile_qualification(&mut inconsistent_receipt_vk, 0);
    assert_eq!(
        inconsistent_receipt_vk.validate(),
        Err(OfflineCashReleaseErrorV1::InvalidValidationReceipt)
    );

    let mut forged_policy = base.clone();
    forged_policy.hardware_policy_digest = [0xF2; 32];
    assert_eq!(
        forged_policy.validate(),
        Err(OfflineCashReleaseErrorV1::InvalidValidationReceipt)
    );

    let mut invalid_embedded_profile = base.clone();
    invalid_embedded_profile.profile_qualifications[0]
        .profile
        .hardware_profile
        .provider_id = [0xF3; 32];
    assert_eq!(
        invalid_embedded_profile.validate(),
        Err(OfflineCashReleaseErrorV1::InvalidValidationReceipt)
    );

    let mut mismatched_policy_epoch = base.clone();
    mismatched_policy_epoch.profile_qualifications[0]
        .profile
        .policy_epoch += 1;
    assert_eq!(
        mismatched_policy_epoch.validate(),
        Err(OfflineCashReleaseErrorV1::InvalidValidationReceipt)
    );

    let mut mismatched_suite = base.clone();
    mismatched_suite.profile_qualifications[0].profile.suite_id = [0xF4; 32];
    assert_eq!(
        mismatched_suite.validate(),
        Err(OfflineCashReleaseErrorV1::InvalidValidationReceipt)
    );

    let mut mismatched_report = base.clone();
    mismatched_report.profile_qualifications[0]
        .profile
        .qualification_report
        .sha256 = [0xF5; 32];
    assert_eq!(
        mismatched_report.validate(),
        Err(OfflineCashReleaseErrorV1::InvalidValidationReceipt)
    );

    let mut manifest_vk = manifest(artifacts, &base);
    for profile in &mut manifest_vk.enabled_profiles {
        profile.vk_digest = [0xF6; 32];
    }
    manifest_vk.hardware_policy_digest =
        offline_cash_hardware_policy_digest_v1(&manifest_vk.enabled_profiles)
            .expect("forged policy remains structurally canonical");
    manifest_vk.release_id = [0; 32];
    manifest_vk = manifest_vk.seal().expect("reseal forged manifest");
    assert_eq!(
        manifest_vk.validate_standalone(),
        Err(OfflineCashReleaseErrorV1::InvalidManifest)
    );
}

#[test]
fn manifest_profile_set_is_canonical_release_identity() {
    let artifacts = artifacts();
    let receipt = receipt(&artifacts);
    let manifest = manifest(artifacts.clone(), &receipt);
    let mut changed = manifest.clone();
    changed.enabled_profiles[1].suite_id = [0xEE; 32];
    changed.release_id = [0; 32];
    changed = changed.seal().expect("reseal changed profile set");
    assert_ne!(manifest.release_id, changed.release_id);
    assert_eq!(
        changed.release_attestation_subject(&receipt, &authority_policy(&authority_keys(), 1)),
        Err(OfflineCashReleaseErrorV1::InvalidManifest)
    );

    let mut changed_wrapper_protocol = manifest.clone();
    changed_wrapper_protocol.commit_wrapper_eq_protocol_digest = [0xED; 32];
    changed_wrapper_protocol.release_id = [0; 32];
    changed_wrapper_protocol = changed_wrapper_protocol
        .seal()
        .expect("reseal wrapper protocol");
    assert_ne!(manifest.release_id, changed_wrapper_protocol.release_id);

    let mut unordered = manifest;
    unordered.enabled_profiles.reverse();
    unordered.release_id = unordered.expected_release_id().expect("unordered id");
    assert_eq!(
        unordered.validate_standalone(),
        Err(OfflineCashReleaseErrorV1::InvalidManifest)
    );

    let mut duplicate = receipt;
    duplicate.profile_qualifications[1].profile = duplicate.profile_qualifications[0].profile;
    assert_eq!(
        duplicate.validate(),
        Err(OfflineCashReleaseErrorV1::InvalidValidationReceipt)
    );
}

#[test]
fn relations_are_closed_ordered_and_bind_exact_verifier_artifacts() {
    let artifacts = artifacts();
    let base = receipt(&artifacts);

    let mut missing = base.clone();
    missing.profile_qualifications[0].relations.pop();
    assert!(missing.validate().is_err());

    let mut unordered = base.clone();
    unordered.profile_qualifications[0].relations.swap(0, 1);
    assert!(unordered.validate().is_err());

    let mut wrong_wrapper_role = base.clone();
    let wrapper = wrong_wrapper_role.profile_qualifications[0]
        .relations
        .last_mut()
        .expect("wrapper relation");
    wrapper.eq_verifying_key = artifact(&artifacts, OfflineCashArtifactRoleV1::StateVkEq);
    assert!(wrong_wrapper_role.validate().is_err());

    let mut wrong_protocol = base.clone();
    wrong_protocol.profile_qualifications[0].relations[0].eq_protocol_digest =
        WRAPPER_EQ_PROTOCOL_DIGEST;
    assert!(wrong_protocol.validate().is_err());

    let mut wrapper_using_state_protocol = base.clone();
    wrapper_using_state_protocol.profile_qualifications[0]
        .relations
        .last_mut()
        .expect("wrapper relation")
        .ep_protocol_digest = STATE_EP_PROTOCOL_DIGEST;
    assert!(wrapper_using_state_protocol.validate().is_err());

    let mut exact_limits = base.clone();
    let relation = &mut exact_limits.profile_qualifications[0].relations[0];
    relation.complete_proof_bytes =
        u32::try_from(OFFLINE_CASH_PAIRED_PROOF_MAX_BYTES_V1).expect("proof cap fits u32");
    relation.eq_circuit_rows = 1_u32 << OFFLINE_CASH_HALO2_K_V1;
    relation.eq_verifying_key.byte_len = OFFLINE_CASH_VERIFYING_KEY_MAX_BYTES_V1;
    reseal_profile_qualification(&mut exact_limits, 0);
    exact_limits.validate().expect("exact relation limits pass");
    exact_limits.profile_qualifications[0].relations[0].complete_proof_bytes += 1;
    assert!(exact_limits.validate().is_err());

    let mut substituted_vk = base.clone();
    substituted_vk.profile_qualifications[0].relations[0]
        .eq_verifying_key
        .sha256 = [0xEF; 32];
    reseal_profile_qualification(&mut substituted_vk, 0);
    let substituted_manifest = manifest(artifacts.clone(), &substituted_vk);
    assert_eq!(
        substituted_manifest
            .release_attestation_subject(&substituted_vk, &authority_policy(&authority_keys(), 1)),
        Err(OfflineCashReleaseErrorV1::InvalidManifest)
    );

    let mut aliased_wrapper = artifacts;
    let state_eq = artifact(&aliased_wrapper, OfflineCashArtifactRoleV1::StateVkEq).sha256;
    let wrapper_eq = aliased_wrapper
        .iter_mut()
        .find(|binding| binding.role == OfflineCashArtifactRoleV1::CommitWrapperVkEq)
        .expect("wrapper Eq binding");
    wrapper_eq.sha256 = state_eq;
    assert_eq!(
        offline_cash_artifact_set_digest_v1(&aliased_wrapper),
        Err(OfflineCashReleaseErrorV1::InvalidArtifactSet)
    );
}

#[test]
fn mint_authorization_artifacts_helpers_and_acceptance_relation_are_frozen() {
    assert_eq!(OfflineCashArtifactRoleV1::ALL.len(), 26);
    assert_eq!(
        &OfflineCashArtifactRoleV1::ALL[6..10],
        &[
            OfflineCashArtifactRoleV1::MintAuthorizationPkEq,
            OfflineCashArtifactRoleV1::MintAuthorizationVkEq,
            OfflineCashArtifactRoleV1::MintAuthorizationPkEp,
            OfflineCashArtifactRoleV1::MintAuthorizationVkEp,
        ]
    );
    assert_eq!(
        OfflineCashQualifiedHelperCircuitV1::ALL,
        [
            OfflineCashQualifiedHelperCircuitV1::MintAuthorization,
            OfflineCashQualifiedHelperCircuitV1::MintCredit,
            OfflineCashQualifiedHelperCircuitV1::PlatformCredential,
            OfflineCashQualifiedHelperCircuitV1::GuardBundle,
        ]
    );
    assert_eq!(OfflineCashQualifiedRelationV1::ALL.len(), 9);
    assert_eq!(
        OfflineCashQualifiedRelationV1::AcceptanceIntentAuthorization.expected_vk_roles(),
        (
            OfflineCashArtifactRoleV1::CommitWrapperVkEq,
            OfflineCashArtifactRoleV1::CommitWrapperVkEp,
        )
    );

    let artifacts = artifacts();
    let mut missing_mint_authorization = artifacts.clone();
    missing_mint_authorization
        .retain(|artifact| artifact.role != OfflineCashArtifactRoleV1::MintAuthorizationPkEq);
    assert_eq!(
        offline_cash_artifact_set_digest_v1(&missing_mint_authorization),
        Err(OfflineCashReleaseErrorV1::InvalidArtifactSet)
    );
    let base = receipt(&artifacts);
    let acceptance = base.profile_qualifications[0]
        .relations
        .iter()
        .find(|relation| {
            relation.relation == OfflineCashQualifiedRelationV1::AcceptanceIntentAuthorization
        })
        .expect("acceptance-intent authorization qualification");
    assert_eq!(
        (acceptance.eq_protocol_digest, acceptance.ep_protocol_digest),
        (WRAPPER_EQ_PROTOCOL_DIGEST, WRAPPER_EP_PROTOCOL_DIGEST)
    );
}

#[test]
fn helper_circuits_are_complete_measured_and_artifact_bound() {
    let artifacts = artifacts();
    let base = receipt(&artifacts);

    let mut missing = base.clone();
    missing.profile_qualifications[0].helper_circuits.pop();
    assert!(missing.validate().is_err());

    let mut wrong_role = base.clone();
    wrong_role.profile_qualifications[0].helper_circuits[0].eq_verifying_key =
        artifact(&artifacts, OfflineCashArtifactRoleV1::StateVkEq);
    assert!(wrong_role.validate().is_err());

    let mut wrong_protocol = base.clone();
    wrong_protocol.profile_qualifications[0].helper_circuits[1].eq_protocol_digest =
        GUARD_EQ_PROTOCOL_DIGEST;
    assert!(wrong_protocol.validate().is_err());

    let mut exact = base.clone();
    let helper = &mut exact.profile_qualifications[0].helper_circuits[2];
    helper.eq_circuit_rows = 1_u32 << OFFLINE_CASH_HALO2_K_V1;
    helper.complete_proof_bytes =
        u32::try_from(OFFLINE_CASH_PAIRED_PROOF_MAX_BYTES_V1).expect("proof cap fits u32");
    helper.prove_p95_ms = OFFLINE_CASH_PROVE_P95_MAX_MS_V1;
    helper.verify_p95_ms = OFFLINE_CASH_VERIFY_P95_MAX_MS_V1;
    helper.process_rss_bytes = OFFLINE_CASH_PROCESS_RSS_MAX_BYTES_V1;
    reseal_profile_qualification(&mut exact, 0);
    exact.validate().expect("exact helper limits pass");
    exact.profile_qualifications[0].helper_circuits[2].complete_proof_bytes += 1;
    assert!(exact.validate().is_err());

    let mut substituted = base.clone();
    substituted.profile_qualifications[0].helper_circuits[0]
        .eq_verifying_key
        .sha256 = [0xE7; 32];
    reseal_profile_qualification(&mut substituted, 0);
    let substituted_manifest = manifest(artifacts, &substituted);
    assert_eq!(
        substituted_manifest
            .release_attestation_subject(&substituted, &authority_policy(&authority_keys(), 1)),
        Err(OfflineCashReleaseErrorV1::InvalidManifest)
    );
}

#[test]
fn batch_occupancies_and_recursive_depths_are_exact() {
    let artifacts = artifacts();
    let base = receipt(&artifacts);

    let mut missing_occupancy = base.clone();
    missing_occupancy.profile_qualifications[0]
        .receive_fold_occupancies
        .remove(7);
    assert!(missing_occupancy.validate().is_err());

    let mut unordered_occupancy = base.clone();
    unordered_occupancy.profile_qualifications[0]
        .receive_fold_occupancies
        .swap(7, 8);
    assert!(unordered_occupancy.validate().is_err());

    let mut shallow = base.clone();
    shallow.profile_qualifications[0].recursive_depths[3].depth = 1_024;
    shallow.profile_qualifications[0].recursive_depths[3].verified_handoffs = 1_024;
    assert!(shallow.validate().is_err());

    let mut unverifiable = base;
    unverifiable.profile_qualifications[0].recursive_depths[2].verified_handoffs -= 1;
    assert!(unverifiable.validate().is_err());
}

#[test]
fn recursive_depth_wire_sizes_are_bounded_and_invariant() {
    let artifacts = artifacts();
    let base = receipt(&artifacts);

    let mut variant = base.clone();
    variant.profile_qualifications[0].recursive_depths[3].complete_proof_bytes -= 1;
    assert!(variant.validate().is_err());

    let mut exact = base.clone();
    for depth in &mut exact.profile_qualifications[0].recursive_depths {
        depth.complete_proof_bytes =
            u32::try_from(OFFLINE_CASH_PAIRED_PROOF_MAX_BYTES_V1).expect("proof cap fits u32");
        depth.raw_session_bytes =
            u32::try_from(OFFLINE_CASH_SESSION_MAX_BYTES_V1).expect("raw cap fits u32");
        depth.text_session_bytes =
            u32::try_from(OFFLINE_CASH_TEXT_SESSION_MAX_BYTES_V1).expect("text cap fits u32");
    }
    exact.profile_qualifications[0].envelope.raw_session_bytes =
        u32::try_from(OFFLINE_CASH_SESSION_MAX_BYTES_V1).expect("raw cap fits u32");
    exact.profile_qualifications[0].envelope.text_session_bytes =
        u32::try_from(OFFLINE_CASH_TEXT_SESSION_MAX_BYTES_V1).expect("text cap fits u32");
    reseal_profile_qualification(&mut exact, 0);
    exact.validate().expect("exact invariant depth limits pass");

    for depth in &mut exact.profile_qualifications[0].recursive_depths {
        depth.complete_proof_bytes += 1;
    }
    assert!(exact.validate().is_err());
}

#[test]
fn quantitative_and_wire_gates_are_typed_per_profile() {
    let artifacts = artifacts();
    let base = receipt(&artifacts);

    let mut exact = base.clone();
    exact.profile_qualifications[0].envelope.raw_session_bytes =
        u32::try_from(OFFLINE_CASH_SESSION_MAX_BYTES_V1).expect("raw cap fits u32");
    exact.profile_qualifications[0].envelope.text_session_bytes =
        u32::try_from(OFFLINE_CASH_TEXT_SESSION_MAX_BYTES_V1).expect("text cap fits u32");
    let raw_session_bytes = exact.profile_qualifications[0].envelope.raw_session_bytes;
    let text_session_bytes = exact.profile_qualifications[0].envelope.text_session_bytes;
    for depth in &mut exact.profile_qualifications[0].recursive_depths {
        depth.raw_session_bytes = raw_session_bytes;
        depth.text_session_bytes = text_session_bytes;
    }
    reseal_profile_qualification(&mut exact, 0);
    exact.validate().expect("exact envelope limits pass");
    exact.profile_qualifications[0].envelope.raw_session_bytes += 1;
    assert!(exact.validate().is_err());

    let mut aggregate = base.clone();
    aggregate.profile_qualifications[0]
        .aggregate_balance
        .independent_payments -= 1;
    assert!(aggregate.validate().is_err());

    let mut not_one_spend = base.clone();
    not_one_spend.profile_qualifications[0]
        .aggregate_balance
        .spend_payments = 2;
    assert!(not_one_spend.validate().is_err());

    let mut thermal = base;
    thermal.profile_qualifications[0].thermal.folded_credits -= 1;
    assert!(thermal.validate().is_err());
}

#[test]
fn acceptance_cases_and_reproducible_builds_are_closed() {
    let artifacts = artifacts();
    let base = receipt(&artifacts);
    assert!(OfflineCashAcceptanceCaseV1::ALL.contains(&OfflineCashAcceptanceCaseV1::SingleExact));
    assert!(
        OfflineCashAcceptanceCaseV1::ALL
            .contains(&OfflineCashAcceptanceCaseV1::CrashAfterCommitWrapperGeneratedBeforeInstall)
    );

    let mut missing_case = base.clone();
    missing_case.profile_qualifications[0]
        .acceptance_cases
        .pop();
    assert!(missing_case.validate().is_err());

    let mut unordered_case = base.clone();
    unordered_case.profile_qualifications[0]
        .acceptance_cases
        .swap(1, 2);
    assert!(unordered_case.validate().is_err());

    let mut wrong_validator_count = base.clone();
    let four_peer = wrong_validator_count.profile_qualifications[0]
        .acceptance_cases
        .iter_mut()
        .find(|case| case.case == OfflineCashAcceptanceCaseV1::FourPeerActivationRestartReplay)
        .expect("four-peer case");
    four_peer.validator_count = 3;
    assert!(wrong_validator_count.validate().is_err());

    let manifest = manifest(artifacts.clone(), &base);
    let mut cross_profile_substitution = base.clone();
    let first_reports: Vec<_> = cross_profile_substitution.profile_qualifications[0]
        .acceptance_cases
        .iter()
        .map(|evidence| evidence.report)
        .collect();
    for (evidence, report) in cross_profile_substitution.profile_qualifications[1]
        .acceptance_cases
        .iter_mut()
        .zip(first_reports)
    {
        evidence.report = report;
    }
    reseal_profile_qualification(&mut cross_profile_substitution, 1);
    cross_profile_substitution
        .validate()
        .expect("substituted matrix remains structurally complete");
    assert_eq!(
        manifest.release_attestation_subject(
            &cross_profile_substitution,
            &authority_policy(&authority_keys(), 1)
        ),
        Err(OfflineCashReleaseErrorV1::InvalidManifest)
    );

    let mut one_build = base.clone();
    one_build.reproducible_builds.pop();
    assert!(one_build.validate().is_err());

    let mut duplicate_builder = base.clone();
    duplicate_builder.reproducible_builds[1].builder_id =
        duplicate_builder.reproducible_builds[0].builder_id;
    assert!(duplicate_builder.validate().is_err());

    let mut wrong_artifacts = base;
    wrong_artifacts.reproducible_builds[1].artifact_set_digest = [0xFE; 32];
    assert!(wrong_artifacts.validate().is_err());
}

#[test]
fn evidence_file_bindings_are_nonempty_and_bounded_provenance() {
    let artifacts = artifacts();
    let mut receipt = receipt(&artifacts);
    receipt.security_review_report.sha256 = [0; 32];
    assert!(receipt.validate().is_err());
    receipt = self::receipt(&artifacts);
    receipt.profile_qualifications[0].relations[0]
        .report
        .byte_len = OFFLINE_CASH_RELEASE_EVIDENCE_FILE_MAX_BYTES_V1 + 1;
    assert!(receipt.validate().is_err());
}

#[test]
fn rejects_unknown_invalid_or_insufficient_authority_approvals() {
    let artifacts = artifacts();
    let receipt = receipt(&artifacts);
    let manifest = manifest(artifacts, &receipt);
    let keys = authority_keys();
    let policy = authority_policy(&keys, 2);
    let unknown = KeyPair::from_seed(vec![0x99; 32], Algorithm::Ed25519);

    let unknown_attestation = release_attestation(
        &manifest,
        &receipt,
        &policy,
        core::slice::from_ref(&unknown),
    );
    assert_eq!(
        manifest.authenticate(&receipt, &policy, &unknown_attestation),
        Err(OfflineCashReleaseErrorV1::UnknownSigner)
    );

    let insufficient = release_attestation(&manifest, &receipt, &policy, &keys[..1]);
    assert_eq!(
        manifest.authenticate(&receipt, &policy, &insufficient),
        Err(OfflineCashReleaseErrorV1::InsufficientThreshold {
            collected: 1,
            required: 2,
        })
    );

    let mut invalid_signature = release_attestation(&manifest, &receipt, &policy, &keys[..2]);
    invalid_signature.approvals[0].signature = SignatureOf::try_new(
        unknown.private_key(),
        &invalid_signature.subject.approval_payload(),
    )
    .expect("mismatched release signature");
    assert_eq!(
        manifest.authenticate(&receipt, &policy, &invalid_signature),
        Err(OfflineCashReleaseErrorV1::InvalidSignature)
    );
}

#[test]
fn exact_release_decoders_reject_outer_caps_and_forged_lengths() {
    assert_eq!(
        OfflineCashInternalValidationReceiptV1::decode_canonical_exact(&vec![
            0;
            OFFLINE_CASH_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1
                + 1
        ]),
        Err(OfflineCashReleaseErrorV1::InvalidValidationReceipt)
    );
    assert_eq!(
        OfflineCashReleaseManifestV1::decode_canonical_exact(&vec![
            0;
            OFFLINE_CASH_RELEASE_MANIFEST_MAX_BYTES_V1
                + 1
        ]),
        Err(OfflineCashReleaseErrorV1::InvalidManifest)
    );

    const PAYLOAD_LENGTH_OFFSET: usize = 4 + 1 + 1 + 16 + 1;
    const PAYLOAD_LENGTH_END: usize = PAYLOAD_LENGTH_OFFSET + 8;
    let artifacts = artifacts();
    let receipt = receipt(&artifacts);
    let manifest = manifest(artifacts, &receipt);
    let mut bytes = norito::encode_canonical(&manifest).expect("encode manifest");
    bytes[PAYLOAD_LENGTH_OFFSET..PAYLOAD_LENGTH_END].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(OfflineCashReleaseManifestV1::decode_canonical_exact(&bytes).is_err());
}

#[test]
fn manifest_and_receipt_reject_semantic_profile_caps() {
    let artifacts = artifacts();
    let base = receipt(&artifacts);
    let mut receipt = base.clone();
    let vk_digest = base.profile_qualifications[0].profile.vk_digest;
    while receipt.profile_qualifications.len() <= OFFLINE_CASH_RELEASE_MAX_ENABLED_PROFILES_V1 {
        let seed = 0x50
            + u8::try_from(receipt.profile_qualifications.len()).expect("small profile fixture");
        receipt.profile_qualifications.push(profile_qualification(
            enabled_profile(seed, vk_digest),
            &artifacts,
            &base.helper_protocols,
            seed.wrapping_add(0x40),
        ));
    }
    receipt
        .profile_qualifications
        .sort_by_key(|qualification| qualification.profile.hardware_profile_id);
    assert_eq!(
        receipt.validate(),
        Err(OfflineCashReleaseErrorV1::InvalidValidationReceipt)
    );

    let mut manifest = manifest(artifacts, &base);
    manifest.enabled_profiles = receipt
        .profile_qualifications
        .iter()
        .map(|qualification| qualification.profile)
        .collect();
    manifest.release_id = manifest
        .expected_release_id()
        .expect("oversized profile id");
    assert_eq!(
        manifest.validate_standalone(),
        Err(OfflineCashReleaseErrorV1::InvalidManifest)
    );
}

#[test]
fn maximal_64_profile_manifest_fits_the_64_kib_admission_cap() {
    assert_eq!(OFFLINE_CASH_RELEASE_MANIFEST_MAX_BYTES_V1, 64 * 1024);
    let artifacts = artifacts();
    let receipt =
        receipt_with_profile_count(&artifacts, OFFLINE_CASH_RELEASE_MAX_ENABLED_PROFILES_V1);
    receipt.validate().expect("64-profile receipt");
    let receipt_bytes = norito::encode_canonical(&receipt).expect("encode 64-profile receipt");
    assert!(receipt_bytes.len() <= OFFLINE_CASH_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1);

    let manifest = manifest(artifacts, &receipt);
    assert_eq!(
        manifest.enabled_profiles.len(),
        OFFLINE_CASH_RELEASE_MAX_ENABLED_PROFILES_V1
    );
    let manifest_bytes = norito::encode_canonical(&manifest).expect("encode 64-profile manifest");
    assert!(
        manifest_bytes.len() > 16 * 1024,
        "complete embedded hardware profiles exceed the retired 16-KiB budget"
    );
    assert!(manifest_bytes.len() <= OFFLINE_CASH_RELEASE_MANIFEST_MAX_BYTES_V1);
    assert_eq!(
        OfflineCashReleaseManifestV1::decode_canonical_exact(&manifest_bytes)
            .expect("admit maximal profile manifest"),
        manifest
    );
}

#[test]
fn evidence_closure_and_release_profile_are_receipt_authority_inputs() {
    let artifacts = artifacts();
    let base = receipt(&artifacts);
    base.validate()
        .expect("complete signed-observation projection");
    let base_digest = base.canonical_digest().expect("base receipt digest");

    let mut changed_observations = base.clone();
    changed_observations
        .evidence_closure
        .verification_records_digest = [0xF3; 32];
    assert_ne!(
        changed_observations
            .canonical_digest()
            .expect("changed observation receipt"),
        base_digest
    );

    let mut changed_candidate = base.clone();
    changed_candidate.evidence_closure.candidate_context_digest = [0xF7; 32];
    assert_ne!(
        changed_candidate
            .canonical_digest()
            .expect("changed candidate-context receipt"),
        base_digest
    );
    let mut missing_candidate = base.clone();
    missing_candidate.evidence_closure.candidate_context_digest = [0; 32];
    assert_eq!(
        missing_candidate.validate(),
        Err(OfflineCashReleaseErrorV1::InvalidValidationReceipt)
    );

    let mut changed_manifest = base.clone();
    changed_manifest.evidence_closure.evidence_manifest.sha256 = [0xF4; 32];
    assert_ne!(
        changed_manifest
            .canonical_digest()
            .expect("changed manifest receipt"),
        base_digest
    );

    let mut changed_policy = base.clone();
    changed_policy.evidence_closure.observer_policy.sha256 = [0xF5; 32];
    assert_ne!(
        changed_policy
            .canonical_digest()
            .expect("changed observer policy receipt"),
        base_digest
    );

    let mut excessive = base.clone();
    excessive.evidence_closure.total_transcript_bytes =
        OFFLINE_CASH_RELEASE_TRANSCRIPT_TOTAL_MAX_BYTES_V1 + 1;
    assert_eq!(
        excessive.validate(),
        Err(OfflineCashReleaseErrorV1::InvalidValidationReceipt)
    );

    let mut substituted_shape = base;
    substituted_shape.circuit_shape_report.sha256 = [0xF6; 32];
    assert_eq!(
        substituted_shape.validate(),
        Err(OfflineCashReleaseErrorV1::InvalidValidationReceipt)
    );
}

#[test]
fn release_evidence_python_projection_digest_goldens_match_rust_norito() {
    let artifacts = artifacts();
    assert_eq!(
        offline_cash_artifact_set_digest_v1(&artifacts).expect("artifact-set digest"),
        [
            0xC2, 0xE2, 0x30, 0x1C, 0x23, 0x41, 0x17, 0x7A, 0xB4, 0x5E, 0x50, 0xA3, 0x9F, 0x63,
            0x13, 0xDF, 0x3C, 0xAE, 0x77, 0xB2, 0xB6, 0x55, 0x91, 0x8C, 0x62, 0xC1, 0xAE, 0x53,
            0x50, 0xC7, 0xF4, 0x6F,
        ]
    );
    let helper_protocols = helper_protocols();
    assert_eq!(
        offline_cash_vk_set_digest_v1(
            &artifacts,
            STATE_EQ_PROTOCOL_DIGEST,
            STATE_EP_PROTOCOL_DIGEST,
            WRAPPER_EQ_PROTOCOL_DIGEST,
            WRAPPER_EP_PROTOCOL_DIGEST,
            &helper_protocols,
        )
        .expect("VK-set digest"),
        [
            0xA5, 0x0F, 0x9E, 0xF0, 0x0D, 0x78, 0xF4, 0x88, 0x9B, 0xC4, 0xDA, 0x2E, 0x61, 0x1E,
            0x25, 0xE9, 0xB1, 0x9E, 0x97, 0x27, 0x96, 0x82, 0xBD, 0x65, 0x91, 0x67, 0x05, 0x61,
            0xBA, 0x9D, 0xC4, 0xB1,
        ]
    );
    assert_eq!(
        offline_cash_release_profile_digest_v1(
            evidence(5),
            STATE_EQ_PROTOCOL_DIGEST,
            STATE_EP_PROTOCOL_DIGEST,
            WRAPPER_EQ_PROTOCOL_DIGEST,
            WRAPPER_EP_PROTOCOL_DIGEST,
            &helper_protocols,
        )
        .expect("release profile digest"),
        [
            0xD4, 0xC6, 0x03, 0x71, 0x47, 0x28, 0xFF, 0x42, 0x08, 0xE1, 0xDE, 0x2C, 0x74, 0x19,
            0xF6, 0x9D, 0x58, 0x93, 0xF5, 0x59, 0x3F, 0xCF, 0x29, 0x97, 0x0F, 0x68, 0x21, 0xB6,
            0xC9, 0xEF, 0x0F, 0xC1,
        ]
    );
}
