#[test]
fn compact_recursive_state_boundary_has_a_distinct_v5_protocol() {
    assert_eq!(
        (
            KAGEMUSHA_RECURSIVE_SPEND_STATE_BOUNDARY_VERSION_V5,
            KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V5,
            KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V5,
            KAGEMUSHA_RECURSIVE_SPEND_STATE_BOUNDARY_DOMAIN_V5,
        ),
        (
            5,
            5,
            138,
            b"iroha:kagemusha:recursive-state-boundary:v5".as_slice()
        )
    );
}

#[test]
fn v4_promotion_record_is_distinct_and_fail_closed() {
    let record = promoted_release();
    record.validate().expect("valid V4 promotion record");
    macro_rules! rejects {
        ($($mutation:expr),+ $(,)?) => {$({
            let mut tampered = record.clone();
            let mutation: fn(&mut KagemushaRecursiveSpendPromotedReleaseV4) = $mutation;
            mutation(&mut tampered);
            assert_eq!(
                tampered.validate(),
                Err(KagemushaReleaseVerificationError::InvalidPromotionRecord)
            );
        })+};
    }
    rejects!(
        |value: &mut KagemushaRecursiveSpendPromotedReleaseV4| value.schema =
            "retired-promoted-release".to_owned(),
        |value| value.version = KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V1,
        |value| value.generation = "not portable/".to_owned(),
        |value| value.authenticated_source_seal_projection_sha256 = [0; 32],
        |value| value.reviewed_cargo_binary_sha256 = [0; 32],
        |value| value.reviewed_rustc_binary_sha256 = [0; 32],
        |value| value.manifest_sha256 = [0; 32],
        |value| value.release_policy_sha256 = value.release_attestation_sha256,
        |value| value.approved_signers.swap(0, 1),
        |value| {
            value.approved_signers.pop();
        },
        |value| {
            let duplicate_signer = value.approved_signers[0].clone();
            value.approved_signers.insert(1, duplicate_signer);
        },
        |value| value.artifact_inventory_verified = false,
        |value| value.bridge_abi_version = KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4 - 1,
        |value| value.artifact_roles.swap(0, 1),
        |value| value.max_proof_bytes = 0,
        |value| value.max_proof_bytes =
            KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4 + 1,
    );
}

#[test]
fn v4_release_record_binds_promotion_build_provenance_to_manifest() {
    let mut record = release_activation_wire_fixture().release_record;
    let benchmark = b"wire-bound benchmark evidence";
    let reviewer = KeyPair::from_seed(vec![34; 32], Algorithm::Ed25519);
    let candidate = unsigned_candidate(&record.manifest);
    record.cryptographic_review_summary = signed_review_bytes(&candidate, &[&reviewer]);
    record.manifest.benchmark_evidence_sha256 = digest(benchmark);
    record.manifest.cryptographic_review_sha256 = digest(&record.cryptographic_review_summary);
    record.release_attestation.subject = record
        .manifest
        .release_attestation_subject()
        .expect("release-record fixture has a canonical attestation subject");
    record.manifest.release_attestation_sha256 = digest(
        &norito::encode_canonical(&record.release_attestation)
            .expect("canonical release-record attestation"),
    );
    record.promotion_record.manifest_sha256 = record
        .manifest
        .canonical_sha256()
        .expect("canonical release-record manifest");
    record.promotion_record.release_attestation_sha256 = record.manifest.release_attestation_sha256;
    record
        .validate_structure()
        .expect("structurally valid release record before provenance mutation");
    let mutations: [fn(&mut KagemushaRecursiveSpendPromotedReleaseV4); 3] = [
        |value: &mut KagemushaRecursiveSpendPromotedReleaseV4| {
            value.authenticated_source_seal_projection_sha256[0] ^= 1;
        },
        |value| value.reviewed_cargo_binary_sha256[0] ^= 1,
        |value| value.reviewed_rustc_binary_sha256[0] ^= 1,
    ];
    for mutation in mutations {
        let mut tampered = record.clone();
        mutation(&mut tampered.promotion_record);
        assert_eq!(
            tampered.validate_structure(),
            Err(KagemushaReleaseVerificationError::InvalidPromotionRecord)
        );
    }
}

