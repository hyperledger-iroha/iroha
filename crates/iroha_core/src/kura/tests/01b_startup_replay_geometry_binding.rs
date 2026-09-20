// These tests exercise the native retained-geometry/shared-inventory boundary.
// The maintained four-peer catalog recovery test supplies the full signed WSV replay.
#[derive(Clone, Copy)]
enum StartupGeometryTail {
    Add,
    Relabel,
    Retire,
}

struct StartupGeometryBindingFixture {
    _directory: TempDir,
    kura: Arc<Kura>,
    catalogs: Vec<RuntimeLaneConfig>,
    incarnations: Vec<BTreeMap<LaneId, Hash>>,
    activations: Vec<BTreeMap<LaneId, u64>>,
    roots: Vec<Hash>,
    binding: V2StartupReplayStorageBinding,
    retained_added_artifacts: PathBuf,
}

impl StartupGeometryBindingFixture {
    fn new(tail: StartupGeometryTail, with_sidecar: bool) -> Self {
        Self::new_inner(tail, with_sidecar, false)
    }

    fn new_with_missing_namespace() -> Self {
        Self::new_inner(StartupGeometryTail::Add, false, true)
    }

    fn new_inner(tail: StartupGeometryTail, with_sidecar: bool, missing_namespace: bool) -> Self {
        let (directory, config) =
            kura_storage_fixture("startup geometry transition", BLOCKS_IN_MEMORY);
        let initial = RuntimeLaneConfig::default();
        let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &initial)
            .expect("open real primary storage");
        let primary_incarnation = Hash::new(b"startup geometry primary incarnation");
        let lane_incarnation = Hash::new(b"startup geometry added incarnation");
        let baseline = kura
            .lane_geometry_journal_state_for_test()
            .expect("read configured baseline")
            .0
            .expect("configured baseline exists");
        kura.establish_or_verify_configured_primary_geometry_anchor(
            initial.primary(),
            primary_incarnation,
            baseline,
        )
        .expect("authenticate configured primary marker");
        let mut catalogs = vec![initial, two_lane_runtime_config()];
        match tail {
            StartupGeometryTail::Add => {}
            StartupGeometryTail::Relabel => {
                let catalog = LaneCatalog::new(
                    nonzero!(2_u32),
                    vec![
                        ModelLaneConfig::default(),
                        ModelLaneConfig {
                            id: LaneId::from(1),
                            alias: "gamma".to_owned(),
                            ..ModelLaneConfig::default()
                        },
                    ],
                )
                .expect("relabelled lane catalog");
                catalogs.push(RuntimeLaneConfig::from_catalog(&catalog));
            }
            StartupGeometryTail::Retire => catalogs.push(RuntimeLaneConfig::default()),
        }
        let incarnations = catalogs
            .iter()
            .map(|catalog| {
                catalog
                    .entries()
                    .iter()
                    .map(|entry| {
                        (
                            entry.lane_id,
                            if entry.lane_id == LaneId::SINGLE {
                                primary_incarnation
                            } else {
                                lane_incarnation
                            },
                        )
                    })
                    .collect::<BTreeMap<_, _>>()
            })
            .collect::<Vec<_>>();
        let activations = catalogs
            .iter()
            .map(|catalog| {
                catalog
                    .entries()
                    .iter()
                    .map(|entry| {
                        (
                            entry.lane_id,
                            if entry.lane_id == LaneId::SINGLE {
                                0
                            } else {
                                2
                            },
                        )
                    })
                    .collect::<BTreeMap<_, _>>()
            })
            .collect::<Vec<_>>();
        let roots = (0..catalogs.len())
            .map(|index| Hash::new(format!("startup geometry lineage {index}").as_bytes()))
            .collect::<Vec<_>>();
        // Retain actual native journal operations, then rewind them to the replay cursor.
        // No synthetic publication receipt, substituted lane map, or copied sidecar is used.
        for index in 0..catalogs.len() - 1 {
            Self::apply_transition(&kura, &catalogs, &incarnations, &activations, &roots, index);
            if index == 0 && with_sidecar {
                let entry = catalogs[1].entry(LaneId::from(1)).expect("added lane");
                let path = Kura::lane_artifact_dir(&entry.blocks_dir(&kura.store_root()));
                fs::create_dir_all(&path).expect("create retained lane sidecar directory");
                fs::write(
                    path.join("startup-replay-evidence.norito"),
                    b"retained public sidecar image",
                )
                .expect("retain sidecar before geometry rewind");
            }
        }
        if missing_namespace {
            assert!(matches!(tail, StartupGeometryTail::Add) && !with_sidecar);
            let active = Kura::lane_artifact_dir(
                &catalogs[1]
                    .entry(LaneId::from(1))
                    .expect("added lane")
                    .blocks_dir(&kura.store_root()),
            );
            // Remove only an empty optional namespace before native rollback seals the archive.
            // No retained sealed image or canonical block data is edited.
            fs::remove_dir(active).expect("model valid missing namespace before archive sealing");
        }
        kura.restore_lane_segments_with_geometry_before_first_transition_at_height(
            &catalogs[0],
            &incarnations[0],
            &activations[0],
            roots[0],
            2,
        )
        .expect("rewind exact retained journal to configured replay cursor");
        assert!(kura.lane_storage_entry(LaneId::from(1)).is_err());
        assert!(
            !catalogs[1]
                .entry(LaneId::from(1))
                .expect("added lane")
                .blocks_dir(&kura.store_root())
                .exists()
        );

