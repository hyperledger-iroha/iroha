use super::*;
use iroha_crypto::{Algorithm, KeyPair};

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

fn receipt(artifacts: &[OfflineCashArtifactBindingV1]) -> OfflineCashInternalValidationReceiptV1 {
    OfflineCashInternalValidationReceiptV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        source_tree_digest: [1; 32],
        cargo_lock_digest: [2; 32],
        profile_digest: [3; 32],
        eq_protocol_digest: [0x31; 32],
        ep_protocol_digest: [0x32; 32],
        artifact_set_digest: offline_cash_artifact_set_digest_v1(artifacts)
            .expect("artifact digest"),
        hardware_policy_digest: [4; 32],
        circuit_shape_report_digest: [5; 32],
        security_review_digest: [6; 32],
        kat_report_digest: [7; 32],
        fuzz_report_digest: [8; 32],
        resource_report_digest: [9; 32],
        ios_device_report_digest: [10; 32],
        android_device_report_digest: [11; 32],
        four_peer_report_digest: [12; 32],
        max_proof_pair_bytes: 6_000,
        max_session_bytes: 8_900,
        max_process_rss_bytes: 120 * 1024 * 1024,
        prove_p95_ms: 9_000,
        verify_p95_ms: 900,
        handoff_p95_ms: 29_000,
        qualified_handoffs: OFFLINE_CASH_MIN_QUALIFIED_HANDOFFS_V1,
        fuzz_cases: OFFLINE_CASH_MIN_FUZZ_CASES_V1,
        reproducible_builds: OFFLINE_CASH_REPRODUCIBLE_BUILD_COUNT_V1,
        validator_count: OFFLINE_CASH_VALIDATOR_COUNT_V1,
    }
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
        hardware_policy_digest: receipt.hardware_policy_digest,
        validation_receipt_digest: receipt.canonical_digest().expect("receipt digest"),
        halo2_k: OFFLINE_CASH_HALO2_K_V1,
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
fn authenticates_complete_evidence_bound_release() {
    let artifacts = artifacts();
    let receipt = receipt(&artifacts);
    let manifest = manifest(artifacts, &receipt);
    let receipt_bytes = norito::encode_canonical(&receipt).expect("encode receipt");
    let decoded_receipt: OfflineCashInternalValidationReceiptV1 =
        norito::decode_from_bytes(&receipt_bytes).expect("decode receipt");
    assert_eq!(decoded_receipt, receipt);

    let manifest_bytes = norito::encode_canonical(&manifest).expect("encode manifest");
    let decoded_manifest = OfflineCashReleaseManifestV1::decode_canonical_exact(&manifest_bytes)
        .expect("decode manifest");
    assert_eq!(decoded_manifest, manifest);

    let keys = authority_keys();
    let policy = authority_policy(&keys, 2);
    let attestation = release_attestation(&manifest, &receipt, &policy, &keys[..2]);
    let policy_bytes = norito::encode_canonical(&policy).expect("encode authority policy");
    let decoded_policy = OfflineCashReleaseAuthorityPolicyV1::decode_canonical_exact(&policy_bytes)
        .expect("decode authority policy");
    assert_eq!(decoded_policy, policy);
    let attestation_bytes =
        norito::encode_canonical(&attestation).expect("encode release attestation");
    let decoded_attestation =
        OfflineCashReleaseAttestationV1::decode_canonical_exact(&attestation_bytes)
            .expect("decode release attestation");
    assert_eq!(decoded_attestation, attestation);

    let authenticated = decoded_manifest
        .authenticate(&decoded_receipt, &decoded_policy, &decoded_attestation)
        .expect("authenticate");
    assert_eq!(authenticated.release_id(), decoded_manifest.release_id);
    assert_eq!(authenticated.approved_signers().len(), 2);
    assert_eq!(
        authenticated.authority_policy_digest(),
        decoded_policy.canonical_digest().expect("policy digest")
    );
    assert_eq!(
        decoded_attestation.subject.manifest_digest,
        authenticated.manifest_digest()
    );
    assert_eq!(
        decoded_attestation.subject.validation_receipt_digest,
        authenticated.receipt_digest()
    );
    assert_eq!(
        decoded_attestation.subject.artifact_set_digest,
        decoded_receipt.artifact_set_digest
    );
    assert_eq!(
        authenticated
            .artifact(OfflineCashArtifactRoleV1::StateVkEp)
            .role,
        OfflineCashArtifactRoleV1::StateVkEp
    );
}

