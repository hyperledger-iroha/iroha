// Windows online-publication fixtures model an already completed offline qualification.
// They do not execute, port or qualify the Unix operator-preseed helper or its store ingestion.

fn qualify_test_inrou_service(
    root: &Path,
    bundle_file: &Path,
    key_pair: &KeyPair,
    label: &str,
) -> PathBuf {
    let container_path = root.join("container_manifest.json");
    let service_path = root.join("service_manifest.json");
    let mut bundle = UnpublishedDeploymentBundleV1 {
        container: load_json(&container_path).expect("read modelled qualification container"),
        service: load_json(&service_path).expect("read modelled qualification service"),
    };
    validate_inrou_preseed_artifact_count([&bundle]).expect("modelled artifact count");
    let required_count = required_inrou_preseed_store_count([&bundle])
        .expect("modelled qualification requires Inrou");
    let (helper, helper_digest) = test_inrou_preseed_helper(root);
    let config = validate_inrou_operator_preseed(
        Some(required_count),
        &test_inrou_preseed_targets(root),
        test_inrou_preseed_capacity(),
        helper.as_deref(),
        helper_digest.as_deref(),
    )
    .expect("validate modelled qualification inputs")
    .expect("required modelled qualification");
    bind_exact_inrou_placement_targets(&mut bundle.service, &config.placement_targets())
        .expect("bind modelled qualification targets");
    let workspace =
        direct_service_artifact_workspace_dir(&container_path, &service_path, bundle_file);
    let release_identity = SorafsReleaseIdentityV1::new(test_sorafs_retention_epoch());
    let mut prepared = prepare_service_artifacts(
        bundle_file,
        workspace.as_deref(),
        bundle,
        key_pair,
        release_identity,
    )
    .expect("prepare exact modelled qualification artifacts");
    if service_uses_public_inrou_http_route(&prepared.admitted_bundle) {
        let (_, _, discovery) =
            prepare_public_service_discovery(&prepared.admitted_bundle, key_pair, release_identity)
                .expect("prepare modelled public discovery artifact");
        prepared.artifacts.push(discovery);
    }
    let artifacts = distinct_prepared_sorafs_artifacts(prepared.artifacts.iter());
    let receipt = expected_inrou_preseed_receipt(&config, &artifacts)
        .expect("construct canonical modelled qualification");
    receipt.validate().expect("valid modelled qualification");
    let directory_path = root.join(format!("{label}-inrou-qualification"));
    let directory = iroha_fs::PrivateDirectory::open_or_create(&directory_path)
        .expect("create native private qualification fixture directory");
    let bytes = json::to_vec(&receipt).expect("encode modelled qualification");
    directory
        .write_atomic(
            "qualification.json",
            &bytes,
            iroha_fs::PublishMode::CreateNew,
        )
        .expect("publish private modelled qualification fixture");
    directory
        .revalidate()
        .expect("qualification directory custody");
    directory_path.join("qualification.json")
}

fn native_inrou_qualification_fixture()
-> (tempfile::TempDir, PathBuf, OperatorPreseedSessionReceiptV1) {
    let temporary = tempfile::tempdir().expect("native qualification fixture root");
    let root = temporary.path();
    let (helper, digest) = test_inrou_preseed_helper(root);
    let config = test_inrou_preseed_config(
        root,
        helper.as_deref().expect("fixture helper path"),
        digest.as_deref().expect("fixture helper digest"),
    );
    let artifact = test_inrou_preseed_artifact(root, "qualification");
    let receipt = expected_inrou_preseed_receipt(&config, &[&artifact])
        .expect("canonical modelled qualification");
    receipt.validate().expect("valid modelled qualification");
    let path = native_inrou_qualification_write(
        root,
        "qualification.json",
        &json::to_vec(&receipt).expect("encode qualification"),
    );
    (temporary, path, receipt)
}

fn native_inrou_qualification_write(root: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    use std::io::Write as _;

    let path = root.join(name);
    let mut writer = iroha_fs::RetainedFile::create_new_private(&path)
        .expect("create private qualification fixture");
    writer
        .file_mut()
        .write_all(bytes)
        .expect("write qualification");
    writer.file().sync_all().expect("sync qualification");
    writer.revalidate().expect("qualification writer custody");
    drop(writer);
    path
}