        let retained = snapshot_regular_test_tree(&kura.store_root());
        let mut sources = retained.keys().filter(|path| {
            path.file_name()
                .is_some_and(|name| name == "unpublished_blocks")
        });
        let retained_blocks = sources.next().expect("native retained Create source");
        assert!(
            sources.next().is_none(),
            "fixture has one exact retained Create source"
        );
        let retained_added_artifacts =
            Kura::lane_artifact_dir(&kura.store_root().join(retained_blocks));
        assert_eq!(retained_added_artifacts.exists(), !missing_namespace);

        let block = DummyBlocks::new().next();
        kura.store_block(Arc::clone(&block))
            .expect("store authenticated inventory fixture block");
        let artifact = v2_finality_artifact_for_block(&block);
        let _receipt = kura
            .store_v2_finality_artifact(&artifact)
            .expect("store finality artifact");
        let checkpoint = Hash::new(b"startup geometry checkpoint");
        kura.store_wsv_checkpoint(1, block.hash(), checkpoint)
            .expect("store checkpoint");
        kura.store_commit_manifest(
            CommitManifest::new(1, block.hash(), None, None, checkpoint, None)
                .with_authenticated_v2_commit_authority(&artifact),
        )
        .expect("store manifest");
        kura.clear_v2_finality_verification_cache_for_test();
        kura.reset_v2_finality_crypto_verifications_for_test();
        let inventory = kura
            .validate_v2_finality_inventory_on_startup(true)
            .expect("authenticate startup inventory");
        kura.install_v2_startup_finality_verification_inventory(inventory);
        kura.refresh_v2_startup_replay_auxiliary_binding()
            .expect("capture baseline auxiliary identities before planning");
        let session = kura
            .begin_v2_startup_finality_verification()
            .expect("open verification session")
            .expect("shared startup inventory");
        let binding = session
            .storage_binding()
            .expect("mint original shared replay binding");
        drop(session);
        kura.validate_v2_startup_replay_storage_binding(&binding)
            .expect("original shared binding is valid");
        kura.reset_startup_replay_historical_payload_reads_for_test();
        Self {
            _directory: directory,
            kura,
            catalogs,
            incarnations,
            activations,
            roots,
            binding,
            retained_added_artifacts,
        }
    }

    fn apply_transition(
        kura: &Kura,
        catalogs: &[RuntimeLaneConfig],
        incarnations: &[BTreeMap<LaneId, Hash>],
        activations: &[BTreeMap<LaneId, u64>],
        roots: &[Hash],
        index: usize,
    ) {
        kura.apply_lane_geometry_transition_at_height_with_lineage_roots(
            &catalogs[index],
            &catalogs[index + 1],
            &incarnations[index],
            &incarnations[index + 1],
            &activations[index],
            &activations[index + 1],
            roots[index],
            roots[index + 1],
            &BTreeSet::new(),
            u64::try_from(index).expect("bounded fixture index") + 2,
        )
        .expect("apply exact retained native geometry operation");
        kura.mark_lane_geometry_catalog_published_with_lineage_root(
            &catalogs[index + 1],
            &incarnations[index + 1],
            &activations[index + 1],
            roots[index + 1],
            None,
        )
        .expect("mark exact native geometry publication");
    }

    fn requests(&self) -> Vec<super::lane_geometry::ReplayGeometryBindingRequest<'_>> {
        (0..self.catalogs.len() - 1)
            .map(|index| super::lane_geometry::ReplayGeometryBindingRequest {
                previous: &self.catalogs[index],
                updated: &self.catalogs[index + 1],
                previous_incarnations: &self.incarnations[index],
                updated_incarnations: &self.incarnations[index + 1],
                previous_activation_heights: &self.activations[index],
                updated_activation_heights: &self.activations[index + 1],
                previous_lineage_root: self.roots[index],
                updated_lineage_root: self.roots[index + 1],
                transition_height: u64::try_from(index).expect("bounded fixture index") + 2,
            })
            .collect()
    }

    fn apply_all(&self, transition: &mut super::lane_geometry::StartupReplayGeometryTransition) {
        for (index, request) in self.requests().iter().enumerate() {
            self.kura
                .apply_startup_replay_geometry_transition(request, &BTreeSet::new(), transition)
                .expect("apply retained native operation with exact namespace effects");
            self.kura
                .mark_lane_geometry_catalog_published_with_lineage_root(
                    &self.catalogs[index + 1],
                    &self.incarnations[index + 1],
                    &self.activations[index + 1],
                    self.roots[index + 1],
                    None,
                )
                .expect("publish retained native operation with collected effects");
        }
    }

    fn rewind(&self) {
        self.kura
            .restore_lane_segments_with_geometry_before_first_transition_at_height(
                &self.catalogs[0],
                &self.incarnations[0],
                &self.activations[0],
                self.roots[0],
                2,
            )
            .expect("restore exact pre-publication geometry before effect cleanup");
    }

    fn added_artifacts(&self) -> PathBuf {
        Kura::lane_artifact_dir(
            &self.catalogs[1]
                .entry(LaneId::from(1))
                .expect("added lane")
                .blocks_dir(&self.kura.store_root()),
        )
    }

    fn finish(
        &self,
        transition: &super::lane_geometry::StartupReplayGeometryTransition,
    ) -> Result<V2StartupReplayStorageBinding> {
        let _lease = self.kura.canonical_publication_lease();
        self.kura
            .finish_startup_replay_geometry_transition(transition)
    }
}

