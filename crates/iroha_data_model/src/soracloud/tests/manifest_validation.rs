macro_rules! zero_prehash_field_rejection_test {
    ($name:ident, $zero:ident, $factory:expr; $($field:ident = $value:expr => ($reported:literal, $message:literal);)+) => {
        #[test]
        fn $name() {
            let $zero = zero_prehash_statement_hash();
            $(
                let mut subject = $factory;
                subject.$field = $value;
                let error = subject.validate().expect_err($message);
                assert_zero_prehash_digest_error(&error, $reported);
            )+
        }
    };
}

#[test]
fn container_validate_rejects_config_export_for_nonrequired_config() {
    let mut container = sample_container();
    container.config_exports = vec![SoraConfigExportV1 {
        config_name: "runtime/feature_flag".to_string(),
        target: SoraConfigExportTargetV1::Env("FEATURE_FLAG_JSON".to_string()),
    }];
    let error = container
        .validate()
        .expect_err("config export must reference a required config");
    assert_soracloud_invalid_field(error, "config_exports");
}
#[test]
fn container_validate_rejects_duplicate_config_export_env_targets() {
    let mut container = sample_container();
    container.required_config_names = vec![
        "runtime/theme".to_string(),
        "runtime/feature_flag".to_string(),
    ];
    container.config_exports = vec![
        SoraConfigExportV1 {
            config_name: "runtime/theme".to_string(),
            target: SoraConfigExportTargetV1::Env("APP_CONFIG_JSON".to_string()),
        },
        SoraConfigExportV1 {
            config_name: "runtime/feature_flag".to_string(),
            target: SoraConfigExportTargetV1::Env("APP_CONFIG_JSON".to_string()),
        },
    ];
    let error = container
        .validate()
        .expect_err("duplicate config export env targets must fail");
    assert_soracloud_invalid_field(error, "config_exports");
}
#[test]
fn container_validate_accepts_required_config_exports() {
    let mut container = sample_container();
    container.required_config_names = vec!["runtime/theme".to_string()];
    container.config_exports = vec![
        SoraConfigExportV1 {
            config_name: "runtime/theme".to_string(),
            target: SoraConfigExportTargetV1::Env("THEME_JSON".to_string()),
        },
        SoraConfigExportV1 {
            config_name: "runtime/theme".to_string(),
            target: SoraConfigExportTargetV1::File("runtime/theme.json".to_string()),
        },
    ];
    assert!(
        container.validate().is_ok(),
        "required config exports should validate"
    );
}
#[test]
fn service_validate_rejects_zero_prehash_container_ref_sentinel() {
    let mut manifest = sample_service(vec![sample_binding("session")]);
    manifest.container.manifest_hash = zero_prehash_statement_hash();
    let error = manifest
        .validate()
        .expect_err("service container placeholder hash must fail admission");
    assert_zero_prehash_digest_error(&error, "container.manifest_hash");
}
#[test]
fn service_validate_rejects_zero_prehash_artifact_hash_sentinel() {
    let mut manifest = sample_service(vec![sample_binding("session")]);
    manifest.artifacts[0].artifact_hash = zero_prehash_statement_hash();
    let error = manifest
        .validate()
        .expect_err("service artifact placeholder hash must fail admission");
    assert_zero_prehash_digest_error(&error, "artifact_hash");
}
#[test]
fn service_validate_rejects_duplicate_binding_names() {
    let binding = sample_binding("session_store");
    let manifest = SoraServiceManifestV1 {
        schema_version: SORA_SERVICE_MANIFEST_VERSION_V1,
        service_name: "wallet".parse().expect("valid name"),
        service_version: "1.0.0".to_string(),
        execution_plane: SoraServiceExecutionPlaneV1::DeterministicService,
        container: SoraContainerManifestRefV1 {
            manifest_hash: sample_hash(13),
            expected_schema_version: SORA_CONTAINER_MANIFEST_VERSION_V1,
        },
        replicas: NonZeroU16::new(2).expect("nonzero"),
        route: Some(SoraRouteTargetV1 {
            host: "wallet.sora".to_string(),
            path_prefix: "/".to_string(),
            service_port: NonZeroU16::new(8080).expect("nonzero"),
            visibility: SoraRouteVisibilityV1::Public,
            tls_mode: SoraTlsModeV1::Required,
        }),
        rollout: SoraRolloutPolicyV1 {
            canary_percent: 10,
            max_unavailable_replicas: 1,
            health_window_secs: NonZeroU32::new(45).expect("nonzero"),
            automatic_rollback_failures: NonZeroU32::new(3).expect("nonzero"),
        },
        economics: SoraHttpServiceEconomicsV1::default(),
        state_bindings: vec![binding.clone(), binding],
        lease_volumes: Vec::new(),
        handlers: sample_handlers(),
        artifacts: sample_artifacts(),
    };
    let error = manifest
        .validate()
        .expect_err("duplicate state binding names must fail");
    assert!(matches!(
        error,
        SoracloudManifestError::DuplicateStateBinding { .. }
    ));
}
#[test]
fn service_validate_accepts_valid_manifest() {
    let manifest = sample_service(vec![sample_binding("session"), sample_binding("profiles")]);
    assert!(manifest.validate().is_ok(), "valid manifest should pass");
}
#[cfg(feature = "json")]
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the staged service and container JSON graph is intentionally audited as one closed matrix"
)]
fn staged_service_and_container_v1_records_reject_unknown_fields() {
    macro_rules! assert_unknown_rejected {
        ($value:expr, $ty:ty, $label:literal) => {{
            let mut value = norito::json::to_value(&$value).expect(concat!("serialize ", $label));
            value
                .as_object_mut()
                .expect(concat!($label, " JSON object"))
                .insert("retired_v0".to_owned(), norito::json!(true));
            norito::json::from_value::<$ty>(value)
                .expect_err(concat!($label, " must reject unknown fields"));
        }};
    }

    let service = sample_service(vec![sample_binding("session")]);
    assert_unknown_rejected!(service, SoraServiceManifestV1, "service manifest");
    assert_unknown_rejected!(
        service.container,
        SoraContainerManifestRefV1,
        "container manifest reference"
    );
    assert_unknown_rejected!(
        service.route.clone().expect("sample route"),
        SoraRouteTargetV1,
        "route target"
    );
    assert_unknown_rejected!(service.rollout, SoraRolloutPolicyV1, "rollout policy");
    assert_unknown_rejected!(
        service.economics.clone(),
        SoraHttpServiceEconomicsV1,
        "service economics"
    );
    assert_unknown_rejected!(
        service.state_bindings[0].clone(),
        SoraStateBindingV1,
        "state binding"
    );
    assert_unknown_rejected!(
        SoraLeaseVolumeBindingV1 {
            volume_name: "root_disk".parse().expect("valid name"),
            kind: SoraLeaseVolumeKindV1::PersistentRootLeaseVolume,
            storage_class: StorageClass::Warm,
            mount_path: "/".to_owned(),
            max_total_bytes: NonZeroU64::new(1024).expect("nonzero"),
        },
        SoraLeaseVolumeBindingV1,
        "lease volume binding"
    );
    assert_unknown_rejected!(
        service.handlers[0].clone(),
        SoraServiceHandlerV1,
        "service handler"
    );
    assert_unknown_rejected!(
        SoraMailboxContractV1 {
            queue_name: "updates".parse().expect("valid name"),
            max_pending_messages: NonZeroU32::new(8).expect("nonzero"),
            max_message_bytes: NonZeroU64::new(1024).expect("nonzero"),
            retention_blocks: NonZeroU32::new(64).expect("nonzero"),
        },
        SoraMailboxContractV1,
        "mailbox contract"
    );
    assert_unknown_rejected!(
        service.artifacts[0].clone(),
        SoraArtifactRefV1,
        "artifact reference"
    );

    let container = sample_container();
    assert_unknown_rejected!(
        SoraNetworkAllowlistEntryV1::new("api.sora.org", [443]),
        SoraNetworkAllowlistEntryV1,
        "network allowlist entry"
    );
    assert_unknown_rejected!(
        container.capabilities.clone(),
        SoraCapabilityPolicyV1,
        "capability policy"
    );
    assert_unknown_rejected!(container.resources, SoraResourceLimitsV1, "resource limits");
    assert_unknown_rejected!(
        container.lifecycle.clone(),
        SoraLifecycleHooksV1,
        "lifecycle hooks"
    );
    assert_unknown_rejected!(
        SoraConfigExportV1 {
            config_name: "runtime/theme".to_owned(),
            target: SoraConfigExportTargetV1::File("runtime/theme.json".to_owned()),
        },
        SoraConfigExportV1,
        "config export"
    );

    let bundle_container = sample_container();
    let mut bundle_service = sample_service(vec![sample_binding("session")]);
    bundle_service.container.manifest_hash = Hash::new(Encode::encode(&bundle_container));
    assert_unknown_rejected!(
        SoraDeploymentBundleV1 {
            schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
            container: bundle_container,
            service: bundle_service,
        },
        SoraDeploymentBundleV1,
        "deployment bundle"
    );
}
#[cfg(feature = "json")]
#[test]
fn staged_service_and_container_v1_records_require_nullable_keys() {
    macro_rules! assert_missing_rejected {
        ($value:expr, $field:literal, $ty:ty, $label:literal) => {{
            let mut value = norito::json::to_value(&$value).expect(concat!("serialize ", $label));
            let removed = value
                .as_object_mut()
                .expect(concat!($label, " JSON object"))
                .remove($field);
            assert!(removed.is_some(), "fixture must contain `{}`", $field);
            norito::json::from_value::<$ty>(value)
                .expect_err(concat!($label, " must require its nullable key"));
        }};
    }

    let service = sample_service(vec![sample_binding("session")]);
    assert_missing_rejected!(service, "route", SoraServiceManifestV1, "service manifest");
    assert_missing_rejected!(
        service.handlers[0].clone(),
        "route_path",
        SoraServiceHandlerV1,
        "service handler"
    );
    assert_missing_rejected!(
        service.handlers[0].clone(),
        "mailbox",
        SoraServiceHandlerV1,
        "service handler"
    );
    assert_missing_rejected!(
        service.artifacts[0].clone(),
        "handler_name",
        SoraArtifactRefV1,
        "artifact reference"
    );
    assert_missing_rejected!(
        sample_container().lifecycle,
        "healthcheck_path",
        SoraLifecycleHooksV1,
        "lifecycle hooks"
    );
}
#[test]
fn service_validate_rejects_uncertified_query_handler() {
    let mut manifest = sample_service(vec![sample_binding("session")]);
    manifest.handlers[1].certified_response = SoraCertifiedResponsePolicyV1::None;
    let error = manifest
        .validate()
        .expect_err("query handlers must stay certified");
    assert_soracloud_invalid_field(error, "certified_response");
}
#[test]
fn service_validate_rejects_private_update_without_mailbox() {
    let mut manifest = sample_service(vec![sample_binding("session")]);
    manifest.handlers[3].mailbox = None;
    let error = manifest
        .validate()
        .expect_err("private_update handlers require a mailbox");
    assert_soracloud_invalid_field(error, "mailbox");
}
#[test]
fn service_validate_accepts_http_service_with_lease_volumes() {
    let mut manifest = sample_service(Vec::new());
    manifest.execution_plane = SoraServiceExecutionPlaneV1::HttpService;
    manifest.state_bindings.clear();
    manifest.handlers.clear();
    manifest.artifacts.clear();
    manifest.lease_volumes = vec![
        SoraLeaseVolumeBindingV1 {
            volume_name: "index_state".parse().expect("valid name"),
            kind: SoraLeaseVolumeKindV1::ServiceLeaseVolume,
            storage_class: StorageClass::Warm,
            mount_path: "/var/lib/ton-indexer".to_string(),
            max_total_bytes: NonZeroU64::new(50 * 1024 * 1024 * 1024).expect("nonzero"),
        },
        SoraLeaseVolumeBindingV1 {
            volume_name: "sealed_state".parse().expect("valid name"),
            kind: SoraLeaseVolumeKindV1::ConfidentialLeaseVolume,
            storage_class: StorageClass::Hot,
            mount_path: "/var/lib/ton-indexer/private".to_string(),
            max_total_bytes: NonZeroU64::new(1024 * 1024).expect("nonzero"),
        },
    ];
    assert!(
        manifest.validate().is_ok(),
        "http services should validate with route + lease volumes and no deterministic handlers"
    );
}
#[test]
fn lease_volume_binding_attachment_semantics_match_kind() {
    let root = SoraLeaseVolumeBindingV1 {
        volume_name: "root_disk".parse().expect("valid name"),
        kind: SoraLeaseVolumeKindV1::PersistentRootLeaseVolume,
        storage_class: StorageClass::Warm,
        mount_path: "/".to_string(),
        max_total_bytes: NonZeroU64::new(8 * 1024 * 1024).expect("nonzero"),
    };
    let shared = SoraLeaseVolumeBindingV1 {
        volume_name: "index_state".parse().expect("valid name"),
        kind: SoraLeaseVolumeKindV1::ServiceLeaseVolume,
        storage_class: StorageClass::Warm,
        mount_path: "/var/lib/ton-indexer".to_string(),
        max_total_bytes: NonZeroU64::new(8 * 1024 * 1024).expect("nonzero"),
    };
    assert!(root.attaches_per_replica());
    assert!(!root.attaches_shared_across_replicas());
    assert!(!shared.attaches_per_replica());
    assert!(shared.attaches_shared_across_replicas());
}
#[test]
fn service_validate_rejects_http_service_with_underfunded_prepaid_balance() {
    let mut manifest = sample_service(Vec::new());
    manifest.execution_plane = SoraServiceExecutionPlaneV1::HttpService;
    manifest.state_bindings.clear();
    manifest.handlers.clear();
    manifest.artifacts.clear();
    manifest.lease_volumes = vec![SoraLeaseVolumeBindingV1 {
        volume_name: "index_state".parse().expect("valid name"),
        kind: SoraLeaseVolumeKindV1::ServiceLeaseVolume,
        storage_class: StorageClass::Warm,
        mount_path: "/var/lib/ton-indexer".to_string(),
        max_total_bytes: NonZeroU64::new(50 * 1024 * 1024 * 1024).expect("nonzero"),
    }];
    manifest.economics.prepaid_runtime_balance = xor_quantity_from_nanos(200_000);
    let error = manifest
        .validate()
        .expect_err("hosted http services must reject obviously underfunded prepaid balances");
    assert_soracloud_invalid_field(error, "economics.prepaid_runtime_balance");
}
#[test]
fn service_validate_rejects_http_service_with_deterministic_handlers() {
    let mut manifest = sample_service(Vec::new());
    manifest.execution_plane = SoraServiceExecutionPlaneV1::HttpService;
    manifest.lease_volumes = vec![SoraLeaseVolumeBindingV1 {
        volume_name: "index_state".parse().expect("valid name"),
        kind: SoraLeaseVolumeKindV1::ServiceLeaseVolume,
        storage_class: StorageClass::Warm,
        mount_path: "/var/lib/ton-indexer".to_string(),
        max_total_bytes: NonZeroU64::new(1024 * 1024).expect("nonzero"),
    }];
    let error = manifest
        .validate()
        .expect_err("http services must not declare deterministic handlers");
    assert_soracloud_invalid_field(error, "handlers");
}
#[test]
fn deployment_bundle_validate_rejects_container_hash_mismatch() {
    let container = sample_container();
    let mut service = sample_service(vec![sample_binding("session")]);
    service.container.manifest_hash = sample_hash(99);
    let bundle = SoraDeploymentBundleV1 {
        schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
        container,
        service,
    };
    let error = bundle
        .validate_for_admission()
        .expect_err("mismatched container hash must fail admission");
    assert_soracloud_invalid_field(error, "service.container.manifest_hash");
}
#[test]
fn deployment_bundle_validate_rejects_mutable_binding_without_write_capability() {
    let mut container = sample_container();
    container.capabilities.allow_state_writes = false;
    let container_hash = Hash::new(Encode::encode(&container));
    let mut service = sample_service(vec![sample_binding("session")]);
    service.container.manifest_hash = container_hash;
    let bundle = SoraDeploymentBundleV1 {
        schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
        container,
        service,
    };
    let error = bundle
        .validate_for_admission()
        .expect_err("mutable bindings require state-write capability");
    assert_soracloud_invalid_field(error, "container.capabilities.allow_state_writes");
}
#[test]
fn deployment_bundle_validate_accepts_consistent_bundle() {
    let container = sample_container();
    let container_hash = Hash::new(Encode::encode(&container));
    let mut service = sample_service(vec![
        sample_binding("session"),
        SoraStateBindingV1 {
            schema_version: SORA_STATE_BINDING_VERSION_V1,
            binding_name: "read_only_profile".parse().expect("valid name"),
            scope: SoraStateScopeV1::AccountMetadata,
            mutability: SoraStateMutabilityV1::ReadOnly,
            encryption: SoraStateEncryptionV1::ClientCiphertext,
            key_prefix: "/state/profile".to_string(),
            max_item_bytes: NonZeroU64::new(2_048).expect("nonzero"),
            max_total_bytes: NonZeroU64::new(65_536).expect("nonzero"),
        },
    ]);
    service.container.manifest_hash = container_hash;
    let bundle = SoraDeploymentBundleV1 {
        schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
        container,
        service,
    };
    assert!(
        bundle.validate_for_admission().is_ok(),
        "consistent deployment bundle must pass"
    );
}
#[test]
fn deployment_bundle_validate_rejects_http_service_with_ivm_runtime() {
    let container = sample_container();
    let container_hash = Hash::new(Encode::encode(&container));
    let mut service = sample_service(Vec::new());
    service.execution_plane = SoraServiceExecutionPlaneV1::HttpService;
    service.container.manifest_hash = container_hash;
    service.state_bindings.clear();
    service.handlers.clear();
    service.artifacts.clear();
    service.lease_volumes = vec![SoraLeaseVolumeBindingV1 {
        volume_name: "index_state".parse().expect("valid name"),
        kind: SoraLeaseVolumeKindV1::ServiceLeaseVolume,
        storage_class: StorageClass::Warm,
        mount_path: "/var/lib/ton-indexer".to_string(),
        max_total_bytes: NonZeroU64::new(1024 * 1024).expect("nonzero"),
    }];
    let bundle = SoraDeploymentBundleV1 {
        schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
        container,
        service,
    };
    let error = bundle
        .validate_for_admission()
        .expect_err("http services must not use IVM runtime");
    assert_soracloud_invalid_field(error, "container.runtime");
}
#[test]
fn deployment_bundle_validate_accepts_inrou_http_service_without_ssh_keys() {
    let mut container = sample_container();
    container.runtime = SoraContainerRuntimeV1::Inrou;
    container.entrypoint = "/app/bin/service".to_string();
    container.inrou = Some(sample_inrou_manifest());
    container
        .inrou
        .as_mut()
        .expect("Inrou manifest")
        .ssh_authorized_keys
        .clear();
    container.capabilities.network = SoraNetworkPolicyV1::Isolated;
    let container_hash = Hash::new(Encode::encode(&container));
    let mut service = sample_service(Vec::new());
    service.execution_plane = SoraServiceExecutionPlaneV1::HttpService;
    service.replicas = NonZeroU16::new(1).expect("nonzero");
    service.container.manifest_hash = container_hash;
    service.state_bindings.clear();
    service.handlers.clear();
    service.artifacts.clear();
    service.lease_volumes = sample_inrou_lease_volumes();
    let bundle = SoraDeploymentBundleV1 {
        schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
        container,
        service,
    };
    assert!(
        bundle.validate_for_admission().is_ok(),
        "Inrou http services must not require an SSH access path"
    );
}
#[test]
fn deployment_bundle_validate_accepts_replicated_inrou_http_service() {
    let mut container = sample_container();
    container.runtime = SoraContainerRuntimeV1::Inrou;
    container.entrypoint = "/app/bin/service".to_string();
    container.inrou = Some(sample_inrou_manifest());
    container.capabilities.network =
        SoraNetworkPolicyV1::Allowlist(vec![SoraNetworkAllowlistEntryV1::new(
            "indexer.ton.example",
            [443],
        )]);
    let container_hash = Hash::new(Encode::encode(&container));
    let mut service = sample_service(Vec::new());
    service.execution_plane = SoraServiceExecutionPlaneV1::HttpService;
    service.replicas = NonZeroU16::new(3).expect("nonzero");
    service.container.manifest_hash = container_hash;
    service.state_bindings.clear();
    service.handlers.clear();
    service.artifacts.clear();
    service.lease_volumes = sample_inrou_lease_volumes();
    let bundle = SoraDeploymentBundleV1 {
        schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
        container,
        service,
    };
    assert!(
        bundle.validate_for_admission().is_ok(),
        "replicated Inrou http services should pass admission"
    );
}