fn native_inrou_qualification_acl(path: &Path) -> Vec<u8> {
    let output = std::process::Command::new("icacls.exe")
        .arg(path)
        .output()
        .expect("inspect qualification ACL");
    assert!(output.status.success(), "qualification ACL query");
    output.stdout
}

#[test]
fn native_inrou_qualification_loads_private_canonical_original_without_mutation() {
    let (temporary, path, receipt) = native_inrou_qualification_fixture();
    let before = iroha_fs::RetainedFile::open_private(&path)
        .unwrap()
        .snapshot()
        .unwrap();
    let before_acl = native_inrou_qualification_acl(&path);
    let bytes = json::to_vec(&receipt).unwrap();
    // Both the ordinary absolute path and the real canonical local-drive spelling are valid.
    for source in [path.clone(), path.canonicalize().unwrap()] {
        let loaded = load_inrou_preseed_qualification(
            Some(SORACLOUD_ARTIFACT_MIN_REPLICAS_V1),
            Some(&source),
        )
        .unwrap()
        .unwrap();
        assert_eq!(loaded.path, source);
        assert_eq!(loaded.receipt, receipt);
        assert_eq!(loaded.bytes, bytes);
        assert_eq!(loaded.targets.len(), SORACLOUD_ARTIFACT_MIN_REPLICAS_V1);
        assert_eq!(loaded.max_capacity_bytes.get(), 64 * 1024 * 1024);
        assert_eq!(loaded.receipt.mode, "ingest");
        assert_eq!(loaded.receipt.artifacts.len(), 1);
        assert_eq!(
            loaded.receipt.artifacts[0].store_count,
            u32::try_from(SORACLOUD_ARTIFACT_MIN_REPLICAS_V1).unwrap()
        );
        loaded.revalidate().unwrap();
        let different = test_inrou_preseed_artifact(temporary.path(), "different");
        assert!(loaded.require_exact_artifacts(&[&different]).is_err());
    }
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(native_inrou_qualification_acl(&path), before_acl);
    assert_eq!(
        iroha_fs::RetainedFile::open_private(&path)
            .unwrap()
            .snapshot()
            .unwrap(),
        before
    );
}