#[test]
fn v4_artifact_binding_requires_the_supplied_manifest_bytes() {
    let manifest = manifest();
    let canonical = norito::encode_canonical(&manifest).expect("canonical V4 manifest");
    let binding = KagemushaRecursiveSpendArtifactBindingV4 {
        version: KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
        generation: manifest.generation.clone(),
        manifest_sha256: digest(&canonical),
    };
    binding
        .validate_manifest(&manifest, &canonical)
        .expect("binding matches exact canonical manifest bytes");

    let arbitrary = b"not a V4 manifest";
    let forged = KagemushaRecursiveSpendArtifactBindingV4 {
        manifest_sha256: digest(arbitrary),
        ..binding.clone()
    };
    assert!(forged.validate_manifest(&manifest, arbitrary).is_err());

    let mut alternate = manifest.clone();
    alternate.release_attestation_sha256 = digest(b"alternate V4 attestation");
    alternate.validate().expect("alternate finalized manifest");
    let alternate_bytes =
        norito::encode_canonical(&alternate).expect("alternate canonical V4 manifest");
    let substituted = KagemushaRecursiveSpendArtifactBindingV4 {
        manifest_sha256: digest(&alternate_bytes),
        ..binding
    };
    assert!(
        substituted
            .validate_manifest(&manifest, &alternate_bytes)
            .is_err()
    );
}

