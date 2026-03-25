//! Canonical fixture and roundtrip checks for `SoraCloud` V1 manifests.

use std::{
    collections::BTreeMap,
    fmt::Debug,
    fs,
    num::{NonZeroU16, NonZeroU32, NonZeroU64},
    path::Path,
};

use iroha_crypto::Hash;
use iroha_data_model::{
    Decode, Encode,
    soracloud::{
        AGENT_APARTMENT_MANIFEST_VERSION_V1, AgentApartmentManifestV1, AgentSpendLimitV1,
        AgentToolCapabilityV1, AgentUpgradePolicyV1, CIPHERTEXT_QUERY_PROOF_VERSION_V1,
        CIPHERTEXT_QUERY_RESPONSE_VERSION_V1, CIPHERTEXT_QUERY_SPEC_VERSION_V1,
        CIPHERTEXT_STATE_RECORD_VERSION_V1, CiphertextInclusionProofV1,
        CiphertextQueryMetadataLevelV1, CiphertextQueryResponseV1, CiphertextQueryResultItemV1,
        CiphertextQuerySpecV1, CiphertextStateMetadataV1, CiphertextStateRecordV1,
        DECRYPTION_AUTHORITY_POLICY_VERSION_V1, DECRYPTION_REQUEST_VERSION_V1,
        DecryptionAuthorityModeV1, DecryptionAuthorityPolicyV1, DecryptionRequestV1,
        FHE_EXECUTION_POLICY_VERSION_V1, FHE_GOVERNANCE_BUNDLE_VERSION_V1, FHE_JOB_SPEC_VERSION_V1,
        FHE_PARAM_SET_VERSION_V1, FheDeterministicRoundingModeV1, FheExecutionPolicyV1,
        FheGovernanceBundleV1, FheJobInputRefV1, FheJobOperationV1, FheJobSpecV1,
        FheParamLifecycleV1, FheParamSetV1, FheSchemeV1, SECRET_ENVELOPE_VERSION_V1,
        SORA_CONTAINER_MANIFEST_VERSION_V1, SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
        SORA_SERVICE_MANIFEST_VERSION_V1, SORA_STATE_BINDING_VERSION_V1,
        SecretEnvelopeEncryptionV1, SecretEnvelopeV1, SoraArtifactKindV1, SoraArtifactRefV1,
        SoraCapabilityPolicyV1, SoraCertifiedResponsePolicyV1, SoraContainerManifestRefV1,
        SoraContainerManifestV1, SoraContainerRuntimeV1, SoraDeploymentBundleV1,
        SoraLifecycleHooksV1, SoraMailboxContractV1, SoraNetworkPolicyV1, SoraResourceLimitsV1,
        SoraRolloutPolicyV1, SoraRouteTargetV1, SoraRouteVisibilityV1, SoraServiceHandlerClassV1,
        SoraServiceHandlerV1, SoraServiceManifestV1, SoraStateBindingV1, SoraStateEncryptionV1,
        SoraStateMutabilityV1, SoraStateScopeV1, SoraTlsModeV1,
    },
};
#[cfg(feature = "json")]
use norito::json::{self, FastJsonWrite, JsonDeserialize, JsonSerialize};

const CONTAINER_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/soracloud/sora_container_manifest_v1.json"
));
const SERVICE_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/soracloud/sora_service_manifest_v1.json"
));
const STATE_BINDING_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/soracloud/sora_state_binding_v1.json"
));
const DEPLOYMENT_BUNDLE_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/soracloud/sora_deployment_bundle_v1.json"
));
const AGENT_APARTMENT_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/soracloud/agent_apartment_manifest_v1.json"
));
const FHE_PARAM_SET_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/soracloud/fhe_param_set_v1.json"
));
const FHE_EXECUTION_POLICY_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/soracloud/fhe_execution_policy_v1.json"
));
const FHE_GOVERNANCE_BUNDLE_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/soracloud/fhe_governance_bundle_v1.json"
));
const SECRET_ENVELOPE_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/soracloud/secret_envelope_v1.json"
));
const CIPHERTEXT_STATE_RECORD_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/soracloud/ciphertext_state_record_v1.json"
));
const FHE_JOB_SPEC_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/soracloud/fhe_job_spec_v1.json"
));
const DECRYPTION_AUTHORITY_POLICY_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/soracloud/decryption_authority_policy_v1.json"
));
const DECRYPTION_REQUEST_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/soracloud/decryption_request_v1.json"
));
const CIPHERTEXT_QUERY_SPEC_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/soracloud/ciphertext_query_spec_v1.json"
));
const CIPHERTEXT_QUERY_RESPONSE_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/soracloud/ciphertext_query_response_v1.json"
));