#[test]
fn container_validate_rejects_open_inrou_network_egress() {
    let mut container = sample_container();
    container.runtime = SoraContainerRuntimeV1::Inrou;
    container.entrypoint = "/app/bin/service".to_string();
    container.inrou = Some(sample_inrou_manifest());
    container.capabilities.network = SoraNetworkPolicyV1::Open;

    let error = container
        .validate()
        .expect_err("Inrou V1 must reject unrestricted network egress");
    assert_soracloud_invalid_field(error, "capabilities.network");
}
#[test]
fn inrou_manifest_validate_accepts_dual_arch_guest_images() {
    let manifest = sample_inrou_manifest();
    assert!(
        manifest.validate().is_ok(),
        "dual-arch guest image profiles should validate"
    );
}
#[test]
fn inrou_guest_image_validate_accepts_published_artifact_distribution() {
    let mut image = sample_inrou_manifest()
        .guest_images
        .get(&SoraInrouGuestIsaV1::X8664)
        .cloned()
        .expect("x86_64 fixture");
    let distribution = SoraArtifactDistributionPolicyV1 {
        target: SoraArtifactDistributionTargetV1::Geographies(BTreeSet::from([
            "ae-dxb".to_string(),
            "us-east".to_string(),
        ])),
        prefer_low_latency: true,
        fallback_to_low_latency_when_geography_unknown: true,
    };
    image.distribution = distribution.clone();
    let mut artifact = sample_published_inrou_guest_image_artifact(0xAA);
    artifact.distribution = distribution;
    image.published_artifact = Some(artifact);
    assert!(
        image.validate().is_ok(),
        "published artifact refs with geo targets should validate"
    );
}
#[test]
fn inrou_guest_image_accepts_distinct_ascii_member_paths() {
    let mut image = sample_inrou_manifest()
        .guest_images
        .get(&SoraInrouGuestIsaV1::X8664)
        .cloned()
        .expect("x86_64 fixture");
    image.kernel_image_path = "/inrou/x86_64/KERNEL-v1.2.bin".to_string();
    image.rootfs_image_path = "/inrou/x86_64/rootfs_01.ext4".to_string();
    image.initrd_image_path = Some("/inrou/x86_64/initrd-01.img".to_string());
    assert!(
        image.validate().is_ok(),
        "distinct portable ASCII member paths should validate"
    );
}
#[test]
fn inrou_content_cid_codec_matches_canonical_lowercase_multibase_base32() {
    let bytes = [0x01, 0x71, 0x1f, 0x20, 0xf3, 0x09, 0x6a, 0xe2];
    let encoded = encode_lowercase_multibase_base32(&bytes);
    assert_eq!(encoded, "bafyr6ihtbfvoe");
    assert_eq!(
        decode_lowercase_multibase_base32(&encoded),
        Some(bytes.to_vec())
    );
}
#[test]
fn published_inrou_artifact_rejects_noncanonical_manifest_digest_hex() {
    for invalid in [
        "A".repeat(64),
        format!("{}g", "a".repeat(63)),
        "a".repeat(62),
        "a".repeat(66),
    ] {
        let mut artifact = sample_published_inrou_guest_image_artifact(0x21);
        artifact.manifest_digest_hex = invalid;
        let error = artifact
            .validate()
            .expect_err("noncanonical manifest digest must fail");
        assert_soracloud_invalid_field(error, "manifest_digest_hex");
    }
}
#[test]
fn published_inrou_artifact_rejects_noncanonical_or_mismatched_manifest_id_hex() {
    for invalid in [
        "A".repeat(64),
        format!("{}g", "a".repeat(63)),
        "a".repeat(62),
        "a".repeat(66),
        "b".repeat(64),
    ] {
        let mut artifact = sample_published_inrou_guest_image_artifact(0x22);
        artifact.manifest_id_hex = Some(invalid);
        let error = artifact
            .validate()
            .expect_err("noncanonical or mismatched manifest identifier must fail");
        assert_soracloud_invalid_field(error, "manifest_id_hex");
    }
}
#[test]
fn published_inrou_artifact_rejects_noncanonical_content_cid() {
    let artifact = sample_published_inrou_guest_image_artifact(0x23);
    let mut uppercase_prefix = artifact.content_cid.clone();
    uppercase_prefix.replace_range(..1, "B");
    let mut wrong_codec = sorafs_manifest::canonical_manifest_root_cid([0x24; 32]);
    wrong_codec[1] = 0x55;
    let mut nonzero_padding = artifact.content_cid.clone().into_bytes();
    let alphabet = b"abcdefghijklmnopqrstuvwxyz234567";
    let last = nonzero_padding
        .last_mut()
        .expect("canonical content CID has a final base32 character");
    let index = alphabet
        .iter()
        .position(|candidate| candidate == last)
        .expect("canonical content CID uses the lowercase base32 alphabet");
    assert_eq!(index % 4, 0, "canonical padding bits must be zero");
    *last = alphabet[index + 1];
    let nonzero_padding = String::from_utf8(nonzero_padding).expect("base32 fixture remains UTF-8");
    for invalid in [
        uppercase_prefix,
        "bafyguestimage".to_string(),
        encode_lowercase_multibase_base32(&wrong_codec),
        nonzero_padding,
    ] {
        let mut artifact = artifact.clone();
        artifact.content_cid = invalid;
        let error = artifact
            .validate()
            .expect_err("noncanonical content CID must fail");
        assert_soracloud_invalid_field(error, "content_cid");
    }
}
#[test]
fn inrou_guest_image_rejects_published_artifact_distribution_mismatch() {
    let mut image = sample_inrou_manifest()
        .guest_images
        .get(&SoraInrouGuestIsaV1::X8664)
        .cloned()
        .expect("x86_64 fixture");
    let mut artifact = sample_published_inrou_guest_image_artifact(0x25);
    artifact.distribution.target =
        SoraArtifactDistributionTargetV1::Geographies(BTreeSet::from(["ae-dxb".to_string()]));
    image.published_artifact = Some(artifact);
    let error = image
        .validate()
        .expect_err("artifact distribution drift must fail");
    assert_soracloud_invalid_field(error, "published_artifact.distribution");
}
#[test]
fn inrou_guest_image_rejects_noncanonical_member_paths_and_aliases() {
    for invalid in [
        "/outside/x86_64/vmlinux",
        "/inrou//vmlinux",
        "/inrou/./vmlinux",
        "/inrou/x86_64/../vmlinux",
        "/inrou/x86_64/vmlinux/",
        "/inrou/x86_64/bad:name",
        "/inrou/x86_64/CON",
    ] {
        let mut image = sample_inrou_manifest()
            .guest_images
            .get(&SoraInrouGuestIsaV1::X8664)
            .cloned()
            .expect("x86_64 fixture");
        image.kernel_image_path = invalid.to_string();
        let error = image
            .validate()
            .expect_err("noncanonical Inrou member path must fail");
        assert_soracloud_invalid_field(error, "kernel_image_path");
    }
    let mut image = sample_inrou_manifest()
        .guest_images
        .get(&SoraInrouGuestIsaV1::X8664)
        .cloned()
        .expect("x86_64 fixture");
    image.rootfs_image_path = image.kernel_image_path.clone();
    let error = image
        .validate()
        .expect_err("duplicate Inrou member paths must fail");
    assert_soracloud_invalid_field(error, "rootfs_image_path");
}
#[test]
fn inrou_guest_image_rejects_non_ascii_member_components() {
    for invalid in ["/inrou/x86_64/vmlinüx", "/inrou/架構/rootfs.ext4"] {
        let mut image = sample_inrou_manifest()
            .guest_images
            .get(&SoraInrouGuestIsaV1::X8664)
            .cloned()
            .expect("x86_64 fixture");
        image.kernel_image_path = invalid.to_string();
        let error = image
            .validate()
            .expect_err("non-ASCII Inrou member path component must fail");
        assert_soracloud_invalid_field(error, "kernel_image_path");
    }
}
#[test]
fn inrou_guest_image_rejects_ascii_case_insensitive_member_path_collisions() {
    let mut rootfs_collision = sample_inrou_manifest()
        .guest_images
        .get(&SoraInrouGuestIsaV1::X8664)
        .cloned()
        .expect("x86_64 fixture");
    rootfs_collision.rootfs_image_path = "/inrou/x86_64/VMLINUX".to_string();
    let error = rootfs_collision
        .validate()
        .expect_err("case-insensitive rootfs member-path collision must fail");
    assert_soracloud_invalid_field(error, "rootfs_image_path");
    let mut initrd_collision = sample_inrou_manifest()
        .guest_images
        .get(&SoraInrouGuestIsaV1::X8664)
        .cloned()
        .expect("x86_64 fixture");
    initrd_collision.initrd_image_path = Some("/inrou/X86_64/VMLINUX".to_string());
    let error = initrd_collision
        .validate()
        .expect_err("case-insensitive initrd member-path collision must fail");
    assert_soracloud_invalid_field(error, "initrd_image_path");
}
#[test]
fn artifact_distribution_policy_rejects_empty_geography_target() {
    let policy = SoraArtifactDistributionPolicyV1 {
        target: SoraArtifactDistributionTargetV1::Geographies(BTreeSet::new()),
        ..SoraArtifactDistributionPolicyV1::default()
    };
    let error = policy
        .validate()
        .expect_err("empty geography target must fail");
    assert_soracloud_invalid_field(error, "target");
}
#[test]
fn inrou_manifest_validate_accepts_one_native_guest_isa() {
    let mut manifest = sample_inrou_manifest();
    manifest
        .guest_images
        .remove(&SoraInrouGuestIsaV1::X8664)
        .expect("fixture x86_64 guest image");
    manifest
        .validate()
        .expect("one native guest ISA profile is sufficient");
}
#[test]
fn inrou_manifest_validate_rejects_empty_guest_image_map() {
    let mut manifest = sample_inrou_manifest();
    manifest.guest_images.clear();
    let error = manifest
        .validate()
        .expect_err("at least one native guest image must be published");
    assert_soracloud_invalid_field(error, "guest_images");
}
#[cfg(feature = "json")]
#[test]
fn inrou_manifest_json_deserialize_rejects_flat_guest_images() {
    let manifest_json = r#"{
          "schema_version": 1,
          "guest_os": {
            "guest_os": "DebianSlim",
            "value": null
          },
          "bootstrap_user_data_path": null,
          "kernel_image_path": "/inrou/shared/vmlinux",
          "rootfs_image_path": "/inrou/shared/rootfs.ext4",
          "initrd_image_path": null,
          "ssh_authorized_keys": ["ssh-ed25519 AAAA canonical"]
        }"#;
    let error = norito::json::from_str::<SoraInrouManifestV1>(manifest_json)
        .expect_err("flat Inrou guest image fields must not deserialize");
    assert!(matches!(
        error,
        json::Error::MissingField { ref field } if field == "guest_images"
    ));
}
#[cfg(feature = "json")]
#[test]
fn inrou_manifest_json_deserialize_accepts_published_guest_image_artifact() {
    let x86_content_cid = encode_lowercase_multibase_base32(
        &sorafs_manifest::canonical_manifest_root_cid([0x31; 32]),
    );
    let aarch64_content_cid = encode_lowercase_multibase_base32(
        &sorafs_manifest::canonical_manifest_root_cid([0x32; 32]),
    );
    let json = r#"{
          "schema_version": 1,
          "guest_os": {
            "guest_os": "DebianSlim",
            "value": null
          },
          "guest_images": {
            "x86_64": {
              "kernel_image_path": "/inrou/x86_64/vmlinux",
              "rootfs_image_path": "/inrou/x86_64/rootfs.ext4",
              "initrd_image_path": null,
              "distribution": {
                "target": {"target": "Global", "value": null},
                "prefer_low_latency": true,
                "fallback_to_low_latency_when_geography_unknown": true
              },
              "published_artifact": {
                "manifest_digest_hex": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "content_cid": "__X86_CONTENT_CID__",
                "manifest_id_hex": null,
                "distribution": {
                  "target": {"target": "Global", "value": null},
                  "prefer_low_latency": true,
                  "fallback_to_low_latency_when_geography_unknown": true
                }
              }
            },
            "aarch64": {
              "kernel_image_path": "/inrou/aarch64/vmlinux",
              "rootfs_image_path": "/inrou/aarch64/rootfs.ext4",
              "initrd_image_path": null,
              "distribution": {
                "target": {"target": "Global", "value": null},
                "prefer_low_latency": true,
                "fallback_to_low_latency_when_geography_unknown": true
              },
              "published_artifact": {
                "manifest_digest_hex": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "content_cid": "__AARCH64_CONTENT_CID__",
                "manifest_id_hex": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "distribution": {
                  "target": {"target": "Global", "value": null},
                  "prefer_low_latency": true,
                  "fallback_to_low_latency_when_geography_unknown": true
                }
              }
            }
          },
          "bootstrap_user_data_path": null,
          "ssh_authorized_keys": ["ssh-ed25519 AAAA real"]
        }"#
        .replace("__X86_CONTENT_CID__", &x86_content_cid)
        .replace("__AARCH64_CONTENT_CID__", &aarch64_content_cid);
    let manifest: SoraInrouManifestV1 =
        norito::json::from_str(&json).expect("published guest artifact JSON should parse");
    let artifact = manifest.guest_images[&SoraInrouGuestIsaV1::X8664]
        .published_artifact
        .as_ref()
        .expect("published artifact");
    assert_eq!(artifact.content_cid, x86_content_cid);
    assert_eq!(
        artifact.distribution,
        SoraArtifactDistributionPolicyV1::default()
    );
    assert!(manifest.validate().is_ok());
}
#[cfg(feature = "json")]
#[test]
fn inrou_manifest_json_deserialize_rejects_flat_guest_image_overlays() {
    let json = r#"{
          "schema_version": 1,
          "guest_os": {
            "guest_os": "DebianSlim",
            "value": null
          },
          "guest_images": {
            "x86_64": {
              "kernel_image_path": "/inrou/x86_64/vmlinux",
              "rootfs_image_path": "/inrou/x86_64/rootfs.ext4",
              "initrd_image_path": null,
              "distribution": {
                "target": {"target": "Global", "value": null},
                "prefer_low_latency": true,
                "fallback_to_low_latency_when_geography_unknown": true
              },
              "published_artifact": null
            },
            "aarch64": {
              "kernel_image_path": "/inrou/aarch64/vmlinux",
              "rootfs_image_path": "/inrou/aarch64/rootfs.ext4",
              "initrd_image_path": null,
              "distribution": {
                "target": {"target": "Global", "value": null},
                "prefer_low_latency": true,
                "fallback_to_low_latency_when_geography_unknown": true
              },
              "published_artifact": null
            }
          },
          "bootstrap_user_data_path": null,
          "ssh_authorized_keys": [],
          "kernel_image_path": "/flat/vmlinux",
          "rootfs_image_path": "/flat/rootfs.ext4"
        }"#;
    let error = norito::json::from_str::<SoraInrouManifestV1>(json)
        .expect_err("flat guest-image overlay fields must be rejected");
    assert!(matches!(
        error,
        json::Error::UnknownField { ref field } if field == "kernel_image_path"
    ));
}
#[cfg(feature = "json")]
#[test]
fn inrou_manifest_json_serialize_omits_flat_guest_image_fields() {
    let manifest = sample_inrou_manifest();
    let value = norito::json::to_value(&manifest).expect("serialize inrou manifest");
    assert!(value.get("kernel_image_path").is_none());
    assert!(value.get("rootfs_image_path").is_none());
    assert!(value.get("initrd_image_path").is_none());
    assert!(value.get("guest_images").is_some());
}
#[cfg(feature = "json")]
#[test]
fn inrou_manifest_json_serialize_emits_valid_string_keyed_guest_images() {
    let manifest = sample_inrou_manifest();
    let json = norito::json::to_json(&manifest).expect("serialize inrou manifest to JSON");
    assert!(
        !json.contains("\"guest_isa\""),
        "guest image keys must render as strings: {json}"
    );
    let value: Value =
        norito::json::from_str(&json).expect("serialized inrou manifest JSON should parse");
    let guest_images = value
        .get("guest_images")
        .and_then(Value::as_object)
        .expect("guest_images should decode as an object");
    assert_eq!(
        guest_images.keys().cloned().collect::<Vec<_>>(),
        vec!["aarch64".to_owned(), "x86_64".to_owned()]
    );
}
#[cfg(feature = "json")]
fn assert_inrou_manifest_field_is_required(canonical: &Value, field: &str) {
    let mut value = canonical.clone();
    assert!(
        value
            .as_object_mut()
            .expect("manifest object")
            .remove(field)
            .is_some()
    );
    let error = norito::json::from_value::<SoraInrouManifestV1>(value)
        .expect_err("first-release Inrou fields must not be omitted");
    assert!(
        matches!(&error, json::Error::MissingField { field: missing } if missing == field),
        "missing `{field}` reported the wrong error: {error:?}"
    );
}