#[test]
fn rejects_receipt_or_artifact_substitution() {
    let artifacts = artifacts();
    let receipt = receipt(&artifacts);
    let manifest = manifest(artifacts.clone(), &receipt);
    let keys = authority_keys();
    let policy = authority_policy(&keys, 2);
    let attestation = release_attestation(&manifest, &receipt, &policy, &keys[..2]);
    let mut wrong_receipt = receipt;
    wrong_receipt.kat_report_digest = [0xAA; 32];
    assert!(
        manifest
            .authenticate(&wrong_receipt, &policy, &attestation)
            .is_err()
    );

    let mut wrong_artifacts = artifacts;
    wrong_artifacts[2].byte_len = OFFLINE_CASH_STATE_PROVING_KEY_MAX_BYTES_V1 + 1;
    assert!(offline_cash_artifact_set_digest_v1(&wrong_artifacts).is_err());
}

#[test]
fn rejects_underqualified_mobile_or_reproducibility_evidence() {
    let artifacts = artifacts();
    let mut receipt = receipt(&artifacts);
    receipt.max_process_rss_bytes = OFFLINE_CASH_PROCESS_RSS_MAX_BYTES_V1 + 1;
    assert_eq!(
        receipt.validate(),
        Err(OfflineCashReleaseErrorV1::InvalidValidationReceipt)
    );
    receipt = self::receipt(&artifacts);
    receipt.reproducible_builds = 1;
    assert!(receipt.validate().is_err());
}

#[test]
fn rejects_unknown_or_invalid_authority_signatures() {
    let artifacts = artifacts();
    let receipt = receipt(&artifacts);
    let manifest = manifest(artifacts, &receipt);
    let keys = authority_keys();
    let policy = authority_policy(&keys, 1);
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

    let mut invalid_signature = release_attestation(&manifest, &receipt, &policy, &keys[..1]);
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
fn rejects_duplicate_unordered_or_insufficient_approvals() {
    let artifacts = artifacts();
    let receipt = receipt(&artifacts);
    let manifest = manifest(artifacts, &receipt);
    let keys = authority_keys();
    let policy = authority_policy(&keys, 2);

    let mut duplicate_policy = policy.clone();
    duplicate_policy.authorized_signers[1] = duplicate_policy.authorized_signers[0].clone();
    assert_eq!(
        duplicate_policy.validate(),
        Err(OfflineCashReleaseErrorV1::InvalidAuthorityPolicy)
    );
    let mut unordered_policy = policy.clone();
    unordered_policy.authorized_signers.reverse();
    assert_eq!(
        unordered_policy.validate(),
        Err(OfflineCashReleaseErrorV1::InvalidAuthorityPolicy)
    );

    let insufficient = release_attestation(&manifest, &receipt, &policy, &keys[..1]);
    assert_eq!(
        manifest.authenticate(&receipt, &policy, &insufficient),
        Err(OfflineCashReleaseErrorV1::InsufficientThreshold {
            collected: 1,
            required: 2,
        })
    );

    let mut duplicate = release_attestation(&manifest, &receipt, &policy, &keys[..2]);
    duplicate
        .approvals
        .insert(1, duplicate.approvals[0].clone());
    assert_eq!(
        manifest.authenticate(&receipt, &policy, &duplicate),
        Err(OfflineCashReleaseErrorV1::DuplicateOrUnorderedSigner)
    );

    let mut unordered = release_attestation(&manifest, &receipt, &policy, &keys[..2]);
    unordered.approvals.reverse();
    assert_eq!(
        manifest.authenticate(&receipt, &policy, &unordered),
        Err(OfflineCashReleaseErrorV1::DuplicateOrUnorderedSigner)
    );
}

#[test]
fn rejects_signed_subject_substitution() {
    let artifacts = artifacts();
    let receipt = receipt(&artifacts);
    let manifest = manifest(artifacts, &receipt);
    let keys = authority_keys();
    let policy = authority_policy(&keys, 1);
    let expected = manifest
        .release_attestation_subject(&receipt, &policy)
        .expect("expected release subject");
    let substituted_subjects = [
        OfflineCashReleaseAttestationSubjectV1 {
            authority_policy_digest: [0xF1; 32],
            ..expected
        },
        OfflineCashReleaseAttestationSubjectV1 {
            release_id: [0xF2; 32],
            ..expected
        },
        OfflineCashReleaseAttestationSubjectV1 {
            manifest_digest: [0xF3; 32],
            ..expected
        },
        OfflineCashReleaseAttestationSubjectV1 {
            validation_receipt_digest: [0xF4; 32],
            ..expected
        },
        OfflineCashReleaseAttestationSubjectV1 {
            artifact_set_digest: [0xF5; 32],
            ..expected
        },
    ];
    for subject in substituted_subjects {
        let substituted = OfflineCashReleaseAttestationV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            subject,
            approvals: vec![OfflineCashReleaseApprovalV1 {
                public_key: keys[0].public_key().clone(),
                signature: SignatureOf::try_new(keys[0].private_key(), &subject.approval_payload())
                    .expect("signature over substituted subject"),
            }],
        };
        assert_eq!(
            manifest.authenticate(&receipt, &policy, &substituted),
            Err(OfflineCashReleaseErrorV1::InvalidAttestation)
        );
    }
}

