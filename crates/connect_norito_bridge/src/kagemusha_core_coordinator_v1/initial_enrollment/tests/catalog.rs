//! Signed synthetic catalog fixture adapted from the canonical API enrollment tests.
//! Every opaque release passes the real threshold verifier with explicit test keys.
//! These measurements are protocol fixtures, never evidence of physical qualification.
use super::*;

pub(super) fn authenticated_release() -> Arc<KagemushaAuthenticatedReleaseV1> {
    let artifacts = artifacts();
    let receipt = receipt(&artifacts);
    let manifest = manifest(artifacts, &receipt);
    let keys = authority_keys();
    let policy = authority_policy(&keys, 2);
    let attestation = release_attestation(&manifest, &receipt, &policy, &keys[..2]);
    let authenticated = manifest
        .authenticate(&receipt, &policy, &attestation)
        .unwrap();
    assert_eq!(authenticated.approved_signers().len(), 2);
    Arc::new(authenticated)
}

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
const MINT_HASH_SHARD_EQ_PROTOCOL_DIGEST: [u8; 32] = [0x41; 32];
const MINT_HASH_SHARD_EP_PROTOCOL_DIGEST: [u8; 32] = [0x42; 32];
const MINT_HASH_CLAIM_EQ_PROTOCOL_DIGEST: [u8; 32] = [0x43; 32];
const MINT_HASH_CLAIM_EP_PROTOCOL_DIGEST: [u8; 32] = [0x44; 32];
const CREDENTIAL_EQ_PROOF_BYTES: u32 = 8_000;
const CREDENTIAL_EP_PROOF_BYTES: u32 = 8_032;
const GUARD_EQ_PROOF_BYTES: u32 = 12_000;
const GUARD_EP_PROOF_BYTES: u32 = 12_032;
const MINT_HASH_SHARD_EQ_PROOF_BYTES: u32 = 4_000;
const MINT_HASH_SHARD_EP_PROOF_BYTES: u32 = 4_032;
const MINT_HASH_CLAIM_EQ_PROOF_BYTES: u32 = 6_016;
const MINT_HASH_CLAIM_EP_PROOF_BYTES: u32 = 6_048;

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
        KagemushaHelperProtocolV1 {
            helper: KagemushaQualifiedHelperCircuitV1::MintHashShard,
            eq_protocol_digest: MINT_HASH_SHARD_EQ_PROTOCOL_DIGEST,
            ep_protocol_digest: MINT_HASH_SHARD_EP_PROTOCOL_DIGEST,
            eq_proof_bytes: MINT_HASH_SHARD_EQ_PROOF_BYTES,
            ep_proof_bytes: MINT_HASH_SHARD_EP_PROOF_BYTES,
        },
        KagemushaHelperProtocolV1 {
            helper: KagemushaQualifiedHelperCircuitV1::MintHashClaim,
            eq_protocol_digest: MINT_HASH_CLAIM_EQ_PROTOCOL_DIGEST,
            ep_protocol_digest: MINT_HASH_CLAIM_EP_PROTOCOL_DIGEST,
            eq_proof_bytes: MINT_HASH_CLAIM_EQ_PROOF_BYTES,
            ep_proof_bytes: MINT_HASH_CLAIM_EP_PROOF_BYTES,
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
            byte_len: if matches!(
                role,
                KagemushaArtifactRoleV1::ParamsEq | KagemushaArtifactRoleV1::ParamsEp
            ) {
                KAGEMUSHA_PARAMS_BYTES_V1
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
        expires_at_ms: u64::MAX,
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
            let (eq_role, ep_role) = match relation {
                KagemushaQualifiedRelationV1::TerminalAuthorization => (
                    KagemushaArtifactRoleV1::TerminalAuthorizationVkEq,
                    KagemushaArtifactRoleV1::TerminalAuthorizationVkEp,
                ),
                KagemushaQualifiedRelationV1::CommitWrapper => (
                    KagemushaArtifactRoleV1::CommitWrapperVkEq,
                    KagemushaArtifactRoleV1::CommitWrapperVkEp,
                ),
                _ => (
                    KagemushaArtifactRoleV1::StateVkEq,
                    KagemushaArtifactRoleV1::StateVkEp,
                ),
            };
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
                | KagemushaQualifiedRelationV1::ReceiveFold
                | KagemushaQualifiedRelationV1::RedeemSplit
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
            let (eq_role, ep_role) = match protocol.helper {
                KagemushaQualifiedHelperCircuitV1::MintAuthorization => (
                    KagemushaArtifactRoleV1::MintAuthorizationVkEq,
                    KagemushaArtifactRoleV1::MintAuthorizationVkEp,
                ),
                KagemushaQualifiedHelperCircuitV1::MintCredit => (
                    KagemushaArtifactRoleV1::MintCreditVkEq,
                    KagemushaArtifactRoleV1::MintCreditVkEp,
                ),
                KagemushaQualifiedHelperCircuitV1::PlatformCredential => (
                    KagemushaArtifactRoleV1::PlatformCredentialVkEq,
                    KagemushaArtifactRoleV1::PlatformCredentialVkEp,
                ),
                KagemushaQualifiedHelperCircuitV1::GuardBundle => (
                    KagemushaArtifactRoleV1::GuardBundleVkEq,
                    KagemushaArtifactRoleV1::GuardBundleVkEp,
                ),
                KagemushaQualifiedHelperCircuitV1::MintHashShard => (
                    KagemushaArtifactRoleV1::MintHashShardVkEq,
                    KagemushaArtifactRoleV1::MintHashShardVkEp,
                ),
                KagemushaQualifiedHelperCircuitV1::MintHashClaim => (
                    KagemushaArtifactRoleV1::MintHashClaimVkEq,
                    KagemushaArtifactRoleV1::MintHashClaimVkEp,
                ),
            };
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
                        | KagemushaQualifiedHelperCircuitV1::MintHashShard
                        | KagemushaQualifiedHelperCircuitV1::MintHashClaim
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
    let provider_policy = provider_policy(&enabled_profiles);
    let provider_policy_root =
        kagemusha_provider_policy_root_v1(&enabled_profiles, &provider_policy).unwrap();
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
        native_profile_digest: [0xB1; 32],
        eq_protocol_digest: STATE_EQ_PROTOCOL_DIGEST,
        ep_protocol_digest: STATE_EP_PROTOCOL_DIGEST,
        artifact_set_digest,
        hardware_policy_digest,
        provider_policy_root,
        provider_policy,
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

fn provider_policy(profiles: &[KagemushaEnabledProfileV1]) -> Vec<KagemushaProviderPolicyEntryV1> {
    profiles
        .iter()
        .enumerate()
        .map(|(index, profile)| {
            let seed = profile.hardware_profile.provider_id[0].wrapping_add(5);
            let key = SigningKey::from_bytes((&[seed; 32]).into()).unwrap();
            authorized_provider_entry(profile, u16::try_from(index).unwrap(), [0xD1; 32], &key)
        })
        .collect()
}

fn authorized_provider_entry(
    profile: &KagemushaEnabledProfileV1,
    position: u16,
    commitment: [u8; 32],
    key: &SigningKey,
) -> KagemushaProviderPolicyEntryV1 {
    let message = kagemusha_provider_policy_signing_bytes_v1(
        profile.hardware_profile_id,
        position,
        commitment,
    )
    .unwrap();
    let signature: p256::ecdsa::Signature = key.sign(&message);
    let signature = signature.normalize_s().unwrap_or(signature);
    KagemushaProviderPolicyEntryV1 {
        hardware_profile_id: profile.hardware_profile_id,
        provider_authority_commitment: commitment,
        provider_profile_index: position,
        issuer_signature: KagemushaDeviceSignatureV1::from_raw_bytes(&signature.to_bytes())
            .unwrap(),
    }
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
