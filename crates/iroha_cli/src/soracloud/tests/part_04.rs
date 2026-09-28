macro_rules! manifest_pair_submission_service_name_case {
    (
        $name:ident,
        temp: $temp:literal,
        endpoint: $endpoint:literal,
        response_label: $response_label:literal,
        seed: $seed:literal,
        args: $args:ident { $($field:ident: $value:expr),* $(,)? },
        success: $success:literal,
        request_label: $request_label:literal,
        decode_label: $decode_label:literal $(,)?
    ) => {
        #[test]
        fn $name() {
            let dir = temp_dir($temp);
            InitArgs {
                output_dir: dir.clone(),
                service_name: "echo_console".to_owned(),
                service_version: "1.0.0".to_owned(),
                template: InitTemplate::HttpService,
                overwrite: false,
            }
            .run()
            .expect("http-service init should succeed");
            let key_pair = soracloud_fixture_key_pair($seed);
            let authority = AccountId::new(key_pair.public_key().clone());
            let response = mock_soracloud_draft_response(&authority, &key_pair);
            let server = MockHttpServer::start(BTreeMap::from([(
                $endpoint.to_owned(),
                MockHttpResponse {
                    status: "200 OK",
                    content_type: "application/json",
                    body: json::to_vec(&response).expect($response_label),
                },
            )]));
            install_mock_submission_config(&authority, &key_pair);
            let output = $args {
                service_name: None,
                container: Some(dir.join("container_manifest.json")),
                service: Some(dir.join("service_manifest.json")),
                $($field: $value,)*
                torii_url: Some(server.base_url.clone()),
                api_token: None,
                timeout_secs: 5,
            }
            .run(&authority, &key_pair)
            .expect($success);
            assert_manifest_pair_service_plan(&output);
            assert_captured_payload_service_name(
                &server,
                $endpoint,
                $request_label,
                $decode_label,
            );
        }
    };
}

macro_rules! manifest_pair_status_service_name_case {
    (
        $name:ident,
        temp: $temp:literal,
        endpoint: $endpoint:literal,
        response: $response:expr,
        response_label: $response_label:literal,
        args: $args:ident { $($field:ident: $value:expr),* $(,)? },
        success: $success:literal,
        request_label: $request_label:literal $(,)?
    ) => {
        #[test]
        fn $name() {
            let dir = temp_dir($temp);
            InitArgs {
                output_dir: dir.clone(),
                service_name: "echo_console".to_owned(),
                service_version: "1.0.0".to_owned(),
                template: InitTemplate::HttpService,
                overwrite: false,
            }
            .run()
            .expect("http-service init should succeed");
            let response = $response;
            let server = MockHttpServer::start(BTreeMap::from([(
                $endpoint.to_owned(),
                MockHttpResponse {
                    status: "200 OK",
                    content_type: "application/json",
                    body: json::to_vec(&response).expect($response_label),
                },
            )]));
            install_mock_protected_read_signer();
            let output = $args {
                service_name: None,
                container: Some(dir.join("container_manifest.json")),
                service: Some(dir.join("service_manifest.json")),
                $($field: $value,)*
                torii_url: Some(server.base_url.clone()),
                api_token: None,
                timeout_secs: 5,
            }
            .run()
            .expect($success);
            assert_manifest_pair_service_plan(&output);
            let request = server
                .requests()
                .into_iter()
                .find(|request| request.method == "GET")
                .expect($request_label);
            assert_eq!(request.path, $endpoint);
        }
    };
}