#[test]
fn v4_activation_verifier_record_requires_exact_release_identity() {
    let key = VerifyingKeyBox::new(
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.to_owned(),
        b"exact V4 verifier key".to_vec(),
    );
    let mut manifest = manifest();
    let profile = manifest
        .profiles
        .iter_mut()
        .find(|profile| profile.parity == KagemushaPastaCycleParityV1::StepEq)
        .expect("Eq profile");
    let descriptor = profile
        .artifacts
        .get_mut(2)
        .expect("Eq verifier descriptor");
    descriptor.payload_size_bytes = u64::try_from(key.bytes.len()).expect("small test key");
    descriptor.size_bytes = descriptor.payload_size_bytes + 256;
    descriptor.payload_sha256 = digest(&key.bytes);
    let candidate = unsigned_candidate(&manifest);
    manifest.qualified_candidate_sha256 = kagemusha_recursive_spend_qualified_candidate_sha256_v4(
        candidate.sha256().expect("modified V4 candidate identity"),
        manifest.qualification_receipt_sha256,
    );
    manifest.validate().expect("modified finalized manifest");

    let manifest_sha256 = manifest.canonical_sha256().expect("manifest identity");
    let schema_hash = kagemusha_recursive_spend_verifier_public_inputs_schema_hash_v4(
        &manifest,
        KagemushaPastaCycleParityV1::StepEq,
    )
    .expect("Eq public-input identity");
    let commitment = verifying_key_commitment_v1(&key).expect("key commitment");
    let circuit_id = manifest.profiles[0].circuit_id.clone();
    let mut record = VerifyingKeyRecord::new_with_owner(
        7,
        circuit_id,
        Some(kagemusha_recursive_spend_verifier_owner_manifest_id_v4(
            manifest_sha256,
        )),
        KAGEMUSHA_VERIFIER_NAMESPACE,
        BackendTag::Halo2IpaPasta,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFIER_CURVE_V4,
        schema_hash,
        commitment,
    );
    record.vk_len = u32::try_from(key.bytes.len()).expect("small test key");
    record.max_proof_bytes = manifest.max_proof_bytes;
    record.activation_height = Some(manifest.activation_height);
    record.key = Some(key);
    record.status = ConfidentialStatus::Active;

    let mut activation = release_activation_wire_fixture();
    activation.release_record.manifest = manifest;
    activation
        .validate_verifier_record(
            &record,
            KagemushaPastaCycleParityV1::StepEq,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFIER_CURVE_V4,
        )
        .expect("exact release-bound verifier record");

    let assert_rejected = |record: &VerifyingKeyRecord| {
        assert_eq!(
            activation.validate_verifier_record(
                record,
                KagemushaPastaCycleParityV1::StepEq,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFIER_CURVE_V4,
            ),
            Err(KagemushaReleaseVerificationError::InvalidPromotionRecord)
        );
    };
    let mut wrong_owner = record.clone();
    wrong_owner.owner_manifest_id = Some("wrong-manifest".to_owned());
    assert_rejected(&wrong_owner);
    let mut wrong_schema = record.clone();
    wrong_schema.public_inputs_schema_hash[0] ^= 1;
    assert_rejected(&wrong_schema);
    let mut wrong_commitment = record;
    wrong_commitment.commitment[0] ^= 1;
    assert_rejected(&wrong_commitment);
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the release-identity regression keeps the complete V5 activation fixture and mutations together"
)]
fn v5_activation_verifier_record_requires_exact_release_identity() {
    let key = VerifyingKeyBox::new(
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.to_owned(),
        b"exact V5 verifier key".to_vec(),
    );
    let mut candidate = unsigned_candidate_v5();
    let profile = candidate
        .manifest
        .profiles
        .iter_mut()
        .find(|profile| profile.parity == KagemushaPastaCycleParityV1::StepEq)
        .expect("V5 Eq profile");
    let descriptor = profile
        .artifacts
        .get_mut(2)
        .expect("V5 Eq verifier descriptor");
    descriptor.payload_size_bytes = u64::try_from(key.bytes.len()).expect("small test key");
    descriptor.size_bytes = descriptor.payload_size_bytes + 256;
    descriptor.payload_sha256 = digest(&key.bytes);
    candidate.validate().expect("modified V5 candidate");
    let candidate_sha256 = candidate.sha256().expect("modified V5 candidate identity");
    let mut manifest = candidate.manifest;
    manifest.qualification_receipt_sha256 = digest(b"V5 activation qualification receipt");
    manifest.qualified_candidate_sha256 = kagemusha_recursive_spend_qualified_candidate_sha256_v5(
        candidate_sha256,
        manifest.qualification_receipt_sha256,
    );
    manifest.benchmark_evidence_sha256 = digest(b"V5 activation benchmark");
    manifest.cryptographic_review_sha256 = digest(b"V5 activation review");
    manifest.release_attestation_sha256 = digest(b"V5 activation attestation");
    manifest.validate().expect("finalized V5 manifest");

    let manifest_sha256 = manifest.canonical_sha256().expect("V5 manifest identity");
    let schema_hash = kagemusha_recursive_spend_verifier_public_inputs_schema_hash_v5(
        &manifest,
        KagemushaPastaCycleParityV1::StepEq,
    )
    .expect("V5 Eq public-input identity");
    let commitment = verifying_key_commitment_v1(&key).expect("V5 key commitment");
    let mut record = VerifyingKeyRecord::new_with_owner(
        7,
        manifest.profiles[0].circuit_id.clone(),
        Some(kagemusha_recursive_spend_verifier_owner_manifest_id_v5(
            manifest_sha256,
        )),
        KAGEMUSHA_VERIFIER_NAMESPACE,
        BackendTag::Halo2IpaPasta,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFIER_CURVE_V4,
        schema_hash,
        commitment,
    );
    record.vk_len = u32::try_from(key.bytes.len()).expect("small test key");
    record.max_proof_bytes = manifest.max_proof_bytes;
    record.activation_height = Some(manifest.activation_height);
    record.key = Some(key);
    record.status = ConfidentialStatus::Active;

    let subject = manifest
        .release_attestation_subject()
        .expect("V5 activation subject");
    let release_attestation_sha256 = manifest.release_attestation_sha256;
    let promotion = KagemushaRecursiveSpendPromotedReleaseV5 {
        schema: KAGEMUSHA_RECURSIVE_SPEND_PROMOTED_RELEASE_SCHEMA_V5.to_owned(),
        version: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V5,
        generation: manifest.generation.clone(),
        candidate_sha256,
        qualification_receipt_sha256: manifest.qualification_receipt_sha256,
        qualified_candidate_sha256: manifest.qualified_candidate_sha256,
        manifest_sha256,
        release_attestation_sha256,
        release_policy_sha256: digest(b"V5 activation policy"),
        approved_signers: Vec::new(),
        artifact_inventory_verified: true,
        bridge_abi_version: manifest.bridge_abi_version,
        artifact_roles: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4
            .map(str::to_owned)
            .to_vec(),
        max_proof_bytes: manifest.max_proof_bytes,
    };
    let mut activation = KagemushaRecursiveSpendReleaseActivationV5 {
        release_record: KagemushaRecursiveSpendReleaseRecordV5 {
            manifest,
            release_attestation: KagemushaRecursiveSpendReleaseAttestationV5 {
                schema: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_SCHEMA_V5.to_owned(),
                version: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V5,
                subject,
                approvals: Vec::new(),
            },
            physical_device_benchmark_summary: Vec::new(),
            cryptographic_review_summary: Vec::new(),
            promotion_record: promotion,
        },
        configured_policy_sha256: digest(b"V5 activation policy"),
        step_eq_verifier_key_id: kagemusha_recursive_spend_verifier_key_id_v5(
            KagemushaPastaCycleParityV1::StepEq,
            manifest_sha256,
        ),
        step_eq_verifier_record: record.clone(),
        step_ep_verifier_key_id: kagemusha_recursive_spend_verifier_key_id_v5(
            KagemushaPastaCycleParityV1::StepEp,
            manifest_sha256,
        ),
        step_ep_verifier_record: record.clone(),
    };
    activation
        .validate_verifier_record(
            &record,
            KagemushaPastaCycleParityV1::StepEq,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFIER_CURVE_V4,
        )
        .expect("exact V5 release-bound verifier record");

    activation.step_eq_verifier_record.owner_manifest_id = Some("wrong-manifest".to_owned());
    assert_eq!(
        activation.validate_verifier_record(
            &activation.step_eq_verifier_record,
            KagemushaPastaCycleParityV1::StepEq,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFIER_CURVE_V4,
        ),
        Err(KagemushaReleaseVerificationError::InvalidPromotionRecord)
    );
    let mut wrong_schema = record.clone();
    wrong_schema.public_inputs_schema_hash[0] ^= 1;
    assert!(
        activation
            .validate_verifier_record(
                &wrong_schema,
                KagemushaPastaCycleParityV1::StepEq,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFIER_CURVE_V4,
            )
            .is_err()
    );
    let mut wrong_commitment = record;
    wrong_commitment.commitment[0] ^= 1;
    assert!(
        activation
            .validate_verifier_record(
                &wrong_commitment,
                KagemushaPastaCycleParityV1::StepEq,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFIER_CURVE_V4,
            )
            .is_err()
    );
}