#[test]
fn startup_replay_geometry_transition_preserves_shared_binding_for_added_lane() {
    for with_sidecar in [false, true] {
        let fixture = StartupGeometryBindingFixture::new(StartupGeometryTail::Add, with_sidecar);
        let mut transition = fixture
            .kura
            .begin_startup_replay_geometry_transition(&fixture.binding, &fixture.requests())
            .expect("exact retained addition admits empty or populated lane sidecar storage");
        fixture.apply_all(&mut transition);
        assert!(
            fixture
                .kura
                .validate_v2_startup_replay_storage_binding(&fixture.binding)
                .is_err(),
            "the old shared plan cannot silently adopt added lane paths"
        );
        let next = fixture
            .finish(&transition)
            .expect("derive exact post-publication binding");
        fixture
            .kura
            .validate_v2_startup_replay_storage_binding(&next)
            .expect("new binding is valid");
        let session = fixture
            .kura
            .begin_v2_startup_finality_verification()
            .expect("open actual post-publication verification session")
            .expect("the unchanged audit remains reusable with its derived publication");
        let resumed = session
            .storage_binding()
            .expect("session carries exact derived binding");
        assert!(
            matches!(
                &resumed,
                V2StartupReplayStorageBinding::StrictAfterGeometryPublication { .. }
            ),
            "active-height recovery must receive the derived binding"
        );
        drop(session);
        fixture
            .kura
            .validate_v2_startup_replay_storage_binding(&resumed)
            .expect("real resumed session uses the new lane inventory");
        assert!(
            fixture
                .kura
                .validate_v2_startup_replay_storage_binding(&fixture.binding)
                .is_err(),
            "publication must not rewrite the shared original inventory"
        );
        assert!(fixture.added_artifacts().is_dir());
        assert!(
            !fixture
                .added_artifacts()
                .join(HISTORICAL_AUTONOMOUS_RECOVERY_DIRECTORY_V1)
                .exists(),
            "the optional historical child remains absent"
        );
        if with_sidecar {
            assert_eq!(
                fs::read(
                    fixture
                        .added_artifacts()
                        .join("startup-replay-evidence.norito")
                )
                .expect("retained sidecar"),
                b"retained public sidecar image"
            );
        }
        assert_eq!(
            fixture.kura.v2_finality_crypto_verifications_for_test(),
            1,
            "geometry publication reuses the independently authenticated inventory"
        );
        assert_eq!(
            fixture
                .kura
                .startup_replay_historical_payload_reads_for_test(),
            0,
            "geometry identity transition does not reopen canonical history"
        );
    }
}

