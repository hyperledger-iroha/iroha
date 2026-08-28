#[test]
#[allow(clippy::bool_assert_comparison)] // for expressiveness
fn default_args() {
    let args = Args::try_parse_from(["test"]).unwrap();
    assert_eq!(args.terminal_colors, is_coloring_supported());
    assert!(!args.startup.check_config);
    assert!(
        args.startup
            .write_kagemusha_catalog_qualification_seal
            .is_none()
    );
    assert!(
        args.startup
            .write_kagemusha_validator_qualification_seal
            .is_none()
    );
    #[cfg(feature = "test-network-parliament-signers")]
    assert_eq!(
        args.test_network_parliament_beacon_signer_mode,
        TestNetworkParliamentBeaconSignerMode::Valid,
    );
}

#[cfg(feature = "test-network-parliament-signers")]
#[test]
fn feature_only_parliament_beacon_signer_mode_is_exact_and_hidden() {
    for (value, expected) in [
        ("valid", TestNetworkParliamentBeaconSignerMode::Valid),
        ("absent", TestNetworkParliamentBeaconSignerMode::Absent),
        ("invalid", TestNetworkParliamentBeaconSignerMode::Invalid),
    ] {
        let args = Args::try_parse_from([
            "test",
            "--test-network-parliament-beacon-signer-mode",
            value,
        ])
        .expect("parse exact feature-only beacon signer mode");
        assert_eq!(args.test_network_parliament_beacon_signer_mode, expected);
    }
    assert!(
        Args::try_parse_from([
            "test",
            "--test-network-parliament-beacon-signer-mode",
            "faulty",
        ])
        .is_err(),
        "unknown feature-only modes must fail closed",
    );
    let help = Args::try_parse_from(["test", "--help"])
        .expect_err("help exits through clap")
        .to_string();
    assert!(
        !help.contains("test-network-parliament-beacon-signer-mode"),
        "the feature-only child-process argument must remain hidden",
    );
}
#[test]
fn check_config_flag_is_opt_in() {
    let args = Args::try_parse_from(["test", "--check-config"]).unwrap();
    assert!(args.startup.check_config);
}
#[test]
fn qualification_seal_writer_requires_check_config() {
    assert!(
        Args::try_parse_from([
            "test",
            "--write-kagemusha-catalog-qualification-seal",
            "/Library/SORA/Taira/seals/catalog.norito",
        ])
        .is_err()
    );
    let args = Args::try_parse_from([
        "test",
        "--check-config",
        "--write-kagemusha-catalog-qualification-seal",
        "/Library/SORA/Taira/seals/catalog.norito",
    ])
    .expect("the explicit writer is valid only with check-config");
    assert!(args.startup.check_config);
    assert_eq!(
        args.startup.write_kagemusha_catalog_qualification_seal,
        Some(PathBuf::from("/Library/SORA/Taira/seals/catalog.norito"))
    );
}

#[test]
fn validator_seal_writer_requires_check_config_and_conflicts_with_catalog_writer() {
    let validator_path = "/Library/SORA/Taira/seals/validator.norito";
    assert!(
        Args::try_parse_from([
            "test",
            "--write-kagemusha-validator-qualification-seal",
            validator_path,
        ])
        .is_err()
    );
    let args = Args::try_parse_from([
        "test",
        "--check-config",
        "--write-kagemusha-validator-qualification-seal",
        validator_path,
    ])
    .expect("the validator writer is valid only with check-config");
    assert_eq!(
        args.startup.write_kagemusha_validator_qualification_seal,
        Some(PathBuf::from(validator_path))
    );
    assert!(
        Args::try_parse_from([
            "test",
            "--check-config",
            "--write-kagemusha-catalog-qualification-seal",
            "/Library/SORA/Taira/seals/catalog.norito",
            "--write-kagemusha-validator-qualification-seal",
            validator_path,
        ])
        .is_err(),
        "one invocation must never publish both artifacts"
    );
}

#[test]
fn failed_full_check_never_invokes_validator_signing_action() {
    use std::cell::Cell;

    let invocations = Cell::new(0_u8);
    let failed_check = Err(
        Report::new(MainError::Config)
            .attach("injected invalid genesis after catalog and reservation preparation"),
    );
    let result = continue_after_full_kagemusha_check(failed_check, |()| {
        invocations.set(invocations.get() + 1);
        Ok(())
    });
    assert!(result.is_err());
    assert_eq!(invocations.get(), 0, "validator signer must remain untouched");
}

