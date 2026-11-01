// Local native filesystem controls; none contacts Taira or exercises Unix host administration.

fn native_taira_workspace_fixture() -> (tempfile::TempDir, [PathBuf; 3], PathBuf) {
    let temporary = taira_test_tempdir("native-taira-portable-");
    let sources = ["kernel", "rootfs", "initrd"].map(|name| temporary.path().join(name));
    for (index, source) in sources.iter().enumerate() {
        fs::write(source, format!("original guest member {index}")).unwrap();
    }
    let workspace = temporary.path().join("workspace");
    (temporary, sources, workspace)
}

fn native_taira_acl(path: &Path) -> Vec<u8> {
    let output = Command::new("icacls.exe").arg(path).output().unwrap();
    assert!(output.status.success(), "native ACL query");
    output.stdout
}

fn native_taira_grant_read(path: &Path) {
    let output = Command::new("icacls.exe")
        .arg(path)
        .args(["/grant", "*S-1-1-0:(R)"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "create actual public-read ACL fixture"
    );
}

#[test]
fn native_taira_workspace_copies_exact_sources_into_fresh_private_layout() {
    let (_temporary, sources, workspace) = native_taira_workspace_fixture();
    let originals = sources.each_ref().map(|path| {
        iroha_fs::FileMetadata::symlink_metadata(path)
            .unwrap()
            .identity()
    });
    let receipt =
        create_taira_inrou_canary_workspace(&sources[0], &sources[1], &sources[2], &workspace)
            .unwrap();
    let root = iroha_fs::PrivateDirectory::open(&workspace).unwrap();
    let root_id = root.identity().unwrap();
    let mut total = 0;
    for ((source, name), identity) in sources
        .iter()
        .zip([
            TAIRA_INROU_WORKSPACE_KERNEL_FILE_V1,
            TAIRA_INROU_WORKSPACE_ROOTFS_FILE_V1,
            TAIRA_INROU_WORKSPACE_INITRD_FILE_V1,
        ])
        .zip(originals)
    {
        let expected = fs::read(source).unwrap();
        total += u64::try_from(expected.len()).unwrap();
        assert_eq!(
            &*iroha_fs::read_private(workspace.join(name), 1024).unwrap(),
            &expected
        );
        assert_eq!(
            iroha_fs::FileMetadata::symlink_metadata(source)
                .unwrap()
                .identity(),
            identity
        );
    }
    assert_eq!(receipt.guest_total_bytes, total);
    validate_generated_taira_inrou_workspace(&workspace).unwrap();
    assert!(
        create_taira_inrou_canary_workspace(&sources[0], &sources[1], &sources[2], &workspace)
            .is_err()
    );
    assert_eq!(root.identity().unwrap(), root_id);
    validate_generated_taira_inrou_workspace(&workspace).unwrap();
}

#[test]
fn native_taira_workspace_refuses_duplicate_linked_and_empty_sources_before_creation() {
    let (temporary, sources, workspace) = native_taira_workspace_fixture();
    assert!(
        create_taira_inrou_canary_workspace(&sources[0], &sources[0], &sources[2], &workspace)
            .is_err()
    );
    assert!(!workspace.exists());
    let alias = temporary.path().join("kernel-alias");
    fs::hard_link(&sources[0], &alias).unwrap();
    assert!(
        create_taira_inrou_canary_workspace(&sources[0], &sources[1], &sources[2], &workspace)
            .is_err()
    );
    assert!(!workspace.exists());
    fs::remove_file(alias).unwrap();
    fs::write(&sources[0], b"").unwrap();
    assert!(
        create_taira_inrou_canary_workspace(&sources[0], &sources[1], &sources[2], &workspace)
            .is_err()
    );
    assert!(!workspace.exists());
}

#[test]
fn native_taira_copy_keeps_captured_original_and_never_replaces_output() {
    let (temporary, sources, _) = native_taira_workspace_fixture();
    let captured = taira_inrou_workspace_source_metadata(&sources[0], "copy source").unwrap();
    let identity = iroha_fs::FileMetadata::symlink_metadata(&sources[0])
        .unwrap()
        .identity();
    assert!(
        fs::OpenOptions::new()
            .write(true)
            .open(&sources[0])
            .is_err(),
        "captured reader excludes writers"
    );
    assert!(
        fs::rename(&sources[0], temporary.path().join("renamed")).is_err(),
        "captured original excludes rename"
    );
    let destination = temporary.path().join("copied");
    let expected = fs::read(&sources[0]).unwrap();
    assert_eq!(
        copy_taira_stage_source_file(&sources[0], &destination, &captured).unwrap(),
        u64::try_from(expected.len()).unwrap()
    );
    assert_eq!(
        &*iroha_fs::read_private(&destination, expected.len()).unwrap(),
        &expected
    );
    let copied_identity = iroha_fs::FileMetadata::symlink_metadata(&destination)
        .unwrap()
        .identity();
    assert_ne!(copied_identity, identity);
    assert!(copy_taira_stage_source_file(&sources[0], &destination, &captured).is_err());
    assert_eq!(
        iroha_fs::FileMetadata::symlink_metadata(&destination)
            .unwrap()
            .identity(),
        copied_identity
    );
    assert_eq!(fs::read(&destination).unwrap(), expected);
    let wrong = temporary.path().join("wrong-route-output");
    assert!(copy_taira_stage_source_file(&sources[1], &wrong, &captured).is_err());
    assert!(!wrong.exists());
    // A second destination reads the same retained original from offset zero.
    let second = temporary.path().join("second");
    copy_taira_stage_source_file(&sources[0], &second, &captured).unwrap();
    assert_eq!(fs::read(second).unwrap(), expected);
    drop(captured);
    assert!(fs::OpenOptions::new().write(true).open(&sources[0]).is_ok());
}

#[test]
fn native_taira_private_stage_reads_preserve_bounds_and_reader_policy() {
    let temporary = taira_test_tempdir("native-taira-read-");
    let source = temporary.path().join("source");
    write_taira_stage_file(&source, b"four").unwrap();
    assert_eq!(
        taira_stage_owned_file_bytes(&source, "fixture", 4).unwrap(),
        b"four"
    );
    assert!(taira_stage_owned_file_bytes(&source, "fixture", 3).is_err());
    let before = fs::read(&source).unwrap();
    assert!(write_taira_stage_file(&source, b"overwrite").is_err());
    assert_eq!(fs::read(&source).unwrap(), before);
    let empty = temporary.path().join("empty");
    write_taira_stage_file(&empty, b"").unwrap();
    assert!(taira_stage_regular_file_bytes(&empty, "fixture", 4).is_err());
    assert!(taira_stage_owned_file_bytes(&empty, "fixture", 4).is_err());
    native_taira_grant_read(&source);
    let acl = native_taira_acl(&source);
    assert_eq!(
        taira_stage_regular_file_bytes(&source, "fixture", 4).unwrap(),
        b"four"
    );
    assert!(taira_stage_owned_file_bytes(&source, "fixture", 4).is_err());
    assert_eq!(native_taira_acl(&source), acl);
}

#[test]
fn native_taira_directory_creation_refuses_existing_or_nonprivate_children() {
    let temporary = taira_test_tempdir("native-taira-create-");
    let output = temporary.path().join("output");
    let owner = create_taira_owner_only_directory(&output, "fixture").unwrap();
    let identity = owner.identity().unwrap();
    assert!(create_taira_owner_only_directory(&output, "fixture").is_err());
    assert_eq!(owner.identity().unwrap(), identity);
    let (created_parent, retained_parent) = create_taira_stage_member_parent(
        &output,
        &["created".into(), "deep".into(), "file".into()],
    )
    .unwrap();
    assert_eq!(created_parent, output.join("created/deep"));
    assert!(fs::rename(&created_parent, output.join("moved")).is_err());
    write_taira_stage_file(&created_parent.join("file"), b"retained parent").unwrap();
    retained_parent.revalidate().unwrap();
    let child = owner.create_child("nested").unwrap();
    let child_path = child.path().to_owned();
    drop(child);
    native_taira_grant_read(&child_path);
    let acl = native_taira_acl(&child_path);
    assert!(create_taira_stage_member_parent(&output, &["nested".into(), "file".into()]).is_err());
    assert_eq!(native_taira_acl(&child_path), acl);
    assert!(!child_path.join("file").exists());
    assert!(create_taira_stage_member_parent(&output, &["..".into(), "file".into()]).is_err());
    owner.revalidate().unwrap();
}

#[test]
fn native_taira_workspace_and_private_reads_refuse_junction_routes() {
    let (temporary, sources, _) = native_taira_workspace_fixture();
    let aliases = taira_test_tempdir("native-taira-alias-");
    let junction = aliases.path().join("junction");
    let output = Command::new("cmd.exe")
        .current_dir(aliases.path())
        .args(["/D", "/C", "mklink", "/J"])
        .arg(&junction)
        .arg(temporary.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "actual junction fixture must be created"
    );
    let destination = aliases.path().join("workspace");
    let refused = create_taira_inrou_canary_workspace(
        &junction.join("kernel"),
        &sources[1],
        &sources[2],
        &destination,
    )
    .is_err();
    let private = temporary.path().join("private");
    write_taira_stage_file(&private, b"private").unwrap();
    let read_refused =
        taira_stage_owned_file_bytes(&junction.join("private"), "private fixture", 32).is_err();
    fs::remove_dir(junction).unwrap();
    assert!(refused && read_refused);
    assert!(!destination.exists());
    assert_eq!(
        taira_stage_owned_file_bytes(&private, "private fixture", 32).unwrap(),
        b"private"
    );
}

#[test]
fn native_taira_stage_roundtrip_keeps_canonical_verification_and_private_tree() {
    let (temporary, sources, workspace) = native_taira_workspace_fixture();
    create_taira_inrou_canary_workspace(&sources[0], &sources[1], &sources[2], &workspace).unwrap();
    let stage = temporary.path().join("stage");
    let key = soracloud_fixture_key_pair(0x6A);
    let receipt = stage_taira_inrou_canary_deployment(
        crate::taira::InrouCanaryMode::Deploy,
        &workspace.join(TAIRA_INROU_WORKSPACE_CONTAINER_FILE_V1),
        &workspace.join(TAIRA_INROU_WORKSPACE_SERVICE_FILE_V1),
        &workspace.join(TAIRA_INROU_WORKSPACE_BUNDLE_FILE_V1),
        &stage,
        &key,
        test_sorafs_retention_epoch(),
        test_inrou_placement_targets(4),
    )
    .unwrap();
    let root = iroha_fs::PrivateDirectory::open(&stage).unwrap();
    let identity = root.identity().unwrap();
    let verified = load_verified_taira_inrou_stage(&stage, &key, MutationMode::Deploy).unwrap();
    assert_eq!(
        json::to_vec(&verified.receipt).unwrap(),
        json::to_vec(&receipt).unwrap()
    );
    assert_eq!(
        verified.bundle.service.placement_targets,
        receipt.placement_targets
    );
    assert_eq!(
        verified.bundle.container.bundle_hash.to_string(),
        receipt.bundle_hash
    );
    assert!(load_verified_taira_inrou_stage(&stage, &key, MutationMode::Upgrade).is_err());
    write_taira_stage_file(&stage.join("unexpected"), b"unknown").unwrap();
    assert!(load_verified_taira_inrou_stage(&stage, &key, MutationMode::Deploy).is_err());
    fs::remove_file(stage.join("unexpected")).unwrap();
    load_verified_taira_inrou_stage(&stage, &key, MutationMode::Deploy).unwrap();
    native_taira_grant_read(&stage.join(TAIRA_INROU_STAGE_RECEIPT_FILE_V1));
    assert!(load_verified_taira_inrou_stage(&stage, &key, MutationMode::Deploy).is_err());
    assert_eq!(root.identity().unwrap(), identity);
}

#[test]
fn native_taira_failed_operation_retains_original_root_and_partial_output() {
    let temporary = taira_test_tempdir("native-taira-partial-");
    let output = temporary.path().join("output");
    let owner = create_taira_owner_only_directory(&output, "partial fixture").unwrap();
    let identity = owner.identity().unwrap();
    write_taira_stage_file(&output.join("partial"), b"owned partial bytes").unwrap();
    assert!(fs::rename(&output, temporary.path().join("replacement")).is_err());
    let result: Result<()> = Err(eyre!("original body failure"));
    let error = taira_stage_windows::finish_directory(&owner, result).unwrap_err();
    assert!(format!("{error:#}").contains("original body failure"));
    assert_eq!(owner.identity().unwrap(), identity);
    assert_eq!(
        fs::read(output.join("partial")).unwrap(),
        b"owned partial bytes"
    );
    assert!(!temporary.path().join("replacement").exists());
}

#[test]
fn native_taira_final_source_check_refuses_late_acl_drift_after_copy() {
    let (temporary, sources, _) = native_taira_workspace_fixture();
    let captured = taira_inrou_workspace_source_metadata(&sources[0], "copy source").unwrap();
    let destination = temporary.path().join("copied");
    copy_taira_stage_source_file(&sources[0], &destination, &captured).unwrap();
    let output = Command::new("icacls.exe")
        .arg(&sources[0])
        .args(["/grant", "*S-1-1-0:(W)"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "introduce actual post-copy source ACL drift"
    );
    let captured = [captured];
    assert!(taira_stage_windows::finish_sources(&captured, Ok(())).is_err());
    let first: Result<()> = Err(eyre!("first stage failure"));
    let error = taira_stage_windows::finish_sources(&captured, first).unwrap_err();
    assert!(format!("{error:#}").contains("first stage failure"));
    assert_eq!(
        &*iroha_fs::read_private(&destination, 1024).unwrap(),
        b"original guest member 0"
    );
}
