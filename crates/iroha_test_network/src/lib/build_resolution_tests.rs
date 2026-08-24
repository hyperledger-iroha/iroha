#[test]
fn workspace_fingerprint_detects_source_modifications() {
    let temp = tempdir().expect("temporary workspace");
    let root = temp.path();
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"member\"]\n",
    )
    .expect("write manifest");
    fs::create_dir_all(root.join("member/src")).expect("create member src directory");
    let file = root.join("member/src/lib.rs");
    fs::write(&file, b"pub fn greet() {}\n").expect("write source file");
    let initial = workspace_fingerprint(root).expect("initial fingerprint");
    thread::sleep(Duration::from_millis(20));
    fs::write(&file, b"pub fn greet() { println!(\"hi\"); }\n").expect("update source file");
    let updated = workspace_fingerprint(root).expect("updated fingerprint");
    assert_ne!(initial, updated);
}
#[test]
fn workspace_fingerprint_ignores_target_directory() {
    let temp = tempdir().expect("temporary workspace");
    let root = temp.path();
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"member\"]\n",
    )
    .expect("write manifest");
    fs::create_dir_all(root.join("member/src")).expect("create member src directory");
    fs::write(root.join("member/src/lib.rs"), b"pub fn greet() {}\n").expect("write source file");
    fs::create_dir_all(root.join("member/target")).expect("create target directory");
    let artifact = root.join("member/target").join("artifact");
    fs::write(&artifact, b"one").expect("write artifact");
    let before = workspace_fingerprint(root).expect("initial fingerprint");
    thread::sleep(Duration::from_millis(20));
    fs::write(&artifact, b"two").expect("update artifact");
    let after = workspace_fingerprint(root).expect("post-artifact fingerprint");
    assert_eq!(before, after);
    thread::sleep(Duration::from_millis(20));
    fs::write(root.join("member/src/lib.rs"), b"pub fn greet() { 1 }\n")
        .expect("update source file");
    let final_fp = workspace_fingerprint(root).expect("final fingerprint");
    assert_ne!(after, final_fp);
}
#[test]
fn workspace_fingerprint_respects_gitignore_directories() {
    let temp = tempdir().expect("temporary workspace");
    let root = temp.path();
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"member\"]\n",
    )
    .expect("write manifest");
    fs::write(root.join(".gitignore"), "member/ignored/\n").expect("write gitignore");
    fs::create_dir_all(root.join("member/src")).expect("create member src directory");
    fs::create_dir_all(root.join("member/ignored")).expect("create ignored directory");
    fs::write(root.join("member/src/lib.rs"), b"pub fn greet() {}\n").expect("write source file");
    let ignored_file = root.join("member/ignored/data.bin");
    fs::write(&ignored_file, b"a").expect("write ignored file");
    let before = workspace_fingerprint(root).expect("initial fingerprint");
    thread::sleep(Duration::from_millis(20));
    fs::write(&ignored_file, b"b").expect("update ignored file");
    let after = workspace_fingerprint(root).expect("post-ignore fingerprint");
    assert_eq!(before, after);
    fs::write(root.join("member/src/lib.rs"), b"pub fn greet() { 2 }\n")
        .expect("update source file");
    let final_fp = workspace_fingerprint(root).expect("final fingerprint");
    assert_ne!(after, final_fp);
}
#[test]
fn workspace_fingerprint_respects_gitignore_globs() {
    let temp = tempdir().expect("temporary workspace");
    let root = temp.path();
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"member\"]\n",
    )
    .expect("write manifest");
    fs::write(root.join(".gitignore"), "target*/\n*.log\n").expect("write gitignore");
    fs::create_dir_all(root.join("member/src")).expect("create member src directory");
    fs::write(root.join("member/src/lib.rs"), b"pub fn greet() {}\n").expect("write source file");
    let target_dir = root.join("member/target-codex");
    fs::create_dir_all(&target_dir).expect("create target-codex directory");
    let target_artifact = target_dir.join("artifact");
    fs::write(&target_artifact, b"one").expect("write target artifact");
    let log_path = root.join("member/build.log");
    fs::write(&log_path, b"initial").expect("write log file");
    let before = workspace_fingerprint(root).expect("initial fingerprint");
    thread::sleep(Duration::from_millis(20));
    fs::write(&target_artifact, b"two").expect("update target artifact");
    fs::write(&log_path, b"updated").expect("update log file");
    let after = workspace_fingerprint(root).expect("post-ignore fingerprint");
    assert_eq!(before, after);
    thread::sleep(Duration::from_millis(20));
    fs::write(root.join("member/src/lib.rs"), b"pub fn greet() { 3 }\n")
        .expect("update source file");
    let final_fp = workspace_fingerprint(root).expect("final fingerprint");
    assert_ne!(after, final_fp);
}
#[test]
fn fingerprint_with_build_args_changes_on_arg_differences() {
    let base = 42_u64;
    let args_a = vec![
        OsString::from("--features"),
        OsString::from("expensive-telemetry"),
    ];
    let args_b = vec![
        OsString::from("--features"),
        OsString::from("other-feature"),
    ];
    let fingerprint_a = fingerprint_with_build_args(base, &args_a);
    let fingerprint_b = fingerprint_with_build_args(base, &args_b);
    assert_ne!(fingerprint_a, fingerprint_b);
    assert_eq!(fingerprint_a, fingerprint_with_build_args(base, &args_a));
}
#[test]
fn workspace_fingerprint_ignores_custom_target_dir_env() {
    let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
    let temp = tempdir().expect("temporary workspace");
    let root = temp.path();
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"member\"]\n",
    )
    .expect("write manifest");
    fs::create_dir_all(root.join("member/src")).expect("create member src directory");
    fs::write(root.join("member/src/lib.rs"), b"pub fn greet() {}\n").expect("write source file");
    let _override_guard = EnvVarGuard::cleared(IROHA_TEST_TARGET_DIR_ENV);
    let _target_guard = EnvVarRestore::set("CARGO_TARGET_DIR", "member/build-output");
    let target_dir = root.join("member/build-output");
    fs::create_dir_all(&target_dir).expect("create target directory");
    let artifact = target_dir.join("artifact");
    fs::write(&artifact, b"one").expect("write target artifact");
    let before = workspace_fingerprint(root).expect("initial fingerprint");
    thread::sleep(Duration::from_millis(20));
    fs::write(&artifact, b"two").expect("update target artifact");
    let after = workspace_fingerprint(root).expect("post-artifact fingerprint");
    assert_eq!(before, after);
    thread::sleep(Duration::from_millis(20));
    fs::write(root.join("member/src/lib.rs"), b"pub fn greet() { 4 }\n")
        .expect("update source file");
    let final_fp = workspace_fingerprint(root).expect("final fingerprint");
    assert_ne!(after, final_fp);
}
#[test]
fn workspace_fingerprint_ignores_test_target_dir_env() {
    let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
    let temp = tempdir().expect("temporary workspace");
    let root = temp.path();
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"member\"]\n",
    )
    .expect("write manifest");
    fs::create_dir_all(root.join("member/src")).expect("create member src directory");
    fs::write(root.join("member/src/lib.rs"), b"pub fn greet() {}\n").expect("write source file");
    let _cargo_guard = EnvVarGuard::cleared("CARGO_TARGET_DIR");
    let _target_guard = EnvVarRestore::set(IROHA_TEST_TARGET_DIR_ENV, "member/test-output");
    let target_dir = root.join("member/test-output");
    fs::create_dir_all(&target_dir).expect("create test target directory");
    let artifact = target_dir.join("artifact");
    fs::write(&artifact, b"one").expect("write target artifact");
    let before = workspace_fingerprint(root).expect("initial fingerprint");
    thread::sleep(Duration::from_millis(20));
    fs::write(&artifact, b"two").expect("update target artifact");
    let after = workspace_fingerprint(root).expect("post-artifact fingerprint");
    assert_eq!(before, after);
    thread::sleep(Duration::from_millis(20));
    fs::write(root.join("member/src/lib.rs"), b"pub fn greet() { 5 }\n")
        .expect("update source file");
    let final_fp = workspace_fingerprint(root).expect("final fingerprint");
    assert_ne!(after, final_fp);
}
#[test]
fn resolve_target_dir_prefers_test_override_and_namespaces_cargo() {
    let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
    let temp = tempdir().expect("temporary workspace");
    let root = temp.path();
    let _clear_release = EnvVarGuard::cleared(IROHA_RELEASE_SOURCE_MANIFEST_SHA256_ENV);
    let _clear_prebuilt = EnvVarGuard::cleared(IROHA_RELEASE_PREBUILT_MANIFEST_SHA256_ENV);
    let _clear_test = EnvVarGuard::cleared(IROHA_TEST_TARGET_DIR_ENV);
    let _clear_cargo = EnvVarGuard::cleared("CARGO_TARGET_DIR");
    assert_eq!(
        resolve_target_dir(root),
        root.join("target").join(IROHA_TEST_TARGET_SUBDIR)
    );
    let _cargo_guard = EnvVarRestore::set("CARGO_TARGET_DIR", "cargo-target");
    assert_eq!(
        resolve_target_dir(root),
        root.join("cargo-target").join(IROHA_TEST_TARGET_SUBDIR)
    );
    let _test_guard = EnvVarRestore::set(IROHA_TEST_TARGET_DIR_ENV, "test-target");
    assert_eq!(resolve_target_dir(root), root.join("test-target"));
}
struct ReleasePrebuiltFixture {
    _temp: tempfile::TempDir,
    repo: PathBuf,
    source_manifest_sha256: String,
    cargo_lock_sha256: String,
    target: PathBuf,
    manifest: PathBuf,
    manifest_sha256: String,
}
impl Drop for ReleasePrebuiltFixture {
    fn drop(&mut self) {
        for directory in [
            self.target.join("message-control/release"),
            self.target.join("message-control"),
            self.target.join("release"),
            self.target.clone(),
        ] {
            set_mode(&directory, 0o700);
        }
    }
}
#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) {
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .unwrap_or_else(|err| panic!("set mode {mode:04o} on {}: {err}", path.display()));
}
#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) {}
fn release_manifest_text(
    source_manifest_sha256: &str,
    cargo_lock_sha256: &str,
    target: &Path,
) -> String {
    let mut rows = vec![
        ("schema_version".to_owned(), "2".to_owned()),
        (
            "source_manifest_sha256".to_owned(),
            source_manifest_sha256.to_owned(),
        ),
        ("cargo_lock_sha256".to_owned(), cargo_lock_sha256.to_owned()),
        ("cargo_version_sha256".to_owned(), "c".repeat(64)),
        ("rustc_version_sha256".to_owned(), "d".repeat(64)),
        ("host_triple".to_owned(), "test-host".to_owned()),
        ("target_triple".to_owned(), "test-target".to_owned()),
        ("profile".to_owned(), "release".to_owned()),
        (
            "bundle_dir".to_owned(),
            target
                .to_str()
                .expect("fixture target must be Unicode")
                .to_owned(),
        ),
    ];
    for kind in ReleasePrebuiltBinary::ALL {
        let path = target.join(kind.relative_path());
        let bytes = fs::read(&path).expect("read fixture release binary");
        let prefix = kind.manifest_prefix();
        rows.extend([
            (
                format!("{prefix}_relative_path"),
                kind.relative_path().to_owned(),
            ),
            (format!("{prefix}_sha256"), lowercase_hex(&sha256(&bytes))),
            (format!("{prefix}_size_bytes"), bytes.len().to_string()),
            (
                format!("{prefix}_mode_octal"),
                RELEASE_BINARY_MODE_OCTAL.to_owned(),
            ),
        ]);
    }
    let mut text = String::new();
    for (key, value) in rows {
        text.push_str(&key);
        text.push('\t');
        text.push_str(&value);
        text.push('\n');
    }
    text
}
fn create_release_prebuilt_fixture() -> ReleasePrebuiltFixture {
    let temp = tempdir().expect("temporary release workspace");
    let repo = temp.path().join("repo");
    fs::create_dir_all(&repo).expect("create release workspace");
    let cargo_lock = repo.join("Cargo.lock");
    fs::write(&cargo_lock, b"release-lock-v1\n").expect("write Cargo.lock");
    let cargo_lock_sha256 = lowercase_hex(&sha256(b"release-lock-v1\n"));
    let source_manifest_sha256 = "a".repeat(64);
    let target = repo
        .join("target")
        .join(SUMERAGI_V2_RELEASE_TARGET_SUBDIR)
        .join(&source_manifest_sha256)
        .join(SUMERAGI_V2_RELEASE_PROGRAMS_SUBDIR)
        .join("invocation.A1b2C3");
    for kind in ReleasePrebuiltBinary::ALL {
        let path = target.join(kind.relative_path());
        fs::create_dir_all(path.parent().expect("binary parent")).expect("create binary parent");
        fs::write(
            &path,
            format!("{} release executable\n", kind.manifest_prefix()),
        )
        .expect("write release binary");
        set_mode(&path, RELEASE_BINARY_MODE);
    }
    let manifest = target.join(SUMERAGI_V2_PREBUILT_MANIFEST);
    let manifest_text = release_manifest_text(&source_manifest_sha256, &cargo_lock_sha256, &target);
    fs::write(&manifest, &manifest_text).expect("write prebuilt manifest");
    set_mode(&manifest, RELEASE_MANIFEST_MODE);
    for directory in [
        target.join("message-control/release"),
        target.join("message-control"),
        target.join("release"),
        target.clone(),
    ] {
        set_mode(&directory, RELEASE_BINARY_MODE);
    }
    let manifest_sha256 = lowercase_hex(&sha256(manifest_text.as_bytes()));
    ReleasePrebuiltFixture {
        _temp: temp,
        repo,
        source_manifest_sha256,
        cargo_lock_sha256,
        target,
        manifest,
        manifest_sha256,
    }
}
fn release_prebuilt_env(
    fixture: &ReleasePrebuiltFixture,
    manifest_sha256: &str,
) -> Vec<EnvVarRestore> {
    vec![
        EnvVarRestore::set(
            IROHA_RELEASE_SOURCE_MANIFEST_SHA256_ENV,
            &fixture.source_manifest_sha256,
        ),
        EnvVarRestore::set(IROHA_RELEASE_PREBUILT_MANIFEST_SHA256_ENV, manifest_sha256),
        EnvVarRestore::set(
            IROHA_RELEASE_CARGO_LOCK_SHA256_ENV,
            &fixture.cargo_lock_sha256,
        ),
        EnvVarRestore::set(IROHA_TEST_TARGET_DIR_ENV, fixture.target.as_os_str()),
        EnvVarRestore::set(IROHA_TEST_SKIP_BUILD_ENV, "1"),
        EnvVarRestore::set(IROHA_TEST_BUILD_PROFILE_ENV, "release"),
        EnvVarRestore::set("PROFILE", "release"),
    ]
}
fn rewrite_release_manifest(fixture: &ReleasePrebuiltFixture, from: &str, to: &str) -> String {
    let text = fs::read_to_string(&fixture.manifest).expect("read release manifest");
    let updated = text.replacen(from, to, 1);
    assert_ne!(updated, text, "fixture manifest replacement must apply");
    set_mode(&fixture.manifest, 0o600);
    fs::write(&fixture.manifest, &updated).expect("rewrite release manifest");
    set_mode(&fixture.manifest, RELEASE_MANIFEST_MODE);
    lowercase_hex(&sha256(updated.as_bytes()))
}
#[test]
fn release_prebuilt_manifest_and_all_program_paths_validate_exactly() {
    let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
    let fixture = create_release_prebuilt_fixture();
    let _env = release_prebuilt_env(&fixture, &fixture.manifest_sha256);
    let contract = release_program_contract(&fixture.repo)
        .expect("validate release contract")
        .expect("release contract active");
    for kind in ReleasePrebuiltBinary::ALL {
        let expected = fixture.target.join(kind.relative_path());
        assert_eq!(
            validate_release_program_candidate(&contract, kind, &expected)
                .expect("validate exact release candidate"),
            expected.canonicalize().expect("canonical candidate")
        );
    }
    let escaped = fixture.repo.join("escaped");
    fs::write(&escaped, b"iroha3d release executable\n").expect("write escaped candidate");
    set_mode(&escaped, RELEASE_BINARY_MODE);
    assert!(
        validate_release_program_candidate(&contract, ReleasePrebuiltBinary::Irohad, escaped)
            .is_err()
    );
}
#[test]
fn release_prebuilt_manifest_rejects_forged_external_digest() {
    let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
    let fixture = create_release_prebuilt_fixture();
    let _env = release_prebuilt_env(&fixture, &"f".repeat(64));
    let err =
        release_program_contract(&fixture.repo).expect_err("forged manifest digest must fail");
    assert!(err.to_string().contains("manifest digest"));
}
#[test]
fn release_source_contract_requires_inherited_prebuilt_manifest_digest() {
    let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
    let fixture = create_release_prebuilt_fixture();
    let _source = EnvVarRestore::set(
        IROHA_RELEASE_SOURCE_MANIFEST_SHA256_ENV,
        &fixture.source_manifest_sha256,
    );
    let _prebuilt = EnvVarGuard::cleared(IROHA_RELEASE_PREBUILT_MANIFEST_SHA256_ENV);
    let err = release_program_contract(&fixture.repo)
        .expect_err("source-bound release contract requires manifest anchor");
    assert!(
        err.to_string()
            .contains(IROHA_RELEASE_PREBUILT_MANIFEST_SHA256_ENV)
    );
}
#[test]
fn release_prebuilt_manifest_rejects_wrong_binary_path_and_hash() {
    let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
    let path_fixture = create_release_prebuilt_fixture();
    let path_manifest_sha256 = rewrite_release_manifest(
        &path_fixture,
        "irohad_relative_path\trelease/iroha3d",
        "irohad_relative_path\trelease/not-iroha3d",
    );
    {
        let _env = release_prebuilt_env(&path_fixture, &path_manifest_sha256);
        assert!(
            release_program_contract(&path_fixture.repo).is_err(),
            "non-exact binary path must fail manifest parsing"
        );
    }
    let hash_fixture = create_release_prebuilt_fixture();
    let original = fs::read_to_string(&hash_fixture.manifest).expect("read manifest");
    let irohad_hash = original
        .lines()
        .find_map(|line| line.strip_prefix("irohad_sha256\t"))
        .expect("irohad digest field");
    let hash_manifest_sha256 = rewrite_release_manifest(
        &hash_fixture,
        &format!("irohad_sha256\t{irohad_hash}"),
        &format!("irohad_sha256\t{}", "0".repeat(64)),
    );
    let _env = release_prebuilt_env(&hash_fixture, &hash_manifest_sha256);
    let contract = release_program_contract(&hash_fixture.repo)
        .expect("parse hash-forged manifest")
        .expect("release contract active");
    assert!(
        validate_release_program_candidate(
            &contract,
            ReleasePrebuiltBinary::Irohad,
            hash_fixture.target.join("release/iroha3d")
        )
        .is_err(),
        "forged binary digest must fail independent hashing"
    );
}
#[cfg(unix)]
#[test]
fn release_prebuilt_binary_rejects_symlink() {
    use std::os::unix::fs::symlink;
    let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
    let fixture = create_release_prebuilt_fixture();
    let _env = release_prebuilt_env(&fixture, &fixture.manifest_sha256);
    let contract = release_program_contract(&fixture.repo)
        .expect("parse release manifest")
        .expect("release contract active");
    let binary = fixture.target.join("release/iroha3d");
    let replacement = fixture.repo.join("replacement-iroha3d");
    fs::write(&replacement, b"iroha3d release executable\n").expect("write replacement");
    set_mode(&replacement, RELEASE_BINARY_MODE);
    let parent = binary.parent().expect("binary parent");
    set_mode(parent, 0o700);
    fs::remove_file(&binary).expect("remove original binary");
    symlink(&replacement, &binary).expect("install symlink");
    set_mode(parent, RELEASE_BINARY_MODE);
    assert!(
        validate_release_program_candidate(&contract, ReleasePrebuiltBinary::Irohad, &binary)
            .is_err()
    );
}
#[test]
fn release_prebuilt_binary_revalidates_mutation_after_initial_resolution() {
    let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
    let fixture = create_release_prebuilt_fixture();
    let _env = release_prebuilt_env(&fixture, &fixture.manifest_sha256);
    let contract = release_program_contract(&fixture.repo)
        .expect("parse release manifest")
        .expect("release contract active");
    let binary = fixture.target.join("release/iroha3d");
    validate_release_program_candidate(&contract, ReleasePrebuiltBinary::Irohad, &binary)
        .expect("initial resolution");
    set_mode(&binary, 0o700);
    fs::write(&binary, b"mutated release executable\n").expect("mutate cached binary");
    set_mode(&binary, RELEASE_BINARY_MODE);
    assert!(
        validate_release_program_candidate(&contract, ReleasePrebuiltBinary::Irohad, &binary)
            .is_err(),
        "fresh verification must reject mutation after an earlier cached resolution"
    );
}
#[cfg(unix)]
#[test]
fn release_prebuilt_binary_rejects_mode_drift_and_hard_links() {
    let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
    let mode_fixture = create_release_prebuilt_fixture();
    let binary = mode_fixture.target.join("release/iroha3d");
    set_mode(&binary, 0o700);
    {
        let _env = release_prebuilt_env(&mode_fixture, &mode_fixture.manifest_sha256);
        let contract = release_program_contract(&mode_fixture.repo)
            .expect("parse release manifest")
            .expect("release contract active");
        assert!(
            validate_release_program_candidate(&contract, ReleasePrebuiltBinary::Irohad, &binary)
                .is_err(),
            "mode drift must fail"
        );
    }
    let link_fixture = create_release_prebuilt_fixture();
    let binary = link_fixture.target.join("release/iroha3d");
    let extra_link = link_fixture.repo.join("iroha3d-hard-link");
    fs::hard_link(&binary, &extra_link).expect("create adversarial hard link");
    let _env = release_prebuilt_env(&link_fixture, &link_fixture.manifest_sha256);
    let contract = release_program_contract(&link_fixture.repo)
        .expect("parse release manifest")
        .expect("release contract active");
    assert!(
        validate_release_program_candidate(&contract, ReleasePrebuiltBinary::Irohad, &binary)
            .is_err(),
        "multiple hard links must fail"
    );
}
#[test]
fn default_build_profile_respects_env_override() {
    let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
    let _clear_profile = EnvVarGuard::cleared("PROFILE");
    let _clear_override = EnvVarGuard::cleared(IROHA_TEST_BUILD_PROFILE_ENV);
    let default_profile = default_build_profile();
    assert_eq!(
        default_profile,
        current_exe_profile_hint().unwrap_or_else(|| "release".to_string())
    );
    let _override_guard = EnvVarRestore::set("PROFILE", "release");
    assert_eq!(default_build_profile(), "release");
    let _override_guard = EnvVarRestore::set(IROHA_TEST_BUILD_PROFILE_ENV, "debug");
    assert_eq!(default_build_profile(), "debug");
}
#[test]
fn profile_hint_from_exe_path_detects_profile_before_deps_dir() {
    let hint = profile_hint_from_exe_path(Path::new(
        "/tmp/iroha-target/debug/deps/integration_tests-abcdef",
    ));
    assert_eq!(hint.as_deref(), Some("debug"));
}
#[test]
fn profile_hint_from_exe_path_detects_non_deps_profile_dir() {
    let hint = profile_hint_from_exe_path(Path::new("/tmp/iroha-target/ci/iroha3d"));
    assert_eq!(hint.as_deref(), Some("ci"));
}
#[test]
fn build_stamp_version_does_not_wrap_to_supported_u32() {
    let temp = tempdir().expect("temporary directory");
    let stamp_path = temp.path().join("stamp.json");
    let wrapped_version = u64::from(BUILD_STAMP_VERSION) + (1_u64 << u32::BITS);
    fs::write(
        &stamp_path,
        format!(
            r#"{{"version":{wrapped_version},"fingerprint":7,"profile":"debug","binary":"iroha3d"}}"#
        ),
    )
    .expect("write wrapped-version stamp");
    assert!(
        read_build_stamp(&stamp_path)
            .expect("read wrapped-version stamp")
            .is_none(),
        "a u64 version that truncates to the supported u32 must be rejected"
    );
    fs::write(
        &stamp_path,
        format!(
            r#"{{"version":{BUILD_STAMP_VERSION},"fingerprint":7,"profile":"debug","binary":"iroha3d"}}"#
        ),
    )
    .expect("write supported-version stamp");
    let stamp = read_build_stamp(&stamp_path)
        .expect("read supported-version stamp")
        .expect("supported version should load");
    assert_eq!(stamp.fingerprint, 7);
    assert_eq!(stamp.profile, "debug");
    assert_eq!(stamp.binary, PathBuf::from("iroha3d"));
}
#[cfg(unix)]
#[test]
fn ensure_binary_fresh_skips_redundant_builds() {
    let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
    let temp = tempdir().expect("temporary workspace");
    let root = temp.path();
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"member\"]\n",
    )
    .expect("write manifest");
    fs::create_dir_all(root.join("member/src")).expect("create member src directory");
    fs::write(root.join("member/src/lib.rs"), b"pub fn greet() {}\n").expect("write source file");
    let target_dir = root.join("target");
    fs::create_dir_all(target_dir.join("debug")).expect("create target debug directory");
    let binary_path = target_dir.join("debug/dummy");
    fs::write(&binary_path, b"binary").expect("create dummy binary");
    let script = root.join("fake-cargo.sh");
    let script_contents = r#"#!/bin/sh
