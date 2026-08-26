use super::*;
use iroha_crypto::{
    Algorithm, KeyPair,
    fhe_bfv::{
        BfvFullBootstrapAccumulatorV1, BfvFullBootstrapCircuitArtifactRoleV1,
        BfvFullBootstrapLinearTransformDiagonalV1, BfvFullBootstrapLinearTransformV1,
        BfvFullBootstrapSampleExtractionV1,
        bfv_full_bootstrap_arithmetic_air_constraint_system_material_v1,
        bfv_full_bootstrap_blind_rotation_key_for_packed_left_rotation_v1,
        bfv_full_bootstrap_evaluator_artifact_set_digest_v1,
        bfv_full_bootstrap_proof_key_material_commitment_from_artifact_v1,
        bfv_full_bootstrap_proof_key_pair_commitment_from_artifacts_v1,
        bfv_full_bootstrap_proof_key_pair_from_key_material_v1,
        bfv_full_bootstrap_proof_public_input_schema_v1,
        bfv_full_bootstrap_sample_extraction_switch_key_from_seed_v1,
        encode_bfv_full_bootstrap_accumulator_artifact_v1,
        encode_bfv_full_bootstrap_arithmetic_air_constraint_system_artifact_v1,
        encode_bfv_full_bootstrap_blind_rotation_artifact_v1,
        encode_bfv_full_bootstrap_linear_transform_artifact_v1,
        encode_bfv_full_bootstrap_native_stark_fri_prover_key_material_v1,
        encode_bfv_full_bootstrap_native_stark_fri_verifier_key_material_v1,
        encode_bfv_full_bootstrap_proof_key_artifact_v1,
        encode_bfv_full_bootstrap_proof_public_input_schema_artifact_v1,
        encode_bfv_full_bootstrap_sample_extraction_switch_key_artifact_v1,
        encode_packed_plaintext_slots, keygen_from_seed, ram_lfe_bfv_parameters_v1,
    },
};
use norito::codec::DecodeAll as _;
use std::collections::{BTreeMap, BTreeSet};
fn sample_hash(seed: u8) -> Hash {
    let mut bytes = [0u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = seed.wrapping_add(u8::try_from(index).expect("index fits in u8"));
    }
    Hash::prehashed(bytes)
}
fn sample_name(name: &str) -> Name {
    name.parse().expect("valid name")
}
fn sample_app_infra_service(name: &str) -> SoraAppInfraServiceRefV1 {
    SoraAppInfraServiceRefV1 {
        schema_version: SORA_APP_INFRA_SERVICE_REF_VERSION_V1,
        service_name: sample_name(name),
        service_version: "1.0.0".to_string(),
        service_manifest_hash: sample_hash(10),
        container_manifest_hash: sample_hash(11),
        execution_plane: SoraServiceExecutionPlaneV1::HttpService,
        runtime: SoraContainerRuntimeV1::Inrou,
        routes: vec![SoraAppRouteProjectionV1 {
            schema_version: SORA_APP_ROUTE_PROJECTION_VERSION_V1,
            public_host: Some("app.example.test".to_string()),
            path_prefix: "/api".to_string(),
            internal_url: Some("soracloud://app_api:8080/api".to_string()),
        }],
        lease_volumes: vec![sample_name("app_data")],
        shard: Some("SORACLOUD_SHARD_ID=0;SORACLOUD_SHARD_COUNT=1".to_string()),
    }
}
fn sample_app_infra_manifest() -> SoraAppInfraManifestV1 {
    SoraAppInfraManifestV1 {
        schema_version: SORA_APP_INFRA_MANIFEST_VERSION_V1,
        app_name: sample_name("sample_app"),
        app_version: "1.0.0".to_string(),
        public_url: "https://app.example.test".to_string(),
        static_site: Some(SoraAppStaticSiteBindingV1 {
            schema_version: SORA_APP_STATIC_SITE_BINDING_VERSION_V1,
            public_url: "https://app.example.test".to_string(),
            content_cid: Some(encode_lowercase_multibase_base32(
                &sorafs_manifest::canonical_manifest_root_cid([0xA4; 32]),
            )),
            manifest_digest_hex: Some("a".repeat(64)),
            mount_path: "/".to_string(),
            api_base_path: Some("/api".to_string()),
        }),
        services: vec![sample_app_infra_service("app_api")],
    }
}
fn sample_signer() -> iroha_crypto::PublicKey {
    KeyPair::try_random()
        .expect("SoraCloud fixture signer key generation should succeed")
        .public_key()
        .clone()
}
fn sample_account_id(seed: u8) -> AccountId {
    let keypair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
        .expect("fixture seed derives Ed25519 keypair");
    AccountId::new(keypair.public_key().clone())
}
fn sample_peer_id(seed: u8) -> String {
    let keypair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
        .expect("fixture seed derives Ed25519 peer keypair");
    PeerId::from(keypair.public_key().clone()).to_string()
}
fn sample_validator_execution_host(seed: u8) -> SoraRuntimeDeterministicValidatorHostV1 {
    let validator_account_id = sample_account_id(seed);
    SoraRuntimeDeterministicValidatorHostV1 {
        lane_id: LaneId::SINGLE,
        peer_id: PeerId::from(validator_account_id.expect_single_signatory().clone()).to_string(),
        validator_account_id,
    }
}
fn sample_ed25519_keypair(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
        .expect("fixture seed derives Ed25519 keypair")
}
fn sample_bls_keypair(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
        .expect("fixture seed derives BLS keypair")
}
fn sample_asset_definition_id(asset_definition_id: &str) -> AssetDefinitionId {
    AssetDefinitionId::parse_address_literal(asset_definition_id)
        .expect("sample asset definition id literal should parse")
}
#[test]
fn app_infra_manifest_validation_rejects_duplicate_services() {
    let mut manifest = sample_app_infra_manifest();
    manifest.services.push(sample_app_infra_service("app_api"));
    let error = manifest
        .validate()
        .expect_err("duplicate app service must be rejected");
    assert!(matches!(
        error,
        SoracloudManifestError::DuplicateAppService { service }
            if service == sample_name("app_api")
    ));
}
#[test]
fn app_infra_manifest_hash_and_provenance_are_canonical() {
    let manifest = sample_app_infra_manifest();
    let precondition = SoraAppInfraMutationPreconditionV1::AppAbsent;
    manifest
        .validate()
        .expect("sample app infra manifest must validate");
    assert_eq!(manifest.services[0].routes[0].path_prefix, "/api");
    assert_eq!(
        manifest.services[0].lease_volumes[0],
        sample_name("app_data")
    );
    assert_eq!(
        manifest.manifest_hash(),
        Hash::new(Encode::encode(&manifest))
    );
    assert_eq!(
        encode_app_infra_provenance_payload(&manifest, &precondition)
            .expect("encode app infra provenance"),
        norito::to_bytes(&(manifest, precondition)).expect("encode canonical app mutation")
    );
}
#[test]
fn app_infra_manifest_rejects_noncanonical_url_and_path_text() {
    let mut padded_url = sample_app_infra_manifest();
    padded_url.public_url.push(' ');
    padded_url
        .validate()
        .expect_err("public URL whitespace must not be normalized");

    let mut padded_path = sample_app_infra_manifest();
    padded_path
        .static_site
        .as_mut()
        .expect("static-site fixture")
        .mount_path
        .push(' ');
    padded_path
        .validate()
        .expect_err("absolute path whitespace must not be normalized");

    let mut traversal_path = sample_app_infra_manifest();
    traversal_path
        .static_site
        .as_mut()
        .expect("static-site fixture")
        .api_base_path = Some("/api/../private".to_owned());
    traversal_path
        .validate()
        .expect_err("noncanonical absolute path components must fail closed");

    for public_url in [
        "https://App.example.test",
        "https://app.example.test:0443/api",
        "https://app.example.test:443/api",
        "https://app.example.test/api/../private",
        "https://127.1/api",
        "https://app.example.test/",
    ] {
        let mut noncanonical_url = sample_app_infra_manifest();
        noncanonical_url.public_url = public_url.to_owned();
        noncanonical_url
            .validate()
            .expect_err("public URL aliases must fail closed");
    }
}
#[test]
fn app_static_site_requires_exact_paired_sorafs_references() {
    let mut manifest = sample_app_infra_manifest();
    manifest
        .validate()
        .expect("canonical static-site references");

    let static_site = manifest.static_site.as_mut().expect("static-site fixture");
    static_site.content_cid = static_site
        .content_cid
        .take()
        .map(|content_cid| content_cid.replacen('b', "B", 1));
    manifest
        .validate()
        .expect_err("noncanonical content CID spelling must fail closed");

    let mut uppercase_digest = sample_app_infra_manifest();
    uppercase_digest
        .static_site
        .as_mut()
        .expect("static-site fixture")
        .manifest_digest_hex = Some("A".repeat(64));
    uppercase_digest
        .validate()
        .expect_err("noncanonical manifest digest spelling must fail closed");

    let mut unpaired = sample_app_infra_manifest();
    unpaired
        .static_site
        .as_mut()
        .expect("static-site fixture")
        .manifest_digest_hex = None;
    unpaired
        .validate()
        .expect_err("partial static content identity must fail closed");
}
#[test]
fn app_route_and_service_references_require_exact_v1_tokens() {
    let canonical = sample_app_infra_service("app_api");
    canonical
        .validate()
        .expect("canonical app service reference");

    let mut uppercase_host = canonical.clone();
    uppercase_host.routes[0].public_host = Some("App.Example.Test".to_owned());
    uppercase_host
        .validate()
        .expect_err("public host case aliases must fail closed");

    let mut legacy_internal_scheme = canonical.clone();
    legacy_internal_scheme.routes[0].internal_url = Some("http://app_api:8080/api".to_owned());
    legacy_internal_scheme
        .validate()
        .expect_err("internal routes must use the exact Soracloud V1 scheme");

    let mut noncanonical_internal_port = canonical.clone();
    noncanonical_internal_port.routes[0].internal_url =
        Some("soracloud://app_api:08080/api".to_owned());
    noncanonical_internal_port
        .validate()
        .expect_err("internal route ports must use canonical decimal spelling");

    let mut padded_version = canonical.clone();
    padded_version.service_version.push(' ');
    padded_version
        .validate()
        .expect_err("service version aliases must fail closed");

    let mut padded_shard = canonical;
    padded_shard.shard = Some(" SORACLOUD_SHARD_ID=0;SORACLOUD_SHARD_COUNT=1".to_owned());
    padded_shard
        .validate()
        .expect_err("shard aliases must fail closed");
}
#[cfg(feature = "json")]
#[test]
fn app_infra_v1_json_graph_is_closed_and_requires_explicit_nullable_and_vector_keys() {
    macro_rules! assert_closed {
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
                    norito::json::Error::UnknownField { ref field } if field == "retired_v0"
                ),
                "{} reported the wrong unknown-field error: {error}",
                $label
            );
        }};
    }
    macro_rules! assert_required_nullable {
        ($value:expr, $field:expr, $ty:ty, $label:literal) => {{
            let canonical =
                norito::json::to_value(&$value).expect(concat!("serialize canonical ", $label));
            let mut missing = canonical.clone();
            assert!(
                missing
                    .as_object_mut()
                    .expect(concat!($label, " JSON object"))
                    .remove($field)
                    .is_some()
            );
            norito::json::from_value::<$ty>(missing)
                .expect_err(concat!($label, " must reject an omitted nullable key"));

            let mut explicit_null = canonical;
            explicit_null
                .as_object_mut()
                .expect(concat!($label, " JSON object"))
                .insert($field.to_owned(), norito::json::Value::Null);
            norito::json::from_value::<$ty>(explicit_null)
                .expect(concat!($label, " must accept an explicit null key"));
        }};
    }
    macro_rules! assert_required_vector {
        ($value:expr, $field:expr, $ty:ty, $label:literal) => {{
            let canonical =
                norito::json::to_value(&$value).expect(concat!("serialize canonical ", $label));
            let mut missing = canonical.clone();
            assert!(
                missing
                    .as_object_mut()
                    .expect(concat!($label, " JSON object"))
                    .remove($field)
                    .is_some()
            );
            norito::json::from_value::<$ty>(missing)
                .expect_err(concat!($label, " must reject an omitted vector key"));

            let mut null = canonical;
            null.as_object_mut()
                .expect(concat!($label, " JSON object"))
                .insert($field.to_owned(), norito::json::Value::Null);
            norito::json::from_value::<$ty>(null)
                .expect_err(concat!($label, " must reject a null vector key"));
        }};
    }

    let manifest = sample_app_infra_manifest();
    let static_site = manifest.static_site.clone().expect("sample static site");
    let service = manifest.services[0].clone();
    let route = service.routes[0].clone();
    let state = SoraAppInfraStateV1 {
        schema_version: SORA_APP_INFRA_STATE_VERSION_V1,
        app_name: manifest.app_name.clone(),
        current_app_version: manifest.app_version.clone(),
        current_manifest_hash: manifest.manifest_hash(),
        revision_count: 1,
        deployed_sequence: 1,
        updated_sequence: 1,
        manifest: manifest.clone(),
    };
    let audit = SoraAppInfraAuditEventV1 {
        schema_version: SORA_APP_INFRA_AUDIT_EVENT_VERSION_V1,
        sequence: 1,
        action: SoraAppInfraActionV1::Deploy,
        app_name: manifest.app_name.clone(),
        from_version: None,
        to_version: manifest.app_version.clone(),
        app_manifest_hash: manifest.manifest_hash(),
        service_count: 1,
        signer: sample_signer(),
    };

    assert_closed!(
        SoraAppInfraActionV1::Deploy,
        SoraAppInfraActionV1,
        "app infra action"
    );
    assert_closed!(
        static_site.clone(),
        SoraAppStaticSiteBindingV1,
        "app static-site binding"
    );
    assert_closed!(
        route.clone(),
        SoraAppRouteProjectionV1,
        "app route projection"
    );
    assert_closed!(
        service.clone(),
        SoraAppInfraServiceRefV1,
        "app service reference"
    );
    assert_closed!(
        manifest.clone(),
        SoraAppInfraManifestV1,
        "app infra manifest"
    );
    assert_closed!(state, SoraAppInfraStateV1, "app infra state");
    assert_closed!(
        audit.clone(),
        SoraAppInfraAuditEventV1,
        "app infra audit event"
    );

    for field in ["content_cid", "manifest_digest_hex", "api_base_path"] {
        assert_required_nullable!(
            static_site.clone(),
            field,
            SoraAppStaticSiteBindingV1,
            "app static-site binding"
        );
    }
    for field in ["public_host", "internal_url"] {
        assert_required_nullable!(
            route.clone(),
            field,
            SoraAppRouteProjectionV1,
            "app route projection"
        );
    }
    assert_required_nullable!(
        service.clone(),
        "shard",
        SoraAppInfraServiceRefV1,
        "app service reference"
    );
    for field in ["routes", "lease_volumes"] {
        assert_required_vector!(
            service.clone(),
            field,
            SoraAppInfraServiceRefV1,
            "app service reference"
        );
    }
    assert_required_nullable!(
        manifest,
        "static_site",
        SoraAppInfraManifestV1,
        "app infra manifest"
    );
    assert_required_nullable!(
        audit,
        "from_version",
        SoraAppInfraAuditEventV1,
        "app infra audit event"
    );
}
#[test]
fn app_infra_service_ref_validate_rejects_zero_prehash_digest_sentinels() {
    let zero_digest = zero_prehash_statement_hash();
    let mut service = sample_app_infra_service("app_api");
    service.service_manifest_hash = zero_digest;
    let error = service
        .validate()
        .expect_err("service manifest placeholder hash must fail admission");
    assert_zero_prehash_digest_error(&error, "service_manifest_hash");
    let mut service = sample_app_infra_service("app_api");
    service.container_manifest_hash = zero_digest;
    let error = service
        .validate()
        .expect_err("container manifest placeholder hash must fail admission");
    assert_zero_prehash_digest_error(&error, "container_manifest_hash");
}
#[test]
fn app_infra_audit_event_validate_rejects_zero_prehash_manifest_hash_sentinel() {
    let event = SoraAppInfraAuditEventV1 {
        schema_version: SORA_APP_INFRA_AUDIT_EVENT_VERSION_V1,
        sequence: 1,
        action: SoraAppInfraActionV1::Deploy,
        app_name: sample_name("sample_app"),
        from_version: None,
        to_version: "1.0.0".to_string(),
        app_manifest_hash: zero_prehash_statement_hash(),
        service_count: 1,
        signer: sample_signer(),
    };
    let error = event
        .validate()
        .expect_err("app manifest placeholder hash must fail admission");
    assert_zero_prehash_digest_error(&error, "app_manifest_hash");
}
#[test]
fn app_infra_audit_versions_are_exact_tokens() {
    let canonical = SoraAppInfraAuditEventV1 {
        schema_version: SORA_APP_INFRA_AUDIT_EVENT_VERSION_V1,
        sequence: 2,
        action: SoraAppInfraActionV1::Upgrade,
        app_name: sample_name("sample_app"),
        from_version: Some("1.0.0".to_owned()),
        to_version: "1.1.0".to_owned(),
        app_manifest_hash: sample_hash(0xB0),
        service_count: 1,
        signer: sample_signer(),
    };
    canonical
        .validate()
        .expect("canonical app-infra audit event");

    let mut padded_from = canonical.clone();
    padded_from.from_version = Some(" 1.0.0".to_owned());
    padded_from
        .validate()
        .expect_err("from-version aliases must fail closed");

    let mut padded_to = canonical;
    padded_to.to_version = "1.1.0 ".to_owned();
    padded_to
        .validate()
        .expect_err("to-version aliases must fail closed");
}
fn sample_model_provenance_ref() -> SoraModelProvenanceRefV1 {
    SoraModelProvenanceRefV1 {
        kind: SoraModelProvenanceKindV1::TrainingJob,
        id: "job-1".to_string(),
    }
}