#[test]
fn release_policy_rejects_threshold_sum_above_attestation_cap() {
    let roles = [
        KagemushaRecursiveSpendReleaseApprovalRoleV1::Release,
        KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview,
        KagemushaRecursiveSpendReleaseApprovalRoleV1::PhysicalDeviceBenchmark,
    ];
    let role_policies = roles
        .into_iter()
        .enumerate()
        .map(|(role_index, role)| {
            let mut authorized_signers = (0_u8..22)
                .map(|offset| {
                    KeyPair::from_seed(
                        vec![
                            100 + u8::try_from(role_index).expect("three roles") * 22 + offset;
                            32
                        ],
                        Algorithm::Ed25519,
                    )
                    .public_key()
                    .clone()
                })
                .collect::<Vec<_>>();
            authorized_signers.sort();
            KagemushaRecursiveSpendReleaseRolePolicyV1 {
                role,
                threshold: 22,
                authorized_signers,
            }
        })
        .collect::<Vec<_>>();
    let mut policy = KagemushaRecursiveSpendReleasePolicyV1 {
        schema: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_POLICY_SCHEMA_V1.to_owned(),
        version: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V1,
        policy_id: "threshold-cap-test".to_owned(),
        roles: role_policies,
    };
    assert_eq!(
        policy.validate(),
        Err(KagemushaReleaseVerificationError::InvalidPolicy)
    );
    policy.roles[1].threshold = 21;
    policy.roles[2].threshold = 21;
    policy
        .validate()
        .expect("exactly 64 required approvals fit the attestation cap");
}
