const TEST_HF_COMMIT_OID: &str = "0123456789abcdef0123456789abcdef01234567";
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    net::{TcpListener, TcpStream},
    path::Path,
    process::Command,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
    time::{Instant, SystemTime, UNIX_EPOCH},
};
fn test_sorafs_retention_epoch() -> NonZeroU64 {
    NonZeroU64::new(2_000_000_000).expect("test retention epoch is nonzero")
}
fn sample_published_inrou_artifact(seed: u8) -> SoraPublishedInrouGuestImageArtifactV1 {
    SoraPublishedInrouGuestImageArtifactV1 {
        manifest_digest_hex: hex::encode([seed; 32]),
        content_cid: encode_content_cid(&sorafs_manifest::canonical_manifest_root_cid([seed; 32])),
    }
}
fn canonical_taira_inrou_source_fixture() -> UnpublishedDeploymentBundleV1 {
    canonical_taira_inrou_canary_deploy_bundle()
        .expect("build canonical Taira Inrou deploy bundle")
        .0
}
fn admit_taira_inrou_source(mut source: UnpublishedDeploymentBundleV1) -> SoraDeploymentBundleV1 {
    source.service.placement_targets = test_inrou_placement_targets(4);
    let mut admitted = source
        .into_admitted(BTreeMap::from([(
            SoraInrouGuestIsaV1::Aarch64,
            sample_published_inrou_artifact(0xAB),
        )]))
        .expect("publish canonical Taira Inrou source");
    install_taira_inrou_canary_service_version(&mut admitted)
        .expect("derive canonical Taira Inrou revision identity");
    admitted
}
fn canonical_taira_inrou_bundle_fixture() -> SoraDeploymentBundleV1 {
    admit_taira_inrou_source(canonical_taira_inrou_source_fixture())
}
fn taira_test_tempdir(prefix: &str) -> tempfile::TempDir {
    let target_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");
    let target_dir = fs::canonicalize(target_dir).expect("canonical workspace target directory");
    tempfile::Builder::new()
        .prefix(prefix)
        .tempdir_in(target_dir)
        .expect("workspace target tempdir")
}
fn refresh_taira_container_reference(bundle: &mut SoraDeploymentBundleV1) {
    bundle.service.container.manifest_hash = Hash::new(Encode::encode(&bundle.container));
    bundle.service.container.expected_schema_version = bundle.container.schema_version;
}
fn refresh_taira_source_container_reference(bundle: &mut UnpublishedDeploymentBundleV1) {
    bundle.service.container.manifest_hash = bundle
        .container
        .workspace_hash()
        .expect("hash unpublished Taira container");
    bundle.service.container.expected_schema_version = bundle.container.schema_version;
}
fn assert_taira_canary_validation_error(bundle: &SoraDeploymentBundleV1, needle: &str) {
    let error = validate_taira_inrou_canary_bundle(bundle)
        .expect_err("noncanonical Taira canary must fail");
    assert!(error.to_string().contains(needle), "{error}");
}
fn taira_source_guest_image_json(value: &mut Value) -> &mut BTreeMap<String, Value> {
    value
        .as_object_mut()
        .and_then(|container| container.get_mut("inrou"))
        .and_then(Value::as_object_mut)
        .and_then(|inrou| inrou.get_mut("guest_images"))
        .and_then(Value::as_object_mut)
        .and_then(|images| images.get_mut("aarch64"))
        .and_then(Value::as_object_mut)
        .expect("canonical Taira source AArch64 image object")
}
#[test]
fn unpublished_inrou_workspace_json_requires_explicit_null_artifact() {
    let source = canonical_taira_inrou_source_fixture();
    let canonical =
        norito::json::to_value(&source.container).expect("encode unpublished Taira container");
    let decoded: UnpublishedContainerManifestV1 = norito::json::from_value(canonical.clone())
        .expect("explicit-null unpublished container must decode");
    assert_eq!(decoded, source.container);
    assert!(
        taira_source_guest_image_json(&mut canonical.clone())
            .get("published_artifact")
            .is_some_and(Value::is_null)
    );

    let mut missing = canonical.clone();
    assert!(
        taira_source_guest_image_json(&mut missing)
            .remove("published_artifact")
            .is_some()
    );
    norito::json::from_value::<UnpublishedContainerManifestV1>(missing)
        .expect_err("omitted unpublished artifact marker must fail");

    let mut object = canonical.clone();
    taira_source_guest_image_json(&mut object).insert(
        "published_artifact".to_owned(),
        norito::json::to_value(&sample_published_inrou_artifact(0xA1))
            .expect("encode forbidden artifact object"),
    );
    norito::json::from_value::<UnpublishedContainerManifestV1>(object)
        .expect_err("prepublished artifact object must fail draft decoding");

    let mut unknown = canonical;
    taira_source_guest_image_json(&mut unknown).insert("retired_v0".to_owned(), Value::from(true));
    norito::json::from_value::<UnpublishedContainerManifestV1>(unknown)
        .expect_err("unpublished guest image must reject unknown fields");
}
#[test]
fn unpublished_inrou_publication_constructs_admitted_bundle_and_refreshes_hash() {
    let source = canonical_taira_inrou_source_fixture();
    let source_hash = source.service.container.manifest_hash;
    let admitted = admit_taira_inrou_source(source);
    admitted
        .validate_for_admission()
        .expect("publication must return an admission-valid bundle");
    assert_ne!(admitted.service.container.manifest_hash, source_hash);
    assert_eq!(
        admitted.service.container.manifest_hash,
        Hash::new(Encode::encode(&admitted.container))
    );
    assert_eq!(
        admitted
            .container
            .inrou
            .as_ref()
            .expect("Inrou manifest")
            .guest_images[&SoraInrouGuestIsaV1::Aarch64]
            .published_artifact,
        sample_published_inrou_artifact(0xAB)
    );

    let _error = canonical_taira_inrou_source_fixture()
        .into_admitted(BTreeMap::new())
        .expect_err("every draft guest ISA must have a published artifact");
    let _error = canonical_taira_inrou_source_fixture()
        .into_admitted(BTreeMap::from([
            (
                SoraInrouGuestIsaV1::Aarch64,
                sample_published_inrou_artifact(0xA2),
            ),
            (
                SoraInrouGuestIsaV1::X8664,
                sample_published_inrou_artifact(0xA3),
            ),
        ]))
        .expect_err("publication must reject artifacts absent from the source workspace");

    let mut dual_source =
        build_split_app_live_service_bundle("dual_source", "dual-source.sora", "1.0.0")
            .expect("build dual-ISA unpublished source");
    let published_artifacts = BTreeMap::from([
        (
            SoraInrouGuestIsaV1::X8664,
            sample_published_inrou_artifact(0xB1),
        ),
        (
            SoraInrouGuestIsaV1::Aarch64,
            sample_published_inrou_artifact(0xB2),
        ),
    ]);
    let error = dual_source
        .clone()
        .into_admitted(published_artifacts.clone())
        .expect_err("published guests alone cannot authorize an unplaced Inrou service");
    assert!(
        format!("{error:#}").contains("identity-bound operator-preseed placement targets"),
        "{error:#}"
    );
    dual_source.service.placement_targets =
        test_inrou_placement_targets(usize::from(dual_source.service.replicas.get()));
    let dual_admitted = dual_source
        .into_admitted(published_artifacts)
        .expect("publication must fill every source guest ISA");
    let images = &dual_admitted
        .container
        .inrou
        .as_ref()
        .expect("dual-ISA admitted manifest")
        .guest_images;
    assert_eq!(images.len(), 2);
    assert_eq!(
        images[&SoraInrouGuestIsaV1::X8664].published_artifact,
        sample_published_inrou_artifact(0xB1)
    );
    assert_eq!(
        images[&SoraInrouGuestIsaV1::Aarch64].published_artifact,
        sample_published_inrou_artifact(0xB2)
    );
}
#[test]
fn unpublished_source_validation_reuses_common_container_rules() {
    validate_unpublished_deployment_source(&canonical_taira_inrou_source_fixture())
        .expect("canonical unpublished source must validate");
    let _error = UnpublishedContainerManifestV1::from_non_inrou_manifest(
        canonical_taira_inrou_bundle_fixture().container,
    )
    .expect_err("an admitted Inrou manifest must never be downgraded to a workspace draft");
    let mut invalid = canonical_taira_inrou_source_fixture();
    invalid.container.bundle_path.clear();
    refresh_taira_source_container_reference(&mut invalid);
    let _error = validate_unpublished_deployment_source(&invalid)
        .expect_err("common container bundle-path validation must run before upload");

    let mut missing_healthcheck = canonical_taira_inrou_source_fixture();
    missing_healthcheck.container.lifecycle.healthcheck_path = None;
    refresh_taira_source_container_reference(&mut missing_healthcheck);
    let _error = validate_unpublished_deployment_source(&missing_healthcheck)
        .expect_err("cross-manifest lifecycle validation must run before upload");
}
#[test]
fn inrou_preseed_artifact_count_enforces_single_session_boundary() {
    let bundle =
        build_split_app_live_service_bundle("preseed_boundary", "preseed-boundary.sora", "1.0.0")
            .expect("build dual-ISA public Inrou service");
    let mut bundles = vec![bundle; OPERATOR_PRESEED_SESSION_MAX_ARTIFACTS_V1 / 4];
    validate_inrou_preseed_artifact_count(bundles.iter())
        .expect("64 dual-ISA public services produce exactly 256 artifacts");
    bundles.push(bundles[0].clone());
    let error = validate_inrou_preseed_artifact_count(bundles.iter())
        .expect_err("65 dual-ISA public services exceed one bounded preseed session");
    assert!(
        error.to_string().contains("requires 260 artifacts"),
        "{error}"
    );
}
#[test]
fn taira_inrou_canary_validator_accepts_exact_v1_bundle() {
    let bundle = canonical_taira_inrou_bundle_fixture();
    validate_taira_inrou_canary_bundle(&bundle).expect("canonical Taira Inrou V1 bundle");
    let source = canonical_taira_inrou_source_fixture();
    validate_taira_inrou_canary_source_bundle(&source)
        .expect("canonical Taira Inrou V1 source bundle");
}
#[test]
fn taira_inrou_canary_storage_accounts_for_distinct_temporary_and_shared_limits() {
    let bundle = canonical_taira_inrou_bundle_fixture();
    let resources = &bundle.container.resources;
    let [root, shared] = bundle.service.lease_volumes.as_slice() else {
        panic!("canonical root and shared volumes");
    };
    assert_ne!(resources.ephemeral_storage_bytes, shared.max_total_bytes);
    assert_eq!(
        root.max_total_bytes.get()
            + resources.ephemeral_storage_bytes.get()
            + shared.max_total_bytes.get(),
        defaults::taira::INROU_MAX_STORAGE_BYTES
    );
    validate_taira_inrou_canary_storage(resources, &bundle.service)
        .expect("temporary storage and shared leases have separate budgets");
    let mut wrong_resources = resources.clone();
    wrong_resources.ephemeral_storage_bytes = shared.max_total_bytes;
    let _ = validate_taira_inrou_canary_storage(&wrong_resources, &bundle.service)
        .expect_err("shared volume capacity must not replace the temporary budget");
    let mut wrong_service = bundle.service.clone();
    wrong_service.lease_volumes[1].max_total_bytes = resources.ephemeral_storage_bytes;
    let _ = validate_taira_inrou_canary_storage(resources, &wrong_service)
        .expect_err("temporary capacity must not replace the shared volume budget");
}
#[test]
fn taira_inrou_canary_source_rejects_valid_noncanonical_policy_values() {
    let mut lifecycle = canonical_taira_inrou_source_fixture();
    lifecycle.container.lifecycle.start_grace_secs =
        NonZeroU32::new(lifecycle.container.lifecycle.start_grace_secs.get() + 1)
            .expect("nonzero lifecycle grace");
    refresh_taira_source_container_reference(&mut lifecycle);
    validate_taira_inrou_canary_bundle(&admit_taira_inrou_source(lifecycle.clone()))
        .expect("generic admission still accepts the alternate lifecycle");
    let _error = validate_taira_inrou_canary_source_bundle(&lifecycle)
        .expect_err("release staging must reject an alternate lifecycle");

    let mut rollout = canonical_taira_inrou_source_fixture();
    rollout.service.rollout.health_window_secs =
        NonZeroU32::new(rollout.service.rollout.health_window_secs.get() + 1)
            .expect("nonzero rollout window");
    validate_taira_inrou_canary_bundle(&admit_taira_inrou_source(rollout.clone()))
        .expect("generic admission still accepts the alternate rollout policy");
    let _error = validate_taira_inrou_canary_source_bundle(&rollout)
        .expect_err("release staging must reject an alternate rollout policy");
}
#[test]
fn taira_inrou_canary_bundle_payload_is_the_exact_canonical_archive() {
    use sorafs_car::bundle_archive::{BundleArchiveLimits, visit_gzip_ustar};

    let payload = canonical_taira_inrou_canary_bundle_payload()
        .expect("encode canonical Taira Inrou server archive");
    assert_eq!(
        payload,
        canonical_taira_inrou_canary_bundle_payload()
            .expect("repeat canonical Taira Inrou server archive encoding")
    );
    validate_taira_inrou_canary_bundle_payload(&payload)
        .expect("accept exact canonical Taira Inrou server archive");

    let source_bytes =
        u64::try_from(TAIRA_INROU_CANARY_SERVER_SOURCE_V1.len()).expect("source byte length");
    let mut files = Vec::new();
    let summary = visit_gzip_ustar(
        std::io::Cursor::new(&payload),
        BundleArchiveLimits {
            max_compressed_bytes: u64::try_from(payload.len()).expect("archive byte length"),
            max_decoded_bytes: 1024 * 1024,
            max_entries: 1,
            max_file_bytes: source_bytes,
            max_total_file_bytes: source_bytes,
        },
        |entry, reader| {
            let mut source = Vec::new();
            std::io::Read::read_to_end(reader, &mut source)?;
            files.push((entry.path().to_owned(), entry.mode(), source));
            Ok(())
        },
    )
    .expect("decode canonical Taira Inrou server archive");
    assert_eq!(summary.entry_count(), 1);
    assert_eq!(summary.file_count(), 1);
    assert_eq!(summary.total_file_bytes(), source_bytes);
    assert_eq!(
        files,
        vec![(
            TAIRA_INROU_CANARY_BUNDLE_MEMBER_V1.to_owned(),
            0o755,
            TAIRA_INROU_CANARY_SERVER_SOURCE_V1.to_vec(),
        )]
    );
}
#[test]
fn taira_inrou_workspace_generator_emits_exact_private_deploy_layout() {
    let temp = taira_test_tempdir("taira-inrou-workspace-");
    let kernel = temp.path().join("kernel.source");
    let rootfs = temp.path().join("rootfs.source");
    let initrd = temp.path().join("initrd.source");
    fs::write(&kernel, b"aarch64-kernel").expect("write kernel source");
    fs::write(&rootfs, b"aarch64-rootfs").expect("write rootfs source");
    fs::write(&initrd, b"aarch64-initrd").expect("write initrd source");
    let output = temp.path().join("workspace");

    let receipt = create_taira_inrou_canary_workspace(&kernel, &rootfs, &initrd, &output)
        .expect("generate canonical Taira Inrou workspace");
    assert_eq!(
        receipt.schema_version,
        TAIRA_INROU_WORKSPACE_SCHEMA_VERSION_V1
    );
    assert_eq!(
        receipt.guest_total_bytes,
        u64::try_from(b"aarch64-kernel".len() + b"aarch64-rootfs".len() + b"aarch64-initrd".len())
            .expect("guest fixture length")
    );
    let mut root_entries = fs::read_dir(&output)
        .expect("read workspace")
        .map(|entry| {
            entry
                .expect("workspace entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();
    root_entries.sort();
    assert_eq!(
        root_entries,
        [
            "bundle.tgz",
            "container_manifest.json",
            "inrou",
            "service_manifest.json",
        ]
    );
    let inrou = output.join("inrou");
    let aarch64 = inrou.join("aarch64");
    let mut guest_entries = fs::read_dir(&aarch64)
        .expect("read AArch64 workspace")
        .map(|entry| {
            entry
                .expect("guest entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();
    guest_entries.sort();
    assert_eq!(guest_entries, ["initrd.img", "rootfs.ext4", "vmlinux"]);
    assert_eq!(
        fs::read(aarch64.join("vmlinux")).expect("read copied kernel"),
        b"aarch64-kernel"
    );
    assert_eq!(
        fs::read(aarch64.join("rootfs.ext4")).expect("read copied rootfs"),
        b"aarch64-rootfs"
    );
    assert_eq!(
        fs::read(aarch64.join("initrd.img")).expect("read copied initrd"),
        b"aarch64-initrd"
    );
    let bundle_payload = fs::read(output.join("bundle.tgz")).expect("read bundle");
    assert_eq!(
        bundle_payload,
        canonical_taira_inrou_canary_bundle_payload().expect("canonical bundle")
    );
    let container: UnpublishedContainerManifestV1 =
        load_json(&output.join("container_manifest.json")).expect("generated container");
    let service: SoraServiceManifestV1 =
        load_json(&output.join("service_manifest.json")).expect("generated service");
    let generated = UnpublishedDeploymentBundleV1 { container, service };
    validate_taira_inrou_canary_source_bundle(&generated)
        .expect("generated deploy source is canonical");
    assert_eq!(generated.container.bundle_hash, Hash::new(&bundle_payload));
    assert_eq!(
        receipt.bundle_hash,
        generated.container.bundle_hash.to_string()
    );
    assert_eq!(
        receipt.container_manifest_hash,
        generated
            .container
            .workspace_hash()
            .expect("hash generated source container")
            .to_string()
    );
    assert_eq!(
        receipt.service_manifest_hash,
        Hash::new(Encode::encode(&generated.service)).to_string()
    );

    let stage_dir = temp.path().join("stage");
    let stage_key = soracloud_fixture_key_pair(0x6A);
    let stage_receipt = stage_taira_inrou_canary_deployment(
        crate::taira::InrouCanaryMode::Deploy,
        &output.join("container_manifest.json"),
        &output.join("service_manifest.json"),
        &output.join("bundle.tgz"),
        &stage_dir,
        &stage_key,
        test_sorafs_retention_epoch(),
        test_inrou_placement_targets(4),
    )
    .expect("consume strict-null Taira source into an admitted stage");
    assert_eq!(
        stage_receipt.sorafs_retention_epoch,
        test_sorafs_retention_epoch().get()
    );
    let staged = load_verified_taira_inrou_stage(&stage_dir, &stage_key, MutationMode::Deploy)
        .expect("verify admitted Taira stage");
    assert_eq!(
        staged.bundle_manifest.manifest.pin_policy.retention_epoch,
        test_sorafs_retention_epoch().get()
    );
    assert_eq!(
        staged.guest_manifest.manifest.pin_policy.retention_epoch,
        test_sorafs_retention_epoch().get()
    );
    assert_eq!(
        staged
            .discovery_manifest
            .manifest
            .pin_policy
            .retention_epoch,
        test_sorafs_retention_epoch().get()
    );
    assert_eq!(
        staged.discovery.document_hash.to_string(),
        stage_receipt.discovery_document_hash
    );
    assert_eq!(
        staged.discovery.content_cid,
        stage_receipt.discovery_content_cid
    );
    let retry_stage_dir = temp.path().join("stage-retry");
    let retry_receipt = stage_taira_inrou_canary_deployment(
        crate::taira::InrouCanaryMode::Deploy,
        &output.join("container_manifest.json"),
        &output.join("service_manifest.json"),
        &output.join("bundle.tgz"),
        &retry_stage_dir,
        &stage_key,
        test_sorafs_retention_epoch(),
        test_inrou_placement_targets(4),
    )
    .expect("retry the exact Taira stage with the retained release identity");
    assert_eq!(
        json::to_vec(&retry_receipt).expect("encode retried Taira receipt"),
        json::to_vec(&stage_receipt).expect("encode initial Taira receipt"),
        "the same explicit retention epoch must reproduce the entire Taira receipt"
    );
    for manifest_path in [
        TAIRA_INROU_STAGE_BUNDLE_MANIFEST_FILE_V1,
        TAIRA_INROU_STAGE_GUEST_MANIFEST_FILE_V1,
        TAIRA_INROU_STAGE_DISCOVERY_MANIFEST_FILE_V1,
    ] {
        assert_eq!(
            fs::read(stage_dir.join(manifest_path)).expect("read initial staged manifest"),
            fs::read(retry_stage_dir.join(manifest_path)).expect("read retried staged manifest"),
            "Taira stage retry must reproduce byte-identical {manifest_path}"
        );
    }
    let mut stage_config = crate::fallback_config();
    stage_config.account = AccountId::new(stage_key.public_key().clone());
    stage_config.key_pair = stage_key.clone();
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
        // Match the public-reset runtime snapshot's exact file custody before
        // exercising the complete identity, payload and manifest verifier.
        let mut pending = vec![stage_dir.clone()];
        let mut frozen_files = 0;
        while let Some(directory) = pending.pop() {
            for entry in fs::read_dir(directory).expect("read generated stage") {
                let path = entry.expect("generated stage entry").path();
                let metadata = fs::symlink_metadata(&path).expect("stage entry metadata");
                if metadata.is_dir() {
                    assert_eq!(metadata.mode() & 0o7777, 0o700);
                    pending.push(path);
                } else {
                    assert!(metadata.is_file());
                    assert_eq!(metadata.mode() & 0o7777, 0o600);
                    let before = fs::read(&path).expect("read prepared stage bytes");
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o400))
                        .expect("freeze runtime stage file");
                    assert_eq!(
                        fs::read(&path).expect("read frozen stage bytes"),
                        before,
                        "freezing stage custody must preserve all signed bytes"
                    );
                    frozen_files += 1;
                }
            }
        }
        assert!(frozen_files >= 11, "freeze the complete generated stage");
    }
    let read_only_identity = load_taira_inrou_stage_identity(
        &stage_config,
        &stage_dir,
        crate::taira::InrouCanaryMode::Deploy,
    )
    .expect("revalidate the complete owner-readonly runtime stage without mutation");
    assert_eq!(
        read_only_identity.container_manifest_hash,
        stage_receipt.container_manifest_hash
    );
    assert_eq!(
        read_only_identity.service_manifest_hash,
        stage_receipt.service_manifest_hash
    );
    assert_eq!(
        read_only_identity.deployment_bundle_hash,
        staged.discovery.deployment_bundle_hash.to_string()
    );
    let pin_server = MockHttpServer::start(BTreeMap::from([(
        "/v1/sorafs/pin/register".to_owned(),
        MockHttpResponse::json(
            json::to_vec(&norito::json!({ "ok": true })).expect("encode mock staged pin response"),
        ),
    )]));
    assert_eq!(
        taira_inrou_canary_pin_readiness_v1(
            &stage_config,
            &stage_dir,
            &pin_server.base_url,
            Duration::from_secs(5),
            crate::taira::InrouCanaryMode::Deploy,
            TairaInrouCanaryPreparedOperationV1::DiscoveryPin,
        )
        .expect("read missing staged discovery pin readiness"),
        TairaInrouCanaryPinReadinessV1::Missing
    );
    install_mock_submission_config(&stage_config.account, &stage_key);
    register_built_sorafs_manifest(
        &staged.discovery_manifest,
        "staged Taira public discovery",
        &pin_server.base_url,
        &stage_config.account,
        &stage_key,
        5,
    )
    .expect("register staged discovery manifest through mock finalized governance");
    assert_eq!(
        taira_inrou_canary_pin_readiness_v1(
            &stage_config,
            &stage_dir,
            &pin_server.base_url,
            Duration::from_secs(5),
            crate::taira::InrouCanaryMode::Deploy,
            TairaInrouCanaryPreparedOperationV1::DiscoveryPin,
        )
        .expect("read approved staged discovery pin readiness"),
        TairaInrouCanaryPinReadinessV1::Approved(2)
    );
    assert_eq!(read_only_identity.stage_mode, "deploy");
    assert_eq!(
        read_only_identity.discovery_document_hash,
        stage_receipt.discovery_document_hash
    );
    assert_eq!(
        read_only_identity.public_discovery_url,
        stage_receipt.public_discovery_url
    );
    assert_eq!(
        read_only_identity.public_discovery_cid_host_url,
        stage_receipt.public_discovery_cid_host_url
    );
    let published = &staged
        .bundle
        .container
        .inrou
        .as_ref()
        .expect("staged Inrou manifest")
        .guest_images[&SoraInrouGuestIsaV1::Aarch64]
        .published_artifact;
    assert_eq!(
        published.manifest_digest_hex,
        stage_receipt.guest_manifest_digest_hex
    );
    assert_eq!(published.content_cid, stage_receipt.guest_content_cid);
    assert_eq!(
        staged.bundle.service.container.manifest_hash,
        Hash::new(Encode::encode(&staged.bundle.container))
    );

    let retry_discovery_path = retry_stage_dir.join(TAIRA_INROU_STAGE_DISCOVERY_DOCUMENT_FILE_V1);
    let mut tampered_discovery =
        fs::read(&retry_discovery_path).expect("read retried discovery document");
    let tamper_index = tampered_discovery.len() / 2;
    tampered_discovery[tamper_index] ^= 1;
    fs::write(&retry_discovery_path, tampered_discovery)
        .expect("tamper retried discovery document");
    assert!(
        load_verified_taira_inrou_stage(&retry_stage_dir, &stage_key, MutationMode::Deploy,)
            .is_err(),
        "one changed discovery byte must invalidate the retained stage"
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        for directory in [&output, &inrou, &aarch64] {
            let metadata = fs::symlink_metadata(directory).expect("workspace directory mode");
            assert_eq!(metadata.mode() & 0o7777, 0o700);
        }
        for file in [
            output.join("container_manifest.json"),
            output.join("service_manifest.json"),
            output.join("bundle.tgz"),
            aarch64.join("vmlinux"),
            aarch64.join("rootfs.ext4"),
            aarch64.join("initrd.img"),
        ] {
            let metadata = fs::symlink_metadata(file).expect("workspace file mode");
            assert_eq!(metadata.mode() & 0o7777, 0o600);
            assert_eq!(metadata.nlink(), 1);
        }
    }

    let _error = create_taira_inrou_canary_workspace(&kernel, &rootfs, &initrd, &output)
        .expect_err("existing workspaces must never be reused");
    validate_generated_taira_inrou_workspace(&output)
        .expect("failed reuse must preserve the original valid workspace");
}
#[cfg(unix)]
#[test]
fn taira_inrou_workspace_generator_rejects_linked_or_empty_assets_before_creation() {
    use std::os::unix::fs::symlink;

    let temp = taira_test_tempdir("taira-inrou-workspace-inputs-");
    let kernel = temp.path().join("kernel.source");
    let linked_kernel = temp.path().join("kernel.link");
    let rootfs = temp.path().join("rootfs.source");
    let initrd = temp.path().join("initrd.source");
    fs::write(&kernel, b"kernel").expect("write kernel source");
    symlink(&kernel, &linked_kernel).expect("link kernel source");
    fs::write(&rootfs, b"rootfs").expect("write rootfs source");
    fs::write(&initrd, b"initrd").expect("write initrd source");
    let linked_output = temp.path().join("linked-output");
    let error =
        create_taira_inrou_canary_workspace(&linked_kernel, &rootfs, &initrd, &linked_output)
            .expect_err("symlinked kernel must fail closed");
    assert!(error.to_string().contains("direct regular file"), "{error}");
    assert!(!linked_output.exists());

    fs::write(&initrd, []).expect("empty initrd source");
    let empty_output = temp.path().join("empty-output");
    let error = create_taira_inrou_canary_workspace(&kernel, &rootfs, &initrd, &empty_output)
        .expect_err("empty initrd must fail closed");
    assert!(error.to_string().contains("must not be empty"), "{error}");
    assert!(!empty_output.exists());
}
#[test]
fn taira_inrou_canary_bundle_payload_rejects_byte_tampering() {
    let canonical = canonical_taira_inrou_canary_bundle_payload()
        .expect("encode canonical Taira Inrou server archive");
    let mut tampered = canonical.clone();
    let tamper_index = tampered.len() / 2;
    tampered[tamper_index] ^= 1;
    let error = validate_taira_inrou_canary_bundle_payload(&tampered)
        .expect_err("one changed archive byte must fail");
    assert!(
        error.to_string().contains("exact deterministic gzip/USTAR"),
        "{error}"
    );

    let mut trailing = canonical;
    trailing.push(0);
    let _error = validate_taira_inrou_canary_bundle_payload(&trailing)
        .expect_err("trailing archive bytes must fail");
}
fn taira_inrou_canary_python_command() -> Command {
    let mut command = Command::new("python3");
    command
        .arg("-I")
        .arg(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("src/soracloud/taira_inrou_canary_server_v1.py"),
        )
        .env_remove("PORT")
        .env_remove("HTTP_SERVICE_NAME")
        .env_remove("SORACLOUD_REPLICA_SLOT")
        .env_remove("SORACLOUD_SERVICE_VERSION")
        .env_remove("SORACLOUD_LEASE_VOLUME_APP_DATA_DIR")
        .env_remove("SORACLOUD_LEASE_VOLUME_APP_DATA_MOUNT_PATH");
    command
}
#[test]
fn taira_inrou_canary_python_server_rejects_missing_or_blank_version() {
    for blank_version in [false, true] {
        let mut command = taira_inrou_canary_python_command();
        command
            .env("PORT", "8787")
            .env("HTTP_SERVICE_NAME", TAIRA_INROU_CANARY_SERVICE_NAME_V1)
            .env("SORACLOUD_REPLICA_SLOT", "1");
        if blank_version {
            command.env("SORACLOUD_SERVICE_VERSION", "");
        }
        let output = command.output().expect("run canonical Python server");
        assert!(
            !output.status.success(),
            "missing or blank service version must fail startup"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("SORACLOUD_SERVICE_VERSION"),
            "unexpected startup failure: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
#[test]
fn taira_inrou_canary_python_server_requires_exact_app_data_projection() {
    let mut command = taira_inrou_canary_python_command();
    let output = command
        .env("PORT", "8787")
        .env("HTTP_SERVICE_NAME", TAIRA_INROU_CANARY_SERVICE_NAME_V1)
        .env("SORACLOUD_REPLICA_SLOT", "1")
        .env(
            "SORACLOUD_SERVICE_VERSION",
            format!(
                "{TAIRA_INROU_CANARY_SERVICE_VERSION_PREFIX_V1}{}",
                "ab".repeat(32)
            ),
        )
        .output()
        .expect("run canonical Python server");
    assert!(
        !output.status.success(),
        "missing app-data projection must fail startup"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("SORACLOUD_LEASE_VOLUME_APP_DATA_DIR"),
        "unexpected startup failure: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn taira_inrou_canary_python_server_rejects_noncanonical_app_data_path() {
    let temp = taira_test_tempdir("taira-inrou-python-wrong-app-data-");
    let noncanonical_path = temp.path().join("app-data");
    let mut command = taira_inrou_canary_python_command();
    let output = command
        .env("PORT", "8787")
        .env("HTTP_SERVICE_NAME", TAIRA_INROU_CANARY_SERVICE_NAME_V1)
        .env("SORACLOUD_REPLICA_SLOT", "1")
        .env(
            "SORACLOUD_SERVICE_VERSION",
            format!(
                "{TAIRA_INROU_CANARY_SERVICE_VERSION_PREFIX_V1}{}",
                "ab".repeat(32)
            ),
        )
        .env("SORACLOUD_LEASE_VOLUME_APP_DATA_DIR", &noncanonical_path)
        .env(
            "SORACLOUD_LEASE_VOLUME_APP_DATA_MOUNT_PATH",
            &noncanonical_path,
        )
        .output()
        .expect("run canonical Python server");
    assert!(
        !output.status.success(),
        "a noncanonical app-data path must fail startup"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("app-data directory must be exactly /var/lib/soracloud/volumes/app_data"),
        "unexpected startup failure: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[cfg(unix)]
fn run_taira_inrou_python_state_transition(
    app_data_dir: &Path,
    guest_boot_id: &str,
) -> std::process::Output {
    const HARNESS: &str = r#"
import importlib.util
import json
import sys

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("taira_inrou_canary_server_v1", sys.argv[1])
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
state = module.load_or_create_health_state(
    sys.argv[2], "taira_inrou_canary", "artifact-test-v1", 2, sys.argv[3]
)
sys.stdout.write(json.dumps(state, ensure_ascii=True, separators=(",", ":")))
"#;
    Command::new("python3")
        .arg("-I")
        .arg("-c")
        .arg(HARNESS)
        .arg(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("src/soracloud/taira_inrou_canary_server_v1.py"),
        )
        .arg(app_data_dir)
        .arg(guest_boot_id)
        .output()
        .expect("run synthetic Taira Inrou durable-state transition")
}
#[cfg(unix)]
#[test]
fn taira_inrou_canary_durable_state_advances_only_for_a_new_guest_boot() {
    use std::os::unix::fs::PermissionsExt as _;

    let temp = taira_test_tempdir("taira-inrou-python-state-");
    let app_data_dir = temp.path().join("app-data");
    fs::create_dir(&app_data_dir).expect("create synthetic app-data volume");
    fs::set_permissions(&app_data_dir, fs::Permissions::from_mode(0o700))
        .expect("set exact synthetic app-data mode");
    let boot_a = "11111111-1111-1111-1111-111111111111";
    let boot_b = "22222222-2222-2222-2222-222222222222";

    let run = |boot_id| {
        let output = run_taira_inrou_python_state_transition(&app_data_dir, boot_id);
        assert!(
            output.status.success(),
            "synthetic state transition failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        json::from_slice::<Value>(&output.stdout).expect("decode synthetic durable state")
    };
    let first = run(boot_a);
    let same_boot = run(boot_a);
    assert_eq!(
        first, same_boot,
        "a service restart must not advance guest boot state"
    );
    assert_eq!(first.get("boot_sequence").and_then(Value::as_u64), Some(1));

    let next_boot = run(boot_b);
    assert_eq!(
        next_boot.get("boot_sequence").and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(next_boot.get("marker_hex"), first.get("marker_hex"));
    assert_ne!(
        next_boot.get("last_guest_boot_id_sha256"),
        first.get("last_guest_boot_id_sha256")
    );

    let state_path = app_data_dir.join("taira-inrou-canary-state-v1.json");
    let state_bytes = fs::read(&state_path).expect("read installed durable state");
    let state: Value = json::from_slice(&state_bytes).expect("decode installed durable state");
    assert_eq!(
        state.as_object().map(|object| object.len()),
        Some(7),
        "durable state must have the exact V1 fields"
    );
    assert_eq!(
        fs::metadata(&state_path)
            .expect("stat installed durable state")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );

    let corrupt = br#"{"schema_version":1,"legacy_state":null}"#;
    fs::write(&state_path, corrupt).expect("install corrupt state fixture");
    let rejected = run_taira_inrou_python_state_transition(&app_data_dir, boot_b);
    assert!(
        !rejected.status.success(),
        "corrupt durable state must fail closed"
    );
    assert!(
        String::from_utf8_lossy(&rejected.stderr).contains("durable state"),
        "unexpected corruption failure: {}",
        String::from_utf8_lossy(&rejected.stderr)
    );
    assert_eq!(
        fs::read(&state_path).expect("read rejected state"),
        corrupt,
        "startup must not repair or replace corrupt durable state"
    );
}
#[cfg(target_os = "linux")]
fn fetch_taira_inrou_python_health(port: u16) -> std::io::Result<Vec<u8>> {
    let mut stream = TcpStream::connect(("127.0.0.1", port))?;
    stream.set_read_timeout(Some(Duration::from_millis(250)))?;
    std::io::Write::write_all(
        &mut stream,
        b"GET /health?taira_inrou_probe=1 HTTP/1.1\r\nHost: taira.sora.org\r\nAccept: application/json\r\nConnection: close\r\n\r\n",
    )?;
    let mut response = Vec::new();
    std::io::Read::read_to_end(&mut stream, &mut response)?;
    Ok(response)
}
#[cfg(target_os = "linux")]
#[test]
fn taira_inrou_canary_python_server_emits_exact_health_identity() {
    use std::os::unix::fs::PermissionsExt as _;

    const HANDLER_HARNESS: &str = r#"
import importlib.util
import sys

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("taira_inrou_canary_server_v1", sys.argv[1])
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
state = module.load_or_create_health_state(
    sys.argv[2], "taira_inrou_canary", sys.argv[4], 3,
    "33333333-3333-3333-3333-333333333333"
)
module.HealthHandler.payload = module.health_payload(
    "taira_inrou_canary", sys.argv[4], 3, state
)
module.HTTPServer(("127.0.0.1", int(sys.argv[3])), module.HealthHandler).serve_forever()
"#;
    let reserved = TcpListener::bind(("127.0.0.1", 0)).expect("reserve loopback port");
    let port = reserved.local_addr().expect("reserved address").port();
    drop(reserved);
    let temp = taira_test_tempdir("taira-inrou-python-health-");
    let app_data_dir = temp.path().join("app-data");
    fs::create_dir(&app_data_dir).expect("create app-data volume");
    fs::set_permissions(&app_data_dir, fs::Permissions::from_mode(0o700))
        .expect("set exact app-data volume mode");

    let service_version = format!(
        "{TAIRA_INROU_CANARY_SERVICE_VERSION_PREFIX_V1}{}",
        "ab".repeat(32)
    );
    let mut command = Command::new("python3");
    let mut child = command
        .arg("-I")
        .arg("-c")
        .arg(HANDLER_HARNESS)
        .arg(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("src/soracloud/taira_inrou_canary_server_v1.py"),
        )
        .arg(&app_data_dir)
        .arg(port.to_string())
        .arg(&service_version)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("start canonical Taira Inrou Python server");
    let deadline = Instant::now() + Duration::from_secs(5);
    let response = loop {
        if let Some(status) = child.try_wait().expect("poll Python server") {
            break Err(format!("Python server exited early with {status}"));
        }
        match fetch_taira_inrou_python_health(port) {
            Ok(response) if response.starts_with(b"HTTP/1.0 200 ") => break Ok(response),
            Ok(response) => {
                if Instant::now() >= deadline {
                    break Err(format!(
                        "Python server returned an unexpected response: {}",
                        String::from_utf8_lossy(&response)
                    ));
                }
            }
            Err(error) => {
                if Instant::now() >= deadline {
                    break Err(format!("Python server did not become ready: {error}"));
                }
            }
        }
        thread::sleep(Duration::from_millis(20));
    };
    let _ = child.kill();
    let output = child.wait_with_output().expect("reap Python server");
    let response = response.unwrap_or_else(|error| {
        panic!(
            "{error}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    });
    let body_offset = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|offset| offset + 4)
        .expect("health response header terminator");
    let health: Value =
        json::from_slice(&response[body_offset..]).expect("decode canonical Taira health response");
    let object = health.as_object().expect("health response object");
    assert_eq!(object.len(), 9, "health response must have exact fields");
    assert_eq!(
        object.get("schema_version").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        object.get("service").and_then(Value::as_str),
        Some(TAIRA_INROU_CANARY_SERVICE_NAME_V1)
    );
    assert_eq!(
        object.get("service_version").and_then(Value::as_str),
        Some(service_version.as_str())
    );
    assert_eq!(object.get("runtime").and_then(Value::as_str), Some("Inrou"));
    assert_eq!(object.get("replica_slot").and_then(Value::as_u64), Some(3));
    assert_eq!(
        object.get("identity").and_then(Value::as_str),
        Some("taira_inrou_canary:replica:3")
    );
    assert!(
        object
            .get("app_data_marker_sha256")
            .and_then(Value::as_str)
            .is_some_and(|value| value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
    );
    assert_eq!(object.get("boot_sequence").and_then(Value::as_u64), Some(1));
    assert!(
        object
            .get("guest_boot_id_sha256")
            .and_then(Value::as_str)
            .is_some_and(|value| value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
    );
}
#[test]
fn taira_inrou_canary_version_is_the_exact_immutable_revision_identity() {
    let first = canonical_taira_inrou_bundle_fixture();
    let same = canonical_taira_inrou_bundle_fixture();
    assert_eq!(first.service.service_version, same.service.service_version);
    assert!(is_taira_inrou_canary_service_version(
        &first.service.service_version
    ));

    let mut next = first.clone();
    next.container.bundle_hash = Hash::new(b"next immutable Taira canary artifact");
    refresh_taira_container_reference(&mut next);
    install_taira_inrou_canary_service_version(&mut next)
        .expect("refresh immutable rollout revision identity");
    assert_ne!(first.service.service_version, next.service.service_version);
    validate_taira_inrou_canary_bundle(&next)
        .expect("a distinct artifact derives a distinct valid revision");

    let mut forged = first;
    forged.service.service_version = "1.0.0".to_owned();
    assert_taira_canary_validation_error(&forged, "artifact-derived revision");
}
#[test]
fn app_infra_mutation_preflight_binds_exact_authoritative_state() {
    let absent = norito::json!({"apps": []});
    assert_eq!(
        derive_app_infra_mutation_precondition(
            &absent,
            "portal_app",
            "2.0.0",
            MutationMode::Deploy,
            "app test",
        )
        .expect("new app must bind absence"),
        SoraAppInfraMutationPreconditionV1::AppAbsent
    );

    let manifest_hash = Hash::new(b"current app manifest");
    let present = norito::json!({
        "apps": [{
            "app_name": "portal_app",
            "current_app_version": "1.0.0",
            "current_manifest_hash": (manifest_hash),
            "revision_count": 3
        }]
    });
    assert_eq!(
        derive_app_infra_mutation_precondition(
            &present,
            "portal_app",
            "2.0.0",
            MutationMode::Upgrade,
            "app test",
        )
        .expect("upgrade must bind the exact app topology"),
        SoraAppInfraMutationPreconditionV1::ExactCurrentRevision(
            SoraAppInfraExactCurrentRevisionPreconditionV1 {
                app_version: "1.0.0".to_owned(),
                manifest_hash,
                revision_count: 3,
            }
        )
    );
    for invalid_hash in [
        Value::from(manifest_hash.to_string()),
        Value::from("hash:invalid"),
        Value::Null,
    ] {
        let mut invalid = present.clone();
        *invalid
            .pointer_mut("/apps/0/current_manifest_hash")
            .expect("app hash") = invalid_hash;
        let error = derive_app_infra_mutation_precondition(
            &invalid,
            "portal_app",
            "2.0.0",
            MutationMode::Upgrade,
            "app test",
        )
        .expect_err("only canonical Norito JSON hashes may bind app upgrades");
        assert!(
            error.to_string().contains("invalid manifest hash"),
            "{error:#}"
        );
    }
}
#[test]
fn service_mutation_preflight_binds_state_and_rejects_identity_drift() {
    let bundle = canonical_taira_inrou_bundle_fixture();
    let service_name = bundle.service.service_name.to_string();
    let candidate_version = bundle.service.service_version.clone();
    let route = bundle
        .service
        .route
        .as_ref()
        .expect("canonical Taira route");
    let current_service_manifest_hash = Hash::new(b"current service manifest");
    let current_container_manifest_hash = Hash::new(b"current container manifest");
    let status = norito::json!({
        "control_plane": {
            "services": [{
                "service_name": (service_name.clone()),
                "current_version": "artifact-1111111111111111111111111111111111111111111111111111111111111111",
                "config_generation": 3,
                "secret_generation": 5,
                "active_rollout": null,
                "latest_revision": {
                    "service_version": "artifact-1111111111111111111111111111111111111111111111111111111111111111",
                    "service_manifest_hash": (current_service_manifest_hash),
                    "container_manifest_hash": (current_container_manifest_hash),
                    "execution_plane": {"execution_plane": "HttpService"},
                    "runtime": {"runtime": "Inrou"},
                    "route_host": (route.host.clone()),
                    "route_path_prefix": (route.path_prefix.clone()),
                    "route_service_port": (route.service_port.get()),
                    "route_visibility": "Public",
                    "route_tls_mode": "Required",
                    "process_generation": 7
                }
            }]
        }
    });
    assert_eq!(
        derive_service_mutation_precondition(
            &status,
            &service_name,
            &candidate_version,
            MutationMode::Upgrade,
            "service test",
        )
        .expect("upgrade must bind the exact service revision"),
        SoraServiceMutationPreconditionV1::ExactCurrentRevision(
            SoraServiceExactCurrentRevisionPreconditionV1 {
                service_version:
                    "artifact-1111111111111111111111111111111111111111111111111111111111111111"
                        .to_owned(),
                service_manifest_hash: current_service_manifest_hash,
                container_manifest_hash: current_container_manifest_hash,
                process_generation: 7,
                config_generation: 3,
                secret_generation: 5,
            }
        )
    );
    for (field, hash) in [
        ("service_manifest_hash", current_service_manifest_hash),
        ("container_manifest_hash", current_container_manifest_hash),
    ] {
        for invalid_hash in [
            Value::from(hash.to_string()),
            Value::from("hash:invalid"),
            Value::Null,
        ] {
            let mut invalid = status.clone();
            *invalid
                .pointer_mut(&format!(
                    "/control_plane/services/0/latest_revision/{field}"
                ))
                .expect("service revision hash") = invalid_hash;
            let error = derive_service_mutation_precondition(
                &invalid,
                &service_name,
                &candidate_version,
                MutationMode::Upgrade,
                "service test",
            )
            .expect_err("only canonical Norito JSON hashes may bind service upgrades");
            assert!(error.to_string().contains("invalid"), "{error:#}");
        }
    }
    preflight_service_upgrade_identity(
        &status,
        &bundle.service,
        bundle.container.runtime,
        MutationMode::Upgrade,
        "service test",
    )
    .expect("unchanged service identity must pass");

    let mut drifted = bundle.service;
    drifted.route.as_mut().expect("canonical Taira route").host =
        "replacement.taira.sora.org".to_owned();
    assert!(
        preflight_service_upgrade_identity(
            &status,
            &drifted,
            bundle.container.runtime,
            MutationMode::Upgrade,
            "service test",
        )
        .expect_err("route drift must fail before publication")
        .to_string()
        .contains("cannot change route identity")
    );
}
#[test]
fn taira_inrou_canary_validator_rejects_non_atomic_rollout() {
    for canary_percent in [0, 1, 99] {
        let mut bundle = canonical_taira_inrou_bundle_fixture();
        bundle.service.rollout.canary_percent = canary_percent;
        // Generic Inrou admission rejects partial revisions; Taira additionally
        // requires 100. Assert the policy field, not which validator runs first.
        assert_taira_canary_validation_error(&bundle, "canary_percent");
    }
}
#[test]
fn taira_inrou_canary_validator_accepts_published_v1_bundle() {
    let mut bundle = canonical_taira_inrou_bundle_fixture();
    bundle
        .container
        .inrou
        .as_mut()
        .expect("Inrou manifest")
        .guest_images
        .get_mut(&SoraInrouGuestIsaV1::Aarch64)
        .expect("AArch64 image")
        .published_artifact = SoraPublishedInrouGuestImageArtifactV1 {
        manifest_digest_hex: "ab".repeat(32),
        content_cid: encode_content_cid(&sorafs_manifest::canonical_manifest_root_cid([0xAB; 32])),
    };
    refresh_taira_container_reference(&mut bundle);
    install_taira_inrou_canary_service_version(&mut bundle)
        .expect("refresh published Taira revision identity");
    validate_taira_inrou_canary_bundle(&bundle).expect("canonical published Taira Inrou V1 bundle");
}
#[test]
fn taira_inrou_canary_validator_rejects_identity_and_route_drift() {
    let mut identity = canonical_taira_inrou_bundle_fixture();
    identity.service.service_name = "another_canary".parse().expect("alternate service name");
    assert_taira_canary_validation_error(&identity, "canonical service identity");

    let mut route_bundle = canonical_taira_inrou_bundle_fixture();
    let route = route_bundle
        .service
        .route
        .as_mut()
        .expect("canonical route");
    route.host = "other.sora".to_owned();
    route.tls_mode = SoraTlsModeV1::Optional;
    assert_taira_canary_validation_error(&route_bundle, "canonical public TLS route");

    let mut env_bundle = canonical_taira_inrou_bundle_fixture();
    env_bundle
        .container
        .env
        .insert("PORT".to_owned(), "8787".to_owned());
    refresh_taira_container_reference(&mut env_bundle);
    assert_taira_canary_validation_error(&env_bundle, "requires exactly");
}
#[test]
fn taira_inrou_canary_rejects_retired_ssh_field_and_path_drift() {
    let mut retired_ssh = json::to_value(&canonical_taira_inrou_bundle_fixture().container)
        .expect("encode canonical canary container");
    retired_ssh
        .get_mut("inrou")
        .and_then(json::Value::as_object_mut)
        .expect("encoded Inrou manifest")
        .insert(
            "ssh_authorized_keys".to_owned(),
            norito::json!(["ssh-ed25519 AAAATEST taira-canary"]),
        );
    json::from_value::<SoraContainerManifestV1>(retired_ssh)
        .expect_err("the retired first-release SSH field must be rejected");

    let mut guest_path = canonical_taira_inrou_bundle_fixture();
    guest_path
        .container
        .inrou
        .as_mut()
        .expect("Inrou manifest")
        .guest_images
        .get_mut(&SoraInrouGuestIsaV1::Aarch64)
        .expect("AArch64 image")
        .rootfs_image_path = "/inrou/aarch64/other.ext4".to_owned();
    refresh_taira_container_reference(&mut guest_path);
    assert_taira_canary_validation_error(
        &guest_path,
        "each ISA must use its exact /inrou/<isa>/vmlinux",
    );
}
#[test]
fn taira_inrou_canary_validator_rejects_storage_geometry_drift() {
    let mut bundle = canonical_taira_inrou_bundle_fixture();
    bundle.service.lease_volumes[0].max_total_bytes =
        NonZeroU64::new(TAIRA_INROU_CANARY_ROOT_VOLUME_BYTES_V1 - 1).expect("smaller root volume");
    assert_taira_canary_validation_error(&bundle, "canonical root");

    validate_taira_inrou_rootfs_source_bytes(TAIRA_INROU_CANARY_ROOT_VOLUME_BYTES_V1)
        .expect("rootfs at the root-volume boundary");
    assert!(
        validate_taira_inrou_rootfs_source_bytes(TAIRA_INROU_CANARY_ROOT_VOLUME_BYTES_V1 + 1)
            .is_err()
    );
}
#[test]
fn taira_inrou_canary_storage_accounts_for_each_writable_volume() {
    let bundle = canonical_taira_inrou_bundle_fixture();
    let ephemeral = bundle.container.resources.ephemeral_storage_bytes.get();
    let root = bundle.service.lease_volumes[0].max_total_bytes.get();
    let shared = bundle.service.lease_volumes[1].max_total_bytes.get();
    assert_ne!(ephemeral, shared);
    assert_eq!(root + ephemeral, TAIRA_INROU_CANARY_HOST_STORAGE_BYTES_V1);
    assert_eq!(
        root + ephemeral + shared,
        defaults::taira::INROU_MAX_STORAGE_BYTES
    );
    validate_taira_inrou_canary_storage(&bundle.container.resources, &bundle.service)
        .expect("canonical host and shared writable budgets are distinct");

    let mut resources = bundle.container.resources;
    resources.ephemeral_storage_bytes =
        NonZeroU64::new(ephemeral + 1).expect("positive altered temporary budget");
    let error = validate_taira_inrou_canary_storage(&resources, &bundle.service)
        .expect_err("additional temporary storage exceeds the exact host budget");
    assert!(error.to_string().contains("host-local bytes"), "{error}");
}
fn canonical_taira_stage_receipt_fixture() -> TairaInrouStageReceiptV1 {
    TairaInrouStageReceiptV1 {
        schema_version: TAIRA_INROU_STAGE_SCHEMA_VERSION_V1,
        sorafs_retention_epoch: test_sorafs_retention_epoch().get(),
        placement_targets: test_inrou_placement_targets(4),
        mutation_mode: "deploy".to_owned(),
        service_name: "taira_inrou_canary".to_owned(),
        service_version: format!(
            "{TAIRA_INROU_CANARY_SERVICE_VERSION_PREFIX_V1}{}",
            "11".repeat(32)
        ),
        container_file: TAIRA_INROU_STAGE_CONTAINER_FILE_V1.to_owned(),
        service_file: TAIRA_INROU_STAGE_SERVICE_FILE_V1.to_owned(),
        bundle_payload_file: TAIRA_INROU_STAGE_BUNDLE_PAYLOAD_FILE_V1.to_owned(),
        bundle_manifest_file: TAIRA_INROU_STAGE_BUNDLE_MANIFEST_FILE_V1.to_owned(),
        bundle_hash: "hash".to_owned(),
        bundle_content_cid: "cid".to_owned(),
        bundle_manifest_digest_hex: "11".repeat(32),
        guest_isa: SoraInrouGuestIsaV1::Aarch64.as_str().to_owned(),
        guest_payload_dir: TAIRA_INROU_STAGE_GUEST_PAYLOAD_DIR_V1.to_owned(),
        guest_manifest_file: TAIRA_INROU_STAGE_GUEST_MANIFEST_FILE_V1.to_owned(),
        guest_content_cid: "guest-cid".to_owned(),
        guest_manifest_digest_hex: "22".repeat(32),
        discovery_payload_dir: TAIRA_INROU_STAGE_DISCOVERY_PAYLOAD_DIR_V1.to_owned(),
        discovery_manifest_file: TAIRA_INROU_STAGE_DISCOVERY_MANIFEST_FILE_V1.to_owned(),
        discovery_document_hash: "33".repeat(32),
        discovery_content_cid: "discovery-cid".to_owned(),
        discovery_manifest_digest_hex: "44".repeat(32),
        public_discovery_url: "https://taira.sora.org/sorafs/cid/discovery-cid/index.json"
            .to_owned(),
        public_discovery_cid_host_url: "https://discovery-cid.sorafs.taira.sora.org/index.json"
            .to_owned(),
        container_manifest_hash: "container-hash".to_owned(),
        service_manifest_hash: "service-hash".to_owned(),
    }
}
#[cfg(unix)]
const TAIRA_VALIDATOR_CONFIG_FIXTURES: [&str; TAIRA_INROU_VALIDATOR_CONFIG_COUNT_V1] = [
    include_str!("../../../../../defaults/kagami/iroha3-dev/peer0.toml"),
    include_str!("../../../../../defaults/kagami/iroha3-dev/peer1.toml"),
    include_str!("../../../../../defaults/kagami/iroha3-dev/peer2.toml"),
    include_str!("../../../../../defaults/kagami/iroha3-dev/peer3.toml"),
];
#[cfg(unix)]
fn taira_validator_fixture_signer(seed: u8) -> toml::Table {
    let key_pair = soracloud_fixture_key_pair(seed);
    let (algorithm, public_key_bytes) = key_pair
        .public_key()
        .try_to_bytes()
        .expect("fixture signer public key must encode");
    assert_eq!(algorithm, Algorithm::Ed25519);
    let public_key_hex = hex::encode(public_key_bytes);
    let authority = AccountId::new(key_pair.public_key().clone()).to_string();
    let mut signer = toml::Table::new();
    signer.insert(
        "handle".to_owned(),
        toml::Value::String(format!("software://taira/inrou/{public_key_hex}")),
    );
    signer.insert("authority".to_owned(), toml::Value::String(authority));
    signer.insert(
        "algorithm".to_owned(),
        toml::Value::String("ed25519".to_owned()),
    );
    signer.insert(
        "public_key_hex".to_owned(),
        toml::Value::String(public_key_hex),
    );
    signer.insert("revision".to_owned(), toml::Value::Integer(1));
    signer.insert(
        "policy_digest_hex".to_owned(),
        toml::Value::String("a7".repeat(32)),
    );
    signer
}
#[cfg(unix)]
fn install_taira_validator_fixture_runtime(root: &mut toml::Table, signer_seed: u8) {
    let mut submission = toml::Table::new();
    submission.insert(
        "fee_payer".to_owned(),
        toml::Value::String("authority".to_owned()),
    );
    submission.insert(
        "signer".to_owned(),
        toml::Value::Table(taira_validator_fixture_signer(signer_seed)),
    );
    let mut egress = toml::Table::new();
    egress.insert("default_allow".to_owned(), toml::Value::Boolean(false));
    egress.insert("allowed_hosts".to_owned(), toml::Value::Array(Vec::new()));
    egress.insert(
        "rate_per_minute".to_owned(),
        toml::Value::Integer(i64::from(defaults::taira::INROU_EGRESS_RATE_PER_MINUTE)),
    );
    egress.insert(
        "max_bytes_per_minute".to_owned(),
        toml::Value::Integer(
            i64::try_from(defaults::taira::INROU_EGRESS_MAX_BYTES_PER_MINUTE)
                .expect("Taira egress byte budget fits TOML integer"),
        ),
    );
    let mut runtime = toml::Table::new();
    runtime.insert("production_mode".to_owned(), toml::Value::Boolean(true));
    runtime.insert("submission".to_owned(), toml::Value::Table(submission));
    runtime.insert("egress".to_owned(), toml::Value::Table(egress));
    root.insert("soracloud_runtime".to_owned(), toml::Value::Table(runtime));
}
#[cfg(unix)]
fn replace_taira_validator_fixture_signer(root: &mut toml::Table, signer_seed: u8) {
    taira_toml_table_at(
        root,
        &["soracloud_runtime", "submission"],
        "fixture submission",
    )
    .expect("fixture submission table");
    root.get_mut("soracloud_runtime")
        .and_then(toml::Value::as_table_mut)
        .and_then(|runtime| runtime.get_mut("submission"))
        .and_then(toml::Value::as_table_mut)
        .expect("fixture submission table")
        .insert(
            "signer".to_owned(),
            toml::Value::Table(taira_validator_fixture_signer(signer_seed)),
        );
}
#[cfg(unix)]
fn parse_complete_taira_validator_fixture(path: &Path, source: &str) -> actual::Root {
    let table = toml::from_str(source).expect("complete generated-like validator TOML");
    actual::Root::from_toml_source(TomlSource::new_sensitive(
        path.to_path_buf(),
        table,
        zeroize_taira_toml_table,
    ))
    .expect("complete generated-like validator config must parse")
}
#[cfg(unix)]
fn write_complete_taira_validator_fixtures(config_dir: &Path) -> TairaInrouStageReceiptV1 {
    use std::os::unix::fs::PermissionsExt as _;

    fs::create_dir(config_dir).expect("create validator fixture directory");
    fs::set_permissions(config_dir, fs::Permissions::from_mode(0o700))
        .expect("make validator fixture directory owner-private");
    // The copied native profile selects this sibling file. Materialize its
    // public checked identity before parsing any of the four fixture configs.
    let network_id = iroha::data_model::NetworkId::from_genesis_hash(
        iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(
            b"Taira validator binding fixture genesis identity",
        )),
    );
    let expected_hash_path = config_dir.join("genesis.expected_hash");
    fs::write(&expected_hash_path, format!("{network_id}\n"))
        .expect("write public fixture genesis identity");
    fs::set_permissions(&expected_hash_path, fs::Permissions::from_mode(0o600))
        .expect("make fixture genesis identity owner-private");
    let mut placements = BTreeSet::new();
    for (peer_index, source) in TAIRA_VALIDATOR_CONFIG_FIXTURES.iter().enumerate() {
        let mut table = toml::from_str(source).expect("parse generated validator fixture");
        install_taira_validator_fixture_runtime(
            &mut table,
            0xA0_u8
                .checked_add(u8::try_from(peer_index).expect("fixture index fits u8"))
                .expect("fixture signer seed"),
        );
        placements.insert(
            taira_validator_placement_from_table(&table, peer_index)
                .expect("fixture validator placement"),
        );
        let rendered = toml::to_string_pretty(&table)
            .expect("render complete generated-like validator fixture");
        let path = config_dir.join(format!("peer{peer_index}.toml"));
        let parsed = parse_complete_taira_validator_fixture(&path, &rendered);
        assert!(parsed.soracloud_runtime.production_mode);
        assert!(!parsed.soracloud_runtime.inrou.enabled);
        fs::write(&path, rendered.as_bytes()).expect("write validator fixture");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .expect("make validator fixture owner-private");
    }
    let trusted_guest = sample_published_inrou_artifact(0x31);
    let mut receipt = canonical_taira_stage_receipt_fixture();
    receipt.guest_manifest_digest_hex = trusted_guest.manifest_digest_hex;
    receipt.guest_content_cid = trusted_guest.content_cid;
    receipt.placement_targets = placements;
    validate_taira_inrou_config_receipt(&receipt).expect("valid binder fixture receipt");
    receipt
}
#[cfg(unix)]
fn taira_validator_config_snapshots(
    config_dir: &Path,
) -> [(Vec<u8>, TairaFileIdentity); TAIRA_INROU_VALIDATOR_CONFIG_COUNT_V1] {
    (0..TAIRA_INROU_VALIDATOR_CONFIG_COUNT_V1)
        .map(|peer_index| {
            let path = config_dir.join(format!("peer{peer_index}.toml"));
            let bytes = fs::read(&path).expect("snapshot validator config bytes");
            let identity = taira_metadata_identity(
                &fs::symlink_metadata(path).expect("snapshot validator config identity"),
            );
            (bytes, identity)
        })
        .collect::<Vec<_>>()
        .try_into()
        .expect("four validator config snapshots")
}
#[cfg(unix)]
fn assert_taira_validator_configs_unchanged(
    config_dir: &Path,
    expected: &[(Vec<u8>, TairaFileIdentity); TAIRA_INROU_VALIDATOR_CONFIG_COUNT_V1],
) {
    assert_eq!(&taira_validator_config_snapshots(config_dir), expected);
}
#[cfg(unix)]
#[test]
fn taira_validator_config_binding_installs_exact_typed_profiles_atomically() {
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

    let temp = taira_test_tempdir("taira-inrou-validator-config-bind-");
    let config_dir = temp.path().join("configs");
    let receipt = write_complete_taira_validator_fixtures(&config_dir);
    let before = taira_validator_config_snapshots(&config_dir);

    bind_taira_inrou_validator_configs(&config_dir, &receipt)
        .expect("bind all four exact Taira Inrou validator configs");

    for peer_index in 0..TAIRA_INROU_VALIDATOR_CONFIG_COUNT_V1 {
        let path = config_dir.join(format!("peer{peer_index}.toml"));
        let metadata = fs::symlink_metadata(&path).expect("inspect bound validator config");
        assert!(metadata.is_file());
        assert_eq!(metadata.permissions().mode() & 0o7777, 0o600);
        assert_ne!(
            (metadata.dev(), metadata.ino()),
            (before[peer_index].1.dev, before[peer_index].1.ino)
        );
        let source = Zeroizing::new(
            fs::read_to_string(&path).expect("read bound validator config for typed assertion"),
        );
        let parsed = parse_complete_taira_validator_fixture(&path, source.as_str());
        assert!(parsed.soracloud_runtime.production_mode);
        assert_eq!(
            parsed.soracloud_runtime.inrou,
            expected_taira_inrou_validator_config(peer_index, &receipt)
                .expect("exact expected typed Inrou profile")
        );
    }
}
#[cfg(unix)]
#[test]
fn taira_validator_config_binding_mismatch_leaves_all_originals_unchanged() {
    let temp = taira_test_tempdir("taira-inrou-validator-config-mismatch-");
    let config_dir = temp.path().join("configs");
    let receipt = write_complete_taira_validator_fixtures(&config_dir);
    let fourth = config_dir.join("peer3.toml");
    let mut table: toml::Table =
        toml::from_str(&fs::read_to_string(&fourth).expect("read fourth validator fixture"))
            .expect("parse fourth validator fixture");
    replace_taira_validator_fixture_signer(&mut table, 0xD3);
    let rendered = toml::to_string_pretty(&table).expect("render mismatched fourth config");
    parse_complete_taira_validator_fixture(&fourth, &rendered);
    fs::write(&fourth, rendered).expect("write mismatched fourth config");
    let before = taira_validator_config_snapshots(&config_dir);

    let error = bind_taira_inrou_validator_configs(&config_dir, &receipt)
        .expect_err("mismatched fourth placement must fail before staging");
    assert!(
        error.to_string().contains("placements do not match"),
        "{error}"
    );
    assert_taira_validator_configs_unchanged(&config_dir, &before);
}
#[cfg(unix)]
#[test]
fn taira_validator_config_binding_render_limit_leaves_all_originals_unchanged() {
    let temp = taira_test_tempdir("taira-inrou-validator-config-limit-");
    let config_dir = temp.path().join("configs");
    let receipt = write_complete_taira_validator_fixtures(&config_dir);
    let fourth = config_dir.join("peer3.toml");
    let mut table: toml::Table =
        toml::from_str(&fs::read_to_string(&fourth).expect("read fourth validator fixture"))
            .expect("parse fourth validator fixture");
    table
        .get_mut("torii")
        .and_then(toml::Value::as_table_mut)
        .expect("generated fixture Torii table")
        .insert("data_dir".to_owned(), toml::Value::String(String::new()));
    let empty = toml::to_string_pretty(&table).expect("render unpadded fourth config");
    let limit = usize::try_from(MAX_TOML_SOURCE_BYTES).expect("TOML limit fits usize");
    let padding = limit
        .checked_sub(empty.len().saturating_add(1))
        .expect("fixture leaves room for near-limit path");
    table
        .get_mut("torii")
        .and_then(toml::Value::as_table_mut)
        .expect("generated fixture Torii table")
        .insert(
            "data_dir".to_owned(),
            toml::Value::String("x".repeat(padding)),
        );
    let rendered = toml::to_string_pretty(&table).expect("render near-limit fourth config");
    assert_eq!(rendered.len(), limit - 1);
    parse_complete_taira_validator_fixture(&fourth, &rendered);
    let mut projected = table.clone();
    insert_taira_inrou_validator_table(&mut projected, 3, &receipt)
        .expect("project fourth Inrou table for boundary assertion");
    assert!(
        toml::to_string_pretty(&projected)
            .expect("render oversized projected config")
            .len()
            > limit
    );
    fs::write(&fourth, rendered).expect("write near-limit fourth config");
    let before = taira_validator_config_snapshots(&config_dir);

    let error = bind_taira_inrou_validator_configs(&config_dir, &receipt)
        .expect_err("oversized rendered config must fail before staging");
    assert!(
        error.to_string().contains("configuration-source limit"),
        "{error}"
    );
    assert_taira_validator_configs_unchanged(&config_dir, &before);
}
#[cfg(unix)]
#[test]
fn taira_validator_inrou_table_uses_the_single_bounded_taira_profile() {
    let receipt = canonical_taira_stage_receipt_fixture();
    let table =
        taira_inrou_validator_table(2, &receipt).expect("build exact Taira validator Inrou table");
    assert_eq!(
        table.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "enabled",
            "guest_image_max_bytes",
            "max_cpu_millis",
            "max_memory_bytes",
            "max_storage_bytes",
            "portable_vm_gid",
            "portable_vm_uid",
            "start_grace_ms",
            "stop_grace_ms",
            "trusted_guest_content_cid",
            "trusted_guest_manifest_digest_hex",
        ])
    );
    assert_eq!(
        table.get("enabled").and_then(toml::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        table
            .get("portable_vm_uid")
            .and_then(toml::Value::as_integer),
        Some(i64::from(
            defaults::soracloud_runtime::INROU_PORTABLE_VM_ID_BASE + 2
        ))
    );
    assert_eq!(
        table
            .get("guest_image_max_bytes")
            .and_then(toml::Value::as_integer),
        i64::try_from(defaults::taira::INROU_GUEST_IMAGE_MAX_BYTES).ok()
    );
    assert_eq!(
        table
            .get("max_cpu_millis")
            .and_then(toml::Value::as_integer),
        Some(i64::from(defaults::taira::INROU_MAX_CPU_MILLIS))
    );
    assert_eq!(
        table
            .get("max_memory_bytes")
            .and_then(toml::Value::as_integer),
        i64::try_from(defaults::taira::INROU_MAX_MEMORY_BYTES).ok()
    );
    assert_eq!(
        table
            .get("max_storage_bytes")
            .and_then(toml::Value::as_integer),
        i64::try_from(defaults::taira::INROU_MAX_STORAGE_BYTES).ok()
    );
    assert_eq!(
        table
            .get("start_grace_ms")
            .and_then(toml::Value::as_integer),
        i64::try_from(defaults::soracloud_runtime::INROU_START_GRACE_MS).ok()
    );
    assert_eq!(
        table.get("stop_grace_ms").and_then(toml::Value::as_integer),
        i64::try_from(defaults::soracloud_runtime::INROU_STOP_GRACE_MS).ok()
    );
}
#[cfg(unix)]
#[test]
fn taira_validator_config_binding_rejects_an_existing_inrou_table() {
    let receipt = canonical_taira_stage_receipt_fixture();
    let mut root = toml::Table::new();
    let mut runtime = toml::Table::new();
    runtime.insert("inrou".to_owned(), toml::Value::Table(toml::Table::new()));
    root.insert("soracloud_runtime".to_owned(), toml::Value::Table(runtime));
    let error = insert_taira_inrou_validator_table(&mut root, 0, &receipt)
        .expect_err("an existing first-release Inrou table must never be reused");
    assert!(
        error.to_string().contains("never reuses or upgrades"),
        "unexpected existing-table error: {error}"
    );
}
#[cfg(unix)]
#[test]
fn taira_validator_config_placement_comes_from_exact_typed_fields() {
    let expected = test_inrou_placement_targets(1)
        .into_iter()
        .next()
        .expect("one placement target");
    let mut signer = toml::Table::new();
    signer.insert(
        "authority".to_owned(),
        toml::Value::String(expected.validator_account_id.to_string()),
    );
    let mut submission = toml::Table::new();
    submission.insert("signer".to_owned(), toml::Value::Table(signer));
    let mut runtime = toml::Table::new();
    runtime.insert("submission".to_owned(), toml::Value::Table(submission));
    let mut root = toml::Table::new();
    root.insert(
        "public_key".to_owned(),
        toml::Value::String(expected.peer_id.clone()),
    );
    root.insert("soracloud_runtime".to_owned(), toml::Value::Table(runtime));
    assert_eq!(
        taira_validator_placement_from_table(&root, 0)
            .expect("read exact validator placement fields"),
        expected
    );
}
#[cfg(unix)]
#[test]
fn taira_validator_config_paths_require_exact_private_custody() {
    use std::os::unix::fs::PermissionsExt as _;

    let temp = taira_test_tempdir("taira-inrou-validator-config-custody-");
    let config_dir = temp.path().join("configs");
    fs::create_dir(&config_dir).expect("create validator config directory");
    fs::set_permissions(&config_dir, fs::Permissions::from_mode(0o700))
        .expect("make validator config directory owner-private");
    let paths = (0..TAIRA_INROU_VALIDATOR_CONFIG_COUNT_V1)
        .map(|index| {
            let path = config_dir.join(format!("peer{index}.toml"));
            fs::write(&path, b"private_key = \"test\"\n").expect("write validator config fixture");
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
                .expect("make validator config fixture owner-private");
            path
        })
        .collect::<Vec<_>>();
    let directory = open_taira_validator_config_directory(&config_dir)
        .expect("retain private validator config directory");
    assert_eq!(
        exact_taira_validator_config_entries(&directory)
            .expect("accept exact private validator configs")
            .len(),
        TAIRA_INROU_VALIDATOR_CONFIG_COUNT_V1
    );

    let extra = config_dir.join("peer4.toml");
    fs::write(&extra, b"private_key = \"extra\"\n").expect("write unexpected validator config");
    fs::set_permissions(&extra, fs::Permissions::from_mode(0o600))
        .expect("make unexpected validator config owner-private");
    let _error = exact_taira_validator_config_entries(&directory)
        .expect_err("an additional peer TOML must be rejected");
    fs::remove_file(extra).expect("remove unexpected validator config");

    fs::set_permissions(&paths[0], fs::Permissions::from_mode(0o640))
        .expect("weaken validator config custody");
    let _error = exact_taira_validator_config_entries(&directory)
        .expect_err("a group-readable validator config must be rejected");
    fs::set_permissions(&paths[0], fs::Permissions::from_mode(0o600))
        .expect("restore validator config custody");

    fs::remove_file(&paths[1]).expect("remove second validator config fixture");
    fs::hard_link(&paths[0], &paths[1]).expect("alias validator config inode");
    let error = exact_taira_validator_config_entries(&directory)
        .expect_err("hard-linked validator configs must be rejected");
    assert!(
        error.to_string().contains("singly-linked"),
        "unexpected linked-config error: {error}"
    );
}
#[cfg(unix)]
#[test]
fn taira_stage_reads_require_private_custody_for_prepared_and_frozen_files() {
    use std::os::unix::fs::{PermissionsExt as _, symlink};

    let temp = taira_test_tempdir("taira-stage-private-read-");
    let path = temp.path().join("receipt.json");
    let contents = b"public stage fixture";
    fs::write(&path, contents).expect("write public stage fixture");
    for mode in [0o600, 0o400] {
        fs::set_permissions(&path, fs::Permissions::from_mode(mode))
            .expect("set admitted stage file mode");
        assert_eq!(
            taira_stage_owned_file_bytes(&path, "stage fixture", 128)
                .expect("read owner-private prepared or frozen stage"),
            contents
        );
    }
    for mode in [0o000, 0o200, 0o404, 0o440, 0o500, 0o604, 0o640, 0o700] {
        fs::set_permissions(&path, fs::Permissions::from_mode(mode))
            .expect("set rejected stage file mode");
        let error = taira_stage_owned_file_bytes(&path, "stage fixture", 128)
            .expect_err("unreadable, executable or shared stage files must be rejected");
        assert!(error.to_string().contains("mode 0400 or 0600"), "{error}");
    }
    fs::set_permissions(&path, fs::Permissions::from_mode(0o400))
        .expect("restore frozen stage file mode");
    let linked = temp.path().join("linked.json");
    fs::hard_link(&path, &linked).expect("create hard-linked stage fixture");
    let error = taira_stage_owned_file_bytes(&path, "stage fixture", 128)
        .expect_err("frozen stage files must remain singly linked");
    assert!(
        error.to_string().contains("exactly one hard link"),
        "{error}"
    );
    fs::remove_file(&linked).expect("remove hard-linked fixture");
    symlink(&path, &linked).expect("create stage symlink fixture");
    let error = taira_stage_owned_file_bytes(&linked, "stage fixture", 128)
        .expect_err("stage readers must not follow symlinks");
    assert!(error.to_string().contains("direct regular file"), "{error}");
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o500))
        .expect("set readonly stage directory mode");
    let error = validate_taira_stage_owned_entry(temp.path(), true, "stage fixture")
        .expect_err("stage directories retain their exact owner0700 contract");
    assert!(error.to_string().contains("mode 0700"), "{error}");
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700))
        .expect("restore stage directory mode for cleanup");
}
#[test]
fn taira_stage_receipt_rejects_noncanonical_or_legacy_layouts() {
    let canonical = canonical_taira_stage_receipt_fixture();
    validate_taira_stage_layout(&canonical, MutationMode::Deploy).expect("canonical stage layout");
    assert!(validate_taira_stage_layout(&canonical, MutationMode::Upgrade).is_err());

    for invalid_version in [
        "1.0.0".to_owned(),
        format!(
            "{TAIRA_INROU_CANARY_SERVICE_VERSION_PREFIX_V1}{}",
            "11".repeat(31)
        ),
        format!(
            "{TAIRA_INROU_CANARY_SERVICE_VERSION_PREFIX_V1}{}",
            "AA".repeat(32)
        ),
        format!(
            "{TAIRA_INROU_CANARY_SERVICE_VERSION_PREFIX_V1}{}",
            "10".repeat(32)
        ),
    ] {
        let mut invalid = canonical.clone();
        invalid.service_version = invalid_version;
        assert!(
            validate_taira_stage_layout(&invalid, MutationMode::Deploy).is_err(),
            "only artifact- followed by one exact canonical Iroha hash is accepted"
        );
    }

    let mut traversal = canonical.clone();
    traversal.guest_payload_dir = "../guest".to_owned();
    assert!(validate_taira_stage_layout(&traversal, MutationMode::Deploy).is_err());
    let mut zero_retention = canonical.clone();
    zero_retention.sorafs_retention_epoch = 0;
    assert!(validate_taira_stage_layout(&zero_retention, MutationMode::Deploy).is_err());
    let mut legacy_version = canonical;
    legacy_version.schema_version = 0;
    assert!(validate_taira_stage_layout(&legacy_version, MutationMode::Deploy).is_err());

    let Value::Object(mut missing_mode) = json::to_value(&canonical_taira_stage_receipt_fixture())
        .expect("encode stage receipt fixture")
    else {
        panic!("stage receipt must encode as an object");
    };
    missing_mode.remove("mutation_mode");
    let bytes = json::to_vec(&Value::Object(missing_mode)).expect("encode legacy receipt");
    assert!(
        decode_taira_stage_json::<TairaInrouStageReceiptV1>(
            &bytes,
            Path::new("legacy-receipt.json"),
        )
        .is_err(),
        "first-release stages must reject receipts without an explicit mutation mode"
    );

    let Value::Object(mut missing_retention) =
        json::to_value(&canonical_taira_stage_receipt_fixture())
            .expect("encode stage receipt fixture")
    else {
        panic!("stage receipt must encode as an object");
    };
    missing_retention.remove("sorafs_retention_epoch");
    let bytes = json::to_vec(&Value::Object(missing_retention)).expect("encode stale receipt");
    assert!(
        decode_taira_stage_json::<TairaInrouStageReceiptV1>(
            &bytes,
            Path::new("stale-receipt.json"),
        )
        .is_err(),
        "first-release stages must reject receipts without the exact retention identity"
    );

    let Value::Object(mut retired_selector) =
        json::to_value(&canonical_taira_stage_receipt_fixture())
            .expect("encode stage receipt fixture")
    else {
        panic!("stage receipt must encode as an object");
    };
    retired_selector.insert(
        "selected_backend".to_owned(),
        Value::String("portable_vm".to_owned()),
    );
    let bytes = json::to_vec(&Value::Object(retired_selector)).expect("encode retired selector");
    assert!(
        decode_taira_stage_json::<TairaInrouStageReceiptV1>(
            &bytes,
            Path::new("retired-selector-receipt.json"),
        )
        .is_err(),
        "first-release stages must reject retired or unknown receipt fields"
    );
}
#[test]
fn taira_stage_guest_budget_accepts_normalized_layout_and_rejects_oversized_source() {
    let sizes = [
        27_236_288_u64,
        TAIRA_INROU_CANARY_ROOT_VOLUME_BYTES_V1,
        13_923_072,
    ];
    let total = sizes.into_iter().sum::<u64>();
    assert_eq!(
        taira_stage_guest_total_bytes(sizes).expect("normalized Taira guest layout"),
        total
    );
    assert!(total <= TAIRA_INROU_STAGE_MAX_GUEST_BYTES_V1);
    let error = taira_stage_guest_total_bytes([27_236_288_u64, 3_085_959_168, 13_923_072])
        .expect_err("unnormalized upstream image exceeds the canonical Taira profile");
    assert!(error.to_string().contains("maximum is"), "{error}");
    assert_eq!(
        taira_stage_guest_total_bytes([TAIRA_INROU_STAGE_MAX_GUEST_BYTES_V1])
            .expect("exact guest byte budget"),
        TAIRA_INROU_STAGE_MAX_GUEST_BYTES_V1,
    );
    assert!(taira_stage_guest_total_bytes([TAIRA_INROU_STAGE_MAX_GUEST_BYTES_V1 + 1]).is_err());
    assert!(taira_stage_guest_total_bytes([u64::MAX, 1]).is_err());
}
#[test]
fn taira_stage_regular_file_read_enforces_exact_byte_limit() {
    let temp = taira_test_tempdir("taira-stage-read-limit-");
    let source = temp.path().join("source.bin");
    fs::write(&source, [1_u8, 2, 3, 4]).expect("write boundary source");
    assert_eq!(
        taira_stage_regular_file_bytes(&source, "test source", 4).expect("read boundary source"),
        [1_u8, 2, 3, 4]
    );
    fs::write(&source, [1_u8, 2, 3, 4, 5]).expect("write oversized source");
    let error = taira_stage_regular_file_bytes(&source, "test source", 4)
        .expect_err("one byte over must fail");
    assert!(error.to_string().contains("maximum is 4"), "{error}");
}
#[test]
fn taira_stage_requires_exact_three_member_layout() {
    let canonical = vec![
        "aarch64/vmlinuz".to_owned(),
        "aarch64/rootfs.ext4".to_owned(),
        "aarch64/initrd.img".to_owned(),
    ];
    let logical = taira_stage_logical_member_paths(&canonical).expect("canonical members");
    assert_eq!(logical.len(), 3);
    assert!(taira_stage_logical_member_paths(&canonical[..2]).is_err());
    let traversal = vec![
        "aarch64/vmlinuz".to_owned(),
        "../rootfs.ext4".to_owned(),
        "aarch64/initrd.img".to_owned(),
    ];
    assert!(taira_stage_logical_member_paths(&traversal).is_err());
}
#[cfg(unix)]
#[test]
fn taira_streaming_plan_matches_canonical_eager_directory_plan() {
    let temp = taira_test_tempdir("taira-streaming-plan-");
    let aarch64 = temp.path().join("aarch64");
    fs::create_dir(&aarch64).expect("aarch64 directory");
    fs::write(aarch64.join("initrd.img"), vec![0x11; 333_333]).expect("initrd");
    fs::write(aarch64.join("rootfs.ext4"), vec![0x22; 1_500_321]).expect("rootfs");
    fs::write(aarch64.join("vmlinuz"), vec![0x33; 700_777]).expect("kernel");
    let members = vec![
        "aarch64/vmlinuz".to_owned(),
        "aarch64/rootfs.ext4".to_owned(),
        "aarch64/initrd.img".to_owned(),
    ];
    let profile = sorafs_chunker::ChunkProfile::DEFAULT;
    let (eager, payload) =
        CarBuildPlan::from_directory_with_profile(temp.path(), profile).expect("eager plan");
    let streaming =
        taira_streaming_directory_plan(temp.path(), &members, profile).expect("streaming plan");
    assert_eq!(streaming, eager);
    assert_eq!(streaming.payload_digest, blake3::hash(&payload));
}
#[cfg(unix)]
#[test]
fn taira_stage_creation_rejects_symlinked_intermediate_parent() {
    use std::os::unix::fs::symlink;
    let temp = taira_test_tempdir("taira-stage-parent-");
    let real_parent = temp.path().join("real");
    fs::create_dir(&real_parent).expect("real parent");
    let linked_parent = temp.path().join("linked");
    symlink(&real_parent, &linked_parent).expect("linked parent");
    let error = create_taira_stage_directory(&linked_parent.join("stage"))
        .expect_err("symlinked parent must fail");
    assert!(
        error.to_string().contains("must not be a symbolic link"),
        "{error}"
    );
    assert!(!real_parent.join("stage").exists());
}
const STATIC_ASSETS_V1: [&str; 21] = [
    include_str!("../assets/v1/tests/http_local.sh"),
    include_str!("../assets/v1/tests/exit_130.sh"),
    include_str!("../assets/v1/tests/http_build_sync.sh"),
    include_str!("../assets/v1/tests/http_deploy.sh"),
    include_str!("../assets/v1/tests/http_upgrade.sh"),
    include_str!("../assets/v1/tests/app_local.sh"),
    include_str!("../assets/v1/tests/exit_130.sh"),
    include_str!("../assets/v1/tests/app_build_sync.sh"),
    include_str!("../assets/v1/tests/app_release.sh"),
    include_str!("../assets/v1/tests/split_release_build.sh"),
    include_str!("../assets/v1/tests/inrou_reuse_build.sh"),
    concat!(
        include_str!("../assets/v1/tests/missing_static_site_publish_mode.prefix.json"),
        "            }"
    ),
    concat!(
        include_str!("../assets/v1/tests/auth_success_values.prefix.mjs"),
        "            "
    ),
    include_str!("../assets/v1/tests/auth_file_state.mjs"),
    include_str!("../assets/v1/tests/auth_shared_setup.mjs"),
    include_str!("../assets/v1/tests/auth_shared_body.mjs"),
    include_str!("../assets/v1/tests/auth_session_cookie.mjs"),
    include_str!("../assets/v1/tests/auth_cleanup_locks.mjs"),
    include_str!("../assets/v1/tests/auth_login_failures.mjs"),
    include_str!("../assets/v1/tests/auth_handlers_success.mjs"),
    include_str!("../assets/v1/tests/auth_bad_requests.mjs"),
];
const TEST_HARNESSES_V1: [&str; 25] = [
    include_str!("../assets/v1/tests/pii_startup_import.mjs"),
    include_str!("../assets/v1/tests/pii_auth_core_import.mjs"),
    include_str!("../assets/v1/tests/http_service_smoke.mjs"),
    include_str!("../assets/v1/tests/single_api_dev_smoke.mjs"),
    include_str!("../assets/v1/tests/split_app_live_smoke.mjs"),
    include_str!("../assets/v1/tests/split_app_vault_dev_smoke.mjs"),
    include_str!("../assets/v1/tests/webapp_strict_key_fail.mjs"),
    include_str!("../assets/v1/tests/webapp_invalid_mode_fail.mjs"),
    include_str!("../assets/v1/tests/webapp_external_state_required_fail.mjs"),
    include_str!("../assets/v1/tests/webapp_external_state_default_required_fail.mjs"),
    include_str!("../assets/v1/tests/webapp_external_state_production_disable_fail.mjs"),
    include_str!("../assets/v1/tests/webapp_invalid_external_adapter_shape_fail.mjs"),
    include_str!("../assets/v1/tests/webapp_external_adapter_smoke.mjs"),
    include_str!("../assets/v1/tests/pii_external_state_required_fail.mjs"),
    include_str!("../assets/v1/tests/pii_external_state_default_required_fail.mjs"),
    include_str!("../assets/v1/tests/pii_external_state_production_disable_fail.mjs"),
    include_str!("../assets/v1/tests/pii_invalid_external_adapter_shape_fail.mjs"),
    include_str!("../assets/v1/tests/pii_external_adapter_smoke.mjs"),
    include_str!("../assets/v1/tests/webapp_capability_map_required.mjs"),
    include_str!("../assets/v1/tests/webapp_shared_sessions_smoke.mjs"),
    include_str!("../assets/v1/tests/webapp_replay_lock_contention.mjs"),
    include_str!("../assets/v1/tests/pii_replay_lock_contention.mjs"),
    include_str!("../assets/v1/tests/webapp_origin_mismatch.mjs"),
    include_str!("../assets/v1/tests/pii_origin_mismatch.mjs"),
    include_str!("../assets/v1/tests/pii_capability_authorization.mjs"),
];
struct FailingSoracloudSignatureNonceRng;
#[derive(Debug)]
struct FailingSoracloudSignatureNonceRngError;
impl fmt::Display for FailingSoracloudSignatureNonceRngError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("failing Soracloud signature nonce RNG")
    }
}
impl TryRngCore for FailingSoracloudSignatureNonceRng {
    type Error = FailingSoracloudSignatureNonceRngError;
    fn try_next_u32(&mut self) -> std::result::Result<u32, Self::Error> {
        Err(FailingSoracloudSignatureNonceRngError)
    }
    fn try_next_u64(&mut self) -> std::result::Result<u64, Self::Error> {
        Err(FailingSoracloudSignatureNonceRngError)
    }
    fn try_fill_bytes(&mut self, _dst: &mut [u8]) -> std::result::Result<(), Self::Error> {
        Err(FailingSoracloudSignatureNonceRngError)
    }
}
impl TryCryptoRng for FailingSoracloudSignatureNonceRng {}
fn temp_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("iroha_soracloud_cli_{name}_{nanos}"));
    fs::create_dir_all(&path).expect("create temp dir");
    path
}
fn test_inrou_preseed_targets(root: &Path) -> Vec<InrouOperatorPreseedTargetArg> {
    (0..SORACLOUD_ARTIFACT_MIN_REPLICAS_V1)
        .map(|index| {
            let path = root.join(format!("inrou-preseed-{index}"));
            fs::create_dir_all(&path).expect("create offline Inrou preseed store");
            let data_dir = fs::canonicalize(path).expect("canonical offline Inrou preseed store");
            let seed = 0x70_u8
                .checked_add(u8::try_from(index).expect("test target index fits u8"))
                .expect("test target seed");
            let validator_key_pair = soracloud_fixture_key_pair(seed);
            let peer_key_pair = soracloud_fixture_key_pair(seed ^ 0x80);
            InrouOperatorPreseedTargetArg {
                validator_account_id: AccountId::new(validator_key_pair.public_key().clone())
                    .to_string(),
                peer_id: PeerId::from(peer_key_pair.public_key().clone()).to_string(),
                data_dir,
            }
        })
        .collect()
}
fn test_inrou_placement_targets(count: usize) -> BTreeSet<SoraInrouPlacementTargetV1> {
    (0..count)
        .map(|index| {
            let seed = 0x60_u8
                .checked_add(u8::try_from(index).expect("test target index fits u8"))
                .expect("test target seed");
            let validator_key_pair = soracloud_fixture_key_pair(seed);
            let peer_key_pair = soracloud_fixture_key_pair(seed ^ 0x80);
            SoraInrouPlacementTargetV1 {
                validator_account_id: AccountId::new(validator_key_pair.public_key().clone()),
                peer_id: PeerId::from(peer_key_pair.public_key().clone()).to_string(),
            }
        })
        .collect()
}
fn test_inrou_preseed_capacity() -> Option<NonZeroU64> {
    Some(NonZeroU64::new(64 * 1024 * 1024).expect("nonzero test SoraFS capacity"))
}
fn write_test_inrou_preseed_helper(
    root: &Path,
    file_name: &str,
    script: &str,
) -> (PathBuf, String) {
    let helper = root.join(file_name);
    fs::write(&helper, script).expect("write test Inrou preseed helper");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o700))
            .expect("make test Inrou preseed helper executable");
    }
    let helper = fs::canonicalize(helper).expect("canonical test Inrou preseed helper");
    let digest = sha256_file(
        &helper,
        "test Inrou preseed helper",
        INROU_PRESEED_HELPER_MAX_BYTES,
    )
    .expect("hash test Inrou preseed helper");
    (helper, digest)
}
fn test_inrou_preseed_helper(root: &Path) -> (Option<PathBuf>, Option<String>) {
    let (helper, digest) = write_test_inrou_preseed_helper(
        root,
        "test-sorafs-node-preseed-helper.sh",
        concat!(
            "#!/bin/sh\n",
            "printf '%s\\n' \"$IROHA_TEST_INROU_PRESEED_RECEIPT\"\n",
            "while IFS= read -r _line; do :; done\n",
            "printf '%s' \"$IROHA_TEST_INROU_PRESEED_RELEASE_ACK\"\n",
        ),
    );
    (Some(helper), Some(digest))
}
fn test_inrou_preseed_receipt_output(root: &Path, label: &str) -> PathBuf {
    let directory = root.join(format!("{label}-inrou-qualification"));
    fs::create_dir_all(&directory).expect("create test Inrou qualification directory");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
            .expect("make test Inrou qualification directory owner-only");
    }
    fs::canonicalize(directory)
        .expect("canonical test Inrou qualification directory")
        .join("qualification.json")
}
fn qualify_test_inrou_service(
    root: &Path,
    bundle_file: &Path,
    key_pair: &KeyPair,
    label: &str,
) -> PathBuf {
    let receipt_out = test_inrou_preseed_receipt_output(root, label);
    let (inrou_preseed_helper, inrou_preseed_helper_sha256) = test_inrou_preseed_helper(root);
    InrouServicePreseedArgs {
        container: root.join("container_manifest.json"),
        service: root.join("service_manifest.json"),
        bundle_file: bundle_file.to_path_buf(),
        sorafs_retention_epoch: test_sorafs_retention_epoch(),
        inrou_preseed_targets: test_inrou_preseed_targets(root),
        inrou_preseed_max_capacity_bytes: test_inrou_preseed_capacity(),
        inrou_preseed_helper,
        inrou_preseed_helper_sha256,
        receipt_out: receipt_out.clone(),
        timeout_secs: 5,
    }
    .run(key_pair)
    .expect("offline service preseed must produce the exact qualification");
    receipt_out
}
fn sync_test_app_manifests(root: &Path) {
    SyncManifestsArgs {
        app_manifest: Some(root.join("app_manifest.json")),
        container: root.join("container_manifest.json"),
        service: root.join("service_manifest.json"),
        bundle_file: None,
    }
    .run()
    .expect("synchronize fixture manifests with exact app bundle bytes");
}
fn qualify_test_inrou_app(root: &Path, key_pair: &KeyPair, label: &str) -> PathBuf {
    // Fixture scripts emit repeatable bytes without invoking a separately built CLI.
    // Bind those bytes here; preseed's next build must match these exact hashes.
    AppBuildAndSyncArgs {
        manifest: root.join("app_manifest.json"),
        dry_run: false,
    }
    .run()
    .expect("build exact app fixture artifacts");
    sync_test_app_manifests(root);
    let receipt_out = test_inrou_preseed_receipt_output(root, label);
    let (inrou_preseed_helper, inrou_preseed_helper_sha256) = test_inrou_preseed_helper(root);
    InrouAppPreseedArgs {
        manifest: root.join("app_manifest.json"),
        sorafs_retention_epoch: test_sorafs_retention_epoch(),
        inrou_preseed_targets: test_inrou_preseed_targets(root),
        inrou_preseed_max_capacity_bytes: test_inrou_preseed_capacity(),
        inrou_preseed_helper,
        inrou_preseed_helper_sha256,
        receipt_out: receipt_out.clone(),
        timeout_secs: 5,
    }
    .run(key_pair)
    .expect("offline app preseed must produce the exact qualification");
    receipt_out
}
fn test_inrou_preseed_config(
    root: &Path,
    helper: &Path,
    helper_digest: &str,
) -> ValidatedInrouOperatorPreseed {
    validate_inrou_operator_preseed(
        Some(SORACLOUD_ARTIFACT_MIN_REPLICAS_V1),
        &test_inrou_preseed_targets(root),
        test_inrou_preseed_capacity(),
        Some(helper),
        Some(helper_digest),
    )
    .expect("validate exact test preseed helper")
    .expect("test Inrou preseed config")
}
fn test_inrou_preseed_artifact(root: &Path, label: &str) -> PreparedSorafsArtifact {
    let input = root.join(format!("{label}.bin"));
    fs::write(&input, format!("exact {label} preseed artifact bytes"))
        .expect("write test preseed artifact");
    prepare_sorafs_file_artifact(
        &input,
        label,
        &soracloud_fixture_key_pair(0x5C),
        SorafsReleaseIdentityV1::new(test_sorafs_retention_epoch()),
    )
    .expect("prepare test preseed artifact")
    .0
}
fn named_service_fixture(
    temp_name: &str,
    service_name: &str,
    template: InitTemplate,
) -> (PathBuf, InitOutput) {
    let dir = temp_dir(temp_name);
    let expectation = format!("{} init should succeed", template.as_str());
    let output = InitArgs {
        output_dir: dir.clone(),
        service_name: service_name.to_owned(),
        service_version: "1.0.0".to_owned(),
        template,
        overwrite: false,
    }
    .run()
    .expect(&expectation);
    (dir, output)
}
fn service_fixture(temp_name: &str, template: InitTemplate) -> (PathBuf, InitOutput) {
    let service_name = match template {
        InitTemplate::Baseline | InitTemplate::HttpService => "echo_console",
        InitTemplate::Site => "docs_portal",
        InitTemplate::Webapp => "agent_console",
        InitTemplate::PiiApp => "clinic_console",
        InitTemplate::HayahiApp => "hayahi_api",
    };
    named_service_fixture(temp_name, service_name, template)
}
fn app_fixture(temp_name: &str, template: AppInitTemplate) -> (PathBuf, AppInitOutput) {
    let dir = temp_dir(temp_name);
    let expectation = format!("{} init should succeed", template.as_str());
    let output = AppInitArgs {
        output_dir: dir.clone(),
        app_name: "travel_ops".to_owned(),
        app_version: "1.0.0".to_owned(),
        template,
        existing_repo: false,
        public_host: None,
        static_site_dist_dir: None,
        overwrite: false,
    }
    .run()
    .expect(&expectation);
    (dir, output)
}
fn app_scaffold_args(
    output_dir: PathBuf,
    app_name: &str,
    template: AppInitTemplate,
) -> AppInitArgs {
    AppInitArgs {
        output_dir,
        app_name: app_name.to_owned(),
        app_version: "1.0.0".to_owned(),
        template,
        existing_repo: false,
        public_host: None,
        static_site_dist_dir: None,
        overwrite: false,
    }
}
fn single_api_fixture(temp_name: &str) -> (PathBuf, AppInitOutput) {
    app_fixture(temp_name, AppInitTemplate::SingleApi)
}
fn split_app_fixture(temp_name: &str) -> (PathBuf, AppInitOutput) {
    app_fixture(temp_name, AppInitTemplate::SplitApp)
}
fn assert_request_has_no_inline_signing_fields(request: &impl JsonSerialize) {
    let Value::Object(body) = norito::json::to_value(request).expect("serialize Soracloud request")
    else {
        panic!("Soracloud request must serialize as a JSON object");
    };
    for field in ["authority", "private_key"] {
        assert!(
            !body.contains_key(field),
            "Soracloud request serialized retired field `{field}`"
        );
    }
}
fn assert_signed_request_signature(
    request: &impl JsonSerialize,
    provenance: &ManifestProvenance,
    payload: &[u8],
) {
    provenance
        .signature
        .verify(&provenance.signer, payload)
        .expect("signature should verify");
    assert_request_has_no_inline_signing_fields(request);
}
#[test]
fn template_renderer_is_single_pass_for_adversarial_substitutions() {
    let first = "__SORACLOUD_SECOND__\n\"quoted\" {braced}";
    assert_eq!(
        render_template(
            "before __SORACLOUD_FIRST__ | __SORACLOUD_SECOND__ | __SORACLOUD_FIRST__ after",
            &[
                ("__SORACLOUD_FIRST__", first),
                ("__SORACLOUD_SECOND__", "second"),
            ],
        ),
        format!("before {first} | second | {first} after")
    );
    assert_eq!(render_template("already complete", &[]), "already complete");
}

#[test]
fn canonical_template_outputs_match_current_v1_manifest() {
    let outputs = [
        site_package_json("travel-ops"),
        webapp_root_package_json("travel-ops"),
        webapp_frontend_package_json("travel-ops"),
        pii_app_root_package_json("travel-ops"),
        pii_app_frontend_package_json("travel-ops"),
        hayahi_app_root_package_json("travel-ops"),
        site_app_vue("travel_ops"),
        single_api_api_dev_server_mjs("travel_ops"),
        http_service_build_sh("http-service.tgz"),
        http_service_build_and_sync_sh("http-service.tgz"),
        http_service_server_mjs("travel_ops"),
        split_app_live_server_mjs("travel_ops"),
        http_service_readme("travel_ops", "travel-ops"),
        split_app_frontend_package_json("travel-ops"),
        split_app_frontend_app_vue("travel_ops"),
        split_app_vault_dev_server_mjs("travel_ops"),
        split_app_vault_contract_ko("travel_ops"),
        split_app_live_readme("travel_ops"),
        split_app_vault_readme("travel_ops"),
        split_app_readme("travel_ops", "travel-ops"),
        split_app_existing_repo_readme("travel_ops"),
        site_readme("travel_ops", "travel-ops.sora"),
        single_api_api_readme("travel_ops"),
        single_api_app_readme("travel_ops", "travel-ops"),
        webapp_readme("travel_ops"),
        pii_app_readme("travel_ops"),
        hayahi_app_readme("travel_ops"),
    ];
    const OUTPUT_NAMES: &str = "site_package_json webapp_root_package_json webapp_frontend_package_json pii_app_root_package_json pii_app_frontend_package_json hayahi_app_root_package_json site_app_vue single_api_api_dev_server_mjs http_service_build_sh http_service_build_and_sync_sh http_service_server_mjs split_app_live_server_mjs http_service_readme split_app_frontend_package_json split_app_frontend_app_vue split_app_vault_dev_server_mjs split_app_vault_contract_ko split_app_live_readme split_app_vault_readme split_app_readme split_app_existing_repo_readme site_readme single_api_api_readme single_api_app_readme webapp_readme pii_app_readme hayahi_app_readme";
    const EXPECTED: &str = include_str!("../assets/v1/tests/canonical_template_outputs.manifest");
    let expected: Vec<_> = EXPECTED.lines().filter(|line| !line.is_empty()).collect();
    let names: Vec<_> = OUTPUT_NAMES.split_ascii_whitespace().collect();
    assert_eq!([outputs.len(), expected.len(), names.len()], [27; 3]);
    for ((output, expected), output_name) in outputs.into_iter().zip(expected).zip(names) {
        let mut fields = expected.split_ascii_whitespace();
        let name = fields.next().expect("manifest function name");
        let expected_len: usize = fields
            .next()
            .expect("manifest byte length")
            .parse()
            .expect("numeric byte length");
        let expected_digest = fields.next().expect("manifest digest");
        assert_eq!(fields.next(), None, "manifest field count");
        assert_eq!(output_name, name, "manifest function order");
        assert_eq!(output.len(), expected_len, "{name} byte length");
        assert_eq!(
            hex::encode(<sha2::Sha256 as sha2::Digest>::digest(output.as_bytes())),
            expected_digest,
            "{name} digest"
        );
        assert!(output.ends_with('\n'), "{name} final LF");
    }
}

include!("../template_extraction_tests.rs");

#[test]
fn bundle_pack_writes_deterministic_canonical_archive_and_reports_exact_bytes() {
    let dir = temp_dir("bundle_pack_canonical");
    let source = dir.join("server.mjs");
    let output = dir.join("build/service.tgz");
    let source_payload = b"#!/usr/bin/env node\nconsole.log('ready');\n";
    fs::write(&source, source_payload).expect("write bundle source");
    fs::create_dir_all(output.parent().expect("output parent")).expect("create output parent");
    fs::write(&output, b"stale archive").expect("write stale output");
    let first = BundlePackArgs {
        source: source.clone(),
        archive_path: "app/server.mjs".to_owned(),
        output: output.clone(),
        executable: true,
    }
    .run()
    .expect("pack canonical Inrou bundle");
    let expected = write_gzip_ustar(
        Vec::new(),
        &[BundleArchiveFile::new(
            "app/server.mjs",
            0o755,
            source_payload,
        )],
    )
    .expect("encode expected canonical archive");
    let installed = fs::read(&output).expect("read installed bundle");
    assert_eq!(installed, expected);
    assert_eq!(first.source_file, source.to_string_lossy().into_owned());
    assert_eq!(
        first.source_size_bytes,
        u64::try_from(source_payload.len()).expect("source length")
    );
    assert_eq!(first.archive_member_path, "app/server.mjs");
    assert_eq!(first.archive_member_mode, 0o755);
    assert_eq!(first.bundle_file, output.to_string_lossy().into_owned());
    assert_eq!(
        first.bundle_size_bytes,
        u64::try_from(expected.len()).expect("archive length")
    );
    assert_eq!(first.bundle_hash, Hash::new(&expected));
    let second = BundlePackArgs {
        source,
        archive_path: "app/server.mjs".to_owned(),
        output: output.clone(),
        executable: true,
    }
    .run()
    .expect("replace bundle with identical canonical bytes");
    assert_eq!(second.bundle_hash, first.bundle_hash);
    assert_eq!(fs::read(&output).expect("read replaced bundle"), expected);
    assert!(
        fs::read_dir(output.parent().expect("output parent"))
            .expect("list output parent")
            .all(|entry| {
                !entry
                    .expect("directory entry")
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".inrou-bundle-pack-")
            }),
        "successful packing must not leave staging files"
    );
}
#[test]
fn bundle_pack_preserves_existing_output_when_archive_member_is_invalid() {
    let dir = temp_dir("bundle_pack_invalid_member");
    let source = dir.join("server.mjs");
    let output = dir.join("service.tgz");
    fs::write(&source, b"source").expect("write source");
    fs::write(&output, b"previous").expect("write previous output");
    let error = BundlePackArgs {
        source,
        archive_path: "../escape".to_owned(),
        output: output.clone(),
        executable: false,
    }
    .run()
    .expect_err("parent-traversing archive path must fail");
    assert!(format!("{error:?}").contains("failed to encode canonical Inrou bundle member"));
    assert_eq!(
        fs::read(&output).expect("read preserved output"),
        b"previous"
    );
    assert!(
        fs::read_dir(&dir)
            .expect("list test directory")
            .all(|entry| {
                !entry
                    .expect("directory entry")
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".inrou-bundle-pack-")
            }),
        "failed packing must clean only its owned staging file"
    );
}
#[test]
fn bundle_pack_rejects_source_as_output_without_changing_it() {
    let dir = temp_dir("bundle_pack_source_output_alias");
    let source = dir.join("server.mjs");
    fs::write(&source, b"source must survive").expect("write source");
    let error = BundlePackArgs {
        source: source.clone(),
        archive_path: "app/server.mjs".to_owned(),
        output: source.clone(),
        executable: true,
    }
    .run()
    .expect_err("source and output must not be the same path");
    assert!(format!("{error:?}").contains("must not replace its source file"));
    assert_eq!(
        fs::read(&source).expect("read preserved source"),
        b"source must survive"
    );
}
#[test]
fn bundle_pack_non_executable_member_uses_canonical_0644_mode() {
    let dir = temp_dir("bundle_pack_non_executable");
    let source = dir.join("config.txt");
    let output = dir.join("config.tgz");
    fs::write(&source, b"configuration\n").expect("write source");
    let report = BundlePackArgs {
        source,
        archive_path: "app/config.txt".to_owned(),
        output: output.clone(),
        executable: false,
    }
    .run()
    .expect("pack non-executable member");
    let expected = write_gzip_ustar(
        Vec::new(),
        &[BundleArchiveFile::new(
            "app/config.txt",
            0o644,
            b"configuration\n",
        )],
    )
    .expect("encode expected non-executable archive");
    assert_eq!(report.archive_member_mode, 0o644);
    assert_eq!(fs::read(output).expect("read archive"), expected);
}
#[cfg(unix)]
#[test]
fn bundle_pack_rejects_symbolic_and_hard_link_sources() {
    use std::os::unix::fs::symlink;
    let dir = temp_dir("bundle_pack_indirect_source");
    let source = dir.join("server.mjs");
    let symbolic = dir.join("symbolic.mjs");
    let hard = dir.join("hard.mjs");
    fs::write(&source, b"source").expect("write source");
    symlink(&source, &symbolic).expect("create symbolic link");
    let symbolic_error = BundlePackArgs {
        source: symbolic,
        archive_path: "app/server.mjs".to_owned(),
        output: dir.join("symbolic.tgz"),
        executable: true,
    }
    .run()
    .expect_err("symbolic-link source must fail");
    assert!(format!("{symbolic_error:?}").contains("stable single-link identity"));
    fs::hard_link(&source, &hard).expect("create hard link");
    let hard_error = BundlePackArgs {
        source,
        archive_path: "app/server.mjs".to_owned(),
        output: dir.join("hard.tgz"),
        executable: true,
    }
    .run()
    .expect_err("hard-linked source must fail");
    assert!(format!("{hard_error:?}").contains("stable single-link identity"));
}
#[cfg(unix)]
#[test]
fn bundle_pack_replaces_output_symlink_without_touching_its_target() {
    use std::os::unix::fs::symlink;
    let dir = temp_dir("bundle_pack_output_symlink");
    let source = dir.join("server.mjs");
    let target = dir.join("target.tgz");
    let output = dir.join("service.tgz");
    fs::write(&source, b"service").expect("write source");
    fs::write(&target, b"target must survive").expect("write symlink target");
    symlink(&target, &output).expect("create output symlink");
    let report = BundlePackArgs {
        source,
        archive_path: "app/server.mjs".to_owned(),
        output: output.clone(),
        executable: true,
    }
    .run()
    .expect("replace output symlink");
    assert_eq!(
        fs::read(&target).expect("read untouched target"),
        b"target must survive"
    );
    assert!(
        fs::symlink_metadata(&output)
            .expect("inspect replaced output")
            .is_file()
    );
    let output_bytes = fs::read(&output).expect("read installed archive");
    assert_eq!(report.bundle_hash, Hash::new(&output_bytes));
}
#[test]
fn bundle_pack_archive_size_limit_accepts_boundary_and_rejects_overflow() {
    ensure_bundle_pack_archive_size_within_limit(INROU_BUNDLE_PACK_MAX_ARCHIVE_BYTES)
        .expect("archive size at limit");
    let error =
        ensure_bundle_pack_archive_size_within_limit(INROU_BUNDLE_PACK_MAX_ARCHIVE_BYTES + 1)
            .expect_err("archive size above limit must fail");
    assert!(format!("{error:?}").contains("exceeds the"));
}
#[test]
fn bundle_pack_archive_writer_stops_before_emitting_bytes_over_its_limit() {
    let dir = temp_dir("bundle_pack_archive_writer_limit");
    let path = dir.join("staged.tgz");
    let mut file = fs::File::create(&path).expect("create staged archive");
    {
        let mut writer = BundlePackArchiveWriter::new(&mut file, 4);
        writer.write_all(b"four").expect("write exact boundary");
        let error = writer
            .write_all(b"overflow")
            .expect_err("write beyond archive boundary must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(writer.written_bytes(), 4);
    }
    assert_eq!(fs::metadata(path).expect("inspect staged archive").len(), 4);
}
#[cfg(windows)]
#[test]
fn windows_bundle_pack_snapshots_handles_and_atomically_replaces_existing_output() {
    let dir = temp_dir("bundle_pack_windows_replace");
    let staged = dir.join("staged.tgz");
    let output = dir.join("output.tgz");
    fs::write(&staged, b"new archive").expect("write staged archive");
    fs::write(&output, b"old archive").expect("write old archive");
    let staged_handle = open_direct_bundle_pack_file(&staged).expect("open staged archive");
    let staged_snapshot =
        snapshot_bundle_pack_handle(&staged_handle).expect("snapshot staged archive");
    assert!(staged_snapshot.is_regular_single_file());
    assert_eq!(
        staged_snapshot.size(),
        u64::try_from(b"new archive".len()).expect("test payload length")
    );
    atomic_replace_bundle_pack_file(&staged, &output).expect("atomically replace existing output");
    let output_handle = open_direct_bundle_pack_file(&output).expect("open replaced output");
    let output_snapshot =
        snapshot_bundle_pack_handle(&output_handle).expect("snapshot replaced output");
    assert!(staged_snapshot.same_identity(output_snapshot));
    assert_eq!(
        fs::read(output).expect("read replaced output"),
        b"new archive"
    );
}
#[test]
fn generated_http_service_build_uses_offline_canonical_bundle_packer() {
    let script = http_service_build_sh("service.tgz");
    assert!(script.contains("IROHA_BIN"));
    assert!(script.contains("IROHA_BIN_SHA256"));
    assert!(script.contains("must be an absolute, executable, non-symlinked regular file"));
    assert!(!script.contains("IROHA_MANIFEST_PATH"));
    assert!(!script.contains("IROHA_SOURCE_DIR"));
    assert!(!script.contains("cargo run"));
    assert!(script.contains("\"${IROHA_CMD[@]}\" soracloud service bundle-pack"));
    assert!(script.contains("--source \"$SCRIPT_DIR/app/server.mjs\""));
    assert!(script.contains("--archive-path \"app/server.mjs\""));
    assert!(script.contains("--output \"$BUNDLE_PATH\""));
    assert!(script.contains("--executable"));
    assert!(!script.contains("tar -czf"));
    assert!(!script.contains("STAGING_DIR"));
    assert!(!script.contains("rm -rf"));
}
#[test]
fn generated_contract_builds_require_digest_qualified_koto_binary() {
    for script in [
        hayahi_app_build_sh(),
        single_api_api_build_sh(),
        single_api_api_verify_build_sh(),
        split_app_vault_build_sh(),
        split_app_vault_verify_build_sh(),
    ] {
        assert!(script.contains("KOTO_BIN must name the absolute"));
        assert!(script.contains("KOTO_BIN_SHA256"));
        assert!(script.contains("operator-qualified same-revision SHA-256"));
        assert!(!script.contains("command -v koto"));
        assert!(!script.contains("cargo run"));
    }
}
#[test]
fn bundle_pack_parses_as_offline_service_command() {
    use clap::Parser as _;
    #[derive(clap::Parser)]
    struct ServiceParser {
        #[command(subcommand)]
        command: ServiceCommand,
    }
    let parsed = ServiceParser::try_parse_from([
        "service",
        "bundle-pack",
        "--source",
        "server.mjs",
        "--archive-path",
        "app/server.mjs",
        "--output",
        "service.tgz",
        "--executable",
    ])
    .expect("parse bundle-pack command");
    let ServiceCommand::BundlePack(args) = &parsed.command else {
        panic!("expected bundle-pack service command");
    };
    assert_eq!(args.source, PathBuf::from("server.mjs"));
    assert_eq!(args.archive_path, "app/server.mjs");
    assert_eq!(args.output, PathBuf::from("service.tgz"));
    assert!(args.executable);
    assert!(parsed.command.allows_fallback_config());
}
