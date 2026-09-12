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
fn container_validate_rejects_lossy_config_export_path_collisions() {
    for relative_path in [
        "runtime/a:b.json",
        "runtime/a?b.json",
        "runtime/a b.json",
        "runtime/café.json",
    ] {
        let mut container = sample_container();
        container.required_config_names = vec!["runtime/theme".to_string()];
        container.config_exports = vec![SoraConfigExportV1 {
            config_name: "runtime/theme".to_string(),
            target: SoraConfigExportTargetV1::File(relative_path.to_string()),
        }];
        let error = container
            .validate()
            .expect_err("noncanonical config export spelling must fail admission");
        assert_soracloud_invalid_field(error, "config_exports");
    }
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
        placement_targets: BTreeSet::new(),
        route: Some(SoraRouteTargetV1 {
            host: "wallet.sora".to_string(),
            path_prefix: "/".to_string(),
            service_port: NonZeroU16::new(8080).expect("nonzero"),
            visibility: SoraRouteVisibilityV1::Public,
            tls_mode: SoraTlsModeV1::Required,
        }),
        rollout: SoraRolloutPolicyV1 {
            canary_percent: 0,
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

fn assert_service_json_unknown_field<T>(value: &T, label: &str)
where
    T: norito::json::JsonSerialize + norito::json::JsonDeserialize + core::fmt::Debug,
{
    let serialize_message = format!("serialize {label}");
    let object_message = format!("{label} JSON object");
    let unknown_message = format!("{label} must reject unknown fields");
    let mut value = norito::json::to_value(value).expect(&serialize_message);
    value
        .as_object_mut()
        .expect(&object_message)
        .insert("retired_v0".to_owned(), norito::json!(true));
    norito::json::from_value::<T>(value).expect_err(&unknown_message);
}

#[test]
fn staged_service_and_container_v1_records_reject_unknown_fields() {
    let service = sample_service(vec![sample_binding("session")]);
    assert_service_json_unknown_field::<SoraServiceManifestV1>(&service, "service manifest");
    assert_service_json_unknown_field::<SoraContainerManifestRefV1>(
        &service.container,
        "container manifest reference",
    );
    assert_service_json_unknown_field::<SoraRouteTargetV1>(
        &service.route.clone().expect("sample route"),
        "route target",
    );
    assert_service_json_unknown_field::<SoraRolloutPolicyV1>(&service.rollout, "rollout policy");
    assert_service_json_unknown_field::<SoraHttpServiceEconomicsV1>(
        &service.economics.clone(),
        "service economics",
    );
    assert_service_json_unknown_field::<SoraStateBindingV1>(
        &service.state_bindings[0].clone(),
        "state binding",
    );
    assert_service_json_unknown_field::<SoraLeaseVolumeBindingV1>(
        &SoraLeaseVolumeBindingV1 {
            volume_name: "root_disk".parse().expect("valid name"),
            kind: SoraLeaseVolumeKindV1::PersistentRootLeaseVolume,
            storage_class: StorageClass::Warm,
            mount_path: "/".to_owned(),
            max_total_bytes: NonZeroU64::new(1024).expect("nonzero"),
        },
        "lease volume binding",
    );
    assert_service_json_unknown_field::<SoraServiceHandlerV1>(
        &service.handlers[0].clone(),
        "service handler",
    );
    assert_service_json_unknown_field::<SoraMailboxContractV1>(
        &SoraMailboxContractV1 {
            queue_name: "updates".parse().expect("valid name"),
            max_pending_messages: NonZeroU32::new(8).expect("nonzero"),
            max_message_bytes: NonZeroU64::new(1024).expect("nonzero"),
            retention_blocks: NonZeroU32::new(64).expect("nonzero"),
        },
        "mailbox contract",
    );
    assert_service_json_unknown_field::<SoraArtifactRefV1>(
        &service.artifacts[0].clone(),
        "artifact reference",
    );

    let container = sample_container();
    assert_service_json_unknown_field::<SoraNetworkAllowlistEntryV1>(
        &SoraNetworkAllowlistEntryV1::new("api.sora.org", [443]),
        "network allowlist entry",
    );
    assert_service_json_unknown_field::<SoraCapabilityPolicyV1>(
        &container.capabilities.clone(),
        "capability policy",
    );
    assert_service_json_unknown_field::<SoraResourceLimitsV1>(
        &container.resources,
        "resource limits",
    );
    assert_service_json_unknown_field::<SoraLifecycleHooksV1>(
        &container.lifecycle.clone(),
        "lifecycle hooks",
    );
    assert_service_json_unknown_field::<SoraConfigExportV1>(
        &SoraConfigExportV1 {
            config_name: "runtime/theme".to_owned(),
            target: SoraConfigExportTargetV1::File("runtime/theme.json".to_owned()),
        },
        "config export",
    );

    let bundle_container = sample_container();
    let mut bundle_service = sample_service(vec![sample_binding("session")]);
    bundle_service.container.manifest_hash = Hash::new(Encode::encode(&bundle_container));
    assert_service_json_unknown_field::<SoraDeploymentBundleV1>(
        &SoraDeploymentBundleV1 {
            schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
            container: bundle_container,
            service: bundle_service,
        },
        "deployment bundle",
    );
}
#[test]
fn capability_policy_norito_rejects_retired_wallet_signing_layout() {
    #[derive(Encode)]
    struct RetiredWalletSigningCapabilityPolicyV1 {
        network: SoraNetworkPolicyV1,
        // Retired boolean wire octet; only 0 and 1 are constructed below.
        allow_wallet_signing: u8,
        allow_state_writes: bool,
        allow_model_inference: bool,
        allow_model_training: bool,
    }

    let capabilities = sample_container().capabilities;
    let retired = RetiredWalletSigningCapabilityPolicyV1 {
        network: capabilities.network,
        allow_wallet_signing: u8::from(false),
        allow_state_writes: capabilities.allow_state_writes,
        allow_model_inference: capabilities.allow_model_inference,
        allow_model_training: capabilities.allow_model_training,
    };
    let bytes = retired.encode();
    assert!(
        SoraCapabilityPolicyV1::decode_all(&mut bytes.as_slice()).is_err(),
        "first-release capability policy must reject the retired wallet-signing Norito layout"
    );

    // The original negative case above retains its exact five-field payload.
    // Check all boolean combinations against the original bool field layout.
    for flags in 0_u8..16 {
        let candidate = RetiredWalletSigningCapabilityPolicyV1 {
            network: retired.network.clone(),
            allow_wallet_signing: flags & 1,
            allow_state_writes: flags & 2 != 0,
            allow_model_inference: flags & 4 != 0,
            allow_model_training: flags & 8 != 0,
        };
        let original_bool_layout = (
            candidate.network.clone(),
            flags & 1 != 0,
            flags & 2 != 0,
            flags & 4 != 0,
            flags & 8 != 0,
        );
        assert_eq!(candidate.encode(), original_bool_layout.encode());
    }
}

#[test]
fn capability_policy_json_rejects_retired_wallet_signing_field() {
    let mut value = norito::json::to_value(&sample_container().capabilities)
        .expect("serialize capability policy");
    value
        .as_object_mut()
        .expect("capability policy object")
        .insert("allow_wallet_signing".to_owned(), Value::Bool(false));
    norito::json::from_value::<SoraCapabilityPolicyV1>(value)
        .expect_err("first-release capability policy must reject wallet-signing compatibility");
}

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
fn service_validate_rejects_ciphertext_update_without_mailbox() {
    let mut manifest = sample_service(vec![sample_binding("session")]);
    manifest.handlers[3].mailbox = None;
    let error = manifest
        .validate()
        .expect_err("ciphertext update handlers require a mailbox");
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
            mount_path: "/var/lib/soracloud/volumes/index_state".to_string(),
            max_total_bytes: NonZeroU64::new(50 * 1024 * 1024 * 1024).expect("nonzero"),
        },
        SoraLeaseVolumeBindingV1 {
            volume_name: "sealed_state".parse().expect("valid name"),
            kind: SoraLeaseVolumeKindV1::ConfidentialLeaseVolume,
            storage_class: StorageClass::Hot,
            mount_path: "/var/lib/soracloud/volumes/sealed_state".to_string(),
            max_total_bytes: NonZeroU64::new(1024 * 1024).expect("nonzero"),
        },
    ];
    assert!(
        manifest.validate().is_ok(),
        "http services should validate with route + lease volumes and no deterministic handlers"
    );
}
#[test]
fn service_validate_rejects_more_non_root_volumes_than_the_inrou_launcher_can_attach() {
    let mut manifest = sample_service(Vec::new());
    manifest.execution_plane = SoraServiceExecutionPlaneV1::HttpService;
    manifest.state_bindings.clear();
    manifest.handlers.clear();
    manifest.artifacts.clear();
    manifest.lease_volumes = (0..=SORA_INROU_DATA_VOLUME_MAX_COUNT_V1)
        .map(|index| {
            let volume_name: Name = format!("volume_{index:02}").parse().expect("valid name");
            SoraLeaseVolumeBindingV1 {
                mount_path: sora_inrou_data_volume_mount_path_v1(&volume_name)
                    .expect("canonical mount path"),
                volume_name,
                kind: SoraLeaseVolumeKindV1::ServiceLeaseVolume,
                storage_class: StorageClass::Warm,
                max_total_bytes: NonZeroU64::new(8 * 1024 * 1024).expect("nonzero"),
            }
        })
        .collect();

    let error = manifest
        .validate()
        .expect_err("the data model must reject volumes the launcher cannot attach");
    assert_soracloud_invalid_field(error, "lease_volumes");
}
#[test]
fn lease_volume_binding_role_semantics_match_kind() {
    let root = SoraLeaseVolumeBindingV1 {
        volume_name: "root_disk".parse().expect("valid name"),
        kind: SoraLeaseVolumeKindV1::PersistentRootLeaseVolume,
        storage_class: StorageClass::Warm,
        mount_path: "/".to_string(),
        max_total_bytes: NonZeroU64::new(8 * 1024 * 1024).expect("nonzero"),
    };
    let data = SoraLeaseVolumeBindingV1 {
        volume_name: "index_state".parse().expect("valid name"),
        kind: SoraLeaseVolumeKindV1::ServiceLeaseVolume,
        storage_class: StorageClass::Warm,
        mount_path: "/var/lib/soracloud/volumes/index_state".to_string(),
        max_total_bytes: NonZeroU64::new(8 * 1024 * 1024).expect("nonzero"),
    };
    assert!(root.is_root_volume());
    assert!(!root.is_data_volume());
    assert!(!data.is_root_volume());
    assert!(data.is_data_volume());
}
#[test]
fn non_root_lease_volumes_require_the_exact_canonical_guest_mount() {
    let volume_name: Name = "index_state".parse().expect("valid name");
    let canonical_path =
        sora_inrou_data_volume_mount_path_v1(&volume_name).expect("portable volume name");
    assert_eq!(canonical_path, "/var/lib/soracloud/volumes/index_state");
    let binding = SoraLeaseVolumeBindingV1 {
        volume_name,
        kind: SoraLeaseVolumeKindV1::ServiceLeaseVolume,
        storage_class: StorageClass::Warm,
        mount_path: canonical_path,
        max_total_bytes: NonZeroU64::new(8 * 1024 * 1024).expect("nonzero"),
    };
    binding.validate().expect("canonical mount must validate");

    for invalid_path in [
        "/",
        "/etc",
        "/var/lib/soracloud",
        SORA_INROU_DATA_VOLUME_MOUNT_ROOT_V1,
        "/var/lib/soracloud/volumes/other",
        "/var/lib/soracloud/volumes/index_state/nested",
    ] {
        let mut invalid = binding.clone();
        invalid.mount_path = invalid_path.to_owned();
        let error = invalid
            .validate()
            .expect_err("noncanonical data-volume mount must fail admission");
        assert_soracloud_invalid_field(error, "mount_path");
    }

    for invalid_name in [
        "..",
        "nested/name",
        "café",
        "bad%specifier",
        "CON",
        "sixteen_chars_xx",
    ] {
        let mut invalid = binding.clone();
        invalid.volume_name = invalid_name.parse().expect("valid generic Name");
        let error = invalid
            .validate()
            .expect_err("volume names must be safe guest path components");
        assert_soracloud_invalid_field(error, "volume_name");
    }
}
#[test]
fn persistent_root_lease_volume_reserves_the_guest_root() {
    let mut root = SoraLeaseVolumeBindingV1 {
        volume_name: "root_disk".parse().expect("valid name"),
        kind: SoraLeaseVolumeKindV1::PersistentRootLeaseVolume,
        storage_class: StorageClass::Warm,
        mount_path: "/".to_owned(),
        max_total_bytes: NonZeroU64::new(8 * 1024 * 1024).expect("nonzero"),
    };
    root.validate()
        .expect("root volume must mount at guest root");
    root.mount_path = "/var/lib/soracloud/volumes/root_disk".to_owned();
    let error = root
        .validate()
        .expect_err("root volume must not mount in the data-volume subtree");
    assert_soracloud_invalid_field(error, "mount_path");
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
        mount_path: "/var/lib/soracloud/volumes/index_state".to_string(),
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
        mount_path: "/var/lib/soracloud/volumes/index_state".to_string(),
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
fn deployment_source_compatibility_reuses_cross_manifest_admission_rules() {
    let mut container = sample_container();
    let service = sample_service(vec![sample_binding("session")]);
    SoraDeploymentBundleV1::validate_source_compatibility(
        container.runtime,
        &container.capabilities,
        container.resources,
        &container.lifecycle,
        &service,
    )
    .expect("compatible source components");

    container.capabilities.allow_state_writes = false;
    let error = SoraDeploymentBundleV1::validate_source_compatibility(
        container.runtime,
        &container.capabilities,
        container.resources,
        &container.lifecycle,
        &service,
    )
    .expect_err("source compatibility must enforce state-write capabilities before admission");
    assert_soracloud_invalid_field(error, "container.capabilities.allow_state_writes");
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
        mount_path: "/var/lib/soracloud/volumes/index_state".to_string(),
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
fn deployment_bundle_validate_accepts_inrou_http_service_without_login_surface() {
    let mut container = sample_container();
    container.runtime = SoraContainerRuntimeV1::Inrou;
    container.entrypoint = "/app/bin/service".to_string();
    container.inrou = Some(sample_inrou_manifest());
    container.capabilities.network = SoraNetworkPolicyV1::Isolated;
    let container_hash = Hash::new(Encode::encode(&container));
    let mut service = sample_service(Vec::new());
    service.execution_plane = SoraServiceExecutionPlaneV1::HttpService;
    service.rollout.canary_percent = 0;
    service.replicas = NonZeroU16::new(1).expect("nonzero");
    service.placement_targets = sample_inrou_placement_targets(service.replicas.get());
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
fn admitted_inrou_bundle_requires_identity_bound_targets_for_every_replica() {
    let (mut bundle, _) = sample_hosted_active_bundle_and_deployment();
    bundle.service.placement_targets.clear();
    let error = bundle
        .validate_for_admission()
        .expect_err("admitted Inrou bundles must not omit operator-preseed targets");
    assert_soracloud_invalid_field(error, "service.placement_targets");

    bundle.service.placement_targets = sample_inrou_placement_targets(2);
    let error = bundle
        .validate_for_admission()
        .expect_err("one identity-bound target per replica is required");
    assert_soracloud_invalid_field(error, "service.placement_targets");
}
#[test]
fn non_inrou_bundle_rejects_placement_targets() {
    let container = sample_container();
    let mut service = sample_service(vec![sample_binding("session")]);
    service.container.manifest_hash = Hash::new(Encode::encode(&container));
    service.placement_targets = sample_inrou_placement_targets(service.replicas.get());
    let error = SoraDeploymentBundleV1 {
        schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
        container,
        service,
    }
    .validate_for_admission()
    .expect_err("placement targets are an Inrou-only admission surface");
    assert_soracloud_invalid_field(error, "service.placement_targets");
}
#[test]
fn inrou_placement_target_accepts_independent_validator_and_consensus_keys() {
    let target = SoraInrouPlacementTargetV1 {
        validator_account_id: sample_account_id(0x91),
        peer_id: sample_bls_peer_id(0xB1),
    };
    target
        .validate()
        .expect("Taira's validator account signer and BLS consensus peer are distinct key roles");

    let mut malformed_peer = target.clone();
    malformed_peer.peer_id = "not-a-canonical-peer".to_owned();
    let error = malformed_peer
        .validate()
        .expect_err("noncanonical peers must fail data-model validation");
    assert_soracloud_invalid_field(error, "peer_id");

    let members = [0x92_u8, 0x93]
        .into_iter()
        .map(|seed| {
            crate::account::MultisigMember::new(
                sample_ed25519_keypair(seed).public_key().clone(),
                1,
            )
            .expect("valid multisig member")
        })
        .collect();
    let policy = crate::account::MultisigPolicy::new(2, members).expect("valid multisig policy");
    let multisig = SoraInrouPlacementTargetV1 {
        validator_account_id: AccountId::new_multisig(policy),
        peer_id: sample_bls_peer_id(0xB2),
    };
    let error = multisig
        .validate()
        .expect_err("validator placement identities must remain single-signatory");
    assert_soracloud_invalid_field(error, "validator_account_id");
}
#[test]
fn service_manifest_rejects_reused_inrou_target_identities() {
    let mut service = sample_service(Vec::new());
    let first = SoraInrouPlacementTargetV1 {
        validator_account_id: sample_account_id(0x94),
        peer_id: sample_bls_peer_id(0xB4),
    };
    service.placement_targets = BTreeSet::from([
        first.clone(),
        SoraInrouPlacementTargetV1 {
            validator_account_id: first.validator_account_id,
            peer_id: sample_bls_peer_id(0xB5),
        },
    ]);
    let error = service
        .validate()
        .expect_err("one validator account must not alias multiple preseed targets");
    assert_soracloud_invalid_field(error, "placement_targets");

    service.placement_targets = BTreeSet::from([
        SoraInrouPlacementTargetV1 {
            validator_account_id: sample_account_id(0x95),
            peer_id: first.peer_id.clone(),
        },
        SoraInrouPlacementTargetV1 {
            validator_account_id: sample_account_id(0x96),
            peer_id: first.peer_id,
        },
    ]);
    let error = service
        .validate()
        .expect_err("one peer must not alias multiple preseed targets");
    assert_soracloud_invalid_field(error, "placement_targets");
}
#[test]
fn deployment_bundle_admission_accepts_exact_inrou_lifecycle_grace_ceiling() {
    let mut container = sample_container();
    container.runtime = SoraContainerRuntimeV1::Inrou;
    container.entrypoint = "/app/bin/service".to_owned();
    container.inrou = Some(sample_inrou_manifest());
    container.capabilities.network = SoraNetworkPolicyV1::Isolated;
    container.lifecycle.start_grace_secs =
        NonZeroU32::new(SORA_INROU_LIFECYCLE_GRACE_MAX_SECS_V1).expect("nonzero ceiling");
    container.lifecycle.stop_grace_secs =
        NonZeroU32::new(SORA_INROU_LIFECYCLE_GRACE_MAX_SECS_V1).expect("nonzero ceiling");
    let container_hash = Hash::new(Encode::encode(&container));
    let mut service = sample_service(Vec::new());
    service.execution_plane = SoraServiceExecutionPlaneV1::HttpService;
    service.rollout.canary_percent = 0;
    service.replicas = NonZeroU16::new(1).expect("nonzero");
    service.placement_targets = sample_inrou_placement_targets(service.replicas.get());
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

    bundle
        .validate_for_admission()
        .expect("the exact Inrou V1 lifecycle ceiling must remain admissible");
}
#[test]
fn deployment_bundle_admission_rejects_inrou_lifecycle_grace_above_ceiling_only() {
    let above_ceiling = SORA_INROU_LIFECYCLE_GRACE_MAX_SECS_V1
        .checked_add(1)
        .expect("the Inrou lifecycle ceiling has a +1 rejection boundary");
    let canonical = {
        let mut container = sample_container();
        container.runtime = SoraContainerRuntimeV1::Inrou;
        container.entrypoint = "/app/bin/service".to_owned();
        container.inrou = Some(sample_inrou_manifest());
        container.capabilities.network = SoraNetworkPolicyV1::Isolated;
        container
    };
    let admission_bundle = |container: SoraContainerManifestV1| {
        let container_hash = Hash::new(Encode::encode(&container));
        let mut service = sample_service(Vec::new());
        service.execution_plane = SoraServiceExecutionPlaneV1::HttpService;
        service.rollout.canary_percent = 0;
        service.replicas = NonZeroU16::new(1).expect("nonzero");
        service.placement_targets = sample_inrou_placement_targets(service.replicas.get());
        service.container.manifest_hash = container_hash;
        service.state_bindings.clear();
        service.handlers.clear();
        service.artifacts.clear();
        service.lease_volumes = sample_inrou_lease_volumes();
        SoraDeploymentBundleV1 {
            schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
            container,
            service,
        }
    };

    let mut above_start = canonical.clone();
    above_start.lifecycle.start_grace_secs =
        NonZeroU32::new(above_ceiling).expect("nonzero +1 boundary");
    let error = admission_bundle(above_start)
        .validate_for_admission()
        .expect_err("one second above the Inrou V1 startup ceiling must fail admission");
    assert_soracloud_invalid_field(error, "lifecycle.start_grace_secs");

    let mut above_stop = canonical;
    above_stop.lifecycle.stop_grace_secs =
        NonZeroU32::new(above_ceiling).expect("nonzero +1 boundary");
    let error = admission_bundle(above_stop)
        .validate_for_admission()
        .expect_err("one second above the Inrou V1 shutdown ceiling must fail admission");
    assert_soracloud_invalid_field(error, "lifecycle.stop_grace_secs");

    let mut ivm = sample_container();
    ivm.lifecycle.start_grace_secs = NonZeroU32::new(above_ceiling).expect("nonzero +1 boundary");
    ivm.lifecycle.stop_grace_secs = NonZeroU32::new(above_ceiling).expect("nonzero +1 boundary");
    let ivm_hash = Hash::new(Encode::encode(&ivm));
    let mut ivm_service = sample_service(vec![sample_binding("session")]);
    ivm_service.container.manifest_hash = ivm_hash;
    SoraDeploymentBundleV1 {
        schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
        container: ivm,
        service: ivm_service,
    }
    .validate_for_admission()
    .expect("the Inrou-only lifecycle ceiling must not change IVM admission");
}
#[test]
fn inrou_manifest_json_rejects_retired_ssh_authorized_keys() {
    let mut manifest = norito::json::to_value(&sample_inrou_manifest())
        .expect("serialize canonical Inrou manifest");
    manifest
        .as_object_mut()
        .expect("Inrou manifest object")
        .insert("ssh_authorized_keys".to_owned(), norito::json!([]));
    norito::json::from_value::<SoraInrouManifestV1>(manifest)
        .expect_err("Inrou V1 must reject the retired SSH login surface");
}
#[test]
fn deployment_bundle_validate_accepts_replica_private_inrou_http_service() {
    let mut container = sample_container();
    container.runtime = SoraContainerRuntimeV1::Inrou;
    container.entrypoint = "/app/bin/service".to_string();
    container.inrou = Some(sample_inrou_manifest());
    container.capabilities.network = SoraNetworkPolicyV1::Isolated;
    let container_hash = Hash::new(Encode::encode(&container));
    let mut service = sample_service(Vec::new());
    service.execution_plane = SoraServiceExecutionPlaneV1::HttpService;
    service.rollout.canary_percent = 0;
    service.replicas = NonZeroU16::new(3).expect("nonzero");
    service.placement_targets = sample_inrou_placement_targets(service.replicas.get());
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
    bundle
        .validate_for_admission()
        .expect("replicated Inrou services use distinct mutable disks for every replica");
}

#[test]
fn deployment_bundle_rejects_inrou_canary_state_split() {
    let mut container = sample_container();
    container.runtime = SoraContainerRuntimeV1::Inrou;
    container.entrypoint = "/app/bin/service".to_string();
    container.inrou = Some(sample_inrou_manifest());
    container.capabilities.network = SoraNetworkPolicyV1::Isolated;
    let container_hash = Hash::new(Encode::encode(&container));
    let mut service = sample_service(Vec::new());
    service.execution_plane = SoraServiceExecutionPlaneV1::HttpService;
    service.rollout.canary_percent = 25;
    service.placement_targets = sample_inrou_placement_targets(service.replicas.get());
    service.container.manifest_hash = container_hash;
    service.state_bindings.clear();
    service.handlers.clear();
    service.artifacts.clear();
    service.lease_volumes = sample_inrou_lease_volumes();
    let error = SoraDeploymentBundleV1 {
        schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
        container,
        service,
    }
    .validate_for_admission()
    .expect_err("Inrou canaries must fail before two revision-private disk sets are admitted");
    assert_soracloud_invalid_field(error, "service.rollout.canary_percent");
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
fn container_validate_rejects_allowlisted_inrou_network_egress() {
    let mut container = sample_container();
    container.runtime = SoraContainerRuntimeV1::Inrou;
    container.entrypoint = "/app/bin/service".to_owned();
    container.inrou = Some(sample_inrou_manifest());
    container.capabilities.network =
        SoraNetworkPolicyV1::Allowlist(vec![SoraNetworkAllowlistEntryV1::new(
            "api.sora.org",
            [443],
        )]);
    let error = container
        .validate()
        .expect_err("Inrou V1 must reject user-space allowlist egress");
    assert_soracloud_invalid_field(error, "capabilities.network");
}
#[test]
fn network_allowlists_require_exact_lowercase_hosts() {
    let entry = SoraNetworkAllowlistEntryV1::new("api.sora.org", [443]);
    assert!(entry.matches_host("api.sora.org"));
    assert!(!entry.matches_host("API.SORA.ORG"));

    let mut apartment = sample_agent_apartment_manifest();
    apartment.network_egress =
        SoraNetworkPolicyV1::Allowlist(vec![SoraNetworkAllowlistEntryV1::new(
            "API.SORA.ORG",
            [443],
        )]);
    let error = apartment
        .validate()
        .expect_err("agent allowlist host case aliases must fail closed");
    assert_soracloud_invalid_field(error, "network_egress");
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
fn inrou_guest_image_validate_accepts_published_artifact() {
    let mut image = sample_inrou_manifest()
        .guest_images
        .get(&SoraInrouGuestIsaV1::X8664)
        .cloned()
        .expect("x86_64 fixture");
    image.published_artifact = sample_published_inrou_guest_image_artifact(0xAA);
    assert!(
        image.validate().is_ok(),
        "canonical published artifact refs should validate"
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
fn inrou_guest_image_source_fields_validate_before_publication() {
    SoraInrouGuestImageV1::validate_source_fields(
        "/inrou/x86_64/vmlinux",
        "/inrou/x86_64/rootfs.ext4",
        Some("/inrou/x86_64/initrd.img"),
    )
    .expect("canonical unpublished guest-image source fields");
    let error = SoraInrouGuestImageV1::validate_source_fields(
        "/inrou/x86_64/VMLINUX",
        "/inrou/x86_64/vmlinux",
        None,
    )
    .expect_err("case-folded member collisions must fail before publication");
    assert_soracloud_invalid_field(error, "rootfs_image_path");
}
#[test]
fn inrou_guest_image_norito_rejects_retired_optional_artifact_layout() {
    #[derive(Encode)]
    struct RetiredOptionalArtifactGuestImageV1 {
        kernel_image_path: String,
        rootfs_image_path: String,
        initrd_image_path: Option<String>,
        published_artifact: Option<SoraPublishedInrouGuestImageArtifactV1>,
    }

    for published_artifact in [
        None,
        Some(sample_published_inrou_guest_image_artifact(0x91)),
    ] {
        let retired = RetiredOptionalArtifactGuestImageV1 {
            kernel_image_path: "/inrou/x86_64/vmlinux".to_owned(),
            rootfs_image_path: "/inrou/x86_64/rootfs.ext4".to_owned(),
            initrd_image_path: None,
            published_artifact,
        };
        let bytes = retired.encode();
        assert!(
            SoraInrouGuestImageV1::decode_all(&mut bytes.as_slice()).is_err(),
            "admitted V1 must reject the retired optional-artifact Norito layout"
        );
    }
}
#[test]
fn published_inrou_artifact_norito_rejects_retired_storage_identifier_layout() {
    #[derive(Encode)]
    struct RetiredStorageIdentifierArtifactV1 {
        manifest_digest_hex: String,
        content_cid: String,
        manifest_id_hex: Option<String>,
    }

    for manifest_id_hex in [None, Some("91".repeat(32))] {
        let retired = RetiredStorageIdentifierArtifactV1 {
            manifest_digest_hex: "91".repeat(32),
            content_cid: encode_lowercase_multibase_base32(
                &sorafs_manifest::canonical_manifest_root_cid([0x91; 32]),
            ),
            manifest_id_hex,
        };
        let bytes = retired.encode();
        assert!(
            SoraPublishedInrouGuestImageArtifactV1::decode_all(&mut bytes.as_slice()).is_err(),
            "admitted V1 must reject the retired redundant storage-identifier layout"
        );
    }
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
fn inrou_paths_reject_more_than_first_release_component_limit() {
    let too_many_components = vec!["a"; SORA_INROU_PORTABLE_PATH_MAX_COMPONENTS_V1 + 1];
    let entrypoint = format!("/{}", too_many_components.join("/"));
    let error = SoraContainerManifestV1::validate_inrou_entrypoint(&entrypoint)
        .expect_err("Inrou entrypoint must enforce the runtime component limit at admission");
    assert_soracloud_invalid_field(error, "entrypoint");

    let mut image = sample_inrou_manifest()
        .guest_images
        .get(&SoraInrouGuestIsaV1::X8664)
        .cloned()
        .expect("x86_64 fixture");
    image.kernel_image_path = format!("/inrou/{}", too_many_components.join("/"));
    let error = image
        .validate()
        .expect_err("published guest-image member must enforce the runtime component limit");
    assert_soracloud_invalid_field(error, "kernel_image_path");
}
#[test]
fn first_release_lease_volume_kinds_are_per_replica() {
    for kind in [
        SoraLeaseVolumeKindV1::ServiceLeaseVolume,
        SoraLeaseVolumeKindV1::ConfidentialLeaseVolume,
        SoraLeaseVolumeKindV1::PersistentRootLeaseVolume,
    ] {
        assert!(kind.is_per_replica());
    }
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
fn inrou_guest_image_norito_rejects_retired_distribution_layout() {
    #[derive(Encode)]
    enum RetiredDistributionTargetV1 {
        Global,
    }
    #[derive(Encode)]
    struct RetiredDistributionPolicyV1 {
        target: RetiredDistributionTargetV1,
        prefer_low_latency: bool,
        fallback_to_low_latency_when_geography_unknown: bool,
    }
    #[derive(Encode)]
    struct RetiredPublishedArtifactV1 {
        manifest_digest_hex: String,
        content_cid: String,
        distribution: RetiredDistributionPolicyV1,
    }
    #[derive(Encode)]
    struct RetiredGuestImageV1 {
        kernel_image_path: String,
        rootfs_image_path: String,
        initrd_image_path: Option<String>,
        distribution: RetiredDistributionPolicyV1,
        published_artifact: RetiredPublishedArtifactV1,
    }
    fn retired_distribution() -> RetiredDistributionPolicyV1 {
        RetiredDistributionPolicyV1 {
            target: RetiredDistributionTargetV1::Global,
            prefer_low_latency: true,
            fallback_to_low_latency_when_geography_unknown: true,
        }
    }

    let retired = RetiredGuestImageV1 {
        kernel_image_path: "/inrou/x86_64/vmlinux".to_owned(),
        rootfs_image_path: "/inrou/x86_64/rootfs.ext4".to_owned(),
        initrd_image_path: None,
        distribution: retired_distribution(),
        published_artifact: RetiredPublishedArtifactV1 {
            manifest_digest_hex: "91".repeat(32),
            content_cid: encode_lowercase_multibase_base32(
                &sorafs_manifest::canonical_manifest_root_cid([0x91; 32]),
            ),
            distribution: retired_distribution(),
        },
    };
    let retired_bytes = retired.encode();
    assert!(
        SoraInrouGuestImageV1::decode_all(&mut retired_bytes.as_slice()).is_err(),
        "first-release Inrou V1 must reject the retired distribution-policy Norito layout"
    );
}
#[test]
fn inrou_manifest_norito_rejects_retired_bootstrap_overlay_layout() {
    #[derive(Encode)]
    struct RetiredBootstrapOverlayManifestV1 {
        schema_version: u16,
        guest_images: BTreeMap<SoraInrouGuestIsaV1, SoraInrouGuestImageV1>,
        bootstrap_user_data_path: Option<String>,
        ssh_authorized_keys: Vec<String>,
    }

    let canonical = sample_inrou_manifest();
    let retired = RetiredBootstrapOverlayManifestV1 {
        schema_version: canonical.schema_version,
        guest_images: canonical.guest_images,
        bootstrap_user_data_path: None,
        ssh_authorized_keys: Vec::new(),
    };
    let retired_bytes = retired.encode();
    assert!(
        SoraInrouManifestV1::decode_all(&mut retired_bytes.as_slice()).is_err(),
        "first-release Inrou V1 must reject the retired root bootstrap-overlay layout"
    );
}
#[test]
fn inrou_manifest_norito_rejects_retired_guest_os_layout() {
    #[derive(Encode)]
    enum RetiredInrouGuestOsV1 {
        DebianSlim,
    }
    #[derive(Encode)]
    struct RetiredGuestOsManifestV1 {
        schema_version: u16,
        guest_os: RetiredInrouGuestOsV1,
        guest_images: BTreeMap<SoraInrouGuestIsaV1, SoraInrouGuestImageV1>,
        ssh_authorized_keys: Vec<String>,
    }

    let canonical = sample_inrou_manifest();
    let retired = RetiredGuestOsManifestV1 {
        schema_version: canonical.schema_version,
        guest_os: RetiredInrouGuestOsV1::DebianSlim,
        guest_images: canonical.guest_images,
        ssh_authorized_keys: Vec::new(),
    };
    let retired_bytes = retired.encode();
    assert!(
        SoraInrouManifestV1::decode_all(&mut retired_bytes.as_slice()).is_err(),
        "first-release Inrou V1 must reject the retired guest_os Norito layout"
    );
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

#[test]
fn inrou_manifest_json_deserialize_rejects_flat_guest_images() {
    let manifest_json = r#"{
          "schema_version": 1,
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
          "guest_images": {
            "x86_64": {
              "kernel_image_path": "/inrou/x86_64/vmlinux",
              "rootfs_image_path": "/inrou/x86_64/rootfs.ext4",
              "initrd_image_path": null,
              "published_artifact": {
                "manifest_digest_hex": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "content_cid": "__X86_CONTENT_CID__"
              }
            },
            "aarch64": {
              "kernel_image_path": "/inrou/aarch64/vmlinux",
              "rootfs_image_path": "/inrou/aarch64/rootfs.ext4",
              "initrd_image_path": null,
              "published_artifact": {
                "manifest_digest_hex": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "content_cid": "__AARCH64_CONTENT_CID__"
              }
            }
          }
        }"#
        .replace("__X86_CONTENT_CID__", &x86_content_cid)
        .replace("__AARCH64_CONTENT_CID__", &aarch64_content_cid);
    let manifest: SoraInrouManifestV1 =
        norito::json::from_str(&json).expect("published guest artifact JSON should parse");
    let artifact = &manifest.guest_images[&SoraInrouGuestIsaV1::X8664].published_artifact;
    assert_eq!(artifact.content_cid, x86_content_cid);
    assert!(manifest.validate().is_ok());
}

#[test]
fn inrou_manifest_json_deserialize_rejects_flat_guest_image_overlays() {
    let mut value = norito::json::to_value(&sample_inrou_manifest())
        .expect("serialize canonical Inrou manifest");
    let object = value.as_object_mut().expect("Inrou manifest object");
    object.insert(
        "kernel_image_path".to_owned(),
        Value::String("/flat/vmlinux".to_owned()),
    );
    object.insert(
        "rootfs_image_path".to_owned(),
        Value::String("/flat/rootfs.ext4".to_owned()),
    );
    let error = norito::json::from_value::<SoraInrouManifestV1>(value)
        .expect_err("flat guest-image overlay fields must be rejected");
    assert!(matches!(
        error,
        json::Error::UnknownField { ref field } if field == "kernel_image_path"
    ));
}

#[test]
fn inrou_manifest_json_serialize_omits_flat_guest_image_fields() {
    let manifest = sample_inrou_manifest();
    let value = norito::json::to_value(&manifest).expect("serialize inrou manifest");
    assert!(value.get("kernel_image_path").is_none());
    assert!(value.get("rootfs_image_path").is_none());
    assert!(value.get("initrd_image_path").is_none());
    assert!(value.get("guest_images").is_some());
}

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
        matches!(&error, json::Error::MissingField { field: missing } if missing == field),
        "missing published-artifact `{field}` reported the wrong error: {error:?}"
    );
}

fn assert_tagged_json_unknown_field<T>(value: &T, label: &str)
where
    T: norito::json::JsonSerialize + norito::json::JsonDeserialize + core::fmt::Debug,
{
    let serialize_message = format!("serialize {label}");
    let object_message = format!("{label} JSON object");
    let unknown_message = format!("{label} must reject unknown envelope fields");
    let mut value = norito::json::to_value(value).expect(&serialize_message);
    value
        .as_object_mut()
        .expect(&object_message)
        .insert("retired_v0".to_owned(), norito::json!(true));
    let error = norito::json::from_value::<T>(value).expect_err(&unknown_message);
    assert!(
        matches!(
            error,
            json::Error::UnknownField { ref field } if field == "retired_v0"
        ),
        "{label} reported the wrong error: {error:?}"
    );
}

#[test]
fn inrou_v1_tagged_enum_envelopes_reject_unknown_fields() {
    assert_tagged_json_unknown_field::<SoraContainerRuntimeV1>(
        &SoraContainerRuntimeV1::Inrou,
        "container runtime",
    );
    assert_tagged_json_unknown_field::<SoraInrouGuestIsaV1>(
        &SoraInrouGuestIsaV1::Aarch64,
        "Inrou guest ISA",
    );
    assert_tagged_json_unknown_field::<SoraNetworkPolicyV1>(
        &SoraNetworkPolicyV1::Isolated,
        "Inrou network policy",
    );
    assert_tagged_json_unknown_field::<SoraConfigExportTargetV1>(
        &SoraConfigExportTargetV1::Env("APP_CONFIG_JSON".to_owned()),
        "Inrou config export target",
    );
    assert_tagged_json_unknown_field::<SoraRouteVisibilityV1>(
        &SoraRouteVisibilityV1::Public,
        "Inrou route visibility",
    );
    assert_tagged_json_unknown_field::<SoraTlsModeV1>(&SoraTlsModeV1::Required, "Inrou TLS mode");
    assert_tagged_json_unknown_field::<SoraServiceExecutionPlaneV1>(
        &SoraServiceExecutionPlaneV1::HttpService,
        "Inrou service execution plane",
    );
    assert_tagged_json_unknown_field::<SoraLeaseVolumeKindV1>(
        &SoraLeaseVolumeKindV1::PersistentRootLeaseVolume,
        "Inrou lease volume kind",
    );
    assert_tagged_json_unknown_field::<SoraServiceHealthStatusV1>(
        &SoraServiceHealthStatusV1::Healthy,
        "Inrou replica health status",
    );
    assert_tagged_json_unknown_field::<SecretEnvelopeEncryptionV1>(
        &SecretEnvelopeEncryptionV1::ClientCiphertext,
        "Inrou secret-envelope encryption",
    );
    assert_tagged_json_unknown_field::<StorageClass>(
        &StorageClass::Warm,
        "Inrou lease-volume storage class",
    );
    assert_tagged_json_unknown_field::<SoraArtifactKindV1>(
        &SoraArtifactKindV1::Bundle,
        "Inrou service artifact kind",
    );
    assert_tagged_json_unknown_field::<SoraServiceLifecycleActionV1>(
        &SoraServiceLifecycleActionV1::Deploy,
        "Soracloud service lifecycle action",
    );
    assert_tagged_json_unknown_field::<SoraStateMutationOperationV1>(
        &SoraStateMutationOperationV1::Upsert,
        "Soracloud state mutation operation",
    );
    assert_tagged_json_unknown_field::<SoraRolloutStageV1>(
        &SoraRolloutStageV1::Canary,
        "Soracloud rollout stage",
    );
    assert_tagged_json_unknown_field::<SoraServiceLeaseStatusV1>(
        &SoraServiceLeaseStatusV1::Active,
        "Soracloud service lease status",
    );
    assert_tagged_json_unknown_field::<SoraStateScopeV1>(
        &SoraStateScopeV1::ServiceState,
        "Soracloud state scope",
    );
    assert_tagged_json_unknown_field::<SoraStateMutabilityV1>(
        &SoraStateMutabilityV1::ReadWrite,
        "Soracloud state mutability",
    );
    assert_tagged_json_unknown_field::<SoraStateEncryptionV1>(
        &SoraStateEncryptionV1::FheCiphertext,
        "Soracloud state encryption",
    );
    assert_tagged_json_unknown_field::<BfvCiphertextBoundModeV1>(
        &BfvCiphertextBoundModeV1::ExactResidualMultiple,
        "Soracloud BFV ciphertext bound mode",
    );
}

#[test]
fn inrou_manifest_json_requires_the_exact_v1_shape() {
    let manifest = sample_inrou_manifest();
    let canonical = norito::json::to_value(&manifest).expect("serialize canonical Inrou manifest");
    assert_eq!(
        norito::json::from_value::<SoraInrouManifestV1>(canonical.clone())
            .expect("the explicit canonical V1 shape must decode"),
        manifest
    );
    for field in ["schema_version", "guest_images"] {
        assert_inrou_manifest_field_is_required(&canonical, field);
    }

    let mut retired_bootstrap_overlay = canonical.clone();
    retired_bootstrap_overlay
        .as_object_mut()
        .expect("manifest object")
        .insert("bootstrap_user_data_path".to_owned(), Value::Null);
    norito::json::from_value::<SoraInrouManifestV1>(retired_bootstrap_overlay)
        .expect_err("the retired root bootstrap overlay field must not be accepted");

    let mut retired_guest_os = canonical.clone();
    retired_guest_os
        .as_object_mut()
        .expect("manifest object")
        .insert(
            "guest_os".to_owned(),
            Value::String("DebianSlim".to_owned()),
        );
    norito::json::from_value::<SoraInrouManifestV1>(retired_guest_os)
        .expect_err("the retired guest_os field must not be accepted");

    for field in [
        "kernel_image_path",
        "rootfs_image_path",
        "initrd_image_path",
        "published_artifact",
    ] {
        assert_inrou_guest_image_field_is_required(&canonical, field);
    }

    let mut retired_guest_distribution = canonical.clone();
    retired_guest_distribution
        .pointer_mut("/guest_images/x86_64")
        .and_then(Value::as_object_mut)
        .expect("x86_64 guest image object")
        .insert("distribution".to_owned(), Value::Null);
    norito::json::from_value::<SoraInrouManifestV1>(retired_guest_distribution)
        .expect_err("first-release guest images must reject the retired distribution field");

    let mut retired_artifact_distribution = canonical.clone();
    retired_artifact_distribution
        .pointer_mut("/guest_images/x86_64/published_artifact")
        .and_then(Value::as_object_mut)
        .expect("published guest-image artifact object")
        .insert("distribution".to_owned(), Value::Null);
    norito::json::from_value::<SoraInrouManifestV1>(retired_artifact_distribution)
        .expect_err("first-release published artifacts must reject the retired distribution field");

    let mut published_manifest = sample_inrou_manifest();
    published_manifest
        .guest_images
        .get_mut(&SoraInrouGuestIsaV1::X8664)
        .expect("x86_64 guest image")
        .published_artifact = sample_published_inrou_guest_image_artifact(31);
    let published = norito::json::to_value(&published_manifest)
        .expect("serialize published guest-image artifact");
    for field in ["manifest_digest_hex", "content_cid"] {
        assert_inrou_published_artifact_field_is_required(&published, field);
    }
    let mut unknown_artifact = published;
    unknown_artifact
        .pointer_mut("/guest_images/x86_64/published_artifact")
        .and_then(Value::as_object_mut)
        .expect("published guest-image artifact object")
        .insert("manifest_id_hex".to_owned(), Value::Null);
    norito::json::from_value::<SoraInrouManifestV1>(unknown_artifact)
        .expect_err("first-release published artifacts must reject retired storage identifiers");
}

#[test]
fn inrou_manifest_json_rejects_null_for_non_optional_v1_fields() {
    let canonical = norito::json::to_value(&sample_inrou_manifest())
        .expect("serialize canonical Inrou manifest");
    for field in ["schema_version", "guest_images"] {
        let mut value = canonical.clone();
        value
            .as_object_mut()
            .expect("manifest object")
            .insert(field.to_owned(), Value::Null);
        norito::json::from_value::<SoraInrouManifestV1>(value)
            .expect_err("non-optional first-release Inrou fields must not accept null");
    }
    for field in [
        "kernel_image_path",
        "rootfs_image_path",
        "published_artifact",
    ] {
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
            "allow_state_writes": false,
            "allow_model_inference": false,
            "allow_model_training": false
          },
          "resources": {
            "cpu_millis": 750,
            "memory_bytes": 536870912,
            "ephemeral_storage_bytes": 2147483648,
            "max_open_files_per_process": 512,
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
            "guest_images": {
              "x86_64": {
                "kernel_image_path": "/inrou/x86_64/vmlinux",
                "rootfs_image_path": "/inrou/x86_64/rootfs.ext4",
                "initrd_image_path": null,
                "published_artifact": {
                  "manifest_digest_hex": "3131313131313131313131313131313131313131313131313131313131313131",
                  "content_cid": "bafyr6ibrgeytcmjrgeytcmjrgeytcmjrgeytcmjrgeytcmjrgeytcmjrge"
                }
              },
              "aarch64": {
                "kernel_image_path": "/inrou/aarch64/vmlinux",
                "rootfs_image_path": "/inrou/aarch64/rootfs.ext4",
                "initrd_image_path": null,
                "published_artifact": {
                  "manifest_digest_hex": "3232323232323232323232323232323232323232323232323232323232323232",
                  "content_cid": "bafyr6ibsgizdemrsgizdemrsgizdemrsgizdemrsgizdemrsgizdemrsgi"
                }
              }
            }
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
            "allow_state_writes": false,
            "allow_model_inference": false,
            "allow_model_training": false
          },
          "resources": {
            "cpu_millis": 2000,
            "memory_bytes": 4294967296,
            "ephemeral_storage_bytes": 8589934592,
            "max_open_files_per_process": 4096,
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
    assert_eq!(
        inrou.guest_images[&SoraInrouGuestIsaV1::X8664].kernel_image_path,
        "/inrou/x86_64/vmlinux"
    );
}
#[test]
fn deployment_bundle_validate_rejects_http_service_without_data_lease_volume() {
    let mut container = sample_container();
    container.runtime = SoraContainerRuntimeV1::Inrou;
    container.entrypoint = "/app/bin/service".to_string();
    container.inrou = Some(sample_inrou_manifest());
    container.capabilities.network = SoraNetworkPolicyV1::Isolated;
    let container_hash = Hash::new(Encode::encode(&container));
    let mut service = sample_service(Vec::new());
    service.execution_plane = SoraServiceExecutionPlaneV1::HttpService;
    service.rollout.canary_percent = 0;
    service.replicas = NonZeroU16::new(2).expect("nonzero");
    service.placement_targets = sample_inrou_placement_targets(service.replicas.get());
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
        .expect_err("Inrou http services must declare a replica-private data volume");
    assert_soracloud_invalid_field(error, "service.lease_volumes");
}
#[test]
fn deployment_bundle_validate_accepts_http_service_with_confidential_data_lease() {
    let mut container = sample_container();
    container.runtime = SoraContainerRuntimeV1::Inrou;
    container.entrypoint = "/app/bin/service".to_string();
    container.inrou = Some(sample_inrou_manifest());
    container.capabilities.network = SoraNetworkPolicyV1::Isolated;
    let container_hash = Hash::new(Encode::encode(&container));
    let mut service = sample_service(Vec::new());
    service.execution_plane = SoraServiceExecutionPlaneV1::HttpService;
    service.rollout.canary_percent = 0;
    service.replicas = NonZeroU16::new(2).expect("nonzero");
    service.placement_targets = sample_inrou_placement_targets(service.replicas.get());
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
            mount_path: "/var/lib/soracloud/volumes/sealed_state".to_string(),
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
        "confidential lease volumes should satisfy the replica-private data-volume requirement"
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
    service.rollout.canary_percent = 0;
    service.replicas = NonZeroU16::new(1).expect("nonzero");
    service.placement_targets = sample_inrou_placement_targets(service.replicas.get());
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
    service.rollout.canary_percent = 0;
    service.replicas = NonZeroU16::new(1).expect("nonzero");
    service.placement_targets = sample_inrou_placement_targets(service.replicas.get());
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
fn deployment_bundle_validate_rejects_unenforceable_inrou_resource_units() {
    let mut container = sample_container();
    container.runtime = SoraContainerRuntimeV1::Inrou;
    container.entrypoint = "/app/bin/service".to_string();
    container.inrou = Some(sample_inrou_manifest());
    container.capabilities.network = SoraNetworkPolicyV1::Isolated;
    container.resources.cpu_millis = NonZeroU32::new(505).expect("nonzero");
    let container_hash = Hash::new(Encode::encode(&container));
    let mut service = sample_service(Vec::new());
    service.execution_plane = SoraServiceExecutionPlaneV1::HttpService;
    service.rollout.canary_percent = 0;
    service.replicas = NonZeroU16::new(1).expect("nonzero");
    service.placement_targets = sample_inrou_placement_targets(service.replicas.get());
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
        .expect_err("hosted HTTP services must use exactly enforceable Inrou resource units");
    assert_soracloud_invalid_field(error, "container.resources.cpu_millis");
}
fn minimum_inrou_resource_limits() -> SoraResourceLimitsV1 {
    SoraResourceLimitsV1 {
        cpu_millis: NonZeroU32::new(SORA_INROU_MIN_CPU_MILLIS_V1).expect("nonzero CPU"),
        memory_bytes: NonZeroU64::new(SORA_INROU_MIN_MEMORY_BYTES_V1).expect("nonzero memory"),
        ephemeral_storage_bytes: NonZeroU64::new(SORA_INROU_EPHEMERAL_STORAGE_ALIGNMENT_BYTES_V1)
            .expect("nonzero ephemeral storage"),
        max_open_files_per_process: NonZeroU32::new(SORA_INROU_MIN_OPEN_FILES_PER_PROCESS_V1)
            .expect("nonzero descriptor cap"),
        max_tasks: NonZeroU16::new(1).expect("nonzero task cap"),
    }
}

#[test]
fn inrou_resource_limits_enforce_exact_units_and_physical_overhead() {
    let exact = minimum_inrou_resource_limits();
    exact
        .validate_for_inrou()
        .expect("exact minimum Inrou resource units must validate");
    SoraResourceLimitsV1 {
        cpu_millis: NonZeroU32::new(SORA_INROU_MAX_CPU_MILLIS_V1).expect("nonzero CPU ceiling"),
        ..exact
    }
    .validate_for_inrou()
    .expect("the qualified Inrou V1 CPU ceiling must validate exactly");
    assert_eq!(
        exact.checked_inrou_host_cpu_millis(),
        Some(u64::from(SORA_INROU_MIN_CPU_MILLIS_V1) + SORA_INROU_VMM_CPU_OVERHEAD_MILLIS_V1)
    );
    assert_eq!(
        exact.checked_inrou_host_memory_bytes(),
        Some(SORA_INROU_MIN_MEMORY_BYTES_V1 + SORA_INROU_VMM_MEMORY_OVERHEAD_BYTES_V1)
    );

    for (limits, field) in [
        (
            SoraResourceLimitsV1 {
                cpu_millis: NonZeroU32::new(SORA_INROU_MIN_CPU_MILLIS_V1 - 1).expect("nonzero CPU"),
                ..exact
            },
            "container.resources.cpu_millis",
        ),
        (
            SoraResourceLimitsV1 {
                cpu_millis: NonZeroU32::new(SORA_INROU_MIN_CPU_MILLIS_V1 + 1).expect("nonzero CPU"),
                ..exact
            },
            "container.resources.cpu_millis",
        ),
        (
            SoraResourceLimitsV1 {
                cpu_millis: NonZeroU32::new(
                    SORA_INROU_MAX_CPU_MILLIS_V1 + SORA_INROU_CPU_MILLIS_ALIGNMENT_V1,
                )
                .expect("nonzero CPU"),
                ..exact
            },
            "container.resources.cpu_millis",
        ),
        (
            SoraResourceLimitsV1 {
                memory_bytes: NonZeroU64::new(SORA_INROU_MIN_MEMORY_BYTES_V1 - 1)
                    .expect("nonzero memory"),
                ..exact
            },
            "container.resources.memory_bytes",
        ),
        (
            SoraResourceLimitsV1 {
                memory_bytes: NonZeroU64::new(SORA_INROU_MIN_MEMORY_BYTES_V1 + 1)
                    .expect("nonzero memory"),
                ..exact
            },
            "container.resources.memory_bytes",
        ),
        (
            SoraResourceLimitsV1 {
                ephemeral_storage_bytes: NonZeroU64::new(
                    SORA_INROU_EPHEMERAL_STORAGE_ALIGNMENT_BYTES_V1 + 1,
                )
                .expect("nonzero ephemeral storage"),
                ..exact
            },
            "container.resources.ephemeral_storage_bytes",
        ),
        (
            SoraResourceLimitsV1 {
                max_open_files_per_process: NonZeroU32::new(
                    SORA_INROU_MIN_OPEN_FILES_PER_PROCESS_V1 - 1,
                )
                .expect("nonzero descriptor cap"),
                ..exact
            },
            "container.resources.max_open_files_per_process",
        ),
    ] {
        let error = limits
            .validate_for_inrou()
            .expect_err("noncanonical Inrou resource unit must fail closed");
        assert_soracloud_invalid_field(error, field);
    }

    let maximum_aligned_memory = u64::MAX - (u64::MAX % SORA_INROU_MEMORY_ALIGNMENT_BYTES_V1);
    let overflow = SoraResourceLimitsV1 {
        memory_bytes: NonZeroU64::new(maximum_aligned_memory).expect("nonzero memory"),
        ..exact
    };
    assert_eq!(overflow.checked_inrou_host_memory_bytes(), None);
    let error = overflow
        .validate_for_inrou()
        .expect_err("an aligned guest memory limit that overflows physical overhead must fail");
    assert_soracloud_invalid_field(error, "container.resources.memory_bytes");
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

fn sample_hosted_service_lease(replica_count: u16) -> SoraServiceLeaseStateV1 {
    let economics = SoraHttpServiceEconomicsV1::default();
    SoraServiceLeaseStateV1 {
        schema_version: SORA_SERVICE_LEASE_STATE_VERSION_V1,
        economic_clock: SoraServiceLeaseClockV1::CanonicalBlockHeight,
        status: SoraServiceLeaseStatusV1::Active,
        quota_class: economics.quota_class,
        replica_count: NonZeroU16::new(replica_count).expect("nonzero replica count"),
        deployment_deposit: economics.deployment_deposit,
        prepaid_runtime_balance: economics.prepaid_runtime_balance,
        runtime_price_per_block: economics.runtime_price_per_block,
        storage_price_per_gib_block: economics.storage_price_per_gib_block,
        egress_price_per_mib: economics.egress_price_per_mib,
        lease_started_height: 10,
        lease_expires_height: 15,
        reporting_epoch: 1,
        settled_egress_bytes: 0,
        egress_reporter_checkpoints: Vec::new(),
        accounted_egress_bytes: 0,
        last_status_reason: None,
    }
}

fn sample_hosted_lease_volume(
    volume_name: &str,
    kind: SoraLeaseVolumeKindV1,
    max_total_bytes: u64,
) -> SoraServiceLeaseVolumeStateV1 {
    let mount_path = match kind {
        SoraLeaseVolumeKindV1::PersistentRootLeaseVolume => "/".to_owned(),
        SoraLeaseVolumeKindV1::ServiceLeaseVolume
        | SoraLeaseVolumeKindV1::ConfidentialLeaseVolume => {
            format!("/var/lib/soracloud/volumes/{volume_name}")
        }
    };
    SoraServiceLeaseVolumeStateV1 {
        schema_version: SORA_SERVICE_LEASE_VOLUME_STATE_VERSION_V1,
        economic_clock: SoraServiceLeaseClockV1::CanonicalBlockHeight,
        volume_name: volume_name.parse().expect("valid volume name"),
        kind,
        storage_class: StorageClass::Warm,
        mount_path,
        max_total_bytes,
        lease_started_height: 10,
        lease_expires_height: 15,
        authoritative_generation: 1,
    }
}

fn sample_hosted_active_bundle_and_deployment()
-> (SoraDeploymentBundleV1, SoraServiceDeploymentStateV1) {
    let mut container = sample_container();
    container.runtime = SoraContainerRuntimeV1::Inrou;
    container.entrypoint = "/app/bin/service".to_owned();
    container.inrou = Some(sample_inrou_manifest());
    container.capabilities.network = SoraNetworkPolicyV1::Isolated;
    let mut service = sample_service(Vec::new());
    service.execution_plane = SoraServiceExecutionPlaneV1::HttpService;
    service.rollout.canary_percent = 0;
    service.placement_targets = sample_inrou_placement_targets(service.replicas.get());
    service.container.manifest_hash = Hash::new(Encode::encode(&container));
    service.state_bindings.clear();
    service.handlers.clear();
    service.artifacts.clear();
    service.lease_volumes = sample_inrou_lease_volumes();
    let bundle = SoraDeploymentBundleV1 {
        schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
        container,
        service,
    };
    let mut deployment = sample_service_deployment_state();
    deployment.service_name = bundle.service.service_name.clone();
    deployment.current_service_version = bundle.service.service_version.clone();
    deployment.current_service_manifest_hash = bundle.service_manifest_hash();
    deployment.current_container_manifest_hash = bundle.container_manifest_hash();
    deployment.service_lease = Some(sample_hosted_service_lease(bundle.service.replicas.get()));
    deployment.lease_volume_states = bundle
        .service
        .lease_volumes
        .iter()
        .map(|volume| {
            let mut state = sample_hosted_lease_volume(
                volume.volume_name.as_ref(),
                volume.kind,
                volume.max_total_bytes.get(),
            );
            state.storage_class = volume.storage_class;
            state.mount_path.clone_from(&volume.mount_path);
            state
        })
        .collect();
    (bundle, deployment)
}

#[test]
fn deployment_active_bundle_binding_rejects_economic_and_volume_drift() {
    let (bundle, deployment) = sample_hosted_active_bundle_and_deployment();
    deployment
        .validate_against_active_bundle(&bundle)
        .expect("exact active bundle economics and volumes must validate");

    let mut wrong_replicas = deployment.clone();
    wrong_replicas
        .service_lease
        .as_mut()
        .expect("hosted lease")
        .replica_count = NonZeroU16::new(1).expect("nonzero");
    assert_soracloud_invalid_field(
        wrong_replicas
            .validate_against_active_bundle(&bundle)
            .expect_err("a restored lease must not understate replica billing"),
        "service_lease.replica_count",
    );

    let mut wrong_price = deployment.clone();
    wrong_price
        .service_lease
        .as_mut()
        .expect("hosted lease")
        .runtime_price_per_block = xor_quantity_from_nanos(1);
    assert_soracloud_invalid_field(
        wrong_price
            .validate_against_active_bundle(&bundle)
            .expect_err("a restored lease must not understate its admitted unit price"),
        "service_lease.runtime_price_per_block",
    );

    let mut missing_volume = deployment.clone();
    missing_volume.lease_volume_states.pop();
    assert_soracloud_invalid_field(
        missing_volume
            .validate_against_active_bundle(&bundle)
            .expect_err("a restored deployment must not omit billed storage"),
        "lease_volume_states",
    );

    let mut shrunken_volume = deployment;
    shrunken_volume.lease_volume_states[0].max_total_bytes -= 1;
    assert_soracloud_invalid_field(
        shrunken_volume
            .validate_against_active_bundle(&bundle)
            .expect_err("a restored deployment must not shrink its admitted storage limit"),
        "lease_volume_states",
    );
}

#[test]
fn deployment_active_bundle_binding_rejects_inrou_active_rollout() {
    let (bundle, mut deployment) = sample_hosted_active_bundle_and_deployment();
    let rollout = SoraServiceRolloutStateV1 {
        schema_version: SORA_SERVICE_ROLLOUT_STATE_VERSION_V1,
        rollout_handle: "portal:rollout:1".to_owned(),
        baseline_version: "0.9.0".to_owned(),
        candidate_version: deployment.current_service_version.clone(),
        canary_percent: 25,
        traffic_percent: 25,
        stage: SoraRolloutStageV1::Canary,
        health_failures: 0,
        max_health_failures: 2,
        health_window_secs: 30,
        created_sequence: 1,
        updated_sequence: 1,
    };
    deployment.active_rollout = Some(rollout.clone());
    deployment.last_rollout = Some(rollout);
    deployment
        .validate()
        .expect("active rollout remains structurally valid before bundle binding");

    assert_soracloud_invalid_field(
        deployment
            .validate_against_active_bundle(&bundle)
            .expect_err("first-release Inrou must reject a second active revision"),
        "active_rollout",
    );
}

#[test]
fn service_lease_norito_rejects_retired_implicit_audit_sequence_clock_layout() {
    #[derive(Encode)]
    struct RetiredImplicitClockLeaseStateV1 {
        schema_version: u16,
        status: SoraServiceLeaseStatusV1,
        quota_class: String,
        replica_count: NonZeroU16,
        deployment_deposit: Quantity,
        prepaid_runtime_balance: Quantity,
        runtime_price_per_block: Quantity,
        storage_price_per_gib_block: Quantity,
        egress_price_per_mib: Quantity,
        lease_started_sequence: u64,
        lease_expires_sequence: u64,
        reporting_epoch: u64,
        settled_egress_bytes: u128,
        egress_reporter_checkpoints: Vec<SoraServiceLeaseEgressCheckpointV1>,
        accounted_egress_bytes: u128,
        last_status_reason: Option<String>,
    }

    let lease = sample_hosted_service_lease(1);
    for status in [
        SoraServiceLeaseStatusV1::Active,
        SoraServiceLeaseStatusV1::Expired,
        SoraServiceLeaseStatusV1::Exhausted,
        SoraServiceLeaseStatusV1::Suspended,
    ] {
        let retired = RetiredImplicitClockLeaseStateV1 {
            schema_version: lease.schema_version,
            status,
            quota_class: lease.quota_class.clone(),
            replica_count: lease.replica_count,
            deployment_deposit: lease.deployment_deposit.clone(),
            prepaid_runtime_balance: lease.prepaid_runtime_balance.clone(),
            runtime_price_per_block: lease.runtime_price_per_block.clone(),
            storage_price_per_gib_block: lease.storage_price_per_gib_block.clone(),
            egress_price_per_mib: lease.egress_price_per_mib.clone(),
            lease_started_sequence: lease.lease_started_height,
            lease_expires_sequence: lease.lease_expires_height,
            reporting_epoch: lease.reporting_epoch,
            settled_egress_bytes: lease.settled_egress_bytes,
            egress_reporter_checkpoints: lease.egress_reporter_checkpoints.clone(),
            accounted_egress_bytes: lease.accounted_egress_bytes,
            last_status_reason: lease.last_status_reason.clone(),
        };
        let encoded = retired.encode();
        assert!(
            SoraServiceLeaseStateV1::decode_all(&mut encoded.as_slice()).is_err(),
            "the first release must reject every retired status tag in the binary layout that omitted an explicit economic clock domain"
        );
    }
}

#[test]
fn hosted_service_lease_bills_only_canonical_elapsed_blocks() {
    let lease = sample_hosted_service_lease(1);

    assert_eq!(lease.billed_blocks_at(9), 0);
    assert_eq!(lease.billed_blocks_at(10), 0);
    assert_eq!(lease.billed_blocks_at(11), 1);
    assert_eq!(lease.billed_blocks_at(14), 4);
    assert_eq!(lease.billed_blocks_at(15), 5);
    assert_eq!(lease.billed_blocks_at(u64::MAX), 5);
    assert_eq!(
        lease.status_at(14, 0).expect("height 14 status"),
        SoraServiceLeaseStatusV1::Active
    );
    assert_eq!(
        lease.status_at(15, 0).expect("height 15 status"),
        SoraServiceLeaseStatusV1::Expired
    );

    let volume =
        sample_hosted_lease_volume("root", SoraLeaseVolumeKindV1::PersistentRootLeaseVolume, 1);
    assert!(volume.is_active_at(14));
    assert!(!volume.is_active_at(15));
}

#[test]
fn deployment_storage_accounting_is_replica_exact_and_fails_on_overflow() {
    let mut deployment = sample_service_deployment_state();
    deployment.service_lease = Some(sample_hosted_service_lease(3));
    deployment.lease_volume_states = vec![
        sample_hosted_lease_volume("root", SoraLeaseVolumeKindV1::PersistentRootLeaseVolume, 2),
        sample_hosted_lease_volume("data", SoraLeaseVolumeKindV1::ServiceLeaseVolume, 5),
    ];
    assert_eq!(
        deployment
            .accounted_storage_bytes()
            .expect("small replica aggregate must fit"),
        21
    );

    deployment
        .service_lease
        .as_mut()
        .expect("hosted lease")
        .replica_count = NonZeroU16::new(2).expect("nonzero");
    deployment.lease_volume_states = vec![sample_hosted_lease_volume(
        "root",
        SoraLeaseVolumeKindV1::PersistentRootLeaseVolume,
        u64::MAX,
    )];
    assert_eq!(
        deployment.accounted_storage_bytes(),
        Err(NumericOperationError::MantissaOverflow)
    );
    let error = deployment
        .validate()
        .expect_err("overflowing replica storage accounting must fail validation");
    assert_soracloud_invalid_field(error, "lease_volume_states");
}

#[test]
fn hosted_minimum_prepaid_multiplies_storage_by_replica_count() {
    let mut manifest = sample_service(Vec::new());
    manifest.execution_plane = SoraServiceExecutionPlaneV1::HttpService;
    manifest.replicas = NonZeroU16::new(2).expect("nonzero");
    manifest.rollout.canary_percent = 0;
    manifest.handlers.clear();
    manifest.artifacts.clear();
    manifest.lease_volumes = vec![
        SoraLeaseVolumeBindingV1 {
            volume_name: "root".parse().expect("valid name"),
            kind: SoraLeaseVolumeKindV1::PersistentRootLeaseVolume,
            storage_class: StorageClass::Warm,
            mount_path: "/".to_owned(),
            max_total_bytes: NonZeroU64::new(SORA_STORAGE_BYTES_PER_GIB).expect("nonzero"),
        },
        SoraLeaseVolumeBindingV1 {
            volume_name: "data".parse().expect("valid name"),
            kind: SoraLeaseVolumeKindV1::ServiceLeaseVolume,
            storage_class: StorageClass::Warm,
            mount_path: "/var/lib/soracloud/volumes/data".to_owned(),
            max_total_bytes: NonZeroU64::new(1).expect("nonzero"),
        },
    ];

    let minimum = manifest
        .minimum_hosted_runtime_prepaid()
        .expect("minimum prepaid calculation");
    let three_gib_storage = manifest
        .economics
        .storage_price_per_gib_block
        .try_mul_decimal(&Numeric::from(3_u64))
        .expect("three-GiB storage charge");
    let expected = manifest
        .economics
        .runtime_price_per_block
        .checked_add(&three_gib_storage)
        .expect("one-block runtime and storage charge");
    assert_eq!(minimum, expected);

    manifest.economics.prepaid_runtime_balance = minimum.clone();
    manifest
        .validate()
        .expect("the exact replica-aware minimum must pass admission");
    manifest.economics.prepaid_runtime_balance = minimum
        .checked_sub(&xor_quantity_from_nanos(1))
        .expect("minimum exceeds one nano-XOR");
    let error = manifest
        .validate()
        .expect_err("one nano-XOR below the replica-aware minimum must fail admission");
    assert_soracloud_invalid_field(error, "economics.prepaid_runtime_balance");
}

fn service_lease_json_fixture() -> (
    SoraServiceLeaseStateV1,
    SoraServiceLeaseVolumeStateV1,
    SoraServiceLeaseEgressCheckpointV1,
) {
    let lease = sample_hosted_service_lease(1);
    let mut volume = sample_hosted_lease_volume(
        "root",
        SoraLeaseVolumeKindV1::PersistentRootLeaseVolume,
        8 * 1024 * 1024 * 1024,
    );
    volume.lease_expires_height = lease.lease_expires_height;
    let checkpoint = SoraServiceLeaseEgressCheckpointV1 {
        reporting_epoch: 1,
        assignment: SoraServiceLeaseReporterAssignmentV1 {
            schema_version: SORA_SERVICE_LEASE_REPORTER_ASSIGNMENT_VERSION_V1,
            service_version: "1.0.0".to_owned(),
            placement: SoraInrouReplicaPlacementV1 {
                replica_slot: 1,
                economic_clock: SoraServiceLeaseClockV1::CanonicalBlockHeight,
                lease_started_height: 10,
                placement_incarnation: Hash::new(b"placement-1"),
                host_availability: SoraInrouReplicaHostAvailabilityV1::Available,
                validator_account_id: sample_account_id(202),
                peer_id: sample_peer_id(202),
                selected_guest_isa: SoraInrouGuestIsaV1::X8664,
            },
            placement_reconciled_at_ms: 1,
        },
        accounted_egress_bytes: 0,
        last_updated_height: 1,
        finalize_reporter: false,
    };
    (lease, volume, checkpoint)
}

#[test]
fn service_lease_v1_json_requires_explicit_null_empty_and_closed_fields() {
    let (lease, volume, checkpoint) = service_lease_json_fixture();
    let mut excessive_replicas = lease.clone();
    excessive_replicas.replica_count = NonZeroU16::new(5).expect("nonzero");
    let error = excessive_replicas
        .validate()
        .expect_err("lease accounting must reject replicas outside the release quota");
    assert_soracloud_invalid_field(error, "replica_count");

    let mut out_of_range_checkpoint = lease.clone();
    let mut replica_five_checkpoint = checkpoint.clone();
    replica_five_checkpoint.assignment.placement.replica_slot = 5;
    out_of_range_checkpoint
        .egress_reporter_checkpoints
        .push(replica_five_checkpoint);
    let error = out_of_range_checkpoint
        .validate()
        .expect_err("egress checkpoints must stay within the release replica range");
    assert_soracloud_invalid_field(error, "egress_reporter_checkpoints");

    let mut padded_checkpoint_lease = lease.clone();
    let mut padded_checkpoint = checkpoint.clone();
    padded_checkpoint.assignment.service_version.push(' ');
    padded_checkpoint_lease
        .egress_reporter_checkpoints
        .push(padded_checkpoint);
    padded_checkpoint_lease
        .validate()
        .expect_err("checkpoint service-version aliases must fail closed");

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
    for field in [
        "economic_clock",
        "replica_count",
        "last_status_reason",
        "egress_reporter_checkpoints",
    ] {
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
    assert_eq!(
        norito::json::from_value::<SoraServiceLeaseVolumeStateV1>(volume_json.clone())
            .expect("canonical lease volume must decode"),
        volume
    );
    let mut missing_clock = volume_json.clone();
    assert!(
        missing_clock
            .as_object_mut()
            .expect("lease volume JSON object")
            .remove("economic_clock")
            .is_some()
    );
    norito::json::from_value::<SoraServiceLeaseVolumeStateV1>(missing_clock)
        .expect_err("omitted economic clock domain must be rejected");

    assert_host_json_unknown_field::<SoraServiceLeaseStateV1>(&lease, "service lease state");
    assert_host_json_unknown_field::<SoraServiceLeaseVolumeStateV1>(
        &volume,
        "service lease volume state",
    );
    assert_host_json_unknown_field::<SoraServiceLeaseEgressCheckpointV1>(
        &checkpoint,
        "service lease egress checkpoint",
    );
}
