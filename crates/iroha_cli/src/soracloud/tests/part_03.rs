fn prepared_inrou_stage_identity_fixture(
    bundle: &SoraDeploymentBundleV1,
) -> TairaInrouStageIdentity {
    let route = bundle
        .service
        .route
        .as_ref()
        .expect("canonical Taira route");
    let guest = &bundle
        .container
        .inrou
        .as_ref()
        .expect("canonical Taira Inrou manifest")
        .guest_images[&SoraInrouGuestIsaV1::Aarch64]
        .published_artifact;
    let (discovery, _, _) = prepare_public_service_discovery(
        bundle,
        &soracloud_fixture_key_pair(0x51),
        SorafsReleaseIdentityV1::new(test_sorafs_retention_epoch()),
    )
    .expect("prepare canonical Taira discovery fixture");
    TairaInrouStageIdentity {
        service_name: bundle.service.service_name.to_string(),
        service_version: bundle.service.service_version.clone(),
        route_host: route.host.clone(),
        route_path_prefix: route.path_prefix.clone(),
        healthcheck_path: bundle
            .container
            .lifecycle
            .healthcheck_path
            .clone()
            .expect("canonical Taira healthcheck"),
        stage_mode: "deploy".to_owned(),
        bundle_hash: bundle.container.bundle_hash.to_string(),
        deployment_bundle_hash: Hash::new(Encode::encode(bundle)).to_string(),
        bundle_content_cid: "bfixturebundle".to_owned(),
        bundle_manifest_digest_hex: "ab".repeat(32),
        guest_content_cid: guest.content_cid.clone(),
        guest_manifest_digest_hex: guest.manifest_digest_hex.clone(),
        discovery_payload_dir: TAIRA_INROU_STAGE_DISCOVERY_PAYLOAD_DIR_V1.to_owned(),
        discovery_document_hash: discovery.document_hash.to_string(),
        discovery_content_cid: discovery.content_cid,
        discovery_manifest_digest_hex: discovery.manifest_digest_hex,
        public_discovery_url: discovery.public_discovery_url,
        public_discovery_cid_host_url: discovery.public_discovery_cid_host_url,
        container_manifest_hash: bundle.container_manifest_hash().to_string(),
        service_manifest_hash: bundle.service_manifest_hash().to_string(),
        placement_targets: bundle.service.placement_targets.clone(),
    }
}

fn sign_prepared_inrou_instruction(
    config: &ClientConfig,
    instruction: InstructionBox,
    binding: &TairaMutationBindingV1,
    operation: &str,
) -> SignedTransaction {
    let client = Client::new(config.clone()).expect("blocking Soracloud fixture client");
    let payload = client
        .account_client()
        .prepare_transaction(iroha::client::AccountTransactionDraft::new(
            [instruction],
            FeePaymentIntent::authority(Vec::new(), None),
            binding.metadata(operation).expect("exact binding metadata"),
        ))
        .expect("build prepared Inrou fixture payload");
    client
        .account_client()
        .sign_transaction(payload)
        .expect("sign prepared Inrou fixture transaction")
}

#[test]
fn prepared_inrou_executable_verifier_binds_pin_manifest_and_authority() {
    let key_pair = soracloud_fixture_key_pair(0x5B);
    let mut config = crate::fallback_config();
    config.account = AccountId::new(key_pair.public_key().clone());
    config.key_pair = key_pair.clone();
    let payload = b"exact prepared Inrou pin fixture";
    let plan = CarBuildPlan::single_file_with_profile(
        payload,
        chunker_registry::default_descriptor().profile,
    )
    .expect("plan pin fixture payload");
    let built = taira_stage_manifest(
        &plan,
        payload,
        &key_pair,
        SorafsReleaseIdentityV1::new(test_sorafs_retention_epoch()),
        "pin fixture",
    )
    .expect("build governed pin fixture");
    let mut stage = prepared_inrou_stage_identity_fixture(&canonical_taira_inrou_bundle_fixture());
    stage.bundle_content_cid = encode_content_cid(&built.manifest.root_cid);
    stage.bundle_manifest_digest_hex = built.digest_hex.clone();
    let binding = TairaMutationBindingV1 {
        authorization_sha256: "ab".repeat(32),
        authorization_nonce: "0123456789abcdef_123456789abcde-".to_owned(),
        kind: "inrou_bundle_pin".to_owned(),
        phase: "pre_edge".to_owned(),
        idempotency_key: "cd".repeat(32),
        execution_expires_at_unix_ms: u64::MAX,
    };
    let instruction =
        iroha::data_model::isi::sorafs::RegisterPinManifest::new(built.bytes, None, None);
    let transaction = sign_prepared_inrou_instruction(
        &config,
        InstructionBox::from(instruction),
        &binding,
        "bundle_pin",
    );
    verify_taira_inrou_prepared_transaction_identity_v1(
        &transaction,
        TairaInrouCanaryPreparedOperationV1::BundlePin,
        &stage,
        &binding.idempotency_key,
    )
    .expect("exact governed bundle pin");

    stage.bundle_manifest_digest_hex = "ef".repeat(32);
    let _error = verify_taira_inrou_prepared_transaction_identity_v1(
        &transaction,
        TairaInrouCanaryPreparedOperationV1::BundlePin,
        &stage,
        &binding.idempotency_key,
    )
    .expect_err("another retained manifest identity must fail closed");
}

#[test]
fn prepared_inrou_executable_verifier_binds_exact_deploy_materials() {
    let key_pair = soracloud_fixture_key_pair(0x5C);
    let mut config = crate::fallback_config();
    config.account = AccountId::new(key_pair.public_key().clone());
    config.key_pair = key_pair.clone();
    let bundle = canonical_taira_inrou_bundle_fixture();
    let stage = prepared_inrou_stage_identity_fixture(&bundle);
    let binding = TairaMutationBindingV1 {
        authorization_sha256: "ab".repeat(32),
        authorization_nonce: "0123456789abcdef_123456789abcde-".to_owned(),
        kind: "inrou_canary".to_owned(),
        phase: "pre_edge".to_owned(),
        idempotency_key: "cd".repeat(32),
        execution_expires_at_unix_ms: u64::MAX,
    };
    let (discovery, _, _) = prepare_public_service_discovery(
        &bundle,
        &soracloud_fixture_key_pair(0x51),
        SorafsReleaseIdentityV1::new(test_sorafs_retention_epoch()),
    )
    .expect("prepare canonical Taira discovery fixture");
    let configs = BTreeMap::from([
        (
            "public_reset_idempotency_v1".to_owned(),
            Json::new(binding.idempotency_key.clone()),
        ),
        (
            PUBLIC_SERVICE_DISCOVERY_CONFIG_NAME.to_owned(),
            taira_inrou_canary_public_discovery_config_value(&discovery)
                .expect("encode canonical Taira discovery registry"),
        ),
    ]);
    let request = signed_bundle_request(
        bundle.clone(),
        configs,
        BTreeMap::new(),
        SoraServiceMutationPreconditionV1::ServiceAbsent,
        Some(&config.account),
        &key_pair,
    )
    .expect("sign exact deploy request");
    let instruction = iroha::data_model::isi::soracloud::DeploySoracloudService {
        bundle: request.bundle,
        initial_service_configs: request.initial_service_configs,
        initial_service_secrets: request.initial_service_secrets,
        precondition: request.precondition,
        provenance: request.provenance,
    };
    let transaction = sign_prepared_inrou_instruction(
        &config,
        InstructionBox::from(instruction),
        &binding,
        "service_mutation",
    );
    verify_taira_inrou_prepared_transaction_identity_v1(
        &transaction,
        TairaInrouCanaryPreparedOperationV1::ServiceMutation,
        &stage,
        &binding.idempotency_key,
    )
    .expect("exact canonical Inrou deploy");
    let _error = verify_taira_inrou_prepared_transaction_identity_v1(
        &transaction,
        TairaInrouCanaryPreparedOperationV1::ServiceMutation,
        &stage,
        &"ef".repeat(32),
    )
    .expect_err("another public-reset idempotency value must fail closed");

    let mut substituted_discovery = discovery;
    substituted_discovery.public_discovery_url =
        "https://taira.sora.org/sorafs/cid/substituted/index.json".to_owned();
    let substituted_configs = BTreeMap::from([
        (
            "public_reset_idempotency_v1".to_owned(),
            Json::new(binding.idempotency_key.clone()),
        ),
        (
            PUBLIC_SERVICE_DISCOVERY_CONFIG_NAME.to_owned(),
            taira_inrou_canary_public_discovery_config_value(&substituted_discovery)
                .expect("encode substituted discovery registry"),
        ),
    ]);
    let substituted_request = signed_bundle_request(
        bundle,
        substituted_configs,
        BTreeMap::new(),
        SoraServiceMutationPreconditionV1::ServiceAbsent,
        Some(&config.account),
        &key_pair,
    )
    .expect("sign substituted discovery request");
    let substituted_transaction = sign_prepared_inrou_instruction(
        &config,
        InstructionBox::from(iroha::data_model::isi::soracloud::DeploySoracloudService {
            bundle: substituted_request.bundle,
            initial_service_configs: substituted_request.initial_service_configs,
            initial_service_secrets: substituted_request.initial_service_secrets,
            precondition: substituted_request.precondition,
            provenance: substituted_request.provenance,
        }),
        &binding,
        "service_mutation",
    );
    let _error = verify_taira_inrou_prepared_transaction_identity_v1(
        &substituted_transaction,
        TairaInrouCanaryPreparedOperationV1::ServiceMutation,
        &stage,
        &binding.idempotency_key,
    )
    .expect_err("a substituted public-discovery registry must fail closed");
}

#[test]
fn prepared_soracloud_draft_quotes_signed_queue_plan_admission() {
    let server = MockHttpServer::start(BTreeMap::new());
    let key_pair = soracloud_fixture_key_pair(0x59);
    let mut config = crate::fallback_config();
    config.account = AccountId::new(key_pair.public_key().clone());
    config.key_pair = key_pair;
    config.torii_api_url = server.base_url.parse().expect("mock Torii URL");
    let binding = TairaMutationBindingV1 {
        authorization_sha256: "ab".repeat(32),
        authorization_nonce: "0123456789abcdef_123456789abcde-".to_owned(),
        kind: "service_mutation".to_owned(),
        phase: "pre_edge".to_owned(),
        idempotency_key: "cd".repeat(32),
        execution_expires_at_unix_ms: u64::MAX,
    };
    let requested = FeePaymentIntent::authority(Vec::new(), None);
    let prepared = prepare_soracloud_draft_transaction(
        &config,
        requested.clone(),
        binding,
        &server.base_url,
        1,
        vec![InstructionBox::from(iroha::data_model::isi::Log::new(
            iroha::data_model::Level::INFO,
            "prepared mutation".to_owned(),
        ))],
        "service_mutation",
    )
    .expect("prepare exact Soracloud mutation");
    let transaction = prepared
        .decode_and_validate()
        .expect("validate prepared Soracloud mutation");
    assert_eq!(transaction.fee_payment_intent(), &requested);
    assert_eq!(
        transaction.admission_intent(),
        TransactionAdmissionIntent::QueuePlanSynced
    );
    let request = server
        .requests()
        .into_iter()
        .find(|request| request.path == iroha_torii_shared::uri::FEES_QUOTE)
        .expect("exact mutation fee quote request");
    let quoted: FeeQuoteWireRequest =
        json::from_slice(&request.body).expect("decode mutation fee quote request");
    assert_eq!(quoted.payload, *transaction.payload());
    assert_eq!(
        quoted.payload.admission_intent,
        TransactionAdmissionIntent::QueuePlanSynced
    );
}

