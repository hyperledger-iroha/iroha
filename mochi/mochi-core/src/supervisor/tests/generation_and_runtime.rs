#[test]
fn post_commit_failure_preserves_reconciliation_and_publication_uncertainty() {
    let failure = combine_post_commit_failures(
        Some(SupervisorError::Config(
            "compatibility reconciliation failed".to_owned(),
        )),
        Some(SupervisorError::PublicationUncertain {
            generation_id: "generation-new".to_owned(),
            source: std::io::Error::other("directory sync failed"),
        }),
    )
    .expect("compound post-commit failure");
    match failure {
        SupervisorError::ReconciliationAndPublicationUncertainty {
            reconciliation,
            uncertainty,
        } => {
            assert!(matches!(*reconciliation, SupervisorError::Config(_)));
            assert!(matches!(
                *uncertainty,
                SupervisorError::PublicationUncertain { .. }
            ));
        }
        other => panic!("expected compound post-commit failure, got {other:?}"),
    }
}
#[test]
fn post_commit_failure_combiner_preserves_single_failures_and_success() {
    let reconciliation = combine_post_commit_failures(
        Some(SupervisorError::Config("reconciliation failed".to_owned())),
        None,
    )
    .expect("reconciliation failure");
    assert!(matches!(reconciliation, SupervisorError::Config(_)));
    let uncertainty = combine_post_commit_failures(
        None,
        Some(SupervisorError::PublicationUncertain {
            generation_id: "generation-new".to_owned(),
            source: std::io::Error::other("directory sync failed"),
        }),
    )
    .expect("publication uncertainty");
    assert!(matches!(
        uncertainty,
        SupervisorError::PublicationUncertain { .. }
    ));
    assert!(combine_post_commit_failures(None, None).is_none());
}
#[test]
fn builder_early_reconciliation_failure_preserves_publication_uncertainty() {
    if !ports_available("builder_early_reconciliation_failure_preserves_publication_uncertainty") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let error = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .with_post_commit_faults_for_test(PublicationFaultPoint::AfterPointerRename)
        .build()
        .expect_err("early reconciliation and uncertainty must both surface");
    match error {
        SupervisorError::ReconciliationAndPublicationUncertainty {
            reconciliation,
            uncertainty,
        } => {
            assert!(matches!(
                *reconciliation,
                SupervisorError::GenerationValidation(ref message)
                    if message.contains("injected early builder")
            ));
            assert!(matches!(
                *uncertainty,
                SupervisorError::PublicationUncertain { .. }
            ));
        }
        other => panic!("expected compound builder post-commit failure, got {other:?}"),
    }
    let paths = NetworkPaths::from_root(
        temp.path(),
        &NetworkProfile::from_preset(ProfilePreset::SinglePeer),
    );
    let selected = current_generation_id(paths.root())
        .expect("read committed selection")
        .expect("post-rename selection");
    verify_selected_generation(paths.root(), &selected)
        .expect("early reconciliation failure must retain committed generation");
}
#[test]
fn selected_storage_lease_blocks_writers_until_every_clone_drops() {
    if !ports_available("selected_storage_lease_blocks_writers_until_every_clone_drops") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    let root = supervisor.paths().root();
    let expected = Some(supervisor.generation_id().to_owned());
    let selected = resolve_selected_peer_storage_paths(root, supervisor.peers()[0].alias())
        .expect("resolve selected storage")
        .expect("selected storage");
    let selected_clone = selected.clone();
    let error = GenerationTransaction::begin_replacing(root, expected.clone())
        .expect_err("selection lease must block an exclusive writer");
    assert!(matches!(error, SupervisorError::GenerationLocked { .. }));
    drop(selected);
    let error = GenerationTransaction::begin_replacing(root, expected.clone())
        .expect_err("cloned selection lease must retain the shared lock");
    assert!(matches!(error, SupervisorError::GenerationLocked { .. }));
    drop(selected_clone);
    GenerationTransaction::begin_replacing(root, expected)
        .expect("dropping every selection lease releases the writer lock");
}
#[test]
fn selected_storage_resolver_rejects_pointer_change_before_shared_lock() {
    if !ports_available("selected_storage_resolver_rejects_pointer_change_before_shared_lock") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let mut initial = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build initial supervisor");
    let root = initial.paths().root().to_path_buf();
    let initial_generation = initial.generation_id().to_owned();
    let mut newer_generation = None;
    let error = resolve_selected_peer_storage_paths_with_hook(&root, "peer0", || {
        initial
            .restart_peer_with_extra_layers("peer0", &[])
            .expect("publish overlay between resolver reads");
        newer_generation = Some(initial.generation_id().to_owned());
    })
    .expect_err("resolver must reject a selection changed before its shared lock");
    let newer_generation = newer_generation.expect("capture newer generation");
    assert!(matches!(
        error,
        SupervisorError::GenerationSelectionChanged {
            expected: Some(ref expected),
            actual: Some(ref actual),
        } if expected == &initial_generation && actual == &newer_generation
    ));
    assert_eq!(
        current_generation_id(&root).expect("read newer selection"),
        Some(newer_generation)
    );
}
#[test]
fn selected_storage_resolver_fails_fast_while_writer_holds_exclusive_lock() {
    if !ports_available("selected_storage_resolver_fails_fast_while_writer_holds_exclusive_lock") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    let root = supervisor.paths().root();
    let expected = Some(supervisor.generation_id().to_owned());
    let writer = GenerationTransaction::begin_replacing(root, expected)
        .expect("hold exclusive generation writer lock");
    let error = resolve_selected_peer_storage_paths(root, supervisor.peers()[0].alias())
        .expect_err("resolver must fail fast while an exclusive writer is active");
    assert!(matches!(error, SupervisorError::GenerationLocked { .. }));
    drop(writer);
    resolve_selected_peer_storage_paths(root, supervisor.peers()[0].alias())
        .expect("resolver succeeds after exclusive writer drops")
        .expect("selection remains available");
}
#[test]
fn session_info_fails_fast_while_writer_holds_exclusive_lock() {
    if !ports_available("session_info_fails_fast_while_writer_holds_exclusive_lock") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    let root = supervisor.paths().root();
    let writer =
        GenerationTransaction::begin_replacing(root, Some(supervisor.generation_id().to_owned()))
            .expect("hold exclusive generation writer lock");
    let error = supervisor
        .session_info()
        .expect_err("session metadata must fail fast while a writer is active");
    assert!(matches!(error, SupervisorError::GenerationLocked { .. }));
    drop(writer);
    supervisor
        .session_info()
        .expect("session metadata succeeds after writer drops");
}
#[test]
fn supervisor_respects_explicit_kagami_override() {
    if !ports_available("supervisor_respects_explicit_kagami_override") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let stub = StandaloneKagamiStub::create(temp.path());
    SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .kagami_path(stub.script_path())
        .build()
        .expect("build supervisor with explicit kagami path");
    let log = fs::read_to_string(stub.log_path()).expect("explicit kagami log");
    assert!(
        log.contains("--genesis-public-key"),
        "expected explicit kagami stub to capture genesis args, got `{log}`"
    );
}
#[test]
fn supervisor_runs_kagami_verify_for_profile() {
    if !ports_available("supervisor_runs_kagami_verify_for_profile") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let stub = StandaloneKagamiStub::create(temp.path());
    let _guard = EnvVarGuard::set("MOCHI_KAGAMI", stub.script_path().as_os_str());
    let _supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .genesis_profile(GenesisProfile::Iroha3Dev)
        .build()
        .expect("build supervisor with kagami verify");
    let log = fs::read_to_string(stub.log_path()).expect("read kagami log");
    let lines: Vec<_> = log.lines().collect();
    assert!(
        lines.contains(&"genesis"),
        "expected kagami genesis invocation, got `{log}`"
    );
    assert!(
        lines.contains(&"generate"),
        "expected kagami generate invocation, got `{log}`"
    );
    assert!(
        lines.contains(&"verify"),
        "expected kagami verify invocation, got `{log}`"
    );
}
#[test]
fn existing_peer_directories_preserve_unmanaged_artifacts_during_build() {
    if !ports_available("existing_peer_directories_preserve_unmanaged_artifacts_during_build") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let slug = NetworkProfile::from_preset(ProfilePreset::SinglePeer).slug();
    let root = temp.path().join(slug);
    let peer_dir = root.join("peers").join("peer0");
    let stale_file = peer_dir.join("stale.bin");
    fs::create_dir_all(&peer_dir).expect("create stale peer dir");
    fs::write(&stale_file, b"leftover").expect("write stale file");
    assert!(
        stale_file.exists(),
        "stale file should exist before supervisor build"
    );
    let _stub = KagamiStub::install(temp.path());
    let supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    assert!(
        stale_file.exists(),
        "building a new generation must not delete unmanaged peer artifacts"
    );
    let rebuilt_storage = supervisor.peers()[0].spec.storage_dir.clone();
    assert!(
        rebuilt_storage.exists(),
        "storage directory should be recreated after cleanup"
    );
}
#[test]
fn selected_storage_resolver_reports_config_only_overlay_separately() {
    if !ports_available("selected_storage_resolver_reports_config_only_overlay_separately") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    let old_config_generation = supervisor.generation_id().to_owned();
    let old_storage = supervisor.peers()[0].storage_dir().to_path_buf();
    supervisor
        .restart_peer_with_extra_layers("peer0", &[])
        .expect("publish config-only overlay");
    let selected = resolve_selected_peer_storage_paths(
        supervisor.paths().root(),
        supervisor.peers()[0].alias(),
    )
    .expect("resolve selected paths")
    .expect("selected generation exists");
    assert_ne!(supervisor.generation_id(), old_config_generation);
    assert_eq!(selected.config_generation_id(), supervisor.generation_id());
    assert_eq!(selected.storage_generation_id(), old_config_generation);
    assert_eq!(selected.storage_dir(), old_storage);
    assert_eq!(selected.storage_dir(), supervisor.peers()[0].storage_dir());
    assert_eq!(
        selected.snapshot_dir(),
        supervisor.peers()[0].snapshot_dir()
    );
}
#[test]
fn second_supervisor_cannot_publish_until_current_owner_drops() {
    if !ports_available("second_supervisor_cannot_publish_until_current_owner_drops") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _kagami = KagamiStub::install(temp.path());
    let current = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build current supervisor");
    let current_generation = current.generation_id().to_owned();
    let current_config = current.peers()[0].config_path().to_path_buf();
    let current_config_bytes = fs::read(&current_config).expect("read current config");
    let error = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect_err("second supervisor must not share a live runtime root");
    assert!(matches!(error, SupervisorError::SupervisorLocked { .. }));
    assert_eq!(
        current_generation_id(current.paths().root()).expect("read preserved selection"),
        Some(current_generation.clone())
    );
    assert_eq!(
        fs::read(&current_config).expect("read preserved config"),
        current_config_bytes
    );
    drop(current);
    let replacement = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("dropping the current owner releases the runtime root");
    assert_ne!(replacement.generation_id(), current_generation);
}
#[test]
fn consuming_same_root_replacement_transfers_ownership() {
    if !ports_available("consuming_same_root_replacement_transfers_ownership") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _kagami = KagamiStub::install(temp.path());
    let previous = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build previous supervisor");
    let previous_generation = previous.generation_id().to_owned();
    let replacement = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build_replacing(previous)
        .expect("consume previous owner into replacement");
    assert_ne!(replacement.generation_id(), previous_generation);
}
#[test]
fn consuming_replacement_returns_previous_only_after_precommit_failure() {
    if !ports_available("consuming_replacement_returns_previous_only_after_precommit_failure") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _kagami = KagamiStub::install(temp.path());
    let previous = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build previous supervisor");
    let generation = previous.generation_id().to_owned();
    let failure = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .nexus_enabled(true)
        .build_replacing(previous)
        .expect_err("invalid replacement must fail before publication");
    let (_error, previous) = failure.into_parts();
    let previous = previous.expect("precommit failure must return previous owner");
    assert_eq!(previous.generation_id(), generation);
    assert_eq!(
        current_generation_id(previous.paths().root()).expect("read selected generation"),
        Some(generation)
    );
}
#[test]
fn consuming_cross_root_replacement_keeps_old_owner_only_until_success() {
    if !ports_available("consuming_cross_root_replacement_keeps_old_owner_only_until_success") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let old_data = temp.path().join("old");
    let new_data = temp.path().join("new");
    let _kagami = KagamiStub::install(temp.path());
    let previous = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(&old_data)
        .build()
        .expect("build old root");
    let old_root = previous.paths().root().to_path_buf();
    let replacement = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(&new_data)
        .build_replacing(previous)
        .expect("build independently owned new root");
    assert_ne!(replacement.paths().root(), old_root);
    SupervisorOwnershipLock::acquire(&old_root).expect("successful transition releases old root");
}
#[test]
fn consuming_postcommit_failure_permanently_retires_previous() {
    if !ports_available("consuming_postcommit_failure_permanently_retires_previous") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _kagami = KagamiStub::install(temp.path());
    let previous = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build previous supervisor");
    let failure = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .with_post_commit_faults_for_test(PublicationFaultPoint::AfterPointerRename)
        .build_replacing(previous)
        .expect_err("inject postcommit uncertainty");
    let (error, previous) = failure.into_parts();
    assert!(matches!(
        error,
        SupervisorError::ReconciliationAndPublicationUncertainty { .. }
    ));
    assert!(previous.is_none(), "stale previous handle must be retired");
}
#[cfg(unix)]
#[test]
fn overlay_rejects_retained_storage_symlink_before_publication() {
    if !ports_available("overlay_rejects_retained_storage_symlink_before_publication") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _kagami = KagamiStub::install(temp.path());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    let generation = supervisor.generation_id().to_owned();
    let attacker = temp.path().join("attacker-overlay-storage");
    let sentinel = redirect_storage_generations_through_symlink(&supervisor.peers()[0], &attacker);
    let error = supervisor
        .restart_peer_with_extra_layers("peer0", &[])
        .expect_err("overlay must reject redirected retained storage");
    assert!(matches!(error, SupervisorError::GenerationValidation(_)));
    assert_eq!(
        current_generation_id(supervisor.paths().root()).expect("read selected generation"),
        Some(generation)
    );
    assert_eq!(
        fs::read(sentinel).expect("read sentinel"),
        b"must-not-touch"
    );
}
#[cfg(unix)]
#[test]
fn overlay_rejects_retained_streaming_symlink_before_publication() {
    if !ports_available("overlay_rejects_retained_streaming_symlink_before_publication") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _kagami = KagamiStub::install(temp.path());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    let generation = supervisor.generation_id().to_owned();
    let attacker = temp.path().join("attacker-streaming");
    fs::create_dir(&attacker).expect("create attacker directory");
    let sentinel = attacker.join("sentinel");
    fs::write(&sentinel, b"must-not-touch").expect("write attacker sentinel");
    symlink(
        &attacker,
        supervisor.peers()[0].storage_dir().join("streaming"),
    )
    .expect("redirect managed streaming directory");
    let error = supervisor
        .restart_peer_with_extra_layers("peer0", &[])
        .expect_err("overlay must reject redirected streaming storage");
    assert!(matches!(error, SupervisorError::GenerationValidation(_)));
    assert_eq!(
        current_generation_id(supervisor.paths().root()).expect("read selected generation"),
        Some(generation)
    );
    assert_eq!(
        fs::read(sentinel).expect("read sentinel"),
        b"must-not-touch"
    );
}
#[cfg(unix)]
#[test]
fn overlay_precommit_failure_preserves_primary_when_peer_restart_fails() {
    if !ports_available("overlay_precommit_failure_preserves_primary_when_peer_restart_fails") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _kagami = KagamiStub::install(temp.path());
    let irohad = write_long_running_irohad_stub(temp.path());
    let _irohad = EnvVarGuard::set("MOCHI_IROHAD", irohad.as_os_str());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    supervisor.start_peer("peer0").expect("start peer0");
    let old_generation = supervisor.generation_id().to_owned();
    let old_config = supervisor.peers()[0].config_path().to_path_buf();
    let old_config_bytes = fs::read(&old_config).expect("read old config");
    let old_storage = supervisor.peers()[0].storage_dir().to_path_buf();
    let mut permissions = fs::metadata(&irohad)
        .expect("irohad stub metadata")
        .permissions();
    permissions.set_mode(0o600);
    fs::set_permissions(&irohad, permissions).expect("disable irohad stub execution");
    let error = supervisor
        .restart_peer_with_extra_layers_with_publication_fault(
            "peer0",
            &[],
            PublicationFaultPoint::BeforeInventory,
        )
        .expect_err("overlay publication and lifecycle restoration must both fail");
    match error {
        SupervisorError::OperationAndRunningSetRestore { primary, restore } => {
            assert!(matches!(*primary, SupervisorError::GenerationValidation(_)));
            assert!(matches!(
                *restore,
                SupervisorError::RunningSetRestore { .. }
            ));
        }
        other => panic!("expected compound overlay restoration error, got {other:?}"),
    }
    assert_eq!(supervisor.generation_id(), old_generation);
    assert_eq!(supervisor.peers()[0].config_path(), old_config);
    assert_eq!(
        fs::read(supervisor.peers()[0].config_path()).expect("read preserved config"),
        old_config_bytes
    );
    assert_eq!(supervisor.peers()[0].storage_dir(), old_storage);
    assert_eq!(
        current_generation_id(supervisor.paths.root()).expect("read old selection"),
        Some(old_generation)
    );
    assert!(!supervisor.peers()[0].is_running());
}
#[cfg(unix)]
#[test]
fn overlay_publication_uncertainty_adopts_commit_before_restart_failure() {
    if !ports_available("overlay_publication_uncertainty_adopts_commit_before_restart_failure") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _kagami = KagamiStub::install(temp.path());
    let irohad = write_long_running_irohad_stub(temp.path());
    let _irohad = EnvVarGuard::set("MOCHI_IROHAD", irohad.as_os_str());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    supervisor.start_peer("peer0").expect("start peer0");
    let old_generation = supervisor.generation_id().to_owned();
    let old_config = supervisor.peers()[0].config_path().to_path_buf();
    let old_storage = supervisor.peers()[0].storage_dir().to_path_buf();
    let mut permissions = fs::metadata(&irohad)
        .expect("irohad stub metadata")
        .permissions();
    permissions.set_mode(0o600);
    fs::set_permissions(&irohad, permissions).expect("disable irohad stub execution");
    let error = supervisor
        .restart_peer_with_extra_layers_with_publication_fault(
            "peer0",
            &[],
            PublicationFaultPoint::AfterPointerRename,
        )
        .expect_err("uncertain overlay commit and restart must both be surfaced");
    let committed_generation = match error {
        SupervisorError::OperationAndRunningSetRestore { primary, restore } => {
            assert!(matches!(
                *restore,
                SupervisorError::RunningSetRestore { .. }
            ));
            match *primary {
                SupervisorError::PublicationUncertain { generation_id, .. } => generation_id,
                other => panic!("expected overlay publication uncertainty, got {other:?}"),
            }
        }
        other => panic!("expected compound overlay postcommit error, got {other:?}"),
    };
    assert_ne!(committed_generation, old_generation);
    assert_eq!(supervisor.generation_id(), committed_generation);
    assert_ne!(supervisor.peers()[0].config_path(), old_config);
    assert_eq!(supervisor.peers()[0].storage_dir(), old_storage);
    assert_eq!(
        current_generation_id(supervisor.paths.root()).expect("read committed selection"),
        Some(committed_generation)
    );
    assert!(!supervisor.peers()[0].is_running());
}
#[cfg(unix)]
#[test]
fn committed_overlay_surfaces_final_peer_restart_failure() {
    if !ports_available("committed_overlay_surfaces_final_peer_restart_failure") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _kagami = KagamiStub::install(temp.path());
    let irohad = write_long_running_irohad_stub(temp.path());
    let _irohad = EnvVarGuard::set("MOCHI_IROHAD", irohad.as_os_str());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    supervisor.start_peer("peer0").expect("start peer0");
    let old_generation = supervisor.generation_id().to_owned();
    let old_config = supervisor.peers()[0].config_path().to_path_buf();
    let old_storage = supervisor.peers()[0].storage_dir().to_path_buf();
    let mut permissions = fs::metadata(&irohad)
        .expect("irohad stub metadata")
        .permissions();
    permissions.set_mode(0o600);
    fs::set_permissions(&irohad, permissions).expect("disable irohad stub execution");
    let error = supervisor
        .restart_peer_with_extra_layers("peer0", &[])
        .expect_err("committed overlay must report its peer restart failure");
    assert!(matches!(error, SupervisorError::RunningSetRestore { .. }));
    let committed_generation = supervisor.generation_id().to_owned();
    assert_ne!(committed_generation, old_generation);
    assert_ne!(supervisor.peers()[0].config_path(), old_config);
    assert_eq!(supervisor.peers()[0].storage_dir(), old_storage);
    assert_eq!(
        current_generation_id(supervisor.paths.root()).expect("read committed selection"),
        Some(committed_generation)
    );
    assert!(!supervisor.peers()[0].is_running());
}
#[test]
fn selected_storage_resolver_rejects_unsafe_alias_without_a_selection() {
    let temp = tempfile::tempdir().expect("tempdir");
    let error = resolve_selected_peer_storage_paths(temp.path(), "../peer0")
        .expect_err("unsafe alias must fail before selection lookup");
    assert!(error.to_string().contains("one safe path component"));
}
#[test]
fn selected_storage_resolver_reports_absent_selection() {
    let temp = tempfile::tempdir().expect("tempdir");
    assert_eq!(
        resolve_selected_peer_storage_paths(temp.path(), "peer0")
            .expect("resolve absent selection"),
        None
    );
}
#[test]
fn wipe_and_regenerate_resets_storage_and_genesis() {
    if !ports_available("wipe_and_regenerate_resets_storage_and_genesis") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    for (idx, peer) in supervisor.peers().iter().enumerate() {
        let storage_file = peer.storage_dir().join(format!("leftover-{idx}.bin"));
        fs::write(&storage_file, b"stale").expect("write stale storage file");
        assert!(
            storage_file.exists(),
            "stale storage file should exist before wipe"
        );
        let snapshot_file = peer.snapshot_dir().join("stale.txt");
        fs::write(&snapshot_file, b"stale-snapshot").expect("write stale snapshot file");
        assert!(
            snapshot_file.exists(),
            "stale snapshot file should exist before wipe"
        );
    }
    let retired_genesis_path = supervisor.genesis_manifest().to_path_buf();
    fs::write(&retired_genesis_path, b"not-json").expect("corrupt genesis manifest");
    supervisor
        .wipe_and_regenerate()
        .expect("wipe and regenerate should succeed");
    let genesis_path = supervisor.genesis_manifest().to_path_buf();
    assert_ne!(
        genesis_path, retired_genesis_path,
        "regeneration must select a new immutable generation"
    );
    let manifest_bytes = fs::read(&genesis_path).expect("read regenerated genesis");
    let manifest: Value =
        norito::json::from_slice(&manifest_bytes).expect("genesis should be valid JSON");
    assert_eq!(
        manifest
            .get("chain")
            .and_then(Value::as_str)
            .expect("chain field present"),
        supervisor.chain_id(),
        "regenerated genesis should carry supervisor chain id"
    );
    for (idx, peer) in supervisor.peers().iter().enumerate() {
        let storage_file = peer.storage_dir().join(format!("leftover-{idx}.bin"));
        assert!(
            !storage_file.exists(),
            "wipe should remove stale storage file for peer {}",
            peer.alias()
        );
        let snapshot_file = peer.snapshot_dir().join("stale.txt");
        assert!(
            !snapshot_file.exists(),
            "wipe should remove stale snapshot file for peer {}",
            peer.alias()
        );
        let generations = peer.snapshot_dir().join(SNAPSHOT_GENERATIONS_DIR_NAME);
        assert!(
            generations.is_dir(),
            "wipe should recreate the snapshot generations directory for peer {}",
            peer.alias()
        );
        assert!(
            fs::read_dir(generations)
                .expect("snapshot generations directory")
                .next()
                .is_none(),
            "wipe should leave snapshot generations empty for peer {}",
            peer.alias()
        );
    }
}
#[test]
fn post_commit_publication_fault_reconciles_generation_and_storage_atomically() {
    if !ports_available(
        "post_commit_publication_fault_reconciles_generation_and_storage_atomically",
    ) {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    let old_generation = supervisor.generation_id().to_owned();
    let old_storage = supervisor
        .peers()
        .iter()
        .map(|peer| peer.storage_dir().to_path_buf())
        .collect::<Vec<_>>();
    for (index, storage) in old_storage.iter().enumerate() {
        fs::write(storage.join(format!("sentinel-{index}")), b"old state")
            .expect("write old-generation sentinel");
    }
    let error = supervisor
        .wipe_and_regenerate_with_publication_fault(PublicationFaultPoint::AfterPointerRename)
        .expect_err("post-rename durability fault must be reported as uncertain");
    let committed_generation = match error {
        SupervisorError::PublicationUncertain { generation_id, .. } => generation_id,
        other => panic!("unexpected post-commit error: {other}"),
    };
    assert_ne!(committed_generation, old_generation);
    assert_eq!(supervisor.generation_id(), committed_generation);
    assert_eq!(
        current_generation_id(supervisor.paths.root()).expect("read committed pointer"),
        Some(committed_generation.clone())
    );
    let verified = verify_selected_generation(supervisor.paths.root(), &committed_generation)
        .expect("post-rename selected generation remains complete");
    supervisor
        .ensure_selected_generation_metadata(&verified)
        .expect("in-memory metadata follows the committed selection");
    for (index, peer) in supervisor.peers().iter().enumerate() {
        assert!(
            peer.config_path().starts_with(&verified.root),
            "peer config must move as one generation"
        );
        assert_ne!(peer.storage_dir(), old_storage[index]);
        assert!(
            !peer
                .storage_dir()
                .join(format!("sentinel-{index}"))
                .exists(),
            "fresh generation storage starts empty"
        );
        assert_eq!(
            fs::read(old_storage[index].join(format!("sentinel-{index}")))
                .expect("retired generation state remains intact"),
            b"old state"
        );
    }
}
#[test]
fn precommit_publication_fault_removes_only_candidate_runtime_storage() {
    if !ports_available("precommit_publication_fault_removes_only_candidate_runtime_storage") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    let generation = supervisor.generation_id().to_owned();
    let storage = supervisor
        .peers()
        .iter()
        .map(|peer| peer.storage_dir().to_path_buf())
        .collect::<Vec<_>>();
    let storage_entries = storage
        .iter()
        .map(|active| {
            fs::read_dir(active.parent().expect("storage-generations parent"))
                .expect("read storage generations")
                .map(|entry| entry.expect("storage generation entry").file_name())
                .collect::<HashSet<_>>()
        })
        .collect::<Vec<_>>();
    for (index, active) in storage.iter().enumerate() {
        fs::write(active.join(format!("sentinel-{index}")), b"active state")
            .expect("write active storage sentinel");
    }
    let error = supervisor
        .wipe_and_regenerate_with_publication_fault(PublicationFaultPoint::BeforeInventory)
        .expect_err("precommit publication fault must fail");
    assert!(matches!(error, SupervisorError::GenerationValidation(_)));
    assert_eq!(supervisor.generation_id(), generation);
    assert_eq!(
        current_generation_id(supervisor.paths.root()).expect("read preserved pointer"),
        Some(generation)
    );
    for (index, peer) in supervisor.peers().iter().enumerate() {
        assert_eq!(peer.storage_dir(), storage[index]);
        assert_eq!(
            fs::read(storage[index].join(format!("sentinel-{index}")))
                .expect("read preserved active state"),
            b"active state"
        );
        let after = fs::read_dir(storage[index].parent().expect("storage-generations parent"))
            .expect("read storage generations after fault")
            .map(|entry| entry.expect("storage generation entry").file_name())
            .collect::<HashSet<_>>();
        assert_eq!(
            after, storage_entries[index],
            "precommit failure must remove the candidate storage generation"
        );
    }
}
#[cfg(unix)]
#[test]
fn precommit_failure_guard_blocks_competing_writer_through_peer_restoration() {
    if !ports_available("precommit_failure_guard_blocks_competing_writer_through_peer_restoration")
    {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let irohad = write_long_running_irohad_stub(temp.path());
    let _irohad = EnvVarGuard::set("MOCHI_IROHAD", irohad.as_os_str());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    supervisor.start_peer("peer0").expect("start peer0");
    let root = supervisor.paths().root().to_path_buf();
    let expected = Some(supervisor.generation_id().to_owned());
    let mut hook_called = false;
    let error = supervisor
        .wipe_and_regenerate_with_publication_fault_and_failure_hook(
            PublicationFaultPoint::BeforeInventory,
            || {
                hook_called = true;
                let error = GenerationTransaction::begin_replacing(&root, expected.clone())
                    .expect_err("failed publication guard must block a competing writer");
                assert!(matches!(error, SupervisorError::GenerationLocked { .. }));
            },
        )
        .expect_err("injected precommit failure");
    assert!(matches!(error, SupervisorError::GenerationValidation(_)));
    assert!(hook_called);
    assert!(
        supervisor.peers()[0].is_running(),
        "captured peer must be restored before the failure guard drops"
    );
    GenerationTransaction::begin_replacing(&root, expected)
        .expect("competing writer succeeds after lifecycle restoration returns");
    supervisor.stop_all().expect("stop restored peer");
}
#[cfg(unix)]
#[test]
fn every_precommit_publication_fault_restores_exact_running_sandbox() {
    if !ports_available("every_precommit_publication_fault_restores_exact_running_sandbox") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _kagami = KagamiStub::install(temp.path());
    let irohad = write_long_running_irohad_stub(temp.path());
    let _irohad = EnvVarGuard::set("MOCHI_IROHAD", irohad.as_os_str());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    supervisor.start_all().expect("start all peers");
    let old_generation = supervisor.generation_id().to_owned();
    let old_configs = supervisor
        .peers()
        .iter()
        .map(|peer| {
            (
                peer.config_path().to_path_buf(),
                fs::read(peer.config_path()).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let old_storage = supervisor
        .peers()
        .iter()
        .map(|peer| peer.storage_dir().to_path_buf())
        .collect::<Vec<_>>();
    for (index, storage) in old_storage.iter().enumerate() {
        fs::write(
            storage.join(format!("running-sentinel-{index}")),
            b"old-state",
        )
        .expect("write running sentinel");
    }
    for point in [
        PublicationFaultPoint::BeforeInventory,
        PublicationFaultPoint::AfterInventory,
        PublicationFaultPoint::AfterTreeSync,
        PublicationFaultPoint::AfterGenerationsSync,
        PublicationFaultPoint::AfterRuntimeStorageSync,
        PublicationFaultPoint::AfterPointerWrite,
        PublicationFaultPoint::AfterPointerSync,
    ] {
        let error = supervisor
            .wipe_and_regenerate_with_publication_fault(point)
            .expect_err("precommit fault must fail");
        assert!(
            matches!(error, SupervisorError::GenerationValidation(_)),
            "successful lifecycle restoration must preserve the primary fault at {point:?}: {error}"
        );
        assert_eq!(supervisor.generation_id(), old_generation);
        assert_eq!(
            current_generation_id(supervisor.paths.root()).expect("read preserved selection"),
            Some(old_generation.clone())
        );
        for (index, peer) in supervisor.peers().iter().enumerate() {
            assert!(
                peer.is_running(),
                "peer {} must be running again after {point:?}",
                peer.alias()
            );
            assert_eq!(peer.config_path(), old_configs[index].0);
            assert_eq!(
                fs::read(peer.config_path()).expect("read preserved config"),
                old_configs[index].1
            );
            assert_eq!(peer.storage_dir(), old_storage[index]);
            assert_eq!(
                fs::read(old_storage[index].join(format!("running-sentinel-{index}")))
                    .expect("read preserved running state"),
                b"old-state"
            );
        }
    }
}
#[cfg(unix)]
#[test]
fn precommit_failure_restores_only_the_captured_partial_running_set() {
    if !ports_available("precommit_failure_restores_only_the_captured_partial_running_set") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _kagami = KagamiStub::install(temp.path());
    let irohad = write_long_running_irohad_stub(temp.path());
    let _irohad = EnvVarGuard::set("MOCHI_IROHAD", irohad.as_os_str());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    supervisor.start_peer("peer0").expect("start peer0");
    supervisor.start_peer("peer2").expect("start peer2");
    let old_generation = supervisor.generation_id().to_owned();
    let old_storage = supervisor
        .peers()
        .iter()
        .map(|peer| peer.storage_dir().to_path_buf())
        .collect::<Vec<_>>();
    supervisor
        .wipe_and_regenerate_with_publication_fault(PublicationFaultPoint::AfterPointerSync)
        .expect_err("precommit fault must fail");
    assert_eq!(supervisor.generation_id(), old_generation);
    for (index, peer) in supervisor.peers().iter().enumerate() {
        let expected_running = matches!(peer.alias(), "peer0" | "peer2");
        assert_eq!(
            peer.is_running(),
            expected_running,
            "peer {} must preserve its exact pre-operation lifecycle state",
            peer.alias()
        );
        assert_eq!(peer.storage_dir(), old_storage[index]);
    }
}
#[cfg(unix)]
#[test]
fn precommit_failure_surfaces_running_set_restoration_failure() {
    if !ports_available("precommit_failure_surfaces_running_set_restoration_failure") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _kagami = KagamiStub::install(temp.path());
    let irohad = write_long_running_irohad_stub(temp.path());
    let _irohad = EnvVarGuard::set("MOCHI_IROHAD", irohad.as_os_str());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    supervisor.start_peer("peer0").expect("start peer0");
    let old_generation = supervisor.generation_id().to_owned();
    let old_config = supervisor.peers()[0].config_path().to_path_buf();
    let old_storage = supervisor.peers()[0].storage_dir().to_path_buf();
    let mut permissions = fs::metadata(&irohad)
        .expect("irohad stub metadata")
        .permissions();
    permissions.set_mode(0o600);
    fs::set_permissions(&irohad, permissions).expect("disable irohad stub execution");
    let error = supervisor
        .wipe_and_regenerate_with_publication_fault(PublicationFaultPoint::BeforeInventory)
        .expect_err("precommit and lifecycle restoration must both fail");
    match error {
        SupervisorError::OperationAndRunningSetRestore { primary, restore } => {
            assert!(matches!(*primary, SupervisorError::GenerationValidation(_)));
            assert!(matches!(
                *restore,
                SupervisorError::RunningSetRestore { .. }
            ));
        }
        other => panic!("expected compound restoration error, got {other:?}"),
    }
    assert_eq!(supervisor.generation_id(), old_generation);
    assert_eq!(supervisor.peers()[0].config_path(), old_config);
    assert_eq!(supervisor.peers()[0].storage_dir(), old_storage);
    assert_eq!(
        current_generation_id(supervisor.paths.root()).expect("read old selection"),
        Some(old_generation)
    );
    assert!(!supervisor.peers()[0].is_running());
}
#[cfg(unix)]
#[test]
fn publication_uncertainty_adopts_commit_before_surfacing_restart_failure() {
    if !ports_available("publication_uncertainty_adopts_commit_before_surfacing_restart_failure") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _kagami = KagamiStub::install(temp.path());
    let irohad = write_long_running_irohad_stub(temp.path());
    let _irohad = EnvVarGuard::set("MOCHI_IROHAD", irohad.as_os_str());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    supervisor.start_peer("peer0").expect("start peer0");
    let old_generation = supervisor.generation_id().to_owned();
    let old_storage = supervisor.peers()[0].storage_dir().to_path_buf();
    fs::write(old_storage.join("old-state"), b"retained").expect("write old state");
    let mut permissions = fs::metadata(&irohad)
        .expect("irohad stub metadata")
        .permissions();
    permissions.set_mode(0o600);
    fs::set_permissions(&irohad, permissions).expect("disable irohad stub execution");
    let error = supervisor
        .wipe_and_regenerate_with_publication_fault(PublicationFaultPoint::AfterPointerRename)
        .expect_err("uncertain commit and restart must both be surfaced");
    let committed_generation = match error {
        SupervisorError::OperationAndRunningSetRestore { primary, restore } => {
            assert!(matches!(
                *restore,
                SupervisorError::RunningSetRestore { .. }
            ));
            match *primary {
                SupervisorError::PublicationUncertain { generation_id, .. } => generation_id,
                other => panic!("expected publication uncertainty, got {other:?}"),
            }
        }
        other => panic!("expected compound postcommit error, got {other:?}"),
    };
    assert_ne!(committed_generation, old_generation);
    assert_eq!(supervisor.generation_id(), committed_generation);
    assert_eq!(
        current_generation_id(supervisor.paths.root()).expect("read committed selection"),
        Some(committed_generation)
    );
    assert_ne!(supervisor.peers()[0].storage_dir(), old_storage);
    assert_eq!(
        fs::read(old_storage.join("old-state")).expect("read retained old state"),
        b"retained"
    );
    assert!(!supervisor.peers()[0].is_running());
}
#[test]
fn genesis_topology_matches_peer_configuration_across_presets() {
    if !ports_available("genesis_topology_matches_peer_configuration_across_presets") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    for preset in [ProfilePreset::SinglePeer, ProfilePreset::FourPeerBft] {
        let supervisor = SupervisorBuilder::new(preset)
            .data_root(temp.path())
            .build()
            .expect("build supervisor");
        let bytes = fs::read(supervisor.genesis_manifest()).expect("genesis manifest readable");
        let manifest: norito::json::Value =
            norito::json::from_slice(&bytes).expect("parse genesis json");
        let transactions = manifest
            .get("transactions")
            .and_then(norito::json::Value::as_array)
            .expect("transactions array");
        let topology = transactions
            .iter()
            .filter_map(|tx| tx.get("topology").and_then(norito::json::Value::as_array))
            .find(|entries| !entries.is_empty())
            .expect("non-empty topology transaction present");
        let actual_peer_ids: Vec<PeerId> = topology
            .iter()
            .map(|entry| {
                let topology_entry: GenesisTopologyEntry =
                    norito::json::from_value(entry.clone()).expect("topology entry should decode");
                topology_entry.peer
            })
            .collect();
        let expected_peer_ids: Vec<PeerId> = supervisor
            .peers()
            .iter()
            .map(|peer| peer.peer_id())
            .collect();
        assert_eq!(
            actual_peer_ids, expected_peer_ids,
            "topology should mirror prepared peers for preset {preset:?}"
        );
        let chain = manifest
            .get("chain")
            .and_then(norito::json::Value::as_str)
            .expect("chain field");
        assert_eq!(
            chain,
            supervisor.chain_id(),
            "manifest chain id should match supervisor for preset {preset:?}"
        );
    }
}
#[test]
fn peer_spec_peer_id_roundtrip() {
    let temp = tempfile::tempdir().expect("tempdir");
    let paths = NetworkPaths::from_root(temp.path(), &NetworkProfile::default());
    paths.ensure().expect("paths");
    let spec = test_peer_spec(&paths, "peer0".into(), 8080, 1337).expect("peer spec");
    let peer_id = spec.peer_id();
    let parsed: PublicKey = peer_id.public_key().clone();
    assert_eq!(parsed, spec.keys.public_key);
}
#[test]
fn generated_peer_config_preserves_all_mochi_managed_paths() {
    let temp = tempfile::tempdir().expect("tempdir");
    let paths = NetworkPaths::from_root(temp.path(), &NetworkProfile::default());
    paths.ensure().expect("paths");
    let spec = test_peer_spec(&paths, "peer0".into(), 8080, 1337).expect("peer spec");
    let genesis = test_genesis_material(&paths);
    spec.write_config(
        "managed-paths-chain",
        &genesis,
        std::slice::from_ref(&spec),
        &PeerConfigOverrides::default(),
        &[],
    )
    .expect("write generated peer config");
    let config =
        ManagedNodeConfig::from_path(&spec.config_path).expect("parse generated peer config");
    validate_managed_peer_paths(&config, &spec, 1)
        .expect("generated config keeps every Mochi-managed path");
}
#[test]
fn managed_peer_path_validation_rejects_runtime_root_redirects() {
    let temp = tempfile::tempdir().expect("tempdir");
    let paths = NetworkPaths::from_root(temp.path(), &NetworkProfile::default());
    paths.ensure().expect("paths");
    let spec = test_peer_spec(&paths, "peer0".into(), 8080, 1337).expect("peer spec");
    let genesis = test_genesis_material(&paths);
    spec.write_config(
        "managed-path-tamper-chain",
        &genesis,
        std::slice::from_ref(&spec),
        &PeerConfigOverrides::default(),
        &[],
    )
    .expect("write generated peer config");
    let load =
        || ManagedNodeConfig::from_path(&spec.config_path).expect("parse generated peer config");
    let mut config = load();
    config.managed_paths.kura_store_dir = temp.path().join("redirected-kura");
    let error = validate_managed_peer_paths(&config, &spec, 1)
        .expect_err("redirected Kura root must fail generation validation");
    assert!(error.to_string().contains("kura.store_dir"));
    let mut config = load();
    config.managed_paths.snapshot_store_dir = temp.path().join("redirected-snapshot");
    let error = validate_managed_peer_paths(&config, &spec, 1)
        .expect_err("redirected snapshot root must fail generation validation");
    assert!(error.to_string().contains("snapshot.store_dir"));
    let mut config = load();
    config.managed_paths.torii_data_dir = temp.path().join("redirected-torii");
    let error = validate_managed_peer_paths(&config, &spec, 1)
        .expect_err("redirected Torii root must fail generation validation");
    assert!(error.to_string().contains("torii.data_dir"));
}
#[test]
fn normalize_peer_config_overrides_sets_lane_count_and_local_services() {
    let mut nexus = toml::Table::new();
    nexus.insert("enabled".into(), toml::Value::Boolean(true));
    let mut lane0 = toml::Table::new();
    lane0.insert("alias".into(), toml::Value::String("core".into()));
    lane0.insert("index".into(), toml::Value::Integer(0));
    let mut lane1 = toml::Table::new();
    lane1.insert("alias".into(), toml::Value::String("governance".into()));
    lane1.insert("index".into(), toml::Value::Integer(1));
    nexus.insert(
        "lane_catalog".into(),
        toml::Value::Array(vec![toml::Value::Table(lane0), toml::Value::Table(lane1)]),
    );
    let mut nexus = Some(nexus);
    let mut sumeragi = None;
    let mut torii = None;
    normalize_peer_config_overrides(&mut nexus, &mut sumeragi, &mut torii)
        .expect("normalize overrides");
    let nexus = nexus.expect("nexus config");
    assert_eq!(
        nexus.get("lane_count").and_then(toml::Value::as_integer),
        Some(2)
    );
    assert!(sumeragi.is_none());
    let torii = torii.expect("torii config");
    let mcp = torii
        .get("mcp")
        .and_then(toml::Value::as_table)
        .expect("mcp table");
    assert!(matches!(
        mcp.get("enabled"),
        Some(toml::Value::Boolean(true))
    ));
    assert!(matches!(
        mcp.get("profile"),
        Some(toml::Value::String(value)) if value == LOCAL_MCP_PROFILE
    ));
}
#[test]
fn normalize_peer_config_overrides_rejects_disabled_nexus_with_lanes() {
    let mut nexus = toml::Table::new();
    nexus.insert("enabled".into(), toml::Value::Boolean(false));
    nexus.insert("lane_count".into(), toml::Value::Integer(3));
    let mut nexus = Some(nexus);
    let mut sumeragi = None;
    let mut torii = None;
    let err = normalize_peer_config_overrides(&mut nexus, &mut sumeragi, &mut torii)
        .expect_err("disabled nexus should fail");
    match err {
        SupervisorError::Config(message) => assert!(
            message.contains("nexus.enabled = false"),
            "unexpected error: {message}"
        ),
        other => panic!("expected SupervisorError::Config, got {other:?}"),
    }
}
#[test]
fn supervisor_defaults_nexus_disabled_for_local_permissioned_profiles() {
    if !ports_available("supervisor_defaults_nexus_disabled_for_local_permissioned_profiles") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("temp dir");
    let _stub = KagamiStub::install(temp.path());
    let supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    let nexus = supervisor
        .nexus_config_overrides()
        .expect("default nexus overrides");
    assert!(matches!(
        nexus.get("enabled"),
        Some(toml::Value::Boolean(false))
    ));
}
#[test]
fn supervisor_rejects_enabled_nexus_without_npos_consensus() {
    if !ports_available("supervisor_rejects_enabled_nexus_without_npos_consensus") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("temp dir");
    let _stub = KagamiStub::install(temp.path());
    let err = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .nexus_enabled(true)
        .build()
        .expect_err("permissioned localnet should reject nexus");
    match err {
        SupervisorError::Config(message) => assert!(
            message.contains("NPoS signed-genesis consensus mode"),
            "unexpected error: {message}"
        ),
        other => panic!("expected SupervisorError::Config, got {other:?}"),
    }
}
#[test]
fn supervisor_exposes_config_overrides() {
    if !ports_available("supervisor_exposes_config_overrides") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("temp dir");
    let _stub = KagamiStub::install(temp.path());
    let mut nexus = toml::Table::new();
    nexus.insert("enabled".into(), toml::Value::Boolean(true));
    let mut sumeragi = toml::Table::new();
    let mut queues = toml::Table::new();
    queues.insert("commands".into(), toml::Value::Integer(1024));
    sumeragi.insert("queues".into(), toml::Value::Table(queues));
    let mut torii = toml::Table::new();
    torii.insert(
        "address".into(),
        toml::Value::String("127.0.0.1:8080".to_owned()),
    );
    let supervisor =
        SupervisorBuilder::with_profile(npos_preset_profile(ProfilePreset::SinglePeer))
            .data_root(temp.path())
            .nexus_config(nexus)
            .sumeragi_config(sumeragi)
            .torii_config(torii)
            .build()
            .expect("build supervisor");
    let nexus = supervisor
        .nexus_config_overrides()
        .expect("nexus overrides");
    assert!(matches!(
        nexus.get("enabled"),
        Some(toml::Value::Boolean(true))
    ));
    assert_eq!(
        supervisor
            .sumeragi_config_overrides()
            .and_then(|table| table.get("queues"))
            .and_then(toml::Value::as_table)
            .and_then(|queues| queues.get("commands"))
            .and_then(toml::Value::as_integer),
        Some(1024)
    );
    let torii = supervisor
        .torii_config_overrides()
        .expect("torii overrides");
    assert!(matches!(
        torii.get("address"),
        Some(toml::Value::String(value)) if value == "127.0.0.1:8080"
    ));
}
#[test]
fn lane_slug_sanitizes_alias() {
    assert_eq!(lane_slug("Core Lane", 0), "core_lane");
    assert_eq!(lane_slug("Gov+Ops", 2), "gov_ops");
    assert_eq!(lane_slug("---", 3), "lane3");
}
#[test]
fn lane_path_comments_include_default_aliases_for_multilane() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut nexus = toml::Table::new();
    nexus.insert("enabled".into(), toml::Value::Boolean(true));
    nexus.insert("lane_count".into(), toml::Value::Integer(3));
    let comments = lane_path_comments(temp.path(), Some(&nexus));
    assert!(
        comments
            .iter()
            .any(|line| line.contains("mochi.lane[0].alias = default"))
    );
    assert!(
        comments
            .iter()
            .any(|line| line.contains("mochi.lane[1].alias = lane1"))
    );
    assert!(
        comments
            .iter()
            .any(|line| line.contains("mochi.lane[2].alias = lane2"))
    );
}
#[test]
fn peer_spec_writes_nexus_and_always_on_da_storage() {
    let temp = tempfile::tempdir().expect("temp dir");
    let profile = NetworkProfile::default();
    let paths = NetworkPaths::from_root(temp.path(), &profile);
    paths.ensure().expect("paths");
    let spec = test_peer_spec(&paths, "peer0".into(), 8080, 1337).expect("peer spec");
    let genesis = test_genesis_material(&paths);
    let mut nexus = toml::Table::new();
    nexus.insert("enabled".into(), toml::Value::Boolean(true));
    nexus.insert("lane_count".into(), toml::Value::Integer(1));
    let overrides = PeerConfigOverrides {
        nexus: Some(nexus),
        sumeragi: None,
        torii: None,
    };
    let specs = vec![spec.clone()];
    spec.write_config("demo-chain", &genesis, &specs, &overrides, &[])
        .expect("write config");
    let contents = fs::read_to_string(&spec.config_path).expect("read config");
    let value: toml::Table = toml::from_str(&contents).expect("parse config");
    let nexus = value
        .get("nexus")
        .and_then(toml::Value::as_table)
        .expect("nexus table");
    assert!(matches!(
        nexus.get("enabled"),
        Some(toml::Value::Boolean(true))
    ));
    let torii = value
        .get("torii")
        .and_then(toml::Value::as_table)
        .expect("torii table");
    let mcp = torii
        .get("mcp")
        .and_then(toml::Value::as_table)
        .expect("mcp table");
    assert!(matches!(
        mcp.get("enabled"),
        Some(toml::Value::Boolean(true))
    ));
    assert!(matches!(
        mcp.get("profile"),
        Some(toml::Value::String(value)) if value == LOCAL_MCP_PROFILE
    ));
    let expected_torii_dir = spec.storage_dir.join("torii").display().to_string();
    assert_eq!(
        torii.get("data_dir").and_then(toml::Value::as_str),
        Some(expected_torii_dir.as_str())
    );
    let da_ingest = torii
        .get("da_ingest")
        .and_then(toml::Value::as_table)
        .expect("da_ingest table");
    let expected_replay = spec
        .storage_dir
        .join("torii")
        .join("da_replay")
        .display()
        .to_string();
    assert_eq!(
        da_ingest
            .get("replay_cache_store_dir")
            .and_then(toml::Value::as_str),
        Some(expected_replay.as_str())
    );
    let expected_manifest = spec
        .storage_dir
        .join("torii")
        .join("da_manifests")
        .display()
        .to_string();
    assert_eq!(
        da_ingest
            .get("manifest_store_dir")
            .and_then(toml::Value::as_str),
        Some(expected_manifest.as_str())
    );
}
#[test]
fn peer_specs_write_distinct_managed_sorafs_state_roots() {
    let temp = tempfile::tempdir().expect("temp dir");
    let profile = NetworkProfile::from_preset(ProfilePreset::FourPeerBft);
    let paths = NetworkPaths::from_root(temp.path(), &profile);
    paths.ensure().expect("paths");
    let specs = (0_u16..4)
        .map(|index| {
            test_peer_spec(&paths, format!("peer{index}"), 8_080 + index, 1_337 + index)
                .expect("peer spec")
        })
        .collect::<Vec<_>>();
    let genesis = test_genesis_material(&paths);
    for spec in &specs {
        spec.write_config(
            "demo-chain",
            &genesis,
            &specs,
            &PeerConfigOverrides::default(),
            &[],
        )
        .expect("write config");
    }
    let mut configured_roots = HashSet::new();
    for spec in &specs {
        let contents = fs::read_to_string(&spec.config_path).expect("read config");
        let value: toml::Table = toml::from_str(&contents).expect("parse config");
        let configured = value
            .get("sorafs")
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("storage"))
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("data_dir"))
            .and_then(toml::Value::as_str)
            .expect("managed SoraFS data directory");
        let expected = spec.storage_dir.join("sorafs").display().to_string();
        assert_eq!(configured, expected);
        assert!(
            configured_roots.insert(configured.to_owned()),
            "each peer must own a distinct SoraFS checkpoint root"
        );
    }
    assert_eq!(configured_roots.len(), specs.len());
}
#[test]
fn peer_spec_preserves_managed_sorafs_root_when_overlay_enables_storage() {
    let temp = tempfile::tempdir().expect("temp dir");
    let profile = NetworkProfile::default();
    let paths = NetworkPaths::from_root(temp.path(), &profile);
    paths.ensure().expect("paths");
    let spec = test_peer_spec(&paths, "peer0".into(), 8080, 1337).expect("peer spec");
    let genesis = test_genesis_material(&paths);
    let mut storage = toml::Table::new();
    storage.insert("enabled".into(), toml::Value::Boolean(true));
    let mut sorafs = toml::Table::new();
    sorafs.insert("storage".into(), toml::Value::Table(storage));
    let mut overlay = toml::Table::new();
    overlay.insert("sorafs".into(), toml::Value::Table(sorafs));
    spec.write_config(
        "demo-chain",
        &genesis,
        std::slice::from_ref(&spec),
        &PeerConfigOverrides::default(),
        &[overlay],
    )
    .expect("write config");
    let contents = fs::read_to_string(&spec.config_path).expect("read config");
    let value: toml::Table = toml::from_str(&contents).expect("parse config");
    let storage = value
        .get("sorafs")
        .and_then(toml::Value::as_table)
        .and_then(|table| table.get("storage"))
        .and_then(toml::Value::as_table)
        .expect("SoraFS storage config");
    assert_eq!(
        storage.get("enabled").and_then(toml::Value::as_bool),
        Some(true)
    );
    let expected = spec.storage_dir.join("sorafs").display().to_string();
    assert_eq!(
        storage.get("data_dir").and_then(toml::Value::as_str),
        Some(expected.as_str())
    );
}
#[test]
fn peer_spec_rejects_sorafs_state_root_override() {
    let temp = tempfile::tempdir().expect("temp dir");
    let profile = NetworkProfile::default();
    let paths = NetworkPaths::from_root(temp.path(), &profile);
    paths.ensure().expect("paths");
    let spec = test_peer_spec(&paths, "peer0".into(), 8080, 1337).expect("peer spec");
    let genesis = test_genesis_material(&paths);
    let mut storage = toml::Table::new();
    storage.insert(
        "data_dir".into(),
        toml::Value::String("/tmp/shared-sorafs".to_owned()),
    );
    let mut sorafs = toml::Table::new();
    sorafs.insert("storage".into(), toml::Value::Table(storage));
    let mut overlay = toml::Table::new();
    overlay.insert("sorafs".into(), toml::Value::Table(sorafs));
    let err = spec
        .write_config(
            "demo-chain",
            &genesis,
            std::slice::from_ref(&spec),
            &PeerConfigOverrides::default(),
            &[overlay],
        )
        .expect_err("SoraFS root override must fail closed");
    match err {
        SupervisorError::Config(message) => assert!(
            message.contains("must preserve Mochi's managed SoraFS root")
                && message.contains(spec.storage_dir.to_string_lossy().as_ref()),
            "unexpected error: {message}"
        ),
        other => panic!("expected SupervisorError::Config, got {other:?}"),
    }
}
#[test]
fn peer_specs_write_distinct_managed_streaming_state_roots() {
    let temp = tempfile::tempdir().expect("temp dir");
    let profile = NetworkProfile::from_preset(ProfilePreset::FourPeerBft);
    let paths = NetworkPaths::from_root(temp.path(), &profile);
    paths.ensure().expect("paths");
    let specs = (0_u16..4)
        .map(|index| {
            test_peer_spec(&paths, format!("peer{index}"), 8_080 + index, 1_337 + index)
                .expect("peer spec")
        })
        .collect::<Vec<_>>();
    let genesis = test_genesis_material(&paths);
    for spec in &specs {
        spec.write_config(
            "demo-chain",
            &genesis,
            &specs,
            &PeerConfigOverrides::default(),
            &[],
        )
        .expect("write config");
    }
    let mut session_roots = HashSet::new();
    let mut soranet_roots = HashSet::new();
    let mut soravpn_roots = HashSet::new();
    for spec in &specs {
        let contents = fs::read_to_string(&spec.config_path).expect("read config");
        let value: toml::Table = toml::from_str(&contents).expect("parse config");
        let streaming = value
            .get("streaming")
            .and_then(toml::Value::as_table)
            .expect("streaming config");
        let session = streaming
            .get("session_store_dir")
            .and_then(toml::Value::as_str)
            .expect("managed streaming session directory");
        let soranet = streaming
            .get("soranet")
            .and_then(toml::Value::as_table)
            .expect("SoraNet streaming config");
        let soranet_spool = soranet
            .get("provision_spool_dir")
            .and_then(toml::Value::as_str)
            .expect("managed SoraNet spool directory");
        let soravpn_spool = streaming
            .get("soravpn")
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("provision_spool_dir"))
            .and_then(toml::Value::as_str)
            .expect("managed SoraVPN spool directory");
        let expected = spec
            .storage_dir
            .canonicalize()
            .expect("storage root")
            .join("streaming");
        assert_eq!(Path::new(session), expected);
        assert_eq!(Path::new(soranet_spool), expected.join("soranet_routes"));
        assert_eq!(Path::new(soravpn_spool), expected.join("soravpn_routes"));
        assert!(Path::new(session).is_absolute());
        assert!(Path::new(soranet_spool).is_absolute());
        assert!(Path::new(soravpn_spool).is_absolute());
        assert_eq!(
            soranet.get("enabled").and_then(toml::Value::as_bool),
            Some(false)
        );
        for required in [
            "exit_multiaddr",
            "padding_budget_ms",
            "access_kind",
            "channel_salt",
            "provision_spool_max_bytes",
            "provision_window_segments",
            "provision_queue_capacity",
        ] {
            assert!(
                soranet.contains_key(required),
                "generated streaming.soranet is missing required field {required}"
            );
        }
        let soravpn = streaming
            .get("soravpn")
            .and_then(toml::Value::as_table)
            .expect("SoraVPN streaming config");
        assert!(soravpn.contains_key("provision_spool_max_bytes"));
        assert!(session_roots.insert(session.to_owned()));
        assert!(soranet_roots.insert(soranet_spool.to_owned()));
        assert!(soravpn_roots.insert(soravpn_spool.to_owned()));
    }
    assert_eq!(session_roots.len(), specs.len());
    assert_eq!(soranet_roots.len(), specs.len());
    assert_eq!(soravpn_roots.len(), specs.len());
}
#[test]
fn peer_specs_stage_distinct_rans_tables_and_write_absolute_paths() {
    let temp = tempfile::tempdir().expect("temp dir");
    let profile = NetworkProfile::from_preset(ProfilePreset::FourPeerBft);
    let paths = NetworkPaths::from_root(temp.path(), &profile);
    paths.ensure().expect("paths");
    let specs = (0_u16..4)
        .map(|index| {
            test_peer_spec(&paths, format!("peer{index}"), 8_080 + index, 1_337 + index)
                .expect("peer spec")
        })
        .collect::<Vec<_>>();
    let genesis = test_genesis_material(&paths);
    for spec in &specs {
        spec.write_config(
            "demo-chain",
            &genesis,
            &specs,
            &PeerConfigOverrides::default(),
            &[],
        )
        .expect("write config");
    }
    let mut configured_paths = HashSet::new();
    for spec in &specs {
        let contents = fs::read_to_string(&spec.config_path).expect("read config");
        let value: toml::Table = toml::from_str(&contents).expect("parse config");
        let codec = value
            .get("streaming")
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("codec"))
            .and_then(toml::Value::as_table)
            .expect("streaming codec config");
        assert_eq!(
            codec.get("cabac_mode").and_then(toml::Value::as_str),
            Some("disabled")
        );
        assert!(
            codec
                .get("trellis_blocks")
                .and_then(toml::Value::as_array)
                .is_some_and(|blocks| blocks.is_empty())
        );
        assert_eq!(
            codec.get("entropy_mode").and_then(toml::Value::as_str),
            Some("rans_bundled")
        );
        assert_eq!(
            codec.get("bundle_width").and_then(toml::Value::as_integer),
            Some(2)
        );
        assert_eq!(
            codec.get("bundle_accel").and_then(toml::Value::as_str),
            Some("none")
        );
        let configured = codec
            .get("rans_tables_path")
            .and_then(toml::Value::as_str)
            .expect("managed rANS tables path");
        let configured = Path::new(configured);
        assert!(configured.is_absolute());
        assert_eq!(configured, spec.rans_tables_path);
        assert!(configured.is_file());
        assert_eq!(
            fs::read(configured).expect("read staged rANS tables"),
            MANAGED_RANS_SEED0_TABLE
        );
        let tables = norito::streaming::codec::load_bundle_tables_from_toml(configured)
            .expect("parse staged SignedRansTablesV1");
        assert!(tables.max_width() >= 2);
        assert!(
            configured_paths.insert(configured.to_path_buf()),
            "each peer must reference a distinct staged rANS table"
        );
    }
    assert_eq!(configured_paths.len(), specs.len());
}
#[test]
fn peer_spec_preserves_managed_streaming_roots_with_shallow_opt_in_overlay() {
    let temp = tempfile::tempdir().expect("temp dir");
    let profile = NetworkProfile::default();
    let paths = NetworkPaths::from_root(temp.path(), &profile);
    paths.ensure().expect("paths");
    let spec = test_peer_spec(&paths, "peer0".into(), 8080, 1337).expect("peer spec");
    let genesis = test_genesis_material(&paths);
    let mut soranet = toml::Table::new();
    soranet.insert("enabled".into(), toml::Value::Boolean(true));
    soranet.insert(
        "exit_multiaddr".into(),
        toml::Value::String("/dns/example.test/tcp/443".to_owned()),
    );
    let mut soravpn = toml::Table::new();
    soravpn.insert(
        "provision_spool_max_bytes".into(),
        toml::Value::Integer(4096),
    );
    let mut codec = toml::Table::new();
    codec.insert(
        "cabac_mode".into(),
        toml::Value::String("adaptive".to_owned()),
    );
    codec.insert(
        "trellis_blocks".into(),
        toml::Value::Array(vec![toml::Value::Integer(16), toml::Value::Integer(32)]),
    );
    codec.insert(
        "entropy_mode".into(),
        toml::Value::String("rans-bundled".to_owned()),
    );
    codec.insert("bundle_width".into(), toml::Value::Integer(3));
    codec.insert(
        "bundle_accel".into(),
        toml::Value::String("cpu_simd".to_owned()),
    );
    let mut streaming = toml::Table::new();
    streaming.insert("feature_bits".into(), toml::Value::Integer(7));
    streaming.insert("codec".into(), toml::Value::Table(codec));
    streaming.insert("soranet".into(), toml::Value::Table(soranet));
    streaming.insert("soravpn".into(), toml::Value::Table(soravpn));
    let mut overlay = toml::Table::new();
    overlay.insert("streaming".into(), toml::Value::Table(streaming));
    spec.write_config(
        "demo-chain",
        &genesis,
        std::slice::from_ref(&spec),
        &PeerConfigOverrides::default(),
        &[overlay],
    )
    .expect("write config");
    let contents = fs::read_to_string(&spec.config_path).expect("read config");
    let value: toml::Table = toml::from_str(&contents).expect("parse config");
    let streaming = value
        .get("streaming")
        .and_then(toml::Value::as_table)
        .expect("streaming config");
    let expected = spec
        .storage_dir
        .canonicalize()
        .expect("storage root")
        .join("streaming");
    assert_eq!(
        streaming
            .get("session_store_dir")
            .and_then(toml::Value::as_str),
        Some(expected.to_string_lossy().as_ref())
    );
    assert_eq!(
        streaming
            .get("feature_bits")
            .and_then(toml::Value::as_integer),
        Some(7)
    );
    assert!(streaming.contains_key("identity_public_key"));
    assert!(streaming.contains_key("identity_private_key"));
    let codec = streaming
        .get("codec")
        .and_then(toml::Value::as_table)
        .expect("streaming codec config");
    assert_eq!(
        codec.get("cabac_mode").and_then(toml::Value::as_str),
        Some("adaptive")
    );
    let trellis_blocks = codec
        .get("trellis_blocks")
        .and_then(toml::Value::as_array)
        .expect("trellis block override");
    assert_eq!(trellis_blocks.len(), 2);
    assert_eq!(trellis_blocks[0].as_integer(), Some(16));
    assert_eq!(trellis_blocks[1].as_integer(), Some(32));
    assert_eq!(
        codec.get("entropy_mode").and_then(toml::Value::as_str),
        Some("rans-bundled")
    );
    assert_eq!(
        codec.get("bundle_width").and_then(toml::Value::as_integer),
        Some(3)
    );
    assert_eq!(
        codec.get("bundle_accel").and_then(toml::Value::as_str),
        Some("cpu_simd")
    );
    let soranet = streaming
        .get("soranet")
        .and_then(toml::Value::as_table)
        .expect("SoraNet config");
    assert_eq!(
        soranet.get("enabled").and_then(toml::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        soranet.get("exit_multiaddr").and_then(toml::Value::as_str),
        Some("/dns/example.test/tcp/443")
    );
    assert_eq!(
        soranet
            .get("provision_spool_dir")
            .and_then(toml::Value::as_str),
        Some(expected.join("soranet_routes").to_string_lossy().as_ref())
    );
    let soravpn = streaming
        .get("soravpn")
        .and_then(toml::Value::as_table)
        .expect("SoraVPN config");
    assert_eq!(
        soravpn
            .get("provision_spool_max_bytes")
            .and_then(toml::Value::as_integer),
        Some(4096)
    );
    assert_eq!(
        soravpn
            .get("provision_spool_dir")
            .and_then(toml::Value::as_str),
        Some(expected.join("soravpn_routes").to_string_lossy().as_ref())
    );
}
#[test]
fn peer_spec_rejects_managed_streaming_state_redirects() {
    let temp = tempfile::tempdir().expect("temp dir");
    let profile = NetworkProfile::default();
    let paths = NetworkPaths::from_root(temp.path(), &profile);
    paths.ensure().expect("paths");
    let spec = test_peer_spec(&paths, "peer0".into(), 8080, 1337).expect("peer spec");
    let genesis = test_genesis_material(&paths);
    for (key, nested, expected_error) in [
        ("session_store_dir", None, "managed streaming session root"),
        (
            "provision_spool_dir",
            Some("soranet"),
            "managed SoraNet provision spool",
        ),
        (
            "provision_spool_dir",
            Some("soravpn"),
            "managed SoraVPN provision spool",
        ),
    ] {
        let redirect = toml::Value::String("/tmp/shared-streaming-state".to_owned());
        let mut streaming = toml::Table::new();
        if let Some(section) = nested {
            let mut table = toml::Table::new();
            table.insert(key.into(), redirect);
            streaming.insert(section.into(), toml::Value::Table(table));
        } else {
            streaming.insert(key.into(), redirect);
        }
        let mut overlay = toml::Table::new();
        overlay.insert("streaming".into(), toml::Value::Table(streaming));
        let err = spec
            .write_config(
                "demo-chain",
                &genesis,
                std::slice::from_ref(&spec),
                &PeerConfigOverrides::default(),
                &[overlay],
            )
            .expect_err("managed streaming redirect must be rejected");
        match err {
            SupervisorError::Config(message) => assert!(
                message.contains(expected_error)
                    && message.contains(spec.storage_dir.to_string_lossy().as_ref()),
                "unexpected error: {message}"
            ),
            other => panic!("expected SupervisorError::Config, got {other:?}"),
        }
    }
}
#[test]
fn peer_spec_config_honors_torii_da_ingest_overrides() {
    let temp = tempfile::tempdir().expect("temp dir");
    let profile = NetworkProfile::default();
    let paths = NetworkPaths::from_root(temp.path(), &profile);
    paths.ensure().expect("paths");
    let spec = test_peer_spec(&paths, "peer0".into(), 8080, 1337).expect("peer spec");
    let genesis = test_genesis_material(&paths);
    let mut da_ingest = toml::Table::new();
    da_ingest.insert(
        "replay_cache_store_dir".into(),
        toml::Value::String("/custom/replay".to_owned()),
    );
    da_ingest.insert(
        "manifest_store_dir".into(),
        toml::Value::String("/custom/manifests".to_owned()),
    );
    let mut torii = toml::Table::new();
    torii.insert("da_ingest".into(), toml::Value::Table(da_ingest));
    let overrides = PeerConfigOverrides {
        nexus: None,
        sumeragi: None,
        torii: Some(torii),
    };
    let specs = vec![spec.clone()];
    spec.write_config("demo-chain", &genesis, &specs, &overrides, &[])
        .expect("write config");
    let contents = fs::read_to_string(&spec.config_path).expect("read config");
    let value: toml::Table = toml::from_str(&contents).expect("parse config");
    let torii = value
        .get("torii")
        .and_then(toml::Value::as_table)
        .expect("torii table");
    let da_ingest = torii
        .get("da_ingest")
        .and_then(toml::Value::as_table)
        .expect("da_ingest table");
    assert_eq!(
        da_ingest
            .get("replay_cache_store_dir")
            .and_then(toml::Value::as_str),
        Some("/custom/replay")
    );
    assert_eq!(
        da_ingest
            .get("manifest_store_dir")
            .and_then(toml::Value::as_str),
        Some("/custom/manifests")
    );
}
#[test]
fn peer_spec_rejects_kura_store_override() {
    let temp = tempfile::tempdir().expect("temp dir");
    let profile = NetworkProfile::default();
    let paths = NetworkPaths::from_root(temp.path(), &profile);
    paths.ensure().expect("paths");
    let spec = test_peer_spec(&paths, "peer0".into(), 8080, 1337).expect("peer spec");
    let genesis = test_genesis_material(&paths);
    let mut kura = toml::Table::new();
    kura.insert(
        "store_dir".into(),
        toml::Value::String("/tmp/unmanaged-kura".into()),
    );
    let mut overlay = toml::Table::new();
    overlay.insert("kura".into(), toml::Value::Table(kura));
    let err = spec
        .write_config(
            "demo-chain",
            &genesis,
            std::slice::from_ref(&spec),
            &PeerConfigOverrides::default(),
            &[overlay],
        )
        .expect_err("Kura root override must fail closed");
    match err {
        SupervisorError::Config(message) => assert!(
            message.contains("must preserve Mochi's managed Kura root")
                && message.contains(spec.kura_dir.to_string_lossy().as_ref()),
            "unexpected error: {message}"
        ),
        other => panic!("expected SupervisorError::Config, got {other:?}"),
    }
}
#[test]
fn peer_spec_rejects_non_string_kura_store_overlay() {
    let temp = tempfile::tempdir().expect("temp dir");
    let profile = NetworkProfile::default();
    let paths = NetworkPaths::from_root(temp.path(), &profile);
    paths.ensure().expect("paths");
    let spec = test_peer_spec(&paths, "peer0".into(), 8080, 1337).expect("peer spec");
    let genesis = test_genesis_material(&paths);
    let mut kura = toml::Table::new();
    kura.insert("store_dir".into(), toml::Value::Integer(7));
    let mut overlay = toml::Table::new();
    overlay.insert("kura".into(), toml::Value::Table(kura));
    let err = spec
        .write_config(
            "demo-chain",
            &genesis,
            std::slice::from_ref(&spec),
            &PeerConfigOverrides::default(),
            &[overlay],
        )
        .expect_err("malformed Kura root override must fail closed");
    match err {
        SupervisorError::Config(message) => assert!(
            message.contains("must preserve Mochi's managed Kura root"),
            "unexpected error: {message}"
        ),
        other => panic!("expected SupervisorError::Config, got {other:?}"),
    }
}
#[test]
fn peer_spec_config_header_includes_lane_paths() {
    let temp = tempfile::tempdir().expect("temp dir");
    let profile = NetworkProfile::default();
    let paths = NetworkPaths::from_root(temp.path(), &profile);
    paths.ensure().expect("paths");
    let spec = test_peer_spec(&paths, "peer0".into(), 8080, 1337).expect("peer spec");
    let genesis = test_genesis_material(&paths);
    let mut lane0 = toml::Table::new();
    lane0.insert("alias".into(), toml::Value::String("Core Lane".into()));
    lane0.insert("index".into(), toml::Value::Integer(0));
    let mut lane1 = toml::Table::new();
    lane1.insert("alias".into(), toml::Value::String("Gov+Ops".into()));
    lane1.insert("index".into(), toml::Value::Integer(1));
    let mut nexus = toml::Table::new();
    nexus.insert("enabled".into(), toml::Value::Boolean(true));
    nexus.insert("lane_count".into(), toml::Value::Integer(2));
    nexus.insert(
        "lane_catalog".into(),
        toml::Value::Array(vec![toml::Value::Table(lane0), toml::Value::Table(lane1)]),
    );
    let overrides = PeerConfigOverrides {
        nexus: Some(nexus),
        sumeragi: None,
        torii: None,
    };
    let specs = vec![spec.clone()];
    spec.write_config("demo-chain", &genesis, &specs, &overrides, &[])
        .expect("write config");
    let contents = fs::read_to_string(&spec.config_path).expect("read config");
    let lane0_slug = lane_slug("Core Lane", 0);
    let lane1_slug = lane_slug("Gov+Ops", 1);
    let lane0_blocks = spec
        .kura_dir
        .join("blocks")
        .join(format!("lane_000_{lane0_slug}"))
        .display()
        .to_string();
    let lane1_blocks = spec
        .kura_dir
        .join("blocks")
        .join(format!("lane_001_{lane1_slug}"))
        .display()
        .to_string();
    let lane0_merge = spec
        .kura_dir
        .join("merge_ledger")
        .join(format!("lane_000_{lane0_slug}_merge.log"))
        .display()
        .to_string();
    let lane1_merge = spec
        .kura_dir
        .join("merge_ledger")
        .join(format!("lane_001_{lane1_slug}_merge.log"))
        .display()
        .to_string();
    assert!(contents.contains("# mochi.lane[0].alias = Core Lane"));
    assert!(contents.contains("# mochi.lane[1].alias = Gov+Ops"));
    assert!(contents.contains(&format!("# mochi.lane[0].blocks_dir = {lane0_blocks}")));
    assert!(contents.contains(&format!("# mochi.lane[1].blocks_dir = {lane1_blocks}")));
    assert!(contents.contains(&format!("# mochi.lane[0].merge_log = {lane0_merge}")));
    assert!(contents.contains(&format!("# mochi.lane[1].merge_log = {lane1_merge}")));
}
#[cfg(unix)]
fn onboarding_test_paths(root: &Path, name: &str) -> NetworkPaths {
    let profile = NetworkProfile::from_preset(ProfilePreset::SinglePeer);
    let paths = NetworkPaths::from_root(root.join(name), &profile);
    paths.ensure().expect("create onboarding test paths");
    paths
}
#[cfg(unix)]
fn write_private_test_file(path: &Path, payload: &[u8]) {
    fs::write(path, payload).expect("write private test file");
    let mut permissions = fs::metadata(path)
        .expect("private test file metadata")
        .permissions();
    permissions.set_mode(0o600);
    fs::set_permissions(path, permissions).expect("set private test file permissions");
}
#[test]
#[cfg(unix)]
fn onboarding_bundle_reuses_existing_material_without_rotation() {
    let temp = tempfile::tempdir().expect("temp dir");
    let paths = onboarding_test_paths(temp.path(), "stable");
    let authority = localnet_admin_signer().expect("localnet admin");
    let first =
        OnboardingRuntimeBundle::create(&paths, authority).expect("create first onboarding bundle");
    let signer_before = fs::read(&first.private_key_file).expect("read signer");
    let token_before = fs::read(&first.token_file).expect("read token");
    let signer_inode = fs::metadata(&first.private_key_file)
        .expect("signer metadata")
        .ino();
    let token_inode = fs::metadata(&first.token_file)
        .expect("token metadata")
        .ino();
    let second =
        OnboardingRuntimeBundle::create(&paths, authority).expect("reuse onboarding bundle");
    assert_eq!(second.token_hash, first.token_hash);
    assert_eq!(second.private_key_file, first.private_key_file);
    assert_eq!(second.token_file, first.token_file);
    assert_eq!(fs::read(&second.private_key_file).unwrap(), signer_before);
    assert_eq!(fs::read(&second.token_file).unwrap(), token_before);
    assert_eq!(
        fs::metadata(&second.private_key_file).unwrap().ino(),
        signer_inode,
        "valid signer must not be replaced"
    );
    assert_eq!(
        fs::metadata(&second.token_file).unwrap().ino(),
        token_inode,
        "valid token must not be rotated or replaced"
    );
}
#[test]
#[cfg(unix)]
fn onboarding_bundle_reuses_dpn_tokens_and_hashes_normalized_body() {
    let temp = tempfile::tempdir().expect("temp dir");
    let authority = localnet_admin_signer().expect("localnet admin");
    let private_key = ExposedPrivateKey(authority.key_pair().private_key().clone());
    let signer_payload = format!("{private_key}\n");
    let token_body = format!("nevo-local-{}", "A".repeat(48));
    for (index, terminator) in [b"\n".as_slice(), b"\r\n".as_slice()]
        .into_iter()
        .enumerate()
    {
        let paths = onboarding_test_paths(temp.path(), &format!("dpn-{index}"));
        let runtime = paths.root().join(LOCAL_ONBOARDING_RUNTIME_DIRECTORY);
        fs::create_dir(&runtime).expect("create runtime");
        fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700))
            .expect("set runtime permissions");
        let signer = runtime.join(LOCAL_ONBOARDING_SIGNER_KEY_FILE);
        let token = runtime.join(LOCAL_ONBOARDING_TOKEN_FILE);
        write_private_test_file(&signer, signer_payload.as_bytes());
        let mut persisted_token = token_body.as_bytes().to_vec();
        persisted_token.extend_from_slice(terminator);
        write_private_test_file(&token, &persisted_token);
        let manifest = runtime.join("onboarding.json");
        write_private_test_file(&manifest, b"legacy DPN-owned manifest\n");
        let signer_inode = fs::metadata(&signer).unwrap().ino();
        let token_inode = fs::metadata(&token).unwrap().ino();
        let manifest_inode = fs::metadata(&manifest).unwrap().ino();
        let bundle = OnboardingRuntimeBundle::create(&paths, authority)
            .expect("reuse DPN onboarding material");
        assert_eq!(
            bundle.token_hash,
            *blake3::hash(token_body.as_bytes()).as_bytes()
        );
        assert_eq!(fs::read(&signer).unwrap(), signer_payload.as_bytes());
        assert_eq!(fs::read(&token).unwrap(), persisted_token);
        assert_eq!(fs::metadata(&signer).unwrap().ino(), signer_inode);
        assert_eq!(fs::metadata(&token).unwrap().ino(), token_inode);
        assert_eq!(fs::read(&manifest).unwrap(), b"legacy DPN-owned manifest\n");
        assert_eq!(fs::metadata(&manifest).unwrap().ino(), manifest_inode);
    }
}
#[test]
#[cfg(unix)]
fn onboarding_bundle_rejects_conflicting_signer_without_mutation() {
    let temp = tempfile::tempdir().expect("temp dir");
    let paths = onboarding_test_paths(temp.path(), "conflict");
    let authority = localnet_admin_signer().expect("localnet admin");
    let first =
        OnboardingRuntimeBundle::create(&paths, authority).expect("create onboarding bundle");
    write_private_test_file(
        &first.private_key_file,
        b"802620ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff\n",
    );
    let signer_before = fs::read(&first.private_key_file).unwrap();
    let token_before = fs::read(&first.token_file).unwrap();
    let error = OnboardingRuntimeBundle::create(&paths, authority)
        .expect_err("conflicting signer must fail");
    assert!(matches!(
        error,
        SupervisorError::Config(message) if message.contains("signer conflicts")
    ));
    assert_eq!(fs::read(&first.private_key_file).unwrap(), signer_before);
    assert_eq!(fs::read(&first.token_file).unwrap(), token_before);
}
#[test]
#[cfg(unix)]
fn onboarding_bundle_rejects_partial_material_without_completion() {
    let temp = tempfile::tempdir().expect("temp dir");
    let authority = localnet_admin_signer().expect("localnet admin");
    let private_key = ExposedPrivateKey(authority.key_pair().private_key().clone());
    let signer_payload = format!("{private_key}\n");
    for signer_only in [true, false] {
        let paths = onboarding_test_paths(
            temp.path(),
            if signer_only {
                "partial-signer"
            } else {
                "partial-token"
            },
        );
        let runtime = paths.root().join(LOCAL_ONBOARDING_RUNTIME_DIRECTORY);
        fs::create_dir(&runtime).expect("create runtime");
        fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700))
            .expect("set runtime permissions");
        let signer = runtime.join(LOCAL_ONBOARDING_SIGNER_KEY_FILE);
        let token = runtime.join(LOCAL_ONBOARDING_TOKEN_FILE);
        if signer_only {
            write_private_test_file(&signer, signer_payload.as_bytes());
        } else {
            write_private_test_file(
                &token,
                b"nevo-local-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            );
        }
        let error = OnboardingRuntimeBundle::create(&paths, authority)
            .expect_err("partial bundle must fail");
        assert!(matches!(
            error,
            SupervisorError::Config(message) if message.contains("must either both exist or both be absent")
        ));
        assert_eq!(signer.exists(), signer_only);
        assert_eq!(token.exists(), !signer_only);
    }
}
#[test]
#[cfg(unix)]
fn onboarding_bundle_rejects_unsafe_private_file_modes() {
    let temp = tempfile::tempdir().expect("temp dir");
    let authority = localnet_admin_signer().expect("localnet admin");
    for target_signer in [true, false] {
        let paths = onboarding_test_paths(
            temp.path(),
            if target_signer {
                "unsafe-signer"
            } else {
                "unsafe-token"
            },
        );
        let bundle =
            OnboardingRuntimeBundle::create(&paths, authority).expect("create onboarding bundle");
        let target = if target_signer {
            &bundle.private_key_file
        } else {
            &bundle.token_file
        };
        fs::set_permissions(target, fs::Permissions::from_mode(0o644))
            .expect("make private file unsafe");
        let error = OnboardingRuntimeBundle::create(&paths, authority)
            .expect_err("unsafe private file must fail");
        assert!(matches!(
            error,
            SupervisorError::Config(message) if message.contains("owner-only 0600")
        ));
        assert_eq!(
            fs::metadata(target).unwrap().permissions().mode() & 0o777,
            0o644,
            "validation must not silently repair an unsafe file"
        );
    }
}
#[test]
#[cfg(unix)]
fn onboarding_bundle_rejects_malformed_tokens() {
    let temp = tempfile::tempdir().expect("temp dir");
    let authority = localnet_admin_signer().expect("localnet admin");
    let private_key = ExposedPrivateKey(authority.key_pair().private_key().clone());
    let signer_payload = format!("{private_key}\n");
    let malformed_tokens = [
        b"too-short".as_slice(),
        b"nevo-local-AAAAAAAAAAAAAAAAAAAA AAAAAAAAAAAAAAAAAAAAAAAAAAA".as_slice(),
        b"nevo-local-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\n\n".as_slice(),
    ];
    for (index, malformed_token) in malformed_tokens.into_iter().enumerate() {
        let paths = onboarding_test_paths(temp.path(), &format!("malformed-{index}"));
        let runtime = paths.root().join(LOCAL_ONBOARDING_RUNTIME_DIRECTORY);
        fs::create_dir(&runtime).expect("create runtime");
        fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700))
            .expect("set runtime permissions");
        write_private_test_file(
            &runtime.join(LOCAL_ONBOARDING_SIGNER_KEY_FILE),
            signer_payload.as_bytes(),
        );
        write_private_test_file(&runtime.join(LOCAL_ONBOARDING_TOKEN_FILE), malformed_token);
        let error = OnboardingRuntimeBundle::create(&paths, authority)
            .expect_err("malformed token must fail");
        assert!(matches!(
            error,
            SupervisorError::Config(message) if message.contains("32 through 256 printable")
        ));
    }
}
#[test]
#[cfg(unix)]
fn four_peer_onboarding_bundle_is_private_identical_and_session_metadata_is_safe() {
    if !ports_available(
        "four_peer_onboarding_bundle_is_private_identical_and_session_metadata_is_safe",
    ) {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("temp dir");
    let _stub = KagamiStub::install(temp.path());
    let supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
        .data_root(temp.path().join("sandbox"))
        .torii_base_port(24_000)
        .p2p_base_port(25_000)
        .build()
        .expect("build four-peer supervisor");
    let session = supervisor.session_info().expect("session info");
    assert_eq!(
        session.onboarding_token_file,
        supervisor.onboarding.token_file
    );
    assert_eq!(
        session.onboarding_credential_id,
        LOCAL_ONBOARDING_CREDENTIAL_ID
    );
    assert_eq!(
        session.onboarding_signer_file,
        supervisor.onboarding.private_key_file
    );
    assert!(session.onboarding_token_file.is_absolute());
    let token =
        fs::read_to_string(&session.onboarding_token_file).expect("read private onboarding token");
    assert!(token.starts_with("iroha-localnet-"));
    assert!((32..=256).contains(&token.len()));
    assert!(token.bytes().all(|byte| (b'!'..=b'~').contains(&byte)));
    assert_eq!(token, token.trim_end());
    let runtime_dir = session
        .onboarding_token_file
        .parent()
        .expect("runtime directory");
    assert_eq!(
        fs::metadata(runtime_dir)
            .expect("runtime metadata")
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    for private_file in [
        &supervisor.onboarding.private_key_file,
        &supervisor.onboarding.token_file,
    ] {
        let metadata = fs::metadata(private_file).expect("private file metadata");
        assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
        assert_eq!(metadata.nlink(), 1);
    }
    let admin = localnet_admin_signer().expect("localnet admin");
    let admin_account = admin.account_id().to_string();
    let admin_private = ExposedPrivateKey(admin.key_pair().private_key().clone()).to_string();
    assert_eq!(
        fs::read_to_string(&supervisor.onboarding.private_key_file)
            .expect("read onboarding signer")
            .trim_end(),
        admin_private
    );
    let expected_digest = format!("blake3:{}", blake3::hash(token.as_bytes()).to_hex());
    let mut expected_onboarding = None;
    for peer in supervisor.peers() {
        let config_text = fs::read_to_string(peer.config_path()).expect("read peer config");
        assert!(!config_text.contains(&token));
        assert!(!config_text.contains(&admin_private));
        let config: toml::Table = toml::from_str(&config_text).expect("parse peer config");
        let onboarding = config
            .get("torii")
            .and_then(toml::Value::as_table)
            .and_then(|torii| torii.get("account_onboarding"))
            .and_then(toml::Value::as_table)
            .expect("managed account onboarding");
        assert_eq!(
            onboarding.get("authority").and_then(toml::Value::as_str),
            Some(admin_account.as_str())
        );
        assert_eq!(
            onboarding
                .get("private_key_file")
                .and_then(toml::Value::as_str),
            Some(
                supervisor
                    .onboarding
                    .private_key_file
                    .to_string_lossy()
                    .as_ref()
            )
        );
        assert!(
            onboarding
                .get("additional_permissions")
                .and_then(toml::Value::as_array)
                .is_some_and(|permissions| permissions.is_empty())
        );
        let credential = onboarding
            .get("credentials")
            .and_then(toml::Value::as_array)
            .and_then(|credentials| credentials.first())
            .and_then(toml::Value::as_table)
            .expect("single onboarding credential");
        assert_eq!(
            credential.get("id").and_then(toml::Value::as_str),
            Some(LOCAL_ONBOARDING_CREDENTIAL_ID)
        );
        assert_eq!(
            credential
                .get("scope")
                .and_then(toml::Value::as_table)
                .and_then(|scope| scope.get("dataspace"))
                .and_then(toml::Value::as_str),
            Some(LOCAL_ONBOARDING_DATASPACE)
        );
        assert_eq!(
            credential.get("token_hash").and_then(toml::Value::as_str),
            Some(expected_digest.as_str())
        );
        let onboarding = toml::Value::Table(onboarding.clone());
        if let Some(expected) = expected_onboarding.as_ref() {
            assert_eq!(&onboarding, expected);
        } else {
            expected_onboarding = Some(onboarding);
        }
    }
    let session_debug = format!("{session:?}");
    assert!(!session_debug.contains(&token));
    assert!(!session_debug.contains(&expected_digest));
    let supervisor_debug = format!("{supervisor:?}");
    assert!(!supervisor_debug.contains(&token));
    assert!(!supervisor_debug.contains(&expected_digest));
}
#[test]
fn supervisor_session_info_reports_workspace_and_mcp_urls() {
    if !ports_available("supervisor_session_info_reports_workspace_and_mcp_urls") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("temp dir");
    let workspace_root = temp.path().join("workspace");
    let sandbox_root = workspace_root.join(".mochi").join("sandbox");
    let _stub = KagamiStub::install(temp.path());
    let supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(&sandbox_root)
        .build()
        .expect("build supervisor");
    let info = supervisor.session_info().expect("session info");
    assert_eq!(
        info.workspace_root.as_deref(),
        Some(workspace_root.as_path())
    );
    assert!(info.sandbox_root.ends_with(Path::new("single-peer")));
    assert_eq!(info.torii_url, "http://127.0.0.1:8080");
    assert_eq!(info.mcp_url, "http://127.0.0.1:8080/v1/mcp");
    assert!(info.account_id.is_some());
    assert!(info.private_key.is_some());
    assert_eq!(
        info.onboarding_token_file,
        info.sandbox_root.join("runtime/onboarding.token")
    );
    assert_eq!(info.onboarding_credential_id, "local-dev");
    assert_eq!(
        info.onboarding_signer_file,
        info.sandbox_root.join("runtime/onboarding-signer.key")
    );
}
#[test]
fn managed_block_stream_unknown_peer_errors() {
    if !ports_available("managed_block_stream_unknown_peer_errors") {
        return;
    }
    let runtime = Runtime::new().expect("runtime");
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    let err = supervisor
        .managed_block_stream("missing", runtime.handle())
        .expect_err("unknown peer should fail");
    matches!(err, SupervisorError::PeerUnknown { .. });
}
#[test]
fn managed_block_stream_returns_handle_for_peer() {
    if !ports_available("managed_block_stream_returns_handle_for_peer") {
        return;
    }
    let runtime = Runtime::new().expect("runtime");
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    let stream = supervisor
        .managed_block_stream("peer0", runtime.handle())
        .expect("managed stream handle");
    assert_eq!(stream.alias(), "peer0");
    stream.abort();
}
#[test]
fn restart_policy_backoff_scales() {
    let policy = RestartPolicy::OnFailure {
        max_restarts: 5,
        backoff: Duration::from_millis(500),
    };
    assert_eq!(policy.backoff_for(1), Duration::from_millis(500));
    assert_eq!(policy.backoff_for(2), Duration::from_millis(1000));
    assert_eq!(policy.backoff_for(3), Duration::from_millis(2000));
    assert_eq!(policy.backoff_for(6), Duration::from_millis(8000));
}
#[test]
fn restart_policy_rejects_zero_attempt() {
    let policy = RestartPolicy::default();
    assert!(!policy.should_retry(0));
    assert_eq!(policy.backoff_for(0), Duration::ZERO);
}
#[cfg(unix)]
#[test]
fn managed_peer_process_uses_its_peer_directory_as_cwd() {
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let profile = NetworkProfile::default();
    let paths = NetworkPaths::from_root(temp.path(), &profile);
    paths.ensure().expect("paths");
    let spec = test_peer_spec(&paths, "peer0".into(), 8080, 1337).expect("peer spec");
    fs::write(&spec.config_path, "chain = \"cwd-test\"\n").expect("write config");
    let cwd_capture = temp.path().join("peer-cwd.txt");
    let stub = temp.path().join("irohad-cwd-stub.sh");
    fs::write(
        &stub,
        "#!/bin/sh\n/bin/pwd > \"$MOCHI_TEST_PEER_CWD\"\nexit 0\n",
    )
    .expect("write irohad stub");
    let mut perms = fs::metadata(&stub).expect("stub metadata").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&stub, perms).expect("set stub permissions");
    let _capture_guard = EnvVarGuard::set("MOCHI_TEST_PEER_CWD", cwd_capture.as_os_str());
    let expected = spec
        .config_path
        .canonicalize()
        .expect("canonical config")
        .parent()
        .expect("peer directory")
        .to_path_buf();
    let logs_dir = temp.path().join("logs");
    let mut peer = PeerHandle::prepared(spec, logs_dir, RestartPolicy::Never);
    let ownership = SupervisorOwnershipLock::acquire(paths.root()).expect("acquire ownership");
    peer.start(&stub, StartReason::Manual, &ownership)
        .expect("start peer");
    for _ in 0..50 {
        if cwd_capture.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let captured = fs::read_to_string(&cwd_capture).expect("captured peer cwd");
    assert_eq!(Path::new(captured.trim()), expected);
    if let Some(child) = peer.process.as_mut() {
        child.wait().expect("wait for peer stub");
    }
}
#[test]
fn manual_stop_cancels_pending_restart() {
    if !ports_available("manual_stop_cancels_pending_restart") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _kagami = KagamiStub::install(temp.path());
    let irohad_stub = temp.path().join("irohad_stub.sh");
    let stub_script = r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo "iroha-stub iroha3"
  exit 0
fi
exit 1
"#;
    fs::write(&irohad_stub, stub_script).expect("write irohad stub");
    #[cfg(unix)]
    {
        let mut perms = fs::metadata(&irohad_stub)
            .expect("stub metadata")
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&irohad_stub, perms).expect("set stub perms");
    }
    let _irohad_guard = EnvVarGuard::set("MOCHI_IROHAD", irohad_stub.as_os_str());
    let mut supervisor = match SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .restart_policy(RestartPolicy::OnFailure {
            max_restarts: 2,
            backoff: Duration::from_millis(200),
        })
        .build()
    {
        Ok(supervisor) => supervisor,
        Err(SupervisorError::Config(message))
            if message.contains("failed to allocate Torii port") =>
        {
            eprintln!("skipping manual_stop_cancels_pending_restart: {message}");
            return;
        }
        Err(err) => panic!("build supervisor: {err}"),
    };
    supervisor.start_peer("peer0").expect("start peer");
    // Stub exits immediately; refresh to observe the failure and schedule a restart.
    std::thread::sleep(Duration::from_millis(10));
    supervisor.refresh_peer_states();
    let peer = &supervisor.peers()[0];
    assert!(
        matches!(peer.state, PeerState::Restarting | PeerState::Stopped),
        "peer should schedule a restart after failure"
    );
    assert!(
        peer.next_restart_at.is_some(),
        "failure should set a restart timer"
    );
    supervisor
        .stop_peer("peer0")
        .expect("manual stop should succeed");
    let peer = &supervisor.peers()[0];
    assert!(
        peer.next_restart_at.is_none(),
        "restart timer should be cleared"
    );
    assert_eq!(peer.restart_attempts, 0);
    assert!(matches!(peer.state, PeerState::Stopped));
    // Allow enough time for the original backoff to elapse and confirm no restart occurs.
    std::thread::sleep(Duration::from_millis(250));
    supervisor.refresh_peer_states();
    let peer = &supervisor.peers()[0];
    assert!(
        peer.process.is_none(),
        "manual stop should keep the peer offline"
    );
    assert!(peer.next_restart_at.is_none());
    assert_eq!(peer.restart_attempts, 0);
    assert!(matches!(peer.state, PeerState::Stopped));
}
#[test]
fn supervisor_exposes_log_stream() {
    if !ports_available("supervisor_exposes_log_stream") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    let stream = supervisor
        .log_stream("peer0")
        .expect("log stream should be available");
    assert_eq!(stream.alias(), "peer0");
}
#[test]
fn start_peer_unknown_alias_errors() {
    if !ports_available("start_peer_unknown_alias_errors") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let mut supervisor = match SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .torii_base_port(20000)
        .p2p_base_port(30000)
        .build()
    {
        Ok(supervisor) => supervisor,
        Err(SupervisorError::Config(message))
            if message.contains("failed to allocate Torii port") =>
        {
            eprintln!("skipping start_peer_unknown_alias_errors: {message}");
            return;
        }
        Err(err) => panic!("build supervisor: {err}"),
    };
    let err = supervisor
        .start_peer("missing-peer")
        .expect_err("unknown peer should fail");
    assert!(matches!(err, SupervisorError::PeerUnknown { .. }));
}
#[test]
fn stop_peer_unknown_alias_errors() {
    if !ports_available("stop_peer_unknown_alias_errors") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let mut supervisor = match SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .torii_base_port(20000)
        .p2p_base_port(30000)
        .build()
    {
        Ok(supervisor) => supervisor,
        Err(SupervisorError::Config(message))
            if message.contains("failed to allocate Torii port") =>
        {
            eprintln!("skipping stop_peer_unknown_alias_errors: {message}");
            return;
        }
        Err(err) => panic!("build supervisor: {err}"),
    };
    let err = supervisor
        .stop_peer("missing-peer")
        .expect_err("unknown peer should fail");
    assert!(matches!(err, SupervisorError::PeerUnknown { .. }));
}
