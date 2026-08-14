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
    let temp = tempfile::tempdir().expect("private test root");
    let canonical_temp = fs::canonicalize(temp.path()).expect("canonical private test root");
    let seal_dir = canonical_temp.join("seals");
    fs::create_dir(&seal_dir).expect("seal directory");
    fs::set_permissions(&seal_dir, fs::Permissions::from_mode(0o700))
        .expect("private seal directory");
    let path = seal_dir.join("catalog.norito");
    let expected_uid = rustix::process::geteuid().as_raw();
    let target = QualificationSealPublicationTarget::prepare_for_owner(&path, expected_uid)
        .expect("trusted absent destination");
    target
        .publish_bytes_and_verify(b"canonical-seal-v1", |published| {
            let bytes = fs::read(published)
                .map_err(|error| format!("failed to read published test seal: {error}"))?;
            if bytes != b"canonical-seal-v1" {
                return Err("published test seal bytes differ".to_owned());
            }
            Ok(())
        })
        .expect("exclusive immutable publication");
    let metadata = fs::symlink_metadata(&path).expect("published seal metadata");
    assert!(metadata.is_file());
    assert_eq!(metadata.uid(), expected_uid);
    assert_eq!(metadata.nlink(), 1);
    assert_eq!(metadata.mode() & 0o7777, 0o444);
    #[cfg(target_os = "macos")]
    require_no_macos_extended_acl(&path, "published qualification seal")
        .expect("published seal is ACL-free");
    let error = QualificationSealPublicationTarget::prepare_for_owner(&path, expected_uid)
        .err()
        .expect("an existing seal is never replaced");
    assert!(error.contains("already exists"));
    let rejected_path = seal_dir.join("rejected.norito");
    let rejected_target =
        QualificationSealPublicationTarget::prepare_for_owner(&rejected_path, expected_uid)
            .expect("second trusted absent destination");
    let error = rejected_target
        .publish_bytes_and_verify(b"canonical-seal-v1", |_| {
            Err("injected final verification failure".to_owned())
        })
        .expect_err("failed final verification must roll back the new inode");
    assert!(error.contains("removed the newly published seal"));
    assert!(!rejected_path.exists());
}
#[cfg(target_os = "macos")]
#[test]
fn qualification_seal_publication_rejects_acl_writable_parent() {
    let temp = tempfile::tempdir().expect("private ACL test root");
    let canonical_temp = fs::canonicalize(temp.path()).expect("canonical ACL test root");
    let seal_dir = canonical_temp.join("seals");
    fs::create_dir(&seal_dir).expect("seal directory");
    let expected_uid = rustix::process::geteuid().as_raw();
    let error = {
        let _acl = add_macos_acl(&seal_dir, "everyone allow write");
        QualificationSealPublicationTarget::prepare_for_owner(
            &seal_dir.join("catalog.norito"),
            expected_uid,
        )
        .err()
        .expect("ACL-writable publication parent must fail closed")
    };
    assert!(error.contains("extended ACL"));
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
fn config_router_disabled_for_single_lane_defaults() {
    let nexus = iroha_config::parameters::actual::Nexus::default();
    assert!(!should_use_config_router(&nexus));
}
#[test]
fn config_router_enabled_when_lane_catalog_expands() {
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
        enabled: true,
        lane_config: iroha_config::parameters::actual::LaneConfig::from_catalog(&lane_catalog),
        lane_catalog,
        ..Default::default()
    };
    assert!(should_use_config_router(&nexus));
}
#[test]
fn multilane_config_requires_nexus_enabled_flag() {
    let err = Config::from_toml_source(TomlSource::inline(multilane_config_table(false)))
        .expect_err("multi-lane catalog must require nexus.enabled");
    let rendered = format!("{err:?}");
    assert!(
        rendered.contains("nexus.enabled"),
        "error should mention nexus.enabled, got: {rendered}"
    );
}
#[test]
fn multilane_config_parses_when_enabled_flag_set() {
    let config = Config::from_toml_source(TomlSource::inline(multilane_config_table(true)))
        .expect("multi-lane config with nexus enabled should parse");
    assert!(config.nexus.enabled);
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
mod soranet_transport {
    use iroha_config::parameters::actual;
    use tempfile::tempdir;
    #[test]
    fn configure_soranet_transport_creates_spool_directory() {
        let temp = tempdir().expect("create temp dir");
        let spool_dir = temp.path().join("spool");
        let mut soranet = actual::StreamingSoranet::from_defaults();
        soranet.enabled = true;
        soranet.provision_spool_dir = spool_dir.clone();
        let mut handle = iroha_core::streaming::StreamingHandle::new();
        super::super::configure_soranet_transport(&mut handle, &soranet)
            .expect("soranet transport configuration should succeed");
        assert!(
            spool_dir.is_dir(),
            "expected configure_soranet_transport to create the spool directory"
        );
    }
    #[test]
    fn configure_soranet_transport_noop_when_disabled() {
        let temp = tempdir().expect("create temp dir");
        let spool_dir = temp.path().join("disabled");
        let mut soranet = actual::StreamingSoranet::from_defaults();
        soranet.enabled = false;
        soranet.provision_spool_dir = spool_dir.clone();
        let mut handle = iroha_core::streaming::StreamingHandle::new();
        super::super::configure_soranet_transport(&mut handle, &soranet)
            .expect("disabled soranet transport should not fail");
        assert!(
            !spool_dir.exists(),
            "disabled configuration must not create the spool directory"
        );
    }
}