#[test]
fn prepared_inrou_pin_preserves_exact_sponsor_fee_identity() {
    let server = MockHttpServer::start(BTreeMap::new());
    let key_pair = soracloud_fixture_key_pair(0x5A);
    let authority = AccountId::new(key_pair.public_key().clone());
    let mut config = crate::fallback_config();
    config.account = authority.clone();
    config.key_pair = key_pair;
    config.torii_api_url = server.base_url.parse().expect("mock Torii URL");
    let manifest = ManifestBuilder::new()
        .root_cid(sorafs_manifest::canonical_manifest_root_cid([0xA1; 32]))
        .dag_codec(DagCodecId(0x71))
        .chunking_profile(ChunkingProfileV1::from_descriptor(
            chunker_registry::default_descriptor(),
        ))
        .chunk_digest_sha3_256([0xB2; 32])
        .por_root([0xC3; 32])
        .content_length(1)
        .car_digest([0xD4; 32])
        .car_size(1)
        .pin_policy(PinPolicy::default())
        .build()
        .expect("build exact pin manifest");
    let bytes = manifest.encode().expect("encode exact pin manifest");
    let built = BuiltSorafsManifest {
        digest_hex: hex::encode(manifest.digest().expect("manifest digest").as_bytes()),
        manifest,
        bytes,
    };
    let sponsor = iroha::data_model::nexus::FeeSponsorProgramId::new(
        authority,
        Name::from_str("public-reset").expect("sponsor program name"),
    );
    let requested = FeePaymentIntent::sponsor(sponsor.clone(), 7, Vec::new(), None);
    let binding = TairaMutationBindingV1 {
        authorization_sha256: "ab".repeat(32),
        authorization_nonce: "0123456789abcdef_123456789abcde-".to_owned(),
        kind: "inrou_bundle_pin".to_owned(),
        phase: "pre_edge".to_owned(),
        idempotency_key: "cd".repeat(32),
        execution_expires_at_unix_ms: u64::MAX,
    };
    let prepared = prepare_built_sorafs_manifest_registration(
        &built,
        "bundle_pin",
        requested.clone(),
        binding,
        &server.base_url,
        &config,
        1,
    )
    .expect("prepare sponsored pin transaction");
    assert_eq!(prepared.fee_payment, requested);
    assert_eq!(prepared.fee_quote.intent, requested);
    assert_eq!(prepared.fee_payment.sponsor_program(), Some((&sponsor, 7)));
    let transaction = prepared
        .decode_and_validate()
        .expect("prepared public pin transaction");
    assert_eq!(
        transaction.admission_intent(),
        TransactionAdmissionIntent::QueuePlanSynced,
        "every prepared pin is submitted through the public transaction API"
    );
    let requests = server.requests();
    let quote_request = requests
        .iter()
        .find(|request| request.path == iroha_torii_shared::uri::FEES_QUOTE)
        .expect("exact pin fee-quote request");
    let quoted: FeeQuoteWireRequest =
        json::from_slice(&quote_request.body).expect("decode pin fee-quote request");
    assert_eq!(
        quoted.payload.admission_intent,
        TransactionAdmissionIntent::QueuePlanSynced,
        "the public admission intent must already be bound during fee quoting"
    );
    let ordinary_transaction = iroha::data_model::transaction::TransactionBuilder::from_payload(
        transaction.payload().clone(),
    )
    .expect("reconstruct exact pin payload")
    .with_admission_intent(TransactionAdmissionIntent::Ordinary)
    .try_sign(config.key_pair.private_key())
    .expect("sign genuinely Ordinary pin payload");
    let mut ordinary = prepared.clone();
    ordinary.wire = ordinary_transaction
        .encode_wire_v1()
        .expect("encode Ordinary pin transaction");
    ordinary.tx_hash_hex = hex::encode(ordinary_transaction.hash().as_ref());
    let error = submit_prepared_soracloud_transaction(
        &config,
        "://invalid",
        Instant::now() + Duration::from_secs(1),
        &ordinary,
    )
    .expect_err("Ordinary prepared public submissions must fail before HTTP setup");
    assert!(error.to_string().contains("QueuePlanSynced"));
    assert_eq!(server.requests().len(), requests.len());
    assert!(prepared.tx_hash_hex.as_bytes().last().is_some_and(|byte| {
        matches!(byte, b'1' | b'3' | b'5' | b'7' | b'9' | b'b' | b'd' | b'f')
    }));
    let mut marker_cleared = prepared.clone();
    marker_cleared.tx_hash_hex.replace_range(63..64, "0");
    let error = marker_cleared
        .decode_and_validate()
        .expect_err("marker-cleared prepared transaction hash must fail closed");
    assert!(error.to_string().contains("Iroha hash marker set"));

    // The real pin recovery path must work on a mixed-dataspace testnet
    // without asking for unrestricted FindTransactions inventory.
    let result = iroha::data_model::transaction::TransactionResult::new(Ok(
        iroha::data_model::transaction::DataTriggerSequence::default(),
    ));
    let output = iroha::data_model::block::execution_output::ExecutionOutputV1::Network(
        iroha::data_model::block::execution_output::NetworkExecutionOutputV1 {
            input_index: 0,
            result,
            completions: Vec::new(),
        },
    );
    let details = iroha_torii_shared::PipelineTransactionDetailsResponse {
        hash: transaction.hash_as_entrypoint().to_string(),
        transaction: iroha::data_model::query::CommittedTransaction {
            block_hash: iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(
                b"exact proof block",
            )),
            entrypoint_hash: transaction.hash_as_entrypoint(),
            entrypoint_proof: iroha_crypto::MerkleProof::from_audit_path(0, Vec::new()),
            entrypoint: TransactionEntrypoint::External(transaction.clone()),
            output_hash: iroha_crypto::HashOf::new(&output),
            output_proof: iroha_crypto::MerkleProof::from_audit_path(0, Vec::new()),
            output,
        },
    };
    for (resolved_from, proof, expected) in [
        (
            "cache",
            None,
            PreparedSoracloudRecoveryV1::Pending {
                terminal_kind: "Applied".to_owned(),
            },
        ),
        (
            "state",
            None,
            PreparedSoracloudRecoveryV1::Pending {
                terminal_kind: "AppliedEvidencePending".to_owned(),
            },
        ),
        (
            "state",
            Some(norito::to_bytes(&details).unwrap()),
            PreparedSoracloudRecoveryV1::Applied {
                block_height: 7,
                evidence_sha256: prepared.tx_hash_hex.clone(),
            },
        ),
    ] {
        let status = PipelineTransactionStatusResponse::new(
            prepared.tx_hash_hex.clone(),
            iroha_torii_shared::PipelineTransactionStatus {
                kind: "Applied".to_owned(),
                block_height: Some(7),
            },
            "global".to_owned(),
            resolved_from.to_owned(),
        );
        let mut routes = BTreeMap::from([
            (
                format!(
                    "/v1/pipeline/transactions/status?hash={}&scope=global",
                    transaction.hash()
                ),
                MockHttpResponse::json(json::to_vec(&status).unwrap()),
            ),
            (
                "/v1/node/capabilities".to_owned(),
                MockHttpResponse::json(
                    json::to_vec(&norito::json!({
                        "data_model_version": (iroha::data_model::DATA_MODEL_VERSION)
                    }))
                    .unwrap(),
                ),
            ),
        ]);
        let (proof_status, body) = match proof {
            Some(body) => ("200 OK", body),
            None => (
                "404 Not Found",
                norito::to_bytes(&iroha_torii_shared::ErrorEnvelope::new(
                    "transaction_details_not_found",
                    "The exact committed transaction proof is not available.",
                ))
                .unwrap(),
            ),
        };
        routes.insert(
            "/v1/pipeline/transactions/details".to_owned(),
            MockHttpResponse {
                status: proof_status,
                content_type: "application/x-norito",
                body,
            },
        );
        let mut malformed_routes = routes.clone();
        malformed_routes.insert(
            "/v1/pipeline/transactions/details".to_owned(),
            MockHttpResponse {
                status: "404 Not Found",
                content_type: "text/plain",
                body: b"untyped missing proof".to_vec(),
            },
        );
        let proof_server = MockHttpServer::start(routes);
        assert_eq!(
            recover_prepared_soracloud_transaction(
                &config,
                &proof_server.base_url,
                Instant::now() + Duration::from_secs(5),
                &prepared
            )
            .expect("exact prepared pin observation"),
            expected
        );
        let requests = proof_server.requests();
        assert_eq!(requests.len(), if resolved_from == "state" { 3 } else { 1 });
        assert!(requests.iter().all(|request| request.method != "POST"
            || request.path == "/v1/pipeline/transactions/details"));
        if resolved_from == "state"
            && matches!(expected, PreparedSoracloudRecoveryV1::Applied { .. })
        {
            let mut bounded_config = config.clone();
            bounded_config.torii_api_url = proof_server.base_url.parse().unwrap();
            let client = Client::new(bounded_config).unwrap();
            let mut substituted = prepared.clone();
            substituted.wire[0] ^= 1;
            let _ = verify_committed_prepared_soracloud_transaction(
                &client,
                &substituted,
                &transaction,
            )
            .expect_err("the exact pin proof cannot authorize altered retained bytes");
            let malformed_server = MockHttpServer::start(malformed_routes);
            let _ = recover_prepared_soracloud_transaction(
                &config,
                &malformed_server.base_url,
                Instant::now() + Duration::from_secs(5),
                &prepared,
            )
            .expect_err("an untyped HTTP404 cannot become pending proof visibility");
            assert_eq!(malformed_server.requests().len(), 3);
        }
    }

    let mut zero_height = prepared;
    zero_height.fee_quote.observation.next_block_height = 0;
    let error = zero_height
        .decode_and_validate()
        .expect_err("prepared quote with an impossible observation must fail closed");
    assert!(error.to_string().contains("semantically invalid"));
}

#[test]
fn prepared_soracloud_recovery_accepts_only_global_state_finality() {
    let response = |kind: &str, scope: &str, resolved_from: &str| {
        PipelineTransactionStatusResponse::new(
            format!("{}b", "a".repeat(63)),
            iroha_torii_shared::PipelineTransactionStatus {
                kind: kind.to_owned(),
                block_height: Some(7),
            },
            scope.to_owned(),
            resolved_from.to_owned(),
        )
    };
    assert!(soracloud_status_is_final_applied(&response(
        "Applied", "global", "state"
    )));
    for (scope, resolved_from) in [("global", "cache"), ("global", "queue"), ("local", "state")] {
        assert!(!soracloud_status_is_final_applied(&response(
            "Applied",
            scope,
            resolved_from
        )));
    }
    for kind in ["Rejected", "Expired"] {
        assert!(soracloud_status_is_final_failure(&response(
            kind, "global", "state"
        )));
        for (scope, resolved_from) in [("global", "cache"), ("global", "queue"), ("local", "state")]
        {
            assert!(!soracloud_status_is_final_failure(&response(
                kind,
                scope,
                resolved_from
            )));
        }
    }
}

