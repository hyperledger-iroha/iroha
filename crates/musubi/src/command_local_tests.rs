// Local compiler profiles, cold workspace execution, and publication evidence boundaries.
use super::*;

#[test]
fn cold_local_demo_checks_builds_and_tests_without_registry_configuration() {
    let temporary = TempDir::new().expect("local demo directory");
    let root = temporary.path();
    for (name, source) in [
        (
            "Musubi.toml",
            include_str!("../../../examples/coffee-club/Musubi.toml"),
        ),
        (
            "contracts/coffee-club.ko",
            include_str!("../../../examples/coffee-club/contracts/coffee-club.ko"),
        ),
        (
            "tests/coffee-club.test.ko",
            include_str!("../../../examples/coffee-club/tests/coffee-club.test.ko"),
        ),
    ] {
        let path = root.join(name);
        fs::create_dir_all(path.parent().expect("fixture parent")).expect("fixture directory");
        fs::write(path, source).expect("fixture source");
    }
    let manifest = root.join(MANIFEST_FILE_NAME);
    for (command, mode) in [
        ("check", "--offline"),
        ("build", "--frozen"),
        ("test", "--frozen"),
        ("fetch", "--frozen"),
    ] {
        let result = invoke([
            OsString::from("musubi"),
            OsString::from("--manifest-path"),
            manifest.as_os_str().to_owned(),
            OsString::from(command),
            OsString::from(mode),
        ]);
        let rendered = result
            .output
            .render(OutputFormat::Human)
            .expect("render output");
        assert_eq!(rendered.exit_code(), 0, "{command}: {}", rendered.stderr());
        if command == "test" {
            assert!(rendered.stdout().contains("4 passed; 0 failed"));
        }
    }
    let lock = LockfileV1::read(&root.join(LOCK_FILE_NAME)).expect("local lock");
    assert!(matches!(lock.context, LockContextV1::Local { .. }));
    assert!(lock.nodes.is_empty());
    assert!(
        lock.registry_context().is_err(),
        "a local build is not registry evidence"
    );
    let before = fs::read(root.join(LOCK_FILE_NAME)).expect("original lock");
    let output = invoke([
        OsString::from("musubi"),
        OsString::from("--manifest-path"),
        manifest.as_os_str().to_owned(),
        OsString::from("metadata"),
    ])
    .output
    .render(OutputFormat::Json)
    .expect("metadata JSON");
    assert_eq!(output.exit_code(), 0);
    assert!(output.stdout().contains("\"kind\":\"local\""));
    assert!(!output.stdout().contains("network_id"));
    assert_eq!(
        fs::read(root.join(LOCK_FILE_NAME)).expect("unchanged lock"),
        before
    );
}

#[test]
fn cold_local_locked_rejects_missing_lock_and_graph_edits() {
    let temp = TempDir::new().expect("local workspace");
    let (root, manifest) = create_test_package(&temp);
    let call = |command: &str, mode: &str| {
        invoke([
            OsString::from("musubi"),
            OsString::from("--manifest-path"),
            manifest.as_os_str().to_owned(),
            OsString::from(command),
            OsString::from(mode),
        ])
    };
    assert_eq!(
        call("check", "--frozen").output.exit_code(),
        ErrorCode::Locked.exit_code()
    );
    assert!(!root.join(LOCK_FILE_NAME).exists());
    assert_eq!(call("check", "--offline").output.exit_code(), 0);
    let bytes = fs::read(root.join(LOCK_FILE_NAME)).expect("original lock");
    let source = fs::read_to_string(&manifest).expect("manifest");
    fs::write(
        &manifest,
        source.replace("version = \"0.1.0\"", "version = \"0.2.0\""),
    )
    .expect("edit package version");
    assert_eq!(
        call("check", "--locked").output.exit_code(),
        ErrorCode::Locked.exit_code()
    );
    assert_eq!(
        fs::read(root.join(LOCK_FILE_NAME)).expect("retained lock"),
        bytes
    );
    assert_eq!(call("check", "--offline").output.exit_code(), 0);
    assert_ne!(
        fs::read(root.join(LOCK_FILE_NAME)).expect("updated lock"),
        bytes
    );
}

#[test]
fn local_compiler_profile_uses_only_explicit_public_configuration() {
    let temp = TempDir::new().expect("local profile directory");
    let (_, manifest) = create_test_package(&temp);
    let config = temp.path().join("public.toml");
    fs::write(&config, "[account]\nchain_discriminant = 369\n").expect("public-only config");
    for (discriminant, expected) in [(369, 0), (753, ErrorCode::Usage.exit_code())] {
        let output = invoke([
            OsString::from("musubi"),
            OsString::from("--manifest-path"),
            manifest.as_os_str().to_owned(),
            OsString::from("check"),
            OsString::from("--offline"),
            OsString::from("--config"),
            config.as_os_str().to_owned(),
            OsString::from("--chain-discriminant"),
            OsString::from(discriminant.to_string()),
        ])
        .output
        .render(OutputFormat::Human)
        .expect("render output");
        assert_eq!(output.exit_code(), expected, "{}", output.stderr());
    }
    assert!(Cli::try_parse_from(["musubi", "check", "--chain-discriminant", "0"]).is_err());
}