fn sample_hash(seed: u8) -> Hash {
    let mut bytes = [0u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = seed.wrapping_add(u8::try_from(index).expect("index fits in u8"));
    }
    Hash::prehashed(bytes)
}

fn expected_state_binding() -> SoraStateBindingV1 {
    SoraStateBindingV1 {
        schema_version: SORA_STATE_BINDING_VERSION_V1,
        binding_name: "session_store".parse().expect("valid name"),
        scope: SoraStateScopeV1::ServiceState,
        mutability: SoraStateMutabilityV1::ReadWrite,
        encryption: SoraStateEncryptionV1::ClientCiphertext,
        key_prefix: "/state/session".to_string(),
        max_item_bytes: NonZeroU64::new(4_096).expect("nonzero"),
        max_total_bytes: NonZeroU64::new(262_144).expect("nonzero"),
    }
}

fn expected_container_manifest() -> SoraContainerManifestV1 {
    SoraContainerManifestV1 {
        schema_version: SORA_CONTAINER_MANIFEST_VERSION_V1,
        runtime: SoraContainerRuntimeV1::Ivm,
        bundle_hash: sample_hash(7),
        bundle_path: "/bundles/webapp.to".to_string(),
        entrypoint: "main".to_string(),
        args: vec!["--http".to_string(), "--port=8080".to_string()],
        env: BTreeMap::from([
            ("APP_ENV".to_string(), "production".to_string()),
            ("LOG_LEVEL".to_string(), "info".to_string()),
        ]),
        required_config_names: Vec::new(),
        required_secret_names: Vec::new(),
        capabilities: SoraCapabilityPolicyV1 {
            network: SoraNetworkPolicyV1::Allowlist(vec![
                "api.sora.internal".to_string(),
                "wallet.sora.internal".to_string(),
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
            stop_grace_secs: NonZeroU32::new(20).expect("nonzero"),
            healthcheck_path: Some("/healthz".to_string()),
        },
    }
}

fn expected_service_manifest() -> SoraServiceManifestV1 {
    SoraServiceManifestV1 {
        schema_version: SORA_SERVICE_MANIFEST_VERSION_V1,
        service_name: "web_portal".parse().expect("valid name"),
        service_version: "2026.02.0".to_string(),
        container: SoraContainerManifestRefV1 {
            manifest_hash: sample_hash(17),
            expected_schema_version: SORA_CONTAINER_MANIFEST_VERSION_V1,
        },
        replicas: NonZeroU16::new(3).expect("nonzero"),
        route: Some(SoraRouteTargetV1 {
            host: "portal.sora".to_string(),
            path_prefix: "/app".to_string(),
            service_port: NonZeroU16::new(8080).expect("nonzero"),
            visibility: SoraRouteVisibilityV1::Public,
            tls_mode: SoraTlsModeV1::Required,
        }),
        rollout: SoraRolloutPolicyV1 {
            canary_percent: 20,
            max_unavailable_replicas: 1,
            health_window_secs: NonZeroU32::new(45).expect("nonzero"),
            automatic_rollback_failures: NonZeroU32::new(3).expect("nonzero"),
        },
        state_bindings: vec![
            expected_state_binding(),
            SoraStateBindingV1 {
                schema_version: SORA_STATE_BINDING_VERSION_V1,
                binding_name: "patient_records".parse().expect("valid name"),
                scope: SoraStateScopeV1::ConfidentialState,
                mutability: SoraStateMutabilityV1::AppendOnly,
                encryption: SoraStateEncryptionV1::FheCiphertext,
                key_prefix: "/state/health".to_string(),
                max_item_bytes: NonZeroU64::new(16_384).expect("nonzero"),
                max_total_bytes: NonZeroU64::new(16_777_216).expect("nonzero"),
            },
        ],
        handlers: vec![
            SoraServiceHandlerV1 {
                handler_name: "assets".parse().expect("valid name"),
                class: SoraServiceHandlerClassV1::Asset,
                entrypoint: "serve_assets".to_string(),
                route_path: Some("/assets".to_string()),
                certified_response: SoraCertifiedResponsePolicyV1::StateCommitment,
                mailbox: None,
            },
            SoraServiceHandlerV1 {
                handler_name: "query".parse().expect("valid name"),
                class: SoraServiceHandlerClassV1::Query,
                entrypoint: "serve_query".to_string(),
                route_path: Some("/query".to_string()),
                certified_response: SoraCertifiedResponsePolicyV1::AuditReceipt,
                mailbox: None,
            },
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
            },
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
                    retention_blocks: NonZeroU32::new(2_880).expect("nonzero"),
                }),
            },
        ],
        artifacts: vec![
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
        ],
    }
}