#[test]
fn startup_replay_geometry_transition_rejects_checkpoint_and_manifest_drift() {
    for checkpoint in [false, true] {
        for before_begin in [false, true] {
            let fixture = StartupGeometryBindingFixture::new(StartupGeometryTail::Add, false);
            let path = if checkpoint {
                fixture.kura.wsv_checkpoint_path(1)
            } else {
                fixture.kura.commit_manifest_path(1)
            };
            let directory = path.parent().expect("sidecar directory").to_path_buf();
            let tamper = || {
                fs::write(&path, b"different retained checkpoint or manifest image")
                    .expect("tamper public fixture sidecar")
            };
            let error = if before_begin {
                tamper();
                fixture
                    .kura
                    .begin_startup_replay_geometry_transition(&fixture.binding, &fixture.requests())
                    .err()
                    .expect("reject pre-publication old-binding drift")
            } else {
                let mut transition = fixture
                    .kura
                    .begin_startup_replay_geometry_transition(&fixture.binding, &fixture.requests())
                    .expect("pin unchanged original evidence");
                fixture.apply_all(&mut transition);
                tamper();
                fixture
                    .finish(&transition)
                    .err()
                    .expect("reject drift before WSV publication")
            };
            assert!(
                error
                    .to_string()
                    .contains(directory.to_string_lossy().as_ref()),
                "identity rejection must identify the changed directory: {error}"
            );
        }
    }
}

#[test]
fn startup_replay_geometry_transition_rejects_restored_lane_sidecar_drift() {
    let fixture = StartupGeometryBindingFixture::new(StartupGeometryTail::Add, true);
    let mut transition = fixture
        .kura
        .begin_startup_replay_geometry_transition(&fixture.binding, &fixture.requests())
        .expect("pin retained unpublished lane before move");
    fixture.apply_all(&mut transition);
    let directory = fixture.added_artifacts();
    fs::write(
        directory.join("startup-replay-evidence.norito"),
        b"substituted after the exact native move",
    )
    .expect("tamper moved public sidecar");
    let error = fixture
        .finish(&transition)
        .err()
        .expect("newly restored lane evidence must not be recaptured as authority");
    assert!(
        error
            .to_string()
            .contains(directory.to_string_lossy().as_ref()),
        "identify restored lane path: {error}"
    );
}

#[test]
fn startup_replay_geometry_transition_preserves_relabelled_and_retired_path_guards() {
    for tail in [StartupGeometryTail::Relabel, StartupGeometryTail::Retire] {
        let fixture =
            StartupGeometryBindingFixture::new(tail, matches!(tail, StartupGeometryTail::Relabel));
        let mut transition = fixture
            .kura
            .begin_startup_replay_geometry_transition(&fixture.binding, &fixture.requests())
            .expect("pin all ordered retained moves");
        fixture.apply_all(&mut transition);
        let next = fixture
            .finish(&transition)
            .expect("derive relabelled or retired binding");
        fixture
            .kura
            .validate_v2_startup_replay_storage_binding(&next)
            .expect("complete ordered transition remains exact");
        let old = fixture.added_artifacts();
        assert!(!old.exists(), "old active path must remain absent");
        fs::create_dir_all(&old).expect("reintroduce moved old path");
        fs::write(old.join("surplus.norito"), b"old path is not forgotten")
            .expect("write public test evidence");
        let error = fixture
            .kura
            .validate_v2_startup_replay_storage_binding(&next)
            .err()
            .expect("retired and relabelled original paths remain guarded");
        let blocks = old.parent().expect("old blocks directory");
        assert!(
            error
                .to_string()
                .contains(blocks.to_string_lossy().as_ref()),
            "identify resurrected source: {error}"
        );
    }
}

