#[test]
fn agent_autonomy_request_commitment_uses_canonical_tuple() {
    let commitment = derive_agent_autonomy_request_commitment(
        "agent-apartment",
        "QmArtifactHash",
        Some("QmProvenanceHash"),
        42_000,
        "agent-apartment:autonomy:77",
        "nightly-retrain",
        Some("{\"inputs\":[\"alpha\",\"beta\"]}"),
        3,
    );
    let expected = Hash::new(Encode::encode(&(
        "agent-apartment",
        "QmArtifactHash",
        Some("QmProvenanceHash"),
        42_000u64,
        "agent-apartment:autonomy:77",
        "nightly-retrain",
        Some("{\"inputs\":[\"alpha\",\"beta\"]}"),
        3u64,
    )));
    assert_eq!(commitment, expected);
}
#[test]
fn training_job_start_provenance_payload_encodes_canonical_tuple() {
    let encoded = encode_training_job_start_provenance_payload(
        "web_portal",
        "model-1",
        "job-1",
        4,
        100,
        20,
        3,
        500,
        50_000,
        4_096,
    )
    .expect("encode payload");
    let expected = norito::to_bytes(&(
        "web_portal",
        "model-1",
        "job-1",
        4u16,
        100u32,
        20u32,
        3u8,
        500u64,
        50_000u64,
        4_096u64,
    ))
    .expect("encode tuple");
    assert_eq!(encoded, expected);
}
#[test]
fn training_job_checkpoint_provenance_payload_encodes_canonical_tuple() {
    let metrics_hash = sample_hash(1);
    let encoded = encode_training_job_checkpoint_provenance_payload(
        "web_portal",
        "job-1",
        20,
        1_024,
        metrics_hash,
    )
    .expect("encode payload");
    let expected = norito::to_bytes(&("web_portal", "job-1", 20u32, 1_024u64, metrics_hash))
        .expect("encode tuple");
    assert_eq!(encoded, expected);
}
#[test]
fn training_job_retry_provenance_payload_encodes_canonical_tuple() {
    let encoded =
        encode_training_job_retry_provenance_payload("web_portal", "job-1", "worker unavailable")
            .expect("encode payload");
    let expected =
        norito::to_bytes(&("web_portal", "job-1", "worker unavailable")).expect("encode tuple");
    assert_eq!(encoded, expected);
}
#[test]
fn model_artifact_register_provenance_payload_encodes_canonical_tuple() {
    let weight_artifact_hash = sample_hash(2);
    let training_config_hash = sample_hash(3);
    let reproducibility_hash = sample_hash(4);
    let provenance_attestation_hash = sample_hash(5);
    let encoded = encode_model_artifact_register_provenance_payload(
        "web_portal",
        "model-1",
        "job-1",
        weight_artifact_hash,
        "dataset://synthetic/v2",
        training_config_hash,
        reproducibility_hash,
        provenance_attestation_hash,
    )
    .expect("encode payload");
    let expected = norito::to_bytes(&(
        "web_portal",
        "model-1",
        "job-1",
        weight_artifact_hash,
        "dataset://synthetic/v2",
        training_config_hash,
        reproducibility_hash,
        provenance_attestation_hash,
    ))
    .expect("encode tuple");
    assert_eq!(encoded, expected);
}
#[test]
fn model_weight_register_provenance_payload_encodes_canonical_tuple() {
    let weight_artifact_hash = sample_hash(6);
    let training_config_hash = sample_hash(7);
    let reproducibility_hash = sample_hash(8);
    let provenance_attestation_hash = sample_hash(9);
    let encoded = encode_model_weight_register_provenance_payload(
        "web_portal",
        "model-1",
        "1.0.0",
        "job-1",
        Some("0.9.0"),
        weight_artifact_hash,
        "dataset://synthetic/v2",
        training_config_hash,
        reproducibility_hash,
        provenance_attestation_hash,
    )
    .expect("encode payload");
    let expected = norito::to_bytes(&(
        "web_portal",
        "model-1",
        "1.0.0",
        "job-1",
        Some("0.9.0"),
        weight_artifact_hash,
        "dataset://synthetic/v2",
        training_config_hash,
        reproducibility_hash,
        provenance_attestation_hash,
    ))
    .expect("encode tuple");
    assert_eq!(encoded, expected);
}
#[test]
fn model_weight_promote_provenance_payload_encodes_canonical_tuple() {
    let gate_report_hash = sample_hash(10);
    let encoded = encode_model_weight_promote_provenance_payload(
        "web_portal",
        "model-1",
        "1.0.0",
        true,
        gate_report_hash,
    )
    .expect("encode payload");
    let expected = norito::to_bytes(&("web_portal", "model-1", "1.0.0", true, gate_report_hash))
        .expect("encode tuple");
    assert_eq!(encoded, expected);
}
#[test]
fn model_weight_rollback_provenance_payload_encodes_canonical_tuple() {
    let encoded = encode_model_weight_rollback_provenance_payload(
        "web_portal",
        "model-1",
        "0.9.0",
        "gate regression",
    )
    .expect("encode payload");
    let expected = norito::to_bytes(&("web_portal", "model-1", "0.9.0", "gate regression"))
        .expect("encode tuple");
    assert_eq!(encoded, expected);
}
#[test]
fn uploaded_model_bundle_register_provenance_payload_encodes_bundle_value() {
    let bundle = SoraUploadedModelBundleV1 {
        schema_version: SORA_UPLOADED_MODEL_BUNDLE_VERSION_V1,
        service_name: sample_name("uploaded_model_registry"),
        model_id: "bundle-1".to_string(),
        weight_version: "v1".to_string(),
        family: "demo-family".to_string(),
        modalities: vec!["text".to_string()],
        plaintext_root: sample_hash(30),
        package_format: SoraUploadedModelPackageFormatV1::NormalizedHuggingFaceSafetensorsV1,
        bundle_root: sample_hash(31),
        sorafs_manifest_digest: ManifestDigest::new([0xA5; 32]),
        chunk_count: 2,
        plaintext_bytes: 2_048,
        ciphertext_bytes: 1_024,
        chunk_manifest_root: sample_hash(33),
        pricing_policy: SoraUploadedModelPricingPolicyV1 {
            storage_price: xor_quantity_from_nanos(10),
        },
    };
    let encoded = encode_uploaded_model_bundle_register_provenance_payload(bundle.clone())
        .expect("encode payload");
    let expected = norito::to_bytes(&bundle).expect("encode bundle");
    assert_eq!(encoded, expected);
}
#[test]
fn hf_shared_lease_join_provenance_payload_encodes_canonical_tuple() {
    let asset_definition_id = sample_asset_definition_id("4cuvDVPuLBKJyN6dPbRQhmLh68sU");
    let base_fee = xor_quantity_from_nanos(10_000);
    let encoded = encode_hf_shared_lease_join_provenance_payload(
        "openai/demo-model",
        "4f9d72c",
        "demo_service",
        Some("demo_apartment"),
        StorageClass::Warm,
        604_800_000,
        &asset_definition_id,
        &base_fee,
    )
    .expect("encode payload");
    let expected = norito::to_bytes(&(
        "openai/demo-model",
        "4f9d72c",
        "demo_service",
        Some("demo_apartment"),
        StorageClass::Warm,
        604_800_000u64,
        asset_definition_id,
        base_fee,
    ))
    .expect("encode tuple");
    assert_eq!(encoded, expected);
}
#[test]
fn hf_shared_lease_leave_provenance_payload_encodes_canonical_tuple() {
    let encoded = encode_hf_shared_lease_leave_provenance_payload(
        "openai/demo-model",
        "4f9d72c",
        StorageClass::Warm,
        604_800_000,
        Some("demo_service"),
        Some("demo_apartment"),
    )
    .expect("encode payload");
    let expected = norito::to_bytes(&(
        "openai/demo-model",
        "4f9d72c",
        StorageClass::Warm,
        604_800_000u64,
        Some("demo_service"),
        Some("demo_apartment"),
    ))
    .expect("encode tuple");
    assert_eq!(encoded, expected);
}
#[test]
fn hf_shared_lease_renew_provenance_payload_encodes_canonical_tuple() {
    let asset_definition_id = sample_asset_definition_id("4cuvDVPuLBKJyN6dPbRQhmLh68sU");
    let base_fee = xor_quantity_from_nanos(10_000);
    let encoded = encode_hf_shared_lease_renew_provenance_payload(
        "openai/demo-model",
        "4f9d72c",
        "demo_service",
        Some("demo_apartment"),
        StorageClass::Warm,
        604_800_000,
        &asset_definition_id,
        &base_fee,
    )
    .expect("encode payload");
    let expected = norito::to_bytes(&(
        "openai/demo-model",
        "4f9d72c",
        "demo_service",
        Some("demo_apartment"),
        StorageClass::Warm,
        604_800_000u64,
        asset_definition_id,
        base_fee,
    ))
    .expect("encode tuple");
    assert_eq!(encoded, expected);
}
#[test]
fn inrou_host_advertise_provenance_payload_encodes_purpose_bound_preimage() {
    let capability = sample_inrou_host_capability_record();
    let encoded =
        encode_inrou_host_advertise_provenance_payload(&capability).expect("encode payload");
    let semantic_payload =
        norito::encode_canonical(&capability).expect("encode capability payload");
    let expected = norito::encode_canonical(&(
        SORACLOUD_RUNTIME_PROVENANCE_DOMAIN_V1.to_vec(),
        SORACLOUD_RUNTIME_PROVENANCE_PREIMAGE_VERSION_V1,
        SoracloudRuntimeProvenancePurposeV1::InrouHostAdvert.wire_id(),
        semantic_payload,
    ))
    .expect("encode expected provenance preimage");
    assert_eq!(encoded, expected);
    validate_soracloud_runtime_provenance_preimage_v1(
        SoracloudRuntimeProvenancePurposeV1::InrouHostAdvert,
        &encoded,
    )
    .expect("Inrou advert purpose must validate");
}
#[test]
fn runtime_provenance_signature_cannot_replay_across_purposes() {
    let canonical_payload =
        norito::encode_canonical(&("same-payload", 7u64)).expect("encode shared semantic payload");
    let withdraw_preimage = encode_soracloud_runtime_provenance_preimage_v1(
        SoracloudRuntimeProvenancePurposeV1::InrouHostWithdraw,
        &canonical_payload,
    )
    .expect("encode heartbeat preimage");
    let inrou_preimage = encode_soracloud_runtime_provenance_preimage_v1(
        SoracloudRuntimeProvenancePurposeV1::InrouHostAdvert,
        &canonical_payload,
    )
    .expect("encode Inrou preimage");
    assert_ne!(withdraw_preimage, inrou_preimage);
    assert_eq!(
        validate_soracloud_runtime_provenance_preimage_v1(
            SoracloudRuntimeProvenancePurposeV1::InrouHostAdvert,
            &withdraw_preimage,
        ),
        Err(SoracloudRuntimeProvenancePreimageErrorV1::PurposeMismatch)
    );
    let signer = sample_ed25519_keypair(0x9A);
    let signature = Signature::try_new(signer.private_key(), &withdraw_preimage)
        .expect("sign withdrawal preimage");
    signature
        .verify(signer.public_key(), &withdraw_preimage)
        .expect("same-purpose signature must verify");
    assert!(
        signature
            .verify(signer.public_key(), &inrou_preimage)
            .is_err(),
        "an Inrou withdrawal signature must not verify as an Inrou advert"
    );
}
#[test]
fn runtime_provenance_preimage_validator_rejects_non_v1_framing() {
    let expected_purpose = SoracloudRuntimeProvenancePurposeV1::InrouHostWithdraw;
    assert_eq!(
        validate_soracloud_runtime_provenance_preimage_v1(
            expected_purpose,
            b"not-a-norito-preimage",
        ),
        Err(SoracloudRuntimeProvenancePreimageErrorV1::Malformed)
    );
    let wrong_domain = norito::encode_canonical(&(
        b"iroha:soracloud:other-domain:v1\x00".to_vec(),
        SORACLOUD_RUNTIME_PROVENANCE_PREIMAGE_VERSION_V1,
        expected_purpose.wire_id(),
        b"payload".to_vec(),
    ))
    .expect("encode wrong-domain preimage");
    assert_eq!(
        validate_soracloud_runtime_provenance_preimage_v1(expected_purpose, &wrong_domain,),
        Err(SoracloudRuntimeProvenancePreimageErrorV1::DomainMismatch)
    );
    let wrong_version = norito::encode_canonical(&(
        SORACLOUD_RUNTIME_PROVENANCE_DOMAIN_V1.to_vec(),
        2u8,
        expected_purpose.wire_id(),
        b"payload".to_vec(),
    ))
    .expect("encode wrong-version preimage");
    assert_eq!(
        validate_soracloud_runtime_provenance_preimage_v1(expected_purpose, &wrong_version,),
        Err(SoracloudRuntimeProvenancePreimageErrorV1::VersionMismatch)
    );
    let mut trailing =
        encode_soracloud_runtime_provenance_preimage_v1(expected_purpose, b"payload")
            .expect("encode canonical preimage");
    trailing.push(0);
    assert_eq!(
        validate_soracloud_runtime_provenance_preimage_v1(expected_purpose, &trailing,),
        Err(SoracloudRuntimeProvenancePreimageErrorV1::Malformed)
    );
}
#[test]
fn runtime_provenance_purpose_rejects_unknown_wire_ids() {
    assert_eq!(
        SoracloudRuntimeProvenancePurposeV1::try_from_wire_id(2),
        Ok(SoracloudRuntimeProvenancePurposeV1::InrouHostWithdraw)
    );
    for unknown in [0, 3, u8::MAX] {
        assert_eq!(
            SoracloudRuntimeProvenancePurposeV1::try_from_wire_id(unknown),
            Err(SoracloudRuntimeProvenancePurposeErrorV1)
        );
    }
}
#[test]
fn inrou_host_withdraw_provenance_payload_is_purpose_bound() {
    let validator_account_id = sample_account_id(0xD1);
    let encoded = encode_inrou_host_withdraw_provenance_payload(&validator_account_id)
        .expect("encode payload");
    let canonical = norito::to_bytes(&validator_account_id).expect("encode account id");
    let expected = encode_soracloud_runtime_provenance_preimage_v1(
        SoracloudRuntimeProvenancePurposeV1::InrouHostWithdraw,
        &canonical,
    )
    .expect("encode purpose-bound payload");
    assert_eq!(encoded, expected);
    validate_soracloud_runtime_provenance_preimage_v1(
        SoracloudRuntimeProvenancePurposeV1::InrouHostWithdraw,
        &encoded,
    )
    .expect("Inrou withdrawal purpose must validate");
    assert_eq!(
        validate_soracloud_runtime_provenance_preimage_v1(
            SoracloudRuntimeProvenancePurposeV1::InrouHostAdvert,
            &encoded,
        ),
        Err(SoracloudRuntimeProvenancePreimageErrorV1::PurposeMismatch)
    );
}
fn sample_fhe_policy_reference() -> SoracloudFhePolicyReferenceV1 {
    SoracloudFhePolicyReferenceV1 {
        schema_version: SORACLOUD_FHE_POLICY_REFERENCE_VERSION_V1,
        policy_name: "fhe_policy_med".parse().expect("valid policy name"),
        version: NonZeroU32::new(2).expect("nonzero version"),
        material_digest: sample_hash(93),
    }
}
#[test]
fn fhe_job_run_provenance_payload_encodes_canonical_payload() {
    let job = sample_fhe_job_spec();
    let policy_reference = sample_fhe_policy_reference();
    let encoded = encode_fhe_job_run_provenance_payload(
        "health_portal",
        "private_state",
        job.clone(),
        policy_reference.clone(),
        None,
        None,
        Vec::new(),
    )
    .expect("encode payload");
    let expected = norito::to_bytes(&FheJobRunProvenancePayloadV1 {
        service_name: "health_portal",
        binding_name: "private_state",
        job,
        policy_reference,
        public_key_proof: None,
        bootstrap_key_zero_refresh_proof: None,
        full_bootstrap_execution_proofs: Vec::new(),
    })
    .expect("encode payload");
    assert_eq!(encoded, expected);
}
#[test]
fn fhe_job_run_provenance_payload_binds_public_and_bootstrap_proofs() {
    let job = sample_fhe_job_spec();
    let reference = sample_fhe_policy_reference();
    let public_key_proof = sample_fhe_public_key_proof();
    let bootstrap_proof = sample_fhe_bootstrap_key_proof();
    let with_proofs = encode_fhe_job_run_provenance_payload(
        "health_portal",
        "private_state",
        job.clone(),
        reference.clone(),
        Some(public_key_proof),
        Some(bootstrap_proof),
        Vec::new(),
    )
    .expect("encode proof-carrying payload");
    let without_proofs = encode_fhe_job_run_provenance_payload(
        "health_portal",
        "private_state",
        job,
        reference,
        None,
        None,
        Vec::new(),
    )
    .expect("encode stripped payload");
    assert_ne!(with_proofs, without_proofs);
}
#[test]
fn fhe_job_run_provenance_payload_binds_execution_proof_order() {
    let first = sample_fhe_full_bootstrap_execution_proof();
    let second = sample_fhe_full_bootstrap_execution_proof_with_statement(sample_hash(22));
    let encoded = |proofs| {
        encode_fhe_job_run_provenance_payload(
            "health_portal",
            "private_state",
            sample_fhe_job_spec(),
            sample_fhe_policy_reference(),
            None,
            None,
            proofs,
        )
        .expect("encode execution proofs")
    };
    assert_ne!(
        encoded(vec![first.clone(), second.clone()]),
        encoded(vec![second, first]),
    );
}
#[test]
fn fhe_job_run_provenance_payload_binds_exact_policy_reference() {
    let mut changed = sample_fhe_policy_reference();
    changed.material_digest = sample_hash(94);
    let encoded = |reference| {
        encode_fhe_job_run_provenance_payload(
            "health_portal",
            "private_state",
            sample_fhe_job_spec(),
            reference,
            None,
            None,
            Vec::new(),
        )
        .expect("encode reference")
    };
    assert_ne!(encoded(sample_fhe_policy_reference()), encoded(changed));
}
#[test]
fn decryption_request_provenance_payload_encodes_canonical_tuple() {
    let policy = sample_decryption_authority_policy();
    let request = sample_decryption_request();
    let encoded = encode_decryption_request_provenance_payload(
        "health_portal",
        policy.clone(),
        request.clone(),
    )
    .expect("encode payload");
    let expected = norito::encode_canonical(&("health_portal", policy.clone(), request.clone()))
        .expect("encode canonical tuple");
    assert_eq!(encoded, expected);
    let _ambient = norito::core::DecodeFlagsGuard::enter(
        norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN,
    );
    assert_eq!(
        encode_decryption_request_provenance_payload("health_portal", policy, request)
            .expect("encode payload under alternate ambient layout"),
        expected
    );
}
#[test]
fn ciphertext_query_provenance_payload_encodes_canonical_layout() {
    let query = sample_ciphertext_query_spec();
    let encoded = encode_ciphertext_query_provenance_payload(&query).expect("encode payload");
    let expected = norito::encode_canonical(&query).expect("encode canonical query");
    assert_eq!(encoded, expected);
    let _ambient = norito::core::DecodeFlagsGuard::enter(
        norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN,
    );
    assert_eq!(
        encode_ciphertext_query_provenance_payload(&query)
            .expect("encode query under alternate ambient layout"),
        expected
    );
}
#[test]
fn bundle_provenance_payload_encodes_canonical_layout() {
    let container = sample_container();
    let mut service = sample_service(vec![sample_binding("private_state")]);
    service.container.manifest_hash = Hash::new(Encode::encode(&container));
    let bundle = SoraDeploymentBundleV1 {
        schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
        container,
        service,
    };
    let encoded = encode_bundle_provenance_payload(&bundle).expect("encode payload");
    let expected = norito::encode_canonical(&bundle).expect("encode canonical layout");
    assert_eq!(encoded, expected);
    let _ambient = norito::core::DecodeFlagsGuard::enter(
        norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN,
    );
    assert_eq!(
        encode_bundle_provenance_payload(&bundle)
            .expect("encode bundle under alternate ambient layout"),
        expected
    );
}
fn sample_binding(name: &str) -> SoraStateBindingV1 {
    SoraStateBindingV1 {
        schema_version: SORA_STATE_BINDING_VERSION_V1,
        binding_name: name.parse().expect("valid name"),
        scope: SoraStateScopeV1::ServiceState,
        mutability: SoraStateMutabilityV1::ReadWrite,
        encryption: SoraStateEncryptionV1::ClientCiphertext,
        key_prefix: "/state/demo".to_string(),
        max_item_bytes: NonZeroU64::new(4_096).expect("nonzero"),
        max_total_bytes: NonZeroU64::new(65_536).expect("nonzero"),
    }
}
fn sample_container() -> SoraContainerManifestV1 {
    SoraContainerManifestV1 {
        schema_version: SORA_CONTAINER_MANIFEST_VERSION_V1,
        runtime: SoraContainerRuntimeV1::Ivm,
        bundle_hash: sample_hash(7),
        bundle_path: "/bundles/site.to".to_string(),
        entrypoint: "main".to_string(),
        args: vec!["--http".to_string()],
        env: BTreeMap::from([("APP_ENV".to_string(), "prod".to_string())]),
        inrou: None,
        required_config_names: Vec::new(),
        required_secret_names: Vec::new(),
        config_exports: Vec::new(),
        capabilities: SoraCapabilityPolicyV1 {
            network: SoraNetworkPolicyV1::Allowlist(vec![
                SoraNetworkAllowlistEntryV1::new("api.sora.internal", [443]),
                SoraNetworkAllowlistEntryV1::new("rpc.sora.internal", [443]),
            ]),
            allow_state_writes: true,
            allow_model_inference: true,
            allow_model_training: false,
        },
        resources: SoraResourceLimitsV1 {
            cpu_millis: NonZeroU32::new(750).expect("nonzero"),
            memory_bytes: NonZeroU64::new(536_870_912).expect("nonzero"),
            ephemeral_storage_bytes: NonZeroU64::new(2_147_483_648).expect("nonzero"),
            max_open_files_per_process: NonZeroU32::new(512).expect("nonzero"),
            max_tasks: NonZeroU16::new(64).expect("nonzero"),
        },
        lifecycle: SoraLifecycleHooksV1 {
            start_grace_secs: NonZeroU32::new(30).expect("nonzero"),
            stop_grace_secs: NonZeroU32::new(15).expect("nonzero"),
            healthcheck_path: Some("/healthz".to_string()),
        },
    }
}
fn sample_inrou_manifest() -> SoraInrouManifestV1 {
    SoraInrouManifestV1 {
        schema_version: SORA_INROU_MANIFEST_VERSION_V1,
        guest_images: BTreeMap::from([
            (
                SoraInrouGuestIsaV1::X8664,
                SoraInrouGuestImageV1 {
                    kernel_image_path: "/inrou/x86_64/vmlinux".to_string(),
                    rootfs_image_path: "/inrou/x86_64/rootfs.ext4".to_string(),
                    initrd_image_path: None,
                    published_artifact: sample_published_inrou_guest_image_artifact(0x31),
                },
            ),
            (
                SoraInrouGuestIsaV1::Aarch64,
                SoraInrouGuestImageV1 {
                    kernel_image_path: "/inrou/aarch64/vmlinux".to_string(),
                    rootfs_image_path: "/inrou/aarch64/rootfs.ext4".to_string(),
                    initrd_image_path: None,
                    published_artifact: sample_published_inrou_guest_image_artifact(0x32),
                },
            ),
        ]),
    }
}
fn sample_published_inrou_guest_image_artifact(seed: u8) -> SoraPublishedInrouGuestImageArtifactV1 {
    SoraPublishedInrouGuestImageArtifactV1 {
        manifest_digest_hex: hex::encode([seed; 32]),
        content_cid: encode_lowercase_multibase_base32(
            &sorafs_manifest::canonical_manifest_root_cid([seed; 32]),
        ),
    }
}
fn assert_soracloud_invalid_field(error: SoracloudManifestError, expected_field: &'static str) {
    match error {
        SoracloudManifestError::InvalidField { field, .. } => {
            assert_eq!(field, expected_field);
        }
        other => panic!("expected invalid `{expected_field}` field, got {other:?}"),
    }
}
fn sample_inrou_lease_volumes() -> Vec<SoraLeaseVolumeBindingV1> {
    vec![
        SoraLeaseVolumeBindingV1 {
            volume_name: "root_disk".parse().expect("valid name"),
            kind: SoraLeaseVolumeKindV1::PersistentRootLeaseVolume,
            storage_class: StorageClass::Warm,
            mount_path: "/".to_string(),
            max_total_bytes: NonZeroU64::new(8 * 1024 * 1024 * 1024).expect("nonzero"),
        },
        SoraLeaseVolumeBindingV1 {
            volume_name: "index_state".parse().expect("valid name"),
            kind: SoraLeaseVolumeKindV1::ServiceLeaseVolume,
            storage_class: StorageClass::Warm,
            mount_path: "/var/lib/soracloud/volumes/index_state".to_string(),
            max_total_bytes: NonZeroU64::new(1024 * 1024).expect("nonzero"),
        },
    ]
}
fn sample_service(state_bindings: Vec<SoraStateBindingV1>) -> SoraServiceManifestV1 {
    SoraServiceManifestV1 {
        schema_version: SORA_SERVICE_MANIFEST_VERSION_V1,
        service_name: "portal".parse().expect("valid name"),
        service_version: "2026.1".to_string(),
        execution_plane: SoraServiceExecutionPlaneV1::DeterministicService,
        container: SoraContainerManifestRefV1 {
            manifest_hash: sample_hash(23),
            expected_schema_version: SORA_CONTAINER_MANIFEST_VERSION_V1,
        },
        replicas: NonZeroU16::new(3).expect("nonzero"),
        placement_targets: BTreeSet::new(),
        route: Some(SoraRouteTargetV1 {
            host: "portal.sora".to_string(),
            path_prefix: "/app".to_string(),
            service_port: NonZeroU16::new(8081).expect("nonzero"),
            visibility: SoraRouteVisibilityV1::Public,
            tls_mode: SoraTlsModeV1::Required,
        }),
        rollout: SoraRolloutPolicyV1 {
            canary_percent: 20,
            max_unavailable_replicas: 1,
            health_window_secs: NonZeroU32::new(60).expect("nonzero"),
            automatic_rollback_failures: NonZeroU32::new(2).expect("nonzero"),
        },
        economics: SoraHttpServiceEconomicsV1::default(),
        state_bindings,
        lease_volumes: Vec::new(),
        handlers: sample_handlers(),
        artifacts: sample_artifacts(),
    }
}
fn sample_asset_handler() -> SoraServiceHandlerV1 {
    SoraServiceHandlerV1 {
        handler_name: "assets".parse().expect("valid name"),
        class: SoraServiceHandlerClassV1::Asset,
        entrypoint: "serve_assets".to_string(),
        route_path: Some("/assets".to_string()),
        certified_response: SoraCertifiedResponsePolicyV1::StateCommitment,
        mailbox: None,
    }
}
fn sample_query_handler() -> SoraServiceHandlerV1 {
    SoraServiceHandlerV1 {
        handler_name: "query".parse().expect("valid name"),
        class: SoraServiceHandlerClassV1::Query,
        entrypoint: "serve_query".to_string(),
        route_path: Some("/query".to_string()),
        certified_response: SoraCertifiedResponsePolicyV1::AuditReceipt,
        mailbox: None,
    }
}
fn sample_update_handler() -> SoraServiceHandlerV1 {
    SoraServiceHandlerV1 {
        handler_name: "update".parse().expect("valid name"),
        class: SoraServiceHandlerClassV1::Update,
        entrypoint: "apply_update".to_string(),
        route_path: Some("/update".to_string()),
        certified_response: SoraCertifiedResponsePolicyV1::None,
        mailbox: Some(SoraMailboxContractV1 {
            queue_name: "updates".parse().expect("valid name"),
            max_pending_messages: NonZeroU32::new(1_024).expect("nonzero"),
            max_message_bytes: NonZeroU64::new(65_536).expect("nonzero"),
            retention_blocks: NonZeroU32::new(1_440).expect("nonzero"),
        }),
    }
}
fn sample_ciphertext_update_handler() -> SoraServiceHandlerV1 {
    SoraServiceHandlerV1 {
        handler_name: "ciphertext_update".parse().expect("valid name"),
        class: SoraServiceHandlerClassV1::Update,
        entrypoint: "apply_ciphertext_update".to_string(),
        route_path: Some("/ciphertext/update".to_string()),
        certified_response: SoraCertifiedResponsePolicyV1::None,
        mailbox: Some(SoraMailboxContractV1 {
            queue_name: "ciphertext_updates".parse().expect("valid name"),
            max_pending_messages: NonZeroU32::new(256).expect("nonzero"),
            max_message_bytes: NonZeroU64::new(131_072).expect("nonzero"),
            retention_blocks: NonZeroU32::new(2_880).expect("nonzero"),
        }),
    }
}
fn sample_handlers() -> Vec<SoraServiceHandlerV1> {
    vec![
        sample_asset_handler(),
        sample_query_handler(),
        sample_update_handler(),
        sample_ciphertext_update_handler(),
    ]
}
fn sample_artifacts() -> Vec<SoraArtifactRefV1> {
    vec![
        SoraArtifactRefV1 {
            kind: SoraArtifactKindV1::StaticAsset,
            artifact_hash: sample_hash(17),
            artifact_path: "/public/index.html".to_string(),
            handler_name: Some("assets".parse().expect("valid name")),
        },
        SoraArtifactRefV1 {
            kind: SoraArtifactKindV1::Journal,
            artifact_hash: sample_hash(17),
            artifact_path: "/journals/portal.journal".to_string(),
            handler_name: Some("update".parse().expect("valid name")),
        },
        SoraArtifactRefV1 {
            kind: SoraArtifactKindV1::Checkpoint,
            artifact_hash: sample_hash(17),
            artifact_path: "/checkpoints/portal.chk".to_string(),
            handler_name: Some("ciphertext_update".parse().expect("valid name")),
        },
    ]
}
fn sample_agent_apartment_manifest() -> AgentApartmentManifestV1 {
    AgentApartmentManifestV1 {
        schema_version: AGENT_APARTMENT_MANIFEST_VERSION_V1,
        apartment_name: "ops_agent".parse().expect("valid name"),
        container: SoraContainerManifestRefV1 {
            manifest_hash: sample_hash(41),
            expected_schema_version: SORA_CONTAINER_MANIFEST_VERSION_V1,
        },
        tool_capabilities: vec![
            AgentToolCapabilityV1 {
                tool: "soracloud.deploy".to_string(),
                max_invocations_per_epoch: NonZeroU32::new(128).expect("nonzero"),
                allow_network: true,
                allow_filesystem_write: false,
            },
            AgentToolCapabilityV1 {
                tool: "wallet.transfer".to_string(),
                max_invocations_per_epoch: NonZeroU32::new(32).expect("nonzero"),
                allow_network: false,
                allow_filesystem_write: false,
            },
        ],
        policy_capabilities: vec![
            "wallet.sign".parse().expect("valid name"),
            "governance.audit".parse().expect("valid name"),
        ],
        spend_limits: vec![AgentSpendLimitV1 {
            asset_definition: "61CtjvNd9T3THAR65GsMVHr82Bjc".to_string(),
            max_per_tx: xor_quantity_from_nanos(5_000_000),
            max_per_day: xor_quantity_from_nanos(20_000_000),
        }],
        state_quota_bytes: NonZeroU64::new(134_217_728).expect("nonzero"),
        network_egress: SoraNetworkPolicyV1::Allowlist(vec![
            SoraNetworkAllowlistEntryV1::new("rpc.sora.internal", [443]),
            SoraNetworkAllowlistEntryV1::new("torii.sora.internal", [443]),
        ]),
        upgrade_policy: AgentUpgradePolicyV1::Governed,
    }
}
fn sample_agent_apartment_record() -> SoraAgentApartmentRecordV1 {
    let manifest = sample_agent_apartment_manifest();
    let mailbox_payload = "{\"ping\":true}".to_string();
    SoraAgentApartmentRecordV1 {
        schema_version: SORA_AGENT_APARTMENT_RECORD_VERSION_V1,
        manifest_hash: manifest.manifest_hash(),
        manifest,
        deployed_sequence: 10,
        lease_started_height: 10,
        lease_expires_height: 110,
        last_renewed_height: 10,
        restart_count: 1,
        last_restart_sequence: Some(30),
        last_restart_reason: Some("policy refresh".to_string()),
        process_generation: 2,
        process_started_sequence: 30,
        last_active_sequence: 35,
        last_checkpoint_sequence: Some(34),
        checkpoint_count: 1,
        persistent_state: SoraAgentPersistentStateV1 {
            total_bytes: 128,
            key_sizes: BTreeMap::from([(String::from("/ops/state"), 128)]),
        },
        revoked_policy_capabilities: BTreeSet::from([String::from("wallet.sign")]),
        pending_wallet_requests: BTreeMap::from([(
            String::from("ops_agent:wallet:35"),
            SoraAgentWalletSpendRequestV1 {
                request_id: "ops_agent:wallet:35".to_string(),
                asset_definition: "61CtjvNd9T3THAR65GsMVHr82Bjc".to_string(),
                amount: xor_quantity_from_nanos(1_000_000),
                created_sequence: 35,
            },
        )]),
        wallet_daily_spend: BTreeMap::from([(
            String::from("61CtjvNd9T3THAR65GsMVHr82Bjc:0"),
            SoraAgentWalletDailySpendEntryV1 {
                asset_definition: "61CtjvNd9T3THAR65GsMVHr82Bjc".to_string(),
                day_bucket: 0,
                spent: xor_quantity_from_nanos(1_000_000),
            },
        )]),
        mailbox_queue: vec![SoraAgentMailboxMessageV1 {
            message_id: "worker_agent:mail:36".to_string(),
            from_apartment: "ops_agent".to_string(),
            channel: "ops".to_string(),
            payload_hash: Hash::new(mailbox_payload.as_bytes()),
            payload: mailbox_payload,
            enqueued_sequence: 36,
        }],
        autonomy_budget_ceiling_units: 500,
        autonomy_budget_remaining_units: 320,
        artifact_allowlist: BTreeMap::from([(
            String::from("hash:ABCD0123#01"),
            SoraAgentArtifactAllowRuleV1 {
                artifact_hash: "hash:ABCD0123#01".to_string(),
                provenance_hash: Some("prov:ABCD0123#01".to_string()),
                added_sequence: 20,
            },
        )]),
        autonomy_run_history: vec![SoraAgentAutonomyRunRecordV1 {
            run_id: "ops_agent:autonomy:33".to_string(),
            artifact_hash: "hash:ABCD0123#01".to_string(),
            provenance_hash: Some("prov:ABCD0123#01".to_string()),
            budget_units: 180,
            run_label: "nightly".to_string(),
            workflow_input_json: Some("{\"inputs\":\"nightly\"}".to_string()),
            approved_process_generation: 1,
            request_commitment: sample_hash(167),
            approved_sequence: 33,
        }],
    }
}
fn sample_agent_apartment_audit_event() -> SoraAgentApartmentAuditEventV1 {
    SoraAgentApartmentAuditEventV1 {
        schema_version: SORA_AGENT_APARTMENT_AUDIT_EVENT_VERSION_V1,
        sequence: 40,
        block_height: 40,
        block_timestamp_ms: 86_400_000,
        action: SoraAgentApartmentActionV1::Restart,
        apartment_name: sample_name("ops_agent"),
        status: SoraAgentRuntimeStatusV1::Running,
        lease_expires_height: 140,
        manifest_hash: sample_hash(44),
        restart_count: 2,
        signer: sample_signer(),
        request_id: None,
        asset_definition: None,
        amount: None,
        capability: None,
        reason: Some("manual recover".to_string()),
        from_apartment: None,
        to_apartment: None,
        channel: None,
        payload_hash: None,
        artifact_hash: None,
        provenance_hash: None,
        run_id: None,
        run_label: None,
        budget_units: None,
        service_name: None,
        service_version: None,
        handler_name: None,
        result_commitment: None,
        runtime_receipt_id: None,
        journal_artifact_hash: None,
        checkpoint_artifact_hash: None,
        succeeded: None,
    }
}
#[test]
fn agent_wallet_request_ids_are_canonical_in_persisted_state() {
    let record = sample_agent_apartment_record();
    record.validate().expect("canonical apartment record");

    let mut noncanonical_record = record;
    let mut noncanonical_request = noncanonical_record
        .pending_wallet_requests
        .remove("ops_agent:wallet:35")
        .expect("wallet request fixture");
    noncanonical_request.request_id = " ops_agent:wallet:35".to_owned();
    noncanonical_record.pending_wallet_requests.insert(
        noncanonical_request.request_id.clone(),
        noncanonical_request,
    );
    noncanonical_record
        .validate()
        .expect_err("noncanonical pending wallet request ID must fail closed");

    let mut wallet_event = sample_agent_apartment_audit_event();
    wallet_event.action = SoraAgentApartmentActionV1::WalletSpendRequested;
    wallet_event.request_id = Some("ops-wallet-request-1".to_owned());
    wallet_event.asset_definition = Some("61CtjvNd9T3THAR65GsMVHr82Bjc".to_owned());
    wallet_event.amount = Some(xor_quantity_from_nanos(1_000_000));
    wallet_event
        .validate()
        .expect("canonical wallet audit event");

    for request_id in [None, Some(" request-1".to_owned())] {
        let mut invalid = wallet_event.clone();
        invalid.request_id = request_id;
        invalid
            .validate()
            .expect_err("missing or noncanonical wallet audit request ID must fail closed");
    }
    let mut missing_amount = wallet_event;
    missing_amount.amount = None;
    missing_amount
        .validate()
        .expect_err("wallet audit event without amount must fail closed");
}
#[cfg(feature = "json")]
#[test]
fn canonical_agent_hosting_records_require_explicit_null_and_empty_keys() {
    macro_rules! assert_required_keys {
        (
            $value:expr,
            $ty:ty,
            [$($field:literal),+ $(,)?],
            [$($nullable:literal),* $(,)?],
            $label:literal
        ) => {{
            let canonical =
                norito::json::to_value(&$value).expect(concat!("serialize ", $label));
            norito::json::from_value::<$ty>(canonical.clone())
                .expect(concat!("canonical ", $label, " must decode"));
            $(
                let mut missing = canonical.clone();
                assert!(
                    missing
                        .as_object_mut()
                        .expect(concat!($label, " JSON object"))
                        .remove($field)
                        .is_some(),
                    "canonical {} must contain `{}`",
                    $label,
                    $field
                );
                norito::json::from_value::<$ty>(missing).expect_err(concat!(
                    $label,
                    " must reject an omitted canonical key"
                ));
            )+
            $(
                let mut explicit_null = canonical.clone();
                explicit_null
                    .as_object_mut()
                    .expect(concat!($label, " JSON object"))
                    .insert($nullable.to_owned(), norito::json::Value::Null);
                norito::json::from_value::<$ty>(explicit_null).expect(concat!(
                    $label,
                    " must accept an explicit nullable key"
                ));
            )*
        }};
    }

    assert_required_keys!(
        SoraAgentArtifactAllowRuleV1 {
            artifact_hash: "hash:ABCD0123#01".to_owned(),
            provenance_hash: None,
            added_sequence: 20,
        },
        SoraAgentArtifactAllowRuleV1,
        ["provenance_hash"],
        ["provenance_hash"],
        "agent artifact allow rule"
    );
    assert_required_keys!(
        SoraAgentAutonomyRunRecordV1 {
            run_id: "ops_agent:autonomy:33".to_owned(),
            artifact_hash: "hash:ABCD0123#01".to_owned(),
            provenance_hash: None,
            budget_units: 180,
            run_label: "nightly".to_owned(),
            workflow_input_json: None,
            approved_process_generation: 1,
            request_commitment: sample_hash(167),
            approved_sequence: 33,
        },
        SoraAgentAutonomyRunRecordV1,
        ["provenance_hash", "workflow_input_json"],
        ["provenance_hash", "workflow_input_json"],
        "agent autonomy-run record"
    );
    assert_required_keys!(
        SoraAgentPersistentStateV1 {
            total_bytes: 0,
            key_sizes: BTreeMap::new(),
        },
        SoraAgentPersistentStateV1,
        ["key_sizes"],
        [],
        "agent persistent state"
    );
    assert_required_keys!(
        sample_agent_apartment_record(),
        SoraAgentApartmentRecordV1,
        [
            "last_restart_sequence",
            "last_restart_reason",
            "last_checkpoint_sequence",
            "revoked_policy_capabilities",
            "pending_wallet_requests",
            "wallet_daily_spend",
            "mailbox_queue",
            "artifact_allowlist",
            "autonomy_run_history",
        ],
        [
            "last_restart_sequence",
            "last_restart_reason",
            "last_checkpoint_sequence",
        ],
        "agent apartment record"
    );
    assert_required_keys!(
        sample_agent_apartment_audit_event(),
        SoraAgentApartmentAuditEventV1,
        [
            "request_id",
            "asset_definition",
            "amount",
            "capability",
            "reason",
            "from_apartment",
            "to_apartment",
            "channel",
            "payload_hash",
            "artifact_hash",
            "provenance_hash",
            "run_id",
            "run_label",
            "budget_units",
            "service_name",
            "service_version",
            "handler_name",
            "result_commitment",
            "runtime_receipt_id",
            "journal_artifact_hash",
            "checkpoint_artifact_hash",
            "succeeded",
        ],
        [
            "request_id",
            "asset_definition",
            "amount",
            "capability",
            "reason",
            "from_apartment",
            "to_apartment",
            "channel",
            "payload_hash",
            "artifact_hash",
            "provenance_hash",
            "run_id",
            "run_label",
            "budget_units",
            "service_name",
            "service_version",
            "handler_name",
            "result_commitment",
            "runtime_receipt_id",
            "journal_artifact_hash",
            "checkpoint_artifact_hash",
            "succeeded",
        ],
        "agent apartment audit event"
    );
}
fn sample_fhe_param_set() -> FheParamSetV1 {
    FheParamSetV1 {
        schema_version: FHE_PARAM_SET_VERSION_V1,
        param_set: "fhe_bfv_med".parse().expect("valid name"),
        version: NonZeroU32::new(2).expect("nonzero"),
        backend: REGISTERED_SORACLOUD_BFV_BACKEND_V1.to_string(),
        scheme: FheSchemeV1::Bfv,
        ciphertext_modulus_bits: vec![
            NonZeroU16::new(60).expect("nonzero"),
            NonZeroU16::new(50).expect("nonzero"),
            NonZeroU16::new(40).expect("nonzero"),
        ],
        plaintext_modulus_bits: NonZeroU16::new(20).expect("nonzero"),
        polynomial_modulus_degree: NonZeroU32::new(8_192).expect("nonzero"),
        slot_count: NonZeroU32::new(4_096).expect("nonzero"),
        security_level_bits: NonZeroU16::new(128).expect("nonzero"),
        max_multiplicative_depth: NonZeroU16::new(2).expect("nonzero"),
        lifecycle: FheParamLifecycleV1::Active,
        activation_height: Some(10_000),
        withdraw_height: Some(40_000),
        parameter_digest: sample_hash(77),
        rns_modulus_chain_digest: sample_hash(78),
        key_switch_decomposition_chain_digest: sample_hash(79),
    }
}
fn sample_bfv_evaluation_key_bundle() -> BfvEvaluationKeyBundle {
    BfvEvaluationKeyBundle {
        relinearization_key: iroha_crypto::fhe_bfv::BfvRelinearizationKey {
            entries: Vec::new(),
        },
        rotation_keys: Vec::new(),
        galois_keys: Vec::new(),
        bootstrap_key: None,
    }
}
fn sample_bfv_refresh_transcript() -> BfvEvaluationKeyRefreshTranscriptV1 {
    BfvEvaluationKeyRefreshTranscriptV1 {
        public_key: BfvPublicKey {
            b: Vec::new(),
            a: Vec::new(),
        },
        rotation_transcripts: Vec::new(),
        bootstrap_transcript: None,
    }
}
fn sample_full_bootstrap_material(
    params: &iroha_crypto::fhe_bfv::BfvParameters,
) -> iroha_crypto::fhe_bfv::BfvFullBootstrapCircuitMaterialV1 {
    let artifacts = sample_full_bootstrap_circuit_artifacts();
    let max_bootstrap_depth = 1;
    let prover_key_material_commitment =
        bfv_full_bootstrap_proof_key_material_commitment_from_artifact_v1(
            params,
            max_bootstrap_depth,
            iroha_crypto::fhe_bfv::BfvFullBootstrapCircuitArtifactRoleV1::ProverKey,
            &artifacts.prover_key,
        )
        .expect("sample full-bootstrap prover-key material commitment");
    let verifier_key_material_commitment =
        bfv_full_bootstrap_proof_key_material_commitment_from_artifact_v1(
            params,
            max_bootstrap_depth,
            iroha_crypto::fhe_bfv::BfvFullBootstrapCircuitArtifactRoleV1::VerifierKey,
            &artifacts.verifier_key,
        )
        .expect("sample full-bootstrap verifier-key material commitment");
    let proof_key_pair_commitment = bfv_full_bootstrap_proof_key_pair_commitment_from_artifacts_v1(
        params,
        max_bootstrap_depth,
        &artifacts.prover_key,
        &artifacts.verifier_key,
    )
    .expect("sample full-bootstrap proof-key pair commitment");
    iroha_crypto::fhe_bfv::BfvFullBootstrapCircuitMaterialV1 {
        circuit_id: iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1.to_string(),
        parameter_digest: iroha_crypto::fhe_bfv::registered_bfv_parameter_digest(params)
            .expect("registered parameter digest"),
        rns_modulus_chain_digest: iroha_crypto::fhe_bfv::registered_bfv_rns_modulus_chain_digest(
            params,
        )
        .expect("registered RNS digest"),
        key_switch_decomposition_chain_digest:
            iroha_crypto::fhe_bfv::registered_bfv_key_switch_decomposition_chain_digest(params)
                .expect("registered decomposition digest"),
        centered_scale_round_source_chain_digest:
            iroha_crypto::fhe_bfv::registered_bfv_centered_scale_round_source_chain_digest(params)
                .expect("registered centered scale-round source-chain digest"),
        coefficient_to_slot_key_digest: Hash::new(&artifacts.coefficient_to_slot_key),
        slot_to_coefficient_key_digest: Hash::new(&artifacts.slot_to_coefficient_key),
        blind_rotation_key_digest: Hash::new(&artifacts.blind_rotation_key),
        sample_extraction_key_digest: Hash::new(&artifacts.sample_extraction_key),
        accumulator_digest: Hash::new(&artifacts.accumulator),
        proof_public_input_schema_digest: Hash::new(&artifacts.proof_public_input_schema),
        arithmetic_air_constraint_system_artifact_digest: Hash::new(
            &artifacts.arithmetic_air_constraint_system,
        ),
        proof_key_pair_commitment,
        prover_key_digest: Hash::new(&artifacts.prover_key),
        prover_key_material_commitment,
        verifier_key_digest: Hash::new(&artifacts.verifier_key),
        verifier_key_material_commitment,
        max_bootstrap_depth,
    }
}
const SAMPLE_FULL_BOOTSTRAP_RELEASE_AUDIT_REVIEWER_ID: &str = "sora-zk-audit-wg-2026";
fn sample_full_bootstrap_release_audit_package_and_digest(
    reviewer_keypair: &KeyPair,
) -> (
    iroha_crypto::fhe_bfv::BfvFullBootstrapReleaseAuditPackageV1,
    Hash,
) {
    let params = ram_lfe_bfv_parameters_v1();
    let artifacts = sample_full_bootstrap_circuit_artifacts();
    let material = sample_full_bootstrap_material(&params);
    let (generated_report_bytes, generated_archive_bytes) =
            iroha_crypto::fhe_bfv::bfv_full_bootstrap_release_audit_report_and_archive_bytes_for_artifacts_v1(
                &params,
                &material,
                &artifacts,
            )
            .expect("fixture release audit generated report/archive bytes");
    let generated_report_body = generated_report_bytes
        .strip_prefix(iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1)
        .expect("generated report bytes carry canonical header");
    let generated_archive_body = generated_archive_bytes
        .strip_prefix(iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_HEADER_V1)
        .expect("generated archive bytes carry canonical header");
    let report_suffix = generated_report_body
        .strip_prefix(b"machine-generated BFV full-bootstrap release audit report inventory v1")
        .expect("generated report body carries deterministic inventory prefix");
    let archive_suffix = generated_archive_body
        .strip_prefix(b"machine-generated BFV full-bootstrap release evidence archive inventory v1")
        .expect("generated archive body carries deterministic inventory prefix");
    let report_body = [
            b"external-review-approved: reviewer-id=sora-zk-audit-wg-2026 independent BFV full-bootstrap release audit report v1"
                .as_slice(),
            report_suffix,
        ]
        .concat();
    let archive_body = [
            b"external-review-evidence-archive: reviewer-id=sora-zk-audit-wg-2026 independent BFV full-bootstrap prover verifier evidence v1"
                .as_slice(),
            archive_suffix,
        ]
        .concat();
    let report_bytes =
        iroha_crypto::fhe_bfv::bfv_full_bootstrap_release_audit_report_bytes_v1(&report_body)
            .expect("fixture external-review report bytes");
    let archive_bytes =
        iroha_crypto::fhe_bfv::bfv_full_bootstrap_release_audit_archive_bytes_v1(&archive_body)
            .expect("fixture external-review archive bytes");
    iroha_crypto::fhe_bfv::bfv_full_bootstrap_release_audit_external_review_package_and_digest_v1(
        &params,
        &material,
        &artifacts,
        &report_bytes,
        &archive_bytes,
        SAMPLE_FULL_BOOTSTRAP_RELEASE_AUDIT_REVIEWER_ID,
        reviewer_keypair.private_key(),
    )
    .expect("fixture external-review release audit package and digest")
}
fn sample_structural_full_bootstrap_release_audit_package_and_digest_with_marker_statements(
    reviewer_keypair: &KeyPair,
    report_marker_statement: &[u8],
    archive_marker_statement: &[u8],
) -> (
    iroha_crypto::fhe_bfv::BfvFullBootstrapReleaseAuditPackageV1,
    Hash,
) {
    let params = ram_lfe_bfv_parameters_v1();
    let artifacts = sample_full_bootstrap_circuit_artifacts();
    let material = sample_full_bootstrap_material(&params);
    let (generated_report_bytes, generated_archive_bytes) =
            iroha_crypto::fhe_bfv::bfv_full_bootstrap_release_audit_report_and_archive_bytes_for_artifacts_v1(
                &params,
                &material,
                &artifacts,
            )
            .expect("fixture release audit generated report/archive bytes");
    let generated_report_body = generated_report_bytes
        .strip_prefix(iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1)
        .expect("generated report bytes carry canonical header");
    let generated_archive_body = generated_archive_bytes
        .strip_prefix(iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_HEADER_V1)
        .expect("generated archive bytes carry canonical header");
    let report_suffix = generated_report_body
        .strip_prefix(b"machine-generated BFV full-bootstrap release audit report inventory v1")
        .expect("generated report body carries deterministic inventory prefix");
    let archive_suffix = generated_archive_body
        .strip_prefix(b"machine-generated BFV full-bootstrap release evidence archive inventory v1")
        .expect("generated archive body carries deterministic inventory prefix");
    let report_body = [report_marker_statement, report_suffix].concat();
    let archive_body = [archive_marker_statement, archive_suffix].concat();
    let report_bytes =
        iroha_crypto::fhe_bfv::bfv_full_bootstrap_release_audit_report_bytes_v1(&report_body)
            .expect("fixture structural report bytes");
    let archive_bytes =
        iroha_crypto::fhe_bfv::bfv_full_bootstrap_release_audit_archive_bytes_v1(&archive_body)
            .expect("fixture structural archive bytes");
    let package = iroha_crypto::fhe_bfv::bfv_full_bootstrap_release_audit_package_v1(
        &params,
        &material,
        &artifacts,
        &report_bytes,
        &archive_bytes,
        SAMPLE_FULL_BOOTSTRAP_RELEASE_AUDIT_REVIEWER_ID,
        reviewer_keypair.private_key(),
    )
    .expect("fixture structural release audit package");
    let package_digest =
        iroha_crypto::fhe_bfv::bfv_full_bootstrap_release_audit_package_digest_v1(&package)
            .expect("fixture structural release audit package digest");
    (package, package_digest)
}
fn sample_fhe_execution_policy() -> FheExecutionPolicyV1 {
    FheExecutionPolicyV1 {
        schema_version: FHE_EXECUTION_POLICY_VERSION_V1,
        policy_name: "fhe_policy_med".parse().expect("valid name"),
        param_set: "fhe_bfv_med".parse().expect("valid name"),
        param_set_version: NonZeroU32::new(2).expect("nonzero"),
        evaluation_key_digest: sample_hash(90),
        evaluation_key_refresh_transcript_digest: sample_hash(91),
        refresh_transcript_mode: BfvRefreshTranscriptModeV1::ExactLift,
        public_key_proof_statement_digest: Some(sample_hash(89)),
        bootstrap_key_zero_refresh_proof_statement_digest: Some(sample_hash(92)),
        full_bootstrap_release_audit_package: None,
        full_bootstrap_release_audit_package_digest: None,
        full_bootstrap_release_audit_trusted_reviewer_id: None,
        full_bootstrap_release_audit_trusted_reviewer_public_key: None,
        max_ciphertext_bytes: NonZeroU64::new(131_072).expect("nonzero"),
        max_plaintext_bytes: NonZeroU64::new(16_384).expect("nonzero"),
        max_input_ciphertexts: NonZeroU16::new(8).expect("nonzero"),
        max_output_ciphertexts: NonZeroU16::new(4).expect("nonzero"),
        max_multiplication_depth: NonZeroU16::new(2).expect("nonzero"),
        max_rotation_count: NonZeroU32::new(128).expect("nonzero"),
        max_bootstrap_count: 1,
        rounding_mode: FheDeterministicRoundingModeV1::NearestTiesToEven,
    }
}
fn sample_fhe_job_spec() -> FheJobSpecV1 {
    FheJobSpecV1 {
        schema_version: FHE_JOB_SPEC_VERSION_V1,
        job_id: "job-add-001".to_string(),
        policy_name: "fhe_policy_med".parse().expect("valid name"),
        param_set: "fhe_bfv_med".parse().expect("valid name"),
        param_set_version: NonZeroU32::new(2).expect("nonzero"),
        operation: FheJobOperationV1::Add,
        inputs: vec![
            FheJobInputRefV1 {
                state_key: "/state/health/patient-1".to_string(),
                payload_bytes: NonZeroU64::new(2_048).expect("nonzero"),
                commitment: sample_hash(121),
            },
            FheJobInputRefV1 {
                state_key: "/state/health/patient-2".to_string(),
                payload_bytes: NonZeroU64::new(2_048).expect("nonzero"),
                commitment: sample_hash(122),
            },
        ],
        output_state_key: "/state/health/result-1".to_string(),
        requested_multiplication_depth: 0,
        rotation_steps: 0,
        bootstrap_count: 0,
    }
}
fn assert_fhe_job_invalid_field(
    label: &str,
    mutate: impl FnOnce(&mut FheJobSpecV1),
    expected_field: &'static str,
) {
    let mut job = sample_fhe_job_spec();
    mutate(&mut job);
    let error = job.validate().expect_err(label);
    assert!(
        matches!(
            &error,
            SoracloudManifestError::InvalidField { field, .. } if *field == expected_field
        ),
        "expected `{expected_field}` invalid-field error for {label}, got {error:?}"
    );
}
fn sample_decryption_authority_policy() -> DecryptionAuthorityPolicyV1 {
    DecryptionAuthorityPolicyV1 {
        schema_version: DECRYPTION_AUTHORITY_POLICY_VERSION_V1,
        policy_name: "phi_threshold_policy".parse().expect("valid name"),
        mode: DecryptionAuthorityModeV1::ThresholdService,
        approver_quorum: NonZeroU16::new(2).expect("nonzero"),
        approver_ids: vec![
            "compliance_council".parse().expect("valid name"),
            "patient_advocate".parse().expect("valid name"),
            "privacy_officer".parse().expect("valid name"),
        ],
        allow_break_glass: false,
        jurisdiction_tag: "us_hipaa".to_string(),
        require_consent_evidence: true,
        max_ttl_blocks: NonZeroU32::new(1_440).expect("nonzero"),
        audit_tag: "phi.access.review".to_string(),
    }
}
fn sample_decryption_request() -> DecryptionRequestV1 {
    DecryptionRequestV1 {
        schema_version: DECRYPTION_REQUEST_VERSION_V1,
        request_id: "decrypt-req-0001".to_string(),
        policy_name: "phi_threshold_policy".parse().expect("valid name"),
        binding_name: "patient_records".parse().expect("valid name"),
        state_key: "/state/health/patient-1".to_string(),
        ciphertext_commitment: sample_hash(131),
        justification: "treatment continuity review".to_string(),
        jurisdiction_tag: "us_hipaa".to_string(),
        consent_evidence_hash: Some(sample_hash(133)),
        requested_ttl_blocks: NonZeroU32::new(120).expect("nonzero"),
        break_glass: false,
        break_glass_reason: None,
        governance_tx_hash: sample_hash(132),
    }
}
fn sample_secret_envelope() -> SecretEnvelopeV1 {
    SecretEnvelopeV1 {
        schema_version: SECRET_ENVELOPE_VERSION_V1,
        encryption: SecretEnvelopeEncryptionV1::FheCiphertext,
        key_id: "kms/fhe/team-a".to_string(),
        key_version: NonZeroU32::new(3).expect("nonzero"),
        nonce: vec![1, 2, 3, 4, 5, 6, 7, 8],
        ciphertext: vec![11, 12, 13, 14, 15, 16, 17, 18, 19, 20],
        commitment: sample_hash(91),
        aad_digest: Some(sample_hash(99)),
    }
}
fn sample_ciphertext_state_record() -> CiphertextStateRecordV1 {
    let secret = sample_secret_envelope();
    let payload_bytes = u64::try_from(secret.ciphertext.len()).expect("fits in u64");
    CiphertextStateRecordV1 {
        schema_version: CIPHERTEXT_STATE_RECORD_VERSION_V1,
        binding_name: "private_state".parse().expect("valid name"),
        state_key: "/state/private/patient-1".to_string(),
        metadata: CiphertextStateMetadataV1 {
            content_type: "application/vnd.sora.secret+norito".to_string(),
            payload_bytes: NonZeroU64::new(payload_bytes).expect("nonzero"),
            commitment: secret.commitment,
            policy_tag: Some("health.phi.minimum".to_string()),
            tags: vec!["phi".to_string(), "tenant:alpha".to_string()],
        },
        secret,
    }
}
fn sample_ciphertext_query_spec() -> CiphertextQuerySpecV1 {
    CiphertextQuerySpecV1 {
        schema_version: CIPHERTEXT_QUERY_SPEC_VERSION_V1,
        service_name: "portal".parse().expect("valid name"),
        binding_name: "private_state".parse().expect("valid name"),
        state_key_prefix: "/state/private".to_string(),
        max_results: NonZeroU16::new(16).expect("nonzero"),
        metadata_level: CiphertextQueryMetadataLevelV1::Minimal,
        include_proof: true,
    }
}
fn sample_ciphertext_query_response() -> CiphertextQueryResponseV1 {
    CiphertextQueryResponseV1 {
        schema_version: CIPHERTEXT_QUERY_RESPONSE_VERSION_V1,
        query_hash: sample_hash(141),
        service_name: "portal".parse().expect("valid name"),
        binding_name: "private_state".parse().expect("valid name"),
        metadata_level: CiphertextQueryMetadataLevelV1::Minimal,
        served_sequence: 42,
        result_count: 1,
        truncated: false,
        results: vec![CiphertextQueryResultItemV1 {
            binding_name: "private_state".parse().expect("valid name"),
            state_key: None,
            state_key_digest: sample_hash(142),
            payload_bytes: NonZeroU64::new(2_048).expect("nonzero"),
            ciphertext_commitment: sample_hash(143),
            encryption: SoraStateEncryptionV1::FheCiphertext,
            last_update_sequence: 40,
            governance_tx_hash: sample_hash(144),
            proof: Some(CiphertextInclusionProofV1 {
                schema_version: CIPHERTEXT_QUERY_PROOF_VERSION_V1,
                proof_scheme: "soracloud.audit_anchor.v1".to_string(),
                leaf_hash: sample_hash(145),
                anchor_hash: sample_hash(146),
                anchor_sequence: 42,
                event_sequence: 40,
            }),
        }],
    }
}
fn sample_state_entry() -> SoraServiceStateEntryV1 {
    let payload = vec![0xC1; 2_048];
    let payload_commitment = Hash::new(&payload);
    SoraServiceStateEntryV1 {
        schema_version: SORA_SERVICE_STATE_ENTRY_VERSION_V1,
        service_name: "portal".parse().expect("valid name"),
        service_version: "1.0.0".to_string(),
        binding_name: "private_state".parse().expect("valid name"),
        state_key: "/state/private/patient-1".to_string(),
        encryption: SoraStateEncryptionV1::FheCiphertext,
        payload_bytes: NonZeroU64::new(2_048).expect("nonzero"),
        payload,
        payload_commitment,
        fhe_public_key_digest: None,
        fhe_residual_multiple_bound: None,
        fhe_bound_mode: None,
        last_update_sequence: 12,
        governance_tx_hash: sample_hash(149),
        source_action: SoraServiceLifecycleActionV1::StateMutation,
    }
}
fn sample_decryption_request_record() -> SoraDecryptionRequestRecordV1 {
    SoraDecryptionRequestRecordV1 {
        schema_version: SORA_DECRYPTION_REQUEST_RECORD_VERSION_V1,
        service_name: "portal".parse().expect("valid name"),
        service_version: "1.0.0".to_string(),
        policy: sample_decryption_authority_policy(),
        request: sample_decryption_request(),
        sequence: 18,
        signer: sample_signer(),
    }
}
fn sample_service_deployment_state() -> SoraServiceDeploymentStateV1 {
    SoraServiceDeploymentStateV1 {
        schema_version: SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1,
        service_name: "portal".parse().expect("valid name"),
        current_service_version: "1.1.0".to_string(),
        current_service_manifest_hash: sample_hash(170),
        current_container_manifest_hash: sample_hash(171),
        revision_count: 2,
        process_generation: 2,
        process_started_sequence: 7,
        active_rollout: None,
        last_rollout: None,
        config_generation: 0,
        secret_generation: 0,
        service_configs: BTreeMap::new(),
        service_secrets: BTreeMap::new(),
        fhe_policy_records: BTreeMap::new(),
        service_lease: None,
        lease_volume_states: Vec::new(),
    }
}
fn sample_service_runtime_state() -> SoraServiceRuntimeStateV1 {
    SoraServiceRuntimeStateV1 {
        schema_version: SORA_SERVICE_RUNTIME_STATE_VERSION_V1,
        service_name: "portal".parse().expect("valid name"),
        active_service_version: "2026.1".to_string(),
        health_status: SoraServiceHealthStatusV1::Healthy,
        load_factor_bps: 750,
        materialized_bundle_hash: sample_hash(160),
    }
}
fn sample_service_audit_event() -> SoraServiceAuditEventV1 {
    SoraServiceAuditEventV1 {
        schema_version: SORA_SERVICE_AUDIT_EVENT_VERSION_V1,
        sequence: 1,
        block_height: 1,
        block_timestamp_ms: 1,
        action: SoraServiceLifecycleActionV1::DecryptionRequest,
        service_name: "portal".parse().expect("valid name"),
        from_version: None,
        to_version: "1.0.0".to_string(),
        service_manifest_hash: sample_hash(172),
        container_manifest_hash: sample_hash(173),
        process_generation: 1,
        config_generation: 0,
        secret_generation: 0,
        config_snapshot_hash: derive_soracloud_service_config_snapshot_hash_v1(&BTreeMap::new()),
        secret_snapshot_hash: derive_soracloud_service_secret_snapshot_hash_v1(&BTreeMap::new()),
        governance_tx_hash: Some(sample_hash(176)),
        binding_name: Some("private_state".parse().expect("valid name")),
        state_key: Some("/state/private/patient-1".to_string()),
        config_mutations: Vec::new(),
        secret_mutations: Vec::new(),
        rollout_state: None,
        policy_name: Some("phi_threshold_policy".parse().expect("valid name")),
        policy_snapshot_hash: Some(sample_hash(177)),
        jurisdiction_tag: Some("us_hipaa".to_string()),
        consent_evidence_hash: Some(sample_hash(178)),
        break_glass: Some(true),
        break_glass_reason: Some("emergency review".to_string()),
        lease_usage: None,
        service_lease_commitment: None,
        lease_reporting_epoch_rollover: None,
        signer: sample_signer(),
    }
}
#[test]
fn persisted_service_runtime_identifiers_and_state_paths_are_exact() {
    let audit = sample_service_audit_event();
    audit.validate().expect("canonical service audit event");

    let mut padded_version = audit.clone();
    padded_version.from_version = Some(" 1.0.0".to_owned());
    padded_version
        .validate()
        .expect_err("audit version aliases must fail closed");

    let mut noncanonical_state_path = audit.clone();
    noncanonical_state_path.state_key = Some("/state//patient-1".to_owned());
    noncanonical_state_path
        .validate()
        .expect_err("state paths must use exact canonical components");

    let mut spaced_jurisdiction = audit;
    spaced_jurisdiction.jurisdiction_tag = Some("us hipaa".to_owned());
    spaced_jurisdiction
        .validate()
        .expect_err("jurisdiction tags must be exact tokens");

    let runtime = sample_service_runtime_state();
    runtime.validate().expect("canonical service runtime state");
    let mut padded_active_version = runtime.clone();
    padded_active_version.active_service_version.push(' ');
    padded_active_version
        .validate()
        .expect_err("active service-version aliases must fail closed");
    let mut inrou_runtime = sample_inrou_replica_runtime_state();
    inrou_runtime.service_version = "2026.4 ".to_owned();
    inrou_runtime
        .validate()
        .expect_err("Inrou runtime service-version aliases must fail closed");
}
fn sample_service_mailbox_message() -> SoraServiceMailboxMessageV1 {
    let mut message = SoraServiceMailboxMessageV1 {
        schema_version: SORA_SERVICE_MAILBOX_MESSAGE_VERSION_V1,
        message_id: Hash::prehashed([0; Hash::LENGTH]),
        from_service: "portal".parse().expect("valid name"),
        from_service_version: "2026.1".to_string(),
        from_handler: "update".parse().expect("valid name"),
        to_service: "audit".parse().expect("valid name"),
        to_service_version: "2026.1".to_string(),
        to_handler: "ciphertext_update".parse().expect("valid name"),
        payload_bytes: b"ciphertext".to_vec(),
        payload_commitment: Hash::new(b"ciphertext"),
        delivery_delay_blocks: 0,
        enqueue_sequence: 10,
        enqueue_height: 10,
        available_after_height: 10,
        expires_at_height: 12,
    };
    message.message_id = derive_soracloud_mailbox_message_id_v1(&message);
    message
}
fn sample_runtime_receipt() -> SoraRuntimeReceiptV1 {
    SoraRuntimeReceiptV1 {
        schema_version: SORA_RUNTIME_RECEIPT_VERSION_V1,
        receipt_id: sample_hash(164),
        service_name: "portal".parse().expect("valid name"),
        service_version: "2026.1".to_string(),
        handler_name: "update".parse().expect("valid name"),
        handler_class: SoraServiceHandlerClassV1::Update,
        request_commitment: sample_hash(165),
        result_commitment: sample_hash(166),
        certified_by: SoraCertifiedResponsePolicyV1::None,
        emitted_sequence: 44,
        execution_host: Some(SoraRuntimeDeterministicValidatorHostV1 {
            lane_id: crate::nexus::LaneId::SINGLE,
            validator_account_id: sample_account_id(171),
            peer_id: sample_peer_id(171),
        }),
        mailbox_message_id: Some(sample_hash(163)),
        journal_artifact_hash: Some(sample_hash(168)),
        checkpoint_artifact_hash: Some(sample_hash(169)),
    }
}
#[test]
fn runtime_receipt_requires_an_exact_selected_validator_peer_id() {
    let mut receipt = sample_runtime_receipt();
    receipt
        .validate()
        .expect("canonical runtime receipt host attribution");
    receipt
        .execution_host
        .as_mut()
        .expect("selected validator fixture")
        .peer_id
        .push(' ');
    receipt
        .validate()
        .expect_err("selected validator peer aliases must fail closed");

    let mut padded_service_version = sample_runtime_receipt();
    padded_service_version.service_version.push(' ');
    padded_service_version
        .validate()
        .expect_err("runtime receipt service-version aliases must fail closed");
}
#[cfg(feature = "json")]
#[test]
fn service_world_records_are_closed_and_require_explicit_nullable_keys() {
    macro_rules! assert_closed_and_required_nullable {
        ($value:expr, $ty:ty, [$($field:literal),+ $(,)?], $label:literal) => {{
            let canonical =
                norito::json::to_value(&$value).expect(concat!("serialize canonical ", $label));
            norito::json::from_value::<$ty>(canonical.clone())
                .expect(concat!("canonical ", $label, " must decode"));

            let mut unknown = canonical.clone();
            unknown
                .as_object_mut()
                .expect(concat!($label, " JSON object"))
                .insert("retired_v0".to_owned(), norito::json::Value::from(true));
            let error = norito::json::from_value::<$ty>(unknown)
                .expect_err(concat!($label, " must reject unknown fields"));
            assert!(
                matches!(
                    error,
                    norito::json::Error::UnknownField { ref field } if field == "retired_v0"
                ),
                "{} reported the wrong unknown-field error: {error}",
                $label
            );

            for field in [$($field),+] {
                let mut missing = canonical.clone();
                assert!(
                    missing
                        .as_object_mut()
                        .expect(concat!($label, " JSON object"))
                        .remove(field)
                        .is_some(),
                    "canonical {} must contain `{field}`",
                    $label
                );
                norito::json::from_value::<$ty>(missing)
                    .expect_err(concat!($label, " must reject an omitted nullable key"));

                let mut explicit_null = canonical.clone();
                explicit_null
                    .as_object_mut()
                    .expect(concat!($label, " JSON object"))
                    .insert(field.to_owned(), norito::json::Value::Null);
                norito::json::from_value::<$ty>(explicit_null)
                    .expect(concat!($label, " must accept an explicit null key"));
            }
        }};
    }

    assert_closed_and_required_nullable!(
        sample_service_audit_event(),
        SoraServiceAuditEventV1,
        [
            "from_version",
            "governance_tx_hash",
            "binding_name",
            "state_key",
            "rollout_state",
            "policy_name",
            "policy_snapshot_hash",
            "jurisdiction_tag",
            "consent_evidence_hash",
            "break_glass",
            "break_glass_reason",
            "lease_usage",
            "service_lease_commitment",
            "lease_reporting_epoch_rollover",
        ],
        "service audit event"
    );
    let mut service_runtime_unknown =
        norito::json::to_value(&sample_service_runtime_state()).expect("serialize runtime state");
    service_runtime_unknown
        .as_object_mut()
        .expect("service runtime state JSON object")
        .insert("retired_v0".to_owned(), norito::json::Value::from(true));
    let error = norito::json::from_value::<SoraServiceRuntimeStateV1>(service_runtime_unknown)
        .expect_err("service runtime state must reject unknown fields");
    assert!(
        matches!(
            error,
            norito::json::Error::UnknownField { ref field } if field == "retired_v0"
        ),
        "service runtime state reported the wrong unknown-field error: {error}"
    );
    assert_closed_and_required_nullable!(
        sample_runtime_receipt(),
        SoraRuntimeReceiptV1,
        [
            "execution_host",
            "mailbox_message_id",
            "journal_artifact_hash",
            "checkpoint_artifact_hash",
        ],
        "runtime receipt"
    );
}
fn sample_host_state_mutation_request_envelope() -> SoracloudHostRequestEnvelopeV1 {
    let payload = vec![1, 2, 3, 4];
    SoracloudHostRequestEnvelopeV1 {
        schema_version: SORACLOUD_HOST_REQUEST_VERSION_V1,
        operation: SoracloudHostOperationV1::EmitStateMutation,
        payload: SoracloudHostRequestPayloadV1::EmitStateMutation(
            SoracloudEmitStateMutationRequestV1 {
                binding_name: "private_state".parse().expect("valid name"),
                state_key: "/state/private/patient-1".to_string(),
                operation: SoraStateMutationOperationV1::Upsert,
                encryption: SoraStateEncryptionV1::FheCiphertext,
                payload_bytes: Some(payload.len() as u64),
                payload_commitment: Some(Hash::new(&payload)),
                payload: Some(payload),
            },
        ),
    }
}
fn host_response_envelope(
    operation: SoracloudHostOperationV1,
    payload: SoracloudHostResponsePayloadV1,
) -> SoracloudHostResponseEnvelopeV1 {
    SoracloudHostResponseEnvelopeV1 {
        schema_version: SORACLOUD_HOST_RESPONSE_VERSION_V1,
        operation,
        payload,
    }
}
fn sample_host_config_response_envelope() -> SoracloudHostResponseEnvelopeV1 {
    host_response_envelope(
        SoracloudHostOperationV1::ReadConfig,
        SoracloudHostResponsePayloadV1::ReadConfig(SoracloudReadConfigResponseV1 {
            found: true,
            payload_bytes: br#"{"ok":true}"#.to_vec(),
        }),
    )
}
#[cfg(feature = "json")]
#[test]
fn host_protocol_v1_json_rejects_unknown_fields_across_the_direct_graph() {
    macro_rules! assert_unknown_rejected {
        ($value:expr, $ty:ty, $label:literal) => {{
            let mut value = norito::json::to_value(&$value).expect(concat!("serialize ", $label));
            value
                .as_object_mut()
                .expect(concat!($label, " JSON object"))
                .insert("retired_v0".to_owned(), norito::json!(true));
            let error = norito::json::from_value::<$ty>(value)
                .expect_err(concat!($label, " must reject unknown fields"));
            assert!(
                matches!(
                    error,
                    json::Error::UnknownField { ref field } if field == "retired_v0"
                ),
                "{} reported the wrong error: {error:?}",
                $label
            );
        }};
    }

    let state_mutation_request = SoracloudEmitStateMutationRequestV1 {
        binding_name: "private_state".parse().expect("valid name"),
        state_key: "/state/private/patient-1".to_owned(),
        operation: SoraStateMutationOperationV1::Delete,
        encryption: SoraStateEncryptionV1::FheCiphertext,
        payload_bytes: None,
        payload: None,
        payload_commitment: None,
    };
    let mailbox_request = SoracloudEmitMailboxMessageRequestV1 {
        to_service: "audit".parse().expect("valid name"),
        to_handler: "update".parse().expect("valid name"),
        payload_bytes: Vec::new(),
        delivery_delay_blocks: 1,
    };
    assert_unknown_rejected!(
        SoracloudHostOperationV1::ReadConfig,
        SoracloudHostOperationV1,
        "host operation"
    );
    assert_unknown_rejected!(
        sample_host_state_mutation_request_envelope(),
        SoracloudHostRequestEnvelopeV1,
        "host request envelope"
    );
    assert_unknown_rejected!(
        SoracloudHostRequestPayloadV1::ReadConfig(SoracloudReadConfigRequestV1 {
            config_name: "runtime/theme".to_owned(),
        }),
        SoracloudHostRequestPayloadV1,
        "host request payload"
    );
    assert_unknown_rejected!(
        sample_host_config_response_envelope(),
        SoracloudHostResponseEnvelopeV1,
        "host response envelope"
    );
    assert_unknown_rejected!(
        SoracloudHostResponsePayloadV1::ReadConfig(SoracloudReadConfigResponseV1 {
            found: false,
            payload_bytes: Vec::new(),
        }),
        SoracloudHostResponsePayloadV1,
        "host response payload"
    );
    assert_unknown_rejected!(
        SoracloudReadCommittedStateRequestV1 {
            binding_name: "private_state".parse().expect("valid name"),
            state_key: "/state/private/patient-1".to_owned(),
        },
        SoracloudReadCommittedStateRequestV1,
        "read committed state request"
    );
    assert_unknown_rejected!(
        SoracloudReadCommittedStateResponseV1 { entry: None },
        SoracloudReadCommittedStateResponseV1,
        "read committed state response"
    );
    assert_unknown_rejected!(
        state_mutation_request,
        SoracloudEmitStateMutationRequestV1,
        "emit state mutation request"
    );
    assert_unknown_rejected!(
        SoracloudEmitStateMutationResponseV1 {
            mutation_commitment: sample_hash(211),
        },
        SoracloudEmitStateMutationResponseV1,
        "emit state mutation response"
    );
    assert_unknown_rejected!(
        mailbox_request,
        SoracloudEmitMailboxMessageRequestV1,
        "emit mailbox message request"
    );
    assert_unknown_rejected!(
        SoracloudEmitMailboxMessageResponseV1 {
            message_id: sample_hash(212),
            payload_commitment: sample_hash(213),
        },
        SoracloudEmitMailboxMessageResponseV1,
        "emit mailbox message response"
    );
    assert_unknown_rejected!(
        SoracloudAppendJournalRequestV1 {
            artifact_path: "/journal/1".to_owned(),
            payload_bytes: Vec::new(),
        },
        SoracloudAppendJournalRequestV1,
        "append journal request"
    );
    assert_unknown_rejected!(
        SoracloudAppendJournalResponseV1 {
            artifact_hash: sample_hash(214),
        },
        SoracloudAppendJournalResponseV1,
        "append journal response"
    );
    assert_unknown_rejected!(
        SoracloudPublishCheckpointRequestV1 {
            artifact_path: "/checkpoint/1".to_owned(),
            payload_bytes: Vec::new(),
        },
        SoracloudPublishCheckpointRequestV1,
        "publish checkpoint request"
    );
    assert_unknown_rejected!(
        SoracloudPublishCheckpointResponseV1 {
            artifact_hash: sample_hash(215),
        },
        SoracloudPublishCheckpointResponseV1,
        "publish checkpoint response"
    );
    assert_unknown_rejected!(
        SoracloudReadConfigRequestV1 {
            config_name: "runtime/theme".to_owned(),
        },
        SoracloudReadConfigRequestV1,
        "read config request"
    );
    assert_unknown_rejected!(
        SoracloudReadConfigResponseV1 {
            found: false,
            payload_bytes: Vec::new(),
        },
        SoracloudReadConfigResponseV1,
        "read config response"
    );
    assert_unknown_rejected!(
        SoracloudReadSecretEnvelopeRequestV1 {
            secret_name: "db/password".to_owned(),
        },
        SoracloudReadSecretEnvelopeRequestV1,
        "read secret envelope request"
    );
    assert_unknown_rejected!(
        SoracloudReadSecretEnvelopeResponseV1 { envelope: None },
        SoracloudReadSecretEnvelopeResponseV1,
        "read secret envelope response"
    );
}
#[cfg(feature = "json")]
#[test]
fn host_protocol_v1_json_requires_explicit_null_and_empty_keys() {
    macro_rules! assert_required_fields {
        ($value:expr, $ty:ty, [$($field:literal),+ $(,)?], $label:literal) => {{
            let value = $value;
            let canonical = norito::json::to_value(&value).expect(concat!("serialize ", $label));
            assert_eq!(
                norito::json::from_value::<$ty>(canonical.clone())
                    .expect(concat!("decode canonical ", $label)),
                value
            );
            $(
                assert!(canonical.get($field).is_some(), "canonical {} must emit `{}`", $label, $field);
                let mut missing = canonical.clone();
                assert!(
                    missing
                        .as_object_mut()
                        .expect(concat!($label, " JSON object"))
                        .remove($field)
                        .is_some()
                );
                norito::json::from_value::<$ty>(missing)
                    .expect_err(concat!($label, " must reject omitted V1 fields"));
            )+
        }};
    }
    macro_rules! assert_required_nulls {
        ($value:expr, $ty:ty, [$($field:literal),+ $(,)?], $label:literal) => {{
            let value = $value;
            let canonical = norito::json::to_value(&value).expect(concat!("serialize ", $label));
            $(
                assert!(
                    canonical.get($field).is_some_and(norito::json::Value::is_null),
                    "canonical {} must emit nullable `{}` as null",
                    $label,
                    $field
                );
                let mut missing = canonical.clone();
                assert!(
                    missing
                        .as_object_mut()
                        .expect(concat!($label, " JSON object"))
                        .remove($field)
                        .is_some()
                );
                norito::json::from_value::<$ty>(missing)
                    .expect_err(concat!($label, " must reject omitted nullable V1 fields"));
            )+
            assert_eq!(
                norito::json::from_value::<$ty>(canonical)
                    .expect(concat!("decode explicit-null ", $label)),
                value
            );
        }};
    }

    assert_required_nulls!(
        SoracloudReadCommittedStateResponseV1 { entry: None },
        SoracloudReadCommittedStateResponseV1,
        ["entry"],
        "read committed state response"
    );
    assert_required_nulls!(
        SoracloudEmitStateMutationRequestV1 {
            binding_name: "private_state".parse().expect("valid name"),
            state_key: "/state/private/patient-1".to_owned(),
            operation: SoraStateMutationOperationV1::Delete,
            encryption: SoraStateEncryptionV1::FheCiphertext,
            payload_bytes: None,
            payload: None,
            payload_commitment: None,
        },
        SoracloudEmitStateMutationRequestV1,
        ["payload_bytes", "payload", "payload_commitment"],
        "emit state mutation request"
    );
    assert_required_fields!(
        SoracloudEmitMailboxMessageRequestV1 {
            to_service: "audit".parse().expect("valid name"),
            to_handler: "update".parse().expect("valid name"),
            payload_bytes: Vec::new(),
            delivery_delay_blocks: 1,
        },
        SoracloudEmitMailboxMessageRequestV1,
        ["payload_bytes", "delivery_delay_blocks"],
        "emit mailbox message request"
    );
    assert_required_fields!(
        SoracloudAppendJournalRequestV1 {
            artifact_path: "/journal/1".to_owned(),
            payload_bytes: Vec::new(),
        },
        SoracloudAppendJournalRequestV1,
        ["payload_bytes"],
        "append journal request"
    );
    assert_required_fields!(
        SoracloudPublishCheckpointRequestV1 {
            artifact_path: "/checkpoint/1".to_owned(),
            payload_bytes: Vec::new(),
        },
        SoracloudPublishCheckpointRequestV1,
        ["payload_bytes"],
        "publish checkpoint request"
    );
    assert_required_fields!(
        SoracloudReadConfigResponseV1 {
            found: false,
            payload_bytes: Vec::new(),
        },
        SoracloudReadConfigResponseV1,
        ["payload_bytes"],
        "read config response"
    );
    assert_required_nulls!(
        SoracloudReadSecretEnvelopeResponseV1 { envelope: None },
        SoracloudReadSecretEnvelopeResponseV1,
        ["envelope"],
        "read secret envelope response"
    );
}
#[cfg(feature = "json")]
#[test]
fn signed_fhe_request_model_v1_json_is_closed_and_requires_canonical_keys() {
    macro_rules! assert_closed {
        ($value:expr, $ty:ty, $label:literal) => {{
            let value = $value;
            let canonical =
                norito::json::to_value(&value).expect(concat!("serialize ", $label));
            assert_eq!(
                norito::json::from_value::<$ty>(canonical.clone())
                    .expect(concat!("decode canonical ", $label)),
                value
            );

            let mut unknown = canonical;
            unknown
                .as_object_mut()
                .expect(concat!($label, " JSON object"))
                .insert("retired_v0".to_owned(), norito::json!(true));
            let error = norito::json::from_value::<$ty>(unknown)
                .expect_err(concat!($label, " must reject unknown fields"));
            assert!(
                matches!(
                    error,
                    json::Error::UnknownField { ref field } if field == "retired_v0"
                ),
                "{} reported the wrong unknown-field error: {error:?}",
                $label
            );
        }};
    }
    macro_rules! assert_required_fields {
        ($value:expr, $ty:ty, [$($field:literal),+ $(,)?], $label:literal) => {{
            let value = $value;
            let canonical =
                norito::json::to_value(&value).expect(concat!("serialize ", $label));
            assert_eq!(
                norito::json::from_value::<$ty>(canonical.clone())
                    .expect(concat!("decode canonical ", $label)),
                value
            );
            $(
                assert!(canonical.get($field).is_some(), "canonical {} must emit `{}`", $label, $field);
                let mut missing = canonical.clone();
                assert!(
                    missing
                        .as_object_mut()
                        .expect(concat!($label, " JSON object"))
                        .remove($field)
                        .is_some()
                );
                norito::json::from_value::<$ty>(missing)
                    .expect_err(concat!($label, " must reject omitted V1 fields"));
            )+
        }};
    }
    macro_rules! assert_required_nulls {
        ($value:expr, $ty:ty, [$($field:literal),+ $(,)?], $label:literal) => {{
            let value = $value;
            let canonical =
                norito::json::to_value(&value).expect(concat!("serialize ", $label));
            $(
                assert!(
                    canonical.get($field).is_some_and(norito::json::Value::is_null),
                    "canonical {} must emit nullable `{}` as null",
                    $label,
                    $field
                );
                let mut missing = canonical.clone();
                assert!(
                    missing
                        .as_object_mut()
                        .expect(concat!($label, " JSON object"))
                        .remove($field)
                        .is_some()
                );
                norito::json::from_value::<$ty>(missing)
                    .expect_err(concat!($label, " must reject omitted nullable V1 fields"));
            )+
            assert_eq!(
                norito::json::from_value::<$ty>(canonical)
                    .expect(concat!("decode explicit-null ", $label)),
                value
            );
        }};
    }

    let job = sample_fhe_job_spec();
    assert_closed!(
        FheJobOperationV1::Add,
        FheJobOperationV1,
        "FHE job operation"
    );
    assert_closed!(
        job.inputs[0].clone(),
        FheJobInputRefV1,
        "FHE job input reference"
    );
    assert_closed!(job.clone(), FheJobSpecV1, "FHE job specification");
    assert_required_fields!(job, FheJobSpecV1, ["inputs"], "FHE job specification");

    assert_closed!(
        sample_fhe_policy_reference(),
        SoracloudFhePolicyReferenceV1,
        "FHE policy reference"
    );
    assert_closed!(
        sample_fhe_public_key_proof(),
        SoracloudFhePublicKeyProofV1,
        "FHE public-key proof"
    );
    assert_closed!(
        sample_fhe_bootstrap_key_proof(),
        SoracloudFheBootstrapKeyProofV1,
        "FHE bootstrap-key proof"
    );
    assert_closed!(
        sample_fhe_full_bootstrap_execution_proof(),
        SoracloudFheFullBootstrapExecutionProofV1,
        "FHE full-bootstrap execution proof"
    );

    let input_admission_with_key = sample_fhe_input_admission_proof();
    let public_key = input_admission_with_key
        .public_key
        .clone()
        .expect("sample input admission has a public key");
    assert_closed!(public_key, BfvPublicKey, "BFV public key");
    assert_closed!(
        BfvCiphertextBoundModeV1::ExactResidualMultiple,
        BfvCiphertextBoundModeV1,
        "BFV ciphertext bound mode"
    );

    let mut input_admission = input_admission_with_key;
    input_admission.public_key = None;
    input_admission.ciphertext_proof_statement_digests.clear();
    assert_closed!(
        input_admission.clone(),
        SoracloudFheInputAdmissionProofV1,
        "FHE input admission proof"
    );
    assert_required_nulls!(
        input_admission.clone(),
        SoracloudFheInputAdmissionProofV1,
        ["public_key"],
        "FHE input admission proof"
    );
    assert_required_fields!(
        input_admission,
        SoracloudFheInputAdmissionProofV1,
        ["ciphertext_proof_statement_digests", "bound_mode"],
        "FHE input admission proof"
    );

    let policy = sample_decryption_authority_policy();
    assert_closed!(
        DecryptionAuthorityModeV1::ThresholdService,
        DecryptionAuthorityModeV1,
        "decryption authority mode"
    );
    assert_closed!(
        policy.clone(),
        DecryptionAuthorityPolicyV1,
        "decryption authority policy"
    );
    assert_required_fields!(
        policy,
        DecryptionAuthorityPolicyV1,
        ["approver_ids"],
        "decryption authority policy"
    );

    let mut request = sample_decryption_request();
    request.consent_evidence_hash = None;
    request.break_glass_reason = None;
    assert_closed!(request.clone(), DecryptionRequestV1, "decryption request");
    assert_required_nulls!(
        request,
        DecryptionRequestV1,
        ["consent_evidence_hash", "break_glass_reason"],
        "decryption request"
    );

    assert_closed!(
        CiphertextQueryMetadataLevelV1::Minimal,
        CiphertextQueryMetadataLevelV1,
        "ciphertext query metadata level"
    );
    assert_closed!(
        sample_ciphertext_query_spec(),
        CiphertextQuerySpecV1,
        "ciphertext query specification"
    );
}
#[cfg(feature = "json")]
#[test]
fn ciphertext_query_response_v1_json_is_closed_and_requires_null_and_empty_keys() {
    macro_rules! assert_closed {
        ($value:expr, $ty:ty, $label:literal) => {{
            let value = $value;
            let canonical =
                norito::json::to_value(&value).expect(concat!("serialize ", $label));
            assert_eq!(
                norito::json::from_value::<$ty>(canonical.clone())
                    .expect(concat!("decode canonical ", $label)),
                value
            );
            let mut unknown = canonical;
            unknown
                .as_object_mut()
                .expect(concat!($label, " JSON object"))
                .insert("retired_v0".to_owned(), norito::json!(true));
            let error = norito::json::from_value::<$ty>(unknown)
                .expect_err(concat!($label, " must reject unknown fields"));
            assert!(
                matches!(
                    error,
                    json::Error::UnknownField { ref field } if field == "retired_v0"
                ),
                "{} reported the wrong unknown-field error: {error:?}",
                $label
            );
        }};
    }

    let mut response = sample_ciphertext_query_response();
    response.results[0].proof = None;
    assert_closed!(
        response.results[0].clone(),
        CiphertextQueryResultItemV1,
        "ciphertext query result item"
    );

    let canonical_item = norito::json::to_value(&response.results[0])
        .expect("serialize ciphertext query result item");
    for field in ["state_key", "proof"] {
        assert!(
            canonical_item
                .get(field)
                .is_some_and(norito::json::Value::is_null),
            "canonical nullable `{field}` must be explicit null"
        );
        let mut missing = canonical_item.clone();
        assert!(
            missing
                .as_object_mut()
                .expect("ciphertext query result item JSON object")
                .remove(field)
                .is_some()
        );
        norito::json::from_value::<CiphertextQueryResultItemV1>(missing)
            .expect_err("ciphertext query result item must reject omitted nullable fields");
    }
    assert_eq!(
        norito::json::from_value::<CiphertextQueryResultItemV1>(canonical_item)
            .expect("ciphertext query result item accepts explicit nulls"),
        response.results[0]
    );

    assert_closed!(
        response.clone(),
        CiphertextQueryResponseV1,
        "ciphertext query response"
    );
    let inclusion_proof = sample_ciphertext_query_response().results[0]
        .proof
        .clone()
        .expect("sample response includes a proof");
    assert_closed!(
        inclusion_proof,
        CiphertextInclusionProofV1,
        "ciphertext inclusion proof"
    );

    response.results.clear();
    response.result_count = 0;
    let canonical_response =
        norito::json::to_value(&response).expect("serialize empty ciphertext query response");
    assert_eq!(
        canonical_response
            .get("results")
            .and_then(norito::json::Value::as_array)
            .map(Vec::len),
        Some(0),
        "canonical empty ciphertext query results must be explicit"
    );
    assert_eq!(
        norito::json::from_value::<CiphertextQueryResponseV1>(canonical_response.clone())
            .expect("explicit empty ciphertext query results must decode"),
        response
    );
    let mut missing_results = canonical_response;
    assert!(
        missing_results
            .as_object_mut()
            .expect("ciphertext query response JSON object")
            .remove("results")
            .is_some()
    );
    norito::json::from_value::<CiphertextQueryResponseV1>(missing_results)
        .expect_err("ciphertext query response must reject omitted results");
}
#[test]
fn decryption_request_non_break_glass_reason_must_be_null() {
    let mut request = sample_decryption_request();
    request.break_glass = false;
    request.break_glass_reason = Some("legacy implicit reason".to_owned());
    let error = request
        .validate()
        .expect_err("non-break-glass request must reject a non-null reason");
    assert!(
        matches!(
            error,
            SoracloudManifestError::InvalidField {
                manifest: "decryption request",
                field: "break_glass_reason",
                ref reason,
            } if reason == "must be null when break_glass=false"
        ),
        "unexpected non-break-glass reason error: {error:?}"
    );
}
#[test]
fn state_binding_validate_rejects_plaintext_confidential_scope() {
    let mut binding = sample_binding("private_state");
    binding.scope = SoraStateScopeV1::ConfidentialState;
    binding.encryption = SoraStateEncryptionV1::Plaintext;
    let error = binding
        .validate()
        .expect_err("plaintext confidential state must be rejected");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            field: "encryption",
            ..
        }
    ));
}
#[test]
fn canonical_request_witness_roundtrips_through_norito() {
    let signer = KeyPair::try_random_with_algorithm(Algorithm::Ed25519)
        .expect("generate checked canonical request witness fixture keypair");
    let signature = Signature::try_new(signer.private_key(), b"canonical-request-witness")
        .expect("checked canonical request witness fixture signature");
    signature
        .verify(signer.public_key(), b"canonical-request-witness")
        .expect("checked canonical request witness fixture signature verifies");
    let witness = CanonicalRequestWitnessV1 {
        schema_version: CANONICAL_REQUEST_WITNESS_VERSION_V1,
        subject_account: sample_account_id(9),
        timestamp_ms: 1_717_171_717,
        nonce: "witness-roundtrip".to_owned(),
        canonical_request_hash: sample_hash(61),
        signatures: vec![CanonicalRequestSignatureWitnessV1 {
            signer: signer.public_key().clone(),
            signature,
        }],
    };
    let encoded = norito::to_bytes(&witness).expect("encode witness");
    let decoded: CanonicalRequestWitnessV1 =
        norito::decode_from_bytes(&encoded).expect("decode witness");
    assert_eq!(decoded, witness);
}
#[cfg(feature = "json")]
#[test]
fn canonical_request_witness_v1_json_requires_explicit_signatures_and_closed_fields() {
    let witness = CanonicalRequestWitnessV1 {
        schema_version: CANONICAL_REQUEST_WITNESS_VERSION_V1,
        subject_account: sample_account_id(9),
        timestamp_ms: 1_717_171_717,
        nonce: "witness-json-closure".to_owned(),
        canonical_request_hash: sample_hash(62),
        signatures: Vec::new(),
    };
    let canonical = norito::json::to_value(&witness).expect("serialize canonical request witness");
    assert_eq!(
        canonical
            .get("signatures")
            .and_then(norito::json::Value::as_array)
            .map(Vec::len),
        Some(0),
        "canonical empty signature list must be explicit"
    );
    assert_eq!(
        norito::json::from_value::<CanonicalRequestWitnessV1>(canonical.clone())
            .expect("explicit empty signature list must decode"),
        witness
    );

    let mut missing_signatures = canonical.clone();
    assert!(
        missing_signatures
            .as_object_mut()
            .expect("canonical request witness JSON object")
            .remove("signatures")
            .is_some()
    );
    norito::json::from_value::<CanonicalRequestWitnessV1>(missing_signatures)
        .expect_err("omitted canonical request signatures must be rejected");

    let mut unknown_witness = canonical;
    unknown_witness
        .as_object_mut()
        .expect("canonical request witness JSON object")
        .insert("retired_v0".to_owned(), norito::json!(true));
    let error = norito::json::from_value::<CanonicalRequestWitnessV1>(unknown_witness)
        .expect_err("canonical request witness must reject unknown fields");
    assert!(
        matches!(
            error,
            json::Error::UnknownField { ref field } if field == "retired_v0"
        ),
        "unexpected canonical request witness unknown-field rejection: {error}"
    );

    let signer = KeyPair::try_random_with_algorithm(Algorithm::Ed25519)
        .expect("generate canonical request signature witness fixture keypair");
    let signature = Signature::try_new(signer.private_key(), b"canonical-request-signature-json")
        .expect("create canonical request signature witness fixture");
    let mut signature_json = norito::json::to_value(&CanonicalRequestSignatureWitnessV1 {
        signer: signer.public_key().clone(),
        signature,
    })
    .expect("serialize canonical request signature witness");
    signature_json
        .as_object_mut()
        .expect("canonical request signature witness JSON object")
        .insert("retired_v0".to_owned(), norito::json!(true));
    let error = norito::json::from_value::<CanonicalRequestSignatureWitnessV1>(signature_json)
        .expect_err("canonical request signature witness must reject unknown fields");
    assert!(
        matches!(
            error,
            json::Error::UnknownField { ref field } if field == "retired_v0"
        ),
        "unexpected canonical request signature unknown-field rejection: {error}"
    );
}
#[test]
fn host_request_envelope_validation_accepts_consistent_payload() {
    let envelope = sample_host_state_mutation_request_envelope();
    assert!(
        envelope.validate().is_ok(),
        "valid host request envelope must pass"
    );
}
#[test]
fn host_request_envelope_validation_rejects_payload_operation_mismatch() {
    let mut envelope = sample_host_state_mutation_request_envelope();
    envelope.operation = SoracloudHostOperationV1::ReadConfig;
    let error = envelope
        .validate()
        .expect_err("host request operation must match payload variant");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            field: "operation",
            ..
        }
    ));
}
#[test]
fn host_request_envelope_validation_rejects_zero_prehash_digest_sentinels() {
    let zero_digest = zero_prehash_statement_hash();
    let mut state_mutation = sample_host_state_mutation_request_envelope();
    let SoracloudHostRequestPayloadV1::EmitStateMutation(request) = &mut state_mutation.payload
    else {
        panic!("sample request uses state mutation payload");
    };
    request.payload_commitment = Some(zero_digest);
    let error = state_mutation
        .validate()
        .expect_err("state mutation payload placeholder commitment must fail admission");
    assert_zero_prehash_digest_error(&error, "payload_commitment");
}
#[test]
fn host_request_envelope_validation_rejects_adversarial_state_payload_metadata() {
    let mut wrong_length = sample_host_state_mutation_request_envelope();
    let SoracloudHostRequestPayloadV1::EmitStateMutation(request) = &mut wrong_length.payload
    else {
        panic!("sample request uses state mutation payload");
    };
    request.payload_bytes = Some(99);
    let error = wrong_length
        .validate()
        .expect_err("payload length must bind to the actual payload");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            field: "payload_bytes",
            ..
        }
    ));
    let mut wrong_commitment = sample_host_state_mutation_request_envelope();
    let SoracloudHostRequestPayloadV1::EmitStateMutation(request) = &mut wrong_commitment.payload
    else {
        panic!("sample request uses state mutation payload");
    };
    request.payload_commitment = Some(sample_hash(230));
    let error = wrong_commitment
        .validate()
        .expect_err("payload commitment must bind to the actual payload");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            field: "payload_commitment",
            ..
        }
    ));
    let mut delete_with_payload = sample_host_state_mutation_request_envelope();
    let SoracloudHostRequestPayloadV1::EmitStateMutation(request) =
        &mut delete_with_payload.payload
    else {
        panic!("sample request uses state mutation payload");
    };
    request.operation = SoraStateMutationOperationV1::Delete;
    let error = delete_with_payload
        .validate()
        .expect_err("delete mutation must not smuggle payload material");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            field: "payload",
            ..
        }
    ));
}
#[test]
fn host_response_envelope_validation_accepts_consistent_payload() {
    let envelope = sample_host_config_response_envelope();
    assert!(
        envelope.validate().is_ok(),
        "valid host response envelope must pass"
    );
}
#[test]
fn host_response_envelope_validation_rejects_payload_operation_mismatch() {
    let mut envelope = sample_host_config_response_envelope();
    envelope.operation = SoracloudHostOperationV1::ReadCommittedState;
    let error = envelope
        .validate()
        .expect_err("host response operation must match payload variant");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            field: "operation",
            ..
        }
    ));
}
#[test]
fn host_response_envelope_validation_rejects_zero_prehash_digest_sentinels() {
    let zero_digest = zero_prehash_statement_hash();
    macro_rules! assert_response_digest_rejects {
        ($field:literal, $payload:expr, $operation:expr) => {{
            let envelope = host_response_envelope($operation, $payload);
            let error = envelope
                .validate()
                .expect_err("host response placeholder digest must fail admission");
            assert_zero_prehash_digest_error(&error, $field);
        }};
    }
    assert_response_digest_rejects!(
        "mutation_commitment",
        SoracloudHostResponsePayloadV1::EmitStateMutation(SoracloudEmitStateMutationResponseV1 {
            mutation_commitment: zero_digest,
        },),
        SoracloudHostOperationV1::EmitStateMutation
    );
    assert_response_digest_rejects!(
        "message_id",
        SoracloudHostResponsePayloadV1::EmitMailboxMessage(SoracloudEmitMailboxMessageResponseV1 {
            message_id: zero_digest,
            payload_commitment: sample_hash(231),
        },),
        SoracloudHostOperationV1::EmitMailboxMessage
    );
    assert_response_digest_rejects!(
        "payload_commitment",
        SoracloudHostResponsePayloadV1::EmitMailboxMessage(SoracloudEmitMailboxMessageResponseV1 {
            message_id: sample_hash(232),
            payload_commitment: zero_digest,
        },),
        SoracloudHostOperationV1::EmitMailboxMessage
    );
    assert_response_digest_rejects!(
        "artifact_hash",
        SoracloudHostResponsePayloadV1::AppendJournal(SoracloudAppendJournalResponseV1 {
            artifact_hash: zero_digest,
        }),
        SoracloudHostOperationV1::AppendJournal
    );
    assert_response_digest_rejects!(
        "artifact_hash",
        SoracloudHostResponsePayloadV1::PublishCheckpoint(SoracloudPublishCheckpointResponseV1 {
            artifact_hash: zero_digest,
        },),
        SoracloudHostOperationV1::PublishCheckpoint
    );
}
#[test]
fn container_validate_rejects_invalid_healthcheck_path() {
    let mut container = sample_container();
    container.lifecycle.healthcheck_path = Some("healthz".to_string());
    let error = container
        .validate()
        .expect_err("healthcheck path must start with slash");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            field: "lifecycle.healthcheck_path",
            ..
        }
    ));
}
#[test]
fn container_validate_rejects_zero_prehash_bundle_hash_sentinel() {
    let mut container = sample_container();
    container.bundle_hash = zero_prehash_statement_hash();
    let error = container
        .validate()
        .expect_err("container placeholder bundle hash must fail admission");
    assert_zero_prehash_digest_error(&error, "bundle_hash");
}
#[test]
fn container_validate_accepts_canonical_environment_variable_names() {
    let mut container = sample_container();
    container.env = BTreeMap::from([
        ("A".to_string(), "single-letter".to_string()),
        ("APP_ENV_2".to_string(), "alphanumeric".to_string()),
        ("_".to_string(), "underscore".to_string()),
        ("_PRIVATE".to_string(), "prefixed".to_string()),
    ]);
    assert!(
        container.validate().is_ok(),
        "canonical POSIX environment-variable names should validate"
    );
}
#[test]
fn container_validate_rejects_noncanonical_environment_variable_names() {
    for invalid_name in [
        "",
        "1APP_ENV",
        "APP-ENV",
        "APP.ENV",
        "APP ENV",
        "APP=ENV",
        "APP;touch /tmp/injected",
        " APP_ENV",
        "APP_ENV ",
        "ÉNV",
    ] {
        let mut container = sample_container();
        container.env = BTreeMap::from([(invalid_name.to_string(), "value".to_string())]);
        let error = container
            .validate()
            .expect_err("noncanonical environment-variable name must fail admission");
        assert!(
            matches!(
                &error,
                SoracloudManifestError::EmptyField { field: "env", .. }
                    | SoracloudManifestError::InvalidField { field: "env", .. }
            ),
            "unexpected error for environment-variable name {invalid_name:?}: {error}"
        );
    }
}
#[test]
fn container_validate_accepts_inrou_runtime() {
    let mut container = sample_container();
    container.runtime = SoraContainerRuntimeV1::Inrou;
    container.entrypoint = "/app/bin/service".to_string();
    container.inrou = Some(sample_inrou_manifest());
    container.capabilities.network = SoraNetworkPolicyV1::Isolated;
    assert!(
        container.validate().is_ok(),
        "Inrou Soracloud manifests should be admitted by the data model"
    );
}
#[test]
fn container_validate_rejects_nonportable_inrou_bundle_paths() {
    for invalid_entrypoint in [
        "app/bin/service",
        "/",
        "//app/bin/service",
        "/app/bin/service/",
        "/app/../service",
        "/app/./service",
        "/app/servicé",
        "/app/CON",
        "/app/service:stream",
        "/app/service name",
        "/app/service!",
        "/app/service.",
    ] {
        let mut container = sample_container();
        container.runtime = SoraContainerRuntimeV1::Inrou;
        container.entrypoint = invalid_entrypoint.to_string();
        container.inrou = Some(sample_inrou_manifest());
        let error = container
            .validate()
            .expect_err("nonportable Inrou entrypoint must fail admission");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "entrypoint",
                ..
            }
        ));
    }
    let mut container = sample_container();
    container.runtime = SoraContainerRuntimeV1::Inrou;
    container.entrypoint = format!("/{}", "a".repeat(256));
    container.inrou = Some(sample_inrou_manifest());
    let error = container
        .validate()
        .expect_err("an Inrou entrypoint beyond the USTAR path bound must fail admission");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            field: "entrypoint",
            ..
        }
    ));
}
