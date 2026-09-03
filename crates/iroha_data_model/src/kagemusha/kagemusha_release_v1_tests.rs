//! Release-schema tests for the closed KAGEMUSHA V1 artifact and qualification inventory.

use std::collections::BTreeSet;

use super::*;
use crate::kagemusha::{
    KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1, KagemushaDevicePublicKeyV1,
    KagemushaHardwarePlatformClassV1, KagemushaHardwareProfileV1,
};
use iroha_crypto::{Algorithm, KeyPair};
use p256::ecdsa::SigningKey;

const STATE_EQ_PROTOCOL_DIGEST: [u8; 32] = [0x31; 32];
const STATE_EP_PROTOCOL_DIGEST: [u8; 32] = [0x32; 32];
const TERMINAL_AUTHORIZATION_EQ_PROTOCOL_DIGEST: [u8; 32] = [0x33; 32];
const TERMINAL_AUTHORIZATION_EP_PROTOCOL_DIGEST: [u8; 32] = [0x34; 32];
const MINT_AUTHORIZATION_EQ_PROTOCOL_DIGEST: [u8; 32] = [0x3B; 32];
const MINT_AUTHORIZATION_EP_PROTOCOL_DIGEST: [u8; 32] = [0x3C; 32];
const MINT_EQ_PROTOCOL_DIGEST: [u8; 32] = [0x35; 32];
const MINT_EP_PROTOCOL_DIGEST: [u8; 32] = [0x36; 32];
const CREDENTIAL_EQ_PROTOCOL_DIGEST: [u8; 32] = [0x37; 32];
const CREDENTIAL_EP_PROTOCOL_DIGEST: [u8; 32] = [0x38; 32];
const GUARD_EQ_PROTOCOL_DIGEST: [u8; 32] = [0x39; 32];
const GUARD_EP_PROTOCOL_DIGEST: [u8; 32] = [0x3A; 32];
const COMMIT_WRAPPER_EQ_PROTOCOL_DIGEST: [u8; 32] = [0x3D; 32];
const COMMIT_WRAPPER_EP_PROTOCOL_DIGEST: [u8; 32] = [0x3E; 32];
const CREDENTIAL_EQ_PROOF_BYTES: u32 = 8_000;
const CREDENTIAL_EP_PROOF_BYTES: u32 = 8_032;
const GUARD_EQ_PROOF_BYTES: u32 = 12_000;
const GUARD_EP_PROOF_BYTES: u32 = 12_032;

fn helper_protocols() -> Vec<KagemushaHelperProtocolV1> {
    vec![
        KagemushaHelperProtocolV1 {
            helper: KagemushaQualifiedHelperCircuitV1::MintAuthorization,
            eq_protocol_digest: MINT_AUTHORIZATION_EQ_PROTOCOL_DIGEST,
            ep_protocol_digest: MINT_AUTHORIZATION_EP_PROTOCOL_DIGEST,
            eq_proof_bytes: 0,
            ep_proof_bytes: 0,
        },
        KagemushaHelperProtocolV1 {
            helper: KagemushaQualifiedHelperCircuitV1::MintCredit,
            eq_protocol_digest: MINT_EQ_PROTOCOL_DIGEST,
            ep_protocol_digest: MINT_EP_PROTOCOL_DIGEST,
            eq_proof_bytes: 0,
            ep_proof_bytes: 0,
        },
        KagemushaHelperProtocolV1 {
            helper: KagemushaQualifiedHelperCircuitV1::PlatformCredential,
            eq_protocol_digest: CREDENTIAL_EQ_PROTOCOL_DIGEST,
            ep_protocol_digest: CREDENTIAL_EP_PROTOCOL_DIGEST,
            eq_proof_bytes: CREDENTIAL_EQ_PROOF_BYTES,
            ep_proof_bytes: CREDENTIAL_EP_PROOF_BYTES,
        },
        KagemushaHelperProtocolV1 {
            helper: KagemushaQualifiedHelperCircuitV1::GuardBundle,
            eq_protocol_digest: GUARD_EQ_PROTOCOL_DIGEST,
            ep_protocol_digest: GUARD_EP_PROTOCOL_DIGEST,
            eq_proof_bytes: GUARD_EQ_PROOF_BYTES,
            ep_proof_bytes: GUARD_EP_PROOF_BYTES,
        },
    ]
}

fn evidence(seed: u8) -> KagemushaEvidenceFileV1 {
    let seed = if seed == 0 { u8::MAX } else { seed };
    KagemushaEvidenceFileV1 {
        sha256: [seed; 32],
        byte_len: 1_000 + u64::from(seed),
    }
}

