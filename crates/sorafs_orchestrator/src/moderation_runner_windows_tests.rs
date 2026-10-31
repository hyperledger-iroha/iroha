// Native controls included in the existing tests module to reuse its unchanged
// model and signed-manifest fixtures. No native pass is inferred from source checks.

fn native_model_fixture(private: bool) -> (tempfile::TempDir, PathBuf) {
    let temporary = tempdir().expect("native fixture");
    let root = temporary.path().join("models");
    if private {
        drop(iroha_fs::PrivateDirectory::open_or_create(&root).expect("private root"));
    } else {
        fs::create_dir(&root).expect("normal root");
    }
    (temporary, root)
}

fn native_model_acl(path: &Path) -> Vec<u8> {
    let output = std::process::Command::new("icacls.exe")
        .arg(path)
        .output()
        .expect("query fixture ACL");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn native_model_grant_write(path: &Path) {
    let output = std::process::Command::new("icacls.exe")
        .arg(path)
        .args(["/grant", "*S-1-1-0:(W)"])
        .output()
        .expect("grant unsafe fixture mutation");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn native_model_junction(path: &Path, target: &Path) {
    let output = std::process::Command::new("cmd.exe")
        .args(["/D", "/C", "mklink", "/J"])
        .arg(path)
        .arg(target)
        .output()
        .expect("create owned junction");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn native_models_load_nested_safe_normal_and_private_inputs_without_acl_repair() {
    for private in [false, true] {
        let (_temporary, root) = native_model_fixture(private);
        let nested = root.join("nested");
        fs::create_dir(&nested).unwrap();
        let model = artifact([1; 16], 1);
        let (fingerprint, bytes) =
            fingerprint_model_artifact("nested/model.norito", &model, None).unwrap();
        let path = nested.join("model.norito");
        fs::write(&path, &bytes).unwrap();
        let before = (
            native_model_acl(&root),
            native_model_acl(&nested),
            native_model_acl(&path),
        );
        let source = iroha_fs::RetainedFile::open_regular(&path).unwrap();
        let snapshot = source.snapshot().unwrap();
        drop(source);
        let manifest = signed_manifest(vec![fingerprint]);
        let runner =
            LoadedModerationRunnerV1::load_verified(manifest.clone(), &root, [3; 32]).unwrap();
        assert_eq!(runner.models[0].artifact, model);
        assert_eq!(
            runner.canonical_artifacts().unwrap(),
            vec![("nested/model.norito".to_owned(), bytes)]
        );
        let score = runner.infer(b"aaaa", 1024).unwrap();
        assert_eq!(score, runner.infer(b"aaaa", 1024).unwrap());
        assert_eq!(score.model_scores.len(), 1);
        assert_eq!(score.model_scores[0].model_id, model.model_id);
        assert_eq!(score.model_scores[0].score_bps, 10_000);
        assert_eq!(score.combined_score_bps, 10_000);
        let verbatim_root = fs::canonicalize(&root).expect("real local verbatim root");
        assert!(verbatim_root.to_string_lossy().starts_with(r"\\?\"));
        let verbatim =
            LoadedModerationRunnerV1::load_verified(manifest, &verbatim_root, [3; 32]).unwrap();
        assert_eq!(verbatim.infer(b"aaaa", 1024).unwrap(), score);
        assert_eq!(
            verbatim.canonical_artifacts().unwrap(),
            runner.canonical_artifacts().unwrap()
        );
        assert_eq!(
            (
                native_model_acl(&root),
                native_model_acl(&nested),
                native_model_acl(&path)
            ),
            before
        );
        assert_eq!(
            iroha_fs::RetainedFile::open_regular(&path)
                .unwrap()
                .snapshot()
                .unwrap(),
            snapshot
        );
    }
}

#[test]
fn native_models_refuse_unc_device_roots_and_noncanonical_relative_names() {
    for path in [
        r"\\server\share\models",
        r"\\?\UNC\server\share\models",
        r"\\.\C:\models",
    ] {
        assert!(matches!(
            validate_artifact_root(Path::new(path)),
            Err(ModerationRunnerError::InvalidArtifactRoot { .. })
        ));
    }
    let (_temporary, root) = native_model_fixture(false);
    let validated = validate_artifact_root(&root).unwrap();
    let (mut fingerprint, _) =
        fingerprint_model_artifact("model.norito", &artifact([1; 16], 1), None).unwrap();
    for name in [
        "../outside",
        "nested\\model.norito",
        "C:/model",
        "/model",
        "nested//model",
    ] {
        fingerprint.artifact_path = name.into();
        let mut calls = 0;
        assert!(matches!(
            windows::load_model_with_post_open_hook(&validated, &fingerprint, |_| calls += 1),
            Err(ModerationRunnerError::UnsafeArtifactPath { .. })
        ));
        assert_eq!(calls, 0);
    }
}

#[test]
fn native_models_refuse_root_ancestor_and_nested_junctions_before_file_admission() {
    let (temporary, root) = native_model_fixture(false);
    let outside = temporary.path().join("outside");
    fs::create_dir(&outside).unwrap();
    let (fingerprint, bytes) =
        fingerprint_model_artifact("nested/model.norito", &artifact([1; 16], 1), None).unwrap();
    fs::write(outside.join("model.norito"), &bytes).unwrap();
    let nested = root.join("nested");
    native_model_junction(&nested, &outside);
    let validated = validate_artifact_root(&root).unwrap();
    let mut calls = 0;
    assert!(
        windows::load_model_with_post_open_hook(&validated, &fingerprint, |_| calls += 1).is_err()
    );
    assert_eq!(calls, 0);
    drop(validated);
    fs::remove_dir(&nested).unwrap();
    let alias = temporary.path().join("alias");
    native_model_junction(&alias, &root);
    assert!(validate_artifact_root(&alias).is_err());
    fs::remove_dir(&alias).unwrap();
    let ancestor = temporary.path().join("ancestor");
    native_model_junction(&ancestor, temporary.path());
    assert!(validate_artifact_root(&ancestor.join("models")).is_err());
    fs::remove_dir(&ancestor).unwrap();
    assert_eq!(fs::read(outside.join("model.norito")).unwrap(), bytes);
}

#[test]
fn native_models_refuse_hardlinked_files_before_reading() {
    let (temporary, root) = native_model_fixture(false);
    let (fingerprint, bytes) =
        fingerprint_model_artifact("model.norito", &artifact([1; 16], 1), None).unwrap();
    let outside = temporary.path().join("outside.norito");
    fs::write(&outside, &bytes).unwrap();
    fs::hard_link(&outside, root.join("model.norito")).unwrap();
    let validated = validate_artifact_root(&root).unwrap();
    let mut calls = 0;
    assert!(
        windows::load_model_with_post_open_hook(&validated, &fingerprint, |_| calls += 1).is_err()
    );
    assert_eq!(calls, 0);
    assert_eq!(fs::read(&outside).unwrap(), bytes);
}

#[test]
fn native_models_refuse_unsafe_root_nested_or_file_acl_without_repair() {
    for kind in ["root", "nested", "file"] {
        let (_temporary, root) = native_model_fixture(true);
        let nested = root.join("nested");
        fs::create_dir(&nested).unwrap();
        let (fingerprint, bytes) =
            fingerprint_model_artifact("nested/model.norito", &artifact([1; 16], 1), None).unwrap();
        let file = nested.join("model.norito");
        fs::write(&file, &bytes).unwrap();
        let path = match kind {
            "root" => &root,
            "nested" => &nested,
            _ => &file,
        };
        native_model_grant_write(path);
        let before = native_model_acl(path);
        assert!(
            LoadedModerationRunnerV1::load_verified(
                signed_manifest(vec![fingerprint]),
                &root,
                [3; 32]
            )
            .is_err()
        );
        assert_eq!(native_model_acl(path), before);
        assert_eq!(fs::read(&file).unwrap(), bytes);
    }
}

#[test]
fn native_models_refuse_live_writer_and_recover_after_original_close() {
    let (_temporary, root) = native_model_fixture(false);
    let (fingerprint, bytes) =
        fingerprint_model_artifact("model.norito", &artifact([1; 16], 1), None).unwrap();
    let path = root.join("model.norito");
    fs::write(&path, bytes).unwrap();
    let writer = fs::OpenOptions::new().write(true).open(&path).unwrap();
    let manifest = signed_manifest(vec![fingerprint]);
    assert!(LoadedModerationRunnerV1::load_verified(manifest.clone(), &root, [3; 32]).is_err());
    drop(writer);
    assert!(LoadedModerationRunnerV1::load_verified(manifest, &root, [3; 32]).is_ok());
}

#[test]
fn native_models_originals_exclude_write_delete_and_rename_until_decode_finishes() {
    let (temporary, root) = native_model_fixture(false);
    let nested = root.join("nested");
    fs::create_dir(&nested).unwrap();
    let model = artifact([1; 16], 1);
    let (fingerprint, bytes) =
        fingerprint_model_artifact("nested/model.norito", &model, None).unwrap();
    let path = nested.join("model.norito");
    fs::write(&path, &bytes).unwrap();
    let validated = validate_artifact_root(&root).unwrap();
    let mut calls = 0;
    let actual = windows::load_model_with_post_open_hook(&validated, &fingerprint, |opened| {
        calls += 1;
        assert_eq!(
            fs::OpenOptions::new()
                .write(true)
                .open(opened)
                .unwrap_err()
                .raw_os_error(),
            Some(32)
        );
        assert_eq!(
            fs::remove_file(opened).unwrap_err().raw_os_error(),
            Some(32)
        );
        assert_eq!(
            fs::rename(opened, temporary.path().join("moved-file"))
                .unwrap_err()
                .raw_os_error(),
            Some(32)
        );
        assert_eq!(
            fs::rename(&nested, temporary.path().join("moved-nested"))
                .unwrap_err()
                .raw_os_error(),
            Some(32)
        );
        assert_eq!(
            fs::rename(&root, temporary.path().join("moved-root"))
                .unwrap_err()
                .raw_os_error(),
            Some(32)
        );
    })
    .unwrap();
    assert_eq!(calls, 1);
    assert_eq!(actual, model);
    assert_eq!(fs::read(&path).unwrap(), bytes);
    drop(validated);
    assert!(fs::OpenOptions::new().write(true).open(&path).is_ok());
}

#[test]
fn native_models_refuse_post_open_acl_and_directory_changes() {
    for kind in [
        "file-acl",
        "root-acl",
        "nested-acl",
        "root-entry",
        "nested-entry",
    ] {
        let (_temporary, root) = native_model_fixture(true);
        let nested = root.join("nested");
        fs::create_dir(&nested).unwrap();
        let (fingerprint, bytes) =
            fingerprint_model_artifact("nested/model.norito", &artifact([1; 16], 1), None).unwrap();
        let path = nested.join("model.norito");
        fs::write(&path, &bytes).unwrap();
        let validated = validate_artifact_root(&root).unwrap();
        let mut calls = 0;
        assert!(
            windows::load_model_with_post_open_hook(&validated, &fingerprint, |opened| {
                calls += 1;
                match kind {
                    "file-acl" => native_model_grant_write(opened),
                    "root-acl" => native_model_grant_write(&root),
                    "nested-acl" => native_model_grant_write(&nested),
                    "root-entry" => fs::write(root.join("new-entry"), b"mutation").unwrap(),
                    _ => fs::write(nested.join("new-entry"), b"mutation").unwrap(),
                }
            })
            .is_err()
        );
        assert_eq!(calls, 1);
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
}

#[test]
fn native_models_refuse_bad_size_and_signed_oversize_before_read_hook() {
    let (_temporary, root) = native_model_fixture(false);
    let (mut fingerprint, bytes) =
        fingerprint_model_artifact("model.norito", &artifact([1; 16], 1), None).unwrap();
    let path = root.join("model.norito");
    fs::write(&path, &bytes).unwrap();
    let validated = validate_artifact_root(&root).unwrap();
    for expected in [0, MODERATION_MODEL_MAX_ARTIFACT_BYTES_V1 + 1, u64::MAX] {
        fingerprint.artifact_bytes = expected;
        let mut calls = 0;
        assert!(matches!(
            windows::load_model_with_post_open_hook(&validated, &fingerprint, |_| calls += 1),
            Err(ModerationRunnerError::ArtifactDecode { .. })
        ));
        assert_eq!(calls, 0);
    }
    fingerprint.artifact_bytes = u64::try_from(bytes.len()).unwrap();
    drop(validated);
    fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(MODERATION_MODEL_MAX_ARTIFACT_BYTES_V1 + 1)
        .unwrap();
    let validated = validate_artifact_root(&root).unwrap();
    let before = iroha_fs::RetainedFile::open_regular(&path)
        .unwrap()
        .snapshot()
        .unwrap();
    let mut calls = 0;
    assert!(matches!(
        windows::load_model_with_post_open_hook(&validated, &fingerprint, |_| calls += 1),
        Err(ModerationRunnerError::ArtifactSize { .. })
    ));
    assert_eq!(
        calls, 0,
        "oversize refuses before payload read; no allocator measurement claimed"
    );
    assert_eq!(
        iroha_fs::RetainedFile::open_regular(&path)
            .unwrap()
            .snapshot()
            .unwrap(),
        before
    );
}

#[test]
fn native_models_decode_error_precedes_later_custody_failure_and_releases_originals() {
    let (_temporary, root) = native_model_fixture(true);
    let (mut fingerprint, _) =
        fingerprint_model_artifact("model.norito", &artifact([1; 16], 1), None).unwrap();
    let invalid = b"not a Norito model";
    fingerprint.artifact_bytes = u64::try_from(invalid.len()).unwrap();
    fingerprint.artifact_digest = *blake3::hash(invalid).as_bytes();
    let path = root.join("model.norito");
    fs::write(&path, invalid).unwrap();
    let validated = validate_artifact_root(&root).unwrap();
    let error = windows::load_model_with_post_open_hook(&validated, &fingerprint, |_| {
        fs::write(root.join("late-entry"), b"root changed").unwrap();
    })
    .unwrap_err();
    assert!(matches!(
        error,
        ModerationRunnerError::ArtifactDecode { .. }
    ));
    assert!(verify_artifact_root(&validated).is_err());
    drop(validated);
    assert!(fs::OpenOptions::new().write(true).open(&path).is_ok());
}

#[test]
fn native_models_post_open_unwind_releases_file_and_nested_owners() {
    let (temporary, root) = native_model_fixture(false);
    let nested = root.join("nested");
    fs::create_dir(&nested).unwrap();
    let (fingerprint, bytes) =
        fingerprint_model_artifact("nested/model.norito", &artifact([1; 16], 1), None).unwrap();
    let path = nested.join("model.norito");
    fs::write(&path, bytes).unwrap();
    let validated = validate_artifact_root(&root).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = windows::load_model_with_post_open_hook(&validated, &fingerprint, |_| {
            panic!("owned test unwind")
        });
    }));
    assert!(result.is_err());
    assert!(fs::OpenOptions::new().write(true).open(&path).is_ok());
    fs::rename(&nested, root.join("renamed-nested")).unwrap();
    drop(validated);
    fs::rename(&root, temporary.path().join("renamed-root")).unwrap();
}
