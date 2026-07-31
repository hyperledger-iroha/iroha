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
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "config_exports",
                ..
            }
        ));
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
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "config_exports",
                ..
            }
        ));
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

    #[test]
    fn service_validate_rejects_uncertified_query_handler() {
        let mut manifest = sample_service(vec![sample_binding("session")]);
        manifest.handlers[1].certified_response = SoraCertifiedResponsePolicyV1::None;

        let error = manifest
            .validate()
            .expect_err("query handlers must stay certified");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "certified_response",
                ..
            }
        ));
    }

    #[test]
    fn service_validate_rejects_private_update_without_mailbox() {
        let mut manifest = sample_service(vec![sample_binding("session")]);
        manifest.handlers[3].mailbox = None;

        let error = manifest
            .validate()
            .expect_err("private_update handlers require a mailbox");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "mailbox",
                ..
            }
        ));
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
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "economics.prepaid_runtime_balance",
                ..
            }
        ));
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
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "handlers",
                ..
            }
        ));
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
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "service.container.manifest_hash",
                ..
            }
        ));
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
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "container.capabilities.allow_state_writes",
                ..
            }
        ));
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
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "container.runtime",
                ..
            }
        ));
    }

    #[test]
    fn deployment_bundle_validate_accepts_inrou_http_service() {
        let mut container = sample_container();
        container.runtime = SoraContainerRuntimeV1::Inrou;
        container.entrypoint = "/app/bin/service".to_string();
        container.inrou = Some(sample_inrou_manifest());
        container.capabilities.network = SoraNetworkPolicyV1::Open;
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
            "Inrou http services should pass admission"
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
        let nonzero_padding =
            String::from_utf8(nonzero_padding).expect("base32 fixture remains UTF-8");

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
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "target",
                ..
            }
        ));
    }

    #[test]
    fn inrou_manifest_validate_rejects_missing_required_guest_isa() {
        let mut manifest = sample_inrou_manifest();
        manifest
            .guest_images
            .remove(&SoraInrouGuestIsaV1::X8664)
            .expect("fixture x86_64 guest image");

        let error = manifest
            .validate()
            .expect_err("both required guest ISA profiles must be present");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "guest_images",
                ..
            }
        ));
    }

    #[cfg(feature = "json")]
    #[test]
    fn inrou_manifest_json_deserialize_rejects_flat_guest_images() {
        let manifest_json = r#"{
          "schema_version": 1,
          "guest_os": "DebianSlim",
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
          "guest_os": "DebianSlim",
          "guest_images": {
            "x86_64": {
              "kernel_image_path": "/inrou/x86_64/vmlinux",
              "rootfs_image_path": "/inrou/x86_64/rootfs.ext4",
              "initrd_image_path": null,
              "published_artifact": {
                "manifest_digest_hex": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "content_cid": "__X86_CONTENT_CID__",
                "manifest_id_hex": null
              }
            },
            "aarch64": {
              "kernel_image_path": "/inrou/aarch64/vmlinux",
              "rootfs_image_path": "/inrou/aarch64/rootfs.ext4",
              "initrd_image_path": null,
              "published_artifact": {
                "manifest_digest_hex": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "content_cid": "__AARCH64_CONTENT_CID__",
                "manifest_id_hex": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
              }
            }
          },
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
          "guest_os": "DebianSlim",
          "guest_images": {
            "x86_64": {
              "kernel_image_path": "/inrou/x86_64/vmlinux",
              "rootfs_image_path": "/inrou/x86_64/rootfs.ext4",
              "initrd_image_path": null
            },
            "aarch64": {
              "kernel_image_path": "/inrou/aarch64/vmlinux",
              "rootfs_image_path": "/inrou/aarch64/rootfs.ext4",
              "initrd_image_path": null
            }
          },
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
                "initrd_image_path": null
              },
              "aarch64": {
                "kernel_image_path": "/inrou/aarch64/vmlinux",
                "rootfs_image_path": "/inrou/aarch64/rootfs.ext4",
                "initrd_image_path": null
              }
            },
            "bootstrap_user_data_path": null,
            "ssh_authorized_keys": [
              "ssh-ed25519 CHANGE_ME ton-indexer-taira"
            ]
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
        assert_eq!(
            manifest
                .inrou
                .expect("inrou config should be present")
                .guest_images[&SoraInrouGuestIsaV1::X8664]
                .kernel_image_path,
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
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "service.lease_volumes",
                ..
            }
        ));
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
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "network_egress",
                ..
            }
        ));
    }

    #[test]
    fn deployment_bundle_validate_rejects_unknown_http_service_quota_class() {
        let mut container = sample_container();
        container.runtime = SoraContainerRuntimeV1::Inrou;
        container.entrypoint = "/app/bin/service".to_string();
        container.inrou = Some(sample_inrou_manifest());
        container.capabilities.network = SoraNetworkPolicyV1::Open;
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
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "service.economics.quota_class",
                ..
            }
        ));
    }

    #[test]
    fn deployment_bundle_validate_rejects_http_service_resources_over_quota_class_cap() {
        let mut container = sample_container();
        container.runtime = SoraContainerRuntimeV1::Inrou;
        container.entrypoint = "/app/bin/service".to_string();
        container.inrou = Some(sample_inrou_manifest());
        container.capabilities.network = SoraNetworkPolicyV1::Open;
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
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "container.resources.cpu_millis",
                ..
            }
        ));
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
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "container.required_config_names",
                ..
            }
        ));
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
    fn service_runtime_state_validate_rejects_load_out_of_range() {
        let runtime_state = SoraServiceRuntimeStateV1 {
            schema_version: SORA_SERVICE_RUNTIME_STATE_VERSION_V1,
            service_name: "portal".parse().expect("valid name"),
            active_service_version: "2026.1".to_string(),
            health_status: SoraServiceHealthStatusV1::Healthy,
            load_factor_bps: 10_001,
            materialized_bundle_hash: sample_hash(160),
            rollout_handle: Some("rollout-1".to_string()),
            pending_mailbox_message_count: 2,
            last_receipt_id: Some(sample_hash(161)),
        };

        let error = runtime_state
            .validate()
            .expect_err("load factor above 10_000 bps must fail");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "load_factor_bps",
                ..
            }
        ));
    }

    #[test]
    fn service_runtime_state_validate_rejects_zero_prehash_digest_sentinels() {
        let zero_digest = zero_prehash_statement_hash();

        let mut runtime_state = sample_service_runtime_state();
        runtime_state.materialized_bundle_hash = zero_digest;
        let error = runtime_state
            .validate()
            .expect_err("materialized bundle placeholder hash must fail admission");
        assert_zero_prehash_digest_error(&error, "materialized_bundle_hash");

        let mut runtime_state = sample_service_runtime_state();
        runtime_state.last_receipt_id = Some(zero_digest);
        let error = runtime_state
            .validate()
            .expect_err("last receipt placeholder hash must fail admission");
        assert_zero_prehash_digest_error(&error, "last_receipt_id");
    }

    #[test]
    fn inrou_host_capability_record_validate_accepts_hosting_advert() {
        sample_inrou_host_capability_record()
            .validate()
            .expect("valid Inrou host capability advert should pass");
    }

    #[test]
    fn inrou_host_capability_record_validate_rejects_proxy_only_nonzero_capacity() {
        let mut capability = sample_inrou_host_capability_record();
        capability.proxy_only = true;

        let error = capability
            .validate()
            .expect_err("proxy-only adverts must not expose hosting capacity");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "proxy_only",
                ..
            }
        ));
    }

    #[test]
    fn inrou_service_placement_record_validate_rejects_duplicate_slots() {
        let mut placement = sample_inrou_service_placement_record();
        placement.placements.push(placement.placements[0].clone());

        let error = placement
            .validate()
            .expect_err("duplicate replica slots must fail validation");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "placements",
                ..
            }
        ));
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
    fn inrou_replica_runtime_state_validate_rejects_zero_prehash_digest_sentinels() {
        let zero_digest = zero_prehash_statement_hash();

        let mut runtime_state = sample_inrou_replica_runtime_state();
        runtime_state.materialized_bundle_hash = zero_digest;
        let error = runtime_state
            .validate()
            .expect_err("materialized bundle placeholder hash must fail admission");
        assert_zero_prehash_digest_error(&error, "materialized_bundle_hash");

        let mut runtime_state = sample_inrou_replica_runtime_state();
        runtime_state.last_receipt_id = Some(zero_digest);
        let error = runtime_state
            .validate()
            .expect_err("last receipt placeholder hash must fail admission");
        assert_zero_prehash_digest_error(&error, "last_receipt_id");
    }

    #[test]
    fn service_rollout_state_validate_rejects_promoted_partial_traffic() {
        let rollout = SoraServiceRolloutStateV1 {
            schema_version: SORA_SERVICE_ROLLOUT_STATE_VERSION_V1,
            rollout_handle: "portal:rollout:1".to_string(),
            baseline_version: Some("1.0.0".to_string()),
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
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "traffic_percent",
                ..
            }
        ));
    }

    #[test]
    fn service_deployment_state_validate_rejects_non_canary_active_rollout() {
        let deployment = SoraServiceDeploymentStateV1 {
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
                baseline_version: Some("1.0.0".to_string()),
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
            service_lease: None,
            lease_volume_states: Vec::new(),
        };

        let error = deployment
            .validate()
            .expect_err("active rollout must remain in canary state");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "active_rollout.stage",
                ..
            }
        ));
    }

    #[test]
    fn service_deployment_state_validate_rejects_zero_prehash_manifest_hash_sentinels() {
        let zero_digest = zero_prehash_statement_hash();

        let mut deployment = sample_service_deployment_state();
        deployment.current_service_manifest_hash = zero_digest;
        let error = deployment
            .validate()
            .expect_err("current service manifest placeholder hash must fail admission");
        assert_zero_prehash_digest_error(&error, "current_service_manifest_hash");

        let mut deployment = sample_service_deployment_state();
        deployment.current_container_manifest_hash = zero_digest;
        let error = deployment
            .validate()
            .expect_err("current container manifest placeholder hash must fail admission");
        assert_zero_prehash_digest_error(&error, "current_container_manifest_hash");
    }

    #[test]
    fn service_audit_event_validate_rejects_zero_sequence() {
        let event = SoraServiceAuditEventV1 {
            schema_version: SORA_SERVICE_AUDIT_EVENT_VERSION_V1,
            sequence: 0,
            action: SoraServiceLifecycleActionV1::Deploy,
            service_name: "portal".parse().expect("valid name"),
            from_version: None,
            to_version: "1.0.0".to_string(),
            service_manifest_hash: sample_hash(172),
            container_manifest_hash: sample_hash(173),
            governance_tx_hash: None,
            binding_name: None,
            state_key: None,
            config_name: None,
            secret_name: None,
            rollout_handle: None,
            policy_name: None,
            policy_snapshot_hash: None,
            jurisdiction_tag: None,
            consent_evidence_hash: None,
            break_glass: None,
            break_glass_reason: None,
            signer: sample_signer(),
        };

        let error = event
            .validate()
            .expect_err("audit sequences must be greater than zero");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "sequence",
                ..
            }
        ));
    }

    #[test]
    fn service_audit_event_validate_rejects_zero_prehash_digest_sentinels() {
        let zero_digest = zero_prehash_statement_hash();

        let mut event = sample_service_audit_event();
        event.service_manifest_hash = zero_digest;
        let error = event
            .validate()
            .expect_err("service manifest placeholder hash must fail admission");
        assert_zero_prehash_digest_error(&error, "service_manifest_hash");

        let mut event = sample_service_audit_event();
        event.container_manifest_hash = zero_digest;
        let error = event
            .validate()
            .expect_err("container manifest placeholder hash must fail admission");
        assert_zero_prehash_digest_error(&error, "container_manifest_hash");

        let mut event = sample_service_audit_event();
        event.governance_tx_hash = Some(zero_digest);
        let error = event
            .validate()
            .expect_err("governance transaction placeholder hash must fail admission");
        assert_zero_prehash_digest_error(&error, "governance_tx_hash");

        let mut event = sample_service_audit_event();
        event.policy_snapshot_hash = Some(zero_digest);
        let error = event
            .validate()
            .expect_err("policy snapshot placeholder hash must fail admission");
        assert_zero_prehash_digest_error(&error, "policy_snapshot_hash");

        let mut event = sample_service_audit_event();
        event.consent_evidence_hash = Some(zero_digest);
        let error = event
            .validate()
            .expect_err("consent evidence placeholder hash must fail admission");
        assert_zero_prehash_digest_error(&error, "consent_evidence_hash");
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
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "fhe_residual_multiple_bound",
                ..
            }
        ));
    }

    #[test]
    fn service_state_entry_validate_rejects_fhe_public_key_digest_on_non_fhe_rows() {
        let mut entry = sample_state_entry();
        entry.encryption = SoraStateEncryptionV1::ClientCiphertext;
        entry.fhe_public_key_digest = Some(sample_hash(149));

        let error = entry
            .validate()
            .expect_err("BFV public-key digests must only annotate FHE rows");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "fhe_public_key_digest",
                ..
            }
        ));
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
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "fhe_bound_mode",
                ..
            }
        ));

        let mut missing_bound_entry = sample_state_entry();
        missing_bound_entry.fhe_bound_mode = Some(BfvCiphertextBoundModeV1::ExactResidualMultiple);
        let error = missing_bound_entry
            .validate()
            .expect_err("BFV bound mode must require a bound value");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "fhe_bound_mode",
                ..
            }
        ));
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
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "fhe_bound_mode",
                ..
            }
        ));

        let mut exact_entry = sample_state_entry();
        exact_entry.fhe_public_key_digest = Some(sample_hash(151));
        exact_entry.fhe_residual_multiple_bound = Some(u128::MAX);
        exact_entry.fhe_bound_mode = Some(BfvCiphertextBoundModeV1::ExactResidualMultiple);
        let error = exact_entry
            .validate()
            .expect_err("over-capacity exact FHE bound must be rejected");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "fhe_residual_multiple_bound",
                ..
            }
        ));
        assert!(
            error.to_string().contains("exact residual"),
            "unexpected error: {error}"
        );

        let mut bounded_entry = sample_state_entry();
        bounded_entry.fhe_public_key_digest = Some(sample_hash(152));
        bounded_entry.fhe_residual_multiple_bound = Some(u128::MAX);
        bounded_entry.fhe_bound_mode = Some(BfvCiphertextBoundModeV1::BoundedNoise);
        let error = bounded_entry
            .validate()
            .expect_err("bounded-noise FHE bound above capacity must be rejected");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "fhe_residual_multiple_bound",
                ..
            }
        ));
        assert!(
            error.to_string().contains("bounded-noise"),
            "unexpected error: {error}"
        );
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
            action: SoraServiceLifecycleActionV1::DecryptionRequest,
            service_name: "portal".parse().expect("valid name"),
            from_version: None,
            to_version: "1.0.0".to_string(),
            service_manifest_hash: sample_hash(174),
            container_manifest_hash: sample_hash(175),
            governance_tx_hash: Some(sample_hash(176)),
            binding_name: Some("private_state".parse().expect("valid name")),
            state_key: Some("/state/private/patient-1".to_string()),
            config_name: None,
            secret_name: None,
            rollout_handle: None,
            policy_name: Some("phi_threshold_policy".parse().expect("valid name")),
            policy_snapshot_hash: Some(sample_hash(177)),
            jurisdiction_tag: Some("us_hipaa".to_string()),
            consent_evidence_hash: None,
            break_glass: Some(true),
            break_glass_reason: None,
            signer: sample_signer(),
        };

        let error = event
            .validate()
            .expect_err("break_glass events require a reason");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "break_glass_reason",
                ..
            }
        ));
    }

    #[test]
    fn service_mailbox_message_validate_rejects_expired_message() {
        let message = SoraServiceMailboxMessageV1 {
            schema_version: SORA_SERVICE_MAILBOX_MESSAGE_VERSION_V1,
            message_id: sample_hash(162),
            from_service: "portal".parse().expect("valid name"),
            from_handler: "update".parse().expect("valid name"),
            to_service: "audit".parse().expect("valid name"),
            to_handler: "private_update".parse().expect("valid name"),
            payload_bytes: b"ciphertext".to_vec(),
            payload_commitment: Hash::new(b"ciphertext"),
            enqueue_sequence: 10,
            available_after_sequence: 12,
            expires_at_sequence: Some(12),
        };

        let error = message
            .validate()
            .expect_err("message expiry must be after availability");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "expires_at_sequence",
                ..
            }
        ));
    }

    #[test]
    fn service_mailbox_message_validate_rejects_payload_commitment_mismatch() {
        let message = SoraServiceMailboxMessageV1 {
            schema_version: SORA_SERVICE_MAILBOX_MESSAGE_VERSION_V1,
            message_id: sample_hash(162),
            from_service: "portal".parse().expect("valid name"),
            from_handler: "update".parse().expect("valid name"),
            to_service: "audit".parse().expect("valid name"),
            to_handler: "private_update".parse().expect("valid name"),
            payload_bytes: b"ciphertext".to_vec(),
            payload_commitment: sample_hash(163),
            enqueue_sequence: 10,
            available_after_sequence: 10,
            expires_at_sequence: Some(12),
        };

        let error = message
            .validate()
            .expect_err("message commitment must bind the authoritative payload bytes");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "payload_commitment",
                ..
            }
        ));
    }

    #[test]
    fn service_mailbox_message_validate_rejects_zero_prehash_digest_sentinels() {
        let zero_digest = zero_prehash_statement_hash();

        let mut message = sample_service_mailbox_message();
        message.message_id = zero_digest;
        let error = message
            .validate()
            .expect_err("message placeholder id must fail admission");
        assert_zero_prehash_digest_error(&error, "message_id");

        let mut message = sample_service_mailbox_message();
        message.payload_commitment = zero_digest;
        let error = message
            .validate()
            .expect_err("payload placeholder commitment must fail admission");
        assert_zero_prehash_digest_error(&error, "payload_commitment");
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
            placement_id: None,
            selected_validator_account_id: None,
            selected_peer_id: None,
            mailbox_message_id: None,
            journal_artifact_hash: None,
            checkpoint_artifact_hash: None,
        };

        let error = receipt
            .validate()
            .expect_err("query receipts must remain certified");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "certified_by",
                ..
            }
        ));
    }

    #[test]
    fn runtime_receipt_validate_rejects_partial_host_attribution() {
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
            placement_id: Some(sample_hash(170)),
            selected_validator_account_id: Some(sample_account_id(171)),
            selected_peer_id: None,
            mailbox_message_id: None,
            journal_artifact_hash: None,
            checkpoint_artifact_hash: None,
        };

        let error = receipt
            .validate()
            .expect_err("partial host-attribution fields must be rejected");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "placement_id",
                ..
            }
        ));
    }

    #[test]
    fn runtime_receipt_validate_rejects_zero_prehash_digest_sentinels() {
        let zero_digest = zero_prehash_statement_hash();

        let mut receipt = sample_runtime_receipt();
        receipt.receipt_id = zero_digest;
        let error = receipt
            .validate()
            .expect_err("receipt placeholder id must fail admission");
        assert_zero_prehash_digest_error(&error, "receipt_id");

        let mut receipt = sample_runtime_receipt();
        receipt.request_commitment = zero_digest;
        let error = receipt
            .validate()
            .expect_err("request placeholder commitment must fail admission");
        assert_zero_prehash_digest_error(&error, "request_commitment");

        let mut receipt = sample_runtime_receipt();
        receipt.result_commitment = zero_digest;
        let error = receipt
            .validate()
            .expect_err("result placeholder commitment must fail admission");
        assert_zero_prehash_digest_error(&error, "result_commitment");

        let mut receipt = sample_runtime_receipt();
        receipt.placement_id = Some(zero_digest);
        let error = receipt
            .validate()
            .expect_err("placement placeholder id must fail admission");
        assert_zero_prehash_digest_error(&error, "placement_id");

        let mut receipt = sample_runtime_receipt();
        receipt.mailbox_message_id = Some(zero_digest);
        let error = receipt
            .validate()
            .expect_err("mailbox message placeholder id must fail admission");
        assert_zero_prehash_digest_error(&error, "mailbox_message_id");

        let mut receipt = sample_runtime_receipt();
        receipt.journal_artifact_hash = Some(zero_digest);
        let error = receipt
            .validate()
            .expect_err("journal artifact placeholder hash must fail admission");
        assert_zero_prehash_digest_error(&error, "journal_artifact_hash");

        let mut receipt = sample_runtime_receipt();
        receipt.checkpoint_artifact_hash = Some(zero_digest);
        let error = receipt
            .validate()
            .expect_err("checkpoint artifact placeholder hash must fail admission");
        assert_zero_prehash_digest_error(&error, "checkpoint_artifact_hash");
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

    #[test]
    fn agent_apartment_manifest_validate_rejects_excessive_per_tx_limit() {
        let mut manifest = sample_agent_apartment_manifest();
        manifest.spend_limits[0].max_per_tx = xor_quantity_from_nanos(50_000_000);
        let error = manifest
            .validate()
            .expect_err("per-tx spend limit above daily limit must fail");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "spend_limits.max_per_tx",
                ..
            }
        ));
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
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "manifest_hash",
                ..
            }
        ));
    }

    #[test]
    fn agent_apartment_record_validation_rejects_mailbox_payload_hash_mismatch() {
        let mut record = sample_agent_apartment_record();
        record.mailbox_queue[0].payload.push_str("tampered");
        let error = record
            .validate()
            .expect_err("mailbox payload hash must match payload bytes");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "mailbox_queue.payload_hash",
                ..
            }
        ));
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
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "autonomy_run_history",
                ..
            }
        ));
    }

    #[test]
    fn agent_apartment_audit_event_validation_rejects_empty_reason() {
        let mut event = sample_agent_apartment_audit_event();
        event.reason = Some(String::new());
        let error = event
            .validate()
            .expect_err("empty optional reason must be rejected");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "reason",
                ..
            }
        ));
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
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "result_commitment",
                ..
            }
        ));
    }

    #[test]
    fn fhe_param_set_validate_rejects_unregistered_backend() {
        let mut param_set = sample_fhe_param_set();
        param_set.backend = "fhe/bfv-rns/v2".to_string();
        let error = param_set
            .validate()
            .expect_err("first-release parameter-set admission must reject unregistered backends");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "backend",
                ..
            }
        ));
    }

    #[test]
    fn fhe_param_set_validate_rejects_unsupported_scheme() {
        let mut param_set = sample_fhe_param_set();
        param_set.scheme = FheSchemeV1::Ckks;
        let error = param_set
            .validate()
            .expect_err("first-release parameter-set admission must reject non-BFV schemes");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "scheme",
                ..
            }
        ));
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
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "parameter_digest",
                ..
            }
        ));
        assert!(error.to_string().contains("zero prehash sentinel"));

        let mut rns_digest = sample_fhe_param_set();
        rns_digest.rns_modulus_chain_digest = zero_digest;
        let error = rns_digest
            .validate()
            .expect_err("RNS modulus-chain digest placeholder must fail admission");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "rns_modulus_chain_digest",
                ..
            }
        ));
        assert!(error.to_string().contains("zero prehash sentinel"));

        let mut decomposition_digest = sample_fhe_param_set();
        decomposition_digest.key_switch_decomposition_chain_digest = zero_digest;
        let error = decomposition_digest
            .validate()
            .expect_err("key-switch decomposition digest placeholder must fail admission");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "key_switch_decomposition_chain_digest",
                ..
            }
        ));
        assert!(error.to_string().contains("zero prehash sentinel"));
    }

    #[test]
    fn fhe_param_set_validate_rejects_invalid_lifecycle_order() {
        let mut param_set = sample_fhe_param_set();
        param_set.deprecation_height = Some(8_000);
        let error = param_set
            .validate()
            .expect_err("deprecation height before activation must be rejected");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "deprecation_height",
                ..
            }
        ));
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
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "ciphertext_modulus_bits",
                ..
            }
        ));

        let mut plaintext_not_smaller = sample_fhe_param_set();
        plaintext_not_smaller.plaintext_modulus_bits = NonZeroU16::new(60).expect("nonzero");
        let error = plaintext_not_smaller
            .validate()
            .expect_err("plaintext modulus must be below ciphertext modulus");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "plaintext_modulus_bits",
                ..
            }
        ));

        let mut slot_overflow = sample_fhe_param_set();
        slot_overflow.slot_count = NonZeroU32::new(8_193).expect("nonzero");
        let error = slot_overflow
            .validate()
            .expect_err("slot count above polynomial degree must be rejected");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "slot_count",
                ..
            }
        ));

        let mut exhausted_depth = sample_fhe_param_set();
        exhausted_depth.max_multiplicative_depth = NonZeroU16::new(3).expect("nonzero");
        let error = exhausted_depth
            .validate()
            .expect_err("depth consuming the whole modulus chain must be rejected");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "max_multiplicative_depth",
                ..
            }
        ));

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
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "max_multiplicative_depth",
                ..
            }
        ));
    }

    #[test]
    fn fhe_param_set_validate_rejects_adversarial_lifecycle_claims() {
        let mut proposed_with_withdraw = sample_fhe_param_set();
        proposed_with_withdraw.lifecycle = FheParamLifecycleV1::Proposed;
        let error = proposed_with_withdraw
            .validate()
            .expect_err("proposed parameter sets cannot carry withdrawal metadata");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "lifecycle",
                ..
            }
        ));

        let mut active_without_activation = sample_fhe_param_set();
        active_without_activation.activation_height = None;
        active_without_activation.deprecation_height = None;
        active_without_activation.withdraw_height = None;
        let error = active_without_activation
            .validate()
            .expect_err("active parameter sets must declare activation height");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "lifecycle",
                ..
            }
        ));

        let mut withdrawn_without_withdraw_height = sample_fhe_param_set();
        withdrawn_without_withdraw_height.lifecycle = FheParamLifecycleV1::Withdrawn;
        withdrawn_without_withdraw_height.withdraw_height = None;
        let error = withdrawn_without_withdraw_height
            .validate()
            .expect_err("withdrawn parameter sets must carry withdraw height");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "lifecycle",
                ..
            }
        ));

        let mut withdraw_before_deprecation = sample_fhe_param_set();
        withdraw_before_deprecation.withdraw_height = Some(20_000);
        let error = withdraw_before_deprecation
            .validate()
            .expect_err("withdraw height must be after deprecation height");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "withdraw_height",
                ..
            }
        ));
    }
