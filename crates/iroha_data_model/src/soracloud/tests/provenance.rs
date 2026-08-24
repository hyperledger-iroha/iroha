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
        service_name: sample_name("private_model_host"),
        model_id: "bundle-1".to_string(),
        weight_version: "v1".to_string(),
        family: "demo-family".to_string(),
        modalities: vec!["text".to_string()],
        plaintext_root: sample_hash(30),
        runtime_format: SoraUploadedModelRuntimeFormatV1::HuggingFaceSafetensors,
        bundle_root: sample_hash(31),
        sorafs_manifest_digest: ManifestDigest::new([0xA5; 32]),
        chunk_count: 2,
        plaintext_bytes: 2_048,
        ciphertext_bytes: 1_024,
        chunk_manifest_root: sample_hash(33),
        upload_recipient: sample_uploaded_model_encryption_recipient(),
        wrapped_bundle_key: sample_uploaded_model_wrapped_key(),
        pricing_policy: SoraUploadedModelPricingPolicyV1 {
            storage_price: xor_quantity_from_nanos(10),
        },
        decryption_policy_ref: "policy/v1".to_string(),
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
        "demo_model",
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
        "demo_model",
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
        "demo_model",
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
        "demo_model",
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
fn hf_resource_profile_reports_expected_size_bucket() {
    assert_eq!(
        sample_hf_resource_profile().size_bucket(),
        SoraHfModelSizeBucketV1::Medium
    );
}
#[test]
fn hf_source_record_validates_resource_profile() {
    sample_hf_source_record()
        .validate()
        .expect("valid source record");
}
#[test]
fn model_host_capability_record_validates() {
    sample_model_host_capability_record()
        .validate()
        .expect("valid host capability record");
}
#[test]
fn hf_placement_record_validates_and_counts_warm_hosts() {
    let placement = sample_hf_placement_record();
    placement.validate().expect("valid placement record");
    assert_eq!(placement.warm_host_count(), 1);
}
#[test]
fn hf_placement_record_validate_rejects_zero_prehash_digest_sentinels() {
    let zero_digest = zero_prehash_statement_hash();
    macro_rules! assert_placement_digest_rejects {
        ($field:literal, $assign:expr) => {{
            let mut placement = sample_hf_placement_record();
            $assign(&mut placement, zero_digest);
            let error = placement
                .validate()
                .expect_err("placement placeholder digest must fail admission");
            assert_zero_prehash_digest_error(&error, $field);
        }};
    }
    assert_placement_digest_rejects!(
        "placement_id",
        |record: &mut SoraHfPlacementRecordV1, value| {
            record.placement_id = value;
        }
    );
    assert_placement_digest_rejects!(
        "source_id",
        |record: &mut SoraHfPlacementRecordV1, value| {
            record.source_id = value;
        }
    );
    assert_placement_digest_rejects!("pool_id", |record: &mut SoraHfPlacementRecordV1, value| {
        record.pool_id = value;
    });
    assert_placement_digest_rejects!(
        "selection_seed_hash",
        |record: &mut SoraHfPlacementRecordV1, value| {
            record.selection_seed_hash = value;
        }
    );
}
#[test]
fn model_host_advertise_provenance_payload_encodes_canonical_layout() {
    let capability = sample_model_host_capability_record();
    let encoded =
        encode_model_host_advertise_provenance_payload(&capability).expect("encode payload");
    let expected = norito::to_bytes(&capability).expect("encode capability");
    assert_eq!(encoded, expected);
}
#[test]
fn model_host_heartbeat_provenance_payload_encodes_purpose_bound_preimage() {
    let validator_account_id = sample_account_id(0xC3);
    let encoded = encode_model_host_heartbeat_provenance_payload(&validator_account_id, 160_000)
        .expect("encode payload");
    let semantic_payload =
        norito::encode_canonical(&(validator_account_id, 160_000u64)).expect("encode tuple");
    let expected = norito::encode_canonical(&(
        SORACLOUD_RUNTIME_PROVENANCE_DOMAIN_V1.to_vec(),
        SORACLOUD_RUNTIME_PROVENANCE_PREIMAGE_VERSION_V1,
        SoracloudRuntimeProvenancePurposeV1::ModelHostHeartbeat.wire_id(),
        semantic_payload,
    ))
    .expect("encode expected provenance preimage");
    assert_eq!(encoded, expected);
    validate_soracloud_runtime_provenance_preimage_v1(
        SoracloudRuntimeProvenancePurposeV1::ModelHostHeartbeat,
        &encoded,
    )
    .expect("heartbeat purpose must validate");
}
#[test]
fn model_host_withdraw_provenance_payload_encodes_account_id() {
    let validator_account_id = sample_account_id(0xC3);
    let encoded = encode_model_host_withdraw_provenance_payload(&validator_account_id)
        .expect("encode payload");
    let expected = norito::to_bytes(&validator_account_id).expect("encode account id");
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
    let heartbeat_preimage = encode_soracloud_runtime_provenance_preimage_v1(
        SoracloudRuntimeProvenancePurposeV1::ModelHostHeartbeat,
        &canonical_payload,
    )
    .expect("encode heartbeat preimage");
    let inrou_preimage = encode_soracloud_runtime_provenance_preimage_v1(
        SoracloudRuntimeProvenancePurposeV1::InrouHostAdvert,
        &canonical_payload,
    )
    .expect("encode Inrou preimage");
    assert_ne!(heartbeat_preimage, inrou_preimage);
    assert_eq!(
        validate_soracloud_runtime_provenance_preimage_v1(
            SoracloudRuntimeProvenancePurposeV1::InrouHostAdvert,
            &heartbeat_preimage,
        ),
        Err(SoracloudRuntimeProvenancePreimageErrorV1::PurposeMismatch)
    );
    let signer = sample_ed25519_keypair(0x9A);
    let signature = Signature::try_new(signer.private_key(), &heartbeat_preimage)
        .expect("sign heartbeat preimage");
    signature
        .verify(signer.public_key(), &heartbeat_preimage)
        .expect("same-purpose signature must verify");
    assert!(
        signature
            .verify(signer.public_key(), &inrou_preimage)
            .is_err(),
        "a heartbeat signature must not verify as an Inrou advert"
    );
}
#[test]
fn runtime_provenance_preimage_validator_rejects_non_v1_framing() {
    let expected_purpose = SoracloudRuntimeProvenancePurposeV1::ModelHostHeartbeat;
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
        SoracloudRuntimeProvenancePurposeV1::try_from_wire_id(3),
        Ok(SoracloudRuntimeProvenancePurposeV1::InrouHostWithdraw)
    );
    for unknown in [0, 4, u8::MAX] {
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
            allow_wallet_signing: true,
            allow_state_writes: true,
            allow_model_inference: true,
            allow_model_training: false,
        },
        resources: SoraResourceLimitsV1 {
            cpu_millis: NonZeroU32::new(750).expect("nonzero"),
            memory_bytes: NonZeroU64::new(536_870_912).expect("nonzero"),
            ephemeral_storage_bytes: NonZeroU64::new(2_147_483_648).expect("nonzero"),
            max_open_files: NonZeroU32::new(512).expect("nonzero"),
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
        guest_os: SoraInrouGuestOsV1::DebianSlim,
        guest_images: BTreeMap::from([
            (
                SoraInrouGuestIsaV1::X8664,
                SoraInrouGuestImageV1 {
                    kernel_image_path: "/inrou/x86_64/vmlinux".to_string(),
                    rootfs_image_path: "/inrou/x86_64/rootfs.ext4".to_string(),
                    initrd_image_path: None,
                    distribution: SoraArtifactDistributionPolicyV1::default(),
                    published_artifact: None,
                },
            ),
            (
                SoraInrouGuestIsaV1::Aarch64,
                SoraInrouGuestImageV1 {
                    kernel_image_path: "/inrou/aarch64/vmlinux".to_string(),
                    rootfs_image_path: "/inrou/aarch64/rootfs.ext4".to_string(),
                    initrd_image_path: None,
                    distribution: SoraArtifactDistributionPolicyV1::default(),
                    published_artifact: None,
                },
            ),
        ]),
        bootstrap_user_data_path: None,
        ssh_authorized_keys: vec!["ssh-ed25519 test-key soracloud-tests".to_string()],
    }
}
fn sample_published_inrou_guest_image_artifact(seed: u8) -> SoraPublishedInrouGuestImageArtifactV1 {
    let manifest_digest_hex = hex::encode([seed; 32]);
    SoraPublishedInrouGuestImageArtifactV1 {
        manifest_digest_hex: manifest_digest_hex.clone(),
        content_cid: encode_lowercase_multibase_base32(
            &sorafs_manifest::canonical_manifest_root_cid([seed; 32]),
        ),
        manifest_id_hex: Some(manifest_digest_hex),
        distribution: SoraArtifactDistributionPolicyV1::default(),
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
            mount_path: "/var/lib/ton-indexer".to_string(),
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
            retention_sequences: NonZeroU32::new(1_440).expect("nonzero"),
        }),
    }
}
fn sample_private_update_handler() -> SoraServiceHandlerV1 {
    SoraServiceHandlerV1 {
        handler_name: "private_update".parse().expect("valid name"),
        class: SoraServiceHandlerClassV1::PrivateUpdate,
        entrypoint: "apply_private_update".to_string(),
        route_path: Some("/private/update".to_string()),
        certified_response: SoraCertifiedResponsePolicyV1::None,
        mailbox: Some(SoraMailboxContractV1 {
            queue_name: "private_updates".parse().expect("valid name"),
            max_pending_messages: NonZeroU32::new(256).expect("nonzero"),
            max_message_bytes: NonZeroU64::new(131_072).expect("nonzero"),
            retention_sequences: NonZeroU32::new(2_880).expect("nonzero"),
        }),
    }
}
fn sample_handlers() -> Vec<SoraServiceHandlerV1> {
    vec![
        sample_asset_handler(),
        sample_query_handler(),
        sample_update_handler(),
        sample_private_update_handler(),
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
            handler_name: Some("private_update".parse().expect("valid name")),
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
        status: SoraAgentRuntimeStatusV1::Running,
        deployed_sequence: 10,
        lease_started_sequence: 10,
        lease_expires_sequence: 110,
        last_renewed_sequence: 10,
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
        action: SoraAgentApartmentActionV1::Restart,
        apartment_name: sample_name("ops_agent"),
        status: SoraAgentRuntimeStatusV1::Running,
        lease_expires_sequence: 140,
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
        deprecation_height: Some(20_000),
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
        rollout_handle: Some("rollout-1".to_string()),
        pending_mailbox_message_count: 2,
        last_receipt_id: Some(sample_hash(161)),
    }
}
fn sample_service_audit_event() -> SoraServiceAuditEventV1 {
    SoraServiceAuditEventV1 {
        schema_version: SORA_SERVICE_AUDIT_EVENT_VERSION_V1,
        sequence: 1,
        action: SoraServiceLifecycleActionV1::DecryptionRequest,
        service_name: "portal".parse().expect("valid name"),
        from_version: None,
        to_version: "1.0.0".to_string(),
        service_manifest_hash: sample_hash(172),
        container_manifest_hash: sample_hash(173),
        governance_tx_hash: Some(sample_hash(176)),
        binding_name: Some("private_state".parse().expect("valid name")),
        state_key: Some("/state/private/patient-1".to_string()),
        config_name: None,
        secret_name: None,
        rollout_handle: None,
        policy_name: Some("phi_threshold_policy".parse().expect("valid name")),
        policy_snapshot_hash: Some(sample_hash(177)),
        jurisdiction_tag: Some("us_hipaa".to_string()),
        consent_evidence_hash: Some(sample_hash(178)),
        break_glass: Some(true),
        break_glass_reason: Some("emergency review".to_string()),
        signer: sample_signer(),
    }
}
fn sample_service_mailbox_message() -> SoraServiceMailboxMessageV1 {
    SoraServiceMailboxMessageV1 {
        schema_version: SORA_SERVICE_MAILBOX_MESSAGE_VERSION_V1,
        message_id: sample_hash(162),
        from_service: "portal".parse().expect("valid name"),
        from_service_version: "2026.1".to_string(),
        from_handler: "update".parse().expect("valid name"),
        to_service: "audit".parse().expect("valid name"),
        to_service_version: "2026.1".to_string(),
        to_handler: "private_update".parse().expect("valid name"),
        payload_bytes: b"ciphertext".to_vec(),
        payload_commitment: Hash::new(b"ciphertext"),
        delivery_delay_sequences: 0,
        enqueue_sequence: 10,
        available_after_sequence: 10,
        expires_at_sequence: 12,
    }
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
        execution_host: Some(SoraRuntimeExecutionHostV1::HfModelHost(
            SoraRuntimeHfModelHostV1 {
            placement_id: sample_hash(170),
            validator_account_id: sample_account_id(171),
            peer_id: "12D3KooWRuntimePrimary".to_string(),
            },
        )),
        mailbox_message_id: Some(sample_hash(163)),
        journal_artifact_hash: Some(sample_hash(168)),
        checkpoint_artifact_hash: Some(sample_hash(169)),
    }
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
fn host_request_envelope(
    operation: SoracloudHostOperationV1,
    payload: SoracloudHostRequestPayloadV1,
) -> SoracloudHostRequestEnvelopeV1 {
    SoracloudHostRequestEnvelopeV1 {
        schema_version: SORACLOUD_HOST_REQUEST_VERSION_V1,
        operation,
        payload,
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
fn sample_host_egress_response_envelope() -> SoracloudHostResponseEnvelopeV1 {
    let body = br#"{"ok":true}"#.to_vec();
    host_response_envelope(
        SoracloudHostOperationV1::EgressFetch,
        SoracloudHostResponsePayloadV1::EgressFetch(SoracloudEgressFetchResponseV1 {
            status_code: 200,
            content_type: Some("application/json".to_string()),
            body_hash: Hash::new(&body),
            body,
        }),
    )
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
    let egress = host_request_envelope(
        SoracloudHostOperationV1::EgressFetch,
        SoracloudHostRequestPayloadV1::EgressFetch(SoracloudEgressFetchRequestV1 {
            url: "https://oracle.example/data.json".to_string(),
            max_bytes: 4096,
            expected_hash: Some(zero_digest),
        }),
    );
    let error = egress
        .validate()
        .expect_err("egress expected-hash placeholder must fail admission");
    assert_zero_prehash_digest_error(&error, "expected_hash");
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
    let envelope = sample_host_egress_response_envelope();
    assert!(
        envelope.validate().is_ok(),
        "valid host response envelope must pass"
    );
}
#[test]
fn host_response_envelope_validation_rejects_payload_operation_mismatch() {
    let mut envelope = sample_host_egress_response_envelope();
    envelope.operation = SoracloudHostOperationV1::ReadConfig;
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
    assert_response_digest_rejects!(
        "body_hash",
        SoracloudHostResponsePayloadV1::EgressFetch(SoracloudEgressFetchResponseV1 {
            status_code: 200,
            content_type: None,
            body: b"ok".to_vec(),
            body_hash: zero_digest,
        }),
        SoracloudHostOperationV1::EgressFetch
    );
}
#[test]
fn host_response_envelope_validation_rejects_adversarial_egress_metadata() {
    let mut wrong_body_hash = sample_host_egress_response_envelope();
    let SoracloudHostResponsePayloadV1::EgressFetch(response) = &mut wrong_body_hash.payload else {
        panic!("sample response uses egress payload");
    };
    response.body_hash = sample_hash(233);
    let error = wrong_body_hash
        .validate()
        .expect_err("egress body hash must bind to the actual response body");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            field: "body_hash",
            ..
        }
    ));
    let mut empty_content_type = sample_host_egress_response_envelope();
    let SoracloudHostResponsePayloadV1::EgressFetch(response) = &mut empty_content_type.payload
    else {
        panic!("sample response uses egress payload");
    };
    response.content_type = Some(String::new());
    let error = empty_content_type
        .validate()
        .expect_err("egress content type must not be empty");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            field: "content_type",
            ..
        }
    ));
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
    let mut container = sample_container();
    container.runtime = SoraContainerRuntimeV1::Inrou;
    container.entrypoint = "/app/bin/service".to_string();
    let mut inrou = sample_inrou_manifest();
    inrou.bootstrap_user_data_path = Some("/cloud//user-data".to_string());
    container.inrou = Some(inrou);
    let error = container
        .validate()
        .expect_err("nonportable bootstrap member path must fail admission");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            field: "bootstrap_user_data_path",
            ..
        }
    ));
}