fn sample_uploaded_model_bundle() -> SoraUploadedModelBundleV1 {
    SoraUploadedModelBundleV1 {
        schema_version: SORA_UPLOADED_MODEL_BUNDLE_VERSION_V1,
        service_name: sample_name("uploaded_model_registry"),
        model_id: "upload-1".to_string(),
        weight_version: "v1".to_string(),
        family: "demo-family".to_string(),
        modalities: vec!["text".to_string()],
        plaintext_root: sample_hash(30),
        package_format: SoraUploadedModelPackageFormatV1::NormalizedHuggingFaceSafetensorsV1,
        bundle_root: sample_hash(31),
        sorafs_manifest_digest: ManifestDigest::new([0xA5; 32]),
        chunk_count: 2,
        plaintext_bytes: 1_024,
        ciphertext_bytes: 2_048,
        chunk_manifest_root: sample_hash(33),
        pricing_policy: SoraUploadedModelPricingPolicyV1 {
            storage_price: xor_quantity_from_nanos(10),
        },
    }
}
#[test]
fn canonical_hf_repo_ids_require_exact_qualified_provider_spelling() {
    for repo_id in [
        "OpenAI/GPT-OSS",
        "openai-community/gpt2",
        "owner_1/model.v1",
    ] {
        assert!(
            is_canonical_hf_repo_id_v1(repo_id),
            "canonical repository ID `{repo_id}` must be admitted"
        );
    }
    let oversized = format!("owner/{}", "a".repeat(SORA_HF_REPO_ID_MAX_BYTES_V1));
    for repo_id in [
        "",
        "model",
        "/model",
        "owner/",
        "owner//model",
        "owner/./model",
        "./model",
        "../model",
        "owner/..",
        "owner/.model",
        "owner/model-",
        "owner/model--alias",
        "owner/model..alias",
        "owner/model.git",
        "owner%2falias/model",
        "owner\\alias/model",
        " owner/model",
        oversized.as_str(),
    ] {
        assert!(
            !is_canonical_hf_repo_id_v1(repo_id),
            "noncanonical repository ID `{repo_id}` must be rejected"
        );
    }
}
#[test]
fn canonical_hf_source_id_binds_repo_spelling_and_immutable_commit() {
    const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
    let upper = derive_hf_source_id_v1("OpenAI/GPT-OSS", COMMIT)
        .expect("uppercase canonical provider spelling is valid");
    let lower = derive_hf_source_id_v1("openai/gpt-oss", COMMIT)
        .expect("lowercase canonical provider spelling is valid");
    let expected = Hash::new(
        norito::to_bytes(&("soracloud:hf-source-id:v1", "OpenAI/GPT-OSS", COMMIT))
            .expect("canonical domain-separated preimage"),
    );
    let retired_undomained = Hash::new(
        norito::to_bytes(&("OpenAI/GPT-OSS", COMMIT)).expect("retired undomained preimage"),
    );
    assert_eq!(upper, expected);
    assert_ne!(upper, retired_undomained);
    assert_ne!(upper, lower, "case-sensitive identities must not alias");
    assert!(derive_hf_source_id_v1("gpt-oss", COMMIT).is_err());
    assert!(derive_hf_source_id_v1("openai/gpt-oss", "main").is_err());
}
#[test]
fn canonical_hf_shared_lease_pool_id_is_domain_separated() {
    let source_id = Hash::new(b"canonical-hf-source");
    let storage_class = StorageClass::Warm;
    let lease_term_ms = 604_800_000_u64;
    let expected = Hash::new(
        norito::to_bytes(&(
            "soracloud:hf-shared-lease-pool-id:v1",
            source_id,
            storage_class,
            lease_term_ms,
        ))
        .expect("canonical domain-separated pool preimage"),
    );
    let retired_undomained = Hash::new(
        norito::to_bytes(&(source_id, storage_class, lease_term_ms))
            .expect("retired undomained pool preimage"),
    );
    let actual = derive_hf_shared_lease_pool_id_v1(source_id, storage_class, lease_term_ms)
        .expect("canonical shared-lease pool identity");
    assert_eq!(actual, expected);
    assert_ne!(actual, retired_undomained);
}
fn sample_hf_source_record() -> SoraHfSourceRecordV1 {
    let repo_id = "openai/demo-model";
    let resolved_revision = "4f9d72c4f9d72c4f9d72c4f9d72c4f9d72c4f9da";
    SoraHfSourceRecordV1 {
        schema_version: SORA_HF_SOURCE_RECORD_VERSION_V1,
        source_id: derive_hf_source_id_v1(repo_id, resolved_revision)
            .expect("sample HF source identity is canonical"),
        repo_id: repo_id.to_string(),
        resolved_revision: resolved_revision.to_string(),
        created_at_ms: 1_000,
        updated_at_ms: 1_500,
    }
}
#[test]
fn hf_source_record_rejects_repo_aliases_and_mismatched_source_ids() {
    let mut source = sample_hf_source_record();
    source.validate().expect("canonical sample source");
    source.repo_id = "OpenAI/demo-model".to_owned();
    assert!(
        source.validate().is_err(),
        "case drift must not retain the lowercase source identifier"
    );
    source.repo_id = "demo-model".to_owned();
    source.source_id = derive_hf_source_id_v1("openai/demo-model", &source.resolved_revision)
        .expect("canonical comparison identity");
    assert!(
        source.validate().is_err(),
        "unqualified provider aliases must fail"
    );
}
#[test]
fn hf_persisted_binding_tokens_are_exact() {
    let mut member = sample_hf_shared_lease_member();
    member.validate().expect("canonical HF shared-lease member");
    member.service_bindings = BTreeSet::from([" demo_service".to_owned()]);
    member
        .validate()
        .expect_err("service binding aliases must fail closed");

    let mut member = sample_hf_shared_lease_member();
    member.apartment_bindings = BTreeSet::from(["demo apartment".to_owned()]);
    member
        .validate()
        .expect_err("apartment bindings must use canonical Names");

    let mut event = sample_hf_shared_lease_audit_event();
    event
        .validate()
        .expect("canonical HF shared-lease audit event");
    event.service_name = Some("demo_service ".to_owned());
    event
        .validate()
        .expect_err("audit service binding aliases must fail closed");

    let mut event = sample_hf_shared_lease_audit_event();
    event.apartment_name = Some("demo apartment".to_owned());
    event
        .validate()
        .expect_err("audit apartment bindings must use canonical Names");
}

#[cfg(feature = "json")]
#[test]
fn hf_registry_json_rejects_retired_runtime_metadata() {
    for retired_field in [
        "model_name",
        "adapter_id",
        "normalized_runtime_hash",
        "resource_profile",
        "source_artifact_hash",
        "source_profile",
        "status",
        "last_error",
    ] {
        let mut value =
            norito::json::to_value(&sample_hf_source_record()).expect("serialize HF source");
        value
            .as_object_mut()
            .expect("HF source JSON object")
            .insert(
                retired_field.to_owned(),
                norito::json::Value::from("retired"),
            );
        let error = norito::json::from_value::<SoraHfSourceRecordV1>(value)
            .expect_err("retired HF source field must fail closed");
        assert!(
            matches!(
                error,
                norito::json::Error::UnknownField { ref field } if field == retired_field
            ),
            "retired HF source field `{retired_field}` reported the wrong error: {error}"
        );
    }
}