fn artifacts() -> Vec<KagemushaArtifactBindingV1> {
    KagemushaArtifactRoleV1::ALL
        .iter()
        .copied()
        .enumerate()
        .map(|(index, role)| KagemushaArtifactBindingV1 {
            role,
            sha256: [u8::try_from(index + 1).expect("small role index"); 32],
            byte_len: if role.is_params() {
                KAGEMUSHA_PARAMS_BYTES_V1
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
    artifacts: &[KagemushaArtifactBindingV1],
    role: KagemushaArtifactRoleV1,
) -> KagemushaArtifactBindingV1 {
    *artifacts
        .iter()
        .find(|artifact| artifact.role == role)
        .expect("fixture contains every artifact role")
}

fn device_public_key(seed: u8) -> KagemushaDevicePublicKeyV1 {
    let signing_key = SigningKey::from_bytes((&[seed; 32]).into()).expect("P-256 signing key");
    let encoded = signing_key.verifying_key().to_encoded_point(false);
    KagemushaDevicePublicKeyV1::from_sec1_bytes(encoded.as_bytes()).expect("device public key")
}

fn hardware_profile(
    seed: u8,
    suite_id: [u8; 32],
    qualification_report_digest: [u8; 32],
) -> KagemushaHardwareProfileV1 {
    KagemushaHardwareProfileV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        protocol_version: KAGEMUSHA_WIRE_VERSION_V1,
        hardware_profile_id: [0; 32],
        provider_id: [seed; 32],
        platform_class: KagemushaHardwarePlatformClassV1::DedicatedSecureElement,
        product_class_digest: [seed.wrapping_add(1); 32],
        firmware_policy_digest: [seed.wrapping_add(2); 32],
        enrollment_attestation_verifier_digest: [seed.wrapping_add(3); 32],
        attestation_trust_roots_digest: [seed.wrapping_add(4); 32],
        allowed_suite_commitment: kagemusha_suite_commitment_v1(suite_id),
        policy_epoch: u64::from(seed),
        governance_credential_public_key: device_public_key(seed.wrapping_add(5)),
        capability_mask: KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1,
        qualification_report_digest,
        valid_from_ms: 1,
        expires_at_ms: 100_000,
    }
    .seal_hardware_profile_id()
    .expect("hardware profile identity")
}

fn enabled_profile(seed: u8, vk_digest: [u8; 32]) -> KagemushaEnabledProfileV1 {
    let suite_id = [seed.wrapping_add(0x10); 32];
    let qualification_report = evidence(seed.wrapping_add(0x20));
    let hardware_profile = hardware_profile(seed, suite_id, qualification_report.sha256);
    KagemushaEnabledProfileV1 {
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
    profile: KagemushaEnabledProfileV1,
    artifacts: &[KagemushaArtifactBindingV1],
    helper_protocols: &[KagemushaHelperProtocolV1],
    report_seed: u8,
) -> KagemushaProfileQualificationV1 {
    let relations = KagemushaQualifiedRelationV1::ALL
        .iter()
        .copied()
        .enumerate()
        .map(|(index, relation)| {
            let (eq_role, ep_role) = relation.expected_vk_roles();
            let (eq_protocol_digest, ep_protocol_digest) = match relation {
                KagemushaQualifiedRelationV1::TerminalAuthorization => (
                    TERMINAL_AUTHORIZATION_EQ_PROTOCOL_DIGEST,
                    TERMINAL_AUTHORIZATION_EP_PROTOCOL_DIGEST,
                ),
                KagemushaQualifiedRelationV1::CommitWrapper => (
                    COMMIT_WRAPPER_EQ_PROTOCOL_DIGEST,
                    COMMIT_WRAPPER_EP_PROTOCOL_DIGEST,
                ),
                KagemushaQualifiedRelationV1::Bootstrap
                | KagemushaQualifiedRelationV1::MintFold
                | KagemushaQualifiedRelationV1::SendSplit
                | KagemushaQualifiedRelationV1::ReceiveFoldBatch
                | KagemushaQualifiedRelationV1::RedeemSplit
                | KagemushaQualifiedRelationV1::SuiteUpgrade
                | KagemushaQualifiedRelationV1::Rotate => {
                    (STATE_EQ_PROTOCOL_DIGEST, STATE_EP_PROTOCOL_DIGEST)
                }
            };
            KagemushaRelationQualificationV1 {
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
            KagemushaHelperQualificationV1 {
                helper: protocol.helper,
                eq_protocol_digest: protocol.eq_protocol_digest,
                ep_protocol_digest: protocol.ep_protocol_digest,
                eq_verifying_key: artifact(artifacts, eq_role),
                ep_verifying_key: artifact(artifacts, ep_role),
                eq_circuit_rows: 32_000,
                ep_circuit_rows: 32_000,
                eq_proof_bytes: protocol.eq_proof_bytes,
                ep_proof_bytes: protocol.ep_proof_bytes,
                complete_proof_bytes: if matches!(
                    protocol.helper,
                    KagemushaQualifiedHelperCircuitV1::PlatformCredential
                        | KagemushaQualifiedHelperCircuitV1::GuardBundle
                ) {
                    protocol
                        .eq_proof_bytes
                        .checked_add(protocol.ep_proof_bytes)
                        .expect("bounded internal helper proof lengths")
                } else {
                    4_000
                },
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
    let recursive_depths = [8_u32, 64, 1_024, 2_048]
        .into_iter()
        .enumerate()
        .map(|(index, depth)| KagemushaRecursiveDepthQualificationV1 {
            depth,
            verified_handoffs: depth,
            complete_proof_bytes: 6_000,
            raw_complete_exchange_bytes: 9_000,
            text_complete_exchange_bytes: 12_000,
            report: evidence(
                report_seed.wrapping_add(0x30 + u8::try_from(index).expect("small depth index")),
            ),
        })
        .collect();
    let receive_fold_occupancies = (1..=KAGEMUSHA_RECEIVE_FOLD_BATCH_WIDTH_V1)
        .enumerate()
        .map(|(index, occupancy)| KagemushaReceiveFoldOccupancyV1 {
            occupancy,
            complete_proof_bytes: 6_000,
            report: evidence(
                report_seed
                    .wrapping_add(0x20)
                    .wrapping_add(u8::try_from(index).expect("sixteen occupancy indexes fit u8")),
            ),
        })
        .collect();
    let acceptance_cases = KagemushaAcceptanceCaseV1::ALL
        .iter()
        .copied()
        .enumerate()
        .map(|(index, case)| KagemushaAcceptanceCaseEvidenceV1 {
            case,
            validator_count: if matches!(
                case,
                KagemushaAcceptanceCaseV1::FourPeerActivationRestartReplay
            ) {
                KAGEMUSHA_VALIDATOR_COUNT_V1
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
    KagemushaProfileQualificationV1 {
        profile,
        relations,
        helper_circuits,
        receive_fold_occupancies,
        recursive_depths,
        aggregate_balance: KagemushaAggregateBalanceQualificationV1 {
            independent_payments: KAGEMUSHA_MIN_QUALIFIED_AGGREGATED_CREDITS_V1,
            folded_credits: KAGEMUSHA_MIN_QUALIFIED_AGGREGATED_CREDITS_V1,
            spend_payments: 1,
            report: evidence(report_seed.wrapping_add(0x40)),
        },
        thermal: KagemushaThermalQualificationV1 {
            folded_credits: KAGEMUSHA_MIN_THERMAL_FOLDED_CREDITS_V1,
            fold_p95_ms: 9_500,
            process_rss_bytes: 120 * 1024 * 1024,
            operation_energy_millijoules: 11_000,
            report: evidence(report_seed.wrapping_add(0x41)),
        },
        envelope: KagemushaEnvelopeQualificationV1 {
            raw_complete_exchange_bytes: 9_000,
            text_complete_exchange_bytes: 12_000,
            handoff_p95_ms: 29_000,
            report: evidence(report_seed.wrapping_add(0x42)),
        },
        acceptance_cases,
    }
    .seal_qualification_digest()
    .expect("profile qualification digest")
}

fn receipt(artifacts: &[KagemushaArtifactBindingV1]) -> KagemushaInternalValidationReceiptV1 {
    let artifact_set_digest = kagemusha_artifact_set_digest_v1(artifacts).expect("artifact digest");
    let helper_protocols = helper_protocols();
    let vk_digest = kagemusha_vk_set_digest_v1(
        artifacts,
        STATE_EQ_PROTOCOL_DIGEST,
        STATE_EP_PROTOCOL_DIGEST,
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
        kagemusha_hardware_policy_digest_v1(&enabled_profiles).expect("hardware policy digest");
    let circuit_shape_report = evidence(5);
    let profile_digest = kagemusha_release_profile_digest_v1(
        circuit_shape_report,
        STATE_EQ_PROTOCOL_DIGEST,
        STATE_EP_PROTOCOL_DIGEST,
        &helper_protocols,
    )
    .expect("release profile digest");
    KagemushaInternalValidationReceiptV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        source_tree_digest: [1; 32],
        cargo_lock_digest: [2; 32],
        profile_digest,
        eq_protocol_digest: STATE_EQ_PROTOCOL_DIGEST,
        ep_protocol_digest: STATE_EP_PROTOCOL_DIGEST,
        artifact_set_digest,
        hardware_policy_digest,
        evidence_closure: KagemushaEvidenceClosureV1 {
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
            KagemushaReproducibleBuildV1 {
                builder_id: [0xD1; 32],
                artifact_set_digest,
                report: evidence(0xD3),
            },
            KagemushaReproducibleBuildV1 {
                builder_id: [0xD2; 32],
                artifact_set_digest,
                report: evidence(0xD4),
            },
        ],
        fuzz_cases: KAGEMUSHA_MIN_FUZZ_CASES_V1,
    }
}

fn receipt_with_profile_count(
    artifacts: &[KagemushaArtifactBindingV1],
    profile_count: usize,
) -> KagemushaInternalValidationReceiptV1 {
    assert!((1..=KAGEMUSHA_RELEASE_MAX_ENABLED_PROFILES_V1).contains(&profile_count));
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
        kagemusha_hardware_policy_digest_v1(&enabled_profiles).expect("bounded hardware policy");
    receipt
}

fn reseal_profile_qualification(receipt: &mut KagemushaInternalValidationReceiptV1, index: usize) {
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
        kagemusha_hardware_policy_digest_v1(&enabled_profiles).expect("reseal hardware policy");
}

fn manifest(
    artifacts: Vec<KagemushaArtifactBindingV1>,
    receipt: &KagemushaInternalValidationReceiptV1,
) -> KagemushaReleaseManifestV1 {
    KagemushaReleaseManifestV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        release_id: [0; 32],
        source_tree_digest: receipt.source_tree_digest,
        cargo_lock_digest: receipt.cargo_lock_digest,
        profile_digest: receipt.profile_digest,
        eq_protocol_digest: receipt.eq_protocol_digest,
        ep_protocol_digest: receipt.ep_protocol_digest,
        hardware_policy_digest: receipt.hardware_policy_digest,
        validation_receipt_digest: receipt.canonical_digest().expect("receipt digest"),
        halo2_k: KAGEMUSHA_HALO2_K_V1,
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

fn authority_policy(keys: &[KeyPair], threshold: u16) -> KagemushaReleaseAuthorityPolicyV1 {
    KagemushaReleaseAuthorityPolicyV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        authority_set_id: [0x40; 32],
        threshold,
        authorized_signers: keys.iter().map(|key| key.public_key().clone()).collect(),
    }
}

fn release_attestation(
    manifest: &KagemushaReleaseManifestV1,
    receipt: &KagemushaInternalValidationReceiptV1,
    policy: &KagemushaReleaseAuthorityPolicyV1,
    signing_keys: &[KeyPair],
) -> KagemushaReleaseAttestationV1 {
    let subject = manifest
        .release_attestation_subject(receipt, policy)
        .expect("release attestation subject");
    let payload = subject.approval_payload();
    KagemushaReleaseAttestationV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        subject,
        approvals: signing_keys
            .iter()
            .map(|key| KagemushaReleaseApprovalV1 {
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
    let expected_vk_digest = kagemusha_vk_set_digest_v1(
        &artifacts,
        receipt.eq_protocol_digest,
        receipt.ep_protocol_digest,
        &receipt.helper_protocols,
    )
    .expect("expected VK set");
    let manifest = manifest(artifacts, &receipt);
    let receipt_bytes = norito::encode_canonical(&receipt).expect("encode receipt");
    assert!(receipt_bytes.len() <= KAGEMUSHA_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1);
    let decoded_receipt =
        KagemushaInternalValidationReceiptV1::decode_canonical_exact(&receipt_bytes)
            .expect("decode receipt");
    assert_eq!(decoded_receipt, receipt);

    let manifest_bytes = norito::encode_canonical(&manifest).expect("encode manifest");
    let decoded_manifest = KagemushaReleaseManifestV1::decode_canonical_exact(&manifest_bytes)
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
    assert_eq!(authenticated.eq_protocol_digest(), STATE_EQ_PROTOCOL_DIGEST);
    assert_eq!(authenticated.ep_protocol_digest(), STATE_EP_PROTOCOL_DIGEST);
    assert_eq!(authenticated.helper_protocols(), receipt.helper_protocols);
    let mint_authorization = authenticated
        .helper_protocol(KagemushaQualifiedHelperCircuitV1::MintAuthorization)
        .expect("mint-authorization helper protocol");
    assert_eq!(
        mint_authorization.helper,
        KagemushaQualifiedHelperCircuitV1::MintAuthorization
    );
    assert!(
        authenticated
            .helper_protocols()
            .iter()
            .all(|protocol| authenticated.helper_protocol(protocol.helper) == Some(protocol))
    );
    assert_eq!(
        authenticated
            .artifact(KagemushaArtifactRoleV1::InnerStateVkEp)
            .role,
        KagemushaArtifactRoleV1::InnerStateVkEp
    );
}

#[test]
fn vk_qualification_and_hardware_policy_digests_bind_exact_content() {
    let artifacts = artifacts();
    let base = receipt(&artifacts);
    let expected_vk_digest = kagemusha_vk_set_digest_v1(
        &artifacts,
        base.eq_protocol_digest,
        base.ep_protocol_digest,
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
        .find(|artifact| artifact.role == KagemushaArtifactRoleV1::StateVkEq)
        .expect("state Eq VK")
        .sha256 = [0xF0; 32];
    assert_ne!(
        kagemusha_vk_set_digest_v1(
            &changed_vk_artifact,
            base.eq_protocol_digest,
            base.ep_protocol_digest,
            &base.helper_protocols,
        )
        .expect("changed VK-set digest"),
        expected_vk_digest
    );

    let mut duplicate_any_artifact = artifacts.clone();
    let params_digest = artifact(&duplicate_any_artifact, KagemushaArtifactRoleV1::ParamsEq).sha256;
    duplicate_any_artifact
        .iter_mut()
        .find(|artifact| artifact.role == KagemushaArtifactRoleV1::MintCreditPkEq)
        .expect("mint-credit proving key")
        .sha256 = params_digest;
    assert_eq!(
        kagemusha_artifact_set_digest_v1(&duplicate_any_artifact),
        Err(KagemushaReleaseErrorV1::InvalidArtifactSet)
    );

    let mut inconsistent_receipt_vk = base.clone();
    inconsistent_receipt_vk.profile_qualifications[0]
        .profile
        .vk_digest = [0xF1; 32];
    reseal_profile_qualification(&mut inconsistent_receipt_vk, 0);
    assert_eq!(
        inconsistent_receipt_vk.validate(),
        Err(KagemushaReleaseErrorV1::InvalidValidationReceipt)
    );

    let mut forged_policy = base.clone();
    forged_policy.hardware_policy_digest = [0xF2; 32];
    assert_eq!(
        forged_policy.validate(),
        Err(KagemushaReleaseErrorV1::InvalidValidationReceipt)
    );

    let mut invalid_embedded_profile = base.clone();
    invalid_embedded_profile.profile_qualifications[0]
        .profile
        .hardware_profile
        .provider_id = [0xF3; 32];
    assert_eq!(
        invalid_embedded_profile.validate(),
        Err(KagemushaReleaseErrorV1::InvalidValidationReceipt)
    );

    let mut mismatched_policy_epoch = base.clone();
    mismatched_policy_epoch.profile_qualifications[0]
        .profile
        .policy_epoch += 1;
    assert_eq!(
        mismatched_policy_epoch.validate(),
        Err(KagemushaReleaseErrorV1::InvalidValidationReceipt)
    );

    let mut mismatched_suite = base.clone();
    mismatched_suite.profile_qualifications[0].profile.suite_id = [0xF4; 32];
    assert_eq!(
        mismatched_suite.validate(),
        Err(KagemushaReleaseErrorV1::InvalidValidationReceipt)
    );

    let mut mismatched_report = base.clone();
    mismatched_report.profile_qualifications[0]
        .profile
        .qualification_report
        .sha256 = [0xF5; 32];
    assert_eq!(
        mismatched_report.validate(),
        Err(KagemushaReleaseErrorV1::InvalidValidationReceipt)
    );

    let mut manifest_vk = manifest(artifacts, &base);
    for profile in &mut manifest_vk.enabled_profiles {
        profile.vk_digest = [0xF6; 32];
    }
    manifest_vk.hardware_policy_digest =
        kagemusha_hardware_policy_digest_v1(&manifest_vk.enabled_profiles)
            .expect("forged policy remains structurally canonical");
    manifest_vk.release_id = [0; 32];
    manifest_vk = manifest_vk.seal().expect("reseal forged manifest");
    assert_eq!(
        manifest_vk.validate_standalone(),
        Err(KagemushaReleaseErrorV1::InvalidManifest)
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
        Err(KagemushaReleaseErrorV1::InvalidManifest)
    );

    let mut changed_state_protocol = manifest.clone();
    changed_state_protocol.eq_protocol_digest = [0xED; 32];
    changed_state_protocol.release_id = [0; 32];
    changed_state_protocol = changed_state_protocol
        .seal()
        .expect("reseal state protocol");
    assert_ne!(manifest.release_id, changed_state_protocol.release_id);

    let mut unordered = manifest;
    unordered.enabled_profiles.reverse();
    unordered.release_id = unordered.expected_release_id().expect("unordered id");
    assert_eq!(
        unordered.validate_standalone(),
        Err(KagemushaReleaseErrorV1::InvalidManifest)
    );

    let mut duplicate = receipt;
    duplicate.profile_qualifications[1].profile = duplicate.profile_qualifications[0].profile;
    assert_eq!(
        duplicate.validate(),
        Err(KagemushaReleaseErrorV1::InvalidValidationReceipt)
    );
}

#[test]
fn relations_are_closed_ordered_and_bind_exact_verifier_artifacts() {
    let artifacts = artifacts();
    let base = receipt(&artifacts);

    let mut missing = base.clone();
    missing.profile_qualifications[0].relations.pop();
    reseal_profile_qualification(&mut missing, 0);
    assert!(missing.validate().is_err());

    let mut unordered = base.clone();
    unordered.profile_qualifications[0].relations.swap(0, 1);
    reseal_profile_qualification(&mut unordered, 0);
    assert!(unordered.validate().is_err());

    let mut wrong_inner_state_role = base.clone();
    wrong_inner_state_role.profile_qualifications[0].relations[0].eq_verifying_key =
        artifact(&artifacts, KagemushaArtifactRoleV1::InnerStateVkEq);
    reseal_profile_qualification(&mut wrong_inner_state_role, 0);
    assert!(wrong_inner_state_role.validate().is_err());

    let mut wrong_protocol = base.clone();
    wrong_protocol.profile_qualifications[0].relations[0].eq_protocol_digest =
        MINT_AUTHORIZATION_EQ_PROTOCOL_DIGEST;
    reseal_profile_qualification(&mut wrong_protocol, 0);
    assert!(wrong_protocol.validate().is_err());

    let mut helper_protocol_on_direct_relation = base.clone();
    helper_protocol_on_direct_relation.profile_qualifications[0].relations[0].ep_protocol_digest =
        GUARD_EP_PROTOCOL_DIGEST;
    reseal_profile_qualification(&mut helper_protocol_on_direct_relation, 0);
    assert!(helper_protocol_on_direct_relation.validate().is_err());

    let mut terminal_authorization_using_commit_wrapper_key = base.clone();
    let terminal_authorization = terminal_authorization_using_commit_wrapper_key
        .profile_qualifications[0]
        .relations
        .iter_mut()
        .find(|relation| relation.relation == KagemushaQualifiedRelationV1::TerminalAuthorization)
        .expect("terminal-authorization relation");
    terminal_authorization.eq_verifying_key =
        artifact(&artifacts, KagemushaArtifactRoleV1::CommitWrapperVkEq);
    reseal_profile_qualification(&mut terminal_authorization_using_commit_wrapper_key, 0);
    assert!(
        terminal_authorization_using_commit_wrapper_key
            .validate()
            .is_err()
    );

    let mut aliased_relation_protocol = base.clone();
    let terminal_authorization = aliased_relation_protocol.profile_qualifications[0]
        .relations
        .iter_mut()
        .find(|relation| relation.relation == KagemushaQualifiedRelationV1::TerminalAuthorization)
        .expect("terminal-authorization relation");
    terminal_authorization.eq_protocol_digest = COMMIT_WRAPPER_EQ_PROTOCOL_DIGEST;
    reseal_profile_qualification(&mut aliased_relation_protocol, 0);
    assert!(aliased_relation_protocol.validate().is_err());

    let mut zero_relation_protocol = base.clone();
    let acceptance = zero_relation_protocol.profile_qualifications[0]
        .relations
        .iter_mut()
        .find(|relation| relation.relation == KagemushaQualifiedRelationV1::CommitWrapper)
        .expect("post-commit wrapper relation");
    acceptance.ep_protocol_digest = [0; 32];
    reseal_profile_qualification(&mut zero_relation_protocol, 0);
    assert!(zero_relation_protocol.validate().is_err());

    let mut parity_aliased_relation_protocol = base.clone();
    let acceptance = parity_aliased_relation_protocol.profile_qualifications[0]
        .relations
        .iter_mut()
        .find(|relation| relation.relation == KagemushaQualifiedRelationV1::CommitWrapper)
        .expect("post-commit wrapper relation");
    acceptance.ep_protocol_digest = acceptance.eq_protocol_digest;
    reseal_profile_qualification(&mut parity_aliased_relation_protocol, 0);
    assert!(parity_aliased_relation_protocol.validate().is_err());

    let mut inconsistent_direct_profile_protocol = base.clone();
    inconsistent_direct_profile_protocol.profile_qualifications[1].relations[0]
        .eq_protocol_digest = [0xF1; 32];
    reseal_profile_qualification(&mut inconsistent_direct_profile_protocol, 1);
    assert!(inconsistent_direct_profile_protocol.validate().is_err());

    let mut inconsistent_profile_protocol = base.clone();
    let terminal_authorization = inconsistent_profile_protocol.profile_qualifications[1]
        .relations
        .iter_mut()
        .find(|relation| relation.relation == KagemushaQualifiedRelationV1::TerminalAuthorization)
        .expect("terminal-authorization relation");
    terminal_authorization.eq_protocol_digest = [0xF2; 32];
    reseal_profile_qualification(&mut inconsistent_profile_protocol, 1);
    assert!(inconsistent_profile_protocol.validate().is_err());

    let mut exact_limits = base.clone();
    let relation = &mut exact_limits.profile_qualifications[0].relations[0];
    relation.complete_proof_bytes =
        u32::try_from(KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1).expect("proof cap fits u32");
    relation.eq_circuit_rows = 1_u32 << KAGEMUSHA_HALO2_K_V1;
    relation.eq_verifying_key.byte_len = KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1;
    reseal_profile_qualification(&mut exact_limits, 0);
    exact_limits.validate().expect("exact relation limits pass");
    exact_limits.profile_qualifications[0].relations[0].complete_proof_bytes += 1;
    reseal_profile_qualification(&mut exact_limits, 0);
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
        Err(KagemushaReleaseErrorV1::InvalidManifest)
    );

    let mut substituted_terminal_authorization_vk = base.clone();
    substituted_terminal_authorization_vk.profile_qualifications[0]
        .relations
        .iter_mut()
        .find(|relation| relation.relation == KagemushaQualifiedRelationV1::TerminalAuthorization)
        .expect("terminal-authorization relation")
        .eq_verifying_key
        .sha256 = [0xEE; 32];
    reseal_profile_qualification(&mut substituted_terminal_authorization_vk, 0);
    let substituted_manifest = manifest(artifacts.clone(), &substituted_terminal_authorization_vk);
    assert_eq!(
        substituted_manifest.release_attestation_subject(
            &substituted_terminal_authorization_vk,
            &authority_policy(&authority_keys(), 1),
        ),
        Err(KagemushaReleaseErrorV1::InvalidManifest)
    );

    let mut aliased_inner_outer = artifacts;
    let outer_state_eq = artifact(&aliased_inner_outer, KagemushaArtifactRoleV1::StateVkEq).sha256;
    let inner_state_eq = aliased_inner_outer
        .iter_mut()
        .find(|binding| binding.role == KagemushaArtifactRoleV1::InnerStateVkEq)
        .expect("inner-state Eq binding");
    inner_state_eq.sha256 = outer_state_eq;
    assert_eq!(
        kagemusha_artifact_set_digest_v1(&aliased_inner_outer),
        Err(KagemushaReleaseErrorV1::InvalidArtifactSet)
    );
}

#[test]
fn artifact_and_relation_inventories_are_frozen() {
    assert_eq!(KagemushaArtifactRoleV1::ALL.len(), 42);
    assert_eq!(
        KagemushaArtifactRoleV1::ALL
            .into_iter()
            .collect::<BTreeSet<_>>()
            .len(),
        KagemushaArtifactRoleV1::ALL.len()
    );
    assert_eq!(
        &KagemushaArtifactRoleV1::ALL[2..6],
        &[
            KagemushaArtifactRoleV1::InnerStatePkEq,
            KagemushaArtifactRoleV1::InnerStateVkEq,
            KagemushaArtifactRoleV1::InnerStatePkEp,
            KagemushaArtifactRoleV1::InnerStateVkEp,
        ]
    );
    assert_eq!(
        &KagemushaArtifactRoleV1::ALL[6..10],
        &[
            KagemushaArtifactRoleV1::StatePkEq,
            KagemushaArtifactRoleV1::StateVkEq,
            KagemushaArtifactRoleV1::StatePkEp,
            KagemushaArtifactRoleV1::StateVkEp,
        ]
    );
    assert_eq!(
        &KagemushaArtifactRoleV1::ALL[10..14],
        &[
            KagemushaArtifactRoleV1::MintAuthorizationPkEq,
            KagemushaArtifactRoleV1::MintAuthorizationVkEq,
            KagemushaArtifactRoleV1::MintAuthorizationPkEp,
            KagemushaArtifactRoleV1::MintAuthorizationVkEp,
        ]
    );
    assert_eq!(
        &KagemushaArtifactRoleV1::ALL[14..18],
        &[
            KagemushaArtifactRoleV1::MintCreditPkEq,
            KagemushaArtifactRoleV1::MintCreditVkEq,
            KagemushaArtifactRoleV1::MintCreditPkEp,
            KagemushaArtifactRoleV1::MintCreditVkEp,
        ]
    );
    assert_eq!(
        &KagemushaArtifactRoleV1::ALL[18..22],
        &[
            KagemushaArtifactRoleV1::PlatformCredentialPkEq,
            KagemushaArtifactRoleV1::PlatformCredentialVkEq,
            KagemushaArtifactRoleV1::PlatformCredentialPkEp,
            KagemushaArtifactRoleV1::PlatformCredentialVkEp,
        ]
    );
    assert_eq!(
        &KagemushaArtifactRoleV1::ALL[22..26],
        &[
            KagemushaArtifactRoleV1::GuardBundlePkEq,
            KagemushaArtifactRoleV1::GuardBundleVkEq,
            KagemushaArtifactRoleV1::GuardBundlePkEp,
            KagemushaArtifactRoleV1::GuardBundleVkEp,
        ]
    );
    assert_eq!(
        &KagemushaArtifactRoleV1::ALL[26..30],
        &[
            KagemushaArtifactRoleV1::TerminalAuthorizationPkEq,
            KagemushaArtifactRoleV1::TerminalAuthorizationVkEq,
            KagemushaArtifactRoleV1::TerminalAuthorizationPkEp,
            KagemushaArtifactRoleV1::TerminalAuthorizationVkEp,
        ]
    );
    assert_eq!(
        &KagemushaArtifactRoleV1::ALL[30..34],
        &[
            KagemushaArtifactRoleV1::CommitWrapperPkEq,
            KagemushaArtifactRoleV1::CommitWrapperVkEq,
            KagemushaArtifactRoleV1::CommitWrapperPkEp,
            KagemushaArtifactRoleV1::CommitWrapperVkEp,
        ]
    );
    assert_eq!(
        &KagemushaArtifactRoleV1::ALL[34..38],
        &[
            KagemushaArtifactRoleV1::InnerMintAuthorizationPkEq,
            KagemushaArtifactRoleV1::InnerMintAuthorizationVkEq,
            KagemushaArtifactRoleV1::InnerMintAuthorizationPkEp,
            KagemushaArtifactRoleV1::InnerMintAuthorizationVkEp,
        ]
    );
    assert_eq!(
        &KagemushaArtifactRoleV1::ALL[38..42],
        &[
            KagemushaArtifactRoleV1::InnerMintCreditPkEq,
            KagemushaArtifactRoleV1::InnerMintCreditVkEq,
            KagemushaArtifactRoleV1::InnerMintCreditPkEp,
            KagemushaArtifactRoleV1::InnerMintCreditVkEp,
        ]
    );
    assert_eq!(
        KagemushaQualifiedHelperCircuitV1::ALL,
        [
            KagemushaQualifiedHelperCircuitV1::MintAuthorization,
            KagemushaQualifiedHelperCircuitV1::MintCredit,
            KagemushaQualifiedHelperCircuitV1::PlatformCredential,
            KagemushaQualifiedHelperCircuitV1::GuardBundle,
        ]
    );
    assert_eq!(
        KagemushaQualifiedRelationV1::ALL,
        [
            KagemushaQualifiedRelationV1::Bootstrap,
            KagemushaQualifiedRelationV1::MintFold,
            KagemushaQualifiedRelationV1::SendSplit,
            KagemushaQualifiedRelationV1::ReceiveFoldBatch,
            KagemushaQualifiedRelationV1::RedeemSplit,
            KagemushaQualifiedRelationV1::SuiteUpgrade,
            KagemushaQualifiedRelationV1::Rotate,
            KagemushaQualifiedRelationV1::TerminalAuthorization,
            KagemushaQualifiedRelationV1::CommitWrapper,
        ]
    );
    assert!(KagemushaQualifiedHelperCircuitV1::GuardBundle.uses_internal_proof_evidence());
    assert!(KagemushaQualifiedHelperCircuitV1::PlatformCredential.uses_internal_proof_evidence());
    assert!(!KagemushaQualifiedHelperCircuitV1::MintAuthorization.uses_internal_proof_evidence());

    let artifacts = artifacts();
    let mut missing_mint_authorization = artifacts.clone();
    missing_mint_authorization
        .retain(|artifact| artifact.role != KagemushaArtifactRoleV1::MintAuthorizationPkEq);
    assert_eq!(
        kagemusha_artifact_set_digest_v1(&missing_mint_authorization),
        Err(KagemushaReleaseErrorV1::InvalidArtifactSet)
    );
    let base = receipt(&artifacts);
    assert!(base.profile_qualifications.iter().all(|qualification| {
        qualification.relations.iter().all(|relation| {
            let expected = match relation.relation {
                KagemushaQualifiedRelationV1::TerminalAuthorization => (
                    TERMINAL_AUTHORIZATION_EQ_PROTOCOL_DIGEST,
                    TERMINAL_AUTHORIZATION_EP_PROTOCOL_DIGEST,
                    KagemushaArtifactRoleV1::TerminalAuthorizationVkEq,
                    KagemushaArtifactRoleV1::TerminalAuthorizationVkEp,
                ),
                KagemushaQualifiedRelationV1::CommitWrapper => (
                    COMMIT_WRAPPER_EQ_PROTOCOL_DIGEST,
                    COMMIT_WRAPPER_EP_PROTOCOL_DIGEST,
                    KagemushaArtifactRoleV1::CommitWrapperVkEq,
                    KagemushaArtifactRoleV1::CommitWrapperVkEp,
                ),
                KagemushaQualifiedRelationV1::Bootstrap
                | KagemushaQualifiedRelationV1::MintFold
                | KagemushaQualifiedRelationV1::SendSplit
                | KagemushaQualifiedRelationV1::ReceiveFoldBatch
                | KagemushaQualifiedRelationV1::RedeemSplit
                | KagemushaQualifiedRelationV1::SuiteUpgrade
                | KagemushaQualifiedRelationV1::Rotate => (
                    STATE_EQ_PROTOCOL_DIGEST,
                    STATE_EP_PROTOCOL_DIGEST,
                    KagemushaArtifactRoleV1::StateVkEq,
                    KagemushaArtifactRoleV1::StateVkEp,
                ),
            };
            (relation.eq_protocol_digest, relation.ep_protocol_digest) == (expected.0, expected.1)
                && relation.relation.expected_vk_roles() == (expected.2, expected.3)
        })
    }));

    let mut aliased_relation_keys = artifacts;
    let terminal_authorization_eq = artifact(
        &aliased_relation_keys,
        KagemushaArtifactRoleV1::TerminalAuthorizationVkEq,
    )
    .sha256;
    aliased_relation_keys
        .iter_mut()
        .find(|artifact| artifact.role == KagemushaArtifactRoleV1::CommitWrapperVkEq)
        .expect("post-commit wrapper Eq verifier key")
        .sha256 = terminal_authorization_eq;
    assert_eq!(
        kagemusha_artifact_set_digest_v1(&aliased_relation_keys),
        Err(KagemushaReleaseErrorV1::InvalidArtifactSet)
    );
}

#[test]
fn helper_circuits_are_complete_measured_and_artifact_bound() {
    let artifacts = artifacts();
    let base = receipt(&artifacts);

    let mut missing = base.clone();
    missing.profile_qualifications[0].helper_circuits.pop();
    reseal_profile_qualification(&mut missing, 0);
    assert!(missing.validate().is_err());

    let mut wrong_role = base.clone();
    wrong_role.profile_qualifications[0].helper_circuits[0].eq_verifying_key =
        artifact(&artifacts, KagemushaArtifactRoleV1::StateVkEq);
    reseal_profile_qualification(&mut wrong_role, 0);
    assert!(wrong_role.validate().is_err());

    let mut wrong_protocol = base.clone();
    wrong_protocol.profile_qualifications[0].helper_circuits[1].eq_protocol_digest =
        GUARD_EQ_PROTOCOL_DIGEST;
    reseal_profile_qualification(&mut wrong_protocol, 0);
    assert!(wrong_protocol.validate().is_err());

    let mut exact_wire_limit = base.clone();
    let wire_helper = &mut exact_wire_limit.profile_qualifications[0].helper_circuits[0];
    wire_helper.eq_circuit_rows = 1_u32 << KAGEMUSHA_HALO2_K_V1;
    wire_helper.complete_proof_bytes =
        u32::try_from(KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1).expect("proof cap fits u32");
    wire_helper.prove_p95_ms = KAGEMUSHA_PROVE_P95_MAX_MS_V1;
    wire_helper.verify_p95_ms = KAGEMUSHA_VERIFY_P95_MAX_MS_V1;
    wire_helper.process_rss_bytes = KAGEMUSHA_PROCESS_RSS_MAX_BYTES_V1;
    reseal_profile_qualification(&mut exact_wire_limit, 0);
    exact_wire_limit
        .validate()
        .expect("exact wire-helper limits pass");
    exact_wire_limit.profile_qualifications[0].helper_circuits[0].complete_proof_bytes += 1;
    reseal_profile_qualification(&mut exact_wire_limit, 0);
    assert!(exact_wire_limit.validate().is_err());

    let credential = &base.profile_qualifications[0].helper_circuits[2];
    assert_eq!(credential.eq_proof_bytes, CREDENTIAL_EQ_PROOF_BYTES);
    assert_eq!(credential.ep_proof_bytes, CREDENTIAL_EP_PROOF_BYTES);
    assert_eq!(
        credential.complete_proof_bytes,
        CREDENTIAL_EQ_PROOF_BYTES + CREDENTIAL_EP_PROOF_BYTES
    );
    assert!(
        usize::try_from(credential.complete_proof_bytes).expect("u32 fits usize")
            > KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1
    );

    let mut substituted_internal_length = base.clone();
    substituted_internal_length.profile_qualifications[0].helper_circuits[2].eq_proof_bytes += 32;
    substituted_internal_length.profile_qualifications[0].helper_circuits[2]
        .complete_proof_bytes += 32;
    reseal_profile_qualification(&mut substituted_internal_length, 0);
    assert!(substituted_internal_length.validate().is_err());

    let mut wrong_internal_sum = base.clone();
    wrong_internal_sum.profile_qualifications[0].helper_circuits[3].complete_proof_bytes -= 32;
    reseal_profile_qualification(&mut wrong_internal_sum, 0);
    assert!(wrong_internal_sum.validate().is_err());

    let mut substituted = base.clone();
    substituted.profile_qualifications[0].helper_circuits[0]
        .eq_verifying_key
        .sha256 = [0xE7; 32];
    reseal_profile_qualification(&mut substituted, 0);
    let substituted_manifest = manifest(artifacts, &substituted);
    assert_eq!(
        substituted_manifest
            .release_attestation_subject(&substituted, &authority_policy(&authority_keys(), 1)),
        Err(KagemushaReleaseErrorV1::InvalidManifest)
    );
}

#[test]
fn internal_helper_protocol_lengths_are_exact_authenticated_profiles() {
    let protocols = helper_protocols();
    kagemusha_release_profile_digest_v1(
        evidence(5),
        STATE_EQ_PROTOCOL_DIGEST,
        STATE_EP_PROTOCOL_DIGEST,
        &protocols,
    )
    .expect("well-formed internal proof profiles");

    let mut wire_claims_exact_length = protocols.clone();
    wire_claims_exact_length[0].eq_proof_bytes = 32;
    assert!(
        kagemusha_release_profile_digest_v1(
            evidence(5),
            STATE_EQ_PROTOCOL_DIGEST,
            STATE_EP_PROTOCOL_DIGEST,
            &wire_claims_exact_length,
        )
        .is_err()
    );

    let mut non_word_aligned = protocols.clone();
    non_word_aligned[2].eq_proof_bytes += 1;
    assert!(
        kagemusha_release_profile_digest_v1(
            evidence(5),
            STATE_EQ_PROTOCOL_DIGEST,
            STATE_EP_PROTOCOL_DIGEST,
            &non_word_aligned,
        )
        .is_err()
    );

    let mut zero_internal_length = protocols.clone();
    zero_internal_length[3].ep_proof_bytes = 0;
    assert!(
        kagemusha_release_profile_digest_v1(
            evidence(5),
            STATE_EQ_PROTOCOL_DIGEST,
            STATE_EP_PROTOCOL_DIGEST,
            &zero_internal_length,
        )
        .is_err()
    );

    let mut exact_resource_limit = protocols.clone();
    exact_resource_limit[2].eq_proof_bytes = KAGEMUSHA_INTERNAL_HELPER_PROOF_EVIDENCE_MAX_BYTES_V1;
    kagemusha_release_profile_digest_v1(
        evidence(5),
        STATE_EQ_PROTOCOL_DIGEST,
        STATE_EP_PROTOCOL_DIGEST,
        &exact_resource_limit,
    )
    .expect("exact internal proof evidence resource limit");

    let mut over_resource_limit = protocols.clone();
    over_resource_limit[2].eq_proof_bytes =
        KAGEMUSHA_INTERNAL_HELPER_PROOF_EVIDENCE_MAX_BYTES_V1 + 32;
    assert!(
        kagemusha_release_profile_digest_v1(
            evidence(5),
            STATE_EQ_PROTOCOL_DIGEST,
            STATE_EP_PROTOCOL_DIGEST,
            &over_resource_limit,
        )
        .is_err()
    );

    let mut overflowing_pair = protocols;
    overflowing_pair[2].eq_proof_bytes = u32::MAX - 31;
    overflowing_pair[2].ep_proof_bytes = 32;
    assert!(
        kagemusha_release_profile_digest_v1(
            evidence(5),
            STATE_EQ_PROTOCOL_DIGEST,
            STATE_EP_PROTOCOL_DIGEST,
            &overflowing_pair,
        )
        .is_err()
    );
}

#[test]
fn receive_fold_batch_occupancies_are_exact_and_bounded() {
    let artifacts = artifacts();
    let base = receipt(&artifacts);
    assert_eq!(
        base.profile_qualifications[0]
            .receive_fold_occupancies
            .iter()
            .map(|entry| entry.occupancy)
            .collect::<Vec<_>>(),
        (1..=KAGEMUSHA_RECEIVE_FOLD_BATCH_WIDTH_V1).collect::<Vec<_>>()
    );

    let mut missing = base.clone();
    missing.profile_qualifications[0]
        .receive_fold_occupancies
        .pop();
    reseal_profile_qualification(&mut missing, 0);
    assert!(missing.validate().is_err());

    let mut unordered = base.clone();
    unordered.profile_qualifications[0]
        .receive_fold_occupancies
        .swap(0, 1);
    reseal_profile_qualification(&mut unordered, 0);
    assert!(unordered.validate().is_err());

    let mut oversized = base;
    oversized.profile_qualifications[0].receive_fold_occupancies[15].complete_proof_bytes =
        u32::try_from(KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1).expect("proof cap fits u32") + 1;
    reseal_profile_qualification(&mut oversized, 0);
    assert!(oversized.validate().is_err());
}

#[test]
fn recursive_depths_are_exact() {
    let artifacts = artifacts();
    let base = receipt(&artifacts);

    let mut shallow = base.clone();
    shallow.profile_qualifications[0].recursive_depths[3].depth = 1_024;
    shallow.profile_qualifications[0].recursive_depths[3].verified_handoffs = 1_024;
    reseal_profile_qualification(&mut shallow, 0);
    assert!(shallow.validate().is_err());

    let mut unverifiable = base;
    unverifiable.profile_qualifications[0].recursive_depths[2].verified_handoffs -= 1;
    reseal_profile_qualification(&mut unverifiable, 0);
    assert!(unverifiable.validate().is_err());
}

#[test]
fn recursive_depth_wire_sizes_are_bounded_and_invariant() {
    let artifacts = artifacts();
    let base = receipt(&artifacts);

    let mut variant = base.clone();
    variant.profile_qualifications[0].recursive_depths[3].complete_proof_bytes -= 1;
    reseal_profile_qualification(&mut variant, 0);
    assert!(variant.validate().is_err());

    let mut exact = base.clone();
    for depth in &mut exact.profile_qualifications[0].recursive_depths {
        depth.complete_proof_bytes =
            u32::try_from(KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1).expect("proof cap fits u32");
        depth.raw_complete_exchange_bytes =
            u32::try_from(KAGEMUSHA_COMPLETE_EXCHANGE_MAX_BYTES_V1).expect("raw cap fits u32");
        depth.text_complete_exchange_bytes =
            u32::try_from(KAGEMUSHA_COMPLETE_TEXT_EXCHANGE_MAX_BYTES_V1)
                .expect("text cap fits u32");
    }
    exact.profile_qualifications[0]
        .envelope
        .raw_complete_exchange_bytes =
        u32::try_from(KAGEMUSHA_COMPLETE_EXCHANGE_MAX_BYTES_V1).expect("raw cap fits u32");
    exact.profile_qualifications[0]
        .envelope
        .text_complete_exchange_bytes =
        u32::try_from(KAGEMUSHA_COMPLETE_TEXT_EXCHANGE_MAX_BYTES_V1).expect("text cap fits u32");
    reseal_profile_qualification(&mut exact, 0);
    exact.validate().expect("exact invariant depth limits pass");

    for depth in &mut exact.profile_qualifications[0].recursive_depths {
        depth.complete_proof_bytes += 1;
    }
    reseal_profile_qualification(&mut exact, 0);
    assert!(exact.validate().is_err());
}

#[test]
fn quantitative_and_wire_gates_are_typed_per_profile() {
    let artifacts = artifacts();
    let base = receipt(&artifacts);

    let mut exact = base.clone();
    exact.profile_qualifications[0]
        .envelope
        .raw_complete_exchange_bytes =
        u32::try_from(KAGEMUSHA_COMPLETE_EXCHANGE_MAX_BYTES_V1).expect("raw cap fits u32");
    exact.profile_qualifications[0]
        .envelope
        .text_complete_exchange_bytes =
        u32::try_from(KAGEMUSHA_COMPLETE_TEXT_EXCHANGE_MAX_BYTES_V1).expect("text cap fits u32");
    let raw_complete_exchange_bytes = exact.profile_qualifications[0]
        .envelope
        .raw_complete_exchange_bytes;
    let text_complete_exchange_bytes = exact.profile_qualifications[0]
        .envelope
        .text_complete_exchange_bytes;
    for depth in &mut exact.profile_qualifications[0].recursive_depths {
        depth.raw_complete_exchange_bytes = raw_complete_exchange_bytes;
        depth.text_complete_exchange_bytes = text_complete_exchange_bytes;
    }
    reseal_profile_qualification(&mut exact, 0);
    exact.validate().expect("exact envelope limits pass");
    exact.profile_qualifications[0]
        .envelope
        .raw_complete_exchange_bytes += 1;
    reseal_profile_qualification(&mut exact, 0);
    assert!(exact.validate().is_err());

    let mut aggregate = base.clone();
    aggregate.profile_qualifications[0]
        .aggregate_balance
        .independent_payments -= 1;
    reseal_profile_qualification(&mut aggregate, 0);
    assert!(aggregate.validate().is_err());

    let mut not_one_spend = base.clone();
    not_one_spend.profile_qualifications[0]
        .aggregate_balance
        .spend_payments = 2;
    reseal_profile_qualification(&mut not_one_spend, 0);
    assert!(not_one_spend.validate().is_err());

    let mut thermal = base;
    thermal.profile_qualifications[0].thermal.folded_credits -= 1;
    reseal_profile_qualification(&mut thermal, 0);
    assert!(thermal.validate().is_err());
}

#[test]
fn acceptance_cases_and_reproducible_builds_are_closed() {
    let artifacts = artifacts();
    let base = receipt(&artifacts);
    let cases = KagemushaAcceptanceCaseV1::ALL
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    assert_eq!(cases.len(), KagemushaAcceptanceCaseV1::ALL.len());
    assert_eq!(
        &KagemushaAcceptanceCaseV1::ALL[2..17],
        &[
            KagemushaAcceptanceCaseV1::CrashDuringPrepare,
            KagemushaAcceptanceCaseV1::CrashAfterPrepareBeforeProof,
            KagemushaAcceptanceCaseV1::CrashDuringProof,
            KagemushaAcceptanceCaseV1::CrashAfterProofBeforeCandidatePersistence,
            KagemushaAcceptanceCaseV1::CrashDuringCandidatePersistence,
            KagemushaAcceptanceCaseV1::CrashAfterCandidatePersistenceBeforeVerification,
            KagemushaAcceptanceCaseV1::CrashDuringCandidateVerification,
            KagemushaAcceptanceCaseV1::CrashAfterCandidateVerificationBeforeHardwareCommit,
            KagemushaAcceptanceCaseV1::CrashDuringHardwareCommit,
            KagemushaAcceptanceCaseV1::CrashAfterHardwareCommitBeforeTerminalAuthorization,
            KagemushaAcceptanceCaseV1::CrashDuringTerminalAuthorization,
            KagemushaAcceptanceCaseV1::CrashAfterTerminalAuthorizationBeforeFinalEnvelopePersistence,
            KagemushaAcceptanceCaseV1::CrashDuringFinalEnvelopePersistence,
            KagemushaAcceptanceCaseV1::CrashAfterFinalEnvelopePersistenceBeforeExposure,
            KagemushaAcceptanceCaseV1::CrashDuringExposure,
        ]
    );
    for required in [
        KagemushaAcceptanceCaseV1::CrashDuringPrepare,
        KagemushaAcceptanceCaseV1::CrashAfterPrepareBeforeProof,
        KagemushaAcceptanceCaseV1::CrashDuringProof,
        KagemushaAcceptanceCaseV1::CrashDuringCandidatePersistence,
        KagemushaAcceptanceCaseV1::CrashDuringCandidateVerification,
        KagemushaAcceptanceCaseV1::CrashDuringHardwareCommit,
        KagemushaAcceptanceCaseV1::CrashDuringTerminalAuthorization,
        KagemushaAcceptanceCaseV1::CrashDuringFinalEnvelopePersistence,
        KagemushaAcceptanceCaseV1::CrashDuringExposure,
        KagemushaAcceptanceCaseV1::CrashDuringTransport,
        KagemushaAcceptanceCaseV1::CrashDuringInboxStage,
        KagemushaAcceptanceCaseV1::CrashDuringAckRecovery,
        KagemushaAcceptanceCaseV1::CrashDuringRecovery,
        KagemushaAcceptanceCaseV1::MissingSenderAuthorization,
        KagemushaAcceptanceCaseV1::ForgedSenderAuthorization,
        KagemushaAcceptanceCaseV1::ReplayedSenderAuthorization,
        KagemushaAcceptanceCaseV1::CrossReleaseSenderAuthorization,
        KagemushaAcceptanceCaseV1::MissingMintAuthorization,
        KagemushaAcceptanceCaseV1::ForgedMintAuthorization,
        KagemushaAcceptanceCaseV1::ReplayedMintAuthorization,
        KagemushaAcceptanceCaseV1::CrossReleaseMintAuthorization,
        KagemushaAcceptanceCaseV1::AcceptanceTicketSingleExact,
        KagemushaAcceptanceCaseV1::AcceptanceTicketPartialUntilTotal,
        KagemushaAcceptanceCaseV1::AcceptanceTicketBoundedMultiPayment,
        KagemushaAcceptanceCaseV1::AcceptanceTicketOpenReceive,
        KagemushaAcceptanceCaseV1::AcceptanceTicketReplay,
        KagemushaAcceptanceCaseV1::AcceptanceTicketMismatch,
        KagemushaAcceptanceCaseV1::DistinctPaymentsSameRequest,
        KagemushaAcceptanceCaseV1::ShuffledConcurrentPaymentsSameRequest,
        KagemushaAcceptanceCaseV1::DelayedDeliveryAfterRequestExpiry,
        KagemushaAcceptanceCaseV1::DelayedDeliveryAcrossOrdinarySuiteRotation,
        KagemushaAcceptanceCaseV1::DelayedDeliveryAcrossCredentialRotation,
        KagemushaAcceptanceCaseV1::DuplicateTransport,
        KagemushaAcceptanceCaseV1::SameCreditReplay,
        KagemushaAcceptanceCaseV1::TwoSuccessorsFromOnePredecessor,
        KagemushaAcceptanceCaseV1::MonotonicLeaseExpiry,
        KagemushaAcceptanceCaseV1::HardwareEpochRollover,
        KagemushaAcceptanceCaseV1::OrdinaryVerifierRotation,
        KagemushaAcceptanceCaseV1::HardwareCounterRollover,
        KagemushaAcceptanceCaseV1::TranscriptUnlinkability,
        KagemushaAcceptanceCaseV1::X25519LowOrderPublicKeyRejection,
        KagemushaAcceptanceCaseV1::X25519ZeroDhRejection,
        KagemushaAcceptanceCaseV1::AeadCiphertextSubstitution,
        KagemushaAcceptanceCaseV1::AeadAssociatedDataSubstitution,
        KagemushaAcceptanceCaseV1::DeterministicEncryptionInjectedRandomnessKat,
        KagemushaAcceptanceCaseV1::ReceiveFoldBatchOccupancyOneThroughSixteen,
        KagemushaAcceptanceCaseV1::ReceiveFoldBatchPadding,
        KagemushaAcceptanceCaseV1::ReceiveFoldBatchReplayAtomicity,
    ] {
        assert!(cases.contains(&required));
    }

    let mut missing_case = base.clone();
    missing_case.profile_qualifications[0]
        .acceptance_cases
        .pop();
    reseal_profile_qualification(&mut missing_case, 0);
    assert!(missing_case.validate().is_err());

    let mut unordered_case = base.clone();
    unordered_case.profile_qualifications[0]
        .acceptance_cases
        .swap(1, 2);
    reseal_profile_qualification(&mut unordered_case, 0);
    assert!(unordered_case.validate().is_err());

    let mut wrong_validator_count = base.clone();
    let four_peer = wrong_validator_count.profile_qualifications[0]
        .acceptance_cases
        .iter_mut()
        .find(|case| case.case == KagemushaAcceptanceCaseV1::FourPeerActivationRestartReplay)
        .expect("four-peer case");
    four_peer.validator_count = 3;
    reseal_profile_qualification(&mut wrong_validator_count, 0);
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
        Err(KagemushaReleaseErrorV1::InvalidManifest)
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
        .byte_len = KAGEMUSHA_RELEASE_EVIDENCE_FILE_MAX_BYTES_V1 + 1;
    reseal_profile_qualification(&mut receipt, 0);
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
        Err(KagemushaReleaseErrorV1::UnknownSigner)
    );

    let insufficient = release_attestation(&manifest, &receipt, &policy, &keys[..1]);
    assert_eq!(
        manifest.authenticate(&receipt, &policy, &insufficient),
        Err(KagemushaReleaseErrorV1::InsufficientThreshold {
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
        Err(KagemushaReleaseErrorV1::InvalidSignature)
    );
}

#[test]
fn exact_release_decoders_reject_outer_caps_and_forged_lengths() {
    assert_eq!(
        KagemushaInternalValidationReceiptV1::decode_canonical_exact(&vec![
            0;
            KAGEMUSHA_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1
                + 1
        ]),
        Err(KagemushaReleaseErrorV1::InvalidValidationReceipt)
    );
    assert_eq!(
        KagemushaReleaseManifestV1::decode_canonical_exact(&vec![
            0;
            KAGEMUSHA_RELEASE_MANIFEST_MAX_BYTES_V1
                + 1
        ]),
        Err(KagemushaReleaseErrorV1::InvalidManifest)
    );

    const PAYLOAD_LENGTH_OFFSET: usize = 4 + 1 + 1 + 16 + 1;
    const PAYLOAD_LENGTH_END: usize = PAYLOAD_LENGTH_OFFSET + 8;
    let artifacts = artifacts();
    let receipt = receipt(&artifacts);
    let manifest = manifest(artifacts, &receipt);
    let mut bytes = norito::encode_canonical(&manifest).expect("encode manifest");
    bytes[PAYLOAD_LENGTH_OFFSET..PAYLOAD_LENGTH_END].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(KagemushaReleaseManifestV1::decode_canonical_exact(&bytes).is_err());
}

#[test]
fn manifest_and_receipt_reject_semantic_profile_caps() {
    let artifacts = artifacts();
    let base = receipt(&artifacts);
    let mut receipt = base.clone();
    let vk_digest = base.profile_qualifications[0].profile.vk_digest;
    while receipt.profile_qualifications.len() <= KAGEMUSHA_RELEASE_MAX_ENABLED_PROFILES_V1 {
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
        Err(KagemushaReleaseErrorV1::InvalidValidationReceipt)
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
        Err(KagemushaReleaseErrorV1::InvalidManifest)
    );
}

#[test]
fn maximal_64_profile_manifest_fits_the_64_kib_admission_cap() {
    assert_eq!(KAGEMUSHA_RELEASE_MANIFEST_MAX_BYTES_V1, 64 * 1024);
    let artifacts = artifacts();
    let receipt = receipt_with_profile_count(&artifacts, KAGEMUSHA_RELEASE_MAX_ENABLED_PROFILES_V1);
    receipt.validate().expect("64-profile receipt");
    let receipt_bytes = norito::encode_canonical(&receipt).expect("encode 64-profile receipt");
    assert!(receipt_bytes.len() <= KAGEMUSHA_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1);

    let manifest = manifest(artifacts, &receipt);
    assert_eq!(
        manifest.enabled_profiles.len(),
        KAGEMUSHA_RELEASE_MAX_ENABLED_PROFILES_V1
    );
    let manifest_bytes = norito::encode_canonical(&manifest).expect("encode 64-profile manifest");
    assert!(
        manifest_bytes.len() > 16 * 1024,
        "complete embedded hardware profiles exceed the retired 16-KiB budget"
    );
    assert!(manifest_bytes.len() <= KAGEMUSHA_RELEASE_MANIFEST_MAX_BYTES_V1);
    assert_eq!(
        KagemushaReleaseManifestV1::decode_canonical_exact(&manifest_bytes)
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
        Err(KagemushaReleaseErrorV1::InvalidValidationReceipt)
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
        KAGEMUSHA_RELEASE_TRANSCRIPT_TOTAL_MAX_BYTES_V1 + 1;
    assert_eq!(
        excessive.validate(),
        Err(KagemushaReleaseErrorV1::InvalidValidationReceipt)
    );

    let mut substituted_shape = base;
    substituted_shape.circuit_shape_report.sha256 = [0xF6; 32];
    assert_eq!(
        substituted_shape.validate(),
        Err(KagemushaReleaseErrorV1::InvalidValidationReceipt)
    );
}

#[test]
fn release_digests_bind_inner_outer_helper_and_lifecycle_artifact_provenance() {
    let artifacts = artifacts();
    let artifact_set_digest =
        kagemusha_artifact_set_digest_v1(&artifacts).expect("artifact-set digest");
    assert_eq!(
        kagemusha_artifact_set_digest_v1(&artifacts).expect("repeat artifact-set digest"),
        artifact_set_digest
    );

    let helper_protocols = helper_protocols();
    let vk_set_digest = kagemusha_vk_set_digest_v1(
        &artifacts,
        STATE_EQ_PROTOCOL_DIGEST,
        STATE_EP_PROTOCOL_DIGEST,
        &helper_protocols,
    )
    .expect("VK-set digest");
    assert_eq!(
        kagemusha_vk_set_digest_v1(
            &artifacts,
            STATE_EQ_PROTOCOL_DIGEST,
            STATE_EP_PROTOCOL_DIGEST,
            &helper_protocols,
        )
        .expect("repeat VK-set digest"),
        vk_set_digest
    );

    for role in [
        KagemushaArtifactRoleV1::InnerStateVkEq,
        KagemushaArtifactRoleV1::StateVkEq,
        KagemushaArtifactRoleV1::GuardBundleVkEq,
        KagemushaArtifactRoleV1::MintAuthorizationVkEq,
        KagemushaArtifactRoleV1::TerminalAuthorizationVkEq,
        KagemushaArtifactRoleV1::CommitWrapperVkEq,
    ] {
        let mut changed = artifacts.clone();
        changed
            .iter_mut()
            .find(|artifact| artifact.role == role)
            .expect("verifier artifact")
            .sha256 = [0xFC; 32];
        assert_ne!(
            kagemusha_artifact_set_digest_v1(&changed).expect("changed artifact-set digest"),
            artifact_set_digest
        );
        assert_ne!(
            kagemusha_vk_set_digest_v1(
                &changed,
                STATE_EQ_PROTOCOL_DIGEST,
                STATE_EP_PROTOCOL_DIGEST,
                &helper_protocols,
            )
            .expect("changed VK-set digest"),
            vk_set_digest
        );
    }

    let profile_digest = kagemusha_release_profile_digest_v1(
        evidence(5),
        STATE_EQ_PROTOCOL_DIGEST,
        STATE_EP_PROTOCOL_DIGEST,
        &helper_protocols,
    )
    .expect("release profile digest");
    assert_ne!(
        kagemusha_release_profile_digest_v1(
            evidence(6),
            STATE_EQ_PROTOCOL_DIGEST,
            STATE_EP_PROTOCOL_DIGEST,
            &helper_protocols,
        )
        .expect("changed release profile digest"),
        profile_digest
    );
}