manifest_pair_submission_service_name_case!(
    training_job_start_args_can_resolve_service_name_from_manifest_pair,
    temp: "training_job_start_service_name_from_manifest_pair",
    endpoint: "/v1/soracloud/training/job/start",
    response_label: "encode training start response",
    seed: 0x18,
    args: TrainingJobStartArgs {
        model_name: "fare-model".to_owned(),
        job_id: "job-1".to_owned(),
        worker_group_size: 1,
        target_steps: 100,
        checkpoint_interval_steps: 10,
        max_retries: 3,
        step_compute_units: 10,
        compute_budget_units: 1_000,
        storage_budget_bytes: 1_024,
    },
    success: "training-job-start should succeed",
    request_label: "training job start request",
    decode_label: "decode training job start request",
);
manifest_pair_submission_service_name_case!(
    training_job_checkpoint_args_can_resolve_service_name_from_manifest_pair,
    temp: "training_job_checkpoint_service_name_from_manifest_pair",
    endpoint: "/v1/soracloud/training/job/checkpoint",
    response_label: "encode training checkpoint response",
    seed: 0x19,
    args: TrainingJobCheckpointArgs {
        job_id: "job-1".to_owned(),
        completed_step: 10,
        checkpoint_size_bytes: 1_024,
        metrics_hash: Hash::new(b"metrics"),
    },
    success: "training-job-checkpoint should succeed",
    request_label: "training job checkpoint request",
    decode_label: "decode training job checkpoint request",
);
manifest_pair_submission_service_name_case!(
    training_job_retry_args_can_resolve_service_name_from_manifest_pair,
    temp: "training_job_retry_service_name_from_manifest_pair",
    endpoint: "/v1/soracloud/training/job/retry",
    response_label: "encode training retry response",
    seed: 0x1A,
    args: TrainingJobRetryArgs {
        job_id: "job-1".to_owned(),
        reason: "transient failure".to_owned(),
    },
    success: "training-job-retry should succeed",
    request_label: "training job retry request",
    decode_label: "decode training job retry request",
);
manifest_pair_status_service_name_case!(
    training_job_status_args_can_resolve_service_name_from_manifest_pair,
    temp: "training_job_status_service_name_from_manifest_pair",
    endpoint: "/v1/soracloud/training/job/status?service_name=echo_console&job_id=job-1",
    response: training_job_status_fixture("echo_console", "job-1"),
    response_label: "encode training status response",
    args: TrainingJobStatusArgs {
        job_id: "job-1".to_owned(),
    },
    success: "training-job-status should succeed",
    request_label: "training job status request",
);
manifest_pair_submission_service_name_case!(
    model_artifact_register_args_can_resolve_service_name_from_manifest_pair,
    temp: "model_artifact_register_service_name_from_manifest_pair",
    endpoint: "/v1/soracloud/model/artifact/register",
    response_label: "encode model artifact response",
    seed: 0x1B,
    args: ModelArtifactRegisterArgs {
        model_name: "fare-model".to_owned(),
        training_job_id: "job-1".to_owned(),
        weight_artifact_hash: Hash::new(b"weights"),
        dataset_ref: "dataset:v1".to_owned(),
        training_config_hash: Hash::new(b"training-config"),
        reproducibility_hash: Hash::new(b"reproducibility"),
        provenance_attestation_hash: Hash::new(b"provenance"),
    },
    success: "model artifact-register should succeed",
    request_label: "model artifact register request",
    decode_label: "decode model artifact register request",
);
manifest_pair_status_service_name_case!(
    model_artifact_status_args_can_resolve_service_name_from_manifest_pair,
    temp: "model_artifact_status_service_name_from_manifest_pair",
    endpoint: "/v1/soracloud/model/artifact/status?service_name=echo_console&training_job_id=job-1",
    response: model_artifact_status_fixture("echo_console", "job-1"),
    response_label: "encode model artifact status response",
    args: ModelArtifactStatusArgs {
        training_job_id: "job-1".to_owned(),
    },
    success: "model artifact-status should succeed",
    request_label: "model artifact status request",
);
manifest_pair_submission_service_name_case!(
    model_weight_register_args_can_resolve_service_name_from_manifest_pair,
    temp: "model_weight_register_service_name_from_manifest_pair",
    endpoint: "/v1/soracloud/model/weight/register",
    response_label: "encode model weight register response",
    seed: 0x1C,
    args: ModelWeightRegisterArgs {
        model_name: "fare-model".to_owned(),
        weight_version: "v1".to_owned(),
        training_job_id: "job-1".to_owned(),
        parent_version: None,
        weight_artifact_hash: Hash::new(b"weights"),
        dataset_ref: "dataset:v1".to_owned(),
        training_config_hash: Hash::new(b"training-config"),
        reproducibility_hash: Hash::new(b"reproducibility"),
        provenance_attestation_hash: Hash::new(b"provenance"),
    },
    success: "model weight-register should succeed",
    request_label: "model weight register request",
    decode_label: "decode model weight register request",
);
manifest_pair_submission_service_name_case!(
    model_weight_promote_args_can_resolve_service_name_from_manifest_pair,
    temp: "model_weight_promote_service_name_from_manifest_pair",
    endpoint: "/v1/soracloud/model/weight/promote",
    response_label: "encode model weight promote response",
    seed: 0x1D,
    args: ModelWeightPromoteArgs {
        model_name: "fare-model".to_owned(),
        weight_version: "v1".to_owned(),
        gate_approved: true,
        gate_report_hash: Hash::new(b"gate-report"),
    },
    success: "model weight-promote should succeed",
    request_label: "model weight promote request",
    decode_label: "decode model weight promote request",
);
manifest_pair_submission_service_name_case!(
    model_weight_rollback_args_can_resolve_service_name_from_manifest_pair,
    temp: "model_weight_rollback_service_name_from_manifest_pair",
    endpoint: "/v1/soracloud/model/weight/rollback",
    response_label: "encode model weight rollback response",
    seed: 0x1E,
    args: ModelWeightRollbackArgs {
        model_name: "fare-model".to_owned(),
        target_version: "v0".to_owned(),
        reason: "failed eval".to_owned(),
    },
    success: "model weight-rollback should succeed",
    request_label: "model weight rollback request",
    decode_label: "decode model weight rollback request",
);
manifest_pair_status_service_name_case!(
    model_weight_status_args_can_resolve_service_name_from_manifest_pair,
    temp: "model_weight_status_service_name_from_manifest_pair",
    endpoint: "/v1/soracloud/model/weight/status?service_name=echo_console&model_name=fare-model",
    response: model_weight_status_fixture("echo_console", "fare-model"),
    response_label: "encode model weight status response",
    args: ModelWeightStatusArgs {
        model_name: "fare-model".to_owned(),
    },
    success: "model weight-status should succeed",
    request_label: "model weight status request",
);
manifest_pair_status_service_name_case!(
    model_upload_status_args_can_resolve_service_name_from_manifest_pair,
    temp: "model_upload_status_service_name_from_manifest_pair",
    endpoint: "/v1/soracloud/model/upload/status?service_name=echo_console&weight_version=v1&model_name=fare-model",
    response: uploaded_model_status_fixture("echo_console", "fare-model", "v1"),
    response_label: "encode model upload status response",
    args: ModelUploadStatusArgs {
        weight_version: "v1".to_owned(),
        model_id: None,
        model_name: Some("fare-model".to_owned()),
        bundle_root: None,
    },
    success: "model upload-status should succeed",
    request_label: "model upload status request",
);
#[test]
fn fetch_torii_status_rejects_invalid_url() {
    let err = fetch_torii_soracloud_status("not-a-url", None, None, 5)
        .expect_err("invalid URL must fail");
    assert!(err.to_string().contains("invalid --torii-url"));
}
#[test]
fn fetch_torii_agent_autonomy_status_rejects_invalid_url() {
    let err = fetch_torii_soracloud_agent_autonomy_status("not-a-url", "ops_agent", None, 5)
        .expect_err("invalid URL must fail");
    assert!(err.to_string().contains("invalid --torii-url"));
}
#[test]
fn fetch_torii_agent_status_rejects_invalid_url() {
    let err = fetch_torii_soracloud_agent_status("not-a-url", Some("ops_agent"), None, 5)
        .expect_err("invalid URL must fail");
    assert!(err.to_string().contains("invalid --torii-url"));
}
#[test]
fn fetch_torii_agent_mailbox_status_rejects_invalid_url() {
    let err = fetch_torii_soracloud_agent_mailbox_status("not-a-url", "ops_agent", None, 5)
        .expect_err("invalid URL must fail");
    assert!(err.to_string().contains("invalid --torii-url"));
}
#[test]
fn fetch_torii_training_job_status_rejects_invalid_url() {
    let err =
        fetch_torii_soracloud_training_job_status("not-a-url", "web_portal", "job-1", None, 5)
            .expect_err("invalid URL must fail");
    assert!(err.to_string().contains("invalid --torii-url"));
}
#[test]
fn fetch_torii_model_artifact_status_rejects_invalid_url() {
    let err =
        fetch_torii_soracloud_model_artifact_status("not-a-url", "web_portal", "job-1", None, 5)
            .expect_err("invalid URL must fail");
    assert!(err.to_string().contains("invalid --torii-url"));
}
#[test]
fn fetch_torii_model_weight_status_rejects_invalid_url() {
    let err =
        fetch_torii_soracloud_model_weight_status("not-a-url", "web_portal", "model-v1", None, 5)
            .expect_err("invalid URL must fail");
    assert!(err.to_string().contains("invalid --torii-url"));
}
#[test]
fn fetch_torii_hf_status_rejects_invalid_url() {
    let err = fetch_torii_soracloud_hf_status(
        "not-a-url",
        "openai/gpt-oss",
        TEST_HF_COMMIT_OID,
        StorageClass::Warm,
        604_800_000,
        None,
        None,
        5,
    )
    .expect_err("invalid URL must fail");
    assert!(err.to_string().contains("torii-url"));
}
#[test]
fn sign_soracloud_payload_returns_verifiable_signature() {
    let key_pair = soracloud_fixture_key_pair(0x21);
    let payload = b"soracloud cli checked signing";
    let signature =
        sign_soracloud_payload(&key_pair, payload).expect("checked signature construction");
    signature
        .verify(key_pair.public_key(), payload)
        .expect("checked signature should verify");
}
#[test]
fn signed_request_builders_serialize_without_inline_signing_fields() {
    let key_pair = soracloud_fixture_key_pair(0x20);
    let authority = AccountId::new(key_pair.public_key().clone());
    let config_set = signed_service_config_set_request(
        "web_portal",
        "runtime",
        norito::json!({"workers": 2}),
        &authority,
        &key_pair,
    )
    .expect("signed service config set request");
    assert_request_has_no_inline_signing_fields(&config_set);
    let config_delete =
        signed_service_config_delete_request("web_portal", "runtime", &authority, &key_pair)
            .expect("signed service config delete request");
    assert_request_has_no_inline_signing_fields(&config_delete);
    let secret = SecretEnvelopeV1 {
        schema_version: iroha::data_model::soracloud::prelude::SECRET_ENVELOPE_VERSION_V1,
        encryption: iroha::data_model::soracloud::SecretEnvelopeEncryptionV1::ClientCiphertext,
        key_id: "kms/test".to_owned(),
        key_version: NonZeroU32::new(1).expect("non-zero key version"),
        nonce: vec![0x01],
        ciphertext: vec![0x02],
        commitment: Hash::new(b"secret"),
        aad_digest: None,
    };
    let secret_set =
        signed_service_secret_set_request("web_portal", "api_token", secret, &authority, &key_pair)
            .expect("signed service secret set request");
    assert_request_has_no_inline_signing_fields(&secret_set);
    let secret_delete =
        signed_service_secret_delete_request("web_portal", "api_token", &authority, &key_pair)
            .expect("signed service secret delete request");
    assert_request_has_no_inline_signing_fields(&secret_delete);
    let app = signed_app_infra_request(
        MutationMode::Deploy,
        SoraAppInfraManifestV1 {
            schema_version: SORA_APP_INFRA_MANIFEST_VERSION_V1,
            app_name: "test_app".parse().expect("valid app name"),
            app_version: "1".to_owned(),
            public_url: "https://test.invalid".to_owned(),
            static_site: None,
            services: Vec::new(),
        },
        Vec::new(),
        SoraAppInfraMutationPreconditionV1::AppAbsent,
        &key_pair,
    )
    .expect("signed app infra request");
    assert_request_has_no_inline_signing_fields(&app);
}
#[test]
fn signed_bundle_request_uses_verifiable_signature() {
    let container = fixture_container();
    let mut service = fixture_service();
    service.container.manifest_hash = Hash::new(Encode::encode(&container));
    let bundle = SoraDeploymentBundleV1 {
        schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
        container,
        service,
    };
    let key_pair = soracloud_fixture_key_pair(0x22);
    let authority = AccountId::new(key_pair.public_key().clone());
    let request = signed_bundle_request(
        bundle,
        BTreeMap::new(),
        BTreeMap::new(),
        SoraServiceMutationPreconditionV1::ServiceAbsent,
        Some(&authority),
        &key_pair,
    )
    .expect("signed request");
    let payload = iroha_data_model::soracloud::encode_bundle_with_materials_provenance_payload(
        &request.bundle,
        &request.initial_service_configs,
        &request.initial_service_secrets,
        &request.precondition,
    )
    .expect("encode payload");
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .expect("signature should verify");
    assert_request_has_no_inline_signing_fields(&request);
}
#[test]
fn signed_inrou_bundle_request_uses_canonical_guest_images_signature() {
    let mut source = build_split_app_live_service_bundle(
        "signed_inrou_bundle",
        "signed-inrou-bundle.sora",
        "1.0.0",
    )
    .expect("build unpublished dual-ISA Inrou bundle");
    source.service.placement_targets =
        test_inrou_placement_targets(usize::from(source.service.replicas.get()));
    let bundle = source
        .into_admitted(BTreeMap::from([
            (
                SoraInrouGuestIsaV1::X8664,
                sample_published_inrou_artifact(0x31),
            ),
            (
                SoraInrouGuestIsaV1::Aarch64,
                sample_published_inrou_artifact(0x32),
            ),
        ]))
        .expect("construct admitted dual-ISA Inrou bundle");
    assert_eq!(
        bundle
            .container
            .inrou
            .as_ref()
            .expect("Inrou manifest")
            .guest_images
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        vec![SoraInrouGuestIsaV1::X8664, SoraInrouGuestIsaV1::Aarch64,]
    );
    let key_pair = soracloud_fixture_key_pair(0x23);
    let authority = AccountId::new(key_pair.public_key().clone());
    let request = signed_bundle_request(
        bundle,
        BTreeMap::new(),
        BTreeMap::new(),
        SoraServiceMutationPreconditionV1::ServiceAbsent,
        Some(&authority),
        &key_pair,
    )
    .expect("signed request");
    let payload = encode_bundle_with_materials_provenance_payload(
        &request.bundle,
        &request.initial_service_configs,
        &request.initial_service_secrets,
        &request.precondition,
    )
    .expect("encode canonical payload");
    request
        .provenance
        .signature
        .verify(&request.provenance.signer, &payload)
        .expect("canonical signature should verify");
}
#[test]
fn sorafs_cid_host_suffix_for_hostname_prefers_network_suffix() {
    assert_eq!(
        sorafs_cid_host_suffix_for_hostname("solswap-indexer.taira.sora.org"),
        "sorafs.taira.sora.org"
    );
    assert_eq!(
        sorafs_cid_host_suffix_for_hostname("market-api.sora.org"),
        "sorafs.sora.org"
    );
    assert_eq!(
        sorafs_cid_host_suffix_for_hostname("travel-ops.sora"),
        "sorafs.sora"
    );
}
#[test]
fn normalize_public_service_base_url_uses_route_prefix() {
    let container = fixture_container();
    let mut service = fixture_service();
    service.route.as_mut().expect("fixture route").host =
        "solswap-indexer.taira.sora.org".to_owned();
    service.route.as_mut().expect("fixture route").path_prefix = "/".to_owned();
    service.container.manifest_hash = Hash::new(Encode::encode(&container));
    let bundle = SoraDeploymentBundleV1 {
        schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
        container,
        service,
    };
    let base_url = normalize_public_service_base_url(&bundle).expect("public base url");
    assert_eq!(base_url.as_str(), "https://solswap-indexer.taira.sora.org/");
}
macro_rules! signed_request_signature_case {
    ($name:ident, $seed:expr, $builder:expr, $encoder:ident, $expectation:literal) => {
        #[test]
        fn $name() {
            let key_pair = soracloud_fixture_key_pair($seed);
            let authority = AccountId::new(key_pair.public_key().clone());
            let request = ($builder)(&authority, &key_pair).expect($expectation);
            let payload = $encoder(&request.payload).expect("encode payload");
            assert_signed_request_signature(&request, &request.provenance, &payload);
        }
    };
}
signed_request_signature_case!(
    signed_rollback_request_uses_verifiable_signature,
    0x24,
    |authority, key_pair| signed_rollback_request("web_portal", "1.0.0", Some(authority), key_pair),
    encode_rollback_signature_payload,
    "signed rollback request"
);
signed_request_signature_case!(
    signed_rollout_request_uses_verifiable_signature,
    0x25,
    |authority, key_pair| signed_rollout_request(
        "web_portal",
        "web_portal:rollout:2",
        true,
        Some(100),
        Hash::new(b"governance"),
        Some(authority),
        key_pair,
    ),
    encode_rollout_signature_payload,
    "signed rollout request"
);
#[test]
fn signed_rollout_request_rejects_branch_target_mismatches() {
    let key_pair = soracloud_fixture_key_pair(0x26);
    let authority = AccountId::new(key_pair.public_key().clone());
    for (healthy, promote_to_percent, expected) in [
        (true, None, "require --promote-to-percent"),
        (false, Some(25), "forbid --promote-to-percent"),
    ] {
        let error = signed_rollout_request(
            "web_portal",
            "web_portal:rollout:2",
            healthy,
            promote_to_percent,
            Hash::new(b"governance"),
            Some(&authority),
            &key_pair,
        )
        .expect_err("rollout branch/target mismatch must fail before submission");
        assert!(
            error.to_string().contains(expected),
            "unexpected error: {error}"
        );
    }
}
signed_request_signature_case!(
    signed_agent_deploy_request_uses_verifiable_signature,
    0x26,
    |authority, key_pair| signed_agent_deploy_request(
        fixture_agent_apartment(),
        120,
        500,
        authority,
        key_pair,
    ),
    encode_agent_deploy_signature_payload,
    "signed agent deploy request"
);
signed_request_signature_case!(
    signed_agent_lease_renew_request_uses_verifiable_signature,
    0x27,
    |authority, key_pair| signed_agent_lease_renew_request("ops_agent", 120, authority, key_pair,),
    encode_agent_lease_renew_signature_payload,
    "signed agent lease renew request"
);
#[test]
fn signed_hf_shared_lease_join_request_uses_verifiable_signature() {
    let key_pair = soracloud_fixture_key_pair(0x28);
    let authority = AccountId::new(key_pair.public_key().clone());
    let base_fee = "0.00001".parse().expect("canonical exact base fee");
    let request = signed_hf_shared_lease_join_request(
        "openai/gpt-oss",
        TEST_HF_COMMIT_OID,
        "hf_lease_a",
        Some("ops_agent"),
        StorageClass::Warm,
        604_800_000,
        &hf_shared_lease_asset_definition().to_string(),
        &base_fee,
        &authority,
        &key_pair,
    )
    .expect("signed hf shared-lease join request");
    let payload =
        encode_hf_shared_lease_join_signature_payload(&request.payload).expect("encode payload");
    assert_signed_request_signature(&request, &request.provenance, &payload);
    let mut encoded = json::to_value(&request).expect("encode inert HF shared-lease join request");
    let object = encoded
        .as_object()
        .expect("HF shared-lease join request object");
    assert_eq!(object.len(), 2);
    assert!(object.contains_key("payload"));
    assert!(object.contains_key("provenance"));
    encoded
        .as_object_mut()
        .expect("HF shared-lease join request object")
        .insert("generated_service_provenance".to_owned(), Value::Null);
    json::from_value::<SignedHfSharedLeaseJoinRequest>(encoded)
        .expect_err("generated HF service execution provenance is retired in V1");
}
signed_request_signature_case!(
    signed_hf_lease_leave_request_uses_verifiable_signature,
    0x29,
    |authority, key_pair| signed_hf_lease_leave_request(
        "openai/gpt-oss",
        TEST_HF_COMMIT_OID,
        StorageClass::Warm,
        604_800_000,
        Some("hf_lease_a"),
        Some("ops_agent"),
        authority,
        key_pair,
    ),
    encode_hf_lease_leave_signature_payload,
    "signed hf leave request"
);
#[test]
fn signed_hf_lease_renew_request_uses_verifiable_signature() {
    let key_pair = soracloud_fixture_key_pair(0x2A);
    let authority = AccountId::new(key_pair.public_key().clone());
    let base_fee = "0.00001".parse().expect("canonical exact base fee");
    let request = signed_hf_lease_renew_request(
        "openai/gpt-oss",
        TEST_HF_COMMIT_OID,
        "hf_lease_renew",
        Some("ops_agent"),
        StorageClass::Warm,
        604_800_000,
        &hf_shared_lease_asset_definition().to_string(),
        &base_fee,
        &authority,
        &key_pair,
    )
    .expect("signed hf renew request");
    let payload =
        encode_hf_lease_renew_signature_payload(&request.payload).expect("encode payload");
    assert_signed_request_signature(&request, &request.provenance, &payload);
    let mut encoded = json::to_value(&request).expect("encode inert HF renew request");
    let object = encoded.as_object().expect("HF renew request object");
    assert_eq!(object.len(), 2);
    assert!(object.contains_key("payload"));
    assert!(object.contains_key("provenance"));
    encoded
        .as_object_mut()
        .expect("HF renew request object")
        .insert("generated_apartment_provenance".to_owned(), Value::Null);
    json::from_value::<SignedHfLeaseRenewRequest>(encoded)
        .expect_err("generated HF apartment execution provenance is retired in V1");
}
signed_request_signature_case!(
    signed_agent_restart_request_uses_verifiable_signature,
    0x2C,
    |authority, key_pair| signed_agent_restart_request(
        "ops_agent",
        "manual-restart",
        authority,
        key_pair,
    ),
    encode_agent_restart_signature_payload,
    "signed agent restart request"
);
signed_request_signature_case!(
    signed_agent_policy_revoke_request_uses_verifiable_signature,
    0x2D,
    |authority, key_pair| signed_agent_policy_revoke_request(
        "ops_agent",
        "agent.autonomy.run",
        Some("manual-review"),
        authority,
        key_pair,
    ),
    encode_agent_policy_revoke_signature_payload,
    "signed agent policy revoke request"
);
signed_request_signature_case!(
    signed_agent_wallet_spend_request_uses_verifiable_signature,
    0x2E,
    |authority, key_pair| {
        let amount = "0.001".parse().expect("canonical exact amount");
        signed_agent_wallet_spend_request(
            "ops_agent",
            "wallet-spend-fixture-1",
            "61CtjvNd9T3THAR65GsMVHr82Bjc",
            &amount,
            authority,
            key_pair,
        )
    },
    encode_agent_wallet_spend_signature_payload,
    "signed agent wallet spend request"
);
signed_request_signature_case!(
    signed_agent_wallet_approve_request_uses_verifiable_signature,
    0x2F,
    |authority, key_pair| signed_agent_wallet_approve_request(
        "ops_agent",
        "ops_agent:wallet:7",
        authority,
        key_pair,
    ),
    encode_agent_wallet_approve_signature_payload,
    "signed agent wallet approve request"
);
signed_request_signature_case!(
    signed_agent_message_send_request_uses_verifiable_signature,
    0x30,
    |authority, key_pair| signed_agent_message_send_request(
        "ops_agent",
        "worker_agent",
        "ops.sync",
        "rotate-key-42",
        authority,
        key_pair,
    ),
    encode_agent_message_send_signature_payload,
    "signed agent message send request"
);
signed_request_signature_case!(
    signed_agent_message_ack_request_uses_verifiable_signature,
    0x31,
    |authority, key_pair| signed_agent_message_ack_request(
        "worker_agent",
        "worker_agent:mail:3",
        authority,
        key_pair,
    ),
    encode_agent_message_ack_signature_payload,
    "signed agent message ack request"
);
signed_request_signature_case!(
    signed_agent_artifact_allow_request_uses_verifiable_signature,
    0x32,
    |authority, key_pair| signed_agent_artifact_allow_request(
        "ops_agent",
        "hash:ABCD0123#01",
        Some("hash:PROV0001#01"),
        authority,
        key_pair,
    ),
    encode_agent_artifact_allow_signature_payload,
    "signed agent artifact allow request"
);
signed_request_signature_case!(
    signed_training_job_start_request_uses_verifiable_signature,
    0x34,
    |authority, key_pair| signed_training_job_start_request(
        "web_portal",
        "model-1",
        "job-1",
        4,
        100,
        20,
        3,
        500,
        50_000,
        4_000,
        Some(authority),
        key_pair,
    ),
    encode_training_job_start_signature_payload,
    "signed training start request"
);
signed_request_signature_case!(
    signed_training_job_checkpoint_request_uses_verifiable_signature,
    0x35,
    |authority, key_pair| signed_training_job_checkpoint_request(
        "web_portal",
        "job-1",
        20,
        1_024,
        Hash::new(b"metrics"),
        Some(authority),
        key_pair,
    ),
    encode_training_job_checkpoint_signature_payload,
    "signed training checkpoint request"
);
signed_request_signature_case!(
    signed_training_job_retry_request_uses_verifiable_signature,
    0x36,
    |authority, key_pair| signed_training_job_retry_request(
        "web_portal",
        "job-1",
        "worker unavailable",
        Some(authority),
        key_pair,
    ),
    encode_training_job_retry_signature_payload,
    "signed training retry request"
);
signed_request_signature_case!(
    signed_model_artifact_register_request_uses_verifiable_signature,
    0x37,
    |authority, key_pair| signed_model_artifact_register_request(
        "web_portal",
        "model-1",
        "job-1",
        Hash::new(b"weight-artifact"),
        "dataset://synthetic/v2",
        Hash::new(b"train-config"),
        Hash::new(b"repro"),
        Hash::new(b"attestation"),
        Some(authority),
        key_pair,
    ),
    encode_model_artifact_register_signature_payload,
    "signed model artifact request"
);
signed_request_signature_case!(
    signed_model_weight_register_request_uses_verifiable_signature,
    0x38,
    |authority, key_pair| signed_model_weight_register_request(
        "web_portal",
        "model-1",
        "1.0.0",
        "job-1",
        Some("0.9.0"),
        Hash::new(b"weight-artifact"),
        "dataset://synthetic/v2",
        Hash::new(b"train-config"),
        Hash::new(b"repro"),
        Hash::new(b"attestation"),
        Some(authority),
        key_pair,
    ),
    encode_model_weight_register_signature_payload,
    "signed model weight register request"
);
signed_request_signature_case!(
    signed_model_weight_promote_request_uses_verifiable_signature,
    0x39,
    |authority, key_pair| signed_model_weight_promote_request(
        "web_portal",
        "model-1",
        "1.0.0",
        true,
        Hash::new(b"gate-report"),
        Some(authority),
        key_pair,
    ),
    encode_model_weight_promote_signature_payload,
    "signed model weight promote request"
);
signed_request_signature_case!(
    signed_model_weight_rollback_request_uses_verifiable_signature,
    0x3A,
    |authority, key_pair| signed_model_weight_rollback_request(
        "web_portal",
        "model-1",
        "0.9.0",
        "gate regression",
        Some(authority),
        key_pair,
    ),
    encode_model_weight_rollback_signature_payload,
    "signed model weight rollback request"
);
#[test]
fn signed_training_and_model_builders_reject_input_rewrites() {
    let key_pair = soracloud_fixture_key_pair(0x7B);
    let authority = AccountId::new(key_pair.public_key().clone());

    let _error = signed_training_job_start_request(
        " web_portal",
        "model-1",
        "job-1",
        1,
        1,
        1,
        0,
        1,
        1,
        1,
        Some(&authority),
        &key_pair,
    )
    .expect_err("training start must reject padded service names");
    let _error = signed_training_job_checkpoint_request(
        "web_portal",
        "job-1 ",
        1,
        1,
        Hash::new(b"metrics"),
        Some(&authority),
        &key_pair,
    )
    .expect_err("training checkpoint must reject padded job IDs");
    let _error = signed_training_job_retry_request(
        "web_portal",
        "job-1",
        " retry",
        Some(&authority),
        &key_pair,
    )
    .expect_err("training retry must reject padded reasons");
    let _error = signed_model_artifact_register_request(
        "web_portal",
        "model-1",
        "job-1",
        Hash::new(b"weight"),
        " dataset://v1",
        Hash::new(b"config"),
        Hash::new(b"repro"),
        Hash::new(b"attestation"),
        Some(&authority),
        &key_pair,
    )
    .expect_err("model artifact registration must reject padded dataset refs");
    let _error = signed_model_weight_register_request(
        "web_portal",
        "model-1",
        "v1 ",
        "job-1",
        None,
        Hash::new(b"weight"),
        "dataset://v1",
        Hash::new(b"config"),
        Hash::new(b"repro"),
        Hash::new(b"attestation"),
        Some(&authority),
        &key_pair,
    )
    .expect_err("model weight registration must reject padded versions");
    let _error = signed_model_weight_promote_request(
        "web_portal",
        "cafe\u{301}",
        "v1",
        true,
        Hash::new(b"gate"),
        Some(&authority),
        &key_pair,
    )
    .expect_err("model promotion must reject NFC aliases");
    let _error = signed_model_weight_rollback_request(
        "web_portal",
        "model-1",
        "v0",
        " regression",
        Some(&authority),
        &key_pair,
    )
    .expect_err("model rollback must reject padded reasons");
}
#[test]
fn bundle_signature_payload_layout_is_canonical_layout() {
    let container = fixture_container();
    let mut service = fixture_service();
    service.container.manifest_hash = Hash::new(Encode::encode(&container));
    let bundle = SoraDeploymentBundleV1 {
        schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
        container,
        service,
    };
    let encoded = iroha_data_model::soracloud::encode_bundle_provenance_payload(&bundle)
        .expect("encode signature payload");
    let expected = norito::to_bytes(&bundle).expect("encode canonical layout");
    assert_eq!(encoded, expected);
}
#[test]
fn signed_bundle_request_requires_explicit_material_maps_and_closed_fields() {
    let container = fixture_container();
    let mut service = fixture_service();
    service.container.manifest_hash = Hash::new(Encode::encode(&container));
    let key_pair = soracloud_fixture_key_pair(0x43);
    let authority = AccountId::new(key_pair.public_key().clone());
    let request = signed_bundle_request(
        SoraDeploymentBundleV1 {
            schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
            container,
            service,
        },
        BTreeMap::new(),
        BTreeMap::new(),
        SoraServiceMutationPreconditionV1::ServiceAbsent,
        Some(&authority),
        &key_pair,
    )
    .expect("canonical signed bundle request");
    let canonical =
        norito::json::to_value(&request).expect("serialize canonical signed bundle request");
    norito::json::from_value::<SignedBundleRequest>(canonical.clone())
        .expect("canonical signed bundle request must decode");

    for field in ["initial_service_configs", "initial_service_secrets"] {
        let mut missing = canonical.clone();
        assert!(
            missing
                .as_object_mut()
                .expect("signed bundle request JSON object")
                .remove(field)
                .is_some()
        );
        norito::json::from_value::<SignedBundleRequest>(missing)
            .expect_err("omitted initial material map must be rejected");

        let mut null = canonical.clone();
        null.as_object_mut()
            .expect("signed bundle request JSON object")
            .insert(field.to_owned(), norito::json::Value::Null);
        norito::json::from_value::<SignedBundleRequest>(null)
            .expect_err("null initial material map must be rejected");
    }

    let mut unknown = canonical;
    unknown
        .as_object_mut()
        .expect("signed bundle request JSON object")
        .insert("retired_v0".to_owned(), norito::json::Value::from(true));
    norito::json::from_value::<SignedBundleRequest>(unknown)
        .expect_err("signed bundle request must reject unknown fields");

    let mut container = fixture_container();
    let mut service = fixture_service();
    service.container.manifest_hash = Hash::new(Encode::encode(&container));
    let _error = signed_bundle_request(
        SoraDeploymentBundleV1 {
            schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
            container: container.clone(),
            service: service.clone(),
        },
        BTreeMap::from([(
            " padded-config".to_owned(),
            Json::from(norito::json!({"enabled": true})),
        )]),
        BTreeMap::new(),
        SoraServiceMutationPreconditionV1::ServiceAbsent,
        Some(&authority),
        &key_pair,
    )
    .expect_err("initial config keys must be exact before signing");

    container.capabilities.network =
        SoraNetworkPolicyV1::Allowlist(vec![SoraNetworkAllowlistEntryV1::new(
            " api.example.test",
            [443],
        )]);
    service.container.manifest_hash = Hash::new(Encode::encode(&container));
    let _error = signed_bundle_request(
        SoraDeploymentBundleV1 {
            schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
            container,
            service,
        },
        BTreeMap::new(),
        BTreeMap::new(),
        SoraServiceMutationPreconditionV1::ServiceAbsent,
        Some(&authority),
        &key_pair,
    )
    .expect_err("padded network allowlist hosts must fail before signing");
}
#[test]
fn signed_app_infra_request_requires_explicit_service_vectors_and_closed_fields() {
    let key_pair = soracloud_fixture_key_pair(0x45);
    let request = signed_app_infra_request(
        MutationMode::Deploy,
        SoraAppInfraManifestV1 {
            schema_version: SORA_APP_INFRA_MANIFEST_VERSION_V1,
            app_name: "web_portal".parse().expect("valid app name"),
            app_version: "1.0.0".to_owned(),
            public_url: "https://web-portal.example".to_owned(),
            static_site: None,
            services: Vec::new(),
        },
        Vec::new(),
        SoraAppInfraMutationPreconditionV1::AppAbsent,
        &key_pair,
    )
    .expect("canonical signed app request");
    let canonical =
        norito::json::to_value(&request).expect("serialize canonical signed app request");
    norito::json::from_value::<SignedAppInfraRequest>(canonical.clone())
        .expect("canonical signed app request must decode");

    for field in ["deploy_services", "upgrade_services"] {
        let mut missing = canonical.clone();
        assert!(
            missing
                .as_object_mut()
                .expect("signed app request JSON object")
                .remove(field)
                .is_some()
        );
        norito::json::from_value::<SignedAppInfraRequest>(missing)
            .expect_err("signed app request must reject an omitted service vector");

        let mut null = canonical.clone();
        null.as_object_mut()
            .expect("signed app request JSON object")
            .insert(field.to_owned(), norito::json::Value::Null);
        norito::json::from_value::<SignedAppInfraRequest>(null)
            .expect_err("signed app request must reject a null service vector");
    }

    let mut unknown = canonical;
    unknown
        .as_object_mut()
        .expect("signed app request JSON object")
        .insert("retired_v0".to_owned(), norito::json::Value::from(true));
    norito::json::from_value::<SignedAppInfraRequest>(unknown)
        .expect_err("signed app request must reject unknown fields");
}
#[test]
fn service_config_and_secret_payloads_reject_nested_unknown_fields() {
    macro_rules! assert_closed {
        ($value:expr, $ty:ty, $label:literal) => {{
            let mut value =
                norito::json::to_value(&$value).expect(concat!("serialize canonical ", $label));
            norito::json::from_value::<$ty>(value.clone()).expect(concat!(
                "canonical ",
                $label,
                " must decode"
            ));
            value
                .as_object_mut()
                .expect(concat!($label, " JSON object"))
                .insert("retired_v0".to_owned(), norito::json::Value::from(true));
            norito::json::from_value::<$ty>(value)
                .expect_err(concat!($label, " must reject unknown fields"));
        }};
    }

    assert_closed!(
        ServiceConfigSetPayload {
            service_name: "web_portal".to_owned(),
            config_name: "runtime".to_owned(),
            value_json: Json::from(norito::json!({"workers": 2_u64})),
        },
        ServiceConfigSetPayload,
        "service config set payload"
    );
    assert_closed!(
        ServiceConfigDeletePayload {
            service_name: "web_portal".to_owned(),
            config_name: "runtime".to_owned(),
        },
        ServiceConfigDeletePayload,
        "service config delete payload"
    );
    let secret = SecretEnvelopeV1 {
        schema_version: iroha::data_model::soracloud::prelude::SECRET_ENVELOPE_VERSION_V1,
        encryption: SecretEnvelopeEncryptionV1::ClientCiphertext,
        key_id: "kms/test".to_owned(),
        key_version: NonZeroU32::new(1).expect("non-zero key version"),
        nonce: vec![1],
        ciphertext: vec![2],
        commitment: Hash::new(b"secret"),
        aad_digest: None,
    };
    assert_closed!(
        ServiceSecretSetPayload {
            service_name: "web_portal".to_owned(),
            secret_name: "api_token".to_owned(),
            secret,
        },
        ServiceSecretSetPayload,
        "service secret set payload"
    );
    assert_closed!(
        ServiceSecretDeletePayload {
            service_name: "web_portal".to_owned(),
            secret_name: "api_token".to_owned(),
        },
        ServiceSecretDeletePayload,
        "service secret delete payload"
    );
}
#[test]
fn rollout_response_mirrors_are_closed_and_require_non_null_baseline() {
    macro_rules! assert_closed {
        ($value:expr, $ty:ty, $label:literal) => {{
            let mut value =
                norito::json::to_value(&$value).expect(concat!("serialize canonical ", $label));
            norito::json::from_value::<$ty>(value.clone()).expect(concat!(
                "canonical ",
                $label,
                " must decode"
            ));
            value
                .as_object_mut()
                .expect(concat!($label, " JSON object"))
                .insert("retired_v0".to_owned(), norito::json::Value::from(true));
            norito::json::from_value::<$ty>(value)
                .expect_err(concat!($label, " must reject unknown fields"));
        }};
    }
    assert_closed!(SoracloudAction::Deploy, SoracloudAction, "Soracloud action");
    assert_closed!(
        SoracloudAction::LeaseUsage,
        SoracloudAction,
        "lease-usage Soracloud action"
    );
    assert_closed!(RolloutStage::Canary, RolloutStage, "rollout stage");

    let state = RolloutRuntimeState {
        rollout_handle: "web_portal:rollout:2".to_owned(),
        baseline_version: "1.0.0".to_owned(),
        candidate_version: "2.0.0".to_owned(),
        canary_percent: 10,
        traffic_percent: 10,
        stage: RolloutStage::Canary,
        health_failures: 0,
        max_health_failures: 3,
        health_window_secs: 30,
        created_sequence: 1,
        updated_sequence: 1,
    };
    assert_closed!(state.clone(), RolloutRuntimeState, "rollout runtime state");
    let canonical = norito::json::to_value(&state).expect("serialize rollout runtime state");
    assert_eq!(
        canonical
            .get("baseline_version")
            .and_then(norito::json::Value::as_str),
        Some("1.0.0")
    );
    let mut missing = canonical.clone();
    assert!(
        missing
            .as_object_mut()
            .expect("rollout runtime state JSON object")
            .remove("baseline_version")
            .is_some()
    );
    norito::json::from_value::<RolloutRuntimeState>(missing)
        .expect_err("rollout runtime state must reject omitted baseline_version");
    let mut explicit_null = canonical;
    explicit_null
        .as_object_mut()
        .expect("rollout runtime state JSON object")
        .insert("baseline_version".to_owned(), norito::json::Value::Null);
    norito::json::from_value::<RolloutRuntimeState>(explicit_null)
        .expect_err("rollout runtime state must reject explicit null baseline_version");
}
#[test]
fn soracloud_cli_output_graph_rejects_unknown_fields() {
    macro_rules! assert_closed {
        ($($ty:ty),+ $(,)?) => {
            $(
                let error = json::from_str::<$ty>(r#"{"retired_v0":true}"#)
                    .expect_err(concat!(stringify!($ty), " must reject unknown fields"));
                assert!(
                    matches!(
                        error,
                        json::Error::UnknownField { ref field } if field == "retired_v0"
                    ),
                    "{} reported the wrong error: {error}",
                    stringify!($ty)
                );
            )+
        };
    }

    assert_closed!(
        InitOutput,
        BundlePackOutput,
        SyncManifestsOutput,
        SyncManifestEntryOutput,
        StatusOutput,
        AppInitOutput,
        SoracloudAppPhaseReportV1,
        SoracloudAppReportServiceV1,
        SoracloudAppReportV1,
        AppMutationOutput,
        AppServiceMutationOutput,
        ServiceWorkspaceScriptOutput,
        ServiceWorkspaceMutationScriptOutput,
        ServiceMutationOutput,
        ServiceLocalPlanOutput,
        ServiceWorkspaceScriptsOutput,
        ServiceLocalRouteOutput,
        AppStatusOutput,
        AppDoctorCheckOutput,
        AppDoctorOutput,
        AppReleaseOutput,
        AppSimulateOutput,
        AppServiceStatusOutput,
        AppLocalPlanOutput,
        AppLocalWorkspaceScriptsOutput,
        AppLocalDevOutput,
        AppBuildAndSyncOutput,
        AppReleaseWorkspaceScriptOutput,
        AppLocalFrontendPlanOutput,
        AppLocalServicePlanOutput,
        AppLocalServiceWorkspaceScriptsOutput,
        AppLocalRoutePlanOutput,
        AppStaticSiteBindingV1,
        AppStaticSitePublishOutput,
        InrouGuestImageArtifactPublishOutput,
        ServiceBundlePublishOutput,
        SoracloudPublicServiceDiscoveryV1,
        SoracloudPublicServiceDiscoveryRegistryV1,
        PublicServiceDiscoveryPublishOutput,
    );
}
#[test]
fn soracloud_cli_output_graph_requires_explicit_null_and_empty_keys() {
    let sync = SyncManifestsOutput {
        app_manifest_path: None,
        container_manifest_path: None,
        service_manifest_path: None,
        container_manifest_hash: None,
        service_manifest_hash: None,
        bundle_file: None,
        bundle_hash: None,
        services: Vec::new(),
    };
    let canonical = json::to_value(&sync).expect("serialize canonical manifest-sync output");
    for field in [
        "app_manifest_path",
        "container_manifest_path",
        "service_manifest_path",
        "container_manifest_hash",
        "service_manifest_hash",
        "bundle_file",
        "bundle_hash",
    ] {
        assert!(
            canonical.get(field).is_some_and(json::Value::is_null),
            "manifest-sync output must serialize `{field}` as explicit null"
        );
        let mut missing = canonical.clone();
        missing
            .as_object_mut()
            .expect("manifest-sync output JSON object")
            .remove(field);
        json::from_value::<SyncManifestsOutput>(missing)
            .expect_err("manifest-sync output must reject omitted nullable keys");
    }
    assert!(
        canonical
            .get("services")
            .and_then(json::Value::as_array)
            .is_some_and(Vec::is_empty),
        "manifest-sync output must serialize an explicit empty service list"
    );
    let mut missing_services = canonical;
    missing_services
        .as_object_mut()
        .expect("manifest-sync output JSON object")
        .remove("services");
    json::from_value::<SyncManifestsOutput>(missing_services)
        .expect_err("manifest-sync output must reject an omitted service list");

    let frontend = AppLocalFrontendPlanOutput {
        dist_dir: "dist".to_owned(),
        mount_path: "/".to_owned(),
        publish_mode: APP_STATIC_SITE_PUBLISH_MODE_CID_ONLY.to_owned(),
        api_base_path: None,
        cid_gateway_url_template: None,
        root_binding_url: None,
    };
    let canonical = json::to_value(&frontend).expect("serialize canonical frontend plan");
    for field in [
        "api_base_path",
        "cid_gateway_url_template",
        "root_binding_url",
    ] {
        assert!(
            canonical.get(field).is_some_and(json::Value::is_null),
            "frontend plan must serialize `{field}` as explicit null"
        );
        let mut missing = canonical.clone();
        missing
            .as_object_mut()
            .expect("frontend plan JSON object")
            .remove(field);
        json::from_value::<AppLocalFrontendPlanOutput>(missing)
            .expect_err("frontend plan must reject omitted nullable keys");
    }

    let phase = SoracloudAppPhaseReportV1 {
        name: "validate".to_owned(),
        ok: true,
        skipped: false,
        diagnostics: Vec::new(),
    };
    let mut canonical = json::to_value(&phase).expect("serialize canonical app phase report");
    assert!(
        canonical
            .get("diagnostics")
            .and_then(json::Value::as_array)
            .is_some_and(Vec::is_empty),
        "app phase report must serialize an explicit empty diagnostics list"
    );
    canonical
        .as_object_mut()
        .expect("app phase report JSON object")
        .remove("diagnostics");
    json::from_value::<SoracloudAppPhaseReportV1>(canonical)
        .expect_err("app phase report must reject an omitted diagnostics list");
}
macro_rules! signature_payload_layout_case {
    ($name:ident, $payload:ident = $value:expr, $encoder:ident, $expected:expr $(, $extra:block)?) => {
        #[test]
        fn $name() {
            let $payload = $value;
            let encoded = $encoder(&$payload).expect("encode signature payload");
            let expected = norito::to_bytes(&$expected).expect("encode canonical tuple");
            assert_eq!(encoded, expected);
            $($extra)?
        }
    };
}
signature_payload_layout_case!(
    rollback_signature_payload_layout_is_canonical_tuple,
    payload = RollbackPayload {
        service_name: "web_portal".to_owned(),
        target_version: "1.0.0".to_owned(),
    },
    encode_rollback_signature_payload,
    (
        payload.service_name.as_str(),
        payload.target_version.as_str()
    )
);
signature_payload_layout_case!(
    rollout_signature_payload_layout_is_canonical_tuple,
    payload = RolloutAdvancePayload {
        service_name: "web_portal".to_owned(),
        rollout_handle: "web_portal:rollout:2".to_owned(),
        healthy: true,
        promote_to_percent: Some(100),
        governance_tx_hash: Hash::new(b"governance"),
    },
    encode_rollout_signature_payload,
    (
        payload.service_name.as_str(),
        payload.rollout_handle.as_str(),
        payload.healthy,
        payload.promote_to_percent,
        payload.governance_tx_hash.clone(),
    )
);
#[test]
fn rollout_advance_payload_rejects_unknown_fields() {
    let payload = RolloutAdvancePayload {
        service_name: "web_portal".to_owned(),
        rollout_handle: "web_portal:rollout:2".to_owned(),
        healthy: true,
        promote_to_percent: Some(100),
        governance_tx_hash: Hash::new(b"governance"),
    };
    let canonical =
        norito::json::to_value(&payload).expect("serialize canonical rollout advance payload");
    norito::json::from_value::<RolloutAdvancePayload>(canonical.clone())
        .expect("canonical rollout advance payload must decode");

    let mut missing = canonical.clone();
    assert!(
        missing
            .as_object_mut()
            .expect("rollout advance payload JSON object")
            .remove("promote_to_percent")
            .is_some()
    );
    norito::json::from_value::<RolloutAdvancePayload>(missing)
        .expect_err("omitted rollout promotion target must be rejected");

    let mut null = canonical.clone();
    null.as_object_mut()
        .expect("rollout advance payload JSON object")
        .insert("promote_to_percent".to_owned(), norito::json::Value::Null);
    assert!(
        norito::json::from_value::<RolloutAdvancePayload>(null)
            .expect("explicit null rollout promotion target must decode")
            .promote_to_percent
            .is_none()
    );

    let mut unknown = canonical;
    unknown
        .as_object_mut()
        .expect("rollout advance payload JSON object")
        .insert("retired_v0".to_owned(), norito::json::Value::from(true));
    let error = norito::json::from_value::<RolloutAdvancePayload>(unknown)
        .expect_err("rollout advance payload must reject unknown fields");
    assert!(
        matches!(
            error,
            norito::json::Error::UnknownField { ref field } if field == "retired_v0"
        ),
        "unexpected rollout unknown-field rejection: {error}"
    );
}
#[test]
fn rollback_payload_and_signed_request_wrappers_reject_unknown_fields() {
    macro_rules! assert_unknown_rejected {
        ($value:expr, $ty:ty, $label:literal) => {{
            let mut value = norito::json::to_value(&$value)
                .expect(concat!("serialize canonical ", $label));
            norito::json::from_value::<$ty>(value.clone())
                .expect(concat!("canonical ", $label, " must decode"));
            value
                .as_object_mut()
                .expect(concat!($label, " JSON object"))
                .insert("retired_v0".to_owned(), norito::json::Value::from(true));
            let error = norito::json::from_value::<$ty>(value)
                .expect_err(concat!($label, " must reject unknown fields"));
            assert!(
                matches!(
                    error,
                    norito::json::Error::UnknownField { ref field }
                        if field == "retired_v0"
                ),
                "{} reported the wrong error: {error}",
                $label
            );
        }};
    }

    let key_pair = soracloud_fixture_key_pair(0x44);
    let authority = AccountId::new(key_pair.public_key().clone());
    assert_unknown_rejected!(
        RollbackPayload {
            service_name: "web_portal".to_owned(),
            target_version: "1.0.0".to_owned(),
        },
        RollbackPayload,
        "rollback payload"
    );
    assert_unknown_rejected!(
        signed_rollback_request("web_portal", "1.0.0", Some(&authority), &key_pair,)
            .expect("signed rollback request"),
        SignedRollbackRequest,
        "signed rollback request"
    );
    assert_unknown_rejected!(
        signed_rollout_request(
            "web_portal",
            "web_portal:rollout:2",
            true,
            Some(100),
            Hash::new(b"governance"),
            Some(&authority),
            &key_pair,
        )
        .expect("signed rollout request"),
        SignedRolloutAdvanceRequest,
        "signed rollout request"
    );
}
#[test]
fn signed_soracloud_mutation_graph_rejects_unknown_fields() {
    macro_rules! assert_unknown_rejected {
        ($($ty:ty),+ $(,)?) => {
            $(
                let error = json::from_str::<$ty>(r#"{"retired_v0":true}"#)
                    .expect_err(concat!(stringify!($ty), " must reject unknown fields"));
                assert!(
                    matches!(
                        error,
                        json::Error::UnknownField { ref field } if field == "retired_v0"
                    ),
                    "{} reported the wrong error: {error}",
                    stringify!($ty)
                );
            )+
        };
    }
    assert_unknown_rejected!(
        SignedBundleRequest,
        SignedAppInfraRequest,
        RollbackPayload,
        SignedRollbackRequest,
        RolloutAdvancePayload,
        SignedRolloutAdvanceRequest,
        ServiceConfigSetPayload,
        SignedServiceConfigSetRequest,
        ServiceConfigDeletePayload,
        SignedServiceConfigDeleteRequest,
        ServiceSecretSetPayload,
        SignedServiceSecretSetRequest,
        ServiceSecretDeletePayload,
        SignedServiceSecretDeleteRequest,
        AgentDeployPayload,
        SignedAgentDeployRequest,
        AgentLeaseRenewPayload,
        SignedAgentLeaseRenewRequest,
        HfSharedLeaseJoinPayload,
        SignedHfSharedLeaseJoinRequest,
        HfLeaseLeavePayload,
        SignedHfLeaseLeaveRequest,
        HfLeaseRenewPayload,
        SignedHfLeaseRenewRequest,
        AgentRestartPayload,
        SignedAgentRestartRequest,
        AgentPolicyRevokePayload,
        SignedAgentPolicyRevokeRequest,
        AgentWalletSpendPayload,
        SignedAgentWalletSpendRequest,
        AgentWalletApprovePayload,
        SignedAgentWalletApproveRequest,
        AgentMessageSendPayload,
        SignedAgentMessageSendRequest,
        AgentMessageAckPayload,
        SignedAgentMessageAckRequest,
        AgentArtifactAllowPayload,
        SignedAgentArtifactAllowRequest,
        TrainingJobStartPayload,
        SignedTrainingJobStartRequest,
        TrainingJobCheckpointPayload,
        SignedTrainingJobCheckpointRequest,
        TrainingJobRetryPayload,
        SignedTrainingJobRetryRequest,
        ModelArtifactRegisterPayload,
        SignedModelArtifactRegisterRequest,
        ModelWeightRegisterPayload,
        SignedModelWeightRegisterRequest,
        ModelWeightPromotePayload,
        SignedModelWeightPromoteRequest,
        ModelWeightRollbackPayload,
        SignedModelWeightRollbackRequest,
        UploadedModelFinalizePayload,
        UploadedModelRegisterPayload,
        SignedUploadedModelRegisterRequest,
    );
}
#[test]
fn signed_soracloud_mutation_graph_requires_explicit_optional_keys() {
    macro_rules! assert_required_nullable {
        ($value:expr, $ty:ty, [$($field:literal),+ $(,)?], $label:literal) => {{
            let canonical = json::to_value(&$value)
                .expect(concat!("serialize canonical ", $label));
            json::from_value::<$ty>(canonical.clone())
                .expect(concat!("canonical ", $label, " must decode"));
            for field in [$($field),+] {
                assert!(
                    canonical.get(field).is_some_and(json::Value::is_null),
                    "{} must serialize `{field}` as explicit null",
                    $label
                );
                let mut missing = canonical.clone();
                missing
                    .as_object_mut()
                    .expect(concat!($label, " JSON object"))
                    .remove(field);
                json::from_value::<$ty>(missing)
                    .expect_err(concat!($label, " must reject an omitted nullable key"));

                let mut explicit_null = canonical.clone();
                explicit_null
                    .as_object_mut()
                    .expect(concat!($label, " JSON object"))
                    .insert(field.to_owned(), json::Value::Null);
                json::from_value::<$ty>(explicit_null)
                    .expect(concat!($label, " must accept explicit null"));
            }
        }};
    }

    let agent_deploy = AgentDeployPayload {
        manifest: fixture_agent_apartment(),
        lease_ticks: 120,
        autonomy_budget_units: 500,
    };
    let mut missing_budget = json::to_value(&agent_deploy).expect("serialize agent deploy");
    missing_budget
        .as_object_mut()
        .expect("agent deploy object")
        .remove("autonomy_budget_units");
    json::from_value::<AgentDeployPayload>(missing_budget)
        .expect_err("agent deployment must not infer an autonomy budget");

    let hf_shared_lease_join = HfSharedLeaseJoinPayload {
        repo_id: "openai/gpt-oss".to_owned(),
        revision: TEST_HF_COMMIT_OID.to_owned(),
        service_name: "hf_lease_a".to_owned(),
        apartment_name: None,
        storage_class: StorageClass::Warm,
        lease_term_ms: 604_800_000,
        lease_asset_definition_id: hf_shared_lease_asset_definition(),
        base_fee: "0.00001".parse().expect("canonical exact base fee"),
    };
    assert_required_nullable!(
        hf_shared_lease_join.clone(),
        HfSharedLeaseJoinPayload,
        ["apartment_name"],
        "HF shared-lease join payload"
    );
    assert_required_nullable!(
        HfLeaseLeavePayload {
            repo_id: hf_shared_lease_join.repo_id.clone(),
            revision: hf_shared_lease_join.revision.clone(),
            storage_class: hf_shared_lease_join.storage_class,
            lease_term_ms: hf_shared_lease_join.lease_term_ms,
            service_name: None,
            apartment_name: None,
        },
        HfLeaseLeavePayload,
        ["service_name", "apartment_name"],
        "HF lease-leave payload"
    );
    let hf_renew = HfLeaseRenewPayload {
        repo_id: hf_shared_lease_join.repo_id.clone(),
        revision: hf_shared_lease_join.revision.clone(),
        service_name: hf_shared_lease_join.service_name.clone(),
        apartment_name: None,
        storage_class: hf_shared_lease_join.storage_class,
        lease_term_ms: hf_shared_lease_join.lease_term_ms,
        lease_asset_definition_id: hf_shared_lease_join.lease_asset_definition_id.clone(),
        base_fee: hf_shared_lease_join.base_fee.clone(),
    };
    assert_required_nullable!(
        hf_renew.clone(),
        HfLeaseRenewPayload,
        ["apartment_name"],
        "HF lease-renew payload"
    );

    assert_required_nullable!(
        AgentPolicyRevokePayload {
            apartment_name: "ops_agent".to_owned(),
            capability: "agent.autonomy.run".to_owned(),
            reason: None,
        },
        AgentPolicyRevokePayload,
        ["reason"],
        "agent policy-revoke payload"
    );
    assert_required_nullable!(
        AgentArtifactAllowPayload {
            apartment_name: "ops_agent".to_owned(),
            artifact_hash: "hash:ABCD0123#01".to_owned(),
            provenance_hash: None,
        },
        AgentArtifactAllowPayload,
        ["provenance_hash"],
        "agent artifact-allow payload"
    );
    assert_required_nullable!(
        ModelWeightRegisterPayload {
            service_name: "web_portal".to_owned(),
            model_name: "model-1".to_owned(),
            weight_version: "1.0.0".to_owned(),
            training_job_id: "job-1".to_owned(),
            parent_version: None,
            weight_artifact_hash: Hash::new(b"weight-artifact"),
            dataset_ref: "dataset://synthetic/v2".to_owned(),
            training_config_hash: Hash::new(b"train-config"),
            reproducibility_hash: Hash::new(b"repro"),
            provenance_attestation_hash: Hash::new(b"attestation"),
        },
        ModelWeightRegisterPayload,
        ["parent_version"],
        "model-weight register payload"
    );
}
signature_payload_layout_case!(
    agent_deploy_signature_payload_layout_is_canonical_tuple,
    payload = AgentDeployPayload {
        manifest: fixture_agent_apartment(),
        lease_ticks: 120,
        autonomy_budget_units: 500,
    },
    encode_agent_deploy_signature_payload,
    (
        payload.manifest.clone(),
        payload.lease_ticks,
        payload.autonomy_budget_units,
    )
);
signature_payload_layout_case!(
    agent_lease_renew_signature_payload_layout_is_canonical_tuple,
    payload = AgentLeaseRenewPayload {
        apartment_name: "ops_agent".to_owned(),
        lease_ticks: 120,
    },
    encode_agent_lease_renew_signature_payload,
    (payload.apartment_name.as_str(), payload.lease_ticks)
);
signature_payload_layout_case!(
    agent_restart_signature_payload_layout_is_canonical_tuple,
    payload = AgentRestartPayload {
        apartment_name: "ops_agent".to_owned(),
        reason: "manual-restart".to_owned(),
    },
    encode_agent_restart_signature_payload,
    (payload.apartment_name.as_str(), payload.reason.as_str())
);
signature_payload_layout_case!(
    agent_policy_revoke_signature_payload_layout_is_canonical_tuple,
    payload = AgentPolicyRevokePayload {
        apartment_name: "ops_agent".to_owned(),
        capability: "agent.autonomy.run".to_owned(),
        reason: Some("manual-review".to_owned()),
    },
    encode_agent_policy_revoke_signature_payload,
    (
        payload.apartment_name.as_str(),
        payload.capability.as_str(),
        payload.reason.as_deref(),
    )
);
signature_payload_layout_case!(
    agent_wallet_spend_signature_payload_layout_is_canonical_tuple,
    payload = AgentWalletSpendPayload {
        apartment_name: "ops_agent".to_owned(),
        request_id: "wallet-spend-wide-amount-1".to_owned(),
        asset_definition: "61CtjvNd9T3THAR65GsMVHr82Bjc".to_owned(),
        amount: "340282366920938463463374607431768211456.0000000001"
            .parse()
            .expect("wide exact amount"),
    },
    encode_agent_wallet_spend_signature_payload,
    (
        payload.apartment_name.as_str(),
        payload.request_id.as_str(),
        payload.asset_definition.as_str(),
        payload.amount.clone(),
    ),
    {
        let value = json::to_value(&payload).expect("serialize wallet spend payload");
        let object = value.as_object().expect("wallet spend payload object");
        assert_eq!(
            object.get("amount").and_then(norito::json::Value::as_str),
            Some("340282366920938463463374607431768211456.0000000001")
        );
        assert!(!object.contains_key("amount_nanos"));
    }
);
signature_payload_layout_case!(
    agent_wallet_approve_signature_payload_layout_is_canonical_tuple,
    payload = AgentWalletApprovePayload {
        apartment_name: "ops_agent".to_owned(),
        request_id: "ops_agent:wallet:7".to_owned(),
    },
    encode_agent_wallet_approve_signature_payload,
    (payload.apartment_name.as_str(), payload.request_id.as_str())
);
signature_payload_layout_case!(
    agent_message_send_signature_payload_layout_is_canonical_tuple,
    payload = AgentMessageSendPayload {
        from_apartment: "ops_agent".to_owned(),
        to_apartment: "worker_agent".to_owned(),
        channel: "ops.sync".to_owned(),
        payload: "rotate-key-42".to_owned(),
    },
    encode_agent_message_send_signature_payload,
    (
        payload.from_apartment.as_str(),
        payload.to_apartment.as_str(),
        payload.channel.as_str(),
        payload.payload.as_str(),
    )
);
signature_payload_layout_case!(
    agent_message_ack_signature_payload_layout_is_canonical_tuple,
    payload = AgentMessageAckPayload {
        apartment_name: "worker_agent".to_owned(),
        message_id: "worker_agent:mail:3".to_owned(),
    },
    encode_agent_message_ack_signature_payload,
    (payload.apartment_name.as_str(), payload.message_id.as_str())
);
signature_payload_layout_case!(
    agent_artifact_allow_signature_payload_layout_is_canonical_tuple,
    payload = AgentArtifactAllowPayload {
        apartment_name: "ops_agent".to_owned(),
        artifact_hash: "hash:ABCD0123#01".to_owned(),
        provenance_hash: Some("hash:PROV0001#01".to_owned()),
    },
    encode_agent_artifact_allow_signature_payload,
    (
        payload.apartment_name.as_str(),
        payload.artifact_hash.as_str(),
        payload.provenance_hash.as_deref(),
    )
);
#[test]
fn agent_workflow_input_json_parser_requires_exact_canonical_bytes() {
    let canonical = "{\"inputs\":[\"alpha\",\"beta\"],\"parameters\":{\"max_new_tokens\":4}}";
    assert_eq!(
        parse_exact_agent_workflow_input_json(canonical)
            .expect("accept canonical workflow input JSON"),
        canonical
    );
    for noncanonical in [
        "{\n  \"inputs\": [\"alpha\", \"beta\"],\n  \"parameters\": {\"max_new_tokens\": 4}\n}",
        " {\"inputs\":[]}",
        "{\"b\":1,\"a\":2}",
    ] {
        let _error = parse_exact_agent_workflow_input_json(noncanonical)
            .expect_err("non-canonical workflow JSON bytes must be rejected");
    }
}
#[test]
fn hf_repo_id_argument_requires_one_exact_fully_qualified_identity() {
    assert_eq!(
        parse_hf_repo_id_arg("OpenAI/GPT-OSS").expect("canonical repository identity"),
        "OpenAI/GPT-OSS"
    );
    for noncanonical in ["GPT-OSS", "OpenAI//GPT-OSS", " OpenAI/GPT-OSS"] {
        let _error = parse_hf_repo_id_arg(noncanonical)
            .expect_err("repository aliases and whitespace must be rejected");
    }
}
#[test]
fn hf_revision_argument_requires_full_lowercase_commit_oid() {
    assert_eq!(
        parse_hf_revision_arg(TEST_HF_COMMIT_OID).expect("canonical commit OID"),
        TEST_HF_COMMIT_OID
    );
    for revision in [
        "main",
        "rev-1",
        "01234567",
        "0123456789ABCDEF0123456789ABCDEF01234567",
    ] {
        let _error = parse_hf_revision_arg(revision)
            .expect_err("mutable or noncanonical revision must fail");
    }
}
#[test]
fn hf_shared_lease_join_signature_payload_layout_is_canonical_tuple() {
    let asset_definition = hf_shared_lease_asset_definition();
    let payload = HfSharedLeaseJoinPayload {
        repo_id: "openai/gpt-oss".to_owned(),
        revision: TEST_HF_COMMIT_OID.to_owned(),
        service_name: "hf_lease_a".to_owned(),
        apartment_name: Some("ops_agent".to_owned()),
        storage_class: StorageClass::Warm,
        lease_term_ms: 604_800_000,
        lease_asset_definition_id: asset_definition.clone(),
        base_fee: "0.0000000001".parse().expect("sub-nano exact base fee"),
    };
    let encoded =
        encode_hf_shared_lease_join_signature_payload(&payload).expect("encode signature payload");
    let expected = norito::to_bytes(&(
        payload.repo_id.as_str(),
        TEST_HF_COMMIT_OID,
        payload.service_name.as_str(),
        payload.apartment_name.as_deref(),
        payload.storage_class,
        payload.lease_term_ms,
        asset_definition,
        payload.base_fee.clone(),
    ))
    .expect("encode canonical tuple");
    assert_eq!(encoded, expected);
    let value = json::to_value(&payload).expect("serialize HF shared-lease join payload");
    let object = value
        .as_object()
        .expect("HF shared-lease join payload object");
    assert_eq!(
        object.get("base_fee").and_then(norito::json::Value::as_str),
        Some("0.0000000001")
    );
    assert!(!object.contains_key("base_fee_nanos"));
}
#[test]
fn hf_lease_leave_signature_payload_layout_is_canonical_tuple() {
    let payload = HfLeaseLeavePayload {
        repo_id: "openai/gpt-oss".to_owned(),
        revision: TEST_HF_COMMIT_OID.to_owned(),
        storage_class: StorageClass::Warm,
        lease_term_ms: 604_800_000,
        service_name: Some("hf_lease_a".to_owned()),
        apartment_name: Some("ops_agent".to_owned()),
    };
    let encoded =
        encode_hf_lease_leave_signature_payload(&payload).expect("encode signature payload");
    let expected = norito::to_bytes(&(
        payload.repo_id.as_str(),
        TEST_HF_COMMIT_OID,
        payload.storage_class,
        payload.lease_term_ms,
        payload.service_name.as_deref(),
        payload.apartment_name.as_deref(),
    ))
    .expect("encode canonical tuple");
    assert_eq!(encoded, expected);
}
#[test]
fn hf_lease_renew_signature_payload_layout_is_canonical_tuple() {
    let asset_definition = hf_shared_lease_asset_definition();
    let payload = HfLeaseRenewPayload {
        repo_id: "openai/gpt-oss".to_owned(),
        revision: TEST_HF_COMMIT_OID.to_owned(),
        service_name: "hf_lease_renew".to_owned(),
        apartment_name: Some("ops_agent".to_owned()),
        storage_class: StorageClass::Warm,
        lease_term_ms: 604_800_000,
        lease_asset_definition_id: asset_definition.clone(),
        base_fee: "340282366920938463463374607431768211456.0000000001"
            .parse()
            .expect("wide exact base fee"),
    };
    let encoded =
        encode_hf_lease_renew_signature_payload(&payload).expect("encode signature payload");
    let expected = norito::to_bytes(&(
        payload.repo_id.as_str(),
        TEST_HF_COMMIT_OID,
        payload.service_name.as_str(),
        payload.apartment_name.as_deref(),
        payload.storage_class,
        payload.lease_term_ms,
        asset_definition,
        payload.base_fee.clone(),
    ))
    .expect("encode canonical tuple");
    assert_eq!(encoded, expected);
}
#[test]
fn training_job_start_signature_payload_layout_is_canonical_tuple() {
    let payload = TrainingJobStartPayload {
        service_name: "web_portal".to_owned(),
        model_name: "model-1".to_owned(),
        job_id: "job-1".to_owned(),
        worker_group_size: 4,
        target_steps: 100,
        checkpoint_interval_steps: 20,
        max_retries: 3,
        step_compute_units: 500,
        compute_budget_units: 50_000,
        storage_budget_bytes: 4_096,
    };
    let encoded =
        encode_training_job_start_signature_payload(&payload).expect("encode signature payload");
    let expected = norito::to_bytes(&(
        payload.service_name.as_str(),
        payload.model_name.as_str(),
        payload.job_id.as_str(),
        payload.worker_group_size,
        payload.target_steps,
        payload.checkpoint_interval_steps,
        payload.max_retries,
        payload.step_compute_units,
        payload.compute_budget_units,
        payload.storage_budget_bytes,
    ))
    .expect("encode canonical tuple");
    assert_eq!(encoded, expected);
}
#[test]
fn training_job_checkpoint_signature_payload_layout_is_canonical_tuple() {
    let metrics_hash = Hash::new(b"metrics");
    let payload = TrainingJobCheckpointPayload {
        service_name: "web_portal".to_owned(),
        job_id: "job-1".to_owned(),
        completed_step: 20,
        checkpoint_size_bytes: 1_024,
        metrics_hash: metrics_hash.clone(),
    };
    let encoded = encode_training_job_checkpoint_signature_payload(&payload)
        .expect("encode signature payload");
    let expected = norito::to_bytes(&(
        payload.service_name.as_str(),
        payload.job_id.as_str(),
        payload.completed_step,
        payload.checkpoint_size_bytes,
        metrics_hash,
    ))
    .expect("encode canonical tuple");
    assert_eq!(encoded, expected);
}
#[test]
fn training_job_retry_signature_payload_layout_is_canonical_tuple() {
    let payload = TrainingJobRetryPayload {
        service_name: "web_portal".to_owned(),
        job_id: "job-1".to_owned(),
        reason: "worker unavailable".to_owned(),
    };
    let encoded =
        encode_training_job_retry_signature_payload(&payload).expect("encode signature payload");
    let expected = norito::to_bytes(&(
        payload.service_name.as_str(),
        payload.job_id.as_str(),
        payload.reason.as_str(),
    ))
    .expect("encode canonical tuple");
    assert_eq!(encoded, expected);
}
#[test]
fn model_artifact_register_signature_payload_layout_is_canonical_tuple() {
    let weight_artifact_hash = Hash::new(b"weight-artifact");
    let training_config_hash = Hash::new(b"train-config");
    let reproducibility_hash = Hash::new(b"repro");
    let provenance_attestation_hash = Hash::new(b"attestation");
    let payload = ModelArtifactRegisterPayload {
        service_name: "web_portal".to_owned(),
        model_name: "model-1".to_owned(),
        training_job_id: "job-1".to_owned(),
        weight_artifact_hash: weight_artifact_hash.clone(),
        dataset_ref: "dataset://synthetic/v2".to_owned(),
        training_config_hash: training_config_hash.clone(),
        reproducibility_hash: reproducibility_hash.clone(),
        provenance_attestation_hash: provenance_attestation_hash.clone(),
    };
    let encoded = encode_model_artifact_register_signature_payload(&payload)
        .expect("encode signature payload");
    let expected = norito::to_bytes(&(
        payload.service_name.as_str(),
        payload.model_name.as_str(),
        payload.training_job_id.as_str(),
        weight_artifact_hash,
        payload.dataset_ref.as_str(),
        training_config_hash,
        reproducibility_hash,
        provenance_attestation_hash,
    ))
    .expect("encode canonical tuple");
    assert_eq!(encoded, expected);
}
#[test]
fn model_weight_register_signature_payload_layout_is_canonical_tuple() {
    let weight_artifact_hash = Hash::new(b"weight-artifact");
    let training_config_hash = Hash::new(b"train-config");
    let reproducibility_hash = Hash::new(b"repro");
    let provenance_attestation_hash = Hash::new(b"attestation");
    let payload = ModelWeightRegisterPayload {
        service_name: "web_portal".to_owned(),
        model_name: "model-1".to_owned(),
        weight_version: "1.0.0".to_owned(),
        training_job_id: "job-1".to_owned(),
        parent_version: Some("0.9.0".to_owned()),
        weight_artifact_hash: weight_artifact_hash.clone(),
        dataset_ref: "dataset://synthetic/v2".to_owned(),
        training_config_hash: training_config_hash.clone(),
        reproducibility_hash: reproducibility_hash.clone(),
        provenance_attestation_hash: provenance_attestation_hash.clone(),
    };
    let encoded =
        encode_model_weight_register_signature_payload(&payload).expect("encode signature payload");
    let expected = norito::to_bytes(&(
        payload.service_name.as_str(),
        payload.model_name.as_str(),
        payload.weight_version.as_str(),
        payload.training_job_id.as_str(),
        payload.parent_version.as_deref(),
        weight_artifact_hash,
        payload.dataset_ref.as_str(),
        training_config_hash,
        reproducibility_hash,
        provenance_attestation_hash,
    ))
    .expect("encode canonical tuple");
    assert_eq!(encoded, expected);
}
#[test]
fn model_weight_promote_signature_payload_layout_is_canonical_tuple() {
    let gate_report_hash = Hash::new(b"gate-report");
    let payload = ModelWeightPromotePayload {
        service_name: "web_portal".to_owned(),
        model_name: "model-1".to_owned(),
        weight_version: "1.0.0".to_owned(),
        gate_approved: true,
        gate_report_hash: gate_report_hash.clone(),
    };
    let encoded =
        encode_model_weight_promote_signature_payload(&payload).expect("encode signature payload");
    let expected = norito::to_bytes(&(
        payload.service_name.as_str(),
        payload.model_name.as_str(),
        payload.weight_version.as_str(),
        payload.gate_approved,
        gate_report_hash,
    ))
    .expect("encode canonical tuple");
    assert_eq!(encoded, expected);
}
#[test]
fn model_weight_rollback_signature_payload_layout_is_canonical_tuple() {
    let payload = ModelWeightRollbackPayload {
        service_name: "web_portal".to_owned(),
        model_name: "model-1".to_owned(),
        target_version: "0.9.0".to_owned(),
        reason: "gate regression".to_owned(),
    };
    let encoded =
        encode_model_weight_rollback_signature_payload(&payload).expect("encode signature payload");
    let expected = norito::to_bytes(&(
        payload.service_name.as_str(),
        payload.model_name.as_str(),
        payload.target_version.as_str(),
        payload.reason.as_str(),
    ))
    .expect("encode canonical tuple");
    assert_eq!(encoded, expected);
}
#[test]
fn signed_uploaded_model_register_request_uses_verifiable_signatures() {
    let key_pair = soracloud_fixture_key_pair(0x3B);
    let authority = AccountId::new(key_pair.public_key().clone());
    let bundle = sample_uploaded_model_bundle();
    let finalize = sample_uploaded_model_finalize_payload();
    let request = signed_uploaded_model_register_request(bundle, finalize, &authority, &key_pair)
        .expect("signed uploaded-model register request");
    let bundle_payload =
        encode_uploaded_model_bundle_register_provenance_payload(request.payload.bundle.clone())
            .expect("encode bundle payload");
    request
        .bundle_provenance
        .signature
        .verify(&request.bundle_provenance.signer, &bundle_payload)
        .expect("bundle signature should verify");
    let finalize_payload = encode_uploaded_model_finalize_provenance_payload(
        request.payload.bundle.service_name.as_ref(),
        request.payload.model_name.as_str(),
        request.payload.bundle.model_id.as_str(),
        request.payload.artifact_id.as_str(),
        request.payload.bundle.weight_version.as_str(),
        request.payload.bundle.bundle_root,
        request.payload.weight_artifact_hash,
        request.payload.dataset_ref.as_str(),
        request.payload.training_config_hash,
        request.payload.reproducibility_hash,
        request.payload.provenance_attestation_hash,
    )
    .expect("encode finalize payload");
    request
        .finalize_provenance
        .signature
        .verify(&request.finalize_provenance.signer, &finalize_payload)
        .expect("finalize signature should verify");
    assert_request_has_no_inline_signing_fields(&request);
}
#[test]
fn uploaded_model_register_service_name_override_updates_bundle_and_finalize() {
    let key_pair = soracloud_fixture_key_pair(0x3C);
    let authority = AccountId::new(key_pair.public_key().clone());
    let mut bundle = sample_uploaded_model_bundle();
    let mut finalize = sample_uploaded_model_finalize_payload();
    bundle.service_name = "original_models".parse().expect("original service name");
    finalize.service_name = "original_models".to_owned();
    apply_uploaded_model_register_service_name_override(
        &mut bundle,
        &mut finalize,
        Some("resolved_models"),
    )
    .expect("apply service-name override");
    assert_eq!(bundle.service_name.as_ref(), "resolved_models");
    assert_eq!(finalize.service_name, "resolved_models");
    let request = signed_uploaded_model_register_request(bundle, finalize, &authority, &key_pair)
        .expect("override should keep register request signable");
    assert_eq!(
        request.payload.bundle.service_name.as_ref(),
        "resolved_models"
    );
}
#[test]
fn signed_uploaded_model_register_request_rejects_mismatched_finalize_fields() {
    let bundle = sample_uploaded_model_bundle();
    let mut finalize = sample_uploaded_model_finalize_payload();
    finalize.service_name = "other_models".to_owned();
    let error = uploaded_model_register_validation_error(bundle.clone(), finalize);
    assert!(error.contains("must match bundle service_name"));
    let mut finalize = sample_uploaded_model_finalize_payload();
    finalize.model_id = "other-upload".to_owned();
    let error = uploaded_model_register_validation_error(bundle.clone(), finalize);
    assert!(error.contains("must match bundle model_id"));
    let mut finalize = sample_uploaded_model_finalize_payload();
    finalize.weight_version = "2.0.0".to_owned();
    let error = uploaded_model_register_validation_error(bundle.clone(), finalize);
    assert!(error.contains("must match bundle weight_version"));
    let mut finalize = sample_uploaded_model_finalize_payload();
    finalize.bundle_root = Hash::new(b"tampered-bundle-root");
    let error = uploaded_model_register_validation_error(bundle, finalize);
    assert!(error.contains("bundle_root must match"));
}
#[test]
fn signed_uploaded_model_register_request_rejects_noncanonical_finalize_fields() {
    let bundle = sample_uploaded_model_bundle();
    let mut finalize = sample_uploaded_model_finalize_payload();
    finalize.model_name = " \t ".to_owned();
    let error = uploaded_model_register_validation_error(bundle.clone(), finalize);
    assert!(error.contains("register model_name"));
    let mut finalize = sample_uploaded_model_finalize_payload();
    finalize.artifact_id.clear();
    let error = uploaded_model_register_validation_error(bundle.clone(), finalize);
    assert!(error.contains("artifact_id must not be empty"));
    let mut finalize = sample_uploaded_model_finalize_payload();
    finalize.dataset_ref = "\n".to_owned();
    let error = uploaded_model_register_validation_error(bundle, finalize);
    assert!(error.contains("dataset_ref must not contain surrounding whitespace"));

    let bundle = sample_uploaded_model_bundle();
    let mutations: [fn(&mut UploadedModelFinalizePayload); 4] = [
        |finalize: &mut UploadedModelFinalizePayload| {
            finalize.service_name = format!(" {}", finalize.service_name);
        },
        |finalize: &mut UploadedModelFinalizePayload| {
            finalize.model_name = format!("{} ", finalize.model_name);
        },
        |finalize: &mut UploadedModelFinalizePayload| {
            finalize.artifact_id = format!(" {}", finalize.artifact_id);
        },
        |finalize: &mut UploadedModelFinalizePayload| {
            finalize.dataset_ref = format!("{} ", finalize.dataset_ref);
        },
    ];
    for mutate in mutations {
        let mut finalize = sample_uploaded_model_finalize_payload();
        mutate(&mut finalize);
        let error = uploaded_model_register_validation_error(bundle.clone(), finalize);
        assert!(
            error.contains("exact canonical")
                || error.contains("surrounding whitespace")
                || error.contains("only ASCII")
        );
    }
}
#[test]
fn fetch_uploaded_model_status_rejects_invalid_url() {
    let err = fetch_torii_soracloud_uploaded_model_status(
        "not-a-url",
        "v1/soracloud/model/upload/status",
        "private_models",
        "1.0.0",
        Some("upload-1"),
        None,
        None,
        None,
        5,
    )
    .expect_err("invalid URL must fail");
    assert!(err.to_string().contains("invalid --torii-url"));
}
#[test]
fn fetch_uploaded_model_status_rejects_blank_model_name_before_network() {
    let err = fetch_torii_soracloud_uploaded_model_status(
        "http://127.0.0.1:1",
        "v1/soracloud/model/upload/status",
        "private_models",
        "1.0.0",
        None,
        Some(" "),
        None,
        None,
        5,
    )
    .expect_err("blank uploaded-model name must fail locally");
    assert!(err.to_string().contains("invalid --model-name"));
}
include!("../network_auth_tests.rs");
#[test]
fn soracloud_signature_timestamp_fails_closed() {
    let before_epoch = UNIX_EPOCH
        .checked_sub(Duration::from_millis(1))
        .expect("representable pre-epoch timestamp");
    let error = soracloud_signature_timestamp_ms(before_epoch)
        .expect_err("pre-epoch request signatures must fail");
    assert!(error.to_string().contains("precedes the Unix epoch"));

    assert_eq!(
        soracloud_signature_timestamp_ms(
            UNIX_EPOCH
                .checked_add(Duration::from_millis(1_234))
                .expect("representable timestamp"),
        )
        .expect("canonical timestamp"),
        1_234
    );

    let error = soracloud_signature_timestamp_ms_from_elapsed(Duration::new(u64::MAX, 0))
        .expect_err("millisecond overflow must fail");
    assert!(error.to_string().contains("exceeds u64 milliseconds"));
}
#[test]
fn sorafs_retry_after_partial_failure_reuses_exact_release_manifest_identities() {
    fn manifests_for_release(
        payloads: &[&[u8]],
        key_pair: &KeyPair,
        release_identity: SorafsReleaseIdentityV1,
    ) -> Vec<BuiltSorafsManifest> {
        let descriptor = chunker_registry::default_descriptor();
        payloads
            .iter()
            .map(|payload| {
                let plan = CarBuildPlan::single_file_with_profile(payload, descriptor.profile)
                    .expect("build test release CAR plan");
                build_sorafs_artifact_manifest(
                    &plan,
                    payload,
                    descriptor,
                    key_pair,
                    release_identity,
                    SorafsManifestBuildLabels {
                        writer: "prepare test release CAR writer",
                        metadata: "compute test release CAR metadata",
                        root: "test release CAR root",
                        por: "compute test release PoR root",
                        manifest: "build test release manifest",
                        governance: "attach test release governance",
                        encoding: "encode test release manifest",
                        digest: "digest test release manifest",
                    },
                )
                .expect("build deterministic test release manifest")
            })
            .collect()
    }

    let payloads: [&[u8]; 3] = [
        b"release-bundle-v1",
        b"release-guest-v1",
        b"release-discovery-v1",
    ];
    let key_pair = soracloud_fixture_key_pair(0x7D);
    let release_identity = SorafsReleaseIdentityV1::new(test_sorafs_retention_epoch());
    let first_attempt = manifests_for_release(&payloads, &key_pair, release_identity);
    let authority = AccountId::new(key_pair.public_key().clone());
    install_mock_submission_config(&authority, &key_pair);
    let server = MockHttpServer::start(BTreeMap::from([(
        "/v1/sorafs/pin/register".to_owned(),
        MockHttpResponse::json(
            json::to_vec(&norito::json!({ "ok": true })).expect("encode retry pin response"),
        ),
    )]));
    let failed_after = 2;
    let first_error = first_attempt
        .iter()
        .enumerate()
        .try_for_each(|(index, built)| {
            if index == failed_after {
                return Err(eyre!("simulated release failure after {failed_after} pins"));
            }
            register_built_sorafs_manifest(
                built,
                "partial release fixture",
                &server.base_url,
                &authority,
                &key_pair,
                5,
            )
        })
        .expect_err("first attempt must stop at the injected failure boundary");
    assert!(
        first_error
            .to_string()
            .contains("simulated release failure")
    );

    let retry = manifests_for_release(&payloads, &key_pair, release_identity);
    assert_eq!(
        retry
            .iter()
            .map(|built| (&built.bytes, &built.digest_hex))
            .collect::<Vec<_>>(),
        first_attempt
            .iter()
            .map(|built| (&built.bytes, &built.digest_hex))
            .collect::<Vec<_>>(),
        "retry must reproduce byte-identical manifests and digests"
    );
    for built in &retry {
        assert_eq!(
            built.manifest.pin_policy.retention_epoch,
            release_identity.retention_epoch()
        );
        validate_sorafs_release_identity(&built.manifest, release_identity, "retry fixture")
            .expect("release metadata and pin policy must carry the same identity");
    }
    let mut extra_metadata = retry[0].manifest.clone();
    extra_metadata.metadata.push(MetadataEntry {
        key: "soracloud.unexpected_metadata".to_owned(),
        value: "forbidden".to_owned(),
    });
    let _error = validate_sorafs_release_identity(
        &extra_metadata,
        release_identity,
        "retry fixture with extra metadata",
    )
    .expect_err("first-release artifact metadata must be a closed deterministic projection");
    for built in &retry {
        register_built_sorafs_manifest(
            built,
            "retried release fixture",
            &server.base_url,
            &authority,
            &key_pair,
            5,
        )
        .expect("retry must reuse approved pins and finish only the missing pin");
    }
    let registrations = server
        .requests()
        .into_iter()
        .filter(|request| request.method == "POST" && request.path == "/v1/sorafs/pin/register")
        .map(|request| {
            mock_sorafs_pin_registration(&request)
                .expect("retry pin request must be canonical")
                .manifest_digest_hex
        })
        .collect::<Vec<_>>();
    assert_eq!(
        registrations.len(),
        first_attempt.len(),
        "retry must not repost either manifest already accepted before failure"
    );
    assert_eq!(
        registrations.into_iter().collect::<BTreeSet<_>>(),
        first_attempt
            .iter()
            .map(|built| built.digest_hex.clone())
            .collect::<BTreeSet<_>>(),
        "retry may finish missing pins but must not create any new manifest identity"
    );
}
#[test]
fn build_soracloud_mutation_auth_headers_rejects_witness_account_mismatch() {
    let config = crate::fallback_config();
    let endpoint =
        reqwest::Url::parse("http://127.0.0.1:8080/v1/soracloud/deploy").expect("endpoint");
    let body = br#"{"noop":true}"#;
    let other_account = AccountId::new(soracloud_fixture_key_pair(0x3D).public_key().clone());
    let witness = CanonicalRequestWitnessV1 {
        schema_version: CANONICAL_REQUEST_WITNESS_VERSION_V1,
        subject_account: other_account,
        timestamp_ms: 42,
        nonce: "fixture-witness".to_owned(),
        canonical_request_hash: canonical_network_request_hash(
            &config.network_id,
            &reqwest::Method::POST,
            &endpoint,
            body,
        )
        .expect("bounded canonical witness request hash"),
        signatures: Vec::new(),
    };
    let dir = temp_dir("witness_account_mismatch");
    let witness_path = dir.join("witness.json");
    fs::write(
        &witness_path,
        json::to_vec(&witness).expect("encode witness json"),
    )
    .expect("write witness file");
    let err = build_soracloud_mutation_auth_headers(&config, Some(&witness_path), &endpoint, body)
        .expect_err("mismatched witness account must fail");
    assert!(err.to_string().contains("subject_account"));
}
#[test]
fn build_soracloud_mutation_auth_headers_rejects_witness_hash_mismatch() {
    let config = crate::fallback_config();
    let endpoint =
        reqwest::Url::parse("http://127.0.0.1:8080/v1/soracloud/deploy").expect("endpoint");
    let body = br#"{"noop":true}"#;
    let witness = CanonicalRequestWitnessV1 {
        schema_version: CANONICAL_REQUEST_WITNESS_VERSION_V1,
        subject_account: config.account.clone(),
        timestamp_ms: 42,
        nonce: "fixture-witness".to_owned(),
        canonical_request_hash: Hash::new(b"wrong-hash"),
        signatures: Vec::new(),
    };
    let dir = temp_dir("witness_hash_mismatch");
    let witness_path = dir.join("witness.json");
    fs::write(
        &witness_path,
        json::to_vec(&witness).expect("encode witness json"),
    )
    .expect("write witness file");
    let err = build_soracloud_mutation_auth_headers(&config, Some(&witness_path), &endpoint, body)
        .expect_err("mismatched witness hash must fail");
    assert!(err.to_string().contains("canonical_request_hash"));
}
#[test]
fn deploy_requires_torii_url_after_local_simulator_removal() {
    let dir = temp_dir("deploy_requires_torii");
    let container_path = dir.join("container_manifest.json");
    let service_path = dir.join("service_manifest.json");
    let container = fixture_container();
    let mut service = fixture_service();
    service.container.manifest_hash = Hash::new(Encode::encode(&container));
    write_json(&container_path, &container).expect("write container manifest");
    write_json(&service_path, &service).expect("write service manifest");
    let key_pair = soracloud_fixture_key_pair(0x3E);
    let authority = AccountId::new(key_pair.public_key().clone());
    let err = DeployArgs {
        container: container_path,
        service: service_path,
        bundle_file: dir.join("service.tgz"),
        sorafs_retention_epoch: test_sorafs_retention_epoch(),
        initial_configs: None,
        initial_secrets: None,
        inrou_preseed_receipt: None,
        torii_url: None,
        api_token: None,
        timeout_secs: 10,
    }
    .run(MutationMode::Deploy, &authority, &key_pair)
    .expect_err("deploy without torii should fail");
    assert!(err.to_string().contains("--torii-url is required"));
}
#[test]
fn deploy_returns_manifest_backed_service_projection() {
    let (dir, _) = service_fixture("deploy_service_projection", InitTemplate::HttpService);
    let bundle_file = prepare_http_service_bundle(&dir, "deploy");
    let key_pair = soracloud_fixture_key_pair(0x20);
    let authority = AccountId::new(key_pair.public_key().clone());
    let deploy_response = mock_soracloud_draft_response(&authority, &key_pair);
    let status_payload = mock_control_plane_status_payload(&["echo_console"]);
    let server = mock_bundle_mutation_server(
        &dir,
        "/v1/soracloud/deploy",
        &deploy_response,
        &status_payload,
        "encode public discovery pin register response",
        "encode deploy response",
    );
    install_mock_submission_config(&authority, &key_pair);
    let inrou_preseed_receipt = qualify_test_inrou_service(&dir, &bundle_file, &key_pair, "deploy");
    let output = DeployArgs {
        container: dir.join("container_manifest.json"),
        service: dir.join("service_manifest.json"),
        bundle_file,
        sorafs_retention_epoch: test_sorafs_retention_epoch(),
        initial_configs: None,
        initial_secrets: None,
        inrou_preseed_receipt: Some(inrou_preseed_receipt),
        torii_url: Some(server.base_url.clone()),
        api_token: Some("token".to_owned()),
        timeout_secs: 5,
    }
    .run(MutationMode::Deploy, &authority, &key_pair)
    .expect("deploy should succeed");
    assert_eq!(output.service_name, "echo_console");
    assert_eq!(output.mode, "Deploy");
    assert_eq!(output.execution_plane, "HttpService");
    assert_eq!(output.runtime, "Inrou");
    assert_eq!(output.route_path_prefix.as_deref(), Some("/api/v1"));
    assert_eq!(output.lease_volume_count, 2);
    assert_eq!(output.torii_url, server.base_url);
    assert!(output.uses_api_token);
    assert_eq!(output.published_inrou_guest_images.len(), 2);
    assert!(output.workspace_dir.contains("deploy_service_projection"));
    assert_optional_path_ends_with(output.workspace_scripts.local_dev.as_deref(), "dev.sh");
    assert!(
        output
            .routes
            .iter()
            .any(|route| route.route_kind == "hosted_http_prefix" && route.path == "/api/v1")
    );
    assert_eq!(
        output
            .response
            .get("service_name")
            .and_then(norito::json::Value::as_str),
        Some("echo_console")
    );
    assert_eq!(
        output
            .response
            .get("current_version")
            .and_then(norito::json::Value::as_str),
        Some("1.0.0")
    );
    let deploy_request = server
        .requests()
        .into_iter()
        .find(|request| request.method == "POST" && request.path == "/v1/soracloud/deploy")
        .expect("capture admitted deploy request");
    let request: SignedBundleRequest =
        json::from_slice(&deploy_request.body).expect("decode exact signed deployment");
    assert_eq!(
        request.precondition,
        SoraServiceMutationPreconditionV1::ServiceAbsent
    );
    let body: Value = json::from_slice(&deploy_request.body).expect("decode deploy request");
    for guest_isa in ["x86_64", "aarch64"] {
        let artifact = body
            .pointer(&format!(
                "/bundle/container/inrou/guest_images/{guest_isa}/published_artifact"
            ))
            .and_then(Value::as_object)
            .unwrap_or_else(|| {
                panic!("published deploy request must contain a concrete artifact for {guest_isa}")
            });
        assert!(artifact.contains_key("manifest_digest_hex"));
        assert!(!artifact.contains_key("manifest_id_hex"));
    }
    assert_notes_contain(&output.notes, "live Torii status");
}
#[test]
fn deploy_guest_publication_failure_never_signs_or_submits_a_service_mutation() {
    let (dir, _) = service_fixture("deploy_missing_guest_member", InitTemplate::HttpService);
    let bundle_file = prepare_http_service_bundle(&dir, "deploy-missing-guest");
    let key_pair = soracloud_fixture_key_pair(0x7A);
    let inrou_preseed_receipt =
        qualify_test_inrou_service(&dir, &bundle_file, &key_pair, "deploy-missing-guest");
    fs::remove_file(dir.join("http-service/inrou/aarch64/rootfs.ext4"))
        .expect("remove one required guest-image member");
    let authority = AccountId::new(key_pair.public_key().clone());
    let draft_response = mock_soracloud_draft_response(&authority, &key_pair);
    let status_payload = mock_control_plane_status_payload(&["echo_console"]);
    let server = mock_bundle_mutation_server(
        &dir,
        "/v1/soracloud/deploy",
        &draft_response,
        &status_payload,
        "encode pin register response",
        "encode deploy response",
    );
    install_mock_submission_config(&authority, &key_pair);
    let error = DeployArgs {
        container: dir.join("container_manifest.json"),
        service: dir.join("service_manifest.json"),
        bundle_file,
        sorafs_retention_epoch: test_sorafs_retention_epoch(),
        initial_configs: None,
        initial_secrets: None,
        inrou_preseed_receipt: Some(inrou_preseed_receipt),
        torii_url: Some(server.base_url.clone()),
        api_token: None,
        timeout_secs: 5,
    }
    .run(MutationMode::Deploy, &authority, &key_pair)
    .expect_err("missing guest member must stop publication before mutation signing");
    assert!(
        format!("{error:#}").contains("missing Inrou guest image member"),
        "unexpected publication error: {error:#}"
    );
    assert!(
        server.requests().is_empty(),
        "guest-image preflight must fail before bundle pin, registration, signing, or mutation"
    );
}
#[test]
fn deploy_nonportable_inrou_entrypoint_fails_before_any_upload() {
    let (dir, _) = service_fixture("deploy_nonportable_entrypoint", InitTemplate::HttpService);
    let bundle_file = prepare_http_service_bundle(&dir, "deploy-nonportable-entrypoint");
    let container_path = dir.join("container_manifest.json");
    let service_path = dir.join("service_manifest.json");
    let mut container: UnpublishedContainerManifestV1 =
        load_json(&container_path).expect("load unpublished container workspace");
    container.entrypoint = "/app\\server.mjs".to_owned();
    let mut service: SoraServiceManifestV1 =
        load_json(&service_path).expect("load unpublished service manifest");
    service.container.manifest_hash = container
        .workspace_hash()
        .expect("hash malformed unpublished workspace exactly");
    write_json(&container_path, &container).expect("write malformed container workspace");
    write_json(&service_path, &service).expect("write linked service manifest");

    let key_pair = soracloud_fixture_key_pair(0x7B);
    let authority = AccountId::new(key_pair.public_key().clone());
    let draft_response = mock_soracloud_draft_response(&authority, &key_pair);
    let status_payload = mock_control_plane_status_payload(&["echo_console"]);
    let server = mock_bundle_mutation_server(
        &dir,
        "/v1/soracloud/deploy",
        &draft_response,
        &status_payload,
        "encode pin register response",
        "encode deploy response",
    );
    install_mock_submission_config(&authority, &key_pair);
    let error = DeployArgs {
        container: container_path,
        service: service_path,
        bundle_file,
        sorafs_retention_epoch: test_sorafs_retention_epoch(),
        initial_configs: None,
        initial_secrets: None,
        inrou_preseed_receipt: None,
        torii_url: Some(server.base_url.clone()),
        api_token: None,
        timeout_secs: 5,
    }
    .run(MutationMode::Deploy, &authority, &key_pair)
    .expect_err("nonportable Inrou entrypoint must fail before publication");
    assert!(
        format!("{error:#}").contains("entrypoint"),
        "unexpected entrypoint validation error: {error:#}"
    );
    assert!(
        server.requests().is_empty(),
        "entrypoint preflight must fail before every SoraFS and Torii request"
    );
}
#[test]
fn deploy_empty_inrou_guest_image_fails_before_any_upload() {
    let (dir, _) = service_fixture("deploy_empty_guest_image", InitTemplate::HttpService);
    let bundle_file = prepare_http_service_bundle(&dir, "deploy-empty-guest-image");
    let key_pair = soracloud_fixture_key_pair(0x7C);
    let inrou_preseed_receipt =
        qualify_test_inrou_service(&dir, &bundle_file, &key_pair, "deploy-empty-guest-image");
    fs::write(dir.join("http-service/inrou/aarch64/rootfs.ext4"), b"")
        .expect("truncate one required guest-image member");
    let authority = AccountId::new(key_pair.public_key().clone());
    let draft_response = mock_soracloud_draft_response(&authority, &key_pair);
    let status_payload = mock_control_plane_status_payload(&["echo_console"]);
    let server = mock_bundle_mutation_server(
        &dir,
        "/v1/soracloud/deploy",
        &draft_response,
        &status_payload,
        "encode pin register response",
        "encode deploy response",
    );
    install_mock_submission_config(&authority, &key_pair);
    let error = DeployArgs {
        container: dir.join("container_manifest.json"),
        service: dir.join("service_manifest.json"),
        bundle_file,
        sorafs_retention_epoch: test_sorafs_retention_epoch(),
        initial_configs: None,
        initial_secrets: None,
        inrou_preseed_receipt: Some(inrou_preseed_receipt),
        torii_url: Some(server.base_url.clone()),
        api_token: None,
        timeout_secs: 5,
    }
    .run(MutationMode::Deploy, &authority, &key_pair)
    .expect_err("empty Inrou guest image must fail before publication");
    assert!(
        format!("{error:#}").contains("nonempty regular file"),
        "unexpected empty guest-image error: {error:#}"
    );
    assert!(
        server.requests().is_empty(),
        "empty guest-image preflight must fail before every SoraFS and Torii request"
    );
}