set -eu
if [ -n "${TEST_NETWORK_CARGO_LOG:-}" ]; then
  printf '%s\n' "$*" >> "${TEST_NETWORK_CARGO_LOG}"
fi
exit 0
"#;
    fs::write(&script, script_contents).expect("write fake cargo script");
    fs::set_permissions(&script, PermissionsExt::from_mode(0o755)).expect("make script executable");
    let log_path = root.join("build.log");
    struct EnvRestore {
        key: &'static str,
        previous: Option<String>,
    }
    impl EnvRestore {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = env::var(key).ok();
            unsafe { std::env::set_var(key, value) };
            Self { key, previous }
        }
    }
    impl Drop for EnvRestore {
        fn drop(&mut self) {
            if let Some(ref value) = self.previous {
                unsafe { std::env::set_var(self.key, value) };
            } else {
                unsafe { std::env::remove_var(self.key) };
            }
        }
    }
    let script_env = script.to_string_lossy().into_owned();
    let log_env = log_path.to_string_lossy().into_owned();
    let _cargo_guard = EnvRestore::set("TEST_NETWORK_CARGO", &script_env);
    let _log_guard = EnvRestore::set("TEST_NETWORK_CARGO_LOG", &log_env);
    ensure_binary_fresh(
        root,
        "dummy_pkg",
        "dummy",
        &target_dir,
        "debug",
        &binary_path,
        true,
        &[],
    )
    .expect("initial build invocation");
    let first_log = fs::read_to_string(&log_path).expect("read build log after first run");
    assert!(
        !first_log.is_empty(),
        "fake cargo script should log its invocation"
    );
    assert_eq!(
        first_log.trim(),
        "build --locked --offline -p dummy_pkg",
        "test-network child builds must preserve the workspace lockfile and stay offline"
    );
    ensure_binary_fresh(
        root,
        "dummy_pkg",
        "dummy",
        &target_dir,
        "debug",
        &binary_path,
        true,
        &[],
    )
    .expect("second build invocation should be skipped");
    let second_log = fs::read_to_string(&log_path).expect("read build log after second run");
    assert_eq!(
        first_log.lines().count(),
        second_log.lines().count(),
        "second resolve should not trigger an extra cargo invocation"
    );
}
#[cfg(unix)]
#[test]
fn ensure_binary_fresh_retries_after_e0460_by_cleaning_target_dir() {
    let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
    let temp = tempdir().expect("temporary workspace");
    let root = temp.path();
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"member\"]\n",
    )
    .expect("write manifest");
    fs::create_dir_all(root.join("member/src")).expect("create member src directory");
    fs::write(root.join("member/src/lib.rs"), b"pub fn greet() {}\n").expect("write source file");
    let target_dir = root.join("target");
    let binary_path = target_dir.join("debug/dummy");
    let stale_path = target_dir.join("debug/deps/stale");
    fs::create_dir_all(stale_path.parent().expect("stale deps dir"))
        .expect("create stale deps directory");
    fs::write(&stale_path, b"stale").expect("write stale artifact");
    let script = root.join("fake-cargo-e0460.sh");
    let script_contents = r#"#!/bin/sh