#[test]
fn startup_replay_geometry_transition_rejects_unretained_request() {
    let fixture = StartupGeometryBindingFixture::new(StartupGeometryTail::Add, false);
    for change_height in [false, true] {
        let mut requests = fixture.requests();
        if change_height {
            requests[0].transition_height += 1;
        } else {
            requests[0].updated_lineage_root = Hash::new(b"unretained lineage substitution");
        }
        let error = fixture
            .kura
            .begin_startup_replay_geometry_transition(&fixture.binding, &requests)
            .err()
            .expect("a request cannot create authority absent from exact retained journal");
        assert!(
            error
                .to_string()
                .contains("no exact retained journal operation"),
            "exact rejection: {error}"
        );
        fixture
            .kura
            .validate_v2_startup_replay_storage_binding(&fixture.binding)
            .expect("rejection preserves original binding");
        assert!(fixture.kura.lane_storage_entry(LaneId::from(1)).is_err());
    }
}

#[test]
fn startup_replay_geometry_transition_creates_only_missing_retained_namespace_and_cleans_failure() {
    let fixture = StartupGeometryBindingFixture::new_with_missing_namespace();
    assert!(!fixture.retained_added_artifacts.exists());
    let mut transition = fixture
        .kura
        .begin_startup_replay_geometry_transition(&fixture.binding, &fixture.requests())
        .expect("pin valid sealed archive with missing optional namespace");
    assert!(
        !fixture.retained_added_artifacts.exists(),
        "preparation must not mutate the sealed inactive archive"
    );
    fixture.apply_all(&mut transition);
    assert!(
        fixture.added_artifacts().is_dir(),
        "native authoritative activation creates its structural namespace"
    );
    let next = fixture
        .finish(&transition)
        .expect("admit exact native creation receipt");
    fixture
        .kura
        .validate_v2_startup_replay_storage_binding(&next)
        .expect("created namespace is bound by its native effect");

    let fixture = StartupGeometryBindingFixture::new_with_missing_namespace();
    let before = snapshot_regular_test_tree(&fixture.kura.store_root());
    let mut transition = fixture
        .kura
        .begin_startup_replay_geometry_transition(&fixture.binding, &fixture.requests())
        .expect("pin failure fixture");
    fixture
        .kura
        .apply_startup_replay_geometry_transition(
            &fixture.requests()[0],
            &BTreeSet::new(),
            &mut transition,
        )
        .expect("native apply collects the created namespace before publication");
    assert!(fixture.added_artifacts().is_dir());
    fixture.kura.fail_next_lane_geometry_publication_for_test();
    let error = fixture
        .kura
        .mark_lane_geometry_catalog_published_with_lineage_root(
            &fixture.catalogs[1],
            &fixture.incarnations[1],
            &fixture.activations[1],
            fixture.roots[1],
            None,
        )
        .expect_err("injected native publication failure");
    assert!(
        error
            .to_string()
            .contains("publication failed for test injection")
    );
    fixture.rewind();
    fixture
        .kura
        .rollback_startup_replay_geometry_preparation(&transition)
        .expect("remove only exact held native creation after geometry rollback");
    assert!(!fixture.retained_added_artifacts.exists());
    assert_eq!(
        snapshot_regular_test_tree(&fixture.kura.store_root()),
        before,
        "failed publication restores original files, directories, seals and journal bytes"
    );
    fixture
        .kura
        .validate_v2_startup_replay_storage_binding(&fixture.binding)
        .expect("failed native publication preserves original replay authority");

    let fixture = StartupGeometryBindingFixture::new_with_missing_namespace();
    let mut transition = fixture
        .kura
        .begin_startup_replay_geometry_transition(&fixture.binding, &fixture.requests())
        .expect("pin cleanup refusal fixture");
    fixture.apply_all(&mut transition);
    fixture.rewind();
    let unexpected = fixture
        .retained_added_artifacts
        .join("unexpected-public-evidence.norito");
    fs::write(&unexpected, b"must survive refused cleanup").expect("inject changed creation");
    let error = fixture
        .kura
        .rollback_startup_replay_geometry_preparation(&transition)
        .expect_err("cleanup cannot delete changed or nonempty namespace");
    assert!(
        error
            .to_string()
            .contains(fixture.retained_added_artifacts.to_string_lossy().as_ref()),
        "cleanup refusal identifies changed namespace: {error}"
    );
    assert_eq!(
        fs::read(unexpected).expect("refused cleanup preserves evidence"),
        b"must survive refused cleanup"
    );
}