#[cfg(feature = "json")]
fn assert_inrou_guest_image_field_is_required(canonical: &Value, field: &str) {
    let mut value = canonical.clone();
    let guest = value
        .get_mut("guest_images")
        .and_then(Value::as_object_mut)
        .and_then(|images| images.get_mut("x86_64"))
        .and_then(Value::as_object_mut)
        .expect("x86_64 guest image object");
    assert!(guest.remove(field).is_some());
    let error = norito::json::from_value::<SoraInrouManifestV1>(value)
        .expect_err("first-release guest-image fields must not be omitted");
    assert!(
        matches!(&error, json::Error::MissingField { field: missing } if missing == field),
        "missing guest-image `{field}` reported the wrong error: {error:?}"
    );
}

#[cfg(feature = "json")]
fn assert_inrou_distribution_field_is_required(canonical: &Value, field: &str) {
    let mut value = canonical.clone();
    let distribution = value
        .get_mut("guest_images")
        .and_then(Value::as_object_mut)
        .and_then(|images| images.get_mut("x86_64"))
        .and_then(|guest| guest.get_mut("distribution"))
        .and_then(Value::as_object_mut)
        .expect("x86_64 distribution object");
    assert!(distribution.remove(field).is_some());
    let error = norito::json::from_value::<SoraInrouManifestV1>(value)
        .expect_err("first-release distribution fields must not be omitted");
    assert!(
        matches!(&error, json::Error::Message(message) if message == &format!("missing field `{field}`")),
        "missing distribution `{field}` reported the wrong error: {error:?}"
    );
}

#[cfg(feature = "json")]
fn assert_inrou_published_artifact_field_is_required(published: &Value, field: &str) {
    let mut value = published.clone();
    let artifact = value
        .get_mut("guest_images")
        .and_then(Value::as_object_mut)
        .and_then(|images| images.get_mut("x86_64"))
        .and_then(|guest| guest.get_mut("published_artifact"))
        .and_then(Value::as_object_mut)
        .expect("published guest-image artifact object");
    assert!(artifact.remove(field).is_some());
    let error = norito::json::from_value::<SoraInrouManifestV1>(value)
        .expect_err("first-release published-artifact fields must not be omitted");
    assert!(
        matches!(&error, json::Error::Message(message) if message == &format!("missing field `{field}`")),
        "missing published-artifact `{field}` reported the wrong error: {error:?}"
    );
}