#[test]
fn hf_registry_norito_rejects_retired_runtime_metadata_layouts() {
    #[derive(Encode)]
    enum RetiredBackendFamilyV1 {
        Transformers,
    }
    #[derive(Encode)]
    struct RetiredHfResourceProfileV1 {
        required_model_bytes: u64,
        backend_family: RetiredBackendFamilyV1,
        model_format: RetiredHfModelFormatV1,
        selected_weight_file_count: u32,
        weight_selection_commitment: Hash,
        disk_cache_bytes_floor: u64,
        ram_bytes_floor: u64,
        vram_bytes_floor: u64,
    }
    #[derive(Encode)]
    enum RetiredHfModelFormatV1 {
        Safetensors,
    }
    #[derive(Encode)]
    enum RetiredHfSourceStatusV1 {
        Admitted,
    }

    #[derive(Encode)]
    struct RetiredHfSourceRecordV1 {
        schema_version: u16,
        source_id: Hash,
        repo_id: String,
        resolved_revision: String,
        model_name: String,
        adapter_id: String,
        normalized_runtime_hash: Hash,
        resource_profile: Option<RetiredHfResourceProfileV1>,
        status: RetiredHfSourceStatusV1,
        created_at_ms: u64,
        updated_at_ms: u64,
        last_error: Option<String>,
    }
    let canonical = sample_hf_source_record();
    let retired_source = RetiredHfSourceRecordV1 {
        schema_version: canonical.schema_version,
        source_id: canonical.source_id,
        repo_id: canonical.repo_id,
        resolved_revision: canonical.resolved_revision,
        model_name: "demo_model".to_owned(),
        adapter_id: "transformers".to_owned(),
        normalized_runtime_hash: sample_hash(22),
        resource_profile: Some(RetiredHfResourceProfileV1 {
            required_model_bytes: 1,
            backend_family: RetiredBackendFamilyV1::Transformers,
            model_format: RetiredHfModelFormatV1::Safetensors,
            selected_weight_file_count: 1,
            weight_selection_commitment: sample_hash(0x71),
            disk_cache_bytes_floor: 1,
            ram_bytes_floor: 1,
            vram_bytes_floor: 0,
        }),
        status: RetiredHfSourceStatusV1::Admitted,
        created_at_ms: canonical.created_at_ms,
        updated_at_ms: canonical.updated_at_ms,
        last_error: None,
    };
    let retired_source_bytes = retired_source.encode();
    assert!(
        SoraHfSourceRecordV1::decode_all(&mut retired_source_bytes.as_slice()).is_err(),
        "HF source record must reject the retired adapter/runtime-hash Norito layout"
    );
}
fn sample_hf_shared_lease_pool() -> SoraHfSharedLeasePoolV1 {
    SoraHfSharedLeasePoolV1 {
        schema_version: SORA_HF_SHARED_LEASE_POOL_VERSION_V1,
        pool_id: sample_hash(23),
        source_id: sample_hash(21),
        storage_class: StorageClass::Warm,
        lease_asset_definition_id: sample_asset_definition_id("4cuvDVPuLBKJyN6dPbRQhmLh68sU"),
        base_fee: xor_quantity_from_nanos(10_000),
        lease_term_ms: 604_800_000,
        window_started_at_ms: 10_000,
        window_expires_at_ms: 604_810_000,
        active_member_count: 2,
        status: SoraHfSharedLeaseStatusV1::Active,
        queued_next_window: None,
    }
}
fn sample_hf_shared_lease_member() -> SoraHfSharedLeaseMemberV1 {
    SoraHfSharedLeaseMemberV1 {
        schema_version: SORA_HF_SHARED_LEASE_MEMBER_VERSION_V1,
        pool_id: sample_hash(23),
        source_id: sample_hash(21),
        account_id: sample_account_id(0xA1),
        status: SoraHfSharedLeaseMemberStatusV1::Active,
        joined_at_ms: 10_000,
        updated_at_ms: 20_000,
        total_paid: xor_quantity_from_nanos(10_000),
        total_refunded: xor_quantity_from_nanos(5_000),
        last_charge: xor_quantity_from_nanos(10_000),
        service_bindings: BTreeSet::from(["demo_service".to_string()]),
        apartment_bindings: BTreeSet::from(["demo_apartment".to_string()]),
    }
}
fn sample_inrou_host_capability_record() -> SoraInrouHostCapabilityRecordV1 {
    SoraInrouHostCapabilityRecordV1 {
        schema_version: SORA_INROU_HOST_CAPABILITY_RECORD_VERSION_V1,
        validator_account_id: sample_account_id(0xD1),
        peer_id: sample_peer_id(0xD1),
        supported_guest_isas: BTreeSet::from([SoraInrouGuestIsaV1::X8664]),
        trusted_guest_artifact: SoraPublishedInrouGuestImageArtifactV1 {
            manifest_digest_hex: "31".repeat(32),
            content_cid: "bafyr6ibrgeytcmjrgeytcmjrgeytcmjrgeytcmjrgeytcmjrgeytcmjrge".to_owned(),
        },
        max_hosted_replica_capacity: SORA_INROU_HOSTED_REPLICA_CAPACITY_V1,
        max_cpu_millis: 4_000,
        max_memory_bytes: 16 * 1024 * 1024 * 1024,
        max_storage_bytes: 64 * 1024 * 1024 * 1024,
        advertised_at_ms: 100_000,
        heartbeat_expires_at_ms: 160_000,
    }
}
fn sample_inrou_service_placement_record() -> SoraInrouServicePlacementRecordV1 {
    SoraInrouServicePlacementRecordV1 {
        schema_version: SORA_INROU_SERVICE_PLACEMENT_RECORD_VERSION_V1,
        service_name: sample_name("portal"),
        service_version: "2026.4".to_string(),
        desired_replica_count: 2,
        eligible_validator_count: 3,
        placements: vec![
            SoraInrouReplicaPlacementV1 {
                replica_slot: 1,
                economic_clock: SoraServiceLeaseClockV1::CanonicalBlockHeight,
                lease_started_height: 1,
                placement_incarnation: Hash::new(b"placement-1"),
                host_availability: SoraInrouReplicaHostAvailabilityV1::Available,
                validator_account_id: sample_account_id(0xD1),
                peer_id: sample_peer_id(0xD1),
                selected_guest_isa: SoraInrouGuestIsaV1::X8664,
            },
            SoraInrouReplicaPlacementV1 {
                replica_slot: 2,
                economic_clock: SoraServiceLeaseClockV1::CanonicalBlockHeight,
                lease_started_height: 1,
                placement_incarnation: Hash::new(b"placement-2"),
                host_availability: SoraInrouReplicaHostAvailabilityV1::Available,
                validator_account_id: sample_account_id(0xD2),
                peer_id: sample_peer_id(0xD2),
                selected_guest_isa: SoraInrouGuestIsaV1::Aarch64,
            },
        ],
        reconciled_at_ms: 125_000,
        last_error: None,
    }
}
#[test]
fn inrou_service_placement_requires_sorted_unique_bounded_slots() {
    let mut aliased_version = sample_inrou_service_placement_record();
    aliased_version.service_version.push(' ');
    aliased_version
        .validate()
        .expect_err("Inrou placement service-version aliases must fail closed");

    let mut reordered = sample_inrou_service_placement_record();
    reordered.placements.swap(0, 1);
    let error = reordered
        .validate()
        .expect_err("reordered Inrou placements must be rejected");
    assert!(
        error
            .to_string()
            .contains("strictly increasing unique replica slots"),
        "unexpected reordered placement error: {error}"
    );

    let mut holey = sample_inrou_service_placement_record();
    holey.placements.remove(0);
    holey
        .validate()
        .expect("sparse Inrou placement slots are valid when an earlier slot has no host");

    let mut duplicate_slot = sample_inrou_service_placement_record();
    duplicate_slot.placements[1].replica_slot = 1;
    let error = duplicate_slot
        .validate()
        .expect_err("duplicate Inrou placement slots must be rejected");
    assert!(
        error
            .to_string()
            .contains("strictly increasing unique replica slots"),
        "unexpected duplicate-slot placement error: {error}"
    );

    let mut out_of_range = sample_inrou_service_placement_record();
    out_of_range.placements[1].replica_slot = 3;
    let error = out_of_range
        .validate()
        .expect_err("slots beyond desired_replica_count must be rejected");
    assert!(
        error
            .to_string()
            .contains("bounded by desired_replica_count"),
        "unexpected out-of-range placement error: {error}"
    );
}
#[test]
fn inrou_service_placement_requires_one_distinct_eligible_host_per_slot() {
    let mut duplicate_validator = sample_inrou_service_placement_record();
    duplicate_validator.placements[1].validator_account_id = duplicate_validator.placements[0]
        .validator_account_id
        .clone();
    duplicate_validator.placements[1].peer_id = duplicate_validator.placements[0].peer_id.clone();
    let error = duplicate_validator
        .validate()
        .expect_err("duplicate Inrou placement validators must be rejected");
    assert!(
        error.to_string().contains("distinct validator account"),
        "unexpected duplicate-validator error: {error}"
    );

    let mut overcommitted = sample_inrou_service_placement_record();
    overcommitted.eligible_validator_count = 1;
    let error = overcommitted
        .validate()
        .expect_err("placements beyond the eligible validator count must be rejected");
    assert!(
        error.to_string().contains("eligible_validator_count"),
        "unexpected eligible-validator-count error: {error}"
    );

    overcommitted.placements[1].host_availability = SoraInrouReplicaHostAvailabilityV1::Unavailable;
    overcommitted
        .validate()
        .expect("an unavailable retained assignment does not consume current eligible capacity");
}
#[test]
fn inrou_replica_host_availability_norito_roundtrips_both_states() {
    for host_availability in [
        SoraInrouReplicaHostAvailabilityV1::Available,
        SoraInrouReplicaHostAvailabilityV1::Unavailable,
    ] {
        let mut placement = sample_inrou_service_placement_record().placements.remove(0);
        placement.host_availability = host_availability;
        let encoded = placement.encode();
        let decoded = SoraInrouReplicaPlacementV1::decode_all(&mut encoded.as_slice())
            .expect("decode canonical Inrou replica placement");
        assert_eq!(decoded, placement);
    }
}
#[test]
fn hf_free_form_error_and_detail_text_remains_byte_exact() {
    let mut runtime = sample_inrou_replica_runtime_state();
    runtime.last_error = Some(" guest restart scheduled\noperator notified ".to_owned());
    runtime
        .validate()
        .expect("nonblank free-form Inrou runtime errors remain valid");
    assert_eq!(
        runtime.last_error.as_deref(),
        Some(" guest restart scheduled\noperator notified ")
    );

    let mut job = sample_training_job_record();
    job.last_failure_reason = Some(" retry after checkpoint\nworker replaced ".to_owned());
    job.validate()
        .expect("nonblank free-form training reasons remain valid");
    assert_eq!(
        job.last_failure_reason.as_deref(),
        Some(" retry after checkpoint\nworker replaced ")
    );

    let mut audit = sample_training_job_audit_event();
    audit.last_failure_reason = Some(" retry after checkpoint\nworker replaced ".to_owned());
    audit
        .validate()
        .expect("nonblank free-form training audit reasons remain valid");

    for blank in ["", " \t\n "] {
        let mut runtime = sample_inrou_replica_runtime_state();
        runtime.last_error = Some(blank.to_owned());
        runtime
            .validate()
            .expect_err("blank Inrou runtime errors must fail closed");

        let mut job = sample_training_job_record();
        job.last_failure_reason = Some(blank.to_owned());
        job.validate()
            .expect_err("blank training failure reasons must fail closed");

        let mut audit = sample_training_job_audit_event();
        audit.last_failure_reason = Some(blank.to_owned());
        audit
            .validate()
            .expect_err("blank training audit failure reasons must fail closed");
    }
}
fn sample_inrou_replica_runtime_state() -> SoraInrouReplicaRuntimeStateV1 {
    SoraInrouReplicaRuntimeStateV1 {
        schema_version: SORA_INROU_REPLICA_RUNTIME_STATE_VERSION_V1,
        service_name: sample_name("portal"),
        service_version: "2026.4".to_string(),
        replica_slot: 1,
        placement_incarnation: Hash::new(b"placement-1"),
        validator_account_id: sample_account_id(0xD1),
        peer_id: sample_peer_id(0xD1),
        selected_guest_isa: SoraInrouGuestIsaV1::X8664,
        health_status: SoraServiceHealthStatusV1::Healthy,
        load_factor_bps: 375,
        materialized_bundle_hash: sample_hash(28),
        reporting_epoch: 1,
        accounted_egress_bytes: 4_096,
        updated_at_ms: 130_000,
        last_error: None,
    }
}
fn sample_hf_shared_lease_audit_event() -> SoraHfSharedLeaseAuditEventV1 {
    SoraHfSharedLeaseAuditEventV1 {
        schema_version: SORA_HF_SHARED_LEASE_AUDIT_EVENT_VERSION_V1,
        sequence: 44,
        action: SoraHfSharedLeaseActionV1::Join,
        pool_id: sample_hash(23),
        source_id: sample_hash(21),
        account_id: sample_account_id(0xB2),
        occurred_at_ms: 20_000,
        active_member_count: 2,
        charged: xor_quantity_from_nanos(5_000),
        refunded: Quantity::zero(),
        lease_expires_at_ms: 604_810_000,
        failure_reason: None,
        service_name: Some("demo_service".to_string()),
        apartment_name: Some("demo_apartment".to_string()),
    }
}
fn sample_training_job_record() -> SoraTrainingJobRecordV1 {
    SoraTrainingJobRecordV1 {
        schema_version: SORA_TRAINING_JOB_RECORD_VERSION_V1,
        service_name: sample_name("service"),
        service_version: "2026.1".to_string(),
        model_name: "vision_model".to_string(),
        job_id: "job-1".to_string(),
        status: SoraTrainingJobStatusV1::Running,
        worker_group_size: 4,
        target_steps: 100,
        completed_steps: 20,
        checkpoint_interval_steps: 10,
        last_checkpoint_step: Some(20),
        checkpoint_count: 2,
        retry_count: 0,
        max_retries: 3,
        step_compute_units: 50,
        compute_budget_units: 40_000,
        compute_consumed_units: 4_000,
        storage_budget_bytes: 8_192,
        storage_consumed_bytes: 2_048,
        latest_metrics_hash: Some(sample_hash(1)),
        last_failure_reason: None,
        created_sequence: 5,
        updated_sequence: 7,
    }
}
fn sample_training_job_audit_event() -> SoraTrainingJobAuditEventV1 {
    SoraTrainingJobAuditEventV1 {
        schema_version: SORA_TRAINING_JOB_AUDIT_EVENT_VERSION_V1,
        sequence: 7,
        action: SoraTrainingJobActionV1::Checkpoint,
        service_name: sample_name("service"),
        service_version: "2026.1".to_string(),
        model_name: "vision_model".to_string(),
        job_id: "job-1".to_string(),
        status: SoraTrainingJobStatusV1::Running,
        completed_steps: 20,
        checkpoint_count: 2,
        retry_count: 0,
        compute_consumed_units: 4_000,
        storage_consumed_bytes: 2_048,
        last_checkpoint_step: Some(20),
        latest_metrics_hash: Some(sample_hash(1)),
        last_failure_reason: None,
        signer: sample_signer(),
    }
}
fn sample_model_registry() -> SoraModelRegistryV1 {
    SoraModelRegistryV1 {
        schema_version: SORA_MODEL_REGISTRY_VERSION_V1,
        service_name: sample_name("service"),
        service_version: "2026.1".to_string(),
        model_name: "vision_model".to_string(),
        current_version: Some("v1".to_string()),
        updated_sequence: 9,
    }
}
fn sample_model_weight_version_record() -> SoraModelWeightVersionRecordV1 {
    SoraModelWeightVersionRecordV1 {
        schema_version: SORA_MODEL_WEIGHT_VERSION_RECORD_VERSION_V1,
        service_name: sample_name("service"),
        service_version: "2026.1".to_string(),
        model_name: "vision_model".to_string(),
        weight_version: "v2".to_string(),
        parent_version: Some("v1".to_string()),
        training_job_id: "job-1".to_string(),
        source_provenance: Some(sample_model_provenance_ref()),
        weight_artifact_hash: sample_hash(2),
        dataset_ref: "dataset://train".to_string(),
        training_config_hash: sample_hash(3),
        reproducibility_hash: sample_hash(4),
        provenance_attestation_hash: sample_hash(5),
        registered_sequence: 10,
        promoted_sequence: Some(12),
        gate_report_hash: Some(sample_hash(6)),
        promoted_by: Some(sample_signer()),
    }
}
fn sample_model_weight_audit_event() -> SoraModelWeightAuditEventV1 {
    SoraModelWeightAuditEventV1 {
        schema_version: SORA_MODEL_WEIGHT_AUDIT_EVENT_VERSION_V1,
        sequence: 12,
        action: SoraModelWeightActionV1::Promote,
        service_name: sample_name("service"),
        service_version: "2026.1".to_string(),
        model_name: "vision_model".to_string(),
        target_version: "v2".to_string(),
        current_version: Some("v2".to_string()),
        parent_version: Some("v1".to_string()),
        gate_approved: Some(true),
        rollback_reason: None,
        signer: sample_signer(),
    }
}
fn sample_model_artifact_record() -> SoraModelArtifactRecordV1 {
    SoraModelArtifactRecordV1 {
        schema_version: SORA_MODEL_ARTIFACT_RECORD_VERSION_V1,
        service_name: sample_name("service"),
        service_version: "2026.1".to_string(),
        model_name: "vision_model".to_string(),
        artifact_id: "job-1".to_string(),
        training_job_id: "job-1".to_string(),
        weight_version: Some("v2".to_string()),
        source_provenance: Some(sample_model_provenance_ref()),
        weight_artifact_hash: sample_hash(7),
        dataset_ref: "dataset://train".to_string(),
        training_config_hash: sample_hash(8),
        reproducibility_hash: sample_hash(9),
        provenance_attestation_hash: sample_hash(10),
        registered_sequence: 11,
        consumed_by_version: Some("v2".to_string()),
        chunk_manifest_root: None,
    }
}
fn sample_model_artifact_audit_event() -> SoraModelArtifactAuditEventV1 {
    SoraModelArtifactAuditEventV1 {
        schema_version: SORA_MODEL_ARTIFACT_AUDIT_EVENT_VERSION_V1,
        sequence: 11,
        action: SoraModelArtifactActionV1::Register,
        service_name: sample_name("service"),
        service_version: "2026.1".to_string(),
        model_name: "vision_model".to_string(),
        training_job_id: "job-1".to_string(),
        consumed_by_version: Some("v2".to_string()),
        signer: sample_signer(),
    }
}
#[test]
fn model_artifact_audit_identifiers_are_exact_tokens() {
    let canonical = sample_model_artifact_audit_event();
    canonical
        .validate()
        .expect("canonical model-artifact audit event");

    let mut padded_service_version = canonical.clone();
    padded_service_version.service_version.push(' ');
    padded_service_version
        .validate()
        .expect_err("service-version aliases must fail closed");

    let mut spaced_model_name = canonical.clone();
    spaced_model_name.model_name = "vision model".to_owned();
    spaced_model_name
        .validate()
        .expect_err("model names must be exact tokens");

    let mut padded_job = canonical.clone();
    padded_job.training_job_id.insert(0, ' ');
    padded_job
        .validate()
        .expect_err("training-job aliases must fail closed");

    let mut padded_consumed_version = canonical;
    padded_consumed_version.consumed_by_version = Some("v2 ".to_owned());
    padded_consumed_version
        .validate()
        .expect_err("consumed-version aliases must fail closed");
}
#[cfg(feature = "json")]
#[test]
fn canonical_deployment_and_hosting_json_graph_rejects_unknown_fields() {
    macro_rules! assert_unknown_rejected {
        ($ty:ty, $label:literal) => {{
            let error = norito::json::from_str::<$ty>(r#"{"retired_v0":true}"#)
                .expect_err(concat!($label, " must reject an unknown field"));
            assert!(
                matches!(
                    error,
                    norito::json::Error::UnknownField { ref field } if field == "retired_v0"
                ),
                "{} reported the wrong error: {error:?}",
                $label
            );
        }};
    }

    assert_unknown_rejected!(SoraServiceLifecycleActionV1, "service lifecycle action");
    assert_unknown_rejected!(SoraStateMutationOperationV1, "state mutation operation");
    assert_unknown_rejected!(SoraRolloutStageV1, "rollout stage");
    assert_unknown_rejected!(SoraServiceRolloutStateV1, "service rollout state");
    assert_unknown_rejected!(SoraServiceDeploymentStateV1, "service deployment state");
    assert_unknown_rejected!(SoraServiceConfigEntryV1, "service config entry");
    assert_unknown_rejected!(SoraServiceSecretEntryV1, "service secret entry");
    assert_unknown_rejected!(SoraServiceStateEntryV1, "service state entry");
    assert_unknown_rejected!(SoraDecryptionRequestRecordV1, "decryption request record");
    assert_unknown_rejected!(SoraTrainingJobStatusV1, "training status");
    assert_unknown_rejected!(SoraTrainingJobActionV1, "training action");
    assert_unknown_rejected!(SoraTrainingJobRecordV1, "training record");
    assert_unknown_rejected!(SoraTrainingJobAuditEventV1, "training audit event");
    assert_unknown_rejected!(SoraModelRegistryV1, "model registry");
    assert_unknown_rejected!(SoraModelWeightActionV1, "model-weight action");
    assert_unknown_rejected!(SoraModelProvenanceKindV1, "model provenance kind");
    assert_unknown_rejected!(SoraModelProvenanceRefV1, "model provenance ref");
    assert_unknown_rejected!(
        SoraUploadedModelPackageFormatV1,
        "uploaded-model package format"
    );
    assert_unknown_rejected!(SoraUploadedModelPricingPolicyV1, "uploaded-model pricing");
    assert_unknown_rejected!(SoraUploadedModelBundleV1, "uploaded-model bundle");
    assert_unknown_rejected!(SoraModelWeightVersionRecordV1, "model-weight record");
    assert_unknown_rejected!(SoraModelWeightAuditEventV1, "model-weight audit event");
    assert_unknown_rejected!(SoraModelArtifactActionV1, "model-artifact action");
    assert_unknown_rejected!(SoraModelArtifactRecordV1, "model-artifact record");

    assert_unknown_rejected!(SoraInrouHostCapabilityRecordV1, "Inrou host capability");
    assert_unknown_rejected!(
        SoraInrouReplicaHostAvailabilityV1,
        "Inrou replica host availability"
    );
    assert_unknown_rejected!(SoraInrouReplicaPlacementV1, "Inrou replica placement");
    assert_unknown_rejected!(SoraInrouServicePlacementRecordV1, "Inrou service placement");
    assert_unknown_rejected!(SoraHfSourceRecordV1, "HF source record");
    assert_unknown_rejected!(SoraHfSharedLeaseStatusV1, "HF shared-lease status");
    assert_unknown_rejected!(
        SoraHfSharedLeaseMemberStatusV1,
        "HF shared-lease member status"
    );
    assert_unknown_rejected!(SoraHfSharedLeaseActionV1, "HF shared-lease action");
    assert_unknown_rejected!(SoraHfSharedLeaseQueuedWindowV1, "HF queued lease window");
    assert_unknown_rejected!(SoraHfSharedLeasePoolV1, "HF shared-lease pool");
    assert_unknown_rejected!(SoraHfSharedLeaseMemberV1, "HF shared-lease member");
    assert_unknown_rejected!(SoraHfSharedLeaseAuditEventV1, "HF shared-lease audit event");
    assert_unknown_rejected!(SoraModelArtifactAuditEventV1, "model-artifact audit event");
    assert_unknown_rejected!(SoraAgentApartmentActionV1, "agent apartment action");
    assert_unknown_rejected!(SoraAgentRuntimeStatusV1, "agent runtime status");
    assert_unknown_rejected!(SoraAgentWalletSpendRequestV1, "agent wallet-spend request");
    assert_unknown_rejected!(SoraAgentWalletDailySpendEntryV1, "agent daily-spend entry");
    assert_unknown_rejected!(SoraAgentMailboxMessageV1, "agent mailbox message");
    assert_unknown_rejected!(SoraAgentArtifactAllowRuleV1, "agent artifact rule");
    assert_unknown_rejected!(SoraAgentAutonomyRunRecordV1, "agent autonomy-run record");
    assert_unknown_rejected!(SoraAgentPersistentStateV1, "agent persistent state");
    assert_unknown_rejected!(SoraAgentApartmentRecordV1, "agent apartment record");
    assert_unknown_rejected!(
        SoraAgentApartmentAuditEventV1,
        "agent apartment audit event"
    );
    assert_unknown_rejected!(SoraAppInfraActionV1, "app-infra action");
    assert_unknown_rejected!(SoraAppStaticSiteBindingV1, "app static-site binding");
    assert_unknown_rejected!(SoraAppRouteProjectionV1, "app route projection");
    assert_unknown_rejected!(SoraAppInfraServiceRefV1, "app-infra service ref");
    assert_unknown_rejected!(SoraAppInfraManifestV1, "app-infra manifest");
    assert_unknown_rejected!(SoraAppInfraStateV1, "app-infra state");
    assert_unknown_rejected!(SoraAppInfraAuditEventV1, "app-infra audit event");
    assert_unknown_rejected!(SoraServiceAuditEventV1, "service audit event");
    assert_unknown_rejected!(SoraServiceHealthStatusV1, "service health status");
    assert_unknown_rejected!(SoraServiceRuntimeStateV1, "service runtime state");
    assert_unknown_rejected!(
        SoraInrouReplicaRuntimeStateV1,
        "Inrou replica runtime state"
    );
    assert_unknown_rejected!(SoraServiceMailboxMessageV1, "service mailbox message");
    assert_unknown_rejected!(
        SoraRuntimeDeterministicValidatorHostV1,
        "deterministic validator execution host"
    );
    assert_unknown_rejected!(
        SoraOrderedMailboxStateMutationV1,
        "ordered mailbox state mutation"
    );
    assert_unknown_rejected!(SoraOrderedMailboxResultV1, "ordered mailbox result");
    assert_unknown_rejected!(SoraRuntimeReceiptV1, "runtime receipt");
}
#[cfg(feature = "json")]
#[test]
fn canonical_deployment_and_hosting_json_graph_requires_explicit_keys() {
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
        sample_training_job_record(),
        SoraTrainingJobRecordV1,
        [
            "last_checkpoint_step",
            "latest_metrics_hash",
            "last_failure_reason",
        ],
        [
            "last_checkpoint_step",
            "latest_metrics_hash",
            "last_failure_reason",
        ],
        "training-job record"
    );
    assert_required_keys!(
        sample_training_job_audit_event(),
        SoraTrainingJobAuditEventV1,
        [
            "last_checkpoint_step",
            "latest_metrics_hash",
            "last_failure_reason",
        ],
        [
            "last_checkpoint_step",
            "latest_metrics_hash",
            "last_failure_reason",
        ],
        "training-job audit event"
    );
    assert_required_keys!(
        sample_model_registry(),
        SoraModelRegistryV1,
        ["current_version"],
        ["current_version"],
        "model registry"
    );
    assert_required_keys!(
        sample_uploaded_model_bundle(),
        SoraUploadedModelBundleV1,
        ["modalities"],
        [],
        "uploaded-model bundle"
    );
    assert_required_keys!(
        sample_model_weight_version_record(),
        SoraModelWeightVersionRecordV1,
        [
            "parent_version",
            "training_job_id",
            "source_provenance",
            "promoted_sequence",
            "gate_report_hash",
            "promoted_by",
        ],
        [
            "parent_version",
            "source_provenance",
            "promoted_sequence",
            "gate_report_hash",
            "promoted_by",
        ],
        "model-weight record"
    );
    assert_required_keys!(
        sample_model_weight_audit_event(),
        SoraModelWeightAuditEventV1,
        [
            "current_version",
            "parent_version",
            "gate_approved",
            "rollback_reason",
        ],
        [
            "current_version",
            "parent_version",
            "gate_approved",
            "rollback_reason",
        ],
        "model-weight audit event"
    );
    assert_required_keys!(
        sample_model_artifact_record(),
        SoraModelArtifactRecordV1,
        [
            "training_job_id",
            "weight_version",
            "source_provenance",
            "consumed_by_version",
            "chunk_manifest_root",
        ],
        [
            "weight_version",
            "source_provenance",
            "consumed_by_version",
            "chunk_manifest_root",
        ],
        "model-artifact record"
    );
    assert_required_keys!(
        sample_model_artifact_audit_event(),
        SoraModelArtifactAuditEventV1,
        ["consumed_by_version"],
        ["consumed_by_version"],
        "model-artifact audit event"
    );

    let queued_window = SoraHfSharedLeaseQueuedWindowV1 {
        sponsor_account_id: sample_account_id(0xB2),
        lease_asset_definition_id: sample_asset_definition_id("4cuvDVPuLBKJyN6dPbRQhmLh68sU"),
        base_fee: xor_quantity_from_nanos(10_000),
        sponsored_at_ms: 1_000,
        window_started_at_ms: 2_000,
        window_expires_at_ms: 3_000,
        service_name: sample_name("demo_service"),
        apartment_name: None,
    };
    assert_required_keys!(
        queued_window,
        SoraHfSharedLeaseQueuedWindowV1,
        ["apartment_name"],
        ["apartment_name"],
        "HF queued lease window"
    );
    assert_required_keys!(
        sample_hf_shared_lease_pool(),
        SoraHfSharedLeasePoolV1,
        ["queued_next_window"],
        ["queued_next_window"],
        "HF shared-lease pool"
    );
    assert_required_keys!(
        sample_hf_shared_lease_member(),
        SoraHfSharedLeaseMemberV1,
        ["service_bindings", "apartment_bindings"],
        [],
        "HF shared-lease member"
    );
    assert_required_keys!(
        sample_hf_shared_lease_audit_event(),
        SoraHfSharedLeaseAuditEventV1,
        ["service_name", "apartment_name"],
        ["service_name", "apartment_name"],
        "HF shared-lease audit event"
    );
}
#[cfg(feature = "json")]
#[test]
fn inrou_v1_wire_records_reject_retired_fields() {
    let cases = [
        (
            norito::json::to_value(&sample_inrou_host_capability_record())
                .expect("serialize Inrou host capability"),
            "supported_backends",
            norito::json!([{"backend": "PortableVm", "value": null}]),
            "host capability",
        ),
        (
            norito::json::to_value(&sample_inrou_host_capability_record())
                .expect("serialize Inrou host capability"),
            "geography_tags",
            norito::json!(["ae-dxb"]),
            "host capability",
        ),
        (
            norito::json::to_value(&sample_inrou_host_capability_record())
                .expect("serialize Inrou host capability"),
            "observed_latency_ms",
            norito::json!(24),
            "host capability",
        ),
        (
            norito::json::to_value(&sample_inrou_service_placement_record().placements[0])
                .expect("serialize Inrou placement"),
            "selected_backend",
            norito::json!({"backend": "PortableVm", "value": null}),
            "replica placement",
        ),
        (
            norito::json::to_value(&sample_inrou_service_placement_record().placements[0])
                .expect("serialize Inrou placement"),
            "selected_geography_tag",
            norito::json!("ae-dxb"),
            "replica placement",
        ),
        (
            norito::json::to_value(&sample_inrou_service_placement_record().placements[0])
                .expect("serialize Inrou placement"),
            "selection_latency_ms",
            norito::json!(24),
            "replica placement",
        ),
        (
            norito::json::to_value(&sample_inrou_service_placement_record())
                .expect("serialize Inrou service placement record"),
            "selected_backend",
            norito::json!({"backend": "PortableVm", "value": null}),
            "service placement record",
        ),
        (
            norito::json::to_value(&sample_inrou_replica_runtime_state())
                .expect("serialize Inrou runtime state"),
            "selected_backend",
            norito::json!({"backend": "PortableVm", "value": null}),
            "replica runtime state",
        ),
        (
            norito::json::to_value(&sample_inrou_service_placement_record().placements[0])
                .expect("serialize Inrou placement"),
            "requires_state_migration",
            norito::json!(false),
            "replica placement",
        ),
    ];
    for (mut value, retired_field, retired_value, label) in cases {
        let object = value.as_object_mut().expect("Inrou wire record object");
        assert!(
            !object.contains_key(retired_field),
            "canonical {label} must not serialize retired `{retired_field}`"
        );
        object.insert(retired_field.to_owned(), retired_value);
        let rejected = match label {
            "host capability" => {
                norito::json::from_value::<SoraInrouHostCapabilityRecordV1>(value).is_err()
            }
            "replica placement" => {
                norito::json::from_value::<SoraInrouReplicaPlacementV1>(value).is_err()
            }
            "service placement record" => {
                norito::json::from_value::<SoraInrouServicePlacementRecordV1>(value).is_err()
            }
            "replica runtime state" => {
                norito::json::from_value::<SoraInrouReplicaRuntimeStateV1>(value).is_err()
            }
            _ => unreachable!("fixed test case"),
        };
        assert!(rejected, "{label} must reject retired `{retired_field}`");
    }
}
#[cfg(feature = "json")]
#[test]
fn inrou_v1_wire_records_require_every_canonical_field() {
    macro_rules! assert_missing_rejected {
        ($value:expr, $field:literal, $ty:ty, $label:literal) => {{
            let mut value = norito::json::to_value(&$value).expect("serialize Inrou V1 record");
            let removed = value
                .as_object_mut()
                .expect("Inrou V1 record object")
                .remove($field);
            assert!(removed.is_some(), "fixture must contain `{}`", $field);
            norito::json::from_value::<$ty>(value)
                .expect_err(concat!($label, " must reject a missing canonical field"));
        }};
    }

    let host_capability = sample_inrou_host_capability_record();
    assert_missing_rejected!(
        host_capability,
        "trusted_guest_artifact",
        SoraInrouHostCapabilityRecordV1,
        "host capability"
    );
    let placement_record = sample_inrou_service_placement_record();
    let placement = placement_record.placements[0].clone();
    assert_missing_rejected!(
        placement,
        "host_availability",
        SoraInrouReplicaPlacementV1,
        "replica placement"
    );
    assert_missing_rejected!(
        placement_record,
        "placements",
        SoraInrouServicePlacementRecordV1,
        "service placement record"
    );
    assert_missing_rejected!(
        placement_record,
        "last_error",
        SoraInrouServicePlacementRecordV1,
        "service placement record"
    );
    let runtime = sample_inrou_replica_runtime_state();
    assert_missing_rejected!(
        runtime,
        "accounted_egress_bytes",
        SoraInrouReplicaRuntimeStateV1,
        "replica runtime state"
    );
    assert_missing_rejected!(
        runtime,
        "reporting_epoch",
        SoraInrouReplicaRuntimeStateV1,
        "replica runtime state"
    );
    assert_missing_rejected!(
        runtime,
        "last_error",
        SoraInrouReplicaRuntimeStateV1,
        "replica runtime state"
    );
}
#[test]
fn inrou_v1_norito_records_reject_retired_distribution_hints() {
    #[derive(Encode)]
    struct RetiredHostDistributionHintsV1 {
        schema_version: u16,
        validator_account_id: AccountId,
        peer_id: String,
        supported_guest_isas: BTreeSet<SoraInrouGuestIsaV1>,
        max_hosted_replica_capacity: u16,
        max_cpu_millis: u32,
        max_memory_bytes: u64,
        max_storage_bytes: u64,
        geography_tags: BTreeSet<String>,
        observed_latency_ms: Option<u32>,
        advertised_at_ms: u64,
        heartbeat_expires_at_ms: u64,
    }
    #[derive(Encode)]
    struct RetiredPlacementDistributionHintsV1 {
        replica_slot: u16,
        lease_started_sequence: u64,
        placement_incarnation: Hash,
        host_availability: SoraInrouReplicaHostAvailabilityV1,
        validator_account_id: AccountId,
        peer_id: String,
        selected_guest_isa: SoraInrouGuestIsaV1,
        selected_geography_tag: Option<String>,
        selection_latency_ms: Option<u32>,
    }

    let host = sample_inrou_host_capability_record();
    let retired_host = RetiredHostDistributionHintsV1 {
        schema_version: host.schema_version,
        validator_account_id: host.validator_account_id,
        peer_id: host.peer_id,
        supported_guest_isas: host.supported_guest_isas,
        max_hosted_replica_capacity: host.max_hosted_replica_capacity,
        max_cpu_millis: host.max_cpu_millis,
        max_memory_bytes: host.max_memory_bytes,
        max_storage_bytes: host.max_storage_bytes,
        geography_tags: BTreeSet::from(["ae-dxb".to_owned()]),
        observed_latency_ms: Some(24),
        advertised_at_ms: host.advertised_at_ms,
        heartbeat_expires_at_ms: host.heartbeat_expires_at_ms,
    };
    let placement = sample_inrou_service_placement_record()
        .placements
        .into_iter()
        .next()
        .expect("sample Inrou placement");
    let retired_placement = RetiredPlacementDistributionHintsV1 {
        replica_slot: placement.replica_slot,
        lease_started_sequence: placement.lease_started_height,
        placement_incarnation: placement.placement_incarnation,
        host_availability: placement.host_availability,
        validator_account_id: placement.validator_account_id,
        peer_id: placement.peer_id,
        selected_guest_isa: placement.selected_guest_isa,
        selected_geography_tag: Some("ae-dxb".to_owned()),
        selection_latency_ms: Some(24),
    };

    let retired_host_bytes = retired_host.encode();
    assert!(
        SoraInrouHostCapabilityRecordV1::decode_all(&mut retired_host_bytes.as_slice()).is_err(),
        "host capability must reject the retired distribution-hint Norito layout"
    );
    let retired_placement_bytes = retired_placement.encode();
    assert!(
        SoraInrouReplicaPlacementV1::decode_all(&mut retired_placement_bytes.as_slice()).is_err(),
        "replica placement must reject the retired distribution-hint Norito layout"
    );
}
#[test]
fn inrou_v1_norito_records_reject_retired_backend_selector_layouts() {
    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode)]
    enum RetiredBackend {
        PortableVm,
    }
    #[derive(Encode)]
    struct RetiredHostCapability {
        schema_version: u16,
        validator_account_id: AccountId,
        peer_id: String,
        supported_backends: BTreeSet<RetiredBackend>,
        supported_guest_isas: BTreeSet<SoraInrouGuestIsaV1>,
        max_hosted_replica_capacity: u16,
        max_cpu_millis: u32,
        max_memory_bytes: u64,
        max_storage_bytes: u64,
        advertised_at_ms: u64,
        heartbeat_expires_at_ms: u64,
    }
    #[derive(Encode)]
    struct RetiredReplicaPlacement {
        replica_slot: u16,
        lease_started_sequence: u64,
        placement_incarnation: Hash,
        host_availability: SoraInrouReplicaHostAvailabilityV1,
        validator_account_id: AccountId,
        peer_id: String,
        selected_backend: RetiredBackend,
        selected_guest_isa: SoraInrouGuestIsaV1,
    }
    #[derive(Encode)]
    struct RetiredReplicaRuntimeState {
        schema_version: u16,
        service_name: Name,
        service_version: String,
        replica_slot: u16,
        validator_account_id: AccountId,
        peer_id: String,
        selected_backend: RetiredBackend,
        selected_guest_isa: SoraInrouGuestIsaV1,
        health_status: SoraServiceHealthStatusV1,
        load_factor_bps: u16,
        materialized_bundle_hash: Hash,
        accounted_egress_bytes: u64,
        pending_mailbox_message_count: u32,
        last_receipt_id: Option<Hash>,
        updated_at_ms: u64,
        last_error: Option<String>,
    }

    let host = sample_inrou_host_capability_record();
    let retired_host = RetiredHostCapability {
        schema_version: host.schema_version,
        validator_account_id: host.validator_account_id,
        peer_id: host.peer_id,
        supported_backends: BTreeSet::from([RetiredBackend::PortableVm]),
        supported_guest_isas: host.supported_guest_isas,
        max_hosted_replica_capacity: host.max_hosted_replica_capacity,
        max_cpu_millis: host.max_cpu_millis,
        max_memory_bytes: host.max_memory_bytes,
        max_storage_bytes: host.max_storage_bytes,
        advertised_at_ms: host.advertised_at_ms,
        heartbeat_expires_at_ms: host.heartbeat_expires_at_ms,
    };
    let placement = sample_inrou_service_placement_record()
        .placements
        .into_iter()
        .next()
        .expect("sample Inrou placement");
    let retired_placement = RetiredReplicaPlacement {
        replica_slot: placement.replica_slot,
        lease_started_sequence: placement.lease_started_height,
        placement_incarnation: placement.placement_incarnation,
        host_availability: placement.host_availability,
        validator_account_id: placement.validator_account_id,
        peer_id: placement.peer_id,
        selected_backend: RetiredBackend::PortableVm,
        selected_guest_isa: placement.selected_guest_isa,
    };
    let runtime = sample_inrou_replica_runtime_state();
    let retired_runtime = RetiredReplicaRuntimeState {
        schema_version: runtime.schema_version,
        service_name: runtime.service_name,
        service_version: runtime.service_version,
        replica_slot: runtime.replica_slot,
        validator_account_id: runtime.validator_account_id,
        peer_id: runtime.peer_id,
        selected_backend: RetiredBackend::PortableVm,
        selected_guest_isa: runtime.selected_guest_isa,
        health_status: runtime.health_status,
        load_factor_bps: runtime.load_factor_bps,
        materialized_bundle_hash: runtime.materialized_bundle_hash,
        accounted_egress_bytes: runtime.accounted_egress_bytes,
        pending_mailbox_message_count: 2,
        last_receipt_id: Some(sample_hash(29)),
        updated_at_ms: runtime.updated_at_ms,
        last_error: runtime.last_error,
    };

    let retired_host_bytes = retired_host.encode();
    assert!(
        SoraInrouHostCapabilityRecordV1::decode_all(&mut retired_host_bytes.as_slice()).is_err(),
        "host capability must reject the retired backend-selector Norito layout"
    );
    let retired_placement_bytes = retired_placement.encode();
    assert!(
        SoraInrouReplicaPlacementV1::decode_all(&mut retired_placement_bytes.as_slice()).is_err(),
        "replica placement must reject the retired backend-selector Norito layout"
    );
    let retired_runtime_bytes = retired_runtime.encode();
    assert!(
        SoraInrouReplicaRuntimeStateV1::decode_all(&mut retired_runtime_bytes.as_slice()).is_err(),
        "replica runtime state must reject the retired backend-selector Norito layout"
    );
}
#[test]
fn rollback_provenance_payload_encodes_canonical_tuple() {
    let encoded =
        encode_rollback_provenance_payload("web_portal", "1.0.1").expect("encode payload");
    let expected = norito::to_bytes(&("web_portal", "1.0.1")).expect("encode tuple");
    assert_eq!(encoded, expected);
}
#[test]
fn state_mutation_provenance_payload_encodes_canonical_tuple() {
    let governance_tx_hash = sample_hash(11);
    let encoded = encode_state_mutation_provenance_payload(
        "health_portal",
        "private_state",
        "/state/private/records/1",
        "upsert",
        Some(512),
        Some(governance_tx_hash),
        SoraStateEncryptionV1::ClientCiphertext,
        governance_tx_hash,
        None,
    )
    .expect("encode payload");
    let expected = norito::to_bytes(&(
        "health_portal",
        "private_state",
        "/state/private/records/1",
        "upsert",
        Some(512u64),
        Some(governance_tx_hash),
        SoraStateEncryptionV1::ClientCiphertext,
        governance_tx_hash,
        None::<SoracloudFheInputAdmissionProofV1>,
    ))
    .expect("encode tuple");
    assert_eq!(encoded, expected);
}
#[test]
fn fhe_input_admission_statement_hash_encodes_canonical_tuple() {
    let payload_commitment = sample_hash(1);
    let governance_tx_hash = sample_hash(2);
    let parameter_digest = sample_hash(3);
    let rns_digest = sample_hash(4);
    let key_switch_decomposition_digest = sample_hash(5);
    let ciphertext_statement_digests = vec![sample_hash(6), sample_hash(7)];
    let statement_hash = derive_soracloud_fhe_input_admission_statement_hash(
        "health_portal",
        "private_state",
        "/state/private/records/1",
        "upsert",
        512,
        payload_commitment,
        SoraStateEncryptionV1::FheCiphertext,
        governance_tx_hash,
        parameter_digest,
        rns_digest,
        key_switch_decomposition_digest,
        &ciphertext_statement_digests,
        129,
    )
    .expect("derive statement hash");
    let expected_payload = norito::to_bytes(&(
        (
            "health_portal",
            "private_state",
            "/state/private/records/1",
            "upsert",
            512_u64,
            payload_commitment,
            SoraStateEncryptionV1::FheCiphertext,
            governance_tx_hash,
        ),
        (
            parameter_digest,
            rns_digest,
            key_switch_decomposition_digest,
        ),
        ciphertext_statement_digests.clone(),
        129_u128,
        BfvCiphertextBoundModeV1::ExactResidualMultiple,
    ))
    .expect("encode tuple");
    assert_eq!(statement_hash, Hash::new(&expected_payload));
    let bounded_statement_hash =
        derive_soracloud_fhe_input_admission_statement_hash_with_bound_mode(
            "health_portal",
            "private_state",
            "/state/private/records/1",
            "upsert",
            512,
            payload_commitment,
            SoraStateEncryptionV1::FheCiphertext,
            governance_tx_hash,
            parameter_digest,
            rns_digest,
            key_switch_decomposition_digest,
            &ciphertext_statement_digests,
            129,
            BfvCiphertextBoundModeV1::BoundedNoise,
        )
        .expect("derive bounded statement hash");
    assert_ne!(statement_hash, bounded_statement_hash);
    assert_eq!(
        soracloud_fhe_input_admission_public_inputs_schema_hash_v1(),
        <[u8; 32]>::from(Hash::new(
            SORACLOUD_FHE_INPUT_ADMISSION_PUBLIC_INPUTS_SCHEMA_V1,
        ))
    );
}
fn sample_fhe_input_admission_proof() -> SoracloudFheInputAdmissionProofV1 {
    let params = ram_lfe_bfv_parameters_v1();
    let (_, public_key, _) = keygen_from_seed(&params, b"soracloud-input-admission-proof-keygen")
        .expect("sample input-admission public key");
    let vk_hash = [0x42; 32];
    let statement_hash = sample_hash(9);
    let open_proof = StarkFriOpenProofV1 {
        version: 1,
        public_inputs: vec![vec![<[u8; Hash::LENGTH]>::from(statement_hash)]],
        envelope_bytes: vec![0xA5; 32],
    };
    let envelope = OpenVerifyEnvelope::new(
        BackendTag::Stark,
        SORACLOUD_FHE_INPUT_ADMISSION_CIRCUIT_ID_V1,
        vk_hash,
        SORACLOUD_FHE_INPUT_ADMISSION_PUBLIC_INPUTS_SCHEMA_V1.to_vec(),
        norito::encode_canonical(&open_proof)
            .expect("encode canonical FHE input admission STARK wrapper"),
    );
    let proof = crate::proof::ProofBox::new(
        "stark/fri/sha256-goldilocks".into(),
        norito::encode_canonical(&envelope)
            .expect("encode canonical FHE input admission OpenVerifyEnvelope"),
    );
    let mut attachment = ProofAttachment::new_ref(
        "stark/fri/sha256-goldilocks".into(),
        proof,
        crate::proof::VerifyingKeyId::new(
            "stark/fri/sha256-goldilocks",
            SORACLOUD_FHE_INPUT_ADMISSION_CIRCUIT_ID_V1,
        ),
    );
    attachment.vk_commitment = Some(vk_hash);
    attachment.envelope_hash = Some(<[u8; 32]>::from(Hash::new(&attachment.proof.bytes)));
    SoracloudFheInputAdmissionProofV1 {
        schema_version: SORACLOUD_FHE_INPUT_ADMISSION_PROOF_VERSION_V1,
        public_key: Some(public_key),
        ciphertext_proof_statement_digests: vec![sample_hash(10)],
        residual_multiple_bound: 17,
        bound_mode: BfvCiphertextBoundModeV1::ExactResidualMultiple,
        statement_hash,
        proof: attachment,
    }
}
fn replace_fhe_input_admission_open_verify_envelope(
    admission: &mut SoracloudFheInputAdmissionProofV1,
    envelope: &OpenVerifyEnvelope,
) {
    admission.proof.proof.bytes = norito::encode_canonical(envelope)
        .expect("encode canonical FHE input admission OpenVerifyEnvelope");
    admission.proof.envelope_hash = Some(<[u8; 32]>::from(Hash::new(&admission.proof.proof.bytes)));
}
fn sample_fhe_public_key_proof() -> SoracloudFhePublicKeyProofV1 {
    let vk_hash = [0x4A; 32];
    let statement_hash = sample_hash(14);
    let open_proof = StarkFriOpenProofV1 {
        version: 1,
        public_inputs: vec![vec![<[u8; Hash::LENGTH]>::from(statement_hash)]],
        envelope_bytes: vec![0xAA; 32],
    };
    let envelope = OpenVerifyEnvelope::new(
        BackendTag::Stark,
        SORACLOUD_FHE_PUBLIC_KEY_PROOF_CIRCUIT_ID_V1,
        vk_hash,
        SORACLOUD_FHE_PUBLIC_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1.to_vec(),
        norito::encode_canonical(&open_proof)
            .expect("encode canonical FHE public-key STARK wrapper"),
    );
    let proof = crate::proof::ProofBox::new(
        "stark/fri/sha256-goldilocks".into(),
        norito::encode_canonical(&envelope)
            .expect("encode canonical FHE public-key OpenVerifyEnvelope"),
    );
    let mut attachment = ProofAttachment::new_ref(
        "stark/fri/sha256-goldilocks".into(),
        proof,
        crate::proof::VerifyingKeyId::new(
            "stark/fri/sha256-goldilocks",
            SORACLOUD_FHE_PUBLIC_KEY_PROOF_CIRCUIT_ID_V1,
        ),
    );
    attachment.vk_commitment = Some(vk_hash);
    attachment.envelope_hash = Some(<[u8; 32]>::from(Hash::new(&attachment.proof.bytes)));
    SoracloudFhePublicKeyProofV1 {
        schema_version: SORACLOUD_FHE_PUBLIC_KEY_PROOF_VERSION_V1,
        statement_hash,
        proof: attachment,
    }
}
fn replace_fhe_public_key_open_verify_envelope(
    proof: &mut SoracloudFhePublicKeyProofV1,
    envelope: &OpenVerifyEnvelope,
) {
    proof.proof.proof.bytes = norito::encode_canonical(envelope)
        .expect("encode canonical FHE public-key OpenVerifyEnvelope");
    proof.proof.envelope_hash = Some(<[u8; 32]>::from(Hash::new(&proof.proof.proof.bytes)));
}
fn sample_fhe_bootstrap_key_proof() -> SoracloudFheBootstrapKeyProofV1 {
    let vk_hash = [0x52; 32];
    let statement_hash = sample_hash(17);
    let open_proof = StarkFriOpenProofV1 {
        version: 1,
        public_inputs: vec![vec![<[u8; Hash::LENGTH]>::from(statement_hash)]],
        envelope_bytes: vec![0xB5; 32],
    };
    let envelope = OpenVerifyEnvelope::new(
        BackendTag::Stark,
        SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_CIRCUIT_ID_V1,
        vk_hash,
        SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1.to_vec(),
        norito::encode_canonical(&open_proof)
            .expect("encode canonical FHE bootstrap-key STARK wrapper"),
    );
    let proof = crate::proof::ProofBox::new(
        "stark/fri/sha256-goldilocks".into(),
        norito::encode_canonical(&envelope)
            .expect("encode canonical FHE bootstrap-key OpenVerifyEnvelope"),
    );
    let mut attachment = ProofAttachment::new_ref(
        "stark/fri/sha256-goldilocks".into(),
        proof,
        crate::proof::VerifyingKeyId::new(
            "stark/fri/sha256-goldilocks",
            SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_CIRCUIT_ID_V1,
        ),
    );
    attachment.vk_commitment = Some(vk_hash);
    attachment.envelope_hash = Some(<[u8; 32]>::from(Hash::new(&attachment.proof.bytes)));
    SoracloudFheBootstrapKeyProofV1 {
        schema_version: SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_VERSION_V1,
        statement_hash,
        proof: attachment,
    }
}
fn replace_fhe_bootstrap_key_open_verify_envelope(
    proof: &mut SoracloudFheBootstrapKeyProofV1,
    envelope: &OpenVerifyEnvelope,
) {
    proof.proof.proof.bytes = norito::encode_canonical(envelope)
        .expect("encode canonical FHE bootstrap-key OpenVerifyEnvelope");
    proof.proof.envelope_hash = Some(<[u8; 32]>::from(Hash::new(&proof.proof.proof.bytes)));
}
fn sample_fhe_full_bootstrap_execution_proof() -> SoracloudFheFullBootstrapExecutionProofV1 {
    sample_fhe_full_bootstrap_execution_proof_with_statement(sample_hash(20))
}
fn sample_fhe_full_bootstrap_execution_proof_with_statement(
    statement_hash: Hash,
) -> SoracloudFheFullBootstrapExecutionProofV1 {
    let vk_hash = [0x63; 32];
    let open_proof = StarkFriOpenProofV1 {
        version: 1,
        public_inputs: vec![vec![<[u8; Hash::LENGTH]>::from(statement_hash)]],
        envelope_bytes: vec![0xD5; 32],
    };
    let envelope = OpenVerifyEnvelope::new(
        BackendTag::Stark,
        SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_CIRCUIT_ID_V1,
        vk_hash,
        SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_PUBLIC_INPUTS_SCHEMA_V1.to_vec(),
        norito::encode_canonical(&open_proof)
            .expect("encode canonical FHE full-bootstrap execution STARK wrapper"),
    );
    let proof = crate::proof::ProofBox::new(
        "stark/fri/sha256-goldilocks".into(),
        norito::encode_canonical(&envelope)
            .expect("encode canonical FHE full-bootstrap execution OpenVerifyEnvelope"),
    );
    let mut attachment = ProofAttachment::new_ref(
        "stark/fri/sha256-goldilocks".into(),
        proof,
        crate::proof::VerifyingKeyId::new(
            "stark/fri/sha256-goldilocks",
            SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_CIRCUIT_ID_V1,
        ),
    );
    attachment.vk_commitment = Some(vk_hash);
    attachment.envelope_hash = Some(<[u8; 32]>::from(Hash::new(&attachment.proof.bytes)));
    SoracloudFheFullBootstrapExecutionProofV1 {
        schema_version: SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_VERSION_V1,
        statement_hash,
        proof: attachment,
    }
}
#[expect(
    clippy::too_many_lines,
    reason = "test fixture enumerates every full-bootstrap artifact role inline"
)]
fn sample_full_bootstrap_circuit_artifacts() -> BfvFullBootstrapCircuitArtifactBundleV1 {
    let params = ram_lfe_bfv_parameters_v1();
    let linear_transform_artifact = |role: BfvFullBootstrapCircuitArtifactRoleV1| {
        let transform = BfvFullBootstrapLinearTransformV1 {
            input_slot_count: params.polynomial_degree,
            output_slot_count: params.polynomial_degree,
            diagonals: vec![BfvFullBootstrapLinearTransformDiagonalV1 {
                rotation_steps: 0,
                plaintext: encode_packed_plaintext_slots(
                    &params,
                    &vec![1; usize::from(params.polynomial_degree)],
                )
                .expect("encode identity packed-slot mask"),
            }],
        };
        encode_bfv_full_bootstrap_linear_transform_artifact_v1(&params, 1, role, &transform)
            .expect("encode sample full-bootstrap linear transform artifact")
    };
    let accumulator = BfvFullBootstrapAccumulatorV1 {
        slot_count: params.polynomial_degree,
        test_vector: encode_packed_plaintext_slots(
            &params,
            &vec![1; usize::from(params.polynomial_degree)],
        )
        .expect("encode sample full-bootstrap accumulator test vector"),
    };
    let sample_extraction = BfvFullBootstrapSampleExtractionV1 {
        source_slot_count: params.polynomial_degree,
        source_ciphertext_component_count: 2,
        extracted_coefficient_index: 0,
        output_ciphertext_component_count: 2,
    };
    let accumulator_artifact =
        encode_bfv_full_bootstrap_accumulator_artifact_v1(&params, 1, &accumulator)
            .expect("encode sample full-bootstrap accumulator artifact");
    let blind_rotation_key = bfv_full_bootstrap_blind_rotation_key_for_packed_left_rotation_v1(
        &params,
        Hash::new(&accumulator_artifact),
        1,
    )
    .expect("build sample full-bootstrap blind-rotation key");
    let proof_public_input_schema =
        encode_bfv_full_bootstrap_proof_public_input_schema_artifact_v1(
            &params,
            1,
            &bfv_full_bootstrap_proof_public_input_schema_v1(),
        )
        .expect("encode sample full-bootstrap proof public-input schema artifact");
    let proof_public_input_schema_digest = Hash::new(&proof_public_input_schema);
    let arithmetic_air_constraint_system =
        encode_bfv_full_bootstrap_arithmetic_air_constraint_system_artifact_v1(
            &params,
            1,
            &bfv_full_bootstrap_arithmetic_air_constraint_system_material_v1(),
        )
        .expect("encode sample full-bootstrap arithmetic AIR constraint-system artifact");
    let coefficient_to_slot_key =
        linear_transform_artifact(BfvFullBootstrapCircuitArtifactRoleV1::CoefficientToSlotKey);
    let slot_to_coefficient_key =
        linear_transform_artifact(BfvFullBootstrapCircuitArtifactRoleV1::SlotToCoefficientKey);
    let blind_rotation_key_artifact =
        encode_bfv_full_bootstrap_blind_rotation_artifact_v1(&params, 1, &blind_rotation_key)
            .expect("encode sample full-bootstrap blind-rotation artifact");
    let (secret_key, _public_key, _relinearization_key) = keygen_from_seed(
        &params,
        b"soracloud-data-model-full-bootstrap-sample-keygen",
    )
    .expect("derive sample full-bootstrap secret key");
    let sample_extraction_switch_key =
        bfv_full_bootstrap_sample_extraction_switch_key_from_seed_v1(
            &params,
            &secret_key,
            sample_extraction,
            b"soracloud-data-model-full-bootstrap-sample-switch-key",
        )
        .expect("derive sample full-bootstrap sample-extraction switch key");
    let sample_extraction_key = encode_bfv_full_bootstrap_sample_extraction_switch_key_artifact_v1(
        &params,
        1,
        &sample_extraction_switch_key,
    )
    .expect("encode sample full-bootstrap sample-extraction switch-key artifact");
    let evaluator_artifact_set_digest = bfv_full_bootstrap_evaluator_artifact_set_digest_v1(
        &params,
        1,
        &coefficient_to_slot_key,
        &slot_to_coefficient_key,
        &blind_rotation_key_artifact,
        &sample_extraction_key,
        &accumulator_artifact,
        &proof_public_input_schema,
        &arithmetic_air_constraint_system,
    )
    .expect("derive sample full-bootstrap evaluator artifact-set digest");
    let prover_key_material = encode_bfv_full_bootstrap_native_stark_fri_prover_key_material_v1(
        SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_CIRCUIT_ID_V1,
    )
    .expect("encode sample native full-bootstrap prover-key material");
    let verifier_key_material =
        encode_bfv_full_bootstrap_native_stark_fri_verifier_key_material_v1(
            SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_CIRCUIT_ID_V1,
        )
        .expect("encode sample native full-bootstrap verifier-key material");
    let (prover_key, verifier_key) = bfv_full_bootstrap_proof_key_pair_from_key_material_v1(
        &params,
        1,
        proof_public_input_schema_digest,
        evaluator_artifact_set_digest,
        &prover_key_material,
        &verifier_key_material,
    )
    .expect("build sample full-bootstrap proof-key pair");
    let prover_key = encode_bfv_full_bootstrap_proof_key_artifact_v1(
        &params,
        1,
        BfvFullBootstrapCircuitArtifactRoleV1::ProverKey,
        &prover_key,
    )
    .expect("encode sample full-bootstrap prover-key artifact");
    let verifier_key = encode_bfv_full_bootstrap_proof_key_artifact_v1(
        &params,
        1,
        BfvFullBootstrapCircuitArtifactRoleV1::VerifierKey,
        &verifier_key,
    )
    .expect("encode sample full-bootstrap verifier-key artifact");
    BfvFullBootstrapCircuitArtifactBundleV1 {
        coefficient_to_slot_key,
        slot_to_coefficient_key,
        blind_rotation_key: blind_rotation_key_artifact,
        sample_extraction_key,
        accumulator: accumulator_artifact,
        proof_public_input_schema,
        arithmetic_air_constraint_system,
        prover_key,
        verifier_key,
    }
}
fn replace_fhe_full_bootstrap_execution_open_verify_envelope(
    proof: &mut SoracloudFheFullBootstrapExecutionProofV1,
    envelope: &OpenVerifyEnvelope,
) {
    proof.proof.proof.bytes = norito::encode_canonical(envelope)
        .expect("encode canonical FHE full-bootstrap execution OpenVerifyEnvelope");
    proof.proof.envelope_hash = Some(<[u8; 32]>::from(Hash::new(&proof.proof.proof.bytes)));
}
fn encode_alternate_norito_layout<T: norito::NoritoSerialize>(value: &T) -> Vec<u8> {
    let alternate_flags =
        norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
    let alternate = {
        let _guard = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        norito::to_bytes(value).expect("encode alternate-layout Norito frame")
    };
    let canonical = norito::encode_canonical(value).expect("encode canonical comparison frame");
    assert_ne!(
        alternate, canonical,
        "adversarial fixture must use a distinct Norito layout"
    );
    alternate
}
fn alternate_open_verify_layouts(proof_bytes: &[u8]) -> [Vec<u8>; 2] {
    let envelope = norito::decode_canonical::<OpenVerifyEnvelope>(proof_bytes)
        .expect("decode canonical sample OpenVerifyEnvelope");
    let alternate_outer = encode_alternate_norito_layout(&envelope);
    let open_proof = norito::decode_canonical::<StarkFriOpenProofV1>(&envelope.proof_bytes)
        .expect("decode canonical sample STARK wrapper");
    let mut canonical_outer_with_alternate_inner = envelope;
    canonical_outer_with_alternate_inner.proof_bytes = encode_alternate_norito_layout(&open_proof);
    let alternate_inner = norito::encode_canonical(&canonical_outer_with_alternate_inner)
        .expect("encode canonical outer envelope with alternate-layout STARK wrapper");
    [alternate_outer, alternate_inner]
}
fn assert_alternate_open_verify_layouts_rejected<T>(
    sample: T,
    attachment: impl for<'a> Fn(&'a mut T) -> &'a mut ProofAttachment + Copy,
    validate: impl Fn(&T) -> Result<(), SoracloudManifestError>,
    proof_family: &str,
) where
    T: Clone,
{
    let canonical_bytes = {
        let mut sample = sample.clone();
        attachment(&mut sample).proof.bytes.clone()
    };
    for (layout, bytes) in ["outer", "inner"]
        .into_iter()
        .zip(alternate_open_verify_layouts(&canonical_bytes))
    {
        let mut candidate = sample.clone();
        let proof = attachment(&mut candidate);
        proof.proof.bytes = bytes;
        proof.envelope_hash = Some(<[u8; 32]>::from(Hash::new(&proof.proof.bytes)));
        let err = validate(&candidate)
            .expect_err("alternate-layout proof frame must be rejected before verifier execution");
        assert!(
            matches!(
                err,
                SoracloudManifestError::InvalidField {
                    field: "proof.proof.bytes",
                    ..
                }
            ),
            "{proof_family} {layout} alternate layout returned the wrong field: {err}"
        );
        assert!(
            err.to_string().contains("non-canonical"),
            "{proof_family} {layout} alternate layout returned the wrong reason: {err}"
        );
    }
}
#[test]
fn fhe_proof_admission_rejects_alternate_outer_and_nested_norito_layouts() {
    assert_alternate_open_verify_layouts_rejected(
        sample_fhe_input_admission_proof(),
        |proof: &mut SoracloudFheInputAdmissionProofV1| &mut proof.proof,
        SoracloudFheInputAdmissionProofV1::validate,
        "input admission",
    );
    assert_alternate_open_verify_layouts_rejected(
        sample_fhe_public_key_proof(),
        |proof: &mut SoracloudFhePublicKeyProofV1| &mut proof.proof,
        SoracloudFhePublicKeyProofV1::validate,
        "public key",
    );
    assert_alternate_open_verify_layouts_rejected(
        sample_fhe_bootstrap_key_proof(),
        |proof: &mut SoracloudFheBootstrapKeyProofV1| &mut proof.proof,
        SoracloudFheBootstrapKeyProofV1::validate,
        "bootstrap key",
    );
    assert_alternate_open_verify_layouts_rejected(
        sample_fhe_full_bootstrap_execution_proof(),
        |proof: &mut SoracloudFheFullBootstrapExecutionProofV1| &mut proof.proof,
        SoracloudFheFullBootstrapExecutionProofV1::validate,
        "full-bootstrap execution",
    );
}
fn zero_prehash_statement_hash() -> Hash {
    Hash::prehashed([0; Hash::LENGTH])
}
fn open_verify_envelope_with_statement(
    proof_bytes: &[u8],
    statement_hash: Hash,
) -> OpenVerifyEnvelope {
    let mut envelope = norito::decode_canonical::<OpenVerifyEnvelope>(proof_bytes)
        .expect("decode sample OpenVerifyEnvelope");
    let mut open_proof =
        norito::decode_canonical::<StarkFriOpenProofV1>(envelope.proof_bytes.as_slice())
            .expect("decode sample STARK public-input wrapper");
    open_proof.public_inputs = vec![vec![<[u8; Hash::LENGTH]>::from(statement_hash)]];
    envelope.proof_bytes =
        norito::encode_canonical(&open_proof).expect("encode canonical rewritten STARK wrapper");
    envelope
}
fn open_verify_envelope_with_native_envelope_bytes(
    proof_bytes: &[u8],
    native_envelope_bytes: Vec<u8>,
) -> OpenVerifyEnvelope {
    let mut envelope = norito::decode_canonical::<OpenVerifyEnvelope>(proof_bytes)
        .expect("decode sample OpenVerifyEnvelope");
    let mut open_proof =
        norito::decode_canonical::<StarkFriOpenProofV1>(envelope.proof_bytes.as_slice())
            .expect("decode sample STARK public-input wrapper");
    open_proof.envelope_bytes = native_envelope_bytes;
    envelope.proof_bytes =
        norito::encode_canonical(&open_proof).expect("encode canonical rewritten STARK wrapper");
    envelope
}
fn assert_zero_statement_hash_error(err: &SoracloudManifestError) {
    let err_text = err.to_string();
    assert!(
        matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "statement_hash",
                ..
            }
        ),
        "unexpected error: {err_text}"
    );
    assert!(
        err_text.contains("zero prehash sentinel"),
        "unexpected error: {err_text}"
    );
}
fn assert_native_envelope_error(err: &SoracloudManifestError, needle: &str) {
    let err_text = err.to_string();
    assert!(
        matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ),
        "unexpected error: {err_text}"
    );
    assert!(
        err_text.contains(needle),
        "unexpected native-envelope error: {err_text}"
    );
}
fn assert_zero_prehash_digest_error(err: &SoracloudManifestError, expected_field: &'static str) {
    let err_text = err.to_string();
    assert!(
        matches!(
            err,
            SoracloudManifestError::InvalidField { field, .. } if *field == expected_field
        ),
        "expected `{expected_field}` invalid-field error, got {err_text}"
    );
    assert!(
        err_text.contains("zero prehash sentinel"),
        "unexpected error: {err_text}"
    );
}
#[cfg(feature = "json")]
fn assert_soracloud_proof_key_commitment_domains(
    schema_value: &Value,
    pointer: &str,
    context: &str,
    material_domain: &str,
    pair_domain: &str,
) {
    let domains = schema_value
        .pointer(pointer)
        .and_then(Value::as_object)
        .unwrap_or_else(|| {
            panic!("{context} must carry proof-key commitment domains at `{pointer}`")
        });
    for (field, expected) in [("material", material_domain), ("pair", pair_domain)] {
        assert_eq!(
            domains.get(field).and_then(Value::as_str),
            Some(expected),
            "{context} proof-key commitment-domain field `{field}` drifted"
        );
    }
    assert_eq!(
        domains
            .get("separates_material_and_pair")
            .and_then(Value::as_bool),
        Some(true),
        "{context} must advertise material/pair commitment-domain separation"
    );
    assert_ne!(
        domains.get("material").and_then(Value::as_str),
        domains.get("pair").and_then(Value::as_str),
        "{context} proof-key material and pair commitment domains must be distinct"
    );
}
#[cfg(feature = "json")]
fn assert_soracloud_artifact_digest_domains(
    schema_value: &Value,
    pointer: &str,
    context: &str,
    circuit_material_domain: &str,
    evaluator_artifact_set_domain: &str,
    circuit_artifact_bundle_domain: &str,
) {
    let domains = schema_value
        .pointer(pointer)
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("{context} must carry artifact digest domains at `{pointer}`"));
    for (field, expected) in [
        ("circuit_material", circuit_material_domain),
        ("evaluator_artifact_set", evaluator_artifact_set_domain),
        ("circuit_artifact_bundle", circuit_artifact_bundle_domain),
    ] {
        assert_eq!(
            domains.get(field).and_then(Value::as_str),
            Some(expected),
            "{context} artifact digest-domain field `{field}` drifted"
        );
    }
    assert_eq!(
        domains
            .get("separates_material_set_and_bundle")
            .and_then(Value::as_bool),
        Some(true),
        "{context} must advertise artifact digest-domain separation"
    );
    assert_ne!(
        domains.get("circuit_material").and_then(Value::as_str),
        domains
            .get("evaluator_artifact_set")
            .and_then(Value::as_str),
        "{context} circuit-material and evaluator-artifact-set domains must be distinct"
    );
    assert_ne!(
        domains
            .get("evaluator_artifact_set")
            .and_then(Value::as_str),
        domains
            .get("circuit_artifact_bundle")
            .and_then(Value::as_str),
        "{context} evaluator-artifact-set and circuit-artifact-bundle domains must be distinct"
    );
}
#[cfg(feature = "json")]
fn assert_schema_object<'a>(schema_value: &'a Value, pointer: &str, context: &str) -> &'a Value {
    schema_value
        .pointer(pointer)
        .filter(|value| value.as_object().is_some())
        .unwrap_or_else(|| panic!("{context} must carry object at `{pointer}`"))
}
#[cfg(feature = "json")]
fn assert_schema_string_field(section: &Value, field: &str, expected: &str, context: &str) {
    assert_eq!(
        section.get(field).and_then(Value::as_str),
        Some(expected),
        "{context} schema field `{field}` drifted"
    );
}
#[cfg(feature = "json")]
fn assert_schema_u64_field(section: &Value, field: &str, expected: u64, context: &str) {
    assert_eq!(
        section.get(field).and_then(Value::as_u64),
        Some(expected),
        "{context} schema field `{field}` drifted"
    );
}
#[cfg(feature = "json")]
fn assert_schema_bool_field(section: &Value, field: &str, expected: bool, context: &str) {
    assert_eq!(
        section.get(field).and_then(Value::as_bool),
        Some(expected),
        "{context} schema field `{field}` drifted"
    );
}
#[cfg(feature = "json")]
#[allow(clippy::too_many_lines)]
fn assert_soracloud_execution_schema_sections(schema_value: &Value, context: &str) {
    let witness_digest_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_EXECUTION_WITNESS_DIGEST_DOMAIN,
    )
    .expect("execution witness digest domain is valid UTF-8");
    let proof_input_material_digest_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_EXECUTION_PROOF_INPUT_MATERIAL_DIGEST_DOMAIN,
    )
    .expect("execution proof input material digest domain is valid UTF-8");
    let prover_input_material_digest_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_EXECUTION_PROVER_INPUT_MATERIAL_DIGEST_DOMAIN,
    )
    .expect("execution prover input material digest domain is valid UTF-8");
    let air_evaluation_material_digest_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_AIR_EVALUATION_MATERIAL_DIGEST_DOMAIN,
    )
    .expect("arithmetic AIR evaluation material digest domain is valid UTF-8");
    let public_opening_material_digest_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_PUBLIC_OPENING_MATERIAL_DIGEST_DOMAIN,
        )
        .expect("arithmetic trace public-opening material digest domain is valid UTF-8");
    let trace_material_digest_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_MATERIAL_DIGEST_DOMAIN,
    )
    .expect("arithmetic trace material digest domain is valid UTF-8");
    let air_constraint_system_digest_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_AIR_CONSTRAINT_SYSTEM_DIGEST_DOMAIN,
    )
    .expect("arithmetic AIR constraint-system digest domain is valid UTF-8");
    let composition_challenge_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_AIR_COMPOSITION_CHALLENGE_DOMAIN,
    )
    .expect("arithmetic AIR composition-challenge domain is valid UTF-8");
    let witness = assert_schema_object(
        schema_value,
        "/execution_witness_layout",
        "execution witness layout",
    );
    assert_schema_string_field(witness, "digest_domain", witness_digest_domain, context);
    assert_schema_u64_field(
        witness,
        "material_version",
        u64::from(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_EXECUTION_WITNESS_DIGEST_MATERIAL_VERSION_V1,
        ),
        context,
    );
    assert_schema_u64_field(
            witness,
            "material_field_count",
            u64::from(
                iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_EXECUTION_WITNESS_DIGEST_MATERIAL_FIELD_COUNT_V1,
            ),
            context,
        );
    assert_schema_u64_field(
        witness,
        "trace_field_count",
        u64::from(iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_EXECUTION_PREFIX_TRACE_FIELD_COUNT_V1),
        context,
    );
    assert_schema_u64_field(
        witness,
        "trace_bounds_field_count",
        u64::from(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_EXECUTION_PREFIX_TRACE_BOUNDS_FIELD_COUNT_V1,
        ),
        context,
    );
    for field in [
        "binds_galois_key_set_digest",
        "binds_trace",
        "binds_trace_bounds",
    ] {
        assert_schema_bool_field(witness, field, true, context);
    }
    let trace_profile = assert_schema_object(
        schema_value,
        "/arithmetic_trace_profile",
        "arithmetic trace profile",
    );
    for (field, expected) in [
        (
            "version",
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_PROFILE_VERSION_V1,
        ),
        (
            "field_count",
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_PROFILE_FIELD_COUNT_V1,
        ),
        (
            "material_version",
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_MATERIAL_VERSION_V1,
        ),
        (
            "material_field_count",
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_MATERIAL_FIELD_COUNT_V1,
        ),
        (
            "row_width",
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_ROW_WIDTH_V1,
        ),
        (
            "private_row_count",
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_PRIVATE_ROW_COUNT_V1,
        ),
    ] {
        assert_schema_u64_field(trace_profile, field, u64::from(expected), context);
    }
    assert_schema_u64_field(
        trace_profile,
        "private_row_kind",
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_PRIVATE_ROW_KIND_V1,
        context,
    );
    assert_schema_u64_field(
        trace_profile,
        "public_row_kind",
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_PUBLIC_ROW_KIND_V1,
        context,
    );
    assert_schema_bool_field(
            trace_profile,
            "forbids_unmasked_private_row_openings",
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_FORBIDS_UNMASKED_PRIVATE_ROW_OPENINGS_V1,
            context,
        );
    assert_schema_bool_field(
        trace_profile,
        "forbids_duplicate_openings",
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_FORBIDS_DUPLICATE_OPENINGS_V1,
        context,
    );
    let air_contract = assert_schema_object(
        schema_value,
        "/arithmetic_air_contract",
        "arithmetic AIR contract",
    );
    assert_schema_u64_field(
            air_contract,
            "version",
            u64::from(
                iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_AIR_CONSTRAINT_SYSTEM_MATERIAL_VERSION_V1,
            ),
            context,
        );
    assert_schema_u64_field(
            air_contract,
            "field_count",
            u64::from(
                iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_AIR_CONSTRAINT_SYSTEM_MATERIAL_FIELD_COUNT_V1,
            ),
            context,
        );
    assert_schema_string_field(
        air_contract,
        "composition_challenge_domain",
        composition_challenge_domain,
        context,
    );
    assert_schema_u64_field(
            air_contract,
            "composition_challenge_digest_bytes",
            u64::from(
                iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_AIR_COMPOSITION_CHALLENGE_DIGEST_BYTES_V1,
            ),
            context,
        );
    for field in [
        "binds_constraint_system_digest",
        "enforces_goldilocks_field_canonicality",
        "enforces_row_kind_partition",
        "enforces_active_rows_match_witness_material",
        "enforces_full_bootstrap_arithmetic_constraints",
        "enforces_public_padding_rows",
        "enforces_statement_hash_nonzero",
        "enforces_trace_output_matches_claim",
        "enforces_trace_bound_matches_claim",
        "enforces_no_unmasked_private_row_openings",
        "enforces_duplicate_free_openings",
        "derives_opening_schedule_from_statement_hash",
        "derives_opening_schedule_from_trace_material_digest",
        "bounds_opening_schedule_rejection_sampling",
        "validates_transcript_public_padding_openings",
        "binds_composition_challenges_to_statement_hash",
        "binds_composition_challenges_to_trace_material_digest",
        "binds_composition_challenges_to_row_index",
        "binds_composition_challenges_to_column_index",
        "maps_zero_composition_challenge_to_one",
    ] {
        assert_schema_bool_field(air_contract, field, true, context);
    }
    let native_air_envelope =
        assert_schema_object(schema_value, "/native_air_envelope", "native AIR envelope");
    for field in [
        "validates_stark_parameter_profile",
        "binds_domain_tag_to_statement_hash",
        "validates_circuit_id",
        "validates_trace_width",
        "validates_query_opening_count",
        "requires_public_padding_context",
        "requires_verifier_owned_trace_material_digest",
        "rejects_auxiliary_composition_value_commitments",
        "binds_public_digest_to_statement_hash",
        "validates_merkle_path_shape",
        "validates_merkle_path_roots",
        "validates_fri_query_chain",
        "binds_first_fri_values_to_opened_air_values",
        "binds_fri_queries_to_air_commitment_roots",
        "binds_trace_root_to_governed_arithmetic_trace",
        "binds_composition_root_to_governed_air_evaluation",
        "binds_opened_rows_to_governed_arithmetic_trace",
        "binds_opened_composition_values_to_governed_air_evaluation",
        "validates_public_padding_openings",
        "requires_zero_public_padding_composition_values",
        "requires_canonical_base_transcript_label",
        "rejects_suffixed_transcript_label_aliases",
        "rejects_blank_native_envelope_bytes",
        "rejects_placeholder_native_envelope_text",
    ] {
        assert_schema_bool_field(native_air_envelope, field, true, context);
    }
    let artifact_bundle = assert_schema_object(schema_value, "/artifact_bundle", "artifact bundle");
    assert_schema_u64_field(
            artifact_bundle,
            "artifact_digest_count",
            u64::from(
                iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_CIRCUIT_ARTIFACT_BUNDLE_DIGEST_MATERIAL_ARTIFACT_DIGEST_COUNT_V1,
            ),
            context,
        );
    for field in [
        "binds_arithmetic_air_constraint_system_artifact",
        "validates_arithmetic_air_constraint_system_material",
    ] {
        assert_schema_bool_field(artifact_bundle, field, true, context);
    }
    let release_prover_input = assert_schema_object(
        schema_value,
        "/release_prover_input",
        "release prover input",
    );
    assert_schema_u64_field(
        release_prover_input,
        "proof_input_material_version",
        u64::from(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_EXECUTION_PROOF_INPUT_MATERIAL_VERSION_V1,
        ),
        context,
    );
    assert_schema_u64_field(
        release_prover_input,
        "proof_input_material_field_count",
        u64::from(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_EXECUTION_PROOF_INPUT_MATERIAL_FIELD_COUNT_V1,
        ),
        context,
    );
    assert_schema_string_field(
        release_prover_input,
        "proof_input_material_digest_domain",
        proof_input_material_digest_domain,
        context,
    );
    let release_prover_digest_domains = release_prover_input
        .get("release_prover_digest_domains")
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("{context} must carry release-prover digest domains"));
    for (field, expected) in [
        ("proof_input_material", proof_input_material_digest_domain),
        ("prover_input_material", prover_input_material_digest_domain),
        (
            "air_evaluation_material",
            air_evaluation_material_digest_domain,
        ),
        (
            "public_opening_material",
            public_opening_material_digest_domain,
        ),
        ("arithmetic_trace_material", trace_material_digest_domain),
        (
            "arithmetic_air_constraint_system",
            air_constraint_system_digest_domain,
        ),
    ] {
        assert_eq!(
            release_prover_digest_domains
                .get(field)
                .and_then(Value::as_str),
            Some(expected),
            "{context} release-prover digest-domain field `{field}` drifted"
        );
    }
    assert_eq!(
        release_prover_digest_domains
            .get("separates_release_prover_material_domains")
            .and_then(Value::as_bool),
        Some(true),
        "{context} must advertise release-prover material digest-domain separation"
    );
    assert_ne!(
        release_prover_digest_domains
            .get("proof_input_material")
            .and_then(Value::as_str),
        release_prover_digest_domains
            .get("prover_input_material")
            .and_then(Value::as_str),
        "{context} proof-input and prover-input material domains must be distinct"
    );
    assert_ne!(
        release_prover_digest_domains
            .get("prover_input_material")
            .and_then(Value::as_str),
        release_prover_digest_domains
            .get("arithmetic_trace_material")
            .and_then(Value::as_str),
        "{context} prover-input and trace-material domains must be distinct"
    );
    assert_schema_u64_field(
        release_prover_input,
        "prover_input_material_version",
        u64::from(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_EXECUTION_PROVER_INPUT_MATERIAL_VERSION_V1,
        ),
        context,
    );
    assert_schema_u64_field(
            release_prover_input,
            "prover_input_material_field_count",
            u64::from(
                iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_EXECUTION_PROVER_INPUT_MATERIAL_FIELD_COUNT_V1,
            ),
            context,
        );
    assert_schema_u64_field(
        release_prover_input,
        "air_evaluation_material_version",
        u64::from(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_AIR_EVALUATION_MATERIAL_VERSION_V1,
        ),
        context,
    );
    assert_schema_u64_field(
            release_prover_input,
            "air_evaluation_material_field_count",
            u64::from(
                iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_AIR_EVALUATION_MATERIAL_FIELD_COUNT_V1,
            ),
            context,
        );
    assert_schema_u64_field(
            release_prover_input,
            "public_opening_material_version",
            u64::from(
                iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_PUBLIC_OPENING_MATERIAL_VERSION_V1,
            ),
            context,
        );
    assert_schema_u64_field(
            release_prover_input,
            "public_opening_material_field_count",
            u64::from(
                iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_PUBLIC_OPENING_MATERIAL_FIELD_COUNT_V1,
            ),
            context,
        );
    for field in [
        "hashes_proof_input_material",
        "binds_release_prover_arithmetic_air_constraint_system_digest",
        "binds_release_prover_arithmetic_air_constraint_system_artifact_digest",
        "binds_release_prover_arithmetic_air_evaluation_material_digest",
        "binds_arithmetic_air_evaluation_trace_material_digest",
        "requires_zero_arithmetic_air_composition_values",
        "binds_arithmetic_trace_material_digest",
        "binds_trace_proof_input_consistency",
        "binds_generated_proof_key_pair",
        "binds_release_prover_verifier_key",
        "validates_artifact_bound_prover_input",
        "rejects_stale_galois_key_set_replay",
        "rejects_stale_proof_key_artifacts",
        "derives_opening_schedule_from_statement_hash",
        "derives_opening_schedule_from_trace_material_digest",
        "bounds_opening_schedule_rejection_sampling",
        "validates_transcript_public_padding_openings",
        "validates_transcript_public_opening_material",
        "requires_verifier_owned_trace_material_digest",
        "requires_canonical_base_transcript_label",
        "rejects_suffixed_transcript_label_aliases",
    ] {
        assert_schema_bool_field(release_prover_input, field, true, context);
    }
}
#[cfg(feature = "json")]
#[allow(clippy::too_many_lines)]
fn assert_soracloud_release_audit_schema_sections(schema_value: &Value, context: &str) {
    let evidence_digest_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_EVIDENCE_DIGEST_DOMAIN,
    )
    .expect("release-audit evidence digest domain is valid UTF-8");
    let record_digest_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_RECORD_DIGEST_DOMAIN,
    )
    .expect("release-audit record digest domain is valid UTF-8");
    let manifest_digest_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_MANIFEST_DIGEST_DOMAIN,
    )
    .expect("release-audit manifest digest domain is valid UTF-8");
    let package_digest_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PACKAGE_DIGEST_DOMAIN,
    )
    .expect("release-audit package digest domain is valid UTF-8");
    let circuit_material_digest_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_CIRCUIT_MATERIAL_DIGEST_DOMAIN,
    )
    .expect("full-bootstrap circuit-material digest domain is valid UTF-8");
    let evaluator_artifact_set_digest_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_EVALUATOR_ARTIFACT_SET_DIGEST_DOMAIN,
    )
    .expect("full-bootstrap evaluator-artifact-set digest domain is valid UTF-8");
    let circuit_artifact_bundle_digest_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_CIRCUIT_ARTIFACT_BUNDLE_DIGEST_DOMAIN,
    )
    .expect("full-bootstrap circuit-artifact-bundle digest domain is valid UTF-8");
    let proof_input_material_digest_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_EXECUTION_PROOF_INPUT_MATERIAL_DIGEST_DOMAIN,
    )
    .expect("execution proof input material digest domain is valid UTF-8");
    let prover_input_material_digest_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_EXECUTION_PROVER_INPUT_MATERIAL_DIGEST_DOMAIN,
    )
    .expect("execution prover input material digest domain is valid UTF-8");
    let air_evaluation_material_digest_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_AIR_EVALUATION_MATERIAL_DIGEST_DOMAIN,
    )
    .expect("arithmetic AIR evaluation material digest domain is valid UTF-8");
    let public_opening_material_digest_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_PUBLIC_OPENING_MATERIAL_DIGEST_DOMAIN,
        )
        .expect("arithmetic trace public-opening material digest domain is valid UTF-8");
    let trace_material_digest_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_MATERIAL_DIGEST_DOMAIN,
    )
    .expect("arithmetic trace material digest domain is valid UTF-8");
    let air_constraint_system_digest_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_AIR_CONSTRAINT_SYSTEM_DIGEST_DOMAIN,
    )
    .expect("arithmetic AIR constraint-system digest domain is valid UTF-8");
    let proof_key_material_commitment_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_PROOF_KEY_MATERIAL_COMMITMENT_DOMAIN,
    )
    .expect("proof-key material commitment domain is valid UTF-8");
    let proof_key_pair_commitment_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_PROOF_KEY_PAIR_COMMITMENT_DOMAIN,
    )
    .expect("proof-key pair commitment domain is valid UTF-8");
    let evidence = assert_schema_object(
        schema_value,
        "/release_audit_evidence",
        "release-audit evidence",
    );
    assert_schema_u64_field(
        evidence,
        "version",
        u64::from(iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_EVIDENCE_VERSION_V1),
        context,
    );
    assert_schema_u64_field(
        evidence,
        "field_count",
        u64::from(iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_EVIDENCE_FIELD_COUNT_V1),
        context,
    );
    assert_schema_string_field(evidence, "digest_domain", evidence_digest_domain, context);
    assert_schema_u64_field(
        evidence,
        "proof_profile_field_count",
        u64::from(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PROOF_PROFILE_FIELD_COUNT_V1,
        ),
        context,
    );
    assert_schema_u64_field(
            evidence,
            "proof_profile_public_opening_material_version",
            u64::from(
                iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_PUBLIC_OPENING_MATERIAL_VERSION_V1,
            ),
            context,
        );
    assert_schema_u64_field(
            evidence,
            "proof_profile_public_opening_material_field_count",
            u64::from(
                iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_PUBLIC_OPENING_MATERIAL_FIELD_COUNT_V1,
            ),
            context,
        );
    let proof_profile_release_prover_digest_domains = evidence
        .get("proof_profile_release_prover_digest_domains")
        .and_then(Value::as_object)
        .unwrap_or_else(|| {
            panic!("{context} must carry proof-profile release-prover digest domains")
        });
    for (field, expected) in [
        ("proof_input_material", proof_input_material_digest_domain),
        ("prover_input_material", prover_input_material_digest_domain),
        (
            "air_evaluation_material",
            air_evaluation_material_digest_domain,
        ),
        (
            "public_opening_material",
            public_opening_material_digest_domain,
        ),
        ("arithmetic_trace_material", trace_material_digest_domain),
        (
            "arithmetic_air_constraint_system",
            air_constraint_system_digest_domain,
        ),
    ] {
        assert_eq!(
            proof_profile_release_prover_digest_domains
                .get(field)
                .and_then(Value::as_str),
            Some(expected),
            "{context} proof-profile release-prover digest-domain field `{field}` drifted"
        );
    }
    assert_eq!(
        proof_profile_release_prover_digest_domains
            .get("separates_release_prover_material_domains")
            .and_then(Value::as_bool),
        Some(true),
        "{context} proof-profile must advertise release-prover material digest-domain separation"
    );
    assert_schema_bool_field(
        evidence,
        "proof_profile_validates_transcript_public_opening_material",
        true,
        context,
    );
    assert_schema_bool_field(
        evidence,
        "proof_profile_requires_verifier_owned_trace_material_digest",
        true,
        context,
    );
    let proof_profile_proof_key_commitment_domains = evidence
        .get("proof_profile_proof_key_commitment_domains")
        .and_then(Value::as_object)
        .unwrap_or_else(|| {
            panic!("{context} must carry proof-profile proof-key commitment domains")
        });
    assert_eq!(
        proof_profile_proof_key_commitment_domains
            .get("material")
            .and_then(Value::as_str),
        Some(proof_key_material_commitment_domain),
        "{context} proof-profile proof-key material commitment domain drifted"
    );
    assert_eq!(
        proof_profile_proof_key_commitment_domains
            .get("pair")
            .and_then(Value::as_str),
        Some(proof_key_pair_commitment_domain),
        "{context} proof-profile proof-key pair commitment domain drifted"
    );
    assert_eq!(
        proof_profile_proof_key_commitment_domains
            .get("separates_material_and_pair")
            .and_then(Value::as_bool),
        Some(true),
        "{context} proof-profile must advertise proof-key material/pair commitment-domain separation"
    );
    assert_schema_u64_field(
        evidence,
        "key_evidence_field_count",
        u64::from(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_KEY_EVIDENCE_FIELD_COUNT_V1,
        ),
        context,
    );
    assert_soracloud_artifact_digest_domains(
        schema_value,
        "/release_audit_evidence/artifact_digest_domains",
        context,
        circuit_material_digest_domain,
        evaluator_artifact_set_digest_domain,
        circuit_artifact_bundle_digest_domain,
    );
    let signoff = assert_schema_object(
        schema_value,
        "/release_audit_signoff",
        "release-audit signoff",
    );
    assert_schema_u64_field(
        signoff,
        "version",
        u64::from(iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_SIGNOFF_VERSION_V1),
        context,
    );
    assert_schema_u64_field(
        signoff,
        "field_count",
        u64::from(iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_SIGNOFF_FIELD_COUNT_V1),
        context,
    );
    assert_schema_u64_field(
        signoff,
        "payload_version",
        u64::from(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_SIGNOFF_PAYLOAD_VERSION_V1,
        ),
        context,
    );
    assert_schema_u64_field(
        signoff,
        "payload_field_count",
        u64::from(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_SIGNOFF_PAYLOAD_FIELD_COUNT_V1,
        ),
        context,
    );
    assert_schema_u64_field(
        signoff,
        "reviewer_id_max_bytes",
        u64::try_from(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REVIEWER_ID_MAX_BYTES,
        )
        .expect("release-audit reviewer id max bytes fits u64"),
        context,
    );
    for field in [
        "binds_release_audit_evidence_digest",
        "binds_generated_circuit_body_digest",
        "binds_centered_scale_round_source_chain_digest",
        "binds_prover_native_payload_digest",
        "binds_verifier_native_payload_digest",
        "binds_external_audit_report_digest",
        "binds_evidence_archive_digest",
    ] {
        assert_schema_bool_field(signoff, field, true, context);
    }
    let record = assert_schema_object(
        schema_value,
        "/release_audit_record",
        "release-audit record",
    );
    assert_schema_u64_field(
        record,
        "version",
        u64::from(iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_RECORD_VERSION_V1),
        context,
    );
    assert_schema_u64_field(
        record,
        "field_count",
        u64::from(iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_RECORD_FIELD_COUNT_V1),
        context,
    );
    assert_schema_string_field(record, "digest_domain", record_digest_domain, context);
    let manifest = assert_schema_object(
        schema_value,
        "/release_audit_manifest",
        "release-audit manifest",
    );
    assert_schema_u64_field(
        manifest,
        "version",
        u64::from(iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_MANIFEST_VERSION_V1),
        context,
    );
    assert_schema_u64_field(
        manifest,
        "field_count",
        u64::from(iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_MANIFEST_FIELD_COUNT_V1),
        context,
    );
    assert_schema_string_field(manifest, "digest_domain", manifest_digest_domain, context);
    assert_schema_string_field(
        manifest,
        "scope",
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_SCOPE_V1,
        context,
    );
    for field in [
        "binds_release_audit_record_digest",
        "binds_release_audit_evidence_digest",
        "binds_centered_scale_round_source_chain_digest",
        "binds_artifact_bundle_digest",
        "binds_evaluator_artifact_set_digest",
        "binds_proof_key_pair_commitment",
        "binds_prover_native_payload_digest",
        "binds_verifier_native_payload_digest",
        "binds_native_circuit_fingerprint",
        "binds_generated_circuit_body_digest",
        "binds_external_audit_report_digest",
        "binds_evidence_archive_digest",
    ] {
        assert_schema_bool_field(manifest, field, true, context);
    }
    let package = assert_schema_object(
        schema_value,
        "/release_audit_package",
        "release-audit package",
    );
    assert_schema_u64_field(
        package,
        "version",
        u64::from(iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PACKAGE_VERSION_V1),
        context,
    );
    assert_schema_u64_field(
        package,
        "field_count",
        u64::from(iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PACKAGE_FIELD_COUNT_V1),
        context,
    );
    assert_schema_string_field(package, "digest_domain", package_digest_domain, context);
    for (field, expected) in [
        (
            "audit_report_max_bytes",
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_MAX_BYTES,
        ),
        (
            "audit_archive_max_bytes",
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_MAX_BYTES,
        ),
        (
            "audit_report_body_min_bytes",
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_BODY_MIN_BYTES,
        ),
        (
            "audit_archive_body_min_bytes",
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_BODY_MIN_BYTES,
        ),
    ] {
        assert_schema_u64_field(
            package,
            field,
            u64::try_from(expected).expect("release-audit byte bound fits u64"),
            context,
        );
    }
    for field in [
        "requires_evidence_archive_body_prover_native_payload_digest",
        "requires_evidence_archive_body_verifier_native_payload_digest",
    ] {
        assert_schema_bool_field(package, field, true, context);
    }
}
