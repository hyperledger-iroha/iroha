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
fn sample_bls_peer_id(seed: u8) -> String {
    let keypair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
        .expect("fixture seed derives BLS peer keypair");
    PeerId::from(keypair.public_key().clone()).to_string()
}
fn sample_inrou_placement_targets(count: u16) -> BTreeSet<SoraInrouPlacementTargetV1> {
    (0..count)
        .map(|index| {
            let index = u8::try_from(index).expect("fixture target index fits u8");
            SoraInrouPlacementTargetV1 {
                validator_account_id: sample_account_id(0xA0_u8.wrapping_add(index)),
                peer_id: sample_bls_peer_id(0xC0_u8.wrapping_add(index)),
            }
        })
        .collect()
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

fn assert_app_infra_json_closed<T>(value: &T, label: &str)
where
    T: norito::json::JsonSerialize + norito::json::JsonDeserialize + core::fmt::Debug,
{
    let serialize_message = format!("serialize canonical {label}");
    let canonical_message = format!("canonical {label} must decode");
    let object_message = format!("{label} JSON object");
    let unknown_message = format!("{label} must reject unknown fields");
    let mut value = norito::json::to_value(value).expect(&serialize_message);
    norito::json::from_value::<T>(value.clone()).expect(&canonical_message);
    value
        .as_object_mut()
        .expect(&object_message)
        .insert("retired_v0".to_owned(), norito::json::Value::from(true));
    let error = norito::json::from_value::<T>(value).expect_err(&unknown_message);
    assert!(
        matches!(
            error,
            norito::json::Error::UnknownField { ref field } if field == "retired_v0"
        ),
        "{label} reported the wrong unknown-field error: {error}"
    );
}

fn assert_app_infra_required_nullable<T>(value: &T, field: &str, label: &str)
where
    T: norito::json::JsonSerialize + norito::json::JsonDeserialize + core::fmt::Debug,
{
    let serialize_message = format!("serialize canonical {label}");
    let object_message = format!("{label} JSON object");
    let omitted_message = format!("{label} must reject an omitted nullable key");
    let null_message = format!("{label} must accept an explicit null key");
    let canonical = norito::json::to_value(value).expect(&serialize_message);
    let mut missing = canonical.clone();
    assert!(
        missing
            .as_object_mut()
            .expect(&object_message)
            .remove(field)
            .is_some()
    );
    norito::json::from_value::<T>(missing).expect_err(&omitted_message);

    let mut explicit_null = canonical;
    explicit_null
        .as_object_mut()
        .expect(&object_message)
        .insert(field.to_owned(), norito::json::Value::Null);
    norito::json::from_value::<T>(explicit_null).expect(&null_message);
}

fn assert_app_infra_required_vector<T>(value: &T, field: &str, label: &str)
where
    T: norito::json::JsonSerialize + norito::json::JsonDeserialize + core::fmt::Debug,
{
    let serialize_message = format!("serialize canonical {label}");
    let object_message = format!("{label} JSON object");
    let omitted_message = format!("{label} must reject an omitted vector key");
    let null_message = format!("{label} must reject a null vector key");
    let canonical = norito::json::to_value(value).expect(&serialize_message);
    let mut missing = canonical.clone();
    assert!(
        missing
            .as_object_mut()
            .expect(&object_message)
            .remove(field)
            .is_some()
    );
    norito::json::from_value::<T>(missing).expect_err(&omitted_message);

    let mut null = canonical;
    null.as_object_mut()
        .expect(&object_message)
        .insert(field.to_owned(), norito::json::Value::Null);
    norito::json::from_value::<T>(null).expect_err(&null_message);
}

#[test]
fn app_infra_v1_json_graph_is_closed_and_requires_explicit_nullable_and_vector_keys() {
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

    assert_app_infra_json_closed::<SoraAppInfraActionV1>(
        &SoraAppInfraActionV1::Deploy,
        "app infra action",
    );
    assert_app_infra_json_closed::<SoraAppStaticSiteBindingV1>(
        &static_site.clone(),
        "app static-site binding",
    );
    assert_app_infra_json_closed::<SoraAppRouteProjectionV1>(
        &route.clone(),
        "app route projection",
    );
    assert_app_infra_json_closed::<SoraAppInfraServiceRefV1>(
        &service.clone(),
        "app service reference",
    );
    assert_app_infra_json_closed::<SoraAppInfraManifestV1>(&manifest.clone(), "app infra manifest");
    assert_app_infra_json_closed::<SoraAppInfraStateV1>(&state, "app infra state");
    assert_app_infra_json_closed::<SoraAppInfraAuditEventV1>(
        &audit.clone(),
        "app infra audit event",
    );

    for field in ["content_cid", "manifest_digest_hex", "api_base_path"] {
        assert_app_infra_required_nullable::<SoraAppStaticSiteBindingV1>(
            &static_site.clone(),
            field,
            "app static-site binding",
        );
    }
    for field in ["public_host", "internal_url"] {
        assert_app_infra_required_nullable::<SoraAppRouteProjectionV1>(
            &route.clone(),
            field,
            "app route projection",
        );
    }
    assert_app_infra_required_nullable::<SoraAppInfraServiceRefV1>(
        &service.clone(),
        "shard",
        "app service reference",
    );
    for field in ["routes", "lease_volumes"] {
        assert_app_infra_required_vector::<SoraAppInfraServiceRefV1>(
            &service.clone(),
            field,
            "app service reference",
        );
    }
    assert_app_infra_required_nullable::<SoraAppInfraManifestV1>(
        &manifest,
        "static_site",
        "app infra manifest",
    );
    assert_app_infra_required_nullable::<SoraAppInfraAuditEventV1>(
        &audit,
        "from_version",
        "app infra audit event",
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

fn assert_soracloud_required_keys<T>(
    value: &T,
    fields: &[&str],
    nullable_fields: &[&str],
    label: &str,
) where
    T: norito::json::JsonSerialize + norito::json::JsonDeserialize + core::fmt::Debug,
{
    let serialize_message = format!("serialize {label}");
    let canonical_message = format!("canonical {label} must decode");
    let object_message = format!("{label} JSON object");
    let omitted_message = format!("{label} must reject an omitted canonical key");
    let null_message = format!("{label} must accept an explicit nullable key");
    let canonical = norito::json::to_value(value).expect(&serialize_message);
    norito::json::from_value::<T>(canonical.clone()).expect(&canonical_message);
    for &field in fields {
        let mut missing = canonical.clone();
        assert!(
            missing
                .as_object_mut()
                .expect(&object_message)
                .remove(field)
                .is_some(),
            "canonical {label} must contain `{field}`"
        );
        norito::json::from_value::<T>(missing).expect_err(&omitted_message);
    }
    for &nullable in nullable_fields {
        let mut explicit_null = canonical.clone();
        explicit_null
            .as_object_mut()
            .expect(&object_message)
            .insert(nullable.to_owned(), norito::json::Value::Null);
        norito::json::from_value::<T>(explicit_null).expect(&null_message);
    }
}

fn assert_training_and_model_required_keys() {
    assert_soracloud_required_keys::<SoraTrainingJobRecordV1>(
        &sample_training_job_record(),
        &[
            "last_checkpoint_step",
            "latest_metrics_hash",
            "last_failure_reason",
        ],
        &[
            "last_checkpoint_step",
            "latest_metrics_hash",
            "last_failure_reason",
        ],
        "training-job record",
    );
    assert_soracloud_required_keys::<SoraTrainingJobAuditEventV1>(
        &sample_training_job_audit_event(),
        &[
            "last_checkpoint_step",
            "latest_metrics_hash",
            "last_failure_reason",
        ],
        &[
            "last_checkpoint_step",
            "latest_metrics_hash",
            "last_failure_reason",
        ],
        "training-job audit event",
    );
    assert_soracloud_required_keys::<SoraModelRegistryV1>(
        &sample_model_registry(),
        &["current_version"],
        &["current_version"],
        "model registry",
    );
    assert_soracloud_required_keys::<SoraUploadedModelBundleV1>(
        &sample_uploaded_model_bundle(),
        &["modalities"],
        &[],
        "uploaded-model bundle",
    );
    assert_soracloud_required_keys::<SoraModelWeightVersionRecordV1>(
        &sample_model_weight_version_record(),
        &[
            "parent_version",
            "training_job_id",
            "source_provenance",
            "promoted_sequence",
            "gate_report_hash",
            "promoted_by",
        ],
        &[
            "parent_version",
            "source_provenance",
            "promoted_sequence",
            "gate_report_hash",
            "promoted_by",
        ],
        "model-weight record",
    );
    assert_soracloud_required_keys::<SoraModelWeightAuditEventV1>(
        &sample_model_weight_audit_event(),
        &[
            "current_version",
            "parent_version",
            "gate_approved",
            "rollback_reason",
        ],
        &[
            "current_version",
            "parent_version",
            "gate_approved",
            "rollback_reason",
        ],
        "model-weight audit event",
    );
    assert_soracloud_required_keys::<SoraModelArtifactRecordV1>(
        &sample_model_artifact_record(),
        &[
            "training_job_id",
            "weight_version",
            "source_provenance",
            "consumed_by_version",
            "chunk_manifest_root",
        ],
        &[
            "weight_version",
            "source_provenance",
            "consumed_by_version",
            "chunk_manifest_root",
        ],
        "model-artifact record",
    );
    assert_soracloud_required_keys::<SoraModelArtifactAuditEventV1>(
        &sample_model_artifact_audit_event(),
        &["consumed_by_version"],
        &["consumed_by_version"],
        "model-artifact audit event",
    );
}

#[test]
fn canonical_deployment_and_hosting_json_graph_requires_explicit_keys() {
    assert_training_and_model_required_keys();
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
    assert_soracloud_required_keys::<SoraHfSharedLeaseQueuedWindowV1>(
        &queued_window,
        &["apartment_name"],
        &["apartment_name"],
        "HF queued lease window",
    );
    assert_soracloud_required_keys::<SoraHfSharedLeasePoolV1>(
        &sample_hf_shared_lease_pool(),
        &["queued_next_window"],
        &["queued_next_window"],
        "HF shared-lease pool",
    );
    assert_soracloud_required_keys::<SoraHfSharedLeaseMemberV1>(
        &sample_hf_shared_lease_member(),
        &["service_bindings", "apartment_bindings"],
        &[],
        "HF shared-lease member",
    );
    assert_soracloud_required_keys::<SoraHfSharedLeaseAuditEventV1>(
        &sample_hf_shared_lease_audit_event(),
        &["service_name", "apartment_name"],
        &["service_name", "apartment_name"],
        "HF shared-lease audit event",
    );
}

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
fn assert_retired_inrou_layouts_rejected<H: Encode, P: Encode, R: Encode>(
    retired_host: &H,
    retired_placement: &P,
    retired_runtime: &R,
) {
    let retired_host_bytes = H::encode(retired_host);
    assert!(
        SoraInrouHostCapabilityRecordV1::decode_all(&mut retired_host_bytes.as_slice()).is_err(),
        "host capability must reject the retired backend-selector Norito layout"
    );
    let retired_placement_bytes = P::encode(retired_placement);
    assert!(
        SoraInrouReplicaPlacementV1::decode_all(&mut retired_placement_bytes.as_slice()).is_err(),
        "replica placement must reject the retired backend-selector Norito layout"
    );
    let retired_runtime_bytes = R::encode(retired_runtime);
    assert!(
        SoraInrouReplicaRuntimeStateV1::decode_all(&mut retired_runtime_bytes.as_slice()).is_err(),
        "replica runtime state must reject the retired backend-selector Norito layout"
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

    assert_retired_inrou_layouts_rejected(&retired_host, &retired_placement, &retired_runtime);
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
        "stark/fri/poseidon-x7-goldilocks-6x64-v1".into(),
        norito::encode_canonical(&envelope)
            .expect("encode canonical FHE input admission OpenVerifyEnvelope"),
    );
    let mut attachment = ProofAttachment::new_ref(
        "stark/fri/poseidon-x7-goldilocks-6x64-v1".into(),
        proof,
        crate::proof::VerifyingKeyId::new(
            "stark/fri/poseidon-x7-goldilocks-6x64-v1",
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
        "stark/fri/poseidon-x7-goldilocks-6x64-v1".into(),
        norito::encode_canonical(&envelope)
            .expect("encode canonical FHE public-key OpenVerifyEnvelope"),
    );
    let mut attachment = ProofAttachment::new_ref(
        "stark/fri/poseidon-x7-goldilocks-6x64-v1".into(),
        proof,
        crate::proof::VerifyingKeyId::new(
            "stark/fri/poseidon-x7-goldilocks-6x64-v1",
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
        "stark/fri/poseidon-x7-goldilocks-6x64-v1".into(),
        norito::encode_canonical(&envelope)
            .expect("encode canonical FHE bootstrap-key OpenVerifyEnvelope"),
    );
    let mut attachment = ProofAttachment::new_ref(
        "stark/fri/poseidon-x7-goldilocks-6x64-v1".into(),
        proof,
        crate::proof::VerifyingKeyId::new(
            "stark/fri/poseidon-x7-goldilocks-6x64-v1",
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
        "stark/fri/poseidon-x7-goldilocks-6x64-v1".into(),
        norito::encode_canonical(&envelope)
            .expect("encode canonical FHE full-bootstrap execution OpenVerifyEnvelope"),
    );
    let mut attachment = ProofAttachment::new_ref(
        "stark/fri/poseidon-x7-goldilocks-6x64-v1".into(),
        proof,
        crate::proof::VerifyingKeyId::new(
            "stark/fri/poseidon-x7-goldilocks-6x64-v1",
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
