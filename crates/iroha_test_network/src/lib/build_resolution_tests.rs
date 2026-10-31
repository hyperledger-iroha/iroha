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
    let _clear_lock = EnvVarGuard::cleared(IROHA_RELEASE_CARGO_LOCK_SHA256_ENV);
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
// Canonical publication fixtures shared by the resolver's environment/cache controls.
struct ReleasePrebuiltFixture {
    _temp: tempfile::TempDir,
    repo: PathBuf,
    artifact_root: PathBuf,
    source_manifest_sha256: String,
    cargo_lock_sha256: String,
    target: PathBuf,
    manifest: PathBuf,
    manifest_sha256: String,
}
impl Drop for ReleasePrebuiltFixture {
    fn drop(&mut self) {
        set_fixture_access(&self.target, "F");
    }
}
#[cfg(unix)]
fn set_fixture_access(path: &Path, access: &str) {
    use std::os::unix::fs::PermissionsExt as _;
    let mode = match access {
        "F" => 0o700,
        "RX" => 0o500,
        "R" => 0o400,
        _ => panic!("unknown fixture policy"),
    };
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}
#[cfg(windows)]
fn set_fixture_access(path: &Path, access: &str) {
    static SID: OnceLock<String> = OnceLock::new();
    let sid = SID.get_or_init(|| {
        let output = std::process::Command::new("whoami")
            .args(["/user", "/fo", "csv", "/nh"])
            .output()
            .unwrap();
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        let sid = text.rsplit(',').next().unwrap().trim().trim_matches('"');
        assert!(
            sid.starts_with("S-1-")
                && sid
                    .bytes()
                    .all(|b| b.is_ascii_digit() || matches!(b, b'S' | b'-'))
        );
        sid.to_owned()
    });
    // Remove the old ACE first: /grant:r alone can retain an inheritable owner ACE.
    for args in [
        ["/remove:g".to_owned(), format!("*{sid}")],
        ["/grant:r".to_owned(), format!("*{sid}:{access}")],
    ] {
        let output = std::process::Command::new("icacls")
            .arg(path)
            .args(args)
            .arg("/Q")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
fn create_release_prebuilt_fixture() -> ReleasePrebuiltFixture {
    use iroha_fs::{OwnerDirectory, PublishMode};
    use iroha_release_bundle::{
        bundle::NATIVE_TARGET,
        manifest::MANIFEST_NAME,
        publish::{PublishInputs, publish},
    };
    let temp = tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let repo = root.join("repo");
    let _source = OwnerDirectory::open_or_create(&repo).unwrap();
    let artifact_root = root.join("artifacts");
    let artifact = OwnerDirectory::open_or_create(&artifact_root).unwrap();
    let source_manifest_sha256 = "a".repeat(64);
    let lock = repo.join("Cargo.lock");
    fs::write(&lock, b"release-lock-v1\n").unwrap();
    let programs = artifact
        .ensure_child("sumeragi-release")
        .unwrap()
        .ensure_child(&source_manifest_sha256)
        .unwrap()
        .ensure_child("programs")
        .unwrap();
    let build = OwnerDirectory::open_or_create(root.join("build")).unwrap();
    for kind in BinaryKind::ALL {
        fs::write(
            build.path().join(kind.filename(cfg!(windows))),
            crate::native_program_fixture::image(),
        )
        .unwrap();
    }
    let tools = OwnerDirectory::open_or_create(root.join("tools")).unwrap();
    tools
        .write_atomic("cargo", b"cargo 1.93.1 (fixture)\n", PublishMode::CreateNew)
        .unwrap();
    tools
        .write_atomic(
            "rustc",
            format!("rustc 1.93.1 (fixture)\nhost: {NATIVE_TARGET}\n").as_bytes(),
            PublishMode::CreateNew,
        )
        .unwrap();
    let bundle = publish(
        &programs,
        "invocation.A1b2C3",
        PublishInputs {
            source_sha256: &source_manifest_sha256,
            build_directory: build.path(),
            cargo_lock: &lock,
            cargo_version: &tools.path().join("cargo"),
            rustc_version: &tools.path().join("rustc"),
        },
    )
    .unwrap();
    let target = bundle.path().to_owned();
    let manifest = target.join(MANIFEST_NAME);
    let cargo_lock_sha256 = bundle.manifest().cargo_lock_sha256.clone();
    let manifest_sha256 = bundle.manifest_sha256().to_owned();
    drop(bundle);
    ReleasePrebuiltFixture {
        _temp: temp,
        repo,
        artifact_root,
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
        EnvVarRestore::set(IROHA_RELEASE_ARTIFACT_ROOT_ENV, &fixture.artifact_root),
        EnvVarRestore::set(IROHA_TEST_TARGET_DIR_ENV, fixture.target.as_os_str()),
        EnvVarRestore::set(IROHA_TEST_SKIP_BUILD_ENV, "1"),
        EnvVarRestore::set(IROHA_TEST_BUILD_PROFILE_ENV, "release"),
        EnvVarRestore::set("PROFILE", "release"),
    ]
}
fn rewrite_release_manifest(
    fixture: &ReleasePrebuiltFixture,
    mutate: impl FnOnce(&mut iroha_release_bundle::manifest::Manifest),
) -> String {
    let source = fs::read(&fixture.manifest).unwrap();
    let mut manifest: iroha_release_bundle::manifest::Manifest =
        norito::json::from_slice(&source).unwrap();
    mutate(&mut manifest);
    let mut bytes = norito::json::to_vec(&manifest).unwrap();
    bytes.push(b'\n');
    let before = iroha_fs::FileMetadata::symlink_metadata(&fixture.manifest)
        .unwrap()
        .identity();
    set_fixture_access(&fixture.manifest, "F");
    fs::write(&fixture.manifest, &bytes).unwrap();
    set_fixture_access(&fixture.manifest, "R");
    assert_eq!(
        before,
        iroha_fs::FileMetadata::symlink_metadata(&fixture.manifest)
            .unwrap()
            .identity()
    );
    lowercase_hex(&sha256(&bytes))
}
#[test]
fn release_prebuilt_manifest_and_all_program_paths_validate_exactly() {
    let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
    let fixture = create_release_prebuilt_fixture();
    let _env = release_prebuilt_env(&fixture, &fixture.manifest_sha256);
    let contract = release_program_contract(&fixture.repo).unwrap().unwrap();
    for kind in BinaryKind::ALL {
        let expected = fixture.target.join(kind.filename(cfg!(windows)));
        let selected = contract.select(kind, Some(&expected)).unwrap();
        assert_eq!(
            selected.diagnostic_path().unwrap().as_os_str(),
            expected.as_os_str()
        );
        assert!(selected.is_release());
    }
    let escaped = fixture.repo.join("escaped");
    fs::write(&escaped, b"iroha3d release fixture\n").unwrap();
    assert!(contract.select(BinaryKind::Irohad, Some(&escaped)).is_err());
}
#[test]
fn release_prebuilt_requires_exact_external_artifact_root() {
    let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
    let fixture = create_release_prebuilt_fixture();
    let _env = release_prebuilt_env(&fixture, &fixture.manifest_sha256);
    {
        let _missing = EnvVarGuard::cleared(IROHA_RELEASE_ARTIFACT_ROOT_ENV);
        assert!(release_program_contract(&fixture.repo).is_err());
    }
    {
        let _relative = EnvVarRestore::set(IROHA_RELEASE_ARTIFACT_ROOT_ENV, "artifacts");
        assert!(release_program_contract(&fixture.repo).is_err());
    }
    let other = fixture
        .artifact_root
        .parent()
        .unwrap()
        .join("other-artifacts");
    let _owner = iroha_fs::OwnerDirectory::open_or_create(&other).unwrap();
    let _different = EnvVarRestore::set(IROHA_RELEASE_ARTIFACT_ROOT_ENV, &other);
    assert!(release_program_contract(&fixture.repo).is_err());
}
#[test]
fn release_prebuilt_manifest_rejects_forged_external_digest() {
    let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
    let fixture = create_release_prebuilt_fixture();
    let _env = release_prebuilt_env(&fixture, &"f".repeat(64));
    assert!(release_program_contract(&fixture.repo).is_err());
}
#[test]
fn release_source_contract_requires_inherited_prebuilt_manifest_digest() {
    let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
    let fixture = create_release_prebuilt_fixture();
    let _source = EnvVarRestore::set(
        IROHA_RELEASE_SOURCE_MANIFEST_SHA256_ENV,
        &fixture.source_manifest_sha256,
    );
    let _manifest = EnvVarGuard::cleared(IROHA_RELEASE_PREBUILT_MANIFEST_SHA256_ENV);
    assert!(
        release_program_contract(&fixture.repo)
            .unwrap_err()
            .to_string()
            .contains(IROHA_RELEASE_PREBUILT_MANIFEST_SHA256_ENV)
    );
}
#[test]
fn release_prebuilt_manifest_rejects_wrong_binary_path_and_hash() {
    let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
    for wrong_path in [true, false] {
        let fixture = create_release_prebuilt_fixture();
        let anchor = rewrite_release_manifest(&fixture, |manifest| {
            if wrong_path {
                manifest.binaries[0].filename = "wrong-native-name".to_owned();
            } else {
                manifest.binaries[0].sha256 = "0".repeat(64);
            }
        });
        let _env = release_prebuilt_env(&fixture, &anchor);
        assert!(
            release_program_contract(&fixture.repo).is_err(),
            "reanchored path/hash forgery must fail"
        );
    }
}
#[cfg(unix)]
#[test]
fn release_prebuilt_binary_revalidates_mutation_after_initial_resolution() {
    let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
    let fixture = create_release_prebuilt_fixture();
    let _env = release_prebuilt_env(&fixture, &fixture.manifest_sha256);
    let contract = release_program_contract(&fixture.repo).unwrap().unwrap();
    let selected = contract.select(BinaryKind::Irohad, None).unwrap();
    assert!(selected.command().is_ok());
    let binary = fixture.target.join(BinaryKind::Irohad.filename(false));
    set_fixture_access(&binary, "F");
    fs::write(&binary, b"mutated release executable\n").unwrap();
    set_fixture_access(&binary, "RX");
    assert!(selected.command().is_err());
}
#[test]
fn release_prebuilt_cached_selection_rejects_native_policy_drift() {
    let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
    let fixture = create_release_prebuilt_fixture();
    let _env = release_prebuilt_env(&fixture, &fixture.manifest_sha256);
    let contract = release_program_contract(&fixture.repo).unwrap().unwrap();
    let selected = contract.select(BinaryKind::Irohad, None).unwrap();
    assert!(selected.command().is_ok());
    drop(contract);
    set_fixture_access(
        &fixture
            .target
            .join(BinaryKind::Irohad.filename(cfg!(windows))),
        "F",
    );
    assert!(selected.command().is_err());
}
#[test]
fn release_prebuilt_resolution_rejects_feature_isolation_force_build_and_ambient_override() {
    let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
    let fixture = create_release_prebuilt_fixture();
    let _env = release_prebuilt_env(&fixture, &fixture.manifest_sha256);
    for program in [
        Program::IrohadPrivateSettlementRoutes,
        Program::IrohadDisposableBroker,
        Program::IrohadParliamentSigners,
    ] {
        assert!(
            program
                .resolve_internal_in_repo(Some(true), &fixture.repo)
                .unwrap_err()
                .to_string()
                .contains("feature-isolated")
        );
    }
    assert!(
        Program::Iroha
            .resolve_internal_in_repo(Some(false), &fixture.repo)
            .is_err()
    );
    let _override = EnvVarRestore::set(PROGRAM_IROHA_ENV, env::current_exe().unwrap());
    assert!(
        Program::Iroha
            .resolve_internal_in_repo(Some(true), &fixture.repo)
            .is_err()
    );
    let _clear = EnvVarGuard::cleared(PROGRAM_IROHA_ENV);
    assert!(
        Program::Iroha
            .resolve_internal_in_repo(Some(true), &fixture.repo)
            .unwrap()
            .is_release()
    );
}
#[test]
fn release_prebuilt_resolution_admits_before_developer_cache() {
    // Isolate the OnceLock mutation from every other resolver test in this process.
    const CHILD: &str = "IROHA_TEST_RELEASE_CACHE_CONTROL";
    if env::var_os(CHILD).is_none() {
        let mut command = development_process_command(env::current_exe().unwrap()).unwrap();
        command
            .args([
                "--exact",
                "tests::release_prebuilt_resolution_admits_before_developer_cache",
                "--nocapture",
            ])
            .env(CHILD, "1");
        let output = command_output_with_timeout(command, Duration::from_secs(60)).unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        return;
    }
    let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
    let fixture = create_release_prebuilt_fixture();
    let _env = release_prebuilt_env(&fixture, &fixture.manifest_sha256);
    let _clear = EnvVarGuard::cleared(PROGRAM_IROHA_ENV);
    let _ = IROHA_BIN.set(env::current_exe().unwrap());
    let selected = Program::Iroha
        .resolve_internal_in_repo(Some(true), &fixture.repo)
        .unwrap();
    assert!(selected.is_release());
    assert_eq!(
        selected.diagnostic_path().unwrap(),
        fixture
            .target
            .join(BinaryKind::Iroha.filename(cfg!(windows)))
    );
    let _wrong = EnvVarRestore::set(IROHA_RELEASE_PREBUILT_MANIFEST_SHA256_ENV, "f".repeat(64));
    assert!(
        Program::Iroha
            .resolve_internal_in_repo(Some(true), &fixture.repo)
            .is_err()
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

#[test]
fn release_prebuilt_taira_launcher_is_mandatory_and_separately_bound() {
    let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
    for substitute in [false, true] {
        let fixture = create_release_prebuilt_fixture();
        let _valid = release_prebuilt_env(&fixture, &fixture.manifest_sha256);
        let contract = release_program_contract(&fixture.repo).unwrap().unwrap();
        for kind in BinaryKind::ALL {
            assert!(contract.select(kind, None).is_ok());
        }
        drop(contract);
        let anchor = rewrite_release_manifest(&fixture, |manifest| {
            if substitute {
                manifest.binaries[3].filename =
                    BinaryKind::Irohad.filename(cfg!(windows)).to_owned();
            } else {
                manifest.binaries.pop();
            }
        });
        let _changed = EnvVarRestore::set(IROHA_RELEASE_PREBUILT_MANIFEST_SHA256_ENV, anchor);
        assert!(release_program_contract(&fixture.repo).is_err());
    }
    let spec = Program::IrohadTaira.spec();
    assert_eq!(spec.env, "TEST_NETWORK_BIN_IROHAD_TAIRA");
    assert_eq!(spec.name, "iroha3d_taira");
    assert_eq!(
        spec.build_args,
        [OsString::from("--bin"), OsString::from("iroha3d_taira")]
    );
    assert!(Program::IrohadTaira.release_prebuilt_allowed());
}

#[test]
fn developer_tool_selection_runs_an_absolute_native_image_without_release_anchors() {
    let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
    let _source = EnvVarGuard::cleared(IROHA_RELEASE_SOURCE_MANIFEST_SHA256_ENV);
    let _manifest = EnvVarGuard::cleared(IROHA_RELEASE_PREBUILT_MANIFEST_SHA256_ENV);
    let _lock = EnvVarGuard::cleared(IROHA_RELEASE_CARGO_LOCK_SHA256_ENV);
    let fixture = create_release_prebuilt_fixture();
    let tool = fixture
        .target
        .join(BinaryKind::Iroha.filename(cfg!(windows)));
    let output = development_process_command(&tool)
        .unwrap()
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"resolver-probe\n");
}

#[test]
fn developer_tool_selection_refuses_partial_release_anchors_before_path_lookup() {
    let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
    let _source = EnvVarGuard::cleared(IROHA_RELEASE_SOURCE_MANIFEST_SHA256_ENV);
    let _manifest = EnvVarGuard::cleared(IROHA_RELEASE_PREBUILT_MANIFEST_SHA256_ENV);
    let _lock = EnvVarGuard::cleared(IROHA_RELEASE_CARGO_LOCK_SHA256_ENV);
    for (present, required) in [
        (
            IROHA_RELEASE_SOURCE_MANIFEST_SHA256_ENV,
            IROHA_RELEASE_PREBUILT_MANIFEST_SHA256_ENV,
        ),
        (
            IROHA_RELEASE_PREBUILT_MANIFEST_SHA256_ENV,
            IROHA_RELEASE_SOURCE_MANIFEST_SHA256_ENV,
        ),
        (
            IROHA_RELEASE_CARGO_LOCK_SHA256_ENV,
            IROHA_RELEASE_SOURCE_MANIFEST_SHA256_ENV,
        ),
    ] {
        let _partial = EnvVarRestore::set(present, "a".repeat(64));
        let error = development_process_command("").expect_err("release guard must run first");
        assert!(error.to_string().contains(required), "{error}");
        let error = require_development_process_mode().expect_err("optional discovery guard");
        assert!(error.to_string().contains(required), "{error}");
    }
}

#[test]
fn developer_tool_selection_refuses_an_authenticated_release_before_path_lookup() {
    let _guard = lock_env_guard(&PROGRAM_BIN_ENV_GUARD);
    let fixture = create_release_prebuilt_fixture();
    let _environment = release_prebuilt_env(&fixture, &fixture.manifest_sha256);
    let error = crate::development_process::command_in_repo(OsStr::new(""), &fixture.repo)
        .expect_err("an admitted release cannot select a developer tool");
    assert!(
        error
            .to_string()
            .contains("developer tool discovery is forbidden"),
        "{error}"
    );
}