set -eu
count_file="${TEST_NETWORK_CARGO_COUNT_FILE:?}"
bin_path="${TEST_NETWORK_DUMMY_BIN:?}"

count=0
if [ -f "$count_file" ]; then
  count="$(cat "$count_file")"
fi
count=$((count+1))
echo "$count" > "$count_file"

if [ "$count" -eq 1 ]; then
  echo "error[E0460]: found possibly newer version of crate \`norito\`" 1>&2
  echo "For more information about this error, try rustc --explain E0460." 1>&2
  exit 101
fi

mkdir -p "$(dirname "$bin_path")"
printf '%s\n' "binary" > "$bin_path"
exit 0
"#;
    fs::write(&script, script_contents).expect("write fake cargo script");
    fs::set_permissions(&script, PermissionsExt::from_mode(0o755)).expect("make script executable");
    let count_path = root.join("build-count.txt");
    struct EnvRestore {
        key: &'static str,
        previous: Option<String>,
    }
    impl EnvRestore {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = env::var(key).ok();
            unsafe { std::env::set_var(key, value) };
            Self { key, previous }
        }
    }
    impl Drop for EnvRestore {
        fn drop(&mut self) {
            if let Some(ref value) = self.previous {
                unsafe { std::env::set_var(self.key, value) };
            } else {
                unsafe { std::env::remove_var(self.key) };
            }
        }
    }
    let script_env = script.to_string_lossy().into_owned();
    let count_env = count_path.to_string_lossy().into_owned();
    let bin_env = binary_path.to_string_lossy().into_owned();
    let _cargo_guard = EnvRestore::set("TEST_NETWORK_CARGO", &script_env);
    let _count_guard = EnvRestore::set("TEST_NETWORK_CARGO_COUNT_FILE", &count_env);
    let _bin_guard = EnvRestore::set("TEST_NETWORK_DUMMY_BIN", &bin_env);
    ensure_binary_fresh(
        root,
        "dummy_pkg",
        "dummy",
        &target_dir,
        "debug",
        &binary_path,
        true,
        &[],
    )
    .expect("retry build after E0460 should succeed");
    assert!(binary_path.exists(), "dummy binary should be created");
    assert!(
        !stale_path.exists(),
        "cleanup should remove stale build artifacts"
    );
    let count: u32 = fs::read_to_string(&count_path)
        .expect("read build count")
        .trim()
        .parse()
        .expect("parse build count");
    assert_eq!(
        count, 2,
        "fake cargo should be invoked twice (fail then retry)"
    );
}
#[cfg(unix)]
#[test]
fn ensure_binary_fresh_retries_after_e0463_by_cleaning_target_dir() {
    let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
    let temp = tempdir().expect("temporary workspace");
    let root = temp.path();
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"member\"]\n",
    )
    .expect("write manifest");
    fs::create_dir_all(root.join("member/src")).expect("create member src directory");
    fs::write(root.join("member/src/lib.rs"), b"pub fn greet() {}\n").expect("write source file");
    let target_dir = root.join("target");
    let binary_path = target_dir.join("debug/dummy");
    let stale_path = target_dir.join("release/deps/stale");
    fs::create_dir_all(stale_path.parent().expect("stale deps dir"))
        .expect("create stale deps directory");
    fs::write(&stale_path, b"stale").expect("write stale artifact");
    let script = root.join("fake-cargo-e0463.sh");
    let script_contents = r#"#!/bin/sh