fn expected_deployment_bundle() -> SoraDeploymentBundleV1 {
    let container = expected_container_manifest();
    let mut service = expected_service_manifest();
    service.container.manifest_hash = Hash::new(Encode::encode(&container));
    SoraDeploymentBundleV1 {
        schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
        container,
        service,
    }
}

fn expected_agent_apartment_manifest() -> AgentApartmentManifestV1 {
    AgentApartmentManifestV1 {
        schema_version: AGENT_APARTMENT_MANIFEST_VERSION_V1,
        apartment_name: "ops_agent".parse().expect("valid name"),
        container: SoraContainerManifestRefV1 {
            manifest_hash: sample_hash(33),
            expected_schema_version: SORA_CONTAINER_MANIFEST_VERSION_V1,
        },
        tool_capabilities: vec![
            AgentToolCapabilityV1 {
                tool: "soracloud.deploy".to_string(),
                max_invocations_per_epoch: NonZeroU32::new(120).expect("nonzero"),
                allow_network: true,
                allow_filesystem_write: false,
            },
            AgentToolCapabilityV1 {
                tool: "wallet.transfer".to_string(),
                max_invocations_per_epoch: NonZeroU32::new(24).expect("nonzero"),
                allow_network: false,
                allow_filesystem_write: false,
            },
        ],
        policy_capabilities: vec![
            "wallet.sign".parse().expect("valid name"),
            "governance.audit".parse().expect("valid name"),
        ],
        spend_limits: vec![
            AgentSpendLimitV1 {
                asset_definition: "61CtjvNd9T3THAR65GsMVHr82Bjc".to_string(),
                max_per_tx_nanos: NonZeroU64::new(5_000_000).expect("nonzero"),
                max_per_day_nanos: NonZeroU64::new(20_000_000).expect("nonzero"),
            },
            AgentSpendLimitV1 {
                asset_definition: "7t5kWEj537rDAL7AQNp9cZPUGPr5".to_string(),
                max_per_tx_nanos: NonZeroU64::new(2_000_000).expect("nonzero"),
                max_per_day_nanos: NonZeroU64::new(10_000_000).expect("nonzero"),
            },
        ],
        state_quota_bytes: NonZeroU64::new(134_217_728).expect("nonzero"),
        network_egress: SoraNetworkPolicyV1::Allowlist(vec![
            "rpc.sora.internal".to_string(),
            "torii.sora.internal".to_string(),
        ]),
        upgrade_policy: AgentUpgradePolicyV1::Governed,
    }
}

fn expected_fhe_param_set() -> FheParamSetV1 {
    FheParamSetV1 {
        schema_version: FHE_PARAM_SET_VERSION_V1,
        param_set: "fhe_bfv_med".parse().expect("valid name"),
        version: NonZeroU32::new(2).expect("nonzero"),
        backend: "fhe/bfv-rns/v2".to_string(),
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
    }
}