#[cfg(unix)]
#[test]
fn qualification_seal_path_must_be_canonical_and_source_separate() {
    assert!(validate_canonical_absolute_path(Path::new("seal.norito"), "test seal").is_err());
    assert!(
        validate_canonical_absolute_path(Path::new("/trusted/../seal.norito"), "test seal")
            .is_err()
    );
    assert!(
        validate_canonical_absolute_path(Path::new("/trusted/seal.norito"), "test seal").is_ok()
    );
    let mut config = Config::from_toml_source(TomlSource::inline(minimal_config_table()))
        .expect("resolve repository default config");
    config.settlement.offline.kagemusha_release_policy_path =
        Some(PathBuf::from("/qualified/policy/release-policy.norito"));
    config.settlement.offline.kagemusha_artifact_dir = Some(PathBuf::from("/qualified/artifacts"));
    let error = validate_qualification_seal_directory_separation(
        &config,
        Path::new("/qualified/policy/catalog-seal.norito"),
    )
    .expect_err("seal publication must not mutate the policy parent");
    assert!(error.contains("must be separate"));
    let error = validate_qualification_seal_directory_separation(
        &config,
        Path::new("/qualified/artifacts/seals/catalog-seal.norito"),
    )
    .expect_err("seal publication must not mutate the artifact tree");
    assert!(error.contains("must be separate"));
}
#[cfg(unix)]
#[test]
fn qualification_seal_publication_is_immutable_and_exclusive() {
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
    let temp = tempfile::tempdir_in(std::env::current_dir().expect("current directory"))
        .expect("private test root");
    let canonical_temp = fs::canonicalize(temp.path()).expect("canonical private test root");
    let seal_dir = canonical_temp.join("seals");
    fs::create_dir(&seal_dir).expect("seal directory");
    fs::set_permissions(&seal_dir, fs::Permissions::from_mode(0o700))
        .expect("private seal directory");
    let path = seal_dir.join("catalog.norito");
    let expected_uid = rustix::process::geteuid().as_raw();
    let target = QualificationSealPublicationTarget::prepare_for_owner(&path, expected_uid)
        .expect("trusted absent destination");
    // Regression: the generic post-rename custody confirmation must accept
    // the destination directory timestamp change caused by its own rename.
    target
        .publish_bytes_and_verify(b"canonical-seal-v1", |published| {
            let bytes = fs::read(published)
                .map_err(|error| format!("failed to read published test seal: {error}"))?;
            if bytes != b"canonical-seal-v1" {
                return Err("published test seal bytes differ".to_owned());
            }
            Ok(())
        })
        .expect("post-rename confirmation succeeds");
    let metadata = fs::symlink_metadata(&path).expect("published seal metadata");
    assert!(metadata.is_file());
    assert_eq!(metadata.uid(), expected_uid);
    assert_eq!(metadata.nlink(), 1);
    assert_eq!(metadata.mode() & 0o7777, 0o444);
    #[cfg(target_os = "macos")]
    require_no_macos_extended_acl(&path, "published qualification seal")
        .expect("published seal is ACL-free");
    assert_eq!(
        RootOwnedNoReplaceArtifactPublicationTarget::read_bounded_for_owner(
            &path,
            64,
            expected_uid,
            "test reservation",
        )
        .expect("read stable published bytes"),
        b"canonical-seal-v1"
    );
    let bounded_error = RootOwnedNoReplaceArtifactPublicationTarget::read_bounded_for_owner(
        &path,
        8,
        expected_uid,
        "test reservation",
    )
    .expect_err("the descriptor read must enforce its byte bound");
    assert!(bounded_error.contains("outside 1..=8 bytes"));
    let error = QualificationSealPublicationTarget::prepare_for_owner(&path, expected_uid)
        .err()
        .expect("an existing seal is never replaced");
    assert!(error.contains("already exists"));
    let rejected_path = seal_dir.join("rejected.norito");
    let rejected_target = RootOwnedNoReplaceArtifactPublicationTarget::prepare_for_owner(
        &rejected_path,
        expected_uid,
        "injected test artifact",
    )
    .expect("second trusted absent destination");
    let error = rejected_target
        .publish_bytes_and_verify(b"canonical-seal-v1", |_| {
            Err("injected final verification failure".to_owned())
        })
        .expect_err("post-rename verification failure is commit-uncertain");
    assert!(error.to_string().contains("commit-uncertain"));
    assert!(matches!(
        error,
        root_owned_artifact_publication::RootOwnedArtifactPublicationError::CommitUncertain { .. }
    ));
    assert_eq!(
        fs::read(&rejected_path).expect("commit-uncertain final inode remains readable"),
        b"canonical-seal-v1"
    );
    let metadata = fs::symlink_metadata(&rejected_path)
        .expect("commit-uncertain final inode remains in place");
    assert_eq!(metadata.uid(), expected_uid);
    assert_eq!(metadata.nlink(), 1);
    assert_eq!(metadata.mode() & 0o7777, 0o444);
    let second = RootOwnedNoReplaceArtifactPublicationTarget::prepare_for_owner(
        &rejected_path,
        expected_uid,
        "injected test artifact",
    )
    .expect_err("a commit-uncertain final name makes a second prepare terminal");
    assert!(second.to_string().contains("already exists"));
}
#[cfg(unix)]
#[test]
fn root_owned_bounded_reader_rejects_unsafe_file_shapes() {
    use std::os::unix::fs::{PermissionsExt as _, symlink};

    let temp = tempfile::tempdir_in(std::env::current_dir().expect("current directory"))
        .expect("private reader test root");
    let canonical_temp = fs::canonicalize(temp.path()).expect("canonical reader test root");
    let artifact_dir = canonical_temp.join("artifacts");
    fs::create_dir(&artifact_dir).expect("artifact directory");
    fs::set_permissions(&artifact_dir, fs::Permissions::from_mode(0o700))
        .expect("private artifact directory");
    let expected_uid = rustix::process::geteuid().as_raw();

    let writable = artifact_dir.join("writable.norito");
    fs::write(&writable, b"artifact").expect("write writable artifact");
    fs::set_permissions(&writable, fs::Permissions::from_mode(0o644))
        .expect("set writable artifact mode");
    let error = RootOwnedNoReplaceArtifactPublicationTarget::read_bounded_for_owner(
        &writable,
        64,
        expected_uid,
        "test reservation",
    )
    .expect_err("a writable artifact must fail closed");
    assert!(error.contains("mode 0444"));

    let source = artifact_dir.join("source.norito");
    fs::write(&source, b"artifact").expect("write hard-link source");
    fs::set_permissions(&source, fs::Permissions::from_mode(0o444))
        .expect("make hard-link source immutable");
    let linked = artifact_dir.join("linked.norito");
    fs::hard_link(&source, &linked).expect("create second hard link");
    let error = RootOwnedNoReplaceArtifactPublicationTarget::read_bounded_for_owner(
        &linked,
        64,
        expected_uid,
        "test reservation",
    )
    .expect_err("a multiply-linked artifact must fail closed");
    assert!(error.contains("single-link regular file"));

    let symlinked = artifact_dir.join("symlinked.norito");
    symlink(&source, &symlinked).expect("create artifact symlink");
    let error = RootOwnedNoReplaceArtifactPublicationTarget::read_bounded_for_owner(
        &symlinked,
        64,
        expected_uid,
        "test reservation",
    )
    .expect_err("a symlinked artifact must fail closed");
    assert!(error.contains("direct single-link regular file"));
}
#[cfg(target_os = "macos")]
#[test]
fn root_owned_bounded_reader_rejects_extended_attributes() {
    use std::os::unix::fs::PermissionsExt as _;

    let temp = tempfile::tempdir_in(std::env::current_dir().expect("current directory"))
        .expect("private xattr test root");
    let artifact = fs::canonicalize(temp.path())
        .expect("canonical xattr test root")
        .join("reservation.norito");
    fs::write(&artifact, b"artifact").expect("write xattr-bearing artifact");
    let status = std::process::Command::new("/usr/bin/xattr")
        .args(["-w", "com.sora.kagemusha-test", "present"])
        .arg(&artifact)
        .status()
        .expect("run xattr");
    assert!(status.success(), "test xattr must be installed");
    fs::set_permissions(&artifact, fs::Permissions::from_mode(0o444))
        .expect("make xattr-bearing artifact immutable");
    let error = RootOwnedNoReplaceArtifactPublicationTarget::read_bounded_for_owner(
        &artifact,
        64,
        rustix::process::geteuid().as_raw(),
        "test reservation",
    )
    .expect_err("an xattr-bearing artifact must fail closed");
    assert!(error.contains("xattr-free"));
}
#[cfg(target_os = "macos")]
#[test]
fn qualification_seal_publication_rejects_acl_writable_parent() {
    use std::os::unix::fs::PermissionsExt as _;

    let temp = tempfile::tempdir_in(std::env::current_dir().expect("current directory"))
        .expect("private ACL test root");
    let canonical_temp = fs::canonicalize(temp.path()).expect("canonical ACL test root");
    let seal_dir = canonical_temp.join("seals");
    fs::create_dir(&seal_dir).expect("seal directory");
    let expected_uid = rustix::process::geteuid().as_raw();
    let artifact = seal_dir.join("reservation.norito");
    fs::write(&artifact, b"artifact").expect("write reader artifact");
    fs::set_permissions(&artifact, fs::Permissions::from_mode(0o444))
        .expect("make reader artifact immutable");
    let (publication_error, reader_error) = {
        let _acl = add_macos_acl(&seal_dir, "everyone allow write");
        let publication_error = QualificationSealPublicationTarget::prepare_for_owner(
            &seal_dir.join("catalog.norito"),
            expected_uid,
        )
        .err()
        .expect("ACL-writable publication parent must fail closed");
        let reader_error = RootOwnedNoReplaceArtifactPublicationTarget::read_bounded_for_owner(
            &artifact,
            64,
            expected_uid,
            "test reservation",
        )
        .expect_err("ACL-writable reader parent must fail closed");
        (publication_error, reader_error)
    };
    assert!(publication_error.contains("extended ACL"));
    assert!(reader_error.contains("extended ACL"));
}
#[test]
#[allow(clippy::bool_assert_comparison)] // for expressiveness
fn terminal_colors_works_as_expected() -> eyre::Result<()> {
    fn try_with(arg: &str) -> eyre::Result<bool> {
        Ok(Args::try_parse_from(["test", arg])?.terminal_colors)
    }
    assert_eq!(
        Args::try_parse_from(["test"])?.terminal_colors,
        is_coloring_supported()
    );
    assert_eq!(try_with("--terminal-colors")?, true);
    assert_eq!(try_with("--terminal-colors=false")?, false);
    assert_eq!(try_with("--terminal-colors=true")?, true);
    assert!(try_with("--terminal-colors=random").is_err());
    Ok(())
}
#[test]
fn user_provided_config_path_works() {
    let args = Args::try_parse_from(["test", "--config", "/home/custom/file.json"]).unwrap();
    assert_eq!(args.config, Some(PathBuf::from("/home/custom/file.json")));
}
#[test]
fn user_can_provide_any_extension() {
    let _args = Args::try_parse_from(["test", "--config", "file.toml.but.not"])
        .expect("should allow doing this as well");
}
#[test]
fn canonical_single_lane_topology_is_not_custom() {
    let nexus = iroha_config::parameters::actual::Nexus::default();
    assert!(!nexus_topology_is_custom(&nexus));
}
#[test]
fn expanded_lane_catalog_is_custom_topology() {
    use iroha_data_model::nexus::{LaneCatalog, LaneConfig};
    use std::num::NonZeroU32;
    let lane_catalog = LaneCatalog::new(
        NonZeroU32::new(2).expect("nonzero lane count"),
        vec![
            LaneConfig::default(),
            LaneConfig {
                id: LaneId::new(1),
                alias: "lane-1".to_owned(),
                description: None,
                ..LaneConfig::default()
            },
        ],
    )
    .expect("lane catalog");
    let nexus = iroha_config::parameters::actual::Nexus {
        lane_config: iroha_config::parameters::actual::LaneConfig::from_catalog(&lane_catalog),
        lane_catalog,
        ..Default::default()
    };
    assert!(nexus_topology_is_custom(&nexus));
}
#[test]
fn multilane_config_parses() {
    let config = Config::from_toml_source(TomlSource::inline(multilane_config_table()))
        .expect("multi-lane config should parse");
    assert_eq!(config.nexus.lane_catalog.lane_count().get(), 2);
    assert_eq!(config.nexus.lane_config.entries().len(), 2);
}
#[test]
fn read_genesis_handles_decode_failure() {
    // Create a bogus genesis file and ensure we return an error instead of panicking.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.genesis.signed.nrt");
    std::fs::write(&path, [0u8, 1u8, 2u8, 3u8]).unwrap();
    let res = read_genesis(&path);
    assert!(res.is_err());
}
#[test]
fn read_genesis_initializes_instruction_registry() {
    use iroha_data_model::isi::{InstructionRegistry, set_instruction_registry};
    let _registry_guard = instruction_registry_test_guard();
    // Start with an empty registry to simulate uninitialized state.
    set_instruction_registry(InstructionRegistry::new());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.genesis.signed.nrt");
    std::fs::write(&path, [0u8, 1u8, 2u8, 3u8]).unwrap();
    // `read_genesis` should initialize the registry internally and simply
    // return a decode error for the bogus file instead of panicking.
    let res = read_genesis_unlocked(&path);
    assert!(res.is_err());
}
#[cfg(feature = "beep")]
#[test]
fn startup_beep_respects_config_flag() {
    assert!(
        !startup_beep(false),
        "beep disabled by config flag should no-op"
    );
    assert!(
        startup_beep(true),
        "beep enabled by config flag should play once"
    );
}