#[test]
fn invalid_local_profile_does_not_create_a_lock() {
    for config_contents in [
        "[account]\nchain_discriminant = 0\n",
        "[account]\nchain_discriminant = 369\n",
    ] {
        let temp = TempDir::new().expect("invalid profile directory");
        let (root, manifest) = create_test_package(&temp);
        let config = temp.path().join("public.toml");
        fs::write(&config, config_contents).expect("public config");
        let output = invoke([
            OsString::from("musubi"),
            OsString::from("--manifest-path"),
            manifest.as_os_str().to_owned(),
            OsString::from("check"),
            OsString::from("--offline"),
            OsString::from("--config"),
            config.as_os_str().to_owned(),
            OsString::from("--chain-discriminant"),
            OsString::from("753"),
        ])
        .output;
        assert_ne!(output.exit_code(), 0);
        assert!(
            !root.join(LOCK_FILE_NAME).exists(),
            "invalid options must not publish a lock"
        );
    }
}

#[test]
fn compiler_profile_selection_requires_an_exact_configured_match() {
    assert_eq!(
        select_compiler_chain_discriminant(753, None, false).unwrap(),
        753
    );
    assert_eq!(
        select_compiler_chain_discriminant(753, Some(369), false).unwrap(),
        369
    );
    assert_eq!(
        select_compiler_chain_discriminant(369, Some(369), true).unwrap(),
        369
    );
    assert_eq!(
        select_compiler_chain_discriminant(753, Some(369), true)
            .unwrap_err()
            .code(),
        ErrorCode::Usage
    );
}

#[test]
fn source_inventory_is_read_only_without_any_lock_or_registry_configuration() {
    let temporary = TempDir::new().expect("inventory project");
    let (root, manifest) = create_test_package(&temporary);
    fs::write(
        root.join(LOCK_FILE_NAME),
        "invalid lock is not inventory authority",
    )
    .expect("irrelevant local lock");
    let before = fs::read(root.join(LOCK_FILE_NAME)).expect("lock bytes");
    let result = invoke([
        OsString::from("musubi"),
        OsString::from("--manifest-path"),
        manifest.as_os_str().to_owned(),
        OsString::from("package"),
        OsString::from("--list"),
        OsString::from("--frozen"),
    ])
    .output
    .render(OutputFormat::Human)
    .expect("source listing");
    assert_eq!(result.exit_code(), 0, "{}", result.stderr());
    assert!(result.stdout().contains("Musubi.toml"));
    assert!(result.stdout().contains("src/lib.ko"));
    assert!(!result.stdout().contains("verification-lock"));
    assert_eq!(
        fs::read(root.join(LOCK_FILE_NAME)).expect("unchanged lock"),
        before
    );
    assert!(!root.join("target").exists());
}

#[test]
fn publication_lock_evidence_does_not_replace_or_authorize_the_workspace_lock() {
    let temporary = TempDir::new().expect("publication project");
    let (root, _) = create_test_package(&temporary);
    let workspace = load_workspace(&root).expect("workspace");
    let selected = vec!["apps.sora/demo".parse().expect("package selector")];
    let local = resolve_workspace_local(&workspace, &selected, None, ResolveModeV1::UpdateLock)
        .expect("local graph")
        .expect("local context")
        .lockfile;
    write_resolved_lock(&workspace, GraphPurposeV1::Workspace, &local).expect("workspace lock");
    let before = fs::read(root.join(LOCK_FILE_NAME)).expect("workspace bytes");
    assert!(
        read_optional_publication_lock(&workspace)
            .expect("no publication evidence")
            .is_none()
    );
    assert!(write_resolved_lock(&workspace, GraphPurposeV1::Publication, &local).is_err());
    assert!(
        !root.join("target").exists(),
        "reject before publication directories are written"
    );
    let publication = LockfileV1::new(
        LockContextV1::Registry {
            network_id: test_network_id(0x37),
            snapshot: MusubiRegistrySnapshotV1 {
                finalized_height: 7,
                finalized_block_hash: [0x17; 32],
                index_revision: 2,
            },
        },
        local.roots.clone(),
        Vec::new(),
    )
    .expect("registry evidence");
    write_resolved_lock(&workspace, GraphPurposeV1::Publication, &publication)
        .expect("separate publication lock");
    assert_eq!(
        fs::read(root.join(LOCK_FILE_NAME)).expect("preserved local lock"),
        before
    );
    assert_eq!(
        read_optional_publication_lock(&workspace).expect("publication evidence"),
        Some(publication)
    );
    resolve_workspace_local(
        &workspace,
        &selected,
        Some(local.clone()),
        ResolveModeV1::Locked,
    )
    .expect("publication does not break frozen local builds");
    fs::write(
        root.join(PUBLICATION_LOCK_PATH),
        local.render().expect("local encoding"),
    )
    .expect("substitute local lock");
    assert!(
        read_optional_publication_lock(&workspace).is_err(),
        "local context never supplies publication evidence"
    );
}