set -eu
count_file="${TEST_NETWORK_CARGO_COUNT_FILE:?}"
bin_path="${TEST_NETWORK_DUMMY_BIN:?}"

count=0
if [ -f "$count_file" ]; then
  count="$(cat "$count_file")"
fi
count=$((count+1))
echo "$count" > "$count_file"

if [ "$count" -eq 1 ]; then
  echo "error[E0463]: can't find crate for \`norito\`" 1>&2
  echo "For more information about this error, try \`rustc --explain E0463\`." 1>&2
  exit 101
fi

mkdir -p "$(dirname "$bin_path")"
printf '%s\n' "binary" > "$bin_path"
exit 0
"#;
    fs::write(&script, script_contents).expect("write fake cargo script");
    fs::set_permissions(&script, PermissionsExt::from_mode(0o755)).expect("make script executable");
    let count_path = root.join("build-count.txt");
    struct EnvRestore {
        key: &'static str,
        previous: Option<String>,
    }
    impl EnvRestore {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = env::var(key).ok();
            unsafe { std::env::set_var(key, value) };
            Self { key, previous }
        }
    }
    impl Drop for EnvRestore {
        fn drop(&mut self) {
            if let Some(ref value) = self.previous {
                unsafe { std::env::set_var(self.key, value) };
            } else {
                unsafe { std::env::remove_var(self.key) };
            }
        }
    }
    let script_env = script.to_string_lossy().into_owned();
    let count_env = count_path.to_string_lossy().into_owned();
    let bin_env = binary_path.to_string_lossy().into_owned();
    let _cargo_guard = EnvRestore::set("TEST_NETWORK_CARGO", &script_env);
    let _count_guard = EnvRestore::set("TEST_NETWORK_CARGO_COUNT_FILE", &count_env);
    let _bin_guard = EnvRestore::set("TEST_NETWORK_DUMMY_BIN", &bin_env);
    ensure_binary_fresh(
        root,
        "dummy_pkg",
        "dummy",
        &target_dir,
        "debug",
        &binary_path,
        true,
        &[],
    )
    .expect("retry build after E0463 should succeed");
    assert!(binary_path.exists(), "dummy binary should be created");
    assert!(
        !stale_path.exists(),
        "cleanup should remove stale build artifacts"
    );
    let count: u32 = fs::read_to_string(&count_path)
        .expect("read build count")
        .trim()
        .parse()
        .expect("parse build count");
    assert_eq!(
        count, 2,
        "fake cargo should be invoked twice (fail then retry)"
    );
}
#[test]
fn ensure_binary_fresh_refuses_forbidden_child_builds() {
    let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
    let temp = tempdir().expect("temporary workspace");
    let root = temp.path();
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"member\"]\n",
    )
    .expect("write manifest");
    fs::create_dir_all(root.join("member/src")).expect("create member src directory");
    fs::write(root.join("member/src/lib.rs"), b"pub fn greet() {}\n").expect("write source file");
    let target_dir = root.join("target");
    let binary_path = target_dir.join("debug/dummy");
    let err = ensure_binary_fresh(
        root,
        "dummy_pkg",
        "dummy",
        &target_dir,
        "debug",
        &binary_path,
        false,
        &[],
    )
    .expect_err("child build should be rejected when building is disallowed");
    assert!(
        err.to_string().contains("cannot build `dummy`"),
        "unexpected error: {err}"
    );
    assert_eq!(
        cargo_or_rustc_processes(
            b"  PID ELAPSED COMMAND\n  17 00:02 /opt/rust/bin/cargo test\n  18 00:01 \
              /opt/rust/bin/rustc --crate-name demo\n  19 00:01 /bin/sh build.sh\n"
        ),
        vec![
            "pid=17,etime=00:02,program=/opt/rust/bin/cargo".to_owned(),
            "pid=18,etime=00:01,program=/opt/rust/bin/rustc".to_owned(),
        ]
    );
}
#[test]
fn first_existing_candidate_prefers_earlier_existing_path() {
    let temp = tempdir().expect("temporary workspace");
    let primary = temp.path().join("primary-bin");
    let fallback = temp.path().join("fallback-bin");
    fs::write(&primary, b"primary").expect("write primary candidate");
    fs::write(&fallback, b"fallback").expect("write fallback candidate");
    let resolved = first_existing_candidate([
        Cow::Borrowed(primary.as_path()),
        Cow::Borrowed(fallback.as_path()),
    ])
    .expect("first existing candidate should resolve");
    assert_eq!(resolved, primary.canonicalize().expect("canonical primary"));
}
#[test]
fn first_existing_candidate_skips_missing_paths() {
    let temp = tempdir().expect("temporary workspace");
    let missing = temp.path().join("missing-bin");
    let fallback = temp.path().join("fallback-bin");
    fs::write(&fallback, b"fallback").expect("write fallback candidate");
    let resolved = first_existing_candidate([
        Cow::Borrowed(missing.as_path()),
        Cow::Borrowed(fallback.as_path()),
    ])
    .expect("fallback candidate should resolve");
    assert_eq!(
        resolved,
        fallback.canonicalize().expect("canonical fallback")
    );
}
#[test]
fn colocated_binary_candidate_for_resolves_sibling_binary() {
    let temp = tempdir().expect("temporary workspace");
    let current_exe = temp.path().join("release/izanami");
    let sibling = temp.path().join("release/iroha3d");
    fs::create_dir_all(current_exe.parent().expect("current exe parent"))
        .expect("create release dir");
    fs::write(&current_exe, b"izanami").expect("write current exe");
    fs::write(&sibling, b"iroha3d").expect("write sibling binary");
    let resolved = colocated_binary_candidate_for(&current_exe, "iroha3d")
        .expect("sibling binary should resolve");
    assert_eq!(resolved, sibling.canonicalize().expect("canonical sibling"));
}
#[test]
fn colocated_binary_candidate_for_ignores_missing_sibling_binary() {
    let temp = tempdir().expect("temporary workspace");
    let current_exe = temp.path().join("release/izanami");
    fs::create_dir_all(current_exe.parent().expect("current exe parent"))
        .expect("create release dir");
    fs::write(&current_exe, b"izanami").expect("write current exe");
    assert!(
        colocated_binary_candidate_for(&current_exe, "iroha3d").is_none(),
        "missing sibling binary should not resolve"
    );
}
#[test]
fn child_builds_are_forbidden_under_cargo_and_in_release_corridors() {
    assert!(!child_build_allowed(true, false));
    assert!(child_build_allowed(false, false));
    assert!(!child_build_allowed(true, true));
    assert!(!child_build_allowed(false, true));
}
#[test]
fn freshness_validation_runs_only_when_building_is_allowed() {
    assert!(must_validate_binary_freshness(false, true));
    assert!(!must_validate_binary_freshness(true, true));
    assert!(!must_validate_binary_freshness(false, false));
    assert!(!must_validate_binary_freshness(true, false));
}
