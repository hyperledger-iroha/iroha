// Same-scope regression coverage extracted to keep the parent source budget bounded.
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[test]
fn catalog_source_mutex_recovers_after_caught_panic() {
    let mutex = Mutex::new(());
    let panicked = std::panic::catch_unwind(|| {
        let _guard = lock_kagemusha_catalog_source_mutex_v4(&mutex);
        panic!("catalog source mutex poison fixture");
    });
    assert!(panicked.is_err(), "fixture must poison the mutex once");
    let _guard = lock_kagemusha_catalog_source_mutex_v4(&mutex);
    assert!(!mutex.is_poisoned());
}
fn candidate_binding_reviewed_source_closure(
    source_commit: &str,
    source_tree_sha256: [u8; 32],
) -> (KagemushaReviewedSourceClosureV1, [u8; 32]) {
    let tracked_binary_diff_sha256 = Sha256::digest([]).into();
    let untracked_path_mode_blob_oid_manifest_sha256 = Sha256::digest([]).into();
    let mut combined = Sha256::new();
    combined.update(b"iroha-source-diff-v1\0");
    combined.update(b"tracked-binary-diff-sha256\0");
    combined.update(tracked_binary_diff_sha256);
    combined.update(b"untracked-path-blob-manifest-sha256\0");
    combined.update(untracked_path_mode_blob_oid_manifest_sha256);
    let closure = KagemushaReviewedSourceClosureV1 {
        schema: KAGEMUSHA_REVIEWED_SOURCE_CLOSURE_SCHEMA_V1.to_owned(),
        base_commit: source_commit.to_owned(),
        source_commit: source_commit.to_owned(),
        source_repo_dirty: false,
        source_tree_sha256,
        tracked_binary_diff_sha256,
        untracked_file_count: 0,
        untracked_path_mode_blob_oid_manifest: Vec::new(),
        untracked_path_mode_blob_oid_manifest_sha256,
        tracked_cargo_lock_size_bytes: 1,
        tracked_cargo_lock_sha256: Sha256::digest([0x92]).into(),
        combined_source_fingerprint_sha256: combined.finalize().into(),
    };
    let descriptor_sha256 = closure
        .canonical_descriptor_sha256()
        .expect("candidate-binding reviewed source closure");
    (closure, descriptor_sha256)
}
fn candidate_binding_internal_validation_receipt(
    candidate: &iroha_data_model::offline::KagemushaRecursiveSpendCandidateV4,
    manifest: &KagemushaRecursiveSpendArtifactManifestV4,
) -> Vec<u8> {
    fn exact(seed: u8) -> KagemushaExactBytesDigestV1 {
        KagemushaExactBytesDigestV1 {
            byte_len: u64::from(seed) + 1,
            sha256: [seed.max(1); 32],
        }
    }
    fn fuzz(
        target: KagemushaInternalValidationFuzzTargetV1,
        seed: u8,
        source_tree_sha256: [u8; 32],
        tracked_cargo_lock_sha256: [u8; 32],
        standalone_fuzz_cargo_lock_sha256: [u8; 32],
    ) -> KagemushaInternalValidationFuzzOutcomeV1 {
        KagemushaInternalValidationFuzzOutcomeV1 {
            target,
            minimum_executions:
                KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_MIN_FUZZ_EXECUTIONS_V1,
            completed_executions:
                KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_MIN_FUZZ_EXECUTIONS_V1,
            crashes: 0,
            timeouts: 0,
            out_of_memory: 0,
            source_tree_sha256_after: source_tree_sha256,
            tracked_cargo_lock_sha256_after: tracked_cargo_lock_sha256,
            standalone_fuzz_cargo_lock_sha256_after: standalone_fuzz_cargo_lock_sha256,
            initial_corpus: exact(seed),
            final_corpus: exact(seed.wrapping_add(1)),
            engine_report: exact(seed.wrapping_add(2)),
        }
    }
    let runner = KeyPair::from_seed(vec![0xa7; 32], Algorithm::Ed25519);
    let validator_binary = exact(14);
    let reviewed_cargo_fuzz_binary_sha256 = [16; 32];
    let reviewed_fuzz_cargo_proxy_binary_sha256 = [17; 32];
    let reviewed_fuzz_rustc_binary_sha256 = [18; 32];
    let tracked_cargo_lock_sha256 = [6; 32];
    let standalone_fuzz_cargo_lock_sha256 = [7; 32];
    let tool_roles = [
        KagemushaInternalValidationToolRoleV1::Cargo,
        KagemushaInternalValidationToolRoleV1::Rustc,
        KagemushaInternalValidationToolRoleV1::Rustdoc,
        KagemushaInternalValidationToolRoleV1::CargoClippy,
        KagemushaInternalValidationToolRoleV1::ClippyDriver,
        KagemushaInternalValidationToolRoleV1::CargoFuzz,
        KagemushaInternalValidationToolRoleV1::FuzzCargoProxy,
        KagemushaInternalValidationToolRoleV1::FuzzRustc,
        KagemushaInternalValidationToolRoleV1::ValidationRunner,
    ];
    let tools = tool_roles
        .into_iter()
        .enumerate()
        .map(|(index, role)| {
            let mut executable = exact(u8::try_from(index + 8).expect("tool seed fits"));
            if role == KagemushaInternalValidationToolRoleV1::Cargo {
                executable.sha256 = manifest.reviewed_cargo_binary_sha256;
            } else if role == KagemushaInternalValidationToolRoleV1::Rustc {
                executable.sha256 = manifest.reviewed_rustc_binary_sha256;
            } else if role == KagemushaInternalValidationToolRoleV1::CargoFuzz {
                executable.sha256 = reviewed_cargo_fuzz_binary_sha256;
            } else if role == KagemushaInternalValidationToolRoleV1::FuzzCargoProxy {
                executable.sha256 = reviewed_fuzz_cargo_proxy_binary_sha256;
            } else if role == KagemushaInternalValidationToolRoleV1::FuzzRustc {
                executable.sha256 = reviewed_fuzz_rustc_binary_sha256;
            } else if role == KagemushaInternalValidationToolRoleV1::ValidationRunner {
                executable = validator_binary;
            }
            let version_output = if role == KagemushaInternalValidationToolRoleV1::CargoFuzz {
                KagemushaExactBytesDigestV1::from_bytes(
                    KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_CARGO_FUZZ_VERSION_OUTPUT_V1,
                )
                .expect("pinned cargo-fuzz version output")
            } else if role == KagemushaInternalValidationToolRoleV1::FuzzRustc {
                KagemushaExactBytesDigestV1::from_bytes(
                    KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_FUZZ_RUSTC_VERSION_OUTPUT_V1,
                )
                .expect("pinned fuzz rustc version output")
            } else {
                exact(u8::try_from(index + 40).expect("version seed fits"))
            };
            KagemushaInternalValidationToolV1 {
                role,
                executable,
                version_output,
            }
        })
        .collect();
    let commands = KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_REQUIRED_COMMANDS_V1
        .iter()
        .enumerate()
        .map(
            |(index, spec)| KagemushaInternalValidationCommandOutcomeV1 {
                ordinal: u16::try_from(index).expect("command ordinal fits"),
                command_id: spec.command_id.to_owned(),
                program: spec.program,
                argv: spec
                    .argv
                    .iter()
                    .map(|argument| (*argument).to_owned())
                    .collect(),
                working_directory: KagemushaInternalValidationWorkingDirectoryV1::SourceRoot,
                environment_manifest: exact(
                    u8::try_from(index + 70).expect("environment seed fits"),
                ),
                exit_code: 0,
                termination_signal: None,
                timed_out: false,
                log_archive: exact(u8::try_from(index + 130).expect("log seed fits")),
                fuzz: spec.fuzz_target.map(|target| {
                    fuzz(
                        target,
                        u8::try_from(index + 190).expect("fuzz seed fits"),
                        manifest.source_tree_sha256,
                        tracked_cargo_lock_sha256,
                        standalone_fuzz_cargo_lock_sha256,
                    )
                }),
            },
        )
        .collect();
    let validation_runner_public_key = runner.public_key().clone();
    let body = KagemushaRecursiveSpendInternalValidationReceiptBodyV1 {
        signature_domain: KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_SIGNATURE_DOMAIN_V1
            .to_owned(),
        validation_runner_identity_sha256: kagemusha_internal_validation_runner_identity_sha256_v1(
            &validation_runner_public_key,
            &validator_binary,
        )
        .expect("derive validation-runner identity"),
        validation_runner_public_key,
        candidate_sha256: candidate.sha256().expect("candidate identity"),
        qualification_receipt_sha256: manifest.qualification_receipt_sha256,
        qualified_candidate_sha256: manifest.qualified_candidate_sha256,
        source_commit: manifest.source_commit.clone(),
        source_git_tree: manifest.source_commit.clone(),
        source_tree_sha256: manifest.source_tree_sha256,
        source_repo_dirty: manifest.source_repo_dirty,
        reviewed_source_closure_descriptor_sha256: manifest
            .reviewed_source_closure_descriptor_sha256,
        authenticated_source_seal_projection_sha256: manifest
            .authenticated_source_seal_projection_sha256,
        tracked_cargo_lock: KagemushaReviewedTrackedCargoLockV2 {
            path: "Cargo.lock".to_owned(),
            git_blob_oid: "3333333333333333333333333333333333333333".to_owned(),
            git_mode: "100644".to_owned(),
            sha256: tracked_cargo_lock_sha256,
            size_bytes: 1024,
        },
        standalone_fuzz_cargo_lock: KagemushaReviewedTrackedCargoLockV2 {
            path: "fuzz/Cargo.lock".to_owned(),
            git_blob_oid: "4444444444444444444444444444444444444444".to_owned(),
            git_mode: "100644".to_owned(),
            sha256: standalone_fuzz_cargo_lock_sha256,
            size_bytes: 2048,
        },
        reviewed_cargo_binary_sha256: manifest.reviewed_cargo_binary_sha256,
        reviewed_rustc_binary_sha256: manifest.reviewed_rustc_binary_sha256,
        reviewed_cargo_fuzz_binary_sha256,
        reviewed_fuzz_cargo_proxy_binary_sha256,
        fuzz_cargo_proxy_program:
            KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_FUZZ_CARGO_PROXY_PROGRAM_V1.to_owned(),
        fuzz_cargo_proxy_contract:
            KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_FUZZ_CARGO_PROXY_CONTRACT_V1.to_owned(),
        reviewed_fuzz_rustc_binary_sha256,
        cargo_fuzz_version: KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_CARGO_FUZZ_VERSION_V1
            .to_owned(),
        fuzz_rustc_version: KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_FUZZ_RUSTC_VERSION_V1
            .to_owned(),
        generator_binary_sha256: manifest.generator_binary_sha256,
        sealed_candidate_build_report_sha256: manifest.sealed_candidate_build_report_sha256,
        candidate_validation_report: exact(12),
        host_triple: KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_FUZZ_TARGET_TRIPLE_V1.to_owned(),
        target_triple: KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_FUZZ_TARGET_TRIPLE_V1
            .to_owned(),
        validator_binary,
        toolchain_manifest: exact(15),
        tools,
        commands,
    };
    let receipt = KagemushaRecursiveSpendInternalValidationReceiptV1::try_sign(body, &runner)
        .expect("sign candidate-bound internal-validation receipt");
    norito::encode_canonical(&receipt).expect("canonical internal-validation receipt")
}
fn authenticated_candidate_binding_release() -> (
    KagemushaAuthenticatedReleaseV4,
    KagemushaRecursiveSpendPromotedReleaseV4,
    iroha_data_model::offline::KagemushaRecursiveSpendReleaseRecordV4,
) {
    authenticated_candidate_binding_release_for_network(NetworkId::from_genesis_hash(
        iroha_crypto::HashOf::<iroha_data_model::block::BlockHeader>::from_untyped_unchecked(
            iroha_crypto::Hash::new(b"candidate-binding-network"),
        ),
    ))
}
fn authenticated_candidate_binding_release_for_network(
    network_id: NetworkId,
) -> (
    KagemushaAuthenticatedReleaseV4,
    KagemushaRecursiveSpendPromotedReleaseV4,
    iroha_data_model::offline::KagemushaRecursiveSpendReleaseRecordV4,
) {
    let benchmark = b"signed candidate-binding device benchmark";
    let source_commit = "0123456789abcdef0123456789abcdef01234567";
    let source_tree_sha256 = [0x61; 32];
    let (reviewed_source_closure, reviewed_source_closure_descriptor_sha256) =
        candidate_binding_reviewed_source_closure(source_commit, source_tree_sha256);
    let mut manifest = KagemushaRecursiveSpendArtifactManifestV4 {
        schema: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4.to_owned(),
        version: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V4,
        bridge_abi_version:
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
        proof_backend: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.to_owned(),
        transcript_profile: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4.to_owned(),
        generation: "candidate-binding-release".to_owned(),
        source_commit: source_commit.to_owned(),
        source_tree_sha256,
        source_repo_dirty: false,
        reviewed_source_closure,
        reviewed_source_closure_descriptor_sha256,
        authenticated_source_seal_projection_sha256: [0x62; 32],
        reviewed_cargo_binary_sha256: [0x63; 32],
        reviewed_rustc_binary_sha256: [0x64; 32],
        generator_binary_sha256: [0x65; 32],
        sealed_candidate_build_report_sha256: [0x66; 32],
        network_id,
        asset: AssetDefinitionId::derive_from_components(
            DomainId::try_new("candidate", "binding").expect("candidate-binding domain"),
            "asset".parse().expect("candidate-binding asset name"),
        ),
        asset_scale: 2,
        activation_height: 1,
        withdrawal_height: 100,
        max_proof_bytes: 9_000,
        generation_memory_limit_bytes: iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_GENERATION_MEMORY_ABSOLUTE_MAX_BYTES_V4,
        generation_memory_enforcement_profile: iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_GENERATION_MEMORY_ENFORCEMENT_PROFILE_V4.to_owned(),
        qualification_receipt_sha256: [0x64; 32],
        qualified_candidate_sha256: [0; 32],
        internal_validation_receipt_sha256: [0; 32],
        profiles: vec![
            candidate_binding_profile(KagemushaPastaCycleParityV1::StepEq, 0x10),
            candidate_binding_profile(KagemushaPastaCycleParityV1::StepEp, 0x20),
        ],
        topup_finality_roster_artifact: KagemushaTopUpFinalityRosterArtifactReferenceV4 {
            file_name: KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V4.to_owned(),
            size_bytes: 128,
            sha256: [0x31; 32],
            artifact_generation: "candidate-binding-release".to_owned(),
            circuit_id: KAGEMUSHA_TOPUP_FINALITY_CIRCUIT_ID_V2.to_owned(),
            purpose: KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_PURPOSE_V2.to_owned(),
            artifact_type: KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_TYPE_V2.to_owned(),
            required_bridge_abi_version:
                iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
        },
        benchmark_evidence_sha256: Sha256::digest(benchmark).into(),
        cryptographic_review_sha256: [0x63; 32],
        release_attestation_sha256: [0x62; 32],
    };
    let mut candidate_manifest = manifest.clone();
    candidate_manifest.qualification_receipt_sha256 = [0; 32];
    candidate_manifest.qualified_candidate_sha256 = [0; 32];
    candidate_manifest.internal_validation_receipt_sha256 = [0; 32];
    candidate_manifest.benchmark_evidence_sha256 = [0; 32];
    candidate_manifest.cryptographic_review_sha256 = [0; 32];
    candidate_manifest.release_attestation_sha256 = [0; 32];
    let candidate = iroha_data_model::offline::KagemushaRecursiveSpendCandidateV4 {
        schema: iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_CANDIDATE_SCHEMA_V4.to_owned(),
        version: iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_CANDIDATE_VERSION_V4,
        manifest: candidate_manifest,
    };
    manifest.qualified_candidate_sha256 =
        iroha_data_model::offline::kagemusha_recursive_spend_qualified_candidate_sha256_v4(
            candidate
                .sha256()
                .expect("candidate-binding candidate digest"),
            manifest.qualification_receipt_sha256,
        );
    let roles = [
        KagemushaRecursiveSpendReleaseApprovalRoleV1::Release,
        KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview,
        KagemushaRecursiveSpendReleaseApprovalRoleV1::PhysicalDeviceBenchmark,
    ];
    let key_pairs = [
        KeyPair::from_seed(vec![0x71; 32], Algorithm::Ed25519),
        KeyPair::from_seed(vec![0x72; 32], Algorithm::Ed25519),
        KeyPair::from_seed(vec![0x73; 32], Algorithm::Ed25519),
    ];
    let candidate = manifest
        .immutable_candidate()
        .expect("candidate-binding immutable candidate");
    let review_payload = KagemushaRecursiveSpendCryptographicReviewPayloadV4::approved(
        &candidate,
        manifest.qualification_receipt_sha256,
        manifest.qualified_candidate_sha256,
        [0x81; 32],
        [
            [0x82; 32], [0x83; 32], [0x84; 32], [0x85; 32], [0x86; 32], [0x87; 32],
        ],
    )
    .expect("candidate-binding review payload");
    let review = norito::to_bytes(&KagemushaRecursiveSpendCryptographicReviewEvidenceV4 {
        schema: KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_SCHEMA_V4.to_owned(),
        version: KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_VERSION_V4,
        approvals: vec![KagemushaRecursiveSpendCryptographicReviewApprovalV4 {
            public_key: key_pairs[1].public_key().clone(),
            signature: SignatureOf::try_new(key_pairs[1].private_key(), &review_payload)
                .expect("candidate-binding review signature"),
        }],
        payload: review_payload,
    })
    .expect("candidate-binding canonical signed review");
    manifest.cryptographic_review_sha256 = Sha256::digest(&review).into();
    let internal_validation_receipt =
        candidate_binding_internal_validation_receipt(&candidate, &manifest);
    manifest.internal_validation_receipt_sha256 =
        Sha256::digest(&internal_validation_receipt).into();
    let policy = KagemushaRecursiveSpendReleasePolicyV1 {
        schema: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_POLICY_SCHEMA_V1.to_owned(),
        version: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V1,
        policy_id: "candidate-binding-policy".to_owned(),
        internal_validation_runner_identity_sha256:
            KagemushaRecursiveSpendInternalValidationReceiptV1::decode_canonical(
                &internal_validation_receipt,
            )
            .expect("canonical candidate-binding internal-validation receipt")
            .body
            .validation_runner_identity_sha256,
        roles: roles
            .iter()
            .zip(&key_pairs)
            .map(
                |(&role, key_pair)| KagemushaRecursiveSpendReleaseRolePolicyV1 {
                    role,
                    threshold: 1,
                    authorized_signers: vec![key_pair.public_key().clone()],
                },
            )
            .collect(),
    };
    let subject = manifest
        .release_attestation_subject()
        .expect("candidate-binding release subject");
    let attestation = KagemushaRecursiveSpendReleaseAttestationV4 {
        schema: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_SCHEMA_V4.to_owned(),
        version: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V4,
        subject: subject.clone(),
        approvals: roles
            .iter()
            .zip(&key_pairs)
            .map(
                |(&role, key_pair)| KagemushaRecursiveSpendReleaseApprovalV4 {
                    role,
                    public_key: key_pair.public_key().clone(),
                    signature: SignatureOf::try_new(
                        key_pair.private_key(),
                        &subject.approval_payload(role),
                    )
                    .expect("candidate-binding release signature"),
                },
            )
            .collect(),
    };
    manifest.release_attestation_sha256 =
        Sha256::digest(norito::to_bytes(&attestation).expect("candidate-binding attestation"))
            .into();
    let authenticated = KagemushaAuthenticatedReleaseV4::verify(
        &manifest,
        &policy,
        &attestation,
        &internal_validation_receipt,
        benchmark,
        &review,
    )
    .expect("authenticated candidate-binding release");
    let candidate_sha256 = manifest
        .immutable_candidate()
        .and_then(|candidate| candidate.sha256())
        .expect("canonical candidate binding");
    let promotion = KagemushaRecursiveSpendPromotedReleaseV4 {
        schema: KAGEMUSHA_RECURSIVE_SPEND_PROMOTED_RELEASE_SCHEMA_V4.to_owned(),
        version: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V4,
        generation: manifest.generation.clone(),
        authenticated_source_seal_projection_sha256: manifest
            .authenticated_source_seal_projection_sha256,
        reviewed_cargo_binary_sha256: manifest.reviewed_cargo_binary_sha256,
        reviewed_rustc_binary_sha256: manifest.reviewed_rustc_binary_sha256,
        generator_binary_sha256: manifest.generator_binary_sha256,
        sealed_candidate_build_report_sha256: manifest.sealed_candidate_build_report_sha256,
        candidate_sha256,
        qualification_receipt_sha256: manifest.qualification_receipt_sha256,
        qualified_candidate_sha256: manifest.qualified_candidate_sha256,
        internal_validation_receipt_sha256: manifest.internal_validation_receipt_sha256,
        manifest_sha256: authenticated.manifest_sha256(),
        release_attestation_sha256: authenticated.release_attestation_sha256(),
        release_policy_sha256: authenticated.release_policy_sha256(),
        approved_signers: authenticated.approved_signers().to_vec(),
        artifact_inventory_verified: true,
        bridge_abi_version:
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
        artifact_roles: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4
            .map(str::to_owned)
            .to_vec(),
        max_proof_bytes: manifest.max_proof_bytes,
    };
    let release_record = iroha_data_model::offline::KagemushaRecursiveSpendReleaseRecordV4 {
        manifest: manifest.clone(),
        release_attestation: attestation,
        internal_validation_receipt,
        physical_device_benchmark_summary: benchmark.to_vec(),
        cryptographic_review_summary: review,
        promotion_record: promotion.clone(),
    };
    (authenticated, promotion, release_record)
}
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
fn sealed_parity_fixture(
    manifest: &KagemushaRecursiveSpendArtifactManifestV4,
    parity: KagemushaPastaCycleParityV1,
    commitment_tag: u8,
) -> KagemushaCatalogSealedParityQualificationV1 {
    let profile = profile(manifest, parity).expect("fixture parity profile");
    let verifying_key = kagemusha_artifact_descriptor_v4(
        manifest,
        parity,
        KagemushaPastaCycleArtifactKindV4::VerifyingKey,
    )
    .expect("fixture verifier-key descriptor");
    KagemushaCatalogSealedParityQualificationV1 {
        parity,
        circuit_params: profile.circuit_params.clone(),
        compiled_protocol_structure_sha256: profile.compiled_protocol_structure_sha256,
        // The production seal captures the full protocol identity, which
        // intentionally differs from the value-free structure digest.
        compiled_protocol_identity_sha256: [commitment_tag ^ 0x5a; 32],
        processed_verifying_key_len: verifying_key.payload_size_bytes,
        processed_verifying_key_sha256: verifying_key.payload_sha256,
        verifying_key_commitment: [commitment_tag; 32],
        proving_key_embedded_verifying_key_sha256: verifying_key.payload_sha256,
        proving_key_fixed_columns: 1,
        proving_key_permutation_columns: 1,
    }
}
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
fn qualification_seal_fixture(
    policy_path: &Path,
    artifact_dir: &Path,
) -> KagemushaCatalogQualificationSealV1 {
    let (authenticated, promotion, _) = authenticated_candidate_binding_release();
    qualification_seal_fixture_for_release(policy_path, artifact_dir, &authenticated, &promotion)
}
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
fn qualification_seal_fixture_for_release(
    policy_path: &Path,
    artifact_dir: &Path,
    authenticated: &KagemushaAuthenticatedReleaseV4,
    promotion: &KagemushaRecursiveSpendPromotedReleaseV4,
) -> KagemushaCatalogQualificationSealV1 {
    let manifest = authenticated.manifest();
    let executable =
        current_kagemusha_catalog_executable_path_v1().expect("fixture executable path");
    let qualification_receipt_path = artifact_dir
        .join(hex::encode(authenticated.manifest_sha256()))
        .join(KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_FILE_NAME_V4);
    let mut paths = BTreeMap::new();
    let fixture_stat = |inode, mode, length| KagemushaCatalogSealedStatV1 {
        device: 1,
        inode,
        mode,
        owner_uid: 0,
        owner_gid: 0,
        links: 1,
        length,
        modified_seconds: 1,
        modified_nanoseconds: 1,
        changed_seconds: 1,
        changed_nanoseconds: 1,
    };
    for (path, kind, stat) in [
        (
            policy_path,
            KagemushaCatalogSealedPathKindV1::File,
            fixture_stat(1, 0o100440, 1),
        ),
        (
            artifact_dir,
            KagemushaCatalogSealedPathKindV1::Directory,
            fixture_stat(2, 0o040550, 1),
        ),
        (
            executable.as_path(),
            KagemushaCatalogSealedPathKindV1::File,
            fixture_stat(3, 0o100550, 1),
        ),
        (
            qualification_receipt_path.as_path(),
            KagemushaCatalogSealedPathKindV1::File,
            fixture_stat(4, 0o100440, 777),
        ),
    ] {
        let canonical_path =
            canonical_catalog_path_string_v1(path, "qualification seal fixture path")
                .expect("canonical fixture path");
        paths.insert(
            canonical_path.clone(),
            KagemushaCatalogSealedPathV1 {
                canonical_path,
                kind,
                stat,
            },
        );
    }
    let artifacts = manifest
        .profiles
        .iter()
        .flat_map(|profile| {
            profile.artifacts.iter().cloned().map(move |artifact| {
                KagemushaCatalogSealedArtifactDigestV1 {
                    parity: profile.parity,
                    artifact,
                }
            })
        })
        .collect();
    KagemushaCatalogQualificationSealV1 {
        schema: KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_SCHEMA_V1.to_owned(),
        version: KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_VERSION_V1,
        canonical_policy_path: canonical_catalog_path_string_v1(
            policy_path,
            "fixture release policy",
        )
        .expect("fixture policy path"),
        canonical_artifact_dir: canonical_catalog_path_string_v1(
            artifact_dir,
            "fixture artifact root",
        )
        .expect("fixture artifact path"),
        canonical_executable_path: canonical_catalog_path_string_v1(
            &executable,
            "fixture executable",
        )
        .expect("fixture executable path"),
        build_fingerprint_sha256: current_kagemusha_catalog_build_fingerprint_v1(),
        executable_sha256: [0xa1; 32],
        configured_policy_sha256: authenticated.release_policy_sha256(),
        paths: paths.into_values().collect(),
        releases: vec![KagemushaCatalogSealedReleaseQualificationV1 {
            manifest_sha256: authenticated.manifest_sha256(),
            release_attestation_sha256: authenticated.release_attestation_sha256(),
            qualification_receipt_file_name:
                KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_FILE_NAME_V4.to_owned(),
            qualification_receipt_sha256: manifest.qualification_receipt_sha256,
            qualified_candidate_sha256: manifest.qualified_candidate_sha256,
            internal_validation_receipt_sha256: manifest.internal_validation_receipt_sha256,
            source_commit: manifest.source_commit.clone(),
            source_tree_sha256: manifest.source_tree_sha256,
            reviewed_source_closure_descriptor_sha256: manifest
                .reviewed_source_closure_descriptor_sha256,
            authenticated_source_seal_projection_sha256: manifest
                .authenticated_source_seal_projection_sha256,
            reviewed_cargo_binary_sha256: manifest.reviewed_cargo_binary_sha256,
            reviewed_rustc_binary_sha256: manifest.reviewed_rustc_binary_sha256,
            generator_binary_sha256: manifest.generator_binary_sha256,
            sealed_candidate_build_report_sha256: manifest.sealed_candidate_build_report_sha256,
            benchmark_evidence_sha256: manifest.benchmark_evidence_sha256,
            cryptographic_review_sha256: manifest.cryptographic_review_sha256,
            promotion_record_sha256: Sha256::digest(
                norito::encode_canonical(promotion).expect("canonical fixture promotion record"),
            )
            .into(),
            artifacts,
            step_eq: sealed_parity_fixture(manifest, KagemushaPastaCycleParityV1::StepEq, 0xb1),
            step_ep: sealed_parity_fixture(manifest, KagemushaPastaCycleParityV1::StepEp, 0xb2),
        }],
    }
}