#[test]
fn mock_http_server_helpers_track_sorafs_pin_registration_digest() {
    let manifest = ManifestBuilder::new()
        .root_cid(sorafs_manifest::canonical_manifest_root_cid([0xA1; 32]))
        .dag_codec(DagCodecId(0x71))
        .chunking_profile(ChunkingProfileV1::from_descriptor(
            chunker_registry::default_descriptor(),
        ))
        .chunk_digest_sha3_256([0xB2; 32])
        .por_root([0xC3; 32])
        .content_length(1)
        .car_digest([0xD4; 32])
        .car_size(1)
        .pin_policy(PinPolicy::default())
        .build()
        .expect("build mock pin registration manifest");
    let manifest_bytes = manifest.encode().expect("encode mock pin manifest");
    let digest = hex::encode(
        manifest
            .digest()
            .expect("digest mock pin manifest")
            .as_bytes(),
    );
    let key_pair = soracloud_fixture_key_pair(0x4A);
    let authority = AccountId::new(key_pair.public_key().clone());
    let mut config = crate::fallback_config();
    config.account = authority;
    config.key_pair = key_pair;
    let client = Client::new(config).expect("blocking Soracloud fixture client");
    let transaction = {
        let account = client.account_client();
        account
            .prepare_transaction(iroha::client::AccountTransactionDraft::new(
                [iroha::data_model::isi::sorafs::RegisterPinManifest::new(
                    manifest_bytes.clone(),
                    None,
                    None,
                )],
                FeePaymentIntent::authority(Vec::new(), None),
                Metadata::default(),
            ))
            .and_then(|payload| account.sign_transaction(payload))
    }
    .expect("build mock native pin registration transaction");
    let request = CapturedHttpRequest {
        method: "POST".to_owned(),
        path: "/v1/sorafs/pin/register".to_owned(),
        body: transaction.encode_versioned(),
    };
    let registration =
        mock_sorafs_pin_registration(&request).expect("decode mock registration transaction");
    assert_eq!(registration.manifest_payload, manifest_bytes);
    assert_eq!(registration.manifest_digest_hex, digest);
    assert_eq!(
        registration.tx_hash_hex,
        hex::encode(transaction.hash().as_ref())
    );
    let mut registered_pin_manifests = BTreeMap::new();
    let path = format!("/v1/sorafs/pin/{digest}");
    assert!(
        mock_sorafs_pin_registry_response(&path, &registered_pin_manifests).is_none(),
        "unregistered mock pin records should return 404"
    );
    registered_pin_manifests.insert(digest.clone(), manifest_bytes);
    assert_eq!(
        mock_sorafs_pin_registry_path_is_registered(&path, &registered_pin_manifests),
        Some(digest.as_str())
    );
    assert!(
        mock_sorafs_pin_registry_response(&path, &registered_pin_manifests).is_some(),
        "registered mock pin records should be visible to polling GETs"
    );
    let response = mock_sorafs_pin_registry_response(&path, &registered_pin_manifests)
        .expect("build finalized mock pin record");
    let expected = ExpectedSorafsPinManifest {
        digest: ManifestDigest::from_manifest(&manifest).expect("derive expected digest"),
        root_cid: ManifestRootCid::try_from_slice(&manifest.root_cid)
            .expect("decode expected root CID"),
    };
    assert_eq!(
        decode_sorafs_pin_manifest_readiness(&response.body, expected)
            .expect("approved finalized record is deployment-ready"),
        SorafsPinManifestReadiness::Approved(2)
    );
    let wrong_root = ExpectedSorafsPinManifest {
        root_cid: ManifestRootCid::from_blake3_digest([0x7A; 32])
            .expect("build distinct canonical root CID"),
        ..expected
    };
    assert!(
        decode_sorafs_pin_manifest_readiness(&response.body, wrong_root)
            .expect_err("a mismatched root CID must not be deployment-ready")
            .to_string()
            .contains("does not match")
    );

    let mut finalized: PinManifestFinalizedRecordV1 =
        json::from_slice(&response.body).expect("decode finalized mock record");
    finalized.manifest.approved_epoch = Some(9);
    assert!(
        decode_sorafs_pin_manifest_readiness(
            &json::to_vec(&finalized).expect("encode inconsistent approved record"),
            expected,
        )
        .expect_err("an inconsistent approval epoch must be rejected")
        .to_string()
        .contains("does not bind")
    );
    finalized.manifest.status = PinStatus::Pending;
    finalized.manifest.approved_epoch = None;
    assert_eq!(
        decode_sorafs_pin_manifest_readiness(
            &json::to_vec(&finalized).expect("encode pending finalized record"),
            expected,
        )
        .expect("pending finalized record remains waitable"),
        SorafsPinManifestReadiness::Pending
    );
    finalized.manifest.status = PinStatus::Retired(3);
    assert!(
        decode_sorafs_pin_manifest_readiness(
            &json::to_vec(&finalized).expect("encode retired finalized record"),
            expected,
        )
        .expect_err("retired finalized record must not be deployment-ready")
        .to_string()
        .contains("not replication-eligible")
    );
}
#[test]
fn pin_registration_waits_for_approved_without_reposting_pending_manifest() {
    let manifest = ManifestBuilder::new()
        .root_cid(sorafs_manifest::canonical_manifest_root_cid([0xA6; 32]))
        .dag_codec(DagCodecId(0x71))
        .chunking_profile(ChunkingProfileV1::from_descriptor(
            chunker_registry::default_descriptor(),
        ))
        .chunk_digest_sha3_256([0xB6; 32])
        .por_root([0xC6; 32])
        .content_length(1)
        .car_digest([0xD6; 32])
        .car_size(1)
        .pin_policy(PinPolicy::default())
        .build()
        .expect("build pending pin fixture manifest");
    let manifest_bytes = manifest.encode().expect("encode pending pin fixture");
    let digest = hex::encode(
        manifest
            .digest()
            .expect("digest pending pin fixture")
            .as_bytes(),
    );
    let path = format!("/v1/sorafs/pin/{digest}");
    let registry = BTreeMap::from([(digest.clone(), manifest_bytes.clone())]);
    let approved = mock_sorafs_pin_registry_response(&path, &registry)
        .expect("build finalized pending fixture base");
    let mut pending: PinManifestFinalizedRecordV1 =
        json::from_slice(&approved.body).expect("decode finalized pending fixture base");
    pending.manifest.status = PinStatus::Pending;
    pending.manifest.approved_epoch = None;
    let server = MockHttpServer::start(BTreeMap::from([
        (
            path,
            MockHttpResponse::json(
                json::to_vec(&pending).expect("encode finalized pending fixture"),
            ),
        ),
        (
            "/v1/sorafs/pin/register".to_owned(),
            MockHttpResponse::json(
                json::to_vec(&norito::json!({ "ok": true }))
                    .expect("encode unexpected registration response"),
            ),
        ),
    ]));
    let key_pair = soracloud_fixture_key_pair(0x4C);
    let mut config = crate::fallback_config();
    config.account = AccountId::new(key_pair.public_key().clone());
    config.key_pair = key_pair;
    config.torii_api_url = server.base_url.parse().expect("mock Torii URL");
    let error = register_sorafs_pin_manifest_and_wait(
        &Client::new(config).expect("blocking Soracloud fixture client"),
        iroha::client::SorafsPinRegisterArgs {
            manifest_payload: &manifest_bytes,
            alias: None,
            successor_of: None,
        },
        &digest,
        "pending fixture",
        1,
    )
    .expect_err("Pending must not be deployment-ready");
    assert!(error.to_string().contains("reach Approved"), "{error}");
    assert!(
        server
            .requests()
            .iter()
            .all(|request| request.method != "POST"),
        "an existing Pending record must be polled rather than reposted"
    );
}
impl Drop for MockHttpServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(&self.address);
        if let Some(handle) = self.handle.take() {
            handle.join().expect("join mock HTTP server");
        }
    }
}
fn mock_http_connection_closed(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::BrokenPipe
            | std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::NotConnected
            | std::io::ErrorKind::UnexpectedEof
    )
}
fn read_mock_http_request(stream: &mut TcpStream) -> CapturedHttpRequest {
    const READ_IDLE_TIMEOUT: Duration = Duration::from_secs(2);
    let mut request = Vec::new();
    let mut buffer = [0_u8; 1024];
    let mut header_end = None;
    let mut header_deadline = Instant::now() + READ_IDLE_TIMEOUT;
    loop {
        match std::io::Read::read(stream, &mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                request.extend_from_slice(&buffer[..read]);
                header_deadline = Instant::now() + READ_IDLE_TIMEOUT;
                if let Some(end) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                    header_end = Some(end + 4);
                    break;
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                if Instant::now() >= header_deadline {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
                continue;
            }
            Err(error) if mock_http_connection_closed(&error) => {
                break;
            }
            Err(error) if mock_http_connection_closed(&error) => {
                break;
            }
            Err(error) => panic!("read mock HTTP request failed: {error}"),
        }
    }
    let header_end = header_end.unwrap_or(request.len());
    let header_text = String::from_utf8_lossy(&request[..header_end]);
    let mut lines = header_text.lines();
    let request_line = lines.next().unwrap_or_default();
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next().unwrap_or_default().to_owned();
    let path = request_parts.next().unwrap_or_default().to_owned();
    let mut headers = BTreeMap::new();
    for line in lines {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_owned());
        }
    }
    let content_length = headers
        .get("content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = request[header_end..].to_vec();
    let mut body_deadline = Instant::now() + READ_IDLE_TIMEOUT;
    while body.len() < content_length {
        match std::io::Read::read(stream, &mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                body.extend_from_slice(&buffer[..read]);
                body_deadline = Instant::now() + READ_IDLE_TIMEOUT;
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                if Instant::now() >= body_deadline {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
                continue;
            }
            Err(error) if mock_http_connection_closed(&error) => {
                break;
            }
            Err(error) => panic!("read mock HTTP request body failed: {error}"),
        }
    }
    body.truncate(content_length);
    CapturedHttpRequest { method, path, body }
}
fn node_available() -> bool {
    match Command::new("node").arg("--version").output() {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}
fn bash_available() -> bool {
    match Command::new("bash").arg("--version").output() {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}
fn js_string_literal(path: &Path) -> String {
    format!("{:?}", path.to_string_lossy())
}
fn run_bash_syntax_check(script_path: &Path) {
    let output = Command::new("bash")
        .arg("-n")
        .arg(script_path)
        .output()
        .expect("run bash syntax check");
    assert!(
        output.status.success(),
        "bash syntax check failed: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
const SCAFFOLD_FILE_CONTRACTS: &str =
    include_str!("../assets/v1/tests/scaffold_file_contracts.tsv");
fn assert_scaffold_file_contract(root: &Path, case: &str) {
    let mut contents = BTreeMap::<&str, String>::new();
    let bash_available = bash_available();
    let mut matched = 0;
    for row in SCAFFOLD_FILE_CONTRACTS.lines() {
        let mut fields = row.splitn(4, '\t');
        if fields.next() != Some(case) {
            continue;
        }
        matched += 1;
        let operation = fields.next().expect("scaffold contract operation");
        let path = fields.next().expect("scaffold contract path");
        let marker = fields.next().unwrap_or_default();
        let full_path = root.join(path);
        match operation {
            "exists" => assert!(full_path.exists(), "missing scaffold path `{path}`"),
            "missing" => assert!(!full_path.exists(), "unexpected scaffold path `{path}`"),
            "executable" => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt as _;
                    assert_eq!(
                        fs::metadata(&full_path)
                            .unwrap_or_else(|error| panic!("metadata for `{path}`: {error}"))
                            .permissions()
                            .mode()
                            & 0o111,
                        0o111,
                        "scaffold path `{path}` must be executable"
                    );
                }
            }
            "contains" | "excludes" => {
                let file = contents.entry(path).or_insert_with(|| {
                    fs::read_to_string(&full_path)
                        .unwrap_or_else(|error| panic!("read scaffold path `{path}`: {error}"))
                });
                let contains = file.contains(marker);
                assert_eq!(
                    contains,
                    operation == "contains",
                    "scaffold path `{path}` marker `{marker}`"
                );
            }
            "bash" if bash_available => run_bash_syntax_check(&full_path),
            "bash" => {}
            value => panic!("unsupported scaffold contract operation `{value}`"),
        }
    }
    assert!(matched > 0, "missing scaffold file contract `{case}`");
}
fn assert_optional_path_ends_with(path: Option<&str>, suffix: &str) {
    assert!(
        path.is_some_and(|path| path.ends_with(suffix)),
        "expected optional path ending in `{suffix}`, got {path:?}"
    );
}
fn assert_notes_contain(notes: &[String], marker: &str) {
    assert!(
        notes.iter().any(|note| note.contains(marker)),
        "notes do not contain `{marker}`: {notes:?}"
    );
}
static NODE_HARNESS_LOCK: Mutex<()> = Mutex::new(());
fn run_node_harness(script_path: &Path) {
    let _guard = NODE_HARNESS_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let output = Command::new("node")
        .arg(script_path)
        .output()
        .expect("run node harness");
    assert!(
        output.status.success(),
        "node harness failed: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
fn run_generated_auth_harness(
    dir_name: &str,
    template: InitTemplate,
    static_markers: &[&str],
    harness_name: &str,
    harness_index: usize,
    with_state_file: bool,
) {
    let (service_name, server_path) = match template {
        InitTemplate::Webapp => ("agent_console", "webapp/api/server.mjs"),
        InitTemplate::PiiApp => ("clinic_console", "pii-app/api/server.mjs"),
        _ => panic!("generated auth harness requires a webapp or pii-app template"),
    };
    let (dir, _) = named_service_fixture(dir_name, service_name, template);
    let server_path = dir.join(server_path);
    if !node_available() {
        eprintln!("node unavailable; validating static auth markers in scaffold");
        let api = fs::read_to_string(&server_path).expect("read generated auth API");
        for marker in static_markers {
            assert!(
                api.contains(marker),
                "generated auth API missing marker: {marker}"
            );
        }
        return;
    }
    let harness_path = dir.join(harness_name);
    let mut script = TEST_HARNESSES_V1[harness_index].to_owned();
    script = script.replace("__SERVER_PATH__", &js_string_literal(&server_path));
    if with_state_file {
        script = script.replace(
            "__STATE_FILE__",
            &js_string_literal(&dir.join(".shared_auth_state.json")),
        );
    }
    fs::write(&harness_path, script).expect("write node harness");
    run_node_harness(&harness_path);
}
const GENERATED_AUTH_HARNESS_CASES: &str =
    include_str!("../assets/v1/tests/generated_auth_harness_cases.tsv");
const PII_STARTUP_FAILURE_CASES: &str =
    include_str!("../assets/v1/tests/pii_startup_failure_cases.tsv");
fn fixture_case_fields<'a>(table: &'a str, key: &str, field_count: usize) -> Vec<&'a str> {
    let row = table
        .lines()
        .find(|row| {
            row.split_once('\t')
                .is_some_and(|(candidate, _)| candidate == key)
        })
        .unwrap_or_else(|| panic!("missing fixture case `{key}`"));
    let fields = row.split('\t').collect::<Vec<_>>();
    assert_eq!(fields.len(), field_count, "fixture field count for `{key}`");
    fields
}
fn run_generated_auth_harness_case(index: usize) {
    let key = index.to_string();
    let fields = fixture_case_fields(GENERATED_AUTH_HARNESS_CASES, &key, 6);
    let template = match fields[2] {
        "webapp" => InitTemplate::Webapp,
        "pii-app" => InitTemplate::PiiApp,
        value => panic!("unsupported generated auth fixture template `{value}`"),
    };
    let markers = fields[5].split("~|~").collect::<Vec<_>>();
    let with_state_file = fields[4].parse().expect("generated auth fixture boolean");
    run_generated_auth_harness(
        fields[1],
        template,
        &markers,
        fields[3],
        index,
        with_state_file,
    );
}
macro_rules! generated_auth_harness_test {
    ($name:ident, $index:literal) => {
        #[test]
        fn $name() {
            run_generated_auth_harness_case($index);
        }
    };
}
macro_rules! pii_startup_failure_test {
    ($name:ident, $case:literal) => {
        #[test]
        fn $name() {
            let fields = fixture_case_fields(PII_STARTUP_FAILURE_CASES, $case, 5);
            let markers = fields[4].split("~|~").collect::<Vec<_>>();
            assert_generated_pii_app_startup_import_fails(
                concat!("pii_auth_", $case),
                concat!("pii_auth_", $case, "_fail.mjs"),
                &[(fields[1], fields[2])],
                fields[3],
                &markers,
            );
        }
    };
}
fn assert_generated_pii_app_startup_import_fails(
    dir_name: &str,
    harness_name: &str,
    env_overrides: &[(&str, &str)],
    expected_error: &str,
    static_markers: &[&str],
) {
    let (dir, _) = service_fixture(dir_name, InitTemplate::PiiApp);
    let server_path = dir.join("pii-app/api/server.mjs");
    if !node_available() {
        eprintln!("node unavailable; validating static pii startup-failure markers in scaffold");
        let api = fs::read_to_string(&server_path).expect("read pii api");
        for marker in static_markers {
            assert!(
                api.contains(marker),
                "generated pii api missing startup-failure marker: {marker}"
            );
        }
        return;
    }
    let mut env_values = BTreeMap::from([
        ("AUTH_MODE".to_owned(), "strict".to_owned()),
        (
            "AUTH_CAPABILITY_MAP_JSON".to_owned(),
            r#"{"1111111111111111111111111111111111111111111111111111111111111111":["pii.records.read"]}"#
                .to_owned(),
        ),
        (
            "AUTH_CHALLENGE_TTL_SECS".to_owned(),
            "120".to_owned(),
        ),
        (
            "AUTH_REQUIRE_EXTERNAL_SHARED_STATE".to_owned(),
            "0".to_owned(),
        ),
        ("AUTH_SESSION_TTL_SECS".to_owned(), "900".to_owned()),
        (
            "SESSION_HMAC_KEY".to_owned(),
            "0123456789abcdef0123456789abcdef0123456789abcdef".to_owned(),
        ),
        ("NODE_ENV".to_owned(), "development".to_owned()),
        ("PUBLIC_BASE_URL".to_owned(), "http://127.0.0.1".to_owned()),
    ]);
    for (name, value) in env_overrides {
        env_values.insert((*name).to_owned(), (*value).to_owned());
    }
    let env_assignments = env_values
        .iter()
        .map(|(name, value)| format!("process.env.{name} = {value:?};"))
        .collect::<Vec<_>>()
        .join("\n");
    let harness_path = dir.join(harness_name);
    let scenario = harness_name.trim_end_matches(".mjs");
    let mut script = TEST_HARNESSES_V1[0].to_owned();
    script = script.replace("__SERVER_PATH__", &js_string_literal(&server_path));
    script = script.replace("__EXPECTED_ERROR__", &format!("{expected_error:?}"));
    script = script.replace("__ENV_ASSIGNMENTS__", &env_assignments);
    script = script.replace("__SCENARIO__", scenario);
    fs::write(&harness_path, script).expect("write node harness");
    run_node_harness(&harness_path);
}
fn run_generated_pii_app_auth_core_harness(
    dir_name: &str,
    harness_name: &str,
    env_overrides: &[(&str, &str)],
    static_markers: &[&str],
    test_body: &str,
) {
    run_generated_pii_app_auth_core_harness_with_setup(
        dir_name,
        harness_name,
        env_overrides,
        static_markers,
        "",
        test_body,
    );
}
fn run_generated_pii_app_auth_core_harness_with_setup(
    dir_name: &str,
    harness_name: &str,
    env_overrides: &[(&str, &str)],
    static_markers: &[&str],
    setup_before_import: &str,
    test_body: &str,
) {
    let (dir, _) = service_fixture(dir_name, InitTemplate::PiiApp);
    let server_path = dir.join("pii-app/api/server.mjs");
    let api = fs::read_to_string(&server_path).expect("read pii api");
    for marker in static_markers {
        assert!(
            api.contains(marker),
            "generated pii api missing auth-core marker: {marker}"
        );
    }
    if !node_available() {
        eprintln!("node unavailable; static auth-core markers validated in scaffold");
        return;
    }
    let (auth_core, _) = api
        .split_once("\nimport http from \"node:http\";")
        .expect("generated pii api must include auth core before http server");
    let core_module_path = dir.join(format!("{harness_name}.generated.mjs"));
    fs::write(&core_module_path, format!("{auth_core}\n{test_body}"))
        .expect("write auth core harness module");
    let state_file = dir.join(format!("{harness_name}.state.json"));
    let mut env_values = BTreeMap::from([
        ("AUTH_MODE".to_owned(), "strict".to_owned()),
        ("AUTH_CAPABILITY_MAP_JSON".to_owned(), "{}".to_owned()),
        ("AUTH_CHALLENGE_TTL_SECS".to_owned(), "120".to_owned()),
        (
            "AUTH_REQUIRE_EXTERNAL_SHARED_STATE".to_owned(),
            "0".to_owned(),
        ),
        ("AUTH_SESSION_TTL_SECS".to_owned(), "900".to_owned()),
        (
            "SESSION_HMAC_KEY".to_owned(),
            "0123456789abcdef0123456789abcdef0123456789abcdef".to_owned(),
        ),
        ("NODE_ENV".to_owned(), "development".to_owned()),
        ("PUBLIC_BASE_URL".to_owned(), "http://127.0.0.1".to_owned()),
        (
            "SORACLOUD_SHARED_STATE_FILE".to_owned(),
            state_file.to_string_lossy().into_owned(),
        ),
    ]);
    for (name, value) in env_overrides {
        env_values.insert((*name).to_owned(), (*value).to_owned());
    }
    let env_assignments = env_values
        .iter()
        .map(|(name, value)| format!("process.env.{name} = {value:?};"))
        .collect::<Vec<_>>()
        .join("\n");
    let harness_path = dir.join(harness_name);
    let scenario = harness_name.trim_end_matches(".mjs");
    let mut script = TEST_HARNESSES_V1[1].to_owned();
    script = script.replace(
        "__CORE_MODULE_PATH__",
        &js_string_literal(&core_module_path),
    );
    script = script.replace("__ENV_ASSIGNMENTS__", &env_assignments);
    script = script.replace("__SETUP_BEFORE_IMPORT__", setup_before_import);
    script = script.replace("__SCENARIO__", scenario);
    fs::write(&harness_path, script).expect("write node harness");
    run_node_harness(&harness_path);
}
fn install_mock_submission_config(authority: &AccountId, key_pair: &KeyPair) {
    let mut config = crate::fallback_config();
    config.account = authority.clone();
    config.key_pair = key_pair.clone();
    SORACLOUD_INVOCATION_CONTEXT.with(|slot| {
        *slot.borrow_mut() = Some(SoracloudInvocationContext {
            submission_config: config,
            http_witness_file: None,
            fee_payment: Ok(FeePaymentIntent::authority(Vec::new(), None)),
        });
    });
    SORACLOUD_TEST_SUBMITTED_TX_HASH.with(|slot| {
        *slot.borrow_mut() = Some(Hash::new(b"soracloud-cli-test-submission"));
    });
}
fn install_mock_protected_read_signer() {
    let key_pair = soracloud_fixture_key_pair(0x2f);
    let authority = AccountId::new(key_pair.public_key().clone());
    install_mock_submission_config(&authority, &key_pair);
}
fn mock_active_inrou_hosts() -> Vec<SoraInrouHostCapabilityRecordV1> {
    let mut hosts = (0..SORACLOUD_ARTIFACT_MIN_REPLICAS_V1)
        .map(|index| {
            let seed = 0x70_u8
                .checked_add(u8::try_from(index).expect("test host index fits u8"))
                .expect("test host seed");
            let validator_key_pair = soracloud_fixture_key_pair(seed);
            let peer_key_pair = soracloud_fixture_key_pair(seed ^ 0x80);
            SoraInrouHostCapabilityRecordV1 {
                schema_version:
                    iroha::data_model::soracloud::SORA_INROU_HOST_CAPABILITY_RECORD_VERSION_V1,
                validator_account_id: AccountId::new(validator_key_pair.public_key().clone()),
                peer_id: PeerId::from(peer_key_pair.public_key().clone()).to_string(),
                supported_guest_isas: BTreeSet::from([SoraInrouGuestIsaV1::X8664]),
                trusted_guest_artifact: SoraPublishedInrouGuestImageArtifactV1 {
                    manifest_digest_hex: "31".repeat(32),
                    content_cid: "bafyr6ibrgeytcmjrgeytcmjrgeytcmjrgeytcmjrgeytcmjrgeytcmjrge"
                        .to_owned(),
                },
                max_hosted_replica_capacity:
                    iroha::data_model::soracloud::SORA_INROU_HOSTED_REPLICA_CAPACITY_V1,
                max_cpu_millis: 4_000,
                max_memory_bytes: 16 * 1024 * 1024 * 1024,
                max_storage_bytes: 64 * 1024 * 1024 * 1024,
                advertised_at_ms: 100_000,
                heartbeat_expires_at_ms: u64::MAX,
            }
        })
        .collect::<Vec<_>>();
    hosts.sort_by(|left, right| left.validator_account_id.cmp(&right.validator_account_id));
    hosts
}
fn mock_control_plane_status_payload(service_names: &[&str]) -> norito::json::Value {
    let services = service_names
        .iter()
        .map(|service_name| ServiceStatusOutput {
            service_name: (*service_name).to_owned(),
            current_version: "1.0.0".to_owned(),
            revision_count: 1,
            config_generation: 0,
            secret_generation: 0,
            config_entry_count: 0,
            secret_entry_count: 0,
            service_lease: None,
            public_discovery_content_cid: None,
            public_discovery_url: None,
            public_discovery_cid_host_url: None,
            latest_revision: None,
            active_rollout: None,
            last_rollout: None,
        })
        .collect::<Vec<_>>();
    let service_count =
        u32::try_from(services.len()).expect("mock service count fits the V1 range");
    norito::json!({
        "schema_version": 1,
        "control_plane": {
            "schema_version": 1,
            "service_count": (service_count),
            "audit_event_count": 0,
            "active_inrou_hosts": (mock_active_inrou_hosts()),
            "services": (services),
            "recent_audit_events": []
        }
    })
}
fn mock_soracloud_draft_response(authority: &AccountId, key_pair: &KeyPair) -> norito::json::Value {
    use iroha::data_model::{
        domain::Domain,
        isi::{Register, framed_instruction_payload},
    };

    let instruction = InstructionBox::from(Register::domain(Domain::new(
        iroha_model_base::domain::DomainId::try_new("soracloud_cli_test", "universal")
            .expect("canonical mock domain id"),
    )));
    let (wire_id, framed) = framed_instruction_payload(&instruction)
        .expect("frame canonical registered Soracloud mock instruction");
    json::to_value(&SoracloudMutationDraftResponse {
        ok: true,
        authority: authority.clone(),
        signed_by: key_pair.public_key().clone(),
        tx_instructions: vec![SoracloudTxInstruction {
            wire_id: wire_id.to_owned(),
            payload_hex: hex::encode(framed),
        }],
    })
    .expect("encode exact Soracloud mutation draft response")
}
fn mock_soracloud_submission_receipt(
    authority: &AccountId,
    key_pair: &KeyPair,
) -> norito::json::Value {
    json::to_value(&SoracloudMutationSubmissionReceiptV1 {
        schema_version: SORACLOUD_MUTATION_SUBMISSION_RECEIPT_VERSION_V1,
        authority: authority.clone(),
        signed_by: key_pair.public_key().clone(),
        instruction_count: NonZeroU32::new(1).expect("nonzero instruction count"),
        submitted_tx_hash: Hash::new(b"submitted transaction"),
    })
    .expect("encode exact Soracloud submission receipt")
}
fn mock_prior_service_status(workspace: &Path, final_status: &Value) -> Value {
    let mut status = final_status.clone();
    let services = status
        .pointer_mut("/control_plane/services")
        .and_then(Value::as_array_mut)
        .expect("canonical service status list");
    for service_status in services {
        let service_name = service_status
            .get("service_name")
            .and_then(Value::as_str)
            .expect("service name");
        let (container_path, service_path) = if workspace.join("app_manifest.json").exists() {
            let app: SoracloudAppManifestV1 =
                load_json(&workspace.join("app_manifest.json")).expect("app fixture");
            let service = app
                .services
                .iter()
                .find(|service| service.service_name == service_name)
                .expect("app service fixture");
            (
                resolve_manifest_path(workspace, &service.container_manifest),
                resolve_manifest_path(workspace, &service.service_manifest),
            )
        } else {
            (
                workspace.join("container_manifest.json"),
                workspace.join("service_manifest.json"),
            )
        };
        let container: UnpublishedContainerManifestV1 =
            load_json(&container_path).expect("container fixture");
        let mut service: SoraServiceManifestV1 = load_json(&service_path).expect("service fixture");
        service.service_version = "0.9.0".to_owned();
        let route = service.route.as_ref();
        let revision = ControlPlaneServiceRevision {
            sequence: 1,
            action: SoracloudAction::Deploy,
            service_version: service.service_version.clone(),
            service_manifest_hash: Hash::new(Encode::encode(&service)),
            container_manifest_hash: container.workspace_hash().expect("container hash"),
            replicas: service.replicas.get(),
            execution_plane: service.execution_plane,
            route_host: route.map(|route| route.host.clone()),
            route_path_prefix: route.map(|route| route.path_prefix.clone()),
            route_service_port: route.map(|route| route.service_port.get()),
            route_visibility: route.map(|route| format!("{:?}", route.visibility)),
            route_tls_mode: route.map(|route| format!("{:?}", route.tls_mode)),
            base_url: None,
            healthcheck_url: None,
            public_discovery_content_cid: None,
            public_discovery_url: None,
            public_discovery_cid_host_url: None,
            state_binding_count: u32::try_from(service.state_bindings.len())
                .expect("binding count"),
            state_bindings: service.state_bindings.clone(),
            lease_volumes: service.lease_volumes.clone(),
            allow_model_inference: container.capabilities.allow_model_inference,
            allow_model_training: container.capabilities.allow_model_training,
            runtime: container.runtime,
            allow_state_writes: container.capabilities.allow_state_writes,
            network: container.capabilities.network,
            cpu_millis: container.resources.cpu_millis.get(),
            memory_bytes: container.resources.memory_bytes.get(),
            ephemeral_storage_bytes: container.resources.ephemeral_storage_bytes.get(),
            max_open_files_per_process: container.resources.max_open_files_per_process.get(),
            max_tasks: container.resources.max_tasks.get(),
            start_grace_secs: container.lifecycle.start_grace_secs.get(),
            stop_grace_secs: container.lifecycle.stop_grace_secs.get(),
            healthcheck_path: container.lifecycle.healthcheck_path,
            required_config_names: container.required_config_names,
            required_secret_names: container.required_secret_names,
            config_exports: container.config_exports,
            sandbox_profile_hash: Hash::new(b"prior fixture sandbox"),
            process_generation: 1,
            process_started_sequence: 1,
            signed_by: soracloud_fixture_key_pair(0x20).public_key().to_string(),
        };
        let object = service_status
            .as_object_mut()
            .expect("service status object");
        object.insert(
            "current_version".to_owned(),
            json::to_value(&service.service_version).expect("version"),
        );
        object.insert(
            "latest_revision".to_owned(),
            json::to_value(&revision).expect("revision"),
        );
    }
    decode_network_control_plane_snapshot(&status).expect("canonical prior service status");
    status
}
fn mock_bundle_mutation_server(
    workspace: &Path,
    mutation_route: &str,
    draft_response: &norito::json::Value,
    status_payload: &norito::json::Value,
    pin_encode_error: &str,
    draft_encode_error: &str,
) -> MockHttpServer {
    let upgrading = mutation_route.ends_with("/upgrade");
    let initial_status = if upgrading {
        mock_prior_service_status(workspace, status_payload)
    } else {
        mock_control_plane_status_payload(&[])
    };
    let mut initial_apps = app_infra_status_fixture_for_name("travel_ops");
    if upgrading {
        let app = &mut initial_apps.apps[0];
        app.manifest.app_version = "0.9.0".to_owned();
        app.current_app_version = app.manifest.app_version.clone();
        app.current_manifest_hash = app.manifest.manifest_hash();
        initial_apps.recent_audit_events[0].to_version = app.current_app_version.clone();
        initial_apps.recent_audit_events[0].app_manifest_hash = app.current_manifest_hash;
    } else {
        initial_apps.app_count = 0;
        initial_apps.audit_event_count = 0;
        initial_apps.apps.clear();
        initial_apps.recent_audit_events.clear();
    }
    initial_apps.validate().expect("valid initial app status");
    let initial_routes = BTreeMap::from([
        (
            "/v1/soracloud/status".to_owned(),
            MockHttpResponse::json(json::to_vec(&initial_status).expect("initial status")),
        ),
        (
            "/v1/soracloud/apps/status".to_owned(),
            MockHttpResponse::json(json::to_vec(&initial_apps).expect("initial app status")),
        ),
    ]);
    let routes = BTreeMap::from([
        (
            "/v1/sorafs/pin/register".to_owned(),
            MockHttpResponse::json(
                json::to_vec(&norito::json!({ "ok": true })).expect(pin_encode_error),
            ),
        ),
        (
            mutation_route.to_owned(),
            MockHttpResponse::json(json::to_vec(draft_response).expect(draft_encode_error)),
        ),
        (
            "/v1/soracloud/status".to_owned(),
            MockHttpResponse::json(json::to_vec(status_payload).expect("encode status response")),
        ),
        (
            "/v1/soracloud/apps/travel_ops/status".to_owned(),
            MockHttpResponse::json(
                json::to_vec(&app_infra_status_fixture_for_name("travel_ops"))
                    .expect("encode app-infra status response"),
            ),
        ),
        (
            "/api/v1/health".to_owned(),
            MockHttpResponse {
                status: "200 OK",
                content_type: "application/json",
                body: br#"{"status":"ready"}"#.to_vec(),
            },
        ),
        (
            "/".to_owned(),
            MockHttpResponse {
                status: "200 OK",
                content_type: "text/html",
                body: b"<!doctype html><title>ready</title>".to_vec(),
            },
        ),
    ]);
    MockHttpServer::start_with_mutation_transition(
        routes,
        initial_routes,
        Some(mutation_route.to_owned()),
    )
}
fn point_split_app_live_route_at_mock_server(dir: &Path, base_url: &str) {
    let parsed = reqwest::Url::parse(base_url).expect("parse mock server URL");
    let hostname = parsed.host_str().expect("mock server hostname").to_owned();
    let manifest_path = dir.join("app_manifest.json");
    let mut manifest: SoracloudAppManifestV1 =
        load_json(&manifest_path).expect("load app manifest");
    manifest.public_url = base_url.to_owned();
    write_json(&manifest_path, &manifest).expect("write mock app public URL");
    let service_path = dir.join("services/live/service_manifest.json");
    let mut service: SoraServiceManifestV1 =
        load_json(&service_path).expect("load live service manifest");
    service.route.as_mut().expect("live service route").host = hostname;
    write_json(&service_path, &service).expect("write mock live service route");
}
fn assert_workspace_script(path: Option<&str>, file_name: &str) {
    assert!(path.is_some_and(|path| path.ends_with(file_name)));
}
fn assert_service_workspace_scripts(scripts: &ServiceWorkspaceScriptsOutput) {
    assert_workspace_script(scripts.local_dev.as_deref(), "dev.sh");
    assert_workspace_script(scripts.build_and_sync.as_deref(), "build-and-sync.sh");
    assert_service_mutation_workspace_scripts(scripts);
}
fn assert_service_mutation_workspace_scripts(scripts: &ServiceWorkspaceScriptsOutput) {
    assert_workspace_script(scripts.deploy.as_deref(), "deploy.sh");
    assert_workspace_script(scripts.upgrade.as_deref(), "upgrade.sh");
}
fn assert_app_workspace_scripts(scripts: &AppLocalWorkspaceScriptsOutput) {
    assert_workspace_script(scripts.local_dev.as_deref(), "dev.sh");
    assert_workspace_script(scripts.build_and_sync.as_deref(), "build-and-sync.sh");
    assert_workspace_script(scripts.doctor.as_deref(), "doctor.sh");
    assert_workspace_script(scripts.release.as_deref(), "release.sh");
}
#[test]
fn app_live_verification_rejects_non_success_status() {
    let server = MockHttpServer::start(BTreeMap::new());
    let target = AppLiveVerificationTarget {
        label: "missing healthcheck".to_owned(),
        url: reqwest::Url::parse(&format!("{}/missing", server.base_url))
            .expect("parse live verification URL"),
    };
    let error = verify_app_live_targets(&[target], 1)
        .expect_err("a non-2xx live route must block release success");
    let diagnostic = error.to_string();
    assert!(diagnostic.contains("missing healthcheck"), "{diagnostic}");
    assert!(diagnostic.contains("HTTP 404"), "{diagnostic}");
}
#[test]
fn direct_service_mutation_output_keeps_one_exact_authoritative_snapshot() {
    let status = mock_control_plane_status_payload(&["echo_console"]);
    let key_pair = soracloud_fixture_key_pair(0x34);
    let authority = AccountId::new(key_pair.public_key().clone());
    let response = build_service_mutation_output(
        mock_soracloud_submission_receipt(&authority, &key_pair),
        &status,
        "echo_console",
        "Upgrade",
    )
    .expect("build exact service mutation response");
    let output = build_direct_service_mutation_output(
        Path::new("container_manifest.json"),
        Path::new("service_manifest.json"),
        ServiceWorkspacePlan {
            service_name: "echo_console".to_owned(),
            execution_plane: "HttpService".to_owned(),
            runtime: "Inrou".to_owned(),
            route_host: Some("example.test".to_owned()),
            route_path_prefix: Some("/api/v1".to_owned()),
            route_visibility: Some("Public".to_owned()),
            replica_count: 2,
            state_binding_count: 1,
            lease_volume_count: 0,
            handler_count: 0,
            routes: Vec::new(),
            workspace_dir: ".".to_owned(),
            workspace_scripts: ServiceWorkspaceScriptsOutput {
                local_dev: None,
                build_and_sync: None,
                doctor: None,
                release: None,
                deploy: None,
                upgrade: None,
            },
            notes: Vec::new(),
        },
        MutationMode::Upgrade,
        "http://127.0.0.1:8080",
        false,
        None,
        ServiceBundlePublishOutput {
            service_name: "echo_console".to_owned(),
            bundle_file: "service.tgz".to_owned(),
            content_cid: "bcanary".to_owned(),
            manifest_digest_hex: "aa".repeat(32),
            bundle_hash: Hash::new(b"service bundle").to_string(),
            note: "published".to_owned(),
        },
        Vec::new(),
        response,
    );
    assert_eq!(
        output
            .response
            .get("current_version")
            .and_then(norito::json::Value::as_str),
        Some("1.0.0")
    );
    assert!(
        output
            .response
            .get("active_rollout")
            .is_some_and(|v| v.is_null())
    );
    assert!(
        output
            .response
            .get("last_rollout")
            .is_some_and(|v| v.is_null())
    );
    assert!(
        output
            .response
            .get("submission")
            .is_some_and(json::Value::is_object)
    );
    let encoded = json::to_value(&output).expect("encode exact direct service output");
    let root = encoded.as_object().expect("direct service output object");
    for retired in [
        "rollout_handle",
        "rollout_stage",
        "stage",
        "traffic_percent",
        "current_version",
    ] {
        assert!(
            !root.contains_key(retired),
            "retired flattened field `{retired}` must remain absent"
        );
    }
}
fn mock_hf_status_path(
    base_url: &str,
    repo_id: &str,
    revision: &str,
    storage_class: StorageClass,
    lease_term_ms: u64,
    account_id: Option<&str>,
) -> String {
    let mut endpoint = reqwest::Url::parse(base_url)
        .expect("mock base URL")
        .join("v1/soracloud/hf/lease/status")
        .expect("hf status endpoint");
    {
        let mut query = endpoint.query_pairs_mut();
        query
            .append_pair("repo_id", repo_id)
            .append_pair("revision", revision)
            .append_pair("storage_class", storage_class_query_label(storage_class))
            .append_pair("lease_term_ms", &lease_term_ms.to_string());
        if let Some(account_id) = account_id {
            query.append_pair("account_id", account_id);
        }
    }
    let query = endpoint.query().unwrap_or_default();
    format!("{}?{query}", endpoint.path())
}
#[test]
fn status_output_can_represent_torii_control_plane_snapshot() {
    let payload = norito::json!({
        "schema_version": 1,
        "service_health": {
            "mode": "local_only",
            "status": "not_configured"
        },
        "control_plane": {
            "schema_version": 1,
            "service_count": 0,
            "audit_event_count": 0,
            "active_inrou_hosts": [],
            "services": [],
            "recent_audit_events": []
        }
    });
    let output = StatusOutput::from_network(
        "http://127.0.0.1:8080/v1/soracloud/status".to_owned(),
        payload.clone(),
        None,
    )
    .expect("status output should decode");
    assert_eq!(output.source, "torii_control_plane");
    assert!(output.torii_endpoint.is_some());
    assert_eq!(output.schema_version, Some(1));
    assert_eq!(output.service_count, Some(0));
    assert_eq!(output.audit_event_count, Some(0));
    assert!(output.services.is_empty());
    let payload = output.network_status.expect("network payload");
    assert_eq!(
        payload
            .get("schema_version")
            .and_then(norito::json::Value::as_u64),
        Some(1)
    );
}
#[test]
fn status_output_projects_embedded_control_plane_services() {
    let service_value = json::to_value(&ServiceStatusOutput {
        service_name: "echo_console".to_owned(),
        current_version: "1.2.3".to_owned(),
        revision_count: 4,
        config_generation: 0,
        secret_generation: 0,
        config_entry_count: 0,
        secret_entry_count: 0,
        service_lease: None,
        public_discovery_content_cid: Some("bafyteststatus".to_owned()),
        public_discovery_url: Some(
            "https://taira.sora.org/sorafs/cid/bafyteststatus/index.json".to_owned(),
        ),
        public_discovery_cid_host_url: Some(
            "https://bafyteststatus.sorafs.taira.sora.org/index.json".to_owned(),
        ),
        latest_revision: Some(ControlPlaneServiceRevision {
            sequence: 7,
            action: SoracloudAction::Deploy,
            service_version: "1.2.3".to_owned(),
            service_manifest_hash: Hash::new(b"status-output-service-manifest"),
            container_manifest_hash: Hash::new(b"status-output-container-manifest"),
            replicas: 1,
            execution_plane: SoraServiceExecutionPlaneV1::DeterministicService,
            route_host: Some("taira.sora.org".to_owned()),
            route_path_prefix: Some("/api/v1".to_owned()),
            route_service_port: Some(8080),
            route_visibility: Some("Public".to_owned()),
            route_tls_mode: Some("Required".to_owned()),
            base_url: Some("https://taira.sora.org/api/v1/".to_owned()),
            healthcheck_url: Some("https://taira.sora.org/api/v1/health".to_owned()),
            public_discovery_content_cid: Some("bafyteststatus".to_owned()),
            public_discovery_url: Some(
                "https://taira.sora.org/sorafs/cid/bafyteststatus/index.json".to_owned(),
            ),
            public_discovery_cid_host_url: Some(
                "https://bafyteststatus.sorafs.taira.sora.org/index.json".to_owned(),
            ),
            state_binding_count: 0,
            state_bindings: Vec::new(),
            lease_volumes: Vec::new(),
            allow_model_inference: false,
            allow_model_training: false,
            runtime: SoraContainerRuntimeV1::Ivm,
            allow_state_writes: false,
            network: SoraNetworkPolicyV1::Isolated,
            cpu_millis: 1_000,
            memory_bytes: 256 * 1024 * 1024,
            ephemeral_storage_bytes: 512 * 1024 * 1024,
            max_open_files_per_process: 1_024,
            max_tasks: 64,
            start_grace_secs: 30,
            stop_grace_secs: 30,
            healthcheck_path: Some("/health".to_owned()),
            required_config_names: Vec::new(),
            required_secret_names: Vec::new(),
            config_exports: Vec::new(),
            sandbox_profile_hash: Hash::new(b"status-output-sandbox-profile"),
            process_generation: 1,
            process_started_sequence: 7,
            signed_by: "validator".to_owned(),
        }),
        active_rollout: Some(RolloutRuntimeState {
            rollout_handle: "rollout-1".to_owned(),
            baseline_version: "1.2.2".to_owned(),
            candidate_version: "1.2.3".to_owned(),
            canary_percent: 10,
            traffic_percent: 10,
            stage: RolloutStage::Canary,
            health_failures: 0,
            max_health_failures: 3,
            health_window_secs: 30,
            created_sequence: 7,
            updated_sequence: 8,
        }),
        last_rollout: None,
    })
    .expect("encode service status");
    let audit_value = json::to_value(&NetworkControlPlaneAuditEventV1 {
        sequence: 7,
        action: SoracloudAction::Deploy,
        service_name: "echo_console".to_owned(),
        from_version: None,
        to_version: "1.2.3".to_owned(),
        service_manifest_hash: Hash::new(b"status-output-service-manifest"),
        container_manifest_hash: Hash::new(b"status-output-container-manifest"),
        process_generation: 1,
        config_generation: 0,
        secret_generation: 0,
        config_snapshot_hash: Hash::new(b"status-output-config-snapshot"),
        secret_snapshot_hash: Hash::new(b"status-output-secret-snapshot"),
        binding_name: None,
        state_key: None,
        config_mutations: Vec::new(),
        secret_mutations: Vec::new(),
        governance_tx_hash: None,
        rollout_state: None,
        policy_name: None,
        policy_snapshot_hash: None,
        jurisdiction_tag: None,
        consent_evidence_hash: None,
        break_glass: None,
        break_glass_reason: None,
        lease_usage: None,
        service_lease_commitment: None,
        lease_reporting_epoch_rollover: None,
        signed_by: soracloud_fixture_key_pair(0x61).public_key().to_string(),
    })
    .expect("encode control-plane audit event");
    let payload = norito::json!({
        "schema_version": 1,
        "control_plane": {
            "schema_version": 1,
            "service_count": 1,
            "audit_event_count": 1,
            "active_inrou_hosts": [],
            "services": [(service_value)],
            "recent_audit_events": [(audit_value)]
        }
    });
    let output = StatusOutput::from_network(
        "http://127.0.0.1:8080/v1/soracloud/status".to_owned(),
        payload,
        None,
    )
    .expect("status output should decode");
    assert_eq!(output.schema_version, Some(1));
    assert_eq!(output.service_count, Some(1));
    assert_eq!(output.audit_event_count, Some(1));
    assert_eq!(output.services.len(), 1);
    assert_eq!(output.services[0].service_name, "echo_console");
    assert_eq!(output.services[0].current_version, "1.2.3");
    assert_eq!(output.services[0].revision_count, 4);
    assert!(output.services[0].service_lease.is_none());
    assert_eq!(
        output.services[0]
            .latest_revision
            .as_ref()
            .map(|revision| revision.sequence),
        Some(7)
    );
    assert_eq!(
        output.services[0]
            .active_rollout
            .as_ref()
            .map(|rollout| rollout.rollout_handle.as_str()),
        Some("rollout-1")
    );
}
#[test]
fn status_output_audit_event_requires_current_v1_shape() {
    let event = NetworkControlPlaneAuditEventV1 {
        sequence: 9,
        action: SoracloudAction::ConfigMutation,
        service_name: "echo_console".to_owned(),
        from_version: None,
        to_version: "1.2.3".to_owned(),
        service_manifest_hash: Hash::new(b"current-v1-service-manifest"),
        container_manifest_hash: Hash::new(b"current-v1-container-manifest"),
        process_generation: 3,
        config_generation: 2,
        secret_generation: 1,
        config_snapshot_hash: Hash::new(b"current-v1-config-snapshot"),
        secret_snapshot_hash: Hash::new(b"current-v1-secret-snapshot"),
        binding_name: None,
        state_key: None,
        config_mutations: vec![SoraServiceConfigMutationV1::Delete(
            "runtime/theme".to_owned(),
        )],
        secret_mutations: Vec::new(),
        governance_tx_hash: None,
        rollout_state: None,
        policy_name: None,
        policy_snapshot_hash: None,
        jurisdiction_tag: None,
        consent_evidence_hash: None,
        break_glass: None,
        break_glass_reason: None,
        lease_usage: None,
        service_lease_commitment: None,
        lease_reporting_epoch_rollover: None,
        signed_by: soracloud_fixture_key_pair(0x62).public_key().to_string(),
    };
    event
        .validate()
        .expect("current V1 audit event must satisfy authoritative semantics");
    let canonical = json::to_value(&event).expect("encode current V1 audit event fixture");
    let object = canonical.as_object().expect("audit event JSON object");
    assert_eq!(object.len(), 28, "audit event must expose only V1 fields");
    for required in [
        "process_generation",
        "config_generation",
        "secret_generation",
        "config_snapshot_hash",
        "secret_snapshot_hash",
        "config_mutations",
        "secret_mutations",
        "rollout_state",
        "lease_usage",
        "service_lease_commitment",
    ] {
        assert!(
            object.contains_key(required),
            "missing V1 field `{required}`"
        );
    }
    let decoded: NetworkControlPlaneAuditEventV1 =
        json::from_value(canonical.clone()).expect("decode current V1 audit event fixture");
    assert!(matches!(
        decoded.config_mutations.as_slice(),
        [SoraServiceConfigMutationV1::Delete(name)] if name == "runtime/theme"
    ));

    for retired in ["config_name", "secret_name", "rollout_handle"] {
        let mut retired_fixture = canonical.clone();
        retired_fixture
            .as_object_mut()
            .expect("audit event JSON object")
            .insert(retired.to_owned(), json::Value::Null);
        json::from_value::<NetworkControlPlaneAuditEventV1>(retired_fixture)
            .expect_err("retired audit fields must fail closed");
    }

    let status_with_event = |event| {
        norito::json!({
            "schema_version": 1,
            "control_plane": {
                "schema_version": 1,
                "service_count": 0,
                "audit_event_count": 1,
                "active_inrou_hosts": [],
                "services": [],
                "recent_audit_events": [event]
            }
        })
    };
    decode_network_control_plane_snapshot(&status_with_event(canonical.clone()))
        .expect("canonical current V1 audit event must pass the status boundary");
    for (label, field, replacement) in [
        (
            "config mutation with a previous version",
            "from_version",
            json::Value::from("1.2.2"),
        ),
        (
            "config mutation with no exact delta",
            "config_mutations",
            json::Value::Array(Vec::new()),
        ),
        (
            "config mutation with a lease commitment",
            "service_lease_commitment",
            json::to_value(&Hash::new(b"forbidden-service-lease-commitment"))
                .expect("encode forbidden lease commitment"),
        ),
        (
            "persisted read-only action",
            "action",
            norito::json!({"action": "CiphertextQuery", "value": null}),
        ),
        (
            "zero process generation",
            "process_generation",
            json::Value::from(0_u64),
        ),
        (
            "non-key signer",
            "signed_by",
            json::Value::from("validator"),
        ),
    ] {
        let mut invalid = canonical.clone();
        invalid
            .as_object_mut()
            .expect("audit event JSON object")
            .insert(field.to_owned(), replacement);
        let _ =
            decode_network_control_plane_snapshot(&status_with_event(invalid)).expect_err(label);
    }
}
#[test]
fn status_output_rejects_malformed_embedded_service_snapshot() {
    let payload = norito::json!({
        "schema_version": 1,
        "control_plane": {
            "schema_version": 1,
            "service_count": 1,
            "audit_event_count": 0,
            "active_inrou_hosts": [],
            "services": [
                {
                    "service_name": "echo_console"
                }
            ],
            "recent_audit_events": []
        }
    });
    let error = StatusOutput::from_network(
        "http://127.0.0.1:8080/v1/soracloud/status".to_owned(),
        payload,
        None,
    )
    .expect_err("malformed embedded service snapshot must fail");
    assert!(
        error
            .to_string()
            .contains("failed to decode canonical Soracloud `control_plane` status")
    );
}
#[test]
fn status_output_rejects_retired_flattened_service_lease_fields() {
    let mut payload = mock_control_plane_status_payload(&["echo_console"]);
    payload
        .pointer_mut("/control_plane/services/0")
        .and_then(norito::json::Value::as_object_mut)
        .expect("canonical service snapshot")
        .insert("quota_class".to_owned(), norito::json::Value::Null);
    let error = StatusOutput::from_network(
        "http://127.0.0.1:8080/v1/soracloud/status".to_owned(),
        payload,
        None,
    )
    .expect_err("retired flattened lease fields must fail closed");
    assert!(
        error
            .to_string()
            .contains("failed to decode canonical Soracloud `control_plane` status"),
        "unexpected retired-field error: {error:#}"
    );
}
#[test]
fn status_output_requires_the_canonical_control_plane_shape() {
    let endpoint = "http://127.0.0.1:8080/v1/soracloud/status".to_owned();
    let missing = StatusOutput::from_network(
        endpoint.clone(),
        norito::json!({ "schema_version": 1 }),
        None,
    )
    .expect_err("missing control-plane status must fail");
    assert!(missing.to_string().contains("missing `control_plane`"));

    let canonical = norito::json!({
        "schema_version": 1,
        "control_plane": {
            "schema_version": 1,
            "service_count": 0,
            "audit_event_count": 0,
            "active_inrou_hosts": [],
            "services": [],
            "recent_audit_events": []
        }
    });
    for field in [
        "schema_version",
        "service_count",
        "audit_event_count",
        "active_inrou_hosts",
        "services",
        "recent_audit_events",
    ] {
        let mut omitted = canonical.clone();
        omitted
            .get_mut("control_plane")
            .and_then(norito::json::Value::as_object_mut)
            .expect("control-plane object")
            .remove(field);
        let _error = StatusOutput::from_network(endpoint.clone(), omitted, None)
            .expect_err("an omitted canonical control-plane key must fail");
    }

    let mut unknown = canonical.clone();
    unknown
        .get_mut("control_plane")
        .and_then(norito::json::Value::as_object_mut)
        .expect("control-plane object")
        .insert("legacy_services".to_owned(), norito::json::Value::Null);
    let _error = StatusOutput::from_network(endpoint.clone(), unknown, None)
        .expect_err("an unknown control-plane key must fail");

    let mut malformed = canonical.clone();
    malformed
        .get_mut("control_plane")
        .and_then(norito::json::Value::as_object_mut)
        .expect("control-plane object")
        .insert("services".to_owned(), norito::json::Value::Null);
    let _error = StatusOutput::from_network(endpoint.clone(), malformed, None)
        .expect_err("a malformed control-plane service list must fail");

    let mut inconsistent = canonical;
    inconsistent
        .get_mut("control_plane")
        .and_then(norito::json::Value::as_object_mut)
        .expect("control-plane object")
        .insert("service_count".to_owned(), norito::json!(1));
    let error = StatusOutput::from_network(endpoint, inconsistent, None)
        .expect_err("a mismatched service count must fail");
    assert!(error.to_string().contains("does not match"));
}
#[test]
fn filter_soracloud_status_payload_filters_embedded_control_plane_snapshot() {
    let mut payload = mock_control_plane_status_payload(&["alpha", "beta"]);
    let audit_events = ["alpha", "beta"]
        .into_iter()
        .enumerate()
        .map(|(index, service_name)| NetworkControlPlaneAuditEventV1 {
            sequence: u64::try_from(index + 1).expect("small sequence"),
            action: SoracloudAction::Deploy,
            service_name: service_name.to_owned(),
            from_version: None,
            to_version: "1.0.0".to_owned(),
            service_manifest_hash: Hash::new(service_name.as_bytes()),
            container_manifest_hash: Hash::new(service_name.as_bytes()),
            process_generation: 1,
            config_generation: 0,
            secret_generation: 0,
            config_snapshot_hash: Hash::new(format!("{service_name}:config-snapshot").as_bytes()),
            secret_snapshot_hash: Hash::new(format!("{service_name}:secret-snapshot").as_bytes()),
            binding_name: None,
            state_key: None,
            config_mutations: Vec::new(),
            secret_mutations: Vec::new(),
            governance_tx_hash: None,
            rollout_state: None,
            policy_name: None,
            policy_snapshot_hash: None,
            jurisdiction_tag: None,
            consent_evidence_hash: None,
            break_glass: None,
            break_glass_reason: None,
            lease_usage: None,
            service_lease_commitment: None,
            lease_reporting_epoch_rollover: None,
            signed_by: soracloud_fixture_key_pair(0x63).public_key().to_string(),
        })
        .collect::<Vec<_>>();
    let control_plane = payload
        .get_mut("control_plane")
        .and_then(norito::json::Value::as_object_mut)
        .expect("control_plane object");
    control_plane.insert(
        "audit_event_count".to_owned(),
        norito::json::Value::from(2_u64),
    );
    control_plane.insert(
        "recent_audit_events".to_owned(),
        json::to_value(&audit_events).expect("encode canonical audit events"),
    );
    filter_soracloud_status_payload(&mut payload, Some("beta"))
        .expect("filter canonical control-plane status");
    let control_plane = payload
        .get("control_plane")
        .and_then(norito::json::Value::as_object)
        .expect("control_plane object");
    assert_eq!(
        control_plane
            .get("service_count")
            .and_then(norito::json::Value::as_u64),
        Some(1)
    );
    let services = control_plane
        .get("services")
        .and_then(norito::json::Value::as_array)
        .expect("services array");
    assert_eq!(services.len(), 1);
    assert_eq!(
        services[0]
            .get("service_name")
            .and_then(norito::json::Value::as_str),
        Some("beta")
    );
    let audit_events = control_plane
        .get("recent_audit_events")
        .and_then(norito::json::Value::as_array)
        .expect("recent_audit_events array");
    assert_eq!(audit_events.len(), 1);
    assert_eq!(
        audit_events[0]
            .get("service_name")
            .and_then(norito::json::Value::as_str),
        Some("beta")
    );
}
#[test]
fn status_args_can_resolve_service_filter_from_manifest_pair() {
    let (dir, _) = service_fixture(
        "status_service_filter_from_manifest_pair",
        InitTemplate::HttpService,
    );
    let status_payload = mock_control_plane_status_payload(&["echo_console", "unrelated_service"]);
    let server = MockHttpServer::start(BTreeMap::from([(
        "/v1/soracloud/status".to_owned(),
        MockHttpResponse::json(json::to_vec(&status_payload).expect("encode status response")),
    )]));
    install_mock_protected_read_signer();
    let output = StatusArgs {
        service_name: None,
        container: Some(dir.join("container_manifest.json")),
        service: Some(dir.join("service_manifest.json")),
        torii_url: Some(server.base_url.clone()),
        api_token: None,
        timeout_secs: 5,
    }
    .run()
    .expect("status should succeed");
    assert_eq!(output.schema_version, Some(1));
    assert_eq!(output.service_count, Some(1));
    assert_eq!(output.audit_event_count, Some(0));
    assert_eq!(output.services.len(), 1);
    assert_eq!(output.services[0].service_name, "echo_console");
    assert_eq!(output.services[0].current_version, "1.0.0");
    let payload = output.network_status.expect("network payload");
    let services = payload
        .get("control_plane")
        .and_then(norito::json::Value::as_object)
        .and_then(|control_plane| control_plane.get("services"))
        .and_then(norito::json::Value::as_array)
        .expect("filtered services");
    assert_eq!(services.len(), 1);
    assert_eq!(
        services[0]
            .get("service_name")
            .and_then(norito::json::Value::as_str),
        Some("echo_console")
    );
    let service_plan = output.service_plan.expect("manifest-backed service plan");
    assert_eq!(service_plan.service_name, "echo_console");
    assert_eq!(service_plan.execution_plane, "HttpService");
    assert_eq!(service_plan.runtime, "Inrou");
    assert_eq!(service_plan.route_path_prefix.as_deref(), Some("/api/v1"));
    assert_optional_path_ends_with(
        service_plan.workspace_scripts.local_dev.as_deref(),
        "dev.sh",
    );
}
#[test]
fn status_args_reject_conflicting_service_name_against_manifest_pair() {
    let (dir, _) = service_fixture(
        "status_service_name_manifest_mismatch",
        InitTemplate::HttpService,
    );
    let server = MockHttpServer::start(BTreeMap::new());
    let error = StatusArgs {
        service_name: Some("wrong_name".to_owned()),
        container: Some(dir.join("container_manifest.json")),
        service: Some(dir.join("service_manifest.json")),
        torii_url: Some(server.base_url.clone()),
        api_token: None,
        timeout_secs: 5,
    }
    .run()
    .expect_err("conflicting manifest filter should fail");
    assert!(error.to_string().contains("wrong_name"));
    assert!(error.to_string().contains("echo_console"));
    assert!(
        server.requests().is_empty(),
        "manifest mismatch should fail before any network request"
    );
}
#[test]
fn resolve_required_workspace_service_name_requires_explicit_or_manifest_identity() {
    let error = resolve_required_workspace_service_name(
        None,
        None,
        None,
        "iroha soracloud service config-status",
    )
    .expect_err("missing service identity should fail");
    assert!(error.to_string().contains("--service-name"));
    assert!(error.to_string().contains("--container"));
    assert!(error.to_string().contains("--service"));
    let _error = resolve_required_workspace_service_name(
        Some(" echo_console".to_owned()),
        None,
        None,
        "iroha soracloud service config-status",
    )
    .expect_err("service-name surrounding whitespace must fail rather than be trimmed");
    let _error = resolve_required_workspace_service_name(
        Some("cafe\u{301}".to_owned()),
        None,
        None,
        "iroha soracloud service config-status",
    )
    .expect_err("service-name NFC aliases must fail rather than be normalized");
}
#[test]
fn config_status_args_can_resolve_service_name_from_manifest_pair() {
    let (dir, _) = service_fixture(
        "config_status_service_filter_from_manifest_pair",
        InitTemplate::HttpService,
    );
    let response = service_config_status_fixture("echo_console", "demo_config");
    let server = MockHttpServer::start(BTreeMap::from([(
        "/v1/soracloud/service/config/status?service_name=echo_console&config_name=demo_config"
            .to_owned(),
        MockHttpResponse::json(json::to_vec(&response).expect("encode config status response")),
    )]));
    install_mock_protected_read_signer();
    let output = ConfigStatusArgs {
        service_name: None,
        container: Some(dir.join("container_manifest.json")),
        service: Some(dir.join("service_manifest.json")),
        config_name: Some("demo_config".to_owned()),
        torii_url: Some(server.base_url.clone()),
        api_token: Some("token".to_owned()),
        timeout_secs: 5,
    }
    .run()
    .expect("config-status should succeed");
    assert_manifest_pair_output(&output, "build_and_sync", "build-and-sync.sh");
    let request = server
        .requests()
        .into_iter()
        .find(|request| request.method == "GET")
        .expect("config-status request");
    assert_eq!(
        request.path,
        "/v1/soracloud/service/config/status?service_name=echo_console&config_name=demo_config"
    );
}
#[test]
fn secret_status_args_can_resolve_service_name_from_manifest_pair() {
    let (dir, _) = service_fixture(
        "secret_status_service_filter_from_manifest_pair",
        InitTemplate::HttpService,
    );
    let response = service_secret_status_fixture("echo_console", "demo_secret");
    let server = MockHttpServer::start(BTreeMap::from([(
        "/v1/soracloud/service/secret/status?service_name=echo_console&secret_name=demo_secret"
            .to_owned(),
        MockHttpResponse::json(json::to_vec(&response).expect("encode secret status response")),
    )]));
    install_mock_protected_read_signer();
    let output = SecretStatusArgs {
        service_name: None,
        container: Some(dir.join("container_manifest.json")),
        service: Some(dir.join("service_manifest.json")),
        secret_name: Some("demo_secret".to_owned()),
        torii_url: Some(server.base_url.clone()),
        api_token: Some("token".to_owned()),
        timeout_secs: 5,
    }
    .run()
    .expect("secret-status should succeed");
    assert_manifest_pair_output(&output, "upgrade", "upgrade.sh");
    let request = server
        .requests()
        .into_iter()
        .find(|request| request.method == "GET")
        .expect("secret-status request");
    assert_eq!(
        request.path,
        "/v1/soracloud/service/secret/status?service_name=echo_console&secret_name=demo_secret"
    );
}
#[test]
fn rollback_args_can_resolve_service_name_from_manifest_pair() {
    let (dir, _) = service_fixture(
        "rollback_service_name_from_manifest_pair",
        InitTemplate::HttpService,
    );
    let key_pair = soracloud_fixture_key_pair(0x14);
    let authority = AccountId::new(key_pair.public_key().clone());
    let status_payload = mock_control_plane_status_payload(&["echo_console"]);
    let rollback_response = mock_soracloud_draft_response(&authority, &key_pair);
    let server = MockHttpServer::start(BTreeMap::from([
        (
            "/v1/soracloud/rollback".to_owned(),
            MockHttpResponse::json(
                json::to_vec(&rollback_response).expect("encode rollback response"),
            ),
        ),
        (
            "/v1/soracloud/status".to_owned(),
            MockHttpResponse::json(json::to_vec(&status_payload).expect("encode status response")),
        ),
    ]));
    install_mock_submission_config(&authority, &key_pair);
    let output = RollbackArgs {
        service_name: None,
        container: Some(dir.join("container_manifest.json")),
        service: Some(dir.join("service_manifest.json")),
        target_version: "0.9.0".to_owned(),
        torii_url: Some(server.base_url.clone()),
        api_token: None,
        timeout_secs: 5,
    }
    .run(&authority, &key_pair)
    .expect("rollback should succeed");
    assert_manifest_pair_output(&output, "local_dev", "dev.sh");
    assert_captured_payload_service_name(
        &server,
        "/v1/soracloud/rollback",
        "rollback request",
        "decode rollback request",
    );
}
#[test]
fn rollout_args_accept_deterministic_ivm_manifest_pair() {
    let (dir, _) = service_fixture(
        "rollout_deterministic_ivm_manifest_pair",
        InitTemplate::Baseline,
    );
    let key_pair = soracloud_fixture_key_pair(0x15);
    let authority = AccountId::new(key_pair.public_key().clone());
    let mut status_payload = mock_control_plane_status_payload(&["echo_console"]);
    let service = status_payload
        .get_mut("control_plane")
        .and_then(json::Value::as_object_mut)
        .and_then(|control_plane| control_plane.get_mut("services"))
        .and_then(json::Value::as_array_mut)
        .and_then(|services| services.first_mut())
        .and_then(json::Value::as_object_mut)
        .expect("exact rollout service status fixture");
    service.insert("revision_count".to_owned(), json::Value::from(2_u32));
    service.insert(
        "active_rollout".to_owned(),
        json::to_value(&RolloutRuntimeState {
            rollout_handle: "echo_console:rollout:2".to_owned(),
            baseline_version: "1.0.0".to_owned(),
            candidate_version: "1.1.0".to_owned(),
            canary_percent: 10,
            traffic_percent: 50,
            stage: RolloutStage::Promoted,
            health_failures: 0,
            max_health_failures: 1,
            health_window_secs: 60,
            created_sequence: 1,
            updated_sequence: 2,
        })
        .expect("encode exact rollout state"),
    );
    let rollout_response = mock_soracloud_draft_response(&authority, &key_pair);
    let server = MockHttpServer::start(BTreeMap::from([
        (
            "/v1/soracloud/rollout".to_owned(),
            MockHttpResponse::json(
                json::to_vec(&rollout_response).expect("encode rollout response"),
            ),
        ),
        (
            "/v1/soracloud/status".to_owned(),
            MockHttpResponse::json(json::to_vec(&status_payload).expect("encode status response")),
        ),
    ]));
    install_mock_submission_config(&authority, &key_pair);
    let output = RolloutArgs {
        service_name: None,
        container: Some(dir.join("container_manifest.json")),
        service: Some(dir.join("service_manifest.json")),
        rollout_handle: "echo_console:rollout:2".to_owned(),
        health: RolloutHealth::Healthy,
        promote_to_percent: Some(100),
        governance_tx_hash: Hash::new(b"governance"),
        torii_url: Some(server.base_url.clone()),
        api_token: None,
        timeout_secs: 5,
    }
    .run(&authority, &key_pair)
    .expect("rollout should succeed");
    assert_eq!(
        output
            .get("service_name")
            .and_then(norito::json::Value::as_str),
        Some("echo_console")
    );
    let service_plan = output
        .get("service_plan")
        .and_then(norito::json::Value::as_object)
        .expect("manifest-backed deterministic IVM service plan");
    assert_eq!(
        service_plan
            .get("execution_plane")
            .and_then(norito::json::Value::as_str),
        Some("DeterministicService")
    );
    assert_eq!(
        service_plan
            .get("runtime")
            .and_then(norito::json::Value::as_str),
        Some("Ivm")
    );
    assert_captured_payload_service_name(
        &server,
        "/v1/soracloud/rollout",
        "rollout request",
        "decode rollout request",
    );
}
#[test]
fn rollout_args_reject_inrou_manifest_pair_before_network_request() {
    let (dir, _) = service_fixture(
        "rollout_rejects_inrou_manifest_pair",
        InitTemplate::HttpService,
    );
    let key_pair = soracloud_fixture_key_pair(0x35);
    let authority = AccountId::new(key_pair.public_key().clone());
    let server = MockHttpServer::start(BTreeMap::new());
    install_mock_submission_config(&authority, &key_pair);
    let error = RolloutArgs {
        service_name: None,
        container: Some(dir.join("container_manifest.json")),
        service: Some(dir.join("service_manifest.json")),
        rollout_handle: "echo_console:rollout:2".to_owned(),
        health: RolloutHealth::Healthy,
        promote_to_percent: Some(100),
        governance_tx_hash: Hash::new(b"governance"),
        torii_url: Some(server.base_url.clone()),
        api_token: None,
        timeout_secs: 5,
    }
    .run(&authority, &key_pair)
    .expect_err("first-release Inrou rollout must fail locally");
    let diagnostic = error.to_string();
    assert!(diagnostic.contains("first-release HttpService + Inrou"));
    assert!(diagnostic.contains("atomic exact-revision upgrades"));
    assert!(diagnostic.contains("does not accept Inrou manifest pairs"));
    assert!(
        server.requests().is_empty(),
        "Inrou rollout rejection must happen before any network request"
    );
}
#[test]
fn hf_shared_lease_join_args_can_resolve_service_name_from_manifest_pair() {
    let (dir, _) = service_fixture(
        "hf_shared_lease_join_service_name_from_manifest_pair",
        InitTemplate::HttpService,
    );
    let key_pair = soracloud_fixture_key_pair(0x16);
    let authority = AccountId::new(key_pair.public_key().clone());
    let authority_id = authority.to_string();
    let status_path = mock_hf_status_path(
        "http://127.0.0.1:1",
        "openai/gpt-oss",
        TEST_HF_COMMIT_OID,
        StorageClass::Warm,
        604_800_000,
        Some(&authority_id),
    );
    let status_payload = hf_status_fixture();
    let deploy_response = mock_soracloud_draft_response(&authority, &key_pair);
    let server = MockHttpServer::start(BTreeMap::from([
        (
            "/v1/soracloud/hf/lease/join".to_owned(),
            MockHttpResponse::json(
                json::to_vec(&deploy_response).expect("encode hf shared-lease join response"),
            ),
        ),
        (
            status_path.replace("http://127.0.0.1:1", ""),
            MockHttpResponse::json(
                json::to_vec(&status_payload).expect("encode hf status response"),
            ),
        ),
    ]));
    install_mock_submission_config(&authority, &key_pair);
    let output = HfSharedLeaseJoinArgs {
        repo_id: "openai/gpt-oss".to_owned(),
        revision: TEST_HF_COMMIT_OID.to_owned(),
        service_name: None,
        container: Some(dir.join("container_manifest.json")),
        service: Some(dir.join("service_manifest.json")),
        apartment_name: None,
        storage_class: HfStorageClassArg::Warm,
        lease_term_ms: 604_800_000,
        lease_asset_definition: hf_shared_lease_asset_definition().to_string(),
        base_fee: "0.00001".parse().expect("canonical exact base fee"),
        torii_url: Some(server.base_url.clone()),
        api_token: None,
        timeout_secs: 5,
    }
    .run(&authority, &key_pair)
    .expect("hf shared-lease join should succeed");
    assert_manifest_pair_service_plan(&output);
    let deploy_body = assert_captured_payload_service_name(
        &server,
        "/v1/soracloud/hf/lease/join",
        "hf shared-lease join request",
        "decode hf shared-lease join request",
    );
    let deploy_payload = deploy_body
        .get("payload")
        .and_then(norito::json::Value::as_object)
        .expect("HF shared-lease join payload object");
    assert_eq!(
        deploy_payload
            .get("base_fee")
            .and_then(norito::json::Value::as_str),
        Some("0.00001")
    );
    assert!(!deploy_payload.contains_key("base_fee_nanos"));
}
#[test]
fn hf_status_args_can_attach_service_plan_from_manifest_pair() {
    let (dir, _) = service_fixture(
        "hf_status_service_plan_from_manifest_pair",
        InitTemplate::HttpService,
    );
    let status_payload = hf_status_fixture();
    let status_path = mock_hf_status_path(
        "http://127.0.0.1:1",
        "openai/gpt-oss",
        TEST_HF_COMMIT_OID,
        StorageClass::Warm,
        604_800_000,
        None,
    );
    let server = MockHttpServer::start(BTreeMap::from([(
        status_path.replace("http://127.0.0.1:1", ""),
        MockHttpResponse::json(json::to_vec(&status_payload).expect("encode hf status response")),
    )]));
    install_mock_protected_read_signer();
    let output = HfStatusArgs {
        repo_id: "openai/gpt-oss".to_owned(),
        revision: TEST_HF_COMMIT_OID.to_owned(),
        storage_class: HfStorageClassArg::Warm,
        lease_term_ms: 604_800_000,
        account_id: None,
        container: Some(dir.join("container_manifest.json")),
        service: Some(dir.join("service_manifest.json")),
        torii_url: Some(server.base_url.clone()),
        api_token: None,
        timeout_secs: 5,
    }
    .run()
    .expect("hf status should succeed");
    assert_manifest_pair_service_plan(&output);
    assert_eq!(
        output
            .get("source")
            .and_then(norito::json::Value::as_object)
            .and_then(|source| source.get("repo_id"))
            .and_then(norito::json::Value::as_str),
        Some("openai/gpt-oss")
    );
    let request = server
        .requests()
        .into_iter()
        .find(|request| request.method == "GET")
        .expect("hf status request");
    assert_eq!(
        request.path,
        mock_hf_status_path(
            &server.base_url,
            "openai/gpt-oss",
            TEST_HF_COMMIT_OID,
            StorageClass::Warm,
            604_800_000,
            None,
        )
    );
}
#[test]
fn hf_lease_leave_args_can_resolve_service_name_from_manifest_pair() {
    let (dir, _) = service_fixture(
        "hf_lease_leave_service_name_from_manifest_pair",
        InitTemplate::HttpService,
    );
    let key_pair = soracloud_fixture_key_pair(0x17);
    let authority = AccountId::new(key_pair.public_key().clone());
    let authority_id = authority.to_string();
    let status_path = mock_hf_status_path(
        "http://127.0.0.1:1",
        "openai/gpt-oss",
        TEST_HF_COMMIT_OID,
        StorageClass::Warm,
        604_800_000,
        Some(&authority_id),
    );
    let status_payload = hf_status_fixture();
    let leave_response = mock_soracloud_draft_response(&authority, &key_pair);
    let server = MockHttpServer::start(BTreeMap::from([
        (
            "/v1/soracloud/hf/lease/leave".to_owned(),
            MockHttpResponse::json(
                json::to_vec(&leave_response).expect("encode hf leave response"),
            ),
        ),
        (
            status_path.replace("http://127.0.0.1:1", ""),
            MockHttpResponse::json(
                json::to_vec(&status_payload).expect("encode hf status response"),
            ),
        ),
    ]));
    install_mock_submission_config(&authority, &key_pair);
    let output = HfLeaseLeaveArgs {
        repo_id: "openai/gpt-oss".to_owned(),
        revision: TEST_HF_COMMIT_OID.to_owned(),
        storage_class: HfStorageClassArg::Warm,
        lease_term_ms: 604_800_000,
        service_name: None,
        container: Some(dir.join("container_manifest.json")),
        service: Some(dir.join("service_manifest.json")),
        apartment_name: None,
        torii_url: Some(server.base_url.clone()),
        api_token: None,
        timeout_secs: 5,
    }
    .run(&authority, &key_pair)
    .expect("hf lease leave should succeed");
    assert_manifest_pair_service_plan(&output);
    assert_captured_payload_service_name(
        &server,
        "/v1/soracloud/hf/lease/leave",
        "hf lease leave request",
        "decode hf leave request",
    );
}
#[test]
fn hf_lease_renew_args_can_resolve_service_name_from_manifest_pair() {
    let (dir, _) = service_fixture(
        "hf_lease_renew_service_name_from_manifest_pair",
        InitTemplate::HttpService,
    );
    let key_pair = soracloud_fixture_key_pair(0x1F);
    let authority = AccountId::new(key_pair.public_key().clone());
    let authority_id = authority.to_string();
    let status_path = mock_hf_status_path(
        "http://127.0.0.1:1",
        "openai/gpt-oss",
        TEST_HF_COMMIT_OID,
        StorageClass::Warm,
        604_800_000,
        Some(&authority_id),
    );
    let status_payload = hf_status_fixture();
    let renew_response = mock_soracloud_draft_response(&authority, &key_pair);
    let server = MockHttpServer::start(BTreeMap::from([
        (
            "/v1/soracloud/hf/lease/renew".to_owned(),
            MockHttpResponse::json(
                json::to_vec(&renew_response).expect("encode hf renew response"),
            ),
        ),
        (
            status_path.replace("http://127.0.0.1:1", ""),
            MockHttpResponse::json(
                json::to_vec(&status_payload).expect("encode hf status response"),
            ),
        ),
    ]));
    install_mock_submission_config(&authority, &key_pair);
    let output = HfLeaseRenewArgs {
        repo_id: "openai/gpt-oss".to_owned(),
        revision: TEST_HF_COMMIT_OID.to_owned(),
        service_name: None,
        container: Some(dir.join("container_manifest.json")),
        service: Some(dir.join("service_manifest.json")),
        apartment_name: None,
        storage_class: HfStorageClassArg::Warm,
        lease_term_ms: 604_800_000,
        lease_asset_definition: hf_shared_lease_asset_definition().to_string(),
        base_fee: "0.00001".parse().expect("canonical exact base fee"),
        torii_url: Some(server.base_url.clone()),
        api_token: None,
        timeout_secs: 5,
    }
    .run(&authority, &key_pair)
    .expect("hf lease renew should succeed");
    assert_manifest_pair_service_plan(&output);
    let renew_body = assert_captured_payload_service_name(
        &server,
        "/v1/soracloud/hf/lease/renew",
        "hf lease renew request",
        "decode hf renew request",
    );
    let renew_payload = renew_body
        .get("payload")
        .and_then(norito::json::Value::as_object)
        .expect("HF renew payload object");
    assert_eq!(
        renew_payload
            .get("base_fee")
            .and_then(norito::json::Value::as_str),
        Some("0.00001")
    );
    assert!(!renew_payload.contains_key("base_fee_nanos"));
}