#[test]
fn native_inrou_qualification_enforces_nonempty_and_exact_read_bound() {
    let temporary = tempfile::tempdir().unwrap();
    let maximum = usize::try_from(INROU_PRESEED_RECEIPT_MAX_BYTES).unwrap();
    for (index, length) in [0, maximum, maximum + 1].into_iter().enumerate() {
        let bytes = vec![b'A'; length];
        let path = native_inrou_qualification_write(
            temporary.path(),
            &format!("bound-{index}.json"),
            &bytes,
        );
        let result = read_inrou_preseed_qualification_file(&path);
        if length == maximum {
            assert_eq!(result.unwrap(), bytes);
        } else {
            assert!(result.is_err());
        }
        assert!(load_inrou_preseed_qualification(Some(1), Some(&path)).is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
}

#[test]
fn native_inrou_qualification_refuses_hardlinks_missing_and_directory_sources() {
    let (temporary, path, receipt) = native_inrou_qualification_fixture();
    let alias = temporary.path().join("alias.json");
    fs::hard_link(&path, &alias).unwrap();
    for source in [&path, &alias] {
        assert!(load_inrou_preseed_qualification(Some(1), Some(source)).is_err());
        assert_eq!(fs::read(source).unwrap(), json::to_vec(&receipt).unwrap());
    }
    fs::remove_file(alias).unwrap();
    assert!(load_inrou_preseed_qualification(Some(1), Some(&path)).is_ok());
    assert!(load_inrou_preseed_qualification(Some(1), Some(temporary.path())).is_err());
    assert!(
        load_inrou_preseed_qualification(Some(1), Some(&temporary.path().join("missing.json")))
            .is_err()
    );
}

#[test]
fn native_inrou_qualification_refuses_broad_acl_without_repair() {
    let (_temporary, path, receipt) = native_inrou_qualification_fixture();
    let output = std::process::Command::new("icacls.exe")
        .arg(&path)
        .args(["/grant", "*S-1-1-0:(R)"])
        .output()
        .unwrap();
    assert!(output.status.success(), "create actual broad read ACL");
    let before = native_inrou_qualification_acl(&path);
    assert!(load_inrou_preseed_qualification(Some(1), Some(&path)).is_err());
    assert_eq!(native_inrou_qualification_acl(&path), before);
    assert_eq!(fs::read(&path).unwrap(), json::to_vec(&receipt).unwrap());
}

#[test]
fn native_inrou_qualification_refuses_relative_nonlocal_and_traversal_routes() {
    for source in [
        r"qualification.json",
        r"\\server\share\qualification.json",
        r"\\?\UNC\server\share\qualification.json",
        r"\\.\C:\qualification.json",
    ] {
        assert!(load_inrou_preseed_qualification(Some(1), Some(Path::new(source))).is_err());
    }
    let (temporary, path, _) = native_inrou_qualification_fixture();
    let child = temporary.path().join("child");
    fs::create_dir(&child).unwrap();
    let traversal = child.join("..").join("qualification.json");
    assert!(load_inrou_preseed_qualification(Some(1), Some(&traversal)).is_err());
    assert!(load_inrou_preseed_qualification(Some(1), Some(&path)).is_ok());
}

#[test]
fn native_inrou_qualification_rejects_noncanonical_bytes_and_insufficient_targets() {
    let (temporary, path, receipt) = native_inrou_qualification_fixture();
    let mut bytes = json::to_vec(&receipt).unwrap();
    bytes.push(b'\n');
    // A valid JSON value with noncanonical framing must not become qualification evidence.
    assert_eq!(
        json::from_slice::<OperatorPreseedSessionReceiptV1>(&bytes).unwrap(),
        receipt
    );
    let noncanonical = native_inrou_qualification_write(temporary.path(), "newline.json", &bytes);
    let error = load_inrou_preseed_qualification(Some(1), Some(&noncanonical))
        .err()
        .expect("noncanonical receipt must refuse");
    assert!(format!("{error:#}").contains("canonical JSON"));
    let error =
        load_inrou_preseed_qualification(Some(SORACLOUD_ARTIFACT_MIN_REPLICAS_V1 + 1), Some(&path))
            .err()
            .expect("insufficient qualified targets must refuse");
    assert!(format!("{error:#}").contains("qualified targets"));
    assert_eq!(fs::read(&path).unwrap(), json::to_vec(&receipt).unwrap());
}

#[test]
fn native_inrou_qualification_revalidation_rejects_changed_private_bytes() {
    let (_temporary, path, receipt) = native_inrou_qualification_fixture();
    let loaded = load_inrou_preseed_qualification(Some(1), Some(&path))
        .unwrap()
        .unwrap();
    let mut changed = receipt;
    changed.max_capacity_bytes += 1;
    let bytes = json::to_vec(&changed).unwrap();
    fs::write(&path, &bytes).unwrap();
    assert!(load_inrou_preseed_qualification(Some(1), Some(&path)).is_ok());
    let error = loaded
        .revalidate()
        .expect_err("changed original must refuse");
    assert!(format!("{error:#}").contains("changed before online publication"));
    assert_eq!(fs::read(&path).unwrap(), bytes);
}

#[test]
fn native_inrou_qualification_load_refuses_junction_ancestry() {
    let (temporary, path, receipt) = native_inrou_qualification_fixture();
    let alias_root = tempfile::tempdir().unwrap();
    let junction = alias_root.path().join("junction");
    let output = std::process::Command::new("cmd.exe")
        .current_dir(alias_root.path())
        .args(["/D", "/C", "mklink", "/J"])
        .arg(&junction)
        .arg(temporary.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "actual junction fixture must be created"
    );
    let through_junction = junction.join("qualification.json");
    let refused = load_inrou_preseed_qualification(Some(1), Some(&through_junction)).is_err();
    fs::remove_dir(&junction).expect("remove only owned junction");
    assert!(refused, "load must not canonicalize away reparse ancestry");
    assert!(load_inrou_preseed_qualification(Some(1), Some(&path)).is_ok());
    assert_eq!(fs::read(&path).unwrap(), json::to_vec(&receipt).unwrap());
}