#[cfg(feature = "json")]
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the Inrou tagged-enum JSON graph is intentionally audited as one closed matrix"
)]
fn inrou_v1_tagged_enum_envelopes_reject_unknown_fields() {
    macro_rules! assert_unknown_rejected {
        ($value:expr, $ty:ty, $label:literal) => {{
            let mut value = norito::json::to_value(&$value).expect(concat!("serialize ", $label));
            value
                .as_object_mut()
                .expect(concat!($label, " JSON object"))
                .insert("retired_v0".to_owned(), norito::json!(true));
            let error = norito::json::from_value::<$ty>(value)
                .expect_err(concat!($label, " must reject unknown envelope fields"));
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

    assert_unknown_rejected!(
        SoraContainerRuntimeV1::Inrou,
        SoraContainerRuntimeV1,
        "container runtime"
    );
    assert_unknown_rejected!(
        SoraInrouGuestOsV1::DebianSlim,
        SoraInrouGuestOsV1,
        "Inrou guest OS"
    );
    assert_unknown_rejected!(
        SoraInrouGuestIsaV1::Aarch64,
        SoraInrouGuestIsaV1,
        "Inrou guest ISA"
    );
    assert_unknown_rejected!(
        SoraArtifactDistributionTargetV1::Global,
        SoraArtifactDistributionTargetV1,
        "Inrou artifact distribution target"
    );
    assert_unknown_rejected!(
        SoraNetworkPolicyV1::Isolated,
        SoraNetworkPolicyV1,
        "Inrou network policy"
    );
    assert_unknown_rejected!(
        SoraConfigExportTargetV1::Env("APP_CONFIG_JSON".to_owned()),
        SoraConfigExportTargetV1,
        "Inrou config export target"
    );
    assert_unknown_rejected!(
        SoraRouteVisibilityV1::Public,
        SoraRouteVisibilityV1,
        "Inrou route visibility"
    );
    assert_unknown_rejected!(SoraTlsModeV1::Required, SoraTlsModeV1, "Inrou TLS mode");
    assert_unknown_rejected!(
        SoraServiceExecutionPlaneV1::HttpService,
        SoraServiceExecutionPlaneV1,
        "Inrou service execution plane"
    );
    assert_unknown_rejected!(
        SoraLeaseVolumeKindV1::PersistentRootLeaseVolume,
        SoraLeaseVolumeKindV1,
        "Inrou lease volume kind"
    );
    assert_unknown_rejected!(
        SoraServiceHealthStatusV1::Healthy,
        SoraServiceHealthStatusV1,
        "Inrou replica health status"
    );
    assert_unknown_rejected!(
        SecretEnvelopeEncryptionV1::ClientCiphertext,
        SecretEnvelopeEncryptionV1,
        "Inrou secret-envelope encryption"
    );
    assert_unknown_rejected!(
        StorageClass::Warm,
        StorageClass,
        "Inrou lease-volume storage class"
    );
    assert_unknown_rejected!(
        SoraArtifactKindV1::Bundle,
        SoraArtifactKindV1,
        "Inrou service artifact kind"
    );
    assert_unknown_rejected!(
        SoraServiceLifecycleActionV1::Deploy,
        SoraServiceLifecycleActionV1,
        "Soracloud service lifecycle action"
    );
    assert_unknown_rejected!(
        SoraStateMutationOperationV1::Upsert,
        SoraStateMutationOperationV1,
        "Soracloud state mutation operation"
    );
    assert_unknown_rejected!(
        SoraRolloutStageV1::Canary,
        SoraRolloutStageV1,
        "Soracloud rollout stage"
    );
    assert_unknown_rejected!(
        SoraServiceLeaseStatusV1::Active,
        SoraServiceLeaseStatusV1,
        "Soracloud service lease status"
    );
    assert_unknown_rejected!(
        SoraStateScopeV1::ServiceState,
        SoraStateScopeV1,
        "Soracloud state scope"
    );
    assert_unknown_rejected!(
        SoraStateMutabilityV1::ReadWrite,
        SoraStateMutabilityV1,
        "Soracloud state mutability"
    );
    assert_unknown_rejected!(
        SoraStateEncryptionV1::FheCiphertext,
        SoraStateEncryptionV1,
        "Soracloud state encryption"
    );
    assert_unknown_rejected!(
        BfvCiphertextBoundModeV1::ExactResidualMultiple,
        BfvCiphertextBoundModeV1,
        "Soracloud BFV ciphertext bound mode"
    );
}
#[cfg(feature = "json")]
#[test]
fn inrou_manifest_json_requires_the_exact_v1_shape() {
    let manifest = sample_inrou_manifest();
    let canonical = norito::json::to_value(&manifest).expect("serialize canonical Inrou manifest");
    assert_eq!(
        norito::json::from_value::<SoraInrouManifestV1>(canonical.clone())
            .expect("the explicit canonical V1 shape must decode"),
        manifest
    );
    for field in [
        "schema_version",
        "guest_os",
        "guest_images",
        "bootstrap_user_data_path",
        "ssh_authorized_keys",
    ] {
        assert_inrou_manifest_field_is_required(&canonical, field);
    }

    let mut shorthand = canonical.clone();
    shorthand.as_object_mut().expect("manifest object").insert(
        "guest_os".to_owned(),
        Value::String("DebianSlim".to_owned()),
    );
    norito::json::from_value::<SoraInrouManifestV1>(shorthand)
        .expect_err("the retired guest_os shorthand must not be accepted");

    for field in [
        "kernel_image_path",
        "rootfs_image_path",
        "initrd_image_path",
        "distribution",
        "published_artifact",
    ] {
        assert_inrou_guest_image_field_is_required(&canonical, field);
    }

    for field in [
        "target",
        "prefer_low_latency",
        "fallback_to_low_latency_when_geography_unknown",
    ] {
        assert_inrou_distribution_field_is_required(&canonical, field);
    }
    for (field, value) in [
        ("future_distribution", Value::Bool(true)),
        ("legacy_target", Value::Null),
    ] {
        let mut unknown_distribution = canonical.clone();
        unknown_distribution
            .pointer_mut("/guest_images/x86_64/distribution")
            .and_then(Value::as_object_mut)
            .expect("x86_64 distribution object")
            .insert(field.to_owned(), value);
        let error = norito::json::from_value::<SoraInrouManifestV1>(unknown_distribution)
            .expect_err("first-release distribution fields must reject unknown keys");
        assert!(
            matches!(
                &error,
                json::Error::UnknownField { field: reported } if reported == field
            ),
            "unknown distribution field `{field}` reported the wrong error: {error:?}"
        );
    }

    let mut published_manifest = sample_inrou_manifest();
    published_manifest
        .guest_images
        .get_mut(&SoraInrouGuestIsaV1::X8664)
        .expect("x86_64 guest image")
        .published_artifact = Some(sample_published_inrou_guest_image_artifact(31));
    let published = norito::json::to_value(&published_manifest)
        .expect("serialize published guest-image artifact");
    for field in [
        "manifest_digest_hex",
        "content_cid",
        "manifest_id_hex",
        "distribution",
    ] {
        assert_inrou_published_artifact_field_is_required(&published, field);
    }
    for (field, value) in [
        ("future_artifact", Value::Bool(true)),
        ("legacy_manifest_id", Value::Null),
    ] {
        let mut unknown_artifact = published.clone();
        unknown_artifact
            .pointer_mut("/guest_images/x86_64/published_artifact")
            .and_then(Value::as_object_mut)
            .expect("published guest-image artifact object")
            .insert(field.to_owned(), value);
        let error = norito::json::from_value::<SoraInrouManifestV1>(unknown_artifact)
            .expect_err("first-release published-artifact fields must reject unknown keys");
        assert!(
            matches!(
                &error,
                json::Error::UnknownField { field: reported } if reported == field
            ),
            "unknown published-artifact field `{field}` reported the wrong error: {error:?}"
        );
    }
}
#[cfg(feature = "json")]
#[test]
fn inrou_manifest_json_rejects_null_for_non_optional_v1_fields() {
    let canonical = norito::json::to_value(&sample_inrou_manifest())
        .expect("serialize canonical Inrou manifest");
    for field in [
        "schema_version",
        "guest_os",
        "guest_images",
        "ssh_authorized_keys",
    ] {
        let mut value = canonical.clone();
        value
            .as_object_mut()
            .expect("manifest object")
            .insert(field.to_owned(), Value::Null);
        norito::json::from_value::<SoraInrouManifestV1>(value)
            .expect_err("non-optional first-release Inrou fields must not accept null");
    }
    for field in ["kernel_image_path", "rootfs_image_path", "distribution"] {
        let mut value = canonical.clone();
        value
            .get_mut("guest_images")
            .and_then(Value::as_object_mut)
            .and_then(|images| images.get_mut("x86_64"))
            .and_then(Value::as_object_mut)
            .expect("x86_64 guest image object")
            .insert(field.to_owned(), Value::Null);
        norito::json::from_value::<SoraInrouManifestV1>(value)
            .expect_err("non-optional first-release guest-image fields must not accept null");
    }
}
#[cfg(feature = "json")]
#[test]
fn container_manifest_json_deserialize_accepts_null_inrou() {
    let json = r#"{
          "schema_version": 1,
          "runtime": {
            "runtime": "Ivm",
            "value": null
          },
          "bundle_hash": "hash:0708090A0B0C0D0E0F101112131415161718191A1B1C1D1E1F20212223242527#81B4",
          "bundle_path": "/bundles/vault-api.to",
          "entrypoint": "main",
          "args": ["--http", "--port=8788"],
          "env": {
            "SORACLOUD_TEMPLATE": "split-app-vault"
          },
          "inrou": null,
          "required_config_names": [],
          "required_secret_names": [],
          "config_exports": [],
          "capabilities": {
            "network": {
              "mode": "Open",
              "value": null
            },
            "allow_wallet_signing": false,
            "allow_state_writes": false,
            "allow_model_inference": false,
            "allow_model_training": false
          },
          "resources": {
            "cpu_millis": 750,
            "memory_bytes": 536870912,
            "ephemeral_storage_bytes": 2147483648,
            "max_open_files": 512,
            "max_tasks": 64
          },
          "lifecycle": {
            "start_grace_secs": 30,
            "stop_grace_secs": 20,
            "healthcheck_path": "/api/auth/me"
          }
        }"#;
    let manifest: SoraContainerManifestV1 = norito::json::from_str(json)
        .expect("container manifest with null inrou should deserialize");
    assert_eq!(manifest.runtime, SoraContainerRuntimeV1::Ivm);
    assert!(manifest.inrou.is_none());
}
#[cfg(feature = "json")]
#[test]
fn container_manifest_json_deserialize_accepts_inrou_guest_images() {
    let json = r#"{
          "schema_version": 1,
          "runtime": {
            "runtime": "Inrou",
            "value": null
          },
          "bundle_hash": "hash:6F1EB280D8121258AE08C4FCDB5995500A2A0CC36785E5A680F3DFC534F70D2D#FEA9",
          "bundle_path": "/bundles/ton-indexer.inrou",
          "entrypoint": "/app/bin/launch-ton-indexer.sh",
          "args": [],
          "env": {
            "RUST_LOG": "info"
          },
          "inrou": {
            "schema_version": 1,
            "guest_os": {
              "guest_os": "DebianSlim",
              "value": null
            },
            "guest_images": {
              "x86_64": {
                "kernel_image_path": "/inrou/x86_64/vmlinux",
                "rootfs_image_path": "/inrou/x86_64/rootfs.ext4",
                "initrd_image_path": null,
                "distribution": {
                  "target": {"target": "Global", "value": null},
                  "prefer_low_latency": true,
                  "fallback_to_low_latency_when_geography_unknown": true
                },
                "published_artifact": null
              },
              "aarch64": {
                "kernel_image_path": "/inrou/aarch64/vmlinux",
                "rootfs_image_path": "/inrou/aarch64/rootfs.ext4",
                "initrd_image_path": null,
                "distribution": {
                  "target": {"target": "Global", "value": null},
                  "prefer_low_latency": true,
                  "fallback_to_low_latency_when_geography_unknown": true
                },
                "published_artifact": null
              }
            },
            "bootstrap_user_data_path": null,
            "ssh_authorized_keys": []
          },
          "required_config_names": [],
          "required_secret_names": [],
          "config_exports": [],
          "capabilities": {
            "network": {
              "mode": "Allowlist",
              "value": [
                {
                  "host": "taira.sora.org",
                  "ports": [443]
                }
              ]
            },
            "allow_wallet_signing": false,
            "allow_state_writes": false,
            "allow_model_inference": false,
            "allow_model_training": false
          },
          "resources": {
            "cpu_millis": 2000,
            "memory_bytes": 4294967296,
            "ephemeral_storage_bytes": 8589934592,
            "max_open_files": 4096,
            "max_tasks": 512
          },
          "lifecycle": {
            "start_grace_secs": 60,
            "stop_grace_secs": 30,
            "healthcheck_path": "/api/indexer/v1/health"
          }
        }"#;
    let manifest: SoraContainerManifestV1 =
        norito::json::from_str(json).expect("container JSON should deserialize");
    assert_eq!(manifest.runtime, SoraContainerRuntimeV1::Inrou);
    assert_eq!(manifest.bundle_path, "/bundles/ton-indexer.inrou");
    let inrou = manifest.inrou.expect("inrou config should be present");
    assert!(inrou.ssh_authorized_keys.is_empty());
    assert_eq!(
        inrou.guest_images[&SoraInrouGuestIsaV1::X8664].kernel_image_path,
        "/inrou/x86_64/vmlinux"
    );
}
#[test]
fn deployment_bundle_validate_rejects_http_service_without_shared_lease_volume() {
    let mut container = sample_container();
    container.runtime = SoraContainerRuntimeV1::Inrou;
    container.entrypoint = "/app/bin/service".to_string();
    container.inrou = Some(sample_inrou_manifest());
    container.capabilities.network =
        SoraNetworkPolicyV1::Allowlist(vec![SoraNetworkAllowlistEntryV1::new(
            "indexer.ton.example",
            [443],
        )]);
    let container_hash = Hash::new(Encode::encode(&container));
    let mut service = sample_service(Vec::new());
    service.execution_plane = SoraServiceExecutionPlaneV1::HttpService;
    service.replicas = NonZeroU16::new(2).expect("nonzero");
    service.container.manifest_hash = container_hash;
    service.state_bindings.clear();
    service.handlers.clear();
    service.artifacts.clear();
    service.lease_volumes = vec![SoraLeaseVolumeBindingV1 {
        volume_name: "root_disk".parse().expect("valid name"),
        kind: SoraLeaseVolumeKindV1::PersistentRootLeaseVolume,
        storage_class: StorageClass::Warm,
        mount_path: "/".to_string(),
        max_total_bytes: NonZeroU64::new(8 * 1024 * 1024 * 1024).expect("nonzero"),
    }];
    let bundle = SoraDeploymentBundleV1 {
        schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
        container,
        service,
    };
    let error = bundle
        .validate_for_admission()
        .expect_err("replicated Inrou http services must declare shared storage");
    assert_soracloud_invalid_field(error, "service.lease_volumes");
}
#[test]
fn deployment_bundle_validate_accepts_http_service_with_confidential_shared_lease() {
    let mut container = sample_container();
    container.runtime = SoraContainerRuntimeV1::Inrou;
    container.entrypoint = "/app/bin/service".to_string();
    container.inrou = Some(sample_inrou_manifest());
    container.capabilities.network =
        SoraNetworkPolicyV1::Allowlist(vec![SoraNetworkAllowlistEntryV1::new(
            "indexer.ton.example",
            [443],
        )]);
    let container_hash = Hash::new(Encode::encode(&container));
    let mut service = sample_service(Vec::new());
    service.execution_plane = SoraServiceExecutionPlaneV1::HttpService;
    service.replicas = NonZeroU16::new(2).expect("nonzero");
    service.container.manifest_hash = container_hash;
    service.state_bindings.clear();
    service.handlers.clear();
    service.artifacts.clear();
    service.lease_volumes = vec![
        SoraLeaseVolumeBindingV1 {
            volume_name: "root_disk".parse().expect("valid name"),
            kind: SoraLeaseVolumeKindV1::PersistentRootLeaseVolume,
            storage_class: StorageClass::Warm,
            mount_path: "/".to_string(),
            max_total_bytes: NonZeroU64::new(8 * 1024 * 1024 * 1024).expect("nonzero"),
        },
        SoraLeaseVolumeBindingV1 {
            volume_name: "sealed_state".parse().expect("valid name"),
            kind: SoraLeaseVolumeKindV1::ConfidentialLeaseVolume,
            storage_class: StorageClass::Hot,
            mount_path: "/var/lib/ton-indexer/private".to_string(),
            max_total_bytes: NonZeroU64::new(1024 * 1024).expect("nonzero"),
        },
    ];
    let bundle = SoraDeploymentBundleV1 {
        schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
        container,
        service,
    };
    assert!(
        bundle.validate_for_admission().is_ok(),
        "confidential lease volumes should satisfy the shared hosted-storage requirement"
    );
}
#[test]
fn agent_apartment_validate_rejects_allowlist_entry_without_ports() {
    let mut manifest = sample_agent_apartment_manifest();
    manifest.network_egress =
        SoraNetworkPolicyV1::Allowlist(vec![SoraNetworkAllowlistEntryV1::new(
            "rpc.sora.internal",
            [],
        )]);
    let error = manifest
        .validate()
        .expect_err("allowlist entries without ports must be rejected");
    assert_soracloud_invalid_field(error, "network_egress");
}
#[test]
fn deployment_bundle_validate_rejects_unknown_http_service_quota_class() {
    let mut container = sample_container();
    container.runtime = SoraContainerRuntimeV1::Inrou;
    container.entrypoint = "/app/bin/service".to_string();
    container.inrou = Some(sample_inrou_manifest());
    container.capabilities.network = SoraNetworkPolicyV1::Isolated;
    let container_hash = Hash::new(Encode::encode(&container));
    let mut service = sample_service(Vec::new());
    service.execution_plane = SoraServiceExecutionPlaneV1::HttpService;
    service.replicas = NonZeroU16::new(1).expect("nonzero");
    service.container.manifest_hash = container_hash;
    service.state_bindings.clear();
    service.handlers.clear();
    service.artifacts.clear();
    service.economics.quota_class = "taira-unsupported".to_string();
    service.lease_volumes = sample_inrou_lease_volumes();
    let bundle = SoraDeploymentBundleV1 {
        schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
        container,
        service,
    };
    let error = bundle
        .validate_for_admission()
        .expect_err("unknown hosted-service quota classes must fail admission");
    assert_soracloud_invalid_field(error, "service.economics.quota_class");
}
#[test]
fn deployment_bundle_validate_rejects_http_service_resources_over_quota_class_cap() {
    let mut container = sample_container();
    container.runtime = SoraContainerRuntimeV1::Inrou;
    container.entrypoint = "/app/bin/service".to_string();
    container.inrou = Some(sample_inrou_manifest());
    container.capabilities.network = SoraNetworkPolicyV1::Isolated;
    container.resources.cpu_millis = NonZeroU32::new(5_000).expect("nonzero");
    let container_hash = Hash::new(Encode::encode(&container));
    let mut service = sample_service(Vec::new());
    service.execution_plane = SoraServiceExecutionPlaneV1::HttpService;
    service.replicas = NonZeroU16::new(1).expect("nonzero");
    service.container.manifest_hash = container_hash;
    service.state_bindings.clear();
    service.handlers.clear();
    service.artifacts.clear();
    service.lease_volumes = sample_inrou_lease_volumes();
    let bundle = SoraDeploymentBundleV1 {
        schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
        container,
        service,
    };
    let error = bundle
        .validate_for_admission()
        .expect_err("hosted HTTP services must stay within the selected quota class");
    assert_soracloud_invalid_field(error, "container.resources.cpu_millis");
}
#[test]
fn deployment_bundle_validate_rejects_missing_required_service_config() {
    let mut container = sample_container();
    container.required_config_names = vec!["runtime/feature_flag".to_string()];
    let container_hash = Hash::new(Encode::encode(&container));
    let mut service = sample_service(vec![sample_binding("session")]);
    service.container.manifest_hash = container_hash;
    let bundle = SoraDeploymentBundleV1 {
        schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
        container,
        service,
    };
    let error = bundle
        .validate_required_service_materials(&BTreeMap::new(), &BTreeMap::new())
        .expect_err("missing required config must fail");
    assert_soracloud_invalid_field(error, "container.required_config_names");
}
#[test]
fn deployment_bundle_validate_accepts_present_required_service_materials() {
    let mut container = sample_container();
    container.required_config_names = vec!["runtime/feature_flag".to_string()];
    container.required_secret_names = vec!["db/password".to_string()];
    let container_hash = Hash::new(Encode::encode(&container));
    let mut service = sample_service(vec![sample_binding("session")]);
    service.container.manifest_hash = container_hash;
    let bundle = SoraDeploymentBundleV1 {
        schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
        container,
        service,
    };
    let config_value_json = Json::from(norito::json!(true));
    let service_configs = BTreeMap::from([(
        "runtime/feature_flag".to_string(),
        SoraServiceConfigEntryV1 {
            schema_version: SORA_SERVICE_CONFIG_ENTRY_VERSION_V1,
            config_name: "runtime/feature_flag".to_string(),
            value_hash: Hash::new(
                canonical_service_config_json_payload(&config_value_json)
                    .expect("canonical payload"),
            ),
            value_json: config_value_json,
            last_update_sequence: 1,
        },
    )]);
    let secret_envelope = SecretEnvelopeV1 {
        schema_version: SECRET_ENVELOPE_VERSION_V1,
        encryption: SecretEnvelopeEncryptionV1::ClientCiphertext,
        key_id: "kms://tenant/db".to_string(),
        key_version: NonZeroU32::new(1).expect("nonzero"),
        nonce: vec![1, 2, 3],
        ciphertext: vec![4, 5, 6],
        commitment: sample_hash(201),
        aad_digest: None,
    };
    let service_secrets = BTreeMap::from([(
        "db/password".to_string(),
        SoraServiceSecretEntryV1 {
            schema_version: SORA_SERVICE_SECRET_ENTRY_VERSION_V1,
            secret_name: "db/password".to_string(),
            envelope: secret_envelope,
            last_update_sequence: 1,
        },
    )]);
    assert!(
        bundle
            .validate_required_service_materials(&service_configs, &service_secrets)
            .is_ok(),
        "required materials present in the effective deployment state must pass"
    );
}
#[cfg(feature = "json")]
#[test]
fn secret_envelope_v1_requires_explicit_nullable_aad_and_closed_fields() {
    let envelope = SecretEnvelopeV1 {
        schema_version: SECRET_ENVELOPE_VERSION_V1,
        encryption: SecretEnvelopeEncryptionV1::ClientCiphertext,
        key_id: "kms://tenant/db".to_owned(),
        key_version: NonZeroU32::new(1).expect("nonzero"),
        nonce: vec![1, 2, 3],
        ciphertext: vec![4, 5, 6],
        commitment: sample_hash(201),
        aad_digest: None,
    };
    let canonical = norito::json::to_value(&envelope).expect("serialize secret envelope");
    assert!(
        canonical
            .get("aad_digest")
            .is_some_and(norito::json::Value::is_null),
        "canonical nullable aad_digest must be emitted as null"
    );
    assert_eq!(
        norito::json::from_value::<SecretEnvelopeV1>(canonical.clone())
            .expect("explicit null aad_digest must decode"),
        envelope
    );

    let mut missing = canonical.clone();
    assert!(
        missing
            .as_object_mut()
            .expect("secret envelope JSON object")
            .remove("aad_digest")
            .is_some()
    );
    norito::json::from_value::<SecretEnvelopeV1>(missing)
        .expect_err("omitted aad_digest must be rejected");

    let mut unknown = canonical;
    unknown
        .as_object_mut()
        .expect("secret envelope JSON object")
        .insert("retired_v0".to_owned(), norito::json!(true));
    let error = norito::json::from_value::<SecretEnvelopeV1>(unknown)
        .expect_err("secret envelope must reject unknown fields");
    assert!(
        matches!(
            error,
            json::Error::UnknownField { ref field } if field == "retired_v0"
        ),
        "unexpected secret-envelope unknown-field rejection: {error}"
    );
}
#[cfg(feature = "json")]
#[test]
fn service_state_entry_v1_requires_explicit_nullable_fhe_metadata_and_closed_fields() {
    let entry = sample_state_entry();
    let canonical = norito::json::to_value(&entry).expect("serialize service state entry");
    for field in [
        "fhe_public_key_digest",
        "fhe_residual_multiple_bound",
        "fhe_bound_mode",
    ] {
        assert!(
            canonical
                .get(field)
                .is_some_and(norito::json::Value::is_null),
            "canonical nullable `{field}` must be emitted as null"
        );
    }
    assert_eq!(
        norito::json::from_value::<SoraServiceStateEntryV1>(canonical.clone())
            .expect("explicit null FHE metadata must decode"),
        entry
    );

    for field in [
        "fhe_public_key_digest",
        "fhe_residual_multiple_bound",
        "fhe_bound_mode",
    ] {
        let mut missing = canonical.clone();
        assert!(
            missing
                .as_object_mut()
                .expect("service state entry JSON object")
                .remove(field)
                .is_some()
        );
        norito::json::from_value::<SoraServiceStateEntryV1>(missing)
            .expect_err("omitted nullable FHE metadata must be rejected");
    }

    let mut unknown = canonical;
    unknown
        .as_object_mut()
        .expect("service state entry JSON object")
        .insert("retired_v0".to_owned(), norito::json!(true));
    let error = norito::json::from_value::<SoraServiceStateEntryV1>(unknown)
        .expect_err("service state entry must reject unknown fields");
    assert!(
        matches!(
            error,
            json::Error::UnknownField { ref field } if field == "retired_v0"
        ),
        "unexpected service-state unknown-field rejection: {error}"
    );
}
#[cfg(feature = "json")]
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the service-lease null, empty, and closed-field matrix is intentionally audited together"
)]
fn service_lease_v1_json_requires_explicit_null_empty_and_closed_fields() {
    let economics = SoraHttpServiceEconomicsV1::default();
    let lease = SoraServiceLeaseStateV1 {
        schema_version: SORA_SERVICE_LEASE_STATE_VERSION_V1,
        status: SoraServiceLeaseStatusV1::Active,
        quota_class: economics.quota_class,
        deployment_deposit: economics.deployment_deposit,
        prepaid_runtime_balance: economics.prepaid_runtime_balance,
        runtime_price_per_block: economics.runtime_price_per_block,
        storage_price_per_gib_block: economics.storage_price_per_gib_block,
        egress_price_per_mib: economics.egress_price_per_mib,
        lease_started_height: 1,
        lease_expires_height: 100,
        reporting_epoch: 1,
        settled_egress_bytes: 0,
        egress_reporter_checkpoints: Vec::new(),
        accounted_egress_bytes: 0,
        last_status_reason: None,
    };
    let volume = SoraServiceLeaseVolumeStateV1 {
        schema_version: SORA_SERVICE_LEASE_VOLUME_STATE_VERSION_V1,
        volume_name: "root".parse().expect("valid volume name"),
        kind: SoraLeaseVolumeKindV1::PersistentRootLeaseVolume,
        storage_class: StorageClass::Warm,
        mount_path: "/".to_owned(),
        max_total_bytes: 8 * 1024 * 1024 * 1024,
        lease_started_height: 1,
        lease_expires_height: 100,
        authoritative_generation: 1,
        last_materialized_sequence: None,
    };
    let checkpoint = SoraServiceLeaseEgressCheckpointV1 {
        reporting_epoch: 1,
        assignment: SoraServiceLeaseReporterAssignmentV1 {
            schema_version: SORA_SERVICE_LEASE_REPORTER_ASSIGNMENT_VERSION_V1,
            service_version: "1.0.0".to_owned(),
            placement: SoraInrouReplicaPlacementV1 {
                replica_slot: 1,
                validator_account_id: sample_account_id(202),
                peer_id: sample_peer_id(202),
                selected_guest_isa: SoraInrouGuestIsaV1::X8664,
                selected_geography_tag: None,
                selection_latency_ms: None,
            },
            placement_reconciled_at_ms: 1,
        },
        accounted_egress_bytes: 0,
        last_updated_height: 1,
        finalize_reporter: false,
        forced_finalization: false,
    };

    let lease_json = norito::json::to_value(&lease).expect("serialize service lease state");
    assert!(
        lease_json
            .get("last_status_reason")
            .is_some_and(norito::json::Value::is_null),
        "canonical lease status reason must be an explicit null"
    );
    assert_eq!(
        lease_json
            .get("egress_reporter_checkpoints")
            .and_then(norito::json::Value::as_array)
            .map(Vec::len),
        Some(0),
        "canonical empty reporter checkpoint list must be explicit"
    );
    assert_eq!(
        norito::json::from_value::<SoraServiceLeaseStateV1>(lease_json.clone())
            .expect("explicit-null, explicit-empty lease must decode"),
        lease
    );
    for field in ["last_status_reason", "egress_reporter_checkpoints"] {
        let mut missing = lease_json.clone();
        assert!(
            missing
                .as_object_mut()
                .expect("service lease JSON object")
                .remove(field)
                .is_some()
        );
        norito::json::from_value::<SoraServiceLeaseStateV1>(missing)
            .expect_err("omitted service lease V1 fields must be rejected");
    }

    let volume_json = norito::json::to_value(&volume).expect("serialize lease volume state");
    assert!(
        volume_json
            .get("last_materialized_sequence")
            .is_some_and(norito::json::Value::is_null),
        "canonical last materialized sequence must be an explicit null"
    );
    assert_eq!(
        norito::json::from_value::<SoraServiceLeaseVolumeStateV1>(volume_json.clone())
            .expect("explicit-null lease volume must decode"),
        volume
    );
    let mut missing_materialized = volume_json.clone();
    assert!(
        missing_materialized
            .as_object_mut()
            .expect("lease volume JSON object")
            .remove("last_materialized_sequence")
            .is_some()
    );
    norito::json::from_value::<SoraServiceLeaseVolumeStateV1>(missing_materialized)
        .expect_err("omitted last materialized sequence must be rejected");

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
    assert_unknown_rejected!(lease, SoraServiceLeaseStateV1, "service lease state");
    assert_unknown_rejected!(
        volume,
        SoraServiceLeaseVolumeStateV1,
        "service lease volume state"
    );
    assert_unknown_rejected!(
        checkpoint,
        SoraServiceLeaseEgressCheckpointV1,
        "service lease egress checkpoint"
    );
}
#[test]
fn service_runtime_state_validate_rejects_load_out_of_range() {
    let runtime_state = SoraServiceRuntimeStateV1 {
        schema_version: SORA_SERVICE_RUNTIME_STATE_VERSION_V1,
        service_name: "portal".parse().expect("valid name"),
        active_service_version: "2026.1".to_string(),
        health_status: SoraServiceHealthStatusV1::Healthy,
        load_factor_bps: 10_001,
        materialized_bundle_hash: sample_hash(160),
    };
    let error = runtime_state
        .validate()
        .expect_err("load factor above 10_000 bps must fail");
    assert_soracloud_invalid_field(error, "load_factor_bps");
}
zero_prehash_field_rejection_test! {
    service_runtime_state_validate_rejects_zero_prehash_digest_sentinels,
    zero_digest,
    sample_service_runtime_state();
    materialized_bundle_hash = zero_digest =>
        ("materialized_bundle_hash", "materialized bundle placeholder hash must fail admission");
}
#[test]
fn inrou_host_capability_record_validate_accepts_hosting_advert() {
    sample_inrou_host_capability_record()
        .validate()
        .expect("valid Inrou host capability advert should pass");
}
#[test]
fn model_host_capability_record_rejects_peer_from_another_account() {
    let mut capability = sample_model_host_capability_record();
    capability.peer_id = sample_peer_id(0xC4);
    let error = capability
        .validate()
        .expect_err("an HF host peer belonging to another account must fail");
    assert_soracloud_invalid_field(error, "peer_id");
}
#[test]
fn hf_placement_host_assignment_rejects_peer_from_another_account() {
    let mut placement = sample_hf_placement_record();
    placement.assigned_hosts[0].peer_id = sample_peer_id(0xC4);
    let error = placement
        .validate()
        .expect_err("an HF placement peer belonging to another account must fail");
    assert_soracloud_invalid_field(error, "peer_id");
}
#[test]
fn inrou_host_capability_record_validate_rejects_zero_capacity() {
    let mut capability = sample_inrou_host_capability_record();
    capability.max_hosted_replica_capacity = 0;
    let error = capability
        .validate()
        .expect_err("zero-capacity adverts must fail");
    assert_soracloud_invalid_field(error, "max_hosted_replica_capacity");
}
#[test]
fn inrou_host_capability_record_validate_rejects_capacity_above_v1() {
    let mut capability = sample_inrou_host_capability_record();
    capability.max_hosted_replica_capacity = SORA_INROU_HOSTED_REPLICA_CAPACITY_V1 + 1;
    let error = capability
        .validate()
        .expect_err("multi-replica Inrou host adverts must fail in V1");
    assert_soracloud_invalid_field(error, "max_hosted_replica_capacity");
    assert!(
        !capability.can_host_replicas_at(capability.advertised_at_ms),
        "invalid multi-replica adverts must never remain placement-eligible"
    );
}
#[test]
fn inrou_host_capability_record_validate_rejects_multiple_guest_isas() {
    let mut capability = sample_inrou_host_capability_record();
    capability
        .supported_guest_isas
        .insert(SoraInrouGuestIsaV1::Aarch64);
    let error = capability
        .validate()
        .expect_err("one Inrou V1 host advert cannot alias multiple guest ISAs");
    assert_soracloud_invalid_field(error, "supported_guest_isas");
    assert!(
        !capability.can_host_replicas_at(capability.advertised_at_ms),
        "invalid multi-ISA adverts must never remain placement-eligible"
    );
}
#[test]
fn inrou_host_capability_record_validate_rejects_noncanonical_peer_id() {
    let mut capability = sample_inrou_host_capability_record();
    capability.peer_id = "12D3KooWLegacyAlias".to_owned();
    let error = capability
        .validate()
        .expect_err("noncanonical Inrou peer aliases must fail");
    assert_soracloud_invalid_field(error, "peer_id");
    assert!(
        !capability.can_host_replicas_at(capability.advertised_at_ms),
        "an invalid peer route must never remain placement-eligible"
    );
}
#[test]
fn inrou_host_capability_record_rejects_peer_from_another_account() {
    let mut capability = sample_inrou_host_capability_record();
    capability.peer_id = sample_peer_id(0xD2);
    let error = capability
        .validate()
        .expect_err("a canonical peer belonging to another account must fail");
    assert_soracloud_invalid_field(error, "peer_id");
    assert!(
        !capability.can_host_replicas_at(capability.advertised_at_ms),
        "mismatched account/peer attribution must never remain placement-eligible"
    );
}
#[test]
fn inrou_service_placement_record_validate_rejects_duplicate_slots() {
    let mut placement = sample_inrou_service_placement_record();
    placement.placements.push(placement.placements[0].clone());
    let error = placement
        .validate()
        .expect_err("duplicate replica slots must fail validation");
    assert_soracloud_invalid_field(error, "placements");
}
#[test]
fn inrou_service_placement_record_rejects_peer_from_another_account() {
    let mut placement = sample_inrou_service_placement_record();
    placement.placements[0].peer_id = sample_peer_id(0xD2);
    let error = placement
        .validate()
        .expect_err("a placed peer belonging to another validator account must fail");
    assert_soracloud_invalid_field(error, "peer_id");
}
#[test]
fn inrou_replica_runtime_state_validate_rejects_missing_peer_id() {
    let mut runtime_state = sample_inrou_replica_runtime_state();
    runtime_state.peer_id.clear();
    let error = runtime_state
        .validate()
        .expect_err("empty peer_id must fail validation");
    assert!(matches!(
        error,
        SoracloudManifestError::EmptyField {
            field: "peer_id",
            ..
        }
    ));
}
#[test]
fn inrou_replica_runtime_state_rejects_peer_from_another_account() {
    let mut runtime_state = sample_inrou_replica_runtime_state();
    runtime_state.peer_id = sample_peer_id(0xD2);
    let error = runtime_state
        .validate()
        .expect_err("an Inrou runtime peer belonging to another account must fail");
    assert_soracloud_invalid_field(error, "peer_id");
}
zero_prehash_field_rejection_test! {
    inrou_replica_runtime_state_validate_rejects_zero_prehash_digest_sentinels,
    zero_digest,
    sample_inrou_replica_runtime_state();
    materialized_bundle_hash = zero_digest =>
        ("materialized_bundle_hash", "materialized bundle placeholder hash must fail admission");
}
#[test]
fn service_rollout_state_validate_rejects_promoted_partial_traffic() {
    let rollout = SoraServiceRolloutStateV1 {
        schema_version: SORA_SERVICE_ROLLOUT_STATE_VERSION_V1,
        rollout_handle: "portal:rollout:1".to_string(),
        baseline_version: "1.0.0".to_string(),
        candidate_version: "1.1.0".to_string(),
        canary_percent: 10,
        traffic_percent: 50,
        stage: SoraRolloutStageV1::Promoted,
        health_failures: 0,
        max_health_failures: 2,
        health_window_secs: 30,
        created_sequence: 1,
        updated_sequence: 1,
    };
    let error = rollout
        .validate()
        .expect_err("promoted rollouts must serve 100 percent of traffic");
    assert_soracloud_invalid_field(error, "traffic_percent");
}
fn sample_active_rollout_deployment() -> SoraServiceDeploymentStateV1 {
    let mut deployment = sample_service_deployment_state();
    deployment.active_rollout = Some(SoraServiceRolloutStateV1 {
        schema_version: SORA_SERVICE_ROLLOUT_STATE_VERSION_V1,
        rollout_handle: "portal:rollout:7".to_string(),
        baseline_version: "1.0.0".to_string(),
        candidate_version: deployment.current_service_version.clone(),
        canary_percent: 25,
        traffic_percent: 25,
        stage: SoraRolloutStageV1::Canary,
        health_failures: 0,
        max_health_failures: 2,
        health_window_secs: 30,
        created_sequence: 7,
        updated_sequence: 7,
    });
    deployment
}
#[test]
fn service_rollout_state_validate_rejects_missing_or_reused_baseline() {
    for baseline_version in ["", "1.1.0"] {
        let mut deployment = sample_active_rollout_deployment();
        deployment
            .active_rollout
            .as_mut()
            .expect("active rollout")
            .baseline_version = baseline_version.to_owned();
        let error = deployment
            .validate()
            .expect_err("baseline must be present and distinct from the candidate");
        assert_soracloud_invalid_field(error, "baseline_version");
    }
}
#[test]
fn service_deployment_state_validate_rejects_active_candidate_different_from_current() {
    let mut deployment = sample_active_rollout_deployment();
    deployment
        .active_rollout
        .as_mut()
        .expect("active rollout")
        .candidate_version = "1.2.0".to_owned();
    let error = deployment
        .validate()
        .expect_err("active candidate must be the deployment current version");
    assert_soracloud_invalid_field(error, "active_rollout.candidate_version");
}
#[test]
fn service_deployment_state_validate_rejects_zero_or_full_canary_allocations() {
    for canary_percent in [0, 100] {
        let mut deployment = sample_active_rollout_deployment();
        let rollout = deployment.active_rollout.as_mut().expect("active rollout");
        rollout.canary_percent = canary_percent;
        rollout.traffic_percent = canary_percent;
        let error = deployment
            .validate()
            .expect_err("active canary policy must use a partial nonzero allocation");
        assert_soracloud_invalid_field(error, "canary_percent");
    }
    for traffic_percent in [0, 100] {
        let mut deployment = sample_active_rollout_deployment();
        deployment
            .active_rollout
            .as_mut()
            .expect("active rollout")
            .traffic_percent = traffic_percent;
        let error = deployment
            .validate()
            .expect_err("active canary traffic must use a partial nonzero allocation");
        assert_soracloud_invalid_field(error, "traffic_percent");
    }
}
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the complete active-canary relation graph is intentionally exercised in one fixture"
)]
fn service_deployment_state_validate_requires_exact_active_canary_relation() {
    let mut deployment = SoraServiceDeploymentStateV1 {
        schema_version: SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1,
        service_name: "portal".parse().expect("valid name"),
        current_service_version: "1.1.0".to_string(),
        current_service_manifest_hash: sample_hash(170),
        current_container_manifest_hash: sample_hash(171),
        revision_count: 2,
        process_generation: 2,
        process_started_sequence: 7,
        active_rollout: Some(SoraServiceRolloutStateV1 {
            schema_version: SORA_SERVICE_ROLLOUT_STATE_VERSION_V1,
            rollout_handle: "portal:rollout:7".to_string(),
            baseline_version: "1.0.0".to_string(),
            candidate_version: "1.1.0".to_string(),
            canary_percent: 25,
            traffic_percent: 100,
            stage: SoraRolloutStageV1::Promoted,
            health_failures: 0,
            max_health_failures: 2,
            health_window_secs: 30,
            created_sequence: 7,
            updated_sequence: 7,
        }),
        last_rollout: None,
        config_generation: 0,
        secret_generation: 0,
        service_configs: BTreeMap::new(),
        service_secrets: BTreeMap::new(),
        fhe_policy_records: BTreeMap::new(),
        service_lease: None,
        lease_volume_states: Vec::new(),
    };
    let error = deployment
        .validate()
        .expect_err("active rollout must remain in canary state");
    assert_soracloud_invalid_field(error, "active_rollout.stage");

    let active_rollout = deployment.active_rollout.as_mut().expect("active rollout");
    active_rollout.stage = SoraRolloutStageV1::Canary;
    active_rollout.traffic_percent = 25;
    active_rollout.candidate_version = "1.2.0".to_owned();
    let error = deployment
        .validate()
        .expect_err("active rollout candidate must equal the current revision");
    assert_soracloud_invalid_field(error, "active_rollout.candidate_version");

    let current_version = deployment.current_service_version.clone();
    let active_rollout = deployment.active_rollout.as_mut().expect("active rollout");
    active_rollout.candidate_version = current_version.clone();
    active_rollout.baseline_version.clear();
    let error = deployment
        .validate()
        .expect_err("active rollout must name its baseline revision");
    assert_soracloud_invalid_field(error, "active_rollout.baseline_version");

    let active_rollout = deployment.active_rollout.as_mut().expect("active rollout");
    active_rollout.baseline_version = current_version;
    let error = deployment
        .validate()
        .expect_err("active rollout baseline and candidate must differ");
    assert_soracloud_invalid_field(error, "active_rollout.baseline_version");

    let active_rollout = deployment.active_rollout.as_mut().expect("active rollout");
    active_rollout.baseline_version = "1.0.0".to_owned();
    active_rollout.traffic_percent = 0;
    let error = deployment
        .validate()
        .expect_err("active canary traffic may not be zero");
    assert_soracloud_invalid_field(error, "traffic_percent");

    deployment
        .active_rollout
        .as_mut()
        .expect("active rollout")
        .traffic_percent = 25;
    deployment
        .validate()
        .expect("exact active canary relation must pass");

    #[cfg(feature = "json")]
    {
        let canonical = norito::json::to_value(&deployment).expect("serialize deployment state");
        for field in [
            "config_generation",
            "secret_generation",
            "service_configs",
            "service_secrets",
            "active_rollout",
            "last_rollout",
            "service_lease",
            "lease_volume_states",
        ] {
            let mut missing = canonical.clone();
            assert!(
                missing
                    .as_object_mut()
                    .expect("deployment JSON object")
                    .remove(field)
                    .is_some(),
                "canonical deployment must emit `{field}`"
            );
            norito::json::from_value::<SoraServiceDeploymentStateV1>(missing)
                .expect_err("deployment state must reject every omitted V1 field");
        }
        let mut unknown = canonical;
        unknown
            .as_object_mut()
            .expect("deployment JSON object")
            .insert("retired_v0".to_owned(), norito::json!(true));
        norito::json::from_value::<SoraServiceDeploymentStateV1>(unknown)
            .expect_err("deployment state must reject unknown fields");

        let rollout = deployment.active_rollout.as_ref().expect("active rollout");
        let mut missing_baseline =
            norito::json::to_value(rollout).expect("serialize rollout state");
        assert!(
            missing_baseline
                .as_object_mut()
                .expect("rollout JSON object")
                .remove("baseline_version")
                .is_some()
        );
        norito::json::from_value::<SoraServiceRolloutStateV1>(missing_baseline)
            .expect_err("rollout state must require the nullable baseline key");
    }
}
zero_prehash_field_rejection_test! {
    service_deployment_state_validate_rejects_zero_prehash_manifest_hash_sentinels,
    zero_digest,
    sample_service_deployment_state();
    current_service_manifest_hash = zero_digest =>
        ("current_service_manifest_hash", "current service manifest placeholder hash must fail admission");
    current_container_manifest_hash = zero_digest =>
        ("current_container_manifest_hash", "current container manifest placeholder hash must fail admission");
}
#[test]
fn service_audit_event_validate_rejects_zero_sequence() {
    let event = SoraServiceAuditEventV1 {
        schema_version: SORA_SERVICE_AUDIT_EVENT_VERSION_V1,
        sequence: 0,
        block_height: 1,
        block_timestamp_ms: 1,
        action: SoraServiceLifecycleActionV1::Deploy,
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
        governance_tx_hash: None,
        binding_name: None,
        state_key: None,
        config_mutations: Vec::new(),
        secret_mutations: Vec::new(),
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
        signer: sample_signer(),
    };
    let error = event
        .validate()
        .expect_err("audit sequences must be greater than zero");
    assert_soracloud_invalid_field(error, "sequence");
}
zero_prehash_field_rejection_test! {
    service_audit_event_validate_rejects_zero_prehash_digest_sentinels,
    zero_digest,
    sample_service_audit_event();
    service_manifest_hash = zero_digest =>
        ("service_manifest_hash", "service manifest placeholder hash must fail admission");
    container_manifest_hash = zero_digest =>
        ("container_manifest_hash", "container manifest placeholder hash must fail admission");
    governance_tx_hash = Some(zero_digest) =>
        ("governance_tx_hash", "governance transaction placeholder hash must fail admission");
    policy_snapshot_hash = Some(zero_digest) =>
        ("policy_snapshot_hash", "policy snapshot placeholder hash must fail admission");
    consent_evidence_hash = Some(zero_digest) =>
        ("consent_evidence_hash", "consent evidence placeholder hash must fail admission");
    config_snapshot_hash = zero_digest =>
        ("config_snapshot_hash", "config snapshot placeholder hash must fail admission");
    secret_snapshot_hash = zero_digest =>
        ("secret_snapshot_hash", "secret snapshot placeholder hash must fail admission");
}
#[test]
fn service_state_entry_validate_allows_plaintext_rows() {
    let mut entry = sample_state_entry();
    entry.encryption = SoraStateEncryptionV1::Plaintext;
    entry
        .validate()
        .expect("state entries must support plaintext bindings");
}
#[test]
fn service_state_entry_validate_rejects_fhe_residual_bound_on_non_fhe_rows() {
    let mut entry = sample_state_entry();
    entry.encryption = SoraStateEncryptionV1::ClientCiphertext;
    entry.fhe_residual_multiple_bound = Some(17);
    let error = entry
        .validate()
        .expect_err("BFV residual bounds must only annotate FHE rows");
    assert_soracloud_invalid_field(error, "fhe_residual_multiple_bound");
}
#[test]
fn service_state_entry_validate_rejects_fhe_public_key_digest_on_non_fhe_rows() {
    let mut entry = sample_state_entry();
    entry.encryption = SoraStateEncryptionV1::ClientCiphertext;
    entry.fhe_public_key_digest = Some(sample_hash(149));
    let error = entry
        .validate()
        .expect_err("BFV public-key digests must only annotate FHE rows");
    assert_soracloud_invalid_field(error, "fhe_public_key_digest");
}
#[test]
fn service_state_entry_validate_rejects_zero_fhe_public_key_digest() {
    let mut entry = sample_state_entry();
    entry.fhe_public_key_digest = Some(zero_prehash_statement_hash());
    let error = entry
        .validate()
        .expect_err("BFV public-key digest placeholders must fail admission");
    assert_zero_prehash_digest_error(&error, "fhe_public_key_digest");
}
#[test]
fn service_state_entry_validate_rejects_zero_prehash_governance_hash_sentinel() {
    let mut entry = sample_state_entry();
    entry.governance_tx_hash = zero_prehash_statement_hash();
    let error = entry
        .validate()
        .expect_err("governance transaction placeholder hash must fail admission");
    assert_zero_prehash_digest_error(&error, "governance_tx_hash");
}
#[test]
fn service_state_entry_validate_rejects_fhe_bound_mode_without_fhe_bound() {
    let mut non_fhe_entry = sample_state_entry();
    non_fhe_entry.encryption = SoraStateEncryptionV1::ClientCiphertext;
    non_fhe_entry.fhe_bound_mode = Some(BfvCiphertextBoundModeV1::ExactResidualMultiple);
    let error = non_fhe_entry
        .validate()
        .expect_err("BFV bound modes must only annotate FHE rows");
    assert_soracloud_invalid_field(error, "fhe_bound_mode");
    let mut missing_bound_entry = sample_state_entry();
    missing_bound_entry.fhe_bound_mode = Some(BfvCiphertextBoundModeV1::ExactResidualMultiple);
    let error = missing_bound_entry
        .validate()
        .expect_err("BFV bound mode must require a bound value");
    assert_soracloud_invalid_field(error, "fhe_bound_mode");
}
#[test]
fn service_state_entry_validate_rejects_over_capacity_fhe_bounds() {
    let mut missing_mode_entry = sample_state_entry();
    missing_mode_entry.fhe_public_key_digest = Some(sample_hash(150));
    missing_mode_entry.fhe_residual_multiple_bound = Some(17);
    missing_mode_entry.fhe_bound_mode = None;
    let error = missing_mode_entry
        .validate()
        .expect_err("FHE bounds must explicitly advertise their semantics");
    assert_soracloud_invalid_field(error, "fhe_bound_mode");
    let mut exact_entry = sample_state_entry();
    exact_entry.fhe_public_key_digest = Some(sample_hash(151));
    exact_entry.fhe_residual_multiple_bound = Some(u128::MAX);
    exact_entry.fhe_bound_mode = Some(BfvCiphertextBoundModeV1::ExactResidualMultiple);
    let error = exact_entry
        .validate()
        .expect_err("over-capacity exact FHE bound must be rejected");
    assert!(
        error.to_string().contains("exact residual"),
        "unexpected error: {error}"
    );
    assert_soracloud_invalid_field(error, "fhe_residual_multiple_bound");
    let mut bounded_entry = sample_state_entry();
    bounded_entry.fhe_public_key_digest = Some(sample_hash(152));
    bounded_entry.fhe_residual_multiple_bound = Some(u128::MAX);
    bounded_entry.fhe_bound_mode = Some(BfvCiphertextBoundModeV1::BoundedNoise);
    let error = bounded_entry
        .validate()
        .expect_err("bounded-noise FHE bound above capacity must be rejected");
    assert!(
        error.to_string().contains("bounded-noise"),
        "unexpected error: {error}"
    );
    assert_soracloud_invalid_field(error, "fhe_residual_multiple_bound");
}
#[test]
fn decryption_request_record_policy_snapshot_hash_is_deterministic() {
    let record = sample_decryption_request_record();
    let expected = Hash::new(Encode::encode(&record.policy));
    assert_eq!(record.policy_snapshot_hash(), expected);
    record
        .validate()
        .expect("request record should validate against its policy");
}
#[test]
fn service_audit_event_validate_requires_break_glass_reason_when_enabled() {
    let event = SoraServiceAuditEventV1 {
        schema_version: SORA_SERVICE_AUDIT_EVENT_VERSION_V1,
        sequence: 1,
        block_height: 1,
        block_timestamp_ms: 1,
        action: SoraServiceLifecycleActionV1::DecryptionRequest,
        service_name: "portal".parse().expect("valid name"),
        from_version: None,
        to_version: "1.0.0".to_string(),
        service_manifest_hash: sample_hash(174),
        container_manifest_hash: sample_hash(175),
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
        consent_evidence_hash: None,
        break_glass: Some(true),
        break_glass_reason: None,
        lease_usage: None,
        service_lease_commitment: None,
        lease_reporting_epoch_rollover: None,
        signer: sample_signer(),
    };
    let error = event
        .validate()
        .expect_err("break_glass events require a reason");
    assert_soracloud_invalid_field(error, "break_glass_reason");
}
#[test]
fn service_mailbox_message_validate_rejects_expired_message() {
    let message = SoraServiceMailboxMessageV1 {
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
        delivery_delay_blocks: 2,
        enqueue_sequence: 10,
        enqueue_height: 10,
        available_after_height: 12,
        expires_at_height: 12,
    };
    let error = message
        .validate()
        .expect_err("message expiry must be after availability");
    assert_soracloud_invalid_field(error, "expires_at_height");
}
#[test]
fn service_mailbox_message_validate_rejects_payload_commitment_mismatch() {
    let message = SoraServiceMailboxMessageV1 {
        schema_version: SORA_SERVICE_MAILBOX_MESSAGE_VERSION_V1,
        message_id: sample_hash(162),
        from_service: "portal".parse().expect("valid name"),
        from_service_version: "2026.1".to_string(),
        from_handler: "update".parse().expect("valid name"),
        to_service: "audit".parse().expect("valid name"),
        to_service_version: "2026.1".to_string(),
        to_handler: "private_update".parse().expect("valid name"),
        payload_bytes: b"ciphertext".to_vec(),
        payload_commitment: sample_hash(163),
        delivery_delay_blocks: 0,
        enqueue_sequence: 10,
        enqueue_height: 10,
        available_after_height: 10,
        expires_at_height: 12,
    };
    let error = message
        .validate()
        .expect_err("message commitment must bind the authoritative payload bytes");
    assert_soracloud_invalid_field(error, "payload_commitment");
}
#[test]
fn service_mailbox_message_validation_separates_submission_and_persisted_schedule_states() {
    let mut message = sample_service_mailbox_message();
    message
        .validate()
        .expect("canonical ledger-assigned mailbox message must validate");
    let error = message
        .validate_submission()
        .expect_err("mailbox submission must not carry a caller-selected identifier");
    assert_soracloud_invalid_field(error, "message_id");

    message.message_id = Hash::prehashed([0; Hash::LENGTH]);
    let error = message
        .validate_submission()
        .expect_err("mailbox submission must not carry ledger-bound service versions");
    assert_soracloud_invalid_field(error, "from_service_version");

    message.from_service_version.clear();
    message.to_service_version.clear();
    let error = message
        .validate_submission()
        .expect_err("mailbox submission must not carry a caller-selected schedule");
    assert_soracloud_invalid_field(error, "enqueue_sequence");

    message.enqueue_sequence = 0;
    message.available_after_height = 0;
    message.expires_at_height = 0;
    message
        .validate_submission()
        .expect("zero-sentinel mailbox submission must validate");
    message.message_id = sample_hash(162);
    message.from_service_version = "2026.1".to_owned();
    message.to_service_version = "2026.1".to_owned();
    let error = message
        .validate()
        .expect_err("persisted mailbox message requires a ledger-assigned schedule");
    assert_soracloud_invalid_field(error, "enqueue_sequence");
}
#[test]
fn service_mailbox_message_id_binds_every_immutable_field() {
    let message = sample_service_mailbox_message();
    let canonical_id = message.message_id;

    macro_rules! assert_field_bound {
        ($field:literal, $mutate:expr) => {{
            let mut changed = message.clone();
            ($mutate)(&mut changed);
            assert_ne!(
                derive_soracloud_mailbox_message_id_v1(&changed),
                canonical_id,
                "mailbox message identity must bind {}",
                $field
            );
        }};
    }
    assert_field_bound!(
        "schema_version",
        |changed: &mut SoraServiceMailboxMessageV1| {
            changed.schema_version += 1;
        }
    );
    assert_field_bound!(
        "from_service",
        |changed: &mut SoraServiceMailboxMessageV1| {
            changed.from_service = "other_source".parse().expect("valid source name");
        }
    );
    assert_field_bound!(
        "from_service_version",
        |changed: &mut SoraServiceMailboxMessageV1| {
            changed.from_service_version = "2026.2".to_owned();
        }
    );
    assert_field_bound!(
        "from_handler",
        |changed: &mut SoraServiceMailboxMessageV1| {
            changed.from_handler = "other_update".parse().expect("valid handler name");
        }
    );
    assert_field_bound!("to_service", |changed: &mut SoraServiceMailboxMessageV1| {
        changed.to_service = "other_destination".parse().expect("valid service name");
    });
    assert_field_bound!(
        "to_service_version",
        |changed: &mut SoraServiceMailboxMessageV1| {
            changed.to_service_version = "2026.2".to_owned();
        }
    );
    assert_field_bound!("to_handler", |changed: &mut SoraServiceMailboxMessageV1| {
        changed.to_handler = "other_handler".parse().expect("valid handler name");
    });
    assert_field_bound!(
        "payload_bytes",
        |changed: &mut SoraServiceMailboxMessageV1| {
            changed.payload_bytes = b"other ciphertext".to_vec();
        }
    );
    assert_field_bound!(
        "payload_commitment",
        |changed: &mut SoraServiceMailboxMessageV1| {
            changed.payload_commitment = sample_hash(163);
        }
    );
    assert_field_bound!(
        "delivery_delay_blocks",
        |changed: &mut SoraServiceMailboxMessageV1| {
            changed.delivery_delay_blocks = 1;
        }
    );
    assert_field_bound!(
        "enqueue_sequence",
        |changed: &mut SoraServiceMailboxMessageV1| {
            changed.enqueue_sequence += 1;
        }
    );
    assert_field_bound!(
        "available_after_height",
        |changed: &mut SoraServiceMailboxMessageV1| {
            changed.available_after_height += 1;
        }
    );
    assert_field_bound!(
        "expires_at_height",
        |changed: &mut SoraServiceMailboxMessageV1| {
            changed.expires_at_height += 1;
        }
    );

    let mut substituted = message;
    substituted.payload_bytes = b"attacker-selected ciphertext".to_vec();
    substituted.payload_commitment = Hash::new(&substituted.payload_bytes);
    assert_eq!(substituted.message_id, canonical_id);
    let error = substituted
        .validate()
        .expect_err("payload substitution under a canonical message id must fail");
    assert_soracloud_invalid_field(error, "message_id");
}
zero_prehash_field_rejection_test! {
    service_mailbox_message_validate_rejects_zero_prehash_digest_sentinels,
    zero_digest,
    sample_service_mailbox_message();
    message_id = zero_digest =>
        ("message_id", "message placeholder id must fail admission");
    payload_commitment = zero_digest =>
        ("payload_commitment", "payload placeholder commitment must fail admission");
}
#[test]
fn runtime_receipt_validate_rejects_uncertified_query_receipt() {
    let receipt = SoraRuntimeReceiptV1 {
        schema_version: SORA_RUNTIME_RECEIPT_VERSION_V1,
        receipt_id: sample_hash(164),
        service_name: "portal".parse().expect("valid name"),
        service_version: "2026.1".to_string(),
        handler_name: "query".parse().expect("valid name"),
        handler_class: SoraServiceHandlerClassV1::Query,
        request_commitment: sample_hash(165),
        result_commitment: sample_hash(166),
        certified_by: SoraCertifiedResponsePolicyV1::None,
        emitted_sequence: 44,
        execution_host: None,
        mailbox_message_id: None,
        journal_artifact_hash: None,
        checkpoint_artifact_hash: None,
    };
    let error = receipt
        .validate()
        .expect_err("query receipts must remain certified");
    assert_soracloud_invalid_field(error, "certified_by");
}
#[test]
fn runtime_receipt_validation_separates_submission_and_persisted_sequence_states() {
    let mut receipt = sample_runtime_receipt();
    receipt.emitted_sequence = 0;
    receipt
        .validate_submission()
        .expect("an unassigned runtime receipt is valid for ledger submission");
    let error = receipt
        .validate()
        .expect_err("a persisted runtime receipt requires a ledger-assigned sequence");
    assert_soracloud_invalid_field(error, "emitted_sequence");
    receipt.emitted_sequence = 1;
    let error = receipt
        .validate_submission()
        .expect_err("a submission must not select its authoritative sequence");
    assert_soracloud_invalid_field(error, "emitted_sequence");
}
#[test]
fn private_runtime_receipt_validation_separates_submission_and_persisted_sequence_states() {
    let mut receipt = sample_private_uploaded_model_execution_receipt();
    receipt.emitted_sequence = 0;
    receipt.emitted_block_height = 0;
    receipt
        .validate_submission()
        .expect("an unassigned private receipt is valid for ledger submission");
    let error = receipt
        .validate()
        .expect_err("a persisted private receipt requires a ledger-assigned sequence");
    assert_soracloud_invalid_field(error, "emitted_sequence");
    receipt.emitted_sequence = 1;
    let error = receipt
        .validate_submission()
        .expect_err("a private receipt submission must not select its authoritative sequence");
    assert_soracloud_invalid_field(error, "emitted_sequence");
    let error = receipt
        .validate()
        .expect_err("a persisted private receipt requires a ledger-assigned block height");
    assert_soracloud_invalid_field(error, "emitted_block_height");
    receipt.emitted_sequence = 0;
    receipt.emitted_block_height = 1;
    let error = receipt
        .validate_submission()
        .expect_err("a private receipt submission must not select its authoritative block height");
    assert_soracloud_invalid_field(error, "emitted_block_height");
}
#[test]
fn runtime_receipt_validate_rejects_invalid_host_attribution() {
    let pool_id = sample_hash(170);
    let selection_seed_hash = sample_hash(172);
    let receipt = SoraRuntimeReceiptV1 {
        schema_version: SORA_RUNTIME_RECEIPT_VERSION_V1,
        receipt_id: sample_hash(167),
        service_name: "portal".parse().expect("valid name"),
        service_version: "2026.1".to_string(),
        handler_name: "query".parse().expect("valid name"),
        handler_class: SoraServiceHandlerClassV1::Query,
        request_commitment: sample_hash(168),
        result_commitment: sample_hash(169),
        certified_by: SoraCertifiedResponsePolicyV1::AuditReceipt,
        emitted_sequence: 45,
        execution_host: Some(SoraRuntimeExecutionHostV1::HfModelHost(
            SoraRuntimeHfModelHostV1 {
                placement_id: derive_hf_placement_id_v1(pool_id, selection_seed_hash)
                    .expect("canonical sample placement id"),
                source_id: sample_hash(173),
                pool_id,
                selection_seed_hash,
                validator_account_id: sample_account_id(171),
                peer_id: " ".to_owned(),
            },
        )),
        mailbox_message_id: None,
        journal_artifact_hash: None,
        checkpoint_artifact_hash: None,
    };
    let error = receipt
        .validate()
        .expect_err("invalid host attribution must be rejected");
    assert_soracloud_invalid_field(error, "peer_id");
}
#[test]
fn deterministic_validator_host_requires_canonical_account_peer_binding() {
    let mut receipt = sample_runtime_receipt();
    receipt.execution_host = Some(SoraRuntimeExecutionHostV1::DeterministicValidator(
        SoraRuntimeDeterministicValidatorHostV1 {
            lane_id: LaneId::SINGLE,
            validator_account_id: sample_account_id(171),
            peer_id: sample_peer_id(171),
        },
    ));
    receipt
        .validate()
        .expect("matching single-signatory validator host must validate");

    let Some(SoraRuntimeExecutionHostV1::DeterministicValidator(host)) =
        receipt.execution_host.as_mut()
    else {
        unreachable!("fixture carries deterministic-validator attribution")
    };
    host.peer_id = sample_peer_id(172);
    let error = receipt
        .validate()
        .expect_err("a syntactically valid peer from another account must be rejected");
    assert_soracloud_invalid_field(error, "peer_id");
}
#[test]
fn hf_model_host_receipt_requires_canonical_account_peer_binding() {
    let mut receipt = sample_runtime_receipt();
    receipt
        .validate()
        .expect("matching single-signatory HF model host must validate structurally");
    let Some(SoraRuntimeExecutionHostV1::HfModelHost(host)) = receipt.execution_host.as_mut()
    else {
        unreachable!("fixture carries HF model-host attribution")
    };
    host.peer_id = sample_peer_id(172);
    let error = receipt
        .validate()
        .expect_err("an HF receipt peer belonging to another account must fail");
    assert_soracloud_invalid_field(error, "peer_id");
}
#[test]
fn hf_model_host_receipt_requires_canonical_placement_context() {
    let mut receipt = sample_runtime_receipt();
    let Some(SoraRuntimeExecutionHostV1::HfModelHost(host)) = receipt.execution_host.as_mut()
    else {
        unreachable!("fixture carries HF model-host attribution")
    };
    host.selection_seed_hash = sample_hash(174);
    let error = receipt
        .validate()
        .expect_err("placement id must be derived from the attributed pool and selection seed");
    assert_soracloud_invalid_field(error, "placement_id");
}
zero_prehash_field_rejection_test! {
    runtime_receipt_validate_rejects_zero_prehash_digest_sentinels,
    zero_digest,
    sample_runtime_receipt();
    receipt_id = zero_digest => ("receipt_id", "receipt placeholder id must fail admission");
    request_commitment = zero_digest =>
        ("request_commitment", "request placeholder commitment must fail admission");
    result_commitment = zero_digest =>
        ("result_commitment", "result placeholder commitment must fail admission");
    mailbox_message_id = Some(zero_digest) =>
        ("mailbox_message_id", "mailbox message placeholder id must fail admission");
    journal_artifact_hash = Some(zero_digest) =>
        ("journal_artifact_hash", "journal artifact placeholder hash must fail admission");
    checkpoint_artifact_hash = Some(zero_digest) =>
        ("checkpoint_artifact_hash", "checkpoint artifact placeholder hash must fail admission");
}
#[test]
fn runtime_receipt_validate_rejects_zero_hf_placement_digest() {
    let mut receipt = sample_runtime_receipt();
    receipt.execution_host = Some(SoraRuntimeExecutionHostV1::HfModelHost(
        SoraRuntimeHfModelHostV1 {
            placement_id: Hash::prehashed([0; Hash::LENGTH]),
            source_id: sample_hash(173),
            pool_id: sample_hash(170),
            selection_seed_hash: sample_hash(172),
            validator_account_id: sample_account_id(171),
            peer_id: sample_peer_id(171),
        },
    ));
    let error = receipt
        .validate()
        .expect_err("HF placement placeholder digest must fail validation");
    assert_soracloud_invalid_field(error, "placement_id");
}
#[cfg(feature = "json")]
#[test]
fn runtime_receipt_host_attribution_rejects_unknown_fields() {
    let mut value = norito::json::to_value(&sample_runtime_receipt())
        .expect("serialize runtime receipt with host attribution");
    value
        .get_mut("execution_host")
        .and_then(norito::json::Value::as_object_mut)
        .and_then(|host| host.get_mut("value"))
        .and_then(norito::json::Value::as_object_mut)
        .expect("nested execution host JSON object")
        .insert("retired_v0".to_owned(), norito::json!(true));
    norito::json::from_value::<SoraRuntimeReceiptV1>(value)
        .expect_err("execution host attribution must reject unknown fields");
}
#[test]
fn agent_apartment_manifest_validate_rejects_duplicate_tool_capabilities() {
    let mut manifest = sample_agent_apartment_manifest();
    manifest.tool_capabilities.push(AgentToolCapabilityV1 {
        tool: "soracloud.deploy".to_string(),
        max_invocations_per_epoch: NonZeroU32::new(1).expect("nonzero"),
        allow_network: false,
        allow_filesystem_write: false,
    });
    let error = manifest
        .validate()
        .expect_err("duplicate tool capabilities must be rejected");
    assert!(matches!(
        error,
        SoracloudManifestError::DuplicateToolCapability { .. }
    ));
}
#[cfg(feature = "json")]
#[test]
fn signed_agent_deploy_and_service_handler_v1_json_is_closed_and_requires_collections() {
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
                "{} reported the wrong unknown-field error: {error:?}",
                $label
            );
        }};
    }

    let populated = sample_agent_apartment_manifest();
    assert_unknown_rejected!(
        populated.clone(),
        AgentApartmentManifestV1,
        "agent apartment manifest"
    );
    assert_unknown_rejected!(
        populated.tool_capabilities[0].clone(),
        AgentToolCapabilityV1,
        "agent tool capability"
    );
    assert_unknown_rejected!(
        populated.spend_limits[0].clone(),
        AgentSpendLimitV1,
        "agent spend limit"
    );
    assert_unknown_rejected!(
        AgentUpgradePolicyV1::Governed,
        AgentUpgradePolicyV1,
        "agent upgrade policy"
    );
    assert_unknown_rejected!(
        SoraServiceHandlerClassV1::PrivateUpdate,
        SoraServiceHandlerClassV1,
        "service handler class"
    );
    assert_unknown_rejected!(
        SoraCertifiedResponsePolicyV1::AuditReceipt,
        SoraCertifiedResponsePolicyV1,
        "certified response policy"
    );

    let mut empty = populated;
    empty.tool_capabilities.clear();
    empty.policy_capabilities.clear();
    empty.spend_limits.clear();
    let canonical =
        norito::json::to_value(&empty).expect("serialize canonical empty apartment policy");
    assert_eq!(
        norito::json::from_value::<AgentApartmentManifestV1>(canonical.clone())
            .expect("decode explicit empty apartment policy collections"),
        empty
    );
    for field in ["tool_capabilities", "policy_capabilities", "spend_limits"] {
        assert_eq!(
            canonical
                .get(field)
                .and_then(norito::json::Value::as_array)
                .map(Vec::len),
            Some(0),
            "canonical empty apartment policy must emit `{field}`"
        );
        let mut missing = canonical.clone();
        assert!(
            missing
                .as_object_mut()
                .expect("agent apartment manifest JSON object")
                .remove(field)
                .is_some()
        );
        norito::json::from_value::<AgentApartmentManifestV1>(missing)
            .expect_err("agent apartment manifest must reject omitted V1 collections");
    }
}
#[test]
fn agent_apartment_manifest_validate_rejects_excessive_per_tx_limit() {
    let mut manifest = sample_agent_apartment_manifest();
    manifest.spend_limits[0].max_per_tx = xor_quantity_from_nanos(50_000_000);
    let error = manifest
        .validate()
        .expect_err("per-tx spend limit above daily limit must fail");
    assert_soracloud_invalid_field(error, "spend_limits.max_per_tx");
}
#[test]
fn agent_apartment_manifest_validate_rejects_zero_prehash_container_ref_sentinel() {
    let mut manifest = sample_agent_apartment_manifest();
    manifest.container.manifest_hash = zero_prehash_statement_hash();
    let error = manifest
        .validate()
        .expect_err("agent apartment container placeholder hash must fail admission");
    assert_zero_prehash_digest_error(&error, "container.manifest_hash");
}
#[test]
fn agent_apartment_manifest_validate_accepts_consistent_policy() {
    let manifest = sample_agent_apartment_manifest();
    assert!(
        manifest.validate().is_ok(),
        "valid agent apartment manifest must pass"
    );
}
#[test]
fn agent_apartment_manifest_hash_uses_canonical_encoding() {
    let manifest = sample_agent_apartment_manifest();
    assert_eq!(
        manifest.manifest_hash(),
        Hash::new(Encode::encode(&manifest))
    );
}
#[test]
fn agent_apartment_record_validation_accepts_consistent_state() {
    let record = sample_agent_apartment_record();
    assert!(
        record.validate().is_ok(),
        "valid agent apartment record must pass"
    );
}
#[test]
fn agent_apartment_record_validation_rejects_manifest_hash_mismatch() {
    let mut record = sample_agent_apartment_record();
    record.manifest_hash = sample_hash(42);
    let error = record
        .validate()
        .expect_err("manifest hash must match embedded manifest");
    assert_soracloud_invalid_field(error, "manifest_hash");
}
#[test]
fn agent_apartment_record_validation_rejects_mailbox_payload_hash_mismatch() {
    let mut record = sample_agent_apartment_record();
    record.mailbox_queue[0].payload.push_str("tampered");
    let error = record
        .validate()
        .expect_err("mailbox payload hash must match payload bytes");
    assert_soracloud_invalid_field(error, "mailbox_queue.payload_hash");
}
#[test]
fn agent_apartment_record_validation_rejects_zero_prehash_digest_sentinels() {
    let zero_digest = zero_prehash_statement_hash();
    macro_rules! assert_record_digest_rejects {
        ($field:literal, $assign:expr) => {{
            let mut record = sample_agent_apartment_record();
            $assign(&mut record, zero_digest);
            let error = record
                .validate()
                .expect_err("agent apartment placeholder digest must fail admission");
            assert_zero_prehash_digest_error(&error, $field);
        }};
    }
    assert_record_digest_rejects!(
        "manifest_hash",
        |record: &mut SoraAgentApartmentRecordV1, value| {
            record.manifest_hash = value;
        }
    );
    assert_record_digest_rejects!(
        "mailbox_queue.payload_hash",
        |record: &mut SoraAgentApartmentRecordV1, value| {
            record.mailbox_queue[0].payload_hash = value;
        }
    );
    assert_record_digest_rejects!(
        "autonomy_run_history.request_commitment",
        |record: &mut SoraAgentApartmentRecordV1, value| {
            record.autonomy_run_history[0].request_commitment = value;
        }
    );
}
#[test]
fn agent_apartment_record_validation_rejects_invalid_workflow_input_json() {
    let mut record = sample_agent_apartment_record();
    record.autonomy_run_history[0].workflow_input_json = Some("{invalid".to_string());
    let error = record
        .validate()
        .expect_err("invalid workflow_input_json must be rejected");
    assert_soracloud_invalid_field(error, "autonomy_run_history");
}
#[test]
fn agent_apartment_audit_event_validation_rejects_empty_reason() {
    let mut event = sample_agent_apartment_audit_event();
    event.reason = Some(String::new());
    let error = event
        .validate()
        .expect_err("empty optional reason must be rejected");
    assert_soracloud_invalid_field(error, "reason");
}
#[test]
fn agent_apartment_audit_event_validation_rejects_zero_prehash_digest_sentinels() {
    let zero_digest = zero_prehash_statement_hash();
    macro_rules! assert_event_digest_rejects {
        ($field:literal, $assign:expr) => {{
            let mut event = sample_agent_apartment_audit_event();
            $assign(&mut event, zero_digest);
            let error = event
                .validate()
                .expect_err("agent audit placeholder digest must fail admission");
            assert_zero_prehash_digest_error(&error, $field);
        }};
    }
    assert_event_digest_rejects!(
        "manifest_hash",
        |event: &mut SoraAgentApartmentAuditEventV1, value| {
            event.manifest_hash = value;
        }
    );
    assert_event_digest_rejects!(
        "payload_hash",
        |event: &mut SoraAgentApartmentAuditEventV1, value| {
            event.payload_hash = Some(value);
        }
    );
    assert_event_digest_rejects!(
        "result_commitment",
        |event: &mut SoraAgentApartmentAuditEventV1, value| {
            event.result_commitment = Some(value);
        }
    );
    assert_event_digest_rejects!(
        "runtime_receipt_id",
        |event: &mut SoraAgentApartmentAuditEventV1, value| {
            event.runtime_receipt_id = Some(value);
        }
    );
    assert_event_digest_rejects!(
        "journal_artifact_hash",
        |event: &mut SoraAgentApartmentAuditEventV1, value| {
            event.journal_artifact_hash = Some(value);
        }
    );
    assert_event_digest_rejects!(
        "checkpoint_artifact_hash",
        |event: &mut SoraAgentApartmentAuditEventV1, value| {
            event.checkpoint_artifact_hash = Some(value);
        }
    );
}
#[test]
fn agent_apartment_audit_event_validation_requires_execution_fields() {
    let mut event = sample_agent_apartment_audit_event();
    event.action = SoraAgentApartmentActionV1::AutonomyRunExecuted;
    event.run_id = Some("ops_agent:autonomy:9".to_owned());
    event.succeeded = Some(true);
    let error = event
        .validate()
        .expect_err("execution audit events must carry a result commitment");
    assert_soracloud_invalid_field(error, "result_commitment");
}
#[test]
fn fhe_param_set_validate_rejects_unregistered_backend() {
    let mut param_set = sample_fhe_param_set();
    param_set.backend = "fhe/bfv-rns/v2".to_string();
    let error = param_set
        .validate()
        .expect_err("first-release parameter-set admission must reject unregistered backends");
    assert_soracloud_invalid_field(error, "backend");
}
#[test]
fn fhe_param_set_validate_rejects_unsupported_scheme() {
    let mut param_set = sample_fhe_param_set();
    param_set.scheme = FheSchemeV1::Ckks;
    let error = param_set
        .validate()
        .expect_err("first-release parameter-set admission must reject non-BFV schemes");
    assert_soracloud_invalid_field(error, "scheme");
}
#[test]
fn fhe_param_set_validate_rejects_empty_modulus_chain() {
    let mut param_set = sample_fhe_param_set();
    param_set.ciphertext_modulus_bits.clear();
    let error = param_set
        .validate()
        .expect_err("empty modulus chain must be rejected");
    assert!(matches!(
        error,
        SoracloudManifestError::EmptyField {
            field: "ciphertext_modulus_bits",
            ..
        }
    ));
}
#[test]
fn fhe_param_set_validate_rejects_zero_prehash_digest_sentinels() {
    let zero_digest = zero_prehash_statement_hash();
    let mut parameter_digest = sample_fhe_param_set();
    parameter_digest.parameter_digest = zero_digest;
    let error = parameter_digest
        .validate()
        .expect_err("parameter digest placeholder must fail admission");
    assert!(error.to_string().contains("zero prehash sentinel"));
    assert_soracloud_invalid_field(error, "parameter_digest");
    let mut rns_digest = sample_fhe_param_set();
    rns_digest.rns_modulus_chain_digest = zero_digest;
    let error = rns_digest
        .validate()
        .expect_err("RNS modulus-chain digest placeholder must fail admission");
    assert!(error.to_string().contains("zero prehash sentinel"));
    assert_soracloud_invalid_field(error, "rns_modulus_chain_digest");
    let mut decomposition_digest = sample_fhe_param_set();
    decomposition_digest.key_switch_decomposition_chain_digest = zero_digest;
    let error = decomposition_digest
        .validate()
        .expect_err("key-switch decomposition digest placeholder must fail admission");
    assert!(error.to_string().contains("zero prehash sentinel"));
    assert_soracloud_invalid_field(error, "key_switch_decomposition_chain_digest");
}
#[test]
fn fhe_param_set_validate_rejects_invalid_lifecycle_order() {
    let mut param_set = sample_fhe_param_set();
    param_set.deprecation_height = Some(8_000);
    let error = param_set
        .validate()
        .expect_err("deprecation height before activation must be rejected");
    assert_soracloud_invalid_field(error, "deprecation_height");
}
#[test]
fn fhe_param_set_validate_rejects_adversarial_structural_fields() {
    let mut ascending_chain = sample_fhe_param_set();
    ascending_chain.ciphertext_modulus_bits = vec![
        NonZeroU16::new(40).expect("nonzero"),
        NonZeroU16::new(50).expect("nonzero"),
    ];
    let error = ascending_chain
        .validate()
        .expect_err("ascending modulus chains must be rejected");
    assert_soracloud_invalid_field(error, "ciphertext_modulus_bits");
    let mut plaintext_not_smaller = sample_fhe_param_set();
    plaintext_not_smaller.plaintext_modulus_bits = NonZeroU16::new(60).expect("nonzero");
    let error = plaintext_not_smaller
        .validate()
        .expect_err("plaintext modulus must be below ciphertext modulus");
    assert_soracloud_invalid_field(error, "plaintext_modulus_bits");
    let mut slot_overflow = sample_fhe_param_set();
    slot_overflow.slot_count = NonZeroU32::new(8_193).expect("nonzero");
    let error = slot_overflow
        .validate()
        .expect_err("slot count above polynomial degree must be rejected");
    assert_soracloud_invalid_field(error, "slot_count");
    let mut exhausted_depth = sample_fhe_param_set();
    exhausted_depth.max_multiplicative_depth = NonZeroU16::new(3).expect("nonzero");
    let error = exhausted_depth
        .validate()
        .expect_err("depth consuming the whole modulus chain must be rejected");
    assert_soracloud_invalid_field(error, "max_multiplicative_depth");
    let evaluator_budget = BfvEvaluationBudget::exact_evaluator_v1();
    let mut over_evaluator_depth = sample_fhe_param_set();
    over_evaluator_depth.ciphertext_modulus_bits =
        vec![
            NonZeroU16::new(60).expect("nonzero");
            usize::from(evaluator_budget.max_multiplicative_depth) + 2
        ];
    over_evaluator_depth.max_multiplicative_depth =
        NonZeroU16::new(evaluator_budget.max_multiplicative_depth + 1).expect("nonzero");
    let error = over_evaluator_depth
        .validate()
        .expect_err("depth above exact evaluator budget must be rejected");
    assert_soracloud_invalid_field(error, "max_multiplicative_depth");
}
#[test]
fn fhe_param_set_validate_rejects_adversarial_lifecycle_claims() {
    let mut proposed_with_withdraw = sample_fhe_param_set();
    proposed_with_withdraw.lifecycle = FheParamLifecycleV1::Proposed;
    let error = proposed_with_withdraw
        .validate()
        .expect_err("proposed parameter sets cannot carry withdrawal metadata");
    assert_soracloud_invalid_field(error, "lifecycle");
    let mut active_without_activation = sample_fhe_param_set();
    active_without_activation.activation_height = None;
    active_without_activation.deprecation_height = None;
    active_without_activation.withdraw_height = None;
    let error = active_without_activation
        .validate()
        .expect_err("active parameter sets must declare activation height");
    assert_soracloud_invalid_field(error, "lifecycle");
    let mut withdrawn_without_withdraw_height = sample_fhe_param_set();
    withdrawn_without_withdraw_height.lifecycle = FheParamLifecycleV1::Withdrawn;
    withdrawn_without_withdraw_height.withdraw_height = None;
    let error = withdrawn_without_withdraw_height
        .validate()
        .expect_err("withdrawn parameter sets must carry withdraw height");
    assert_soracloud_invalid_field(error, "lifecycle");
    let mut withdraw_before_deprecation = sample_fhe_param_set();
    withdraw_before_deprecation.withdraw_height = Some(20_000);
    let error = withdraw_before_deprecation
        .validate()
        .expect_err("withdraw height must be after deprecation height");
    assert_soracloud_invalid_field(error, "withdraw_height");
}
#[test]
fn validation_helper_schema_version_preserves_error_details() {
    assert_eq!(validate_schema_version("test manifest", 1, 1), Ok(()));
    assert_eq!(
        validate_schema_version("test manifest", 2, 1),
        Err(SoracloudManifestError::UnsupportedVersion {
            manifest: "test manifest",
            expected: 1,
            found: 2,
        })
    );
}
#[test]
fn validation_helper_nonblank_field_rejects_only_blank_text() {
    assert_eq!(
        validate_nonblank_field("test manifest", "name", " value "),
        Ok(())
    );
    assert_eq!(
        validate_nonblank_field("test manifest", "name", " \t\n"),
        Err(SoracloudManifestError::EmptyField {
            manifest: "test manifest",
            field: "name",
        })
    );
}
#[test]
fn validation_helper_optional_nonempty_accepts_absent_values() {
    assert_eq!(
        validate_optional_nonempty("test manifest", "name", None),
        Ok(())
    );
    assert_eq!(
        validate_optional_nonempty("test manifest", "name", Some("value")),
        Ok(())
    );
    assert_eq!(
        validate_optional_nonempty("test manifest", "name", Some("  ")),
        Err(SoracloudManifestError::EmptyField {
            manifest: "test manifest",
            field: "name",
        })
    );
}
#[test]
fn validation_helper_invalid_field_preserves_error_details_and_message() {
    let error = invalid_field("test manifest", "name", "must be canonical");
    assert_eq!(
        error,
        SoracloudManifestError::InvalidField {
            manifest: "test manifest",
            field: "name",
            reason: "must be canonical".to_owned(),
        }
    );
    assert_eq!(
        error.to_string(),
        "test manifest field `name` is invalid: must be canonical"
    );
}