#[test]
fn exact_release_decoders_reject_outer_cap_before_parsing() {
    assert_eq!(
        OfflineCashReleaseManifestV1::decode_canonical_exact(&vec![
            0;
            OFFLINE_CASH_RELEASE_MANIFEST_MAX_BYTES_V1
                + 1
        ]),
        Err(OfflineCashReleaseErrorV1::InvalidManifest)
    );
    assert_eq!(
        OfflineCashReleaseAuthorityPolicyV1::decode_canonical_exact(&vec![
            0;
            OFFLINE_CASH_RELEASE_AUTHORITY_POLICY_MAX_BYTES_V1
                + 1
        ]),
        Err(OfflineCashReleaseErrorV1::InvalidAuthorityPolicy)
    );
    assert_eq!(
        OfflineCashReleaseAttestationV1::decode_canonical_exact(&vec![
            0;
            OFFLINE_CASH_RELEASE_ATTESTATION_MAX_BYTES_V1
                + 1
        ]),
        Err(OfflineCashReleaseErrorV1::InvalidAttestation)
    );
}

#[test]
fn exact_release_decoders_reject_forged_declared_lengths() {
    const NORITO_PAYLOAD_LENGTH_OFFSET: usize = 4 + 1 + 1 + 16 + 1;
    const NORITO_PAYLOAD_LENGTH_END: usize = NORITO_PAYLOAD_LENGTH_OFFSET + 8;

    let artifacts = artifacts();
    let receipt = receipt(&artifacts);
    let manifest = manifest(artifacts, &receipt);
    let keys = authority_keys();
    let policy = authority_policy(&keys, 2);
    let attestation = release_attestation(&manifest, &receipt, &policy, &keys[..2]);
    let mut manifest_bytes = norito::encode_canonical(&manifest).expect("encode manifest");
    let mut policy_bytes = norito::encode_canonical(&policy).expect("encode policy");
    let mut attestation_bytes = norito::encode_canonical(&attestation).expect("encode attestation");
    for bytes in [
        &mut manifest_bytes,
        &mut policy_bytes,
        &mut attestation_bytes,
    ] {
        bytes[NORITO_PAYLOAD_LENGTH_OFFSET..NORITO_PAYLOAD_LENGTH_END]
            .copy_from_slice(&u64::MAX.to_le_bytes());
    }

    assert!(OfflineCashReleaseManifestV1::decode_canonical_exact(&manifest_bytes).is_err());
    assert!(OfflineCashReleaseAuthorityPolicyV1::decode_canonical_exact(&policy_bytes).is_err());
    assert!(OfflineCashReleaseAttestationV1::decode_canonical_exact(&attestation_bytes).is_err());
}

#[test]
fn exact_release_decoders_enforce_semantic_collection_caps() {
    let base_artifacts = artifacts();
    let receipt = receipt(&base_artifacts);
    let mut oversized_manifest = manifest(base_artifacts, &receipt);
    oversized_manifest
        .artifacts
        .push(oversized_manifest.artifacts[0]);
    let manifest_bytes =
        norito::encode_canonical(&oversized_manifest).expect("encode oversized manifest");
    assert_eq!(
        OfflineCashReleaseManifestV1::decode_canonical_exact(&manifest_bytes),
        Err(OfflineCashReleaseErrorV1::InvalidArtifactSet)
    );

    let mut keys = (0_u8..=OFFLINE_CASH_RELEASE_AUTHORITY_MAX_SIGNERS_V1 as u8)
        .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519))
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    let oversized_policy = authority_policy(&keys, 1);
    let policy_bytes =
        norito::encode_canonical(&oversized_policy).expect("encode oversized policy");
    assert_eq!(
        OfflineCashReleaseAuthorityPolicyV1::decode_canonical_exact(&policy_bytes),
        Err(OfflineCashReleaseErrorV1::InvalidAuthorityPolicy)
    );

    let valid_keys = authority_keys();
    let valid_policy = authority_policy(&valid_keys, 1);
    let valid_artifacts = artifacts();
    let valid_receipt = receipt(&valid_artifacts);
    let valid_manifest = manifest(valid_artifacts, &valid_receipt);
    let mut oversized_attestation = release_attestation(
        &valid_manifest,
        &valid_receipt,
        &valid_policy,
        &valid_keys[..1],
    );
    oversized_attestation.approvals = vec![
        oversized_attestation.approvals[0].clone();
        OFFLINE_CASH_RELEASE_AUTHORITY_MAX_SIGNERS_V1 + 1
    ];
    let attestation_bytes =
        norito::encode_canonical(&oversized_attestation).expect("encode oversized attestation");
    assert_eq!(
        OfflineCashReleaseAttestationV1::decode_canonical_exact(&attestation_bytes),
        Err(OfflineCashReleaseErrorV1::InvalidAttestation)
    );
}