fn expected_fhe_execution_policy() -> FheExecutionPolicyV1 {
    FheExecutionPolicyV1 {
        schema_version: FHE_EXECUTION_POLICY_VERSION_V1,
        policy_name: "fhe_policy_med".parse().expect("valid name"),
        param_set: "fhe_bfv_med".parse().expect("valid name"),
        param_set_version: NonZeroU32::new(2).expect("nonzero"),
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

fn expected_fhe_governance_bundle() -> FheGovernanceBundleV1 {
    FheGovernanceBundleV1 {
        schema_version: FHE_GOVERNANCE_BUNDLE_VERSION_V1,
        param_set: expected_fhe_param_set(),
        execution_policy: expected_fhe_execution_policy(),
    }
}

fn expected_fhe_job_spec() -> FheJobSpecV1 {
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
        rotation_count: 0,
        bootstrap_count: 0,
    }
}

fn expected_decryption_authority_policy() -> DecryptionAuthorityPolicyV1 {
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

fn expected_decryption_request() -> DecryptionRequestV1 {
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

fn expected_ciphertext_query_spec() -> CiphertextQuerySpecV1 {
    CiphertextQuerySpecV1 {
        schema_version: CIPHERTEXT_QUERY_SPEC_VERSION_V1,
        service_name: "web_portal".parse().expect("valid name"),
        binding_name: "patient_records".parse().expect("valid name"),
        state_key_prefix: "/state/health".to_string(),
        max_results: NonZeroU16::new(32).expect("nonzero"),
        metadata_level: CiphertextQueryMetadataLevelV1::Minimal,
        include_proof: true,
    }
}

fn expected_ciphertext_query_response() -> CiphertextQueryResponseV1 {
    CiphertextQueryResponseV1 {
        schema_version: CIPHERTEXT_QUERY_RESPONSE_VERSION_V1,
        query_hash: sample_hash(147),
        service_name: "web_portal".parse().expect("valid name"),
        binding_name: "patient_records".parse().expect("valid name"),
        metadata_level: CiphertextQueryMetadataLevelV1::Minimal,
        served_sequence: 19,
        result_count: 1,
        truncated: false,
        results: vec![CiphertextQueryResultItemV1 {
            binding_name: "patient_records".parse().expect("valid name"),
            state_key: None,
            state_key_digest: sample_hash(148),
            payload_bytes: NonZeroU64::new(2_112).expect("nonzero"),
            ciphertext_commitment: sample_hash(149),
            encryption: SoraStateEncryptionV1::FheCiphertext,
            last_update_sequence: 17,
            governance_tx_hash: sample_hash(150),
            proof: Some(CiphertextInclusionProofV1 {
                schema_version: CIPHERTEXT_QUERY_PROOF_VERSION_V1,
                proof_scheme: "soracloud.audit_anchor.v1".to_string(),
                leaf_hash: sample_hash(151),
                anchor_hash: sample_hash(152),
                anchor_sequence: 19,
                event_sequence: 17,
            }),
        }],
    }
}

fn expected_secret_envelope() -> SecretEnvelopeV1 {
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

fn expected_ciphertext_state_record() -> CiphertextStateRecordV1 {
    let secret = expected_secret_envelope();
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

fn assert_norito_roundtrip<T>(value: &T)
where
    T: Encode + Decode + PartialEq + Debug,
{
    let encoded = Encode::encode(value);
    let mut cursor = encoded.as_slice();
    let decoded = <T as Decode>::decode(&mut cursor).expect("decode succeeds");
    assert!(cursor.is_empty(), "decode must consume all bytes");
    assert_eq!(decoded, *value, "roundtrip must preserve payload");
}

#[cfg(feature = "json")]
fn assert_fixture_eq<T>(path: &str, fixture: &str, expected: &T)
where
    T: Clone + PartialEq + Debug + FastJsonWrite + JsonDeserialize + JsonSerialize,
{
    let parsed: T = json::from_str(fixture).expect("fixture must decode");
    assert_eq!(
        parsed, *expected,
        "fixture `{path}` content does not match expected data model"
    );
    let canonical = json::to_json_pretty(expected).expect("serialize fixture");
    assert_eq!(
        fixture.trim(),
        canonical.trim(),
        "fixture `{path}` is not canonical JSON for the current schema"
    );
}

#[cfg(feature = "json")]
fn write_fixture<T: JsonSerialize>(path: &Path, value: &T) {
    let json = json::to_json_pretty(value).expect("serialize fixture");
    fs::write(path, json).unwrap_or_else(|error| {
        panic!("failed writing {}: {error}", path.display());
    });
}

#[cfg(feature = "json")]
#[test]
fn container_manifest_fixture_is_canonical() {
    let manifest = expected_container_manifest();
    assert_fixture_eq(
        "sora_container_manifest_v1.json",
        CONTAINER_FIXTURE,
        &manifest,
    );
    assert_norito_roundtrip(&manifest);
    manifest.validate().expect("fixture should validate");
}

#[cfg(feature = "json")]
#[test]
fn state_binding_fixture_is_canonical() {
    let binding = expected_state_binding();
    assert_fixture_eq(
        "sora_state_binding_v1.json",
        STATE_BINDING_FIXTURE,
        &binding,
    );
    assert_norito_roundtrip(&binding);
    binding.validate().expect("fixture should validate");
}

#[cfg(feature = "json")]
#[test]
fn service_manifest_fixture_is_canonical() {
    let manifest = expected_service_manifest();
    assert_fixture_eq("sora_service_manifest_v1.json", SERVICE_FIXTURE, &manifest);
    assert_norito_roundtrip(&manifest);
    manifest.validate().expect("fixture should validate");
}

#[cfg(feature = "json")]
#[test]
fn deployment_bundle_fixture_is_canonical() {
    let bundle = expected_deployment_bundle();
    assert_fixture_eq(
        "sora_deployment_bundle_v1.json",
        DEPLOYMENT_BUNDLE_FIXTURE,
        &bundle,
    );
    assert_norito_roundtrip(&bundle);
    bundle
        .validate_for_admission()
        .expect("deployment bundle fixture should validate");
}

#[cfg(feature = "json")]
#[test]
fn agent_apartment_manifest_fixture_is_canonical() {
    let manifest = expected_agent_apartment_manifest();
    assert_fixture_eq(
        "agent_apartment_manifest_v1.json",
        AGENT_APARTMENT_FIXTURE,
        &manifest,
    );
    assert_norito_roundtrip(&manifest);
    manifest.validate().expect("fixture should validate");
}

#[cfg(feature = "json")]
#[test]
fn fhe_param_set_fixture_is_canonical() {
    let param_set = expected_fhe_param_set();
    assert_fixture_eq("fhe_param_set_v1.json", FHE_PARAM_SET_FIXTURE, &param_set);
    assert_norito_roundtrip(&param_set);
    param_set.validate().expect("fixture should validate");
}

#[cfg(feature = "json")]
#[test]
fn fhe_execution_policy_fixture_is_canonical() {
    let policy = expected_fhe_execution_policy();
    assert_fixture_eq(
        "fhe_execution_policy_v1.json",
        FHE_EXECUTION_POLICY_FIXTURE,
        &policy,
    );
    assert_norito_roundtrip(&policy);
    policy.validate().expect("fixture should validate");
    policy
        .validate_for_param_set(&expected_fhe_param_set())
        .expect("fixture should match expected parameter set");
}

#[cfg(feature = "json")]
#[test]
fn fhe_governance_bundle_fixture_is_canonical() {
    let bundle = expected_fhe_governance_bundle();
    assert_fixture_eq(
        "fhe_governance_bundle_v1.json",
        FHE_GOVERNANCE_BUNDLE_FIXTURE,
        &bundle,
    );
    assert_norito_roundtrip(&bundle);
    bundle
        .validate_for_admission()
        .expect("fixture should validate");
}

#[cfg(feature = "json")]
#[test]
fn secret_envelope_fixture_is_canonical() {
    let envelope = expected_secret_envelope();
    assert_fixture_eq(
        "secret_envelope_v1.json",
        SECRET_ENVELOPE_FIXTURE,
        &envelope,
    );
    assert_norito_roundtrip(&envelope);
    envelope.validate().expect("fixture should validate");
}

#[cfg(feature = "json")]
#[test]
fn ciphertext_state_record_fixture_is_canonical() {
    let record = expected_ciphertext_state_record();
    assert_fixture_eq(
        "ciphertext_state_record_v1.json",
        CIPHERTEXT_STATE_RECORD_FIXTURE,
        &record,
    );
    assert_norito_roundtrip(&record);
    record.validate().expect("fixture should validate");
}

#[cfg(feature = "json")]
#[test]
fn fhe_job_spec_fixture_is_canonical() {
    let job = expected_fhe_job_spec();
    assert_fixture_eq("fhe_job_spec_v1.json", FHE_JOB_SPEC_FIXTURE, &job);
    assert_norito_roundtrip(&job);
    job.validate().expect("fixture should validate");
    job.validate_for_execution(&expected_fhe_execution_policy(), &expected_fhe_param_set())
        .expect("fixture should pass execution admission checks");
}

#[cfg(feature = "json")]
#[test]
fn decryption_authority_policy_fixture_is_canonical() {
    let policy = expected_decryption_authority_policy();
    assert_fixture_eq(
        "decryption_authority_policy_v1.json",
        DECRYPTION_AUTHORITY_POLICY_FIXTURE,
        &policy,
    );
    assert_norito_roundtrip(&policy);
    policy.validate().expect("fixture should validate");
}

#[cfg(feature = "json")]
#[test]
fn decryption_request_fixture_is_canonical() {
    let request = expected_decryption_request();
    assert_fixture_eq(
        "decryption_request_v1.json",
        DECRYPTION_REQUEST_FIXTURE,
        &request,
    );
    assert_norito_roundtrip(&request);
    request
        .validate_for_policy(&expected_decryption_authority_policy())
        .expect("fixture should pass policy-linked validation");
}

#[cfg(feature = "json")]
#[test]
fn ciphertext_query_spec_fixture_is_canonical() {
    let spec = expected_ciphertext_query_spec();
    assert_fixture_eq(
        "ciphertext_query_spec_v1.json",
        CIPHERTEXT_QUERY_SPEC_FIXTURE,
        &spec,
    );
    assert_norito_roundtrip(&spec);
    spec.validate().expect("fixture should validate");
}

#[cfg(feature = "json")]
#[test]
fn ciphertext_query_response_fixture_is_canonical() {
    let response = expected_ciphertext_query_response();
    assert_fixture_eq(
        "ciphertext_query_response_v1.json",
        CIPHERTEXT_QUERY_RESPONSE_FIXTURE,
        &response,
    );
    assert_norito_roundtrip(&response);
    response.validate().expect("fixture should validate");
}

#[cfg(feature = "json")]
#[test]
#[ignore = "regenerates Soracloud fixture files"]
fn regenerate_soracloud_fixtures() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("soracloud");
    write_fixture(
        &base.join("sora_container_manifest_v1.json"),
        &expected_container_manifest(),
    );
    write_fixture(
        &base.join("sora_service_manifest_v1.json"),
        &expected_service_manifest(),
    );
    write_fixture(
        &base.join("sora_state_binding_v1.json"),
        &expected_state_binding(),
    );
    write_fixture(
        &base.join("sora_deployment_bundle_v1.json"),
        &expected_deployment_bundle(),
    );
    write_fixture(
        &base.join("agent_apartment_manifest_v1.json"),
        &expected_agent_apartment_manifest(),
    );
    write_fixture(
        &base.join("fhe_param_set_v1.json"),
        &expected_fhe_param_set(),
    );
    write_fixture(
        &base.join("fhe_execution_policy_v1.json"),
        &expected_fhe_execution_policy(),
    );
    write_fixture(
        &base.join("fhe_governance_bundle_v1.json"),
        &expected_fhe_governance_bundle(),
    );
    write_fixture(&base.join("fhe_job_spec_v1.json"), &expected_fhe_job_spec());
    write_fixture(
        &base.join("decryption_authority_policy_v1.json"),
        &expected_decryption_authority_policy(),
    );
    write_fixture(
        &base.join("decryption_request_v1.json"),
        &expected_decryption_request(),
    );
    write_fixture(
        &base.join("ciphertext_query_spec_v1.json"),
        &expected_ciphertext_query_spec(),
    );
    write_fixture(
        &base.join("ciphertext_query_response_v1.json"),
        &expected_ciphertext_query_response(),
    );
    write_fixture(
        &base.join("secret_envelope_v1.json"),
        &expected_secret_envelope(),
    );
    write_fixture(
        &base.join("ciphertext_state_record_v1.json"),
        &expected_ciphertext_state_record(),
    );
}
