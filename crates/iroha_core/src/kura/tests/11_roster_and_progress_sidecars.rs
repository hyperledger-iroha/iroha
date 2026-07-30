    #[test]
    fn roster_sidecar_roundtrip_with_stake_snapshot() {
        use iroha_config::base::WithOrigin;

        let temp_dir = TempDir::new().unwrap();
        let (kura, _count) = Kura::new(
            &Config {
                init_mode: InitMode::Strict,
                store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: BLOCKS_IN_MEMORY,
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity:
                    iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: iroha_config::kura::FsyncMode::Batched,
                fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
                block_sync_roster_retention:
                    iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
                roster_sidecar_retention:
                    iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
                eviction_required_replicas:
                    iroha_config::parameters::defaults::kura::EVICTION_REQUIRED_REPLICAS,
            },
            &RuntimeLaneConfig::default(),
        )
        .unwrap();

        let kp = checked_keypair_with_algorithm(Algorithm::BlsNormal);
        let peer = PeerId::new(kp.public_key().clone());
        let roster = vec![peer];
        let block_hash = store_dummy_blocks(&kura, 1)[0];
        let signers_bitmap = vec![0b0000_0001];
        let bls_aggregate_signature = vec![0xAC; 96];
        let cert = Qc {
            phase: Phase::Commit,
            subject_block_hash: block_hash,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 1,
            view: 0,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            mode_tag: PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&roster),
            validator_set_hash_version: iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            validator_set: roster.clone(),
            aggregate: QcAggregate {
                signers_bitmap: signers_bitmap.clone(),
                bls_aggregate_signature: bls_aggregate_signature.clone(),
            },
        };
        let stake_snapshot = crate::sumeragi::stake_snapshot::CommitStakeSnapshot {
            validator_set_hash: HashOf::new(&roster),
            entries: vec![crate::sumeragi::stake_snapshot::CommitStakeSnapshotEntry {
                peer_id: roster[0].clone(),
                stake: iroha_primitives::numeric::Quantity::from(10_u32),
            }],
        };
        let sidecar = RosterSidecar::new(
            1,
            block_hash,
            Some(cert.clone()),
            None,
            Some(stake_snapshot.clone()),
        );

        kura.write_roster_metadata(&sidecar);
        let got = kura.read_roster_metadata(1).expect("sidecar exists");

        assert_eq!(got.height, 1);
        assert_eq!(got.block_hash, block_hash);
        assert_eq!(got.format_label(), "roster.snapshot");
        assert_eq!(got.stake_snapshot, Some(stake_snapshot));
        assert_eq!(got.roster_snapshot(), Some(roster));
    }

    #[derive(Debug, Encode, Decode, PartialEq, Eq)]
    struct DummySidecar {
        height: u64,
    }

    #[derive(Clone, Copy, Debug)]
    enum ProgressSidecarBarrierFailure {
        Data,
        Index,
        ImmediateDirectory,
        AncestorDirectory(usize),
    }

    impl ProgressSidecarBarrierFailure {
        fn inject(self) {
            match self {
                Self::Data => fail_next_indexed_sidecar_data_sync_for_tests(),
                Self::Index => fail_next_indexed_sidecar_index_sync_for_tests(),
                Self::ImmediateDirectory => fail_next_indexed_sidecar_dir_sync_for_tests(),
                Self::AncestorDirectory(index) => {
                    fail_progress_sidecar_ancestor_sync_at_for_tests(index);
                }
            }
        }
    }

    fn strict_progress_sidecar_failure_modes() -> [(&'static str, ProgressSidecarBarrierFailure); 6]
    {
        [
            ("data", ProgressSidecarBarrierFailure::Data),
            ("index", ProgressSidecarBarrierFailure::Index),
            (
                "immediate-directory",
                ProgressSidecarBarrierFailure::ImmediateDirectory,
            ),
            (
                "lane-segment-directory",
                ProgressSidecarBarrierFailure::AncestorDirectory(0),
            ),
            (
                "blocks-directory",
                ProgressSidecarBarrierFailure::AncestorDirectory(1),
            ),
            (
                "store-root-directory",
                ProgressSidecarBarrierFailure::AncestorDirectory(2),
            ),
        ]
    }

    fn strict_indexed_sidecar_failure_modes() -> [(&'static str, fn()); 3] {
        [
            ("data", fail_next_indexed_sidecar_data_sync_for_tests),
            ("index", fail_next_indexed_sidecar_index_sync_for_tests),
            ("directory", fail_next_indexed_sidecar_dir_sync_for_tests),
        ]
    }

    fn strict_sidecar_retry_reissues_barriers_for_exact_existing_payload() {
        for (label, inject_failure) in strict_indexed_sidecar_failure_modes() {
            let temp_dir = TempDir::new().unwrap();
            let data_path = temp_dir.path().join(ROSTER_SIDECARS_DATA_FILE);
            let index_path = temp_dir.path().join(ROSTER_SIDECARS_INDEX_FILE);
            let payload =
                norito::to_bytes(&DummySidecar { height: 1 }).expect("encode dummy sidecar");

            inject_failure();
            assert!(
                !Kura::append_indexed_sidecar(
                    &data_path,
                    &index_path,
                    1,
                    &payload,
                    "roster sidecar",
                    FsyncMode::Always,
                    None,
                    SidecarIndexOrigin::HeightOne,
                ),
                "injected {label} barrier failure must reject the new strict write"
            );
            let readable = Kura::read_indexed_sidecar_from_paths::<DummySidecar, _>(
                1,
                &data_path,
                &index_path,
                norito::decode_from_bytes::<DummySidecar>,
                "roster sidecar",
            )
            .expect("failed barrier leaves an exact page-cache payload readable");
            assert_eq!(readable.height, 1);
            let first_data_len = fs::metadata(&data_path).expect("data metadata").len();

            inject_failure();
            assert!(
                !Kura::append_indexed_sidecar(
                    &data_path,
                    &index_path,
                    1,
                    &payload,
                    "roster sidecar",
                    FsyncMode::Always,
                    None,
                    SidecarIndexOrigin::HeightOne,
                ),
                "exact-existing retry must reissue and observe the {label} barrier failure"
            );
            assert!(
                Kura::append_indexed_sidecar(
                    &data_path,
                    &index_path,
                    1,
                    &payload,
                    "roster sidecar",
                    FsyncMode::Always,
                    None,
                    SidecarIndexOrigin::HeightOne,
                ),
                "retry must succeed once every strict barrier succeeds"
            );
            assert_eq!(
                fs::metadata(&data_path).expect("data metadata").len(),
                first_data_len,
                "exact retries must not append duplicate payload bytes"
            );
        }
    }

    fn initial_preindex_data_sync_failure_rolls_back_payload_before_retry() {
        for overwrite_placeholder in [false, true] {
            let temp_dir = TempDir::new().expect("create temp dir");
            let data_path = temp_dir.path().join(ROSTER_SIDECARS_DATA_FILE);
            let index_path = temp_dir.path().join(ROSTER_SIDECARS_INDEX_FILE);
            let payload = norito::to_bytes(&DummySidecar { height: 1 })
                .expect("encode height-one dummy sidecar");

            let baseline_len = if overwrite_placeholder {
                let height_two = norito::to_bytes(&DummySidecar { height: 2 })
                    .expect("encode height-two dummy sidecar");
                assert!(
                    Kura::append_indexed_sidecar(
                        &data_path,
                        &index_path,
                        2,
                        &height_two,
                        "initial-sync rollback sidecar",
                        FsyncMode::Always,
                        None,
                        SidecarIndexOrigin::HeightOne,
                    ),
                    "prepare a height-one index placeholder"
                );
                fs::metadata(&data_path)
                    .expect("baseline data metadata")
                    .len()
            } else {
                0
            };

            fail_next_indexed_sidecar_initial_data_sync_for_tests();
            assert!(
                !Kura::append_indexed_sidecar(
                    &data_path,
                    &index_path,
                    1,
                    &payload,
                    "initial-sync rollback sidecar",
                    FsyncMode::Always,
                    None,
                    SidecarIndexOrigin::HeightOne,
                ),
                "an initial pre-index data barrier failure must reject the write"
            );
            assert_eq!(
                fs::metadata(&data_path)
                    .expect("rolled-back data metadata")
                    .len(),
                baseline_len,
                "the unpublished payload must be truncated and synchronized before retry"
            );
            assert!(
                Kura::read_indexed_sidecar_from_paths::<DummySidecar, _>(
                    1,
                    &data_path,
                    &index_path,
                    norito::decode_from_bytes::<DummySidecar>,
                    "initial-sync rollback sidecar",
                )
                .is_none(),
                "the failed pre-index write must not publish a readable height-one entry"
            );

            assert!(
                Kura::append_indexed_sidecar(
                    &data_path,
                    &index_path,
                    1,
                    &payload,
                    "initial-sync rollback sidecar",
                    FsyncMode::Always,
                    None,
                    SidecarIndexOrigin::HeightOne,
                ),
                "retry must publish the payload exactly once"
            );
            assert_eq!(
                fs::metadata(&data_path)
                    .expect("retried data metadata")
                    .len(),
                baseline_len + u64::try_from(payload.len()).expect("payload length fits u64"),
                "retry must not retain or duplicate unpublished payload bytes"
            );
            assert_eq!(
                Kura::read_indexed_sidecar_from_paths::<DummySidecar, _>(
                    1,
                    &data_path,
                    &index_path,
                    norito::decode_from_bytes::<DummySidecar>,
                    "initial-sync rollback sidecar",
                ),
                Some(DummySidecar { height: 1 })
            );
        }
    }

    fn unindexed_crash_suffix_is_repaired_before_retry_or_append() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let data_path = temp_dir.path().join(ROSTER_SIDECARS_DATA_FILE);
        let index_path = temp_dir.path().join(ROSTER_SIDECARS_INDEX_FILE);
        let first = norito::to_bytes(&DummySidecar { height: 1 }).expect("encode first payload");
        let replacement =
            norito::to_bytes(&DummySidecar { height: 11 }).expect("encode replacement payload");
        let second = norito::to_bytes(&DummySidecar { height: 2 }).expect("encode second payload");

        assert!(Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            1,
            &first,
            "crash-tail repair sidecar",
            FsyncMode::Always,
            None,
            SidecarIndexOrigin::HeightOne,
        ));
        let append_residue = |bytes: &[u8]| {
            let mut data = std::fs::OpenOptions::new()
                .append(true)
                .open(&data_path)
                .expect("open data for crash residue");
            data.write_all(bytes).expect("append crash residue");
            data.sync_data().expect("persist crash residue fixture");
        };

        append_residue(b"unpublished-exact-retry");
        assert!(Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            1,
            &first,
            "crash-tail repair sidecar",
            FsyncMode::Always,
            None,
            SidecarIndexOrigin::HeightOne,
        ));
        assert_eq!(
            fs::metadata(&data_path).expect("repaired data").len(),
            u64::try_from(first.len()).expect("first length fits u64"),
            "an exact-existing retry must trim a crash suffix before returning"
        );

        append_residue(b"unpublished-replacement");
        assert!(Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            1,
            &replacement,
            "crash-tail repair sidecar",
            FsyncMode::Always,
            None,
            SidecarIndexOrigin::HeightOne,
        ));
        let after_replacement =
            u64::try_from(first.len() + replacement.len()).expect("replacement length fits u64");
        assert_eq!(
            fs::metadata(&data_path).expect("replacement data").len(),
            after_replacement,
            "replacement retry must retain one old payload and one replacement only"
        );

        append_residue(b"unpublished-new-height");
        assert!(Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            2,
            &second,
            "crash-tail repair sidecar",
            FsyncMode::Always,
            None,
            SidecarIndexOrigin::HeightOne,
        ));
        assert_eq!(
            fs::metadata(&data_path).expect("height-two data").len(),
            after_replacement + u64::try_from(second.len()).expect("second length fits u64"),
            "new-height retry must append exactly once after trimming crash residue"
        );
        assert_eq!(
            Kura::read_indexed_sidecar_from_paths::<DummySidecar, _>(
                1,
                &data_path,
                &index_path,
                norito::decode_from_bytes::<DummySidecar>,
                "crash-tail repair sidecar",
            ),
            Some(DummySidecar { height: 11 })
        );
        assert_eq!(
            Kura::read_indexed_sidecar_from_paths::<DummySidecar, _>(
                2,
                &data_path,
                &index_path,
                norito::decode_from_bytes::<DummySidecar>,
                "crash-tail repair sidecar",
            ),
            Some(DummySidecar { height: 2 })
        );
    }

    #[cfg(unix)]
    fn progress_sidecar_mutation_rejects_symlinks_without_external_writes() {
        use std::os::unix::fs::symlink;

        for substitution in ["data", "index", "directory"] {
            let temp_dir = TempDir::new().expect("create temp dir");
            let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
            let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("init Kura");
            let root = kura.store_root();
            let sidecar_dir = root.join(format!("progress-{substitution}"));
            fs::create_dir_all(&sidecar_dir).expect("create progress namespace");
            let data_path = sidecar_dir.join("progress.data");
            let index_path = sidecar_dir.join("progress.index");
            let namespace = kura
                .open_bound_progress_namespace(&data_path, &index_path)
                .expect("bind progress namespace before substitution");
            let external_dir = root.join(format!("external-{substitution}"));
            fs::create_dir_all(&external_dir).expect("create external target directory");
            let external_data = external_dir.join("progress.data");
            let external_index = external_dir.join("progress.index");
            let data_sentinel = b"external-data-sentinel";
            let index_sentinel = b"external-index-sentinel";
            fs::write(&external_data, data_sentinel).expect("write external data sentinel");
            fs::write(&external_index, index_sentinel).expect("write external index sentinel");

            match substitution {
                "data" => symlink(&external_data, &data_path).expect("substitute data symlink"),
                "index" => symlink(&external_index, &index_path).expect("substitute index symlink"),
                "directory" => {
                    let displaced = root.join("displaced-progress");
                    fs::rename(&sidecar_dir, &displaced).expect("displace bound directory");
                    symlink(&external_dir, &sidecar_dir).expect("substitute namespace symlink");
                }
                _ => unreachable!("fixed substitution matrix"),
            }

            let payload = norito::to_bytes(&DummySidecar { height: 1 })
                .expect("encode progress mutation payload");
            assert!(
                !Kura::append_indexed_progress_sidecar(
                    &data_path,
                    &index_path,
                    1,
                    &payload,
                    "symlink progress sidecar",
                    None,
                    SidecarIndexOrigin::FirstWrite,
                    &namespace,
                ),
                "{substitution} substitution must reject the progress write"
            );
            assert_eq!(
                fs::read(&external_data).expect("read external data sentinel"),
                data_sentinel,
                "{substitution} substitution must not mutate external data"
            );
            assert_eq!(
                fs::read(&external_index).expect("read external index sentinel"),
                index_sentinel,
                "{substitution} substitution must not mutate external index"
            );
        }
    }

    fn absent_progress_namespace_requires_every_directory_barrier() {
        for (label, failure) in strict_progress_sidecar_failure_modes().into_iter().skip(2) {
            let temp_dir = TempDir::new().expect("create temp dir");
            let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
            let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("init Kura");
            let sidecar_dir = kura
                .store_root()
                .join("blocks")
                .join("lane")
                .join(LANE_ARTIFACTS_DIR_NAME);
            fs::create_dir_all(&sidecar_dir).expect("create absent progress namespace");
            let data_path = sidecar_dir.join("absent.data");
            let index_path = sidecar_dir.join("absent.index");
            let pair = kura
                .open_bound_progress_pair(&data_path, &index_path)
                .expect("bind absent progress pair");
            let BoundProgressPair::Absent(namespace) = pair else {
                panic!("fresh progress pair must be absent");
            };
            failure.inject();
            assert!(
                !kura.sync_bound_progress_absence(&namespace, "absent progress test"),
                "absent namespace must fail closed at the {label} barrier"
            );
        }

        let temp_dir = TempDir::new().expect("create temp dir");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("init Kura");
        let sidecar_dir = kura
            .store_root()
            .join("blocks")
            .join("lane")
            .join(LANE_ARTIFACTS_DIR_NAME);
        fs::create_dir_all(&sidecar_dir).expect("create absent progress namespace");
        let data_path = sidecar_dir.join("absent.data");
        let index_path = sidecar_dir.join("absent.index");
        let pair = kura
            .open_bound_progress_pair(&data_path, &index_path)
            .expect("bind absent progress pair");
        let BoundProgressPair::Absent(namespace) = pair else {
            panic!("fresh progress pair must be absent");
        };
        fs::write(&data_path, b"appeared after absence scan").expect("publish conflicting data");
        assert!(
            !kura.sync_bound_progress_absence(&namespace, "appeared progress test"),
            "a sidecar appearing after the absence scan must invalidate the witness"
        );
    }

    #[cfg(unix)]
    fn progress_prepend_directory_failure_retries_without_corruption() {
        for (label, failure) in strict_progress_sidecar_failure_modes().into_iter().skip(2) {
            let temp_dir = TempDir::new().expect("create temp dir");
            let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
            let lane_config = RuntimeLaneConfig::default();
            let (kura, _) = Kura::new(&config, &lane_config).expect("init Kura");
            let sidecar_dir = kura
                .store_root()
                .join("blocks")
                .join("lane")
                .join(LANE_ARTIFACTS_DIR_NAME);
            fs::create_dir_all(&sidecar_dir).expect("create progress namespace");
            let data_path = sidecar_dir.join("prepend.data");
            let index_path = sidecar_dir.join("prepend.index");
            let height_two =
                norito::to_bytes(&DummySidecar { height: 2 }).expect("encode height two");
            let height_one =
                norito::to_bytes(&DummySidecar { height: 1 }).expect("encode height one");

            let namespace = kura
                .open_bound_progress_namespace(&data_path, &index_path)
                .expect("bind initial progress namespace");
            assert!(
                Kura::append_indexed_progress_sidecar(
                    &data_path,
                    &index_path,
                    2,
                    &height_two,
                    "progress prepend test",
                    None,
                    SidecarIndexOrigin::FirstWrite,
                    &namespace,
                ),
                "prepare height two for the {label} failure"
            );

            let namespace = kura
                .open_bound_progress_namespace(&data_path, &index_path)
                .expect("rebind progress namespace before prepend");
            failure.inject();
            assert!(
                !Kura::append_indexed_progress_sidecar(
                    &data_path,
                    &index_path,
                    1,
                    &height_one,
                    "progress prepend test",
                    None,
                    SidecarIndexOrigin::FirstWrite,
                    &namespace,
                ),
                "the {label} barrier failure must reject the prepend acknowledgement"
            );
            assert_eq!(
                fs::metadata(&data_path)
                    .expect("failed prepend data metadata")
                    .len(),
                u64::try_from(height_two.len() + height_one.len())
                    .expect("fixture length fits u64"),
                "a post-publication {label} failure must not truncate indexed payload bytes"
            );

            drop(kura);
            let (reopened, _) =
                Kura::new(&config, &lane_config).expect("reopen Kura after failed prepend");
            let namespace = reopened
                .open_bound_progress_namespace(&data_path, &index_path)
                .expect("bind reopened progress namespace");
            assert!(
                Kura::append_indexed_progress_sidecar(
                    &data_path,
                    &index_path,
                    1,
                    &height_one,
                    "progress prepend test",
                    None,
                    SidecarIndexOrigin::FirstWrite,
                    &namespace,
                ),
                "an exact retry after the {label} failure must reissue every barrier"
            );
            assert_eq!(
                Kura::read_indexed_sidecar_from_paths::<DummySidecar, _>(
                    1,
                    &data_path,
                    &index_path,
                    norito::decode_from_bytes::<DummySidecar>,
                    "progress prepend test",
                ),
                Some(DummySidecar { height: 1 })
            );
            assert_eq!(
                Kura::read_indexed_sidecar_from_paths::<DummySidecar, _>(
                    2,
                    &data_path,
                    &index_path,
                    norito::decode_from_bytes::<DummySidecar>,
                    "progress prepend test",
                ),
                Some(DummySidecar { height: 2 })
            );
            assert_eq!(
                fs::metadata(&data_path)
                    .expect("retried prepend data metadata")
                    .len(),
                u64::try_from(height_two.len() + height_one.len())
                    .expect("fixture length fits u64"),
                "retry must preserve exactly one payload for each indexed height"
            );
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[allow(clippy::too_many_lines)]
    fn bound_progress_recovery_handles_crash_phases_without_path_escape() {
        use std::fs::{File, OpenOptions};
        use std::os::unix::fs::{MetadataExt as _, symlink};

        assert_eq!(
            BoundProgressRecoveryFailure::from_io(&std::io::Error::new(
                ErrorKind::WouldBlock,
                "transient recovery fixture",
            )),
            BoundProgressRecoveryFailure::RetryableIo,
        );
        assert_eq!(
            BoundProgressRecoveryFailure::from_io(&std::io::Error::new(
                ErrorKind::InvalidData,
                "structural recovery fixture",
            )),
            BoundProgressRecoveryFailure::InvalidData,
        );

        let fixture = || {
            let temp_dir = TempDir::new().expect("create temp dir");
            let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
            let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default())
                .expect("init bound recovery Kura");
            let sidecar_dir = kura
                .store_root()
                .join("blocks")
                .join("lane")
                .join(LANE_ARTIFACTS_DIR_NAME);
            fs::create_dir_all(&sidecar_dir).expect("create bound recovery namespace");
            let data_path = sidecar_dir.join("bound-recovery.norito");
            let index_path = sidecar_dir.join("bound-recovery.index");
            (temp_dir, kura, data_path, index_path)
        };
        let persist =
            |kura: &Kura, data_path: &Path, index_path: &Path, height: u64, payload: &[u8]| {
                let namespace = kura
                    .open_bound_progress_namespace(data_path, index_path)
                    .expect("bind progress fixture namespace");
                assert!(Kura::append_indexed_progress_sidecar(
                    data_path,
                    index_path,
                    height,
                    payload,
                    "bound recovery test",
                    None,
                    SidecarIndexOrigin::FirstWrite,
                    &namespace,
                ));
            };
        let read = |data_path: &Path, index_path: &Path, height: u64| {
            Kura::read_indexed_sidecar_from_paths::<DummySidecar, _>(
                height,
                data_path,
                index_path,
                norito::decode_from_bytes::<DummySidecar>,
                "bound recovery test",
            )
        };
        let append_intent = |kura: &Kura,
                             data_path: &Path,
                             index_path: &Path,
                             height: u64,
                             pair_was_present: bool,
                             old_data_len: u64,
                             old_index_len: u64,
                             index_write_offset: u64,
                             old_index_bytes: Vec<u8>,
                             new_index_bytes: Vec<u8>,
                             payload: &[u8]| {
            let namespace = kura
                .open_bound_progress_namespace(data_path, index_path)
                .expect("bind progress intent namespace");
            BoundProgressAppendIntentV1 {
                version: BOUND_PROGRESS_APPEND_INTENT_VERSION,
                namespace_components: namespace
                    .stable_relative_components(data_path, index_path)
                    .expect("derive progress intent namespace"),
                data_file: data_path
                    .file_name()
                    .and_then(std::ffi::OsStr::to_str)
                    .expect("UTF-8 data file name")
                    .to_owned(),
                index_file: index_path
                    .file_name()
                    .and_then(std::ffi::OsStr::to_str)
                    .expect("UTF-8 index file name")
                    .to_owned(),
                height,
                pair_was_present,
                old_data_len,
                new_data_len: old_data_len + u64::try_from(payload.len()).expect("payload length"),
                payload_hash: BoundProgressAppendIntentV1::payload_digest(payload),
                old_index_len,
                new_index_len: if index_write_offset == old_index_len {
                    old_index_len
                        + u64::try_from(new_index_bytes.len()).expect("index window length")
                } else {
                    old_index_len
                },
                index_write_offset,
                old_index_bytes,
                new_index_bytes,
                integrity_hash: Hash::prehashed([0; Hash::LENGTH]),
            }
            .seal()
        };
        let stage_intent = |index_path: &Path, intent: &BoundProgressAppendIntentV1| {
            fs::write(
                Kura::bound_progress_append_intent_path(index_path),
                norito::to_bytes(intent).expect("encode progress append intent"),
            )
            .expect("stage progress append intent");
        };

        // Exercise the production writer at every journal-specific file seam.
        // Each failed acknowledgement is recovered before one exact retry.
        let journal_faults: [(&str, fn()); 3] = [
            (
                "intent-file",
                fail_next_bound_progress_intent_file_sync_for_tests,
            ),
            (
                "payload-file",
                fail_next_bound_progress_append_data_sync_for_tests,
            ),
            (
                "index-file",
                fail_next_bound_progress_append_index_sync_for_tests,
            ),
        ];
        for (label, inject) in journal_faults {
            let (_temp_dir, kura, data_path, index_path) = fixture();
            let payload = norito::to_bytes(&DummySidecar { height: 1 })
                .expect("encode journal fault payload");
            let namespace = kura
                .open_bound_progress_namespace(&data_path, &index_path)
                .expect("bind journal fault namespace");
            inject();
            assert!(
                !Kura::append_indexed_progress_sidecar(
                    &data_path,
                    &index_path,
                    1,
                    &payload,
                    "bound recovery journal fault test",
                    None,
                    SidecarIndexOrigin::FirstWrite,
                    &namespace,
                ),
                "the {label} barrier must reject the acknowledgement"
            );
            assert!(kura.recover_bound_progress_sidecar_artifacts(
                &data_path,
                &index_path,
                "bound recovery journal fault test",
            ));
            let namespace = kura
                .open_bound_progress_namespace(&data_path, &index_path)
                .expect("rebind recovered journal fault namespace");
            assert!(Kura::append_indexed_progress_sidecar(
                &data_path,
                &index_path,
                1,
                &payload,
                "bound recovery journal fault test",
                None,
                SidecarIndexOrigin::FirstWrite,
                &namespace,
            ));
            assert_eq!(
                read(&data_path, &index_path, 1),
                Some(DummySidecar { height: 1 })
            );
        }

        // Publication and cleanup each bind the immediate directory and every
        // ancestor through the Kura root. Fail every depth in both phases.
        for calls_before_failure in [0, 1] {
            let phase = if calls_before_failure == 0 {
                "publication"
            } else {
                "cleanup"
            };
            let (_probe, probe_kura, probe_data, probe_index) = fixture();
            let probe_namespace = probe_kura
                .open_bound_progress_namespace(&probe_data, &probe_index)
                .expect("bind intent directory probe");
            let directory_count = probe_namespace.directories.len();
            drop(probe_namespace);
            drop(probe_kura);
            for target_index in 0..directory_count {
                let (_temp_dir, kura, data_path, index_path) = fixture();
                let payload = norito::to_bytes(&DummySidecar { height: 1 })
                    .expect("encode directory fault payload");
                let namespace = kura
                    .open_bound_progress_namespace(&data_path, &index_path)
                    .expect("bind directory fault namespace");
                fail_bound_progress_intent_directory_sync_for_tests(
                    calls_before_failure,
                    target_index,
                );
                assert!(
                    !Kura::append_indexed_progress_sidecar(
                        &data_path,
                        &index_path,
                        1,
                        &payload,
                        "bound recovery intent directory fault test",
                        None,
                        SidecarIndexOrigin::FirstWrite,
                        &namespace,
                    ),
                    "intent {phase} directory barrier {target_index} must fail"
                );
                assert!(kura.recover_bound_progress_sidecar_artifacts(
                    &data_path,
                    &index_path,
                    "bound recovery intent directory fault test",
                ));
                let namespace = kura
                    .open_bound_progress_namespace(&data_path, &index_path)
                    .expect("rebind directory fault namespace");
                assert!(Kura::append_indexed_progress_sidecar(
                    &data_path,
                    &index_path,
                    1,
                    &payload,
                    "bound recovery intent directory fault test",
                    None,
                    SidecarIndexOrigin::FirstWrite,
                    &namespace,
                ));
            }
        }

        // A build name is not authoritative: main mutation cannot start until
        // the atomic no-replace promotion publishes the final intent name.
        {
            let (_temp_dir, kura, data_path, index_path) = fixture();
            let build_path = Kura::bound_progress_append_build_path(&index_path);
            fs::write(&build_path, b"partial unpublished intent").expect("stage intent build");
            assert!(kura.recover_bound_progress_sidecar_artifacts(
                &data_path,
                &index_path,
                "bound recovery test",
            ));
            assert!(!build_path.exists());
            assert!(!data_path.exists() && !index_path.exists());
        }

        // A partial first-write payload/index under a valid intent rolls back
        // to true pair absence, after which the durable source can retry once.
        {
            let (_temp_dir, kura, data_path, index_path) = fixture();
            let payload =
                norito::to_bytes(&DummySidecar { height: 1 }).expect("encode partial first write");
            let entry = SidecarIndexEntry {
                offset: 0,
                len: u64::try_from(payload.len()).expect("payload length"),
            }
            .to_bytes()
            .to_vec();
            let intent = append_intent(
                &kura,
                &data_path,
                &index_path,
                1,
                false,
                0,
                0,
                0,
                Vec::new(),
                entry.clone(),
                &payload,
            );
            stage_intent(&index_path, &intent);
            fs::write(&data_path, &payload[..payload.len() - 1]).expect("stage partial data");
            fs::write(&index_path, &entry[..8]).expect("stage torn index");
            assert!(kura.recover_bound_progress_sidecar_artifacts(
                &data_path,
                &index_path,
                "bound recovery test",
            ));
            assert!(!data_path.exists() && !index_path.exists());
            assert!(!Kura::bound_progress_append_intent_path(&index_path).exists());
            persist(&kura, &data_path, &index_path, 1, &payload);
            assert_eq!(
                read(&data_path, &index_path, 1),
                Some(DummySidecar { height: 1 })
            );
        }

        // Once the exact payload suffix is complete, recovery rolls a torn
        // index forward from the bounded postimage and clears the marker.
        {
            let (_temp_dir, kura, data_path, index_path) = fixture();
            let payload =
                norito::to_bytes(&DummySidecar { height: 1 }).expect("encode complete first write");
            let entry = SidecarIndexEntry {
                offset: 0,
                len: u64::try_from(payload.len()).expect("payload length"),
            }
            .to_bytes()
            .to_vec();
            let intent = append_intent(
                &kura,
                &data_path,
                &index_path,
                1,
                false,
                0,
                0,
                0,
                Vec::new(),
                entry.clone(),
                &payload,
            );
            stage_intent(&index_path, &intent);
            fs::write(&data_path, &payload).expect("stage complete data");
            fs::write(&index_path, &entry[..8]).expect("stage torn index");
            assert!(kura.recover_bound_progress_sidecar_artifacts(
                &data_path,
                &index_path,
                "bound recovery test",
            ));
            assert_eq!(
                read(&data_path, &index_path, 1),
                Some(DummySidecar { height: 1 })
            );
            assert!(!Kura::bound_progress_append_intent_path(&index_path).exists());
        }

        // A full-length suffix with the wrong digest is not a committed
        // postimage. Recovery restores the exact old pair and a later retry
        // appends the intended payload once.
        {
            let (_temp_dir, kura, data_path, index_path) = fixture();
            let first = norito::to_bytes(&DummySidecar { height: 1 })
                .expect("encode wrong-hash predecessor");
            let intended =
                norito::to_bytes(&DummySidecar { height: 2 }).expect("encode intended suffix");
            let wrong =
                norito::to_bytes(&DummySidecar { height: 3 }).expect("encode wrong-hash suffix");
            assert_eq!(intended.len(), wrong.len());
            persist(&kura, &data_path, &index_path, 1, &first);
            let old_data_len = fs::metadata(&data_path).expect("old data metadata").len();
            let old_index_len = fs::metadata(&index_path).expect("old index metadata").len();
            let new_entry = SidecarIndexEntry {
                offset: old_data_len,
                len: u64::try_from(intended.len()).expect("intended suffix length"),
            }
            .to_bytes()
            .to_vec();
            let intent = append_intent(
                &kura,
                &data_path,
                &index_path,
                2,
                true,
                old_data_len,
                old_index_len,
                old_index_len,
                Vec::new(),
                new_entry,
                &intended,
            );
            stage_intent(&index_path, &intent);
            OpenOptions::new()
                .append(true)
                .open(&data_path)
                .expect("open data for wrong-hash suffix")
                .write_all(&wrong)
                .expect("stage wrong-hash suffix");
            assert!(kura.recover_bound_progress_sidecar_artifacts(
                &data_path,
                &index_path,
                "bound recovery wrong-hash test",
            ));
            assert_eq!(
                fs::metadata(&data_path).expect("rolled-back data").len(),
                old_data_len
            );
            assert_eq!(
                fs::metadata(&index_path).expect("rolled-back index").len(),
                old_index_len
            );
            assert_eq!(
                read(&data_path, &index_path, 1),
                Some(DummySidecar { height: 1 })
            );
            assert_eq!(read(&data_path, &index_path, 2), None);
            assert!(!Kura::bound_progress_append_intent_path(&index_path).exists());

            persist(&kura, &data_path, &index_path, 2, &intended);
            assert_eq!(
                read(&data_path, &index_path, 2),
                Some(DummySidecar { height: 2 })
            );
            assert_eq!(
                fs::metadata(&data_path).expect("retried data").len(),
                old_data_len + u64::try_from(intended.len()).expect("intended suffix length")
            );
        }

        // A complete payload followed by a crash before an existing entry is
        // replaced is rolled forward without disturbing unrelated data bytes.
        {
            let (_temp_dir, kura, data_path, index_path) = fixture();
            let original =
                norito::to_bytes(&DummySidecar { height: 1 }).expect("encode original payload");
            let replacement =
                norito::to_bytes(&DummySidecar { height: 9 }).expect("encode replacement payload");
            let unrelated =
                norito::to_bytes(&DummySidecar { height: 2 }).expect("encode unrelated payload");
            persist(&kura, &data_path, &index_path, 1, &original);
            persist(&kura, &data_path, &index_path, 2, &unrelated);
            let old_data_len = fs::metadata(&data_path).expect("old data metadata").len();
            let old_index_len = fs::metadata(&index_path).expect("old index metadata").len();
            let old_index = fs::read(&index_path).expect("old index bytes");
            let new_entry = SidecarIndexEntry {
                offset: old_data_len,
                len: u64::try_from(replacement.len()).expect("replacement length"),
            }
            .to_bytes()
            .to_vec();
            let intent = append_intent(
                &kura,
                &data_path,
                &index_path,
                1,
                true,
                old_data_len,
                old_index_len,
                0,
                old_index[..PIPELINE_INDEX_ENTRY_SIZE].to_vec(),
                new_entry,
                &replacement,
            );
            stage_intent(&index_path, &intent);
            OpenOptions::new()
                .append(true)
                .open(&data_path)
                .expect("open data for replacement suffix")
                .write_all(&replacement)
                .expect("stage replacement suffix");
            assert!(kura.recover_bound_progress_sidecar_artifacts(
                &data_path,
                &index_path,
                "bound recovery test",
            ));
            assert_eq!(
                read(&data_path, &index_path, 1),
                Some(DummySidecar { height: 9 })
            );
            assert_eq!(
                read(&data_path, &index_path, 2),
                Some(DummySidecar { height: 2 })
            );
            assert_eq!(
                &fs::read(&index_path).expect("recovered index")
                    [PIPELINE_INDEX_ENTRY_SIZE..PIPELINE_INDEX_ENTRY_SIZE * 2],
                &old_index[PIPELINE_INDEX_ENTRY_SIZE..PIPELINE_INDEX_ENTRY_SIZE * 2],
                "replacement recovery must preserve the later unrelated entry byte-for-byte"
            );
            assert_eq!(
                fs::metadata(&data_path)
                    .expect("recovered data metadata")
                    .len(),
                old_data_len + u64::try_from(replacement.len()).expect("replacement length")
            );
        }

        // Old binaries could crash after creating the index name or while
        // appending its final entry without a journal. Only the unambiguous
        // empty/partial suffix is rolled back; the durable source then retries.
        {
            let (_temp_dir, kura, data_path, index_path) = fixture();
            fs::write(&index_path, []).expect("stage empty orphan index");
            assert!(kura.recover_bound_progress_sidecar_artifacts(
                &data_path,
                &index_path,
                "bound recovery legacy bootstrap test",
            ));
            assert!(!index_path.exists());

            let based_payload = norito::to_bytes(&DummySidecar { height: 2 })
                .expect("encode legacy based-index payload");
            let based_header = SidecarIndexLayout::base_header(2);
            fs::write(&data_path, &based_payload).expect("stage legacy based-index data");
            fs::write(&index_path, &based_header[..24])
                .expect("stage incomplete based-index header");
            assert!(kura.recover_bound_progress_sidecar_artifacts(
                &data_path,
                &index_path,
                "bound recovery partial base-header test",
            ));
            assert_eq!(fs::metadata(&data_path).expect("base-header data").len(), 0);
            assert_eq!(
                fs::metadata(&index_path).expect("base-header index").len(),
                0
            );

            let malformed_data = b"must not be truncated";
            let mut malformed_header = SidecarIndexLayout::base_header(2);
            malformed_header[8..16].copy_from_slice(&0_u64.to_le_bytes());
            fs::write(&data_path, malformed_data).expect("stage malformed-header data");
            fs::write(&index_path, malformed_header).expect("stage malformed full header");
            assert!(!kura.recover_bound_progress_sidecar_artifacts(
                &data_path,
                &index_path,
                "bound recovery malformed full base-header test",
            ));
            assert_eq!(
                fs::read(&data_path).expect("malformed-header data retained"),
                malformed_data
            );
            assert_eq!(
                fs::read(&index_path).expect("malformed header retained"),
                malformed_header
            );
            fs::remove_file(&data_path).expect("clear malformed-header data fixture");
            fs::remove_file(&index_path).expect("clear malformed-header index fixture");

            let payload = norito::to_bytes(&DummySidecar { height: 1 })
                .expect("encode legacy partial append");
            let entry = SidecarIndexEntry {
                offset: 0,
                len: u64::try_from(payload.len()).expect("payload length"),
            }
            .to_bytes();
            fs::write(&data_path, &payload).expect("stage legacy full data");
            fs::write(&index_path, &entry[..8]).expect("stage legacy partial index");
            assert!(kura.recover_bound_progress_sidecar_artifacts(
                &data_path,
                &index_path,
                "bound recovery legacy append test",
            ));
            assert_eq!(fs::metadata(&data_path).expect("repaired data").len(), 0);
            assert_eq!(fs::metadata(&index_path).expect("repaired index").len(), 0);
            persist(&kura, &data_path, &index_path, 1, &payload);
            assert_eq!(
                read(&data_path, &index_path, 1),
                Some(DummySidecar { height: 1 })
            );
        }

        // A lone data temp precedes the index commit marker and is discarded.
        {
            let (_temp_dir, kura, data_path, index_path) = fixture();
            let main = norito::to_bytes(&DummySidecar { height: 1 }).expect("encode main");
            persist(&kura, &data_path, &index_path, 1, &main);
            let temp_data_path = data_path.with_extension("norito.tmp");
            fs::write(&temp_data_path, b"uncommitted rewrite data").expect("stage data temp");
            assert!(kura.recover_bound_progress_sidecar_artifacts(
                &data_path,
                &index_path,
                "bound recovery test",
            ));
            assert!(!temp_data_path.exists());
            assert_eq!(
                read(&data_path, &index_path, 1),
                Some(DummySidecar { height: 1 })
            );
        }

        // When both pre-publication temps are incomplete, cleanup removes the
        // index commit marker first and preserves the authoritative main pair.
        {
            let (_temp_dir, kura, data_path, index_path) = fixture();
            let main = norito::to_bytes(&DummySidecar { height: 1 }).expect("encode main");
            persist(&kura, &data_path, &index_path, 1, &main);
            let temp_data_path = data_path.with_extension("norito.tmp");
            let temp_index_path = index_path.with_extension("index.tmp");
            fs::write(&temp_data_path, b"incomplete rewrite data").expect("stage data temp");
            fs::write(&temp_index_path, [0_u8; 3]).expect("stage incomplete index temp");
            assert!(kura.recover_bound_progress_sidecar_artifacts(
                &data_path,
                &index_path,
                "bound recovery test",
            ));
            assert!(!temp_data_path.exists());
            assert!(!temp_index_path.exists());
            assert_eq!(
                read(&data_path, &index_path, 1),
                Some(DummySidecar { height: 1 })
            );
        }

        // A complete rewrite pair is published data-first and remains exact.
        {
            let (_temp_dir, kura, data_path, index_path) = fixture();
            let main = norito::to_bytes(&DummySidecar { height: 1 }).expect("encode main");
            persist(&kura, &data_path, &index_path, 1, &main);
            let replacement =
                norito::to_bytes(&DummySidecar { height: 7 }).expect("encode replacement");
            let temp_data_path = data_path.with_extension("norito.tmp");
            let temp_index_path = index_path.with_extension("index.tmp");
            fs::write(&temp_data_path, &replacement).expect("stage complete data temp");
            fs::write(
                &temp_index_path,
                SidecarIndexEntry {
                    offset: 0,
                    len: u64::try_from(replacement.len()).expect("replacement length"),
                }
                .to_bytes(),
            )
            .expect("stage complete index temp");
            assert!(kura.recover_bound_progress_sidecar_artifacts(
                &data_path,
                &index_path,
                "bound recovery test",
            ));
            assert!(!temp_data_path.exists());
            assert!(!temp_index_path.exists());
            assert_eq!(
                read(&data_path, &index_path, 1),
                Some(DummySidecar { height: 7 })
            );
        }

        // An index-only recovery marker represents data that already reached
        // its main name. It is validated against that exact payload and promoted.
        {
            let (_temp_dir, kura, data_path, index_path) = fixture();
            let main = norito::to_bytes(&DummySidecar { height: 1 }).expect("encode main");
            persist(&kura, &data_path, &index_path, 1, &main);
            let replacement =
                norito::to_bytes(&DummySidecar { height: 8 }).expect("encode replacement");
            assert_eq!(main.len(), replacement.len());
            fs::write(&data_path, &replacement).expect("model promoted rewrite data");
            let temp_index_path = index_path.with_extension("index.tmp");
            fs::write(
                &temp_index_path,
                SidecarIndexEntry {
                    offset: 0,
                    len: u64::try_from(replacement.len()).expect("replacement length"),
                }
                .to_bytes(),
            )
            .expect("stage index-only recovery marker");
            assert!(kura.recover_bound_progress_sidecar_artifacts(
                &data_path,
                &index_path,
                "bound recovery test",
            ));
            assert!(!temp_index_path.exists());
            assert_eq!(
                read(&data_path, &index_path, 1),
                Some(DummySidecar { height: 8 })
            );
        }

        let stage_prepend = |complete_payload: bool| {
            let (temp_dir, kura, data_path, index_path) = fixture();
            let height_two =
                norito::to_bytes(&DummySidecar { height: 2 }).expect("encode height two");
            let height_one =
                norito::to_bytes(&DummySidecar { height: 1 }).expect("encode height one");
            persist(&kura, &data_path, &index_path, 2, &height_two);
            let original_len = fs::metadata(&data_path).expect("main data metadata").len();
            let mut source_index = File::open(&index_path).expect("open main index");
            let source_len = source_index.metadata().expect("main index metadata").len();
            let layout = SidecarIndexLayout::read_from(&mut source_index, source_len)
                .expect("decode main based index");
            source_index
                .seek(SeekFrom::Start(layout.entries_offset))
                .expect("seek main entries");
            let temp_index_path = index_path.with_extension("index.prepend.tmp");
            let mut temp_index = File::create(&temp_index_path).expect("create prepend temp");
            temp_index
                .write_all(
                    &SidecarIndexEntry {
                        offset: original_len,
                        len: u64::try_from(height_one.len()).expect("height one length"),
                    }
                    .to_bytes(),
                )
                .expect("write prepended entry");
            std::io::copy(
                &mut source_index.take(
                    layout
                        .entry_count
                        .saturating_mul(PIPELINE_INDEX_ENTRY_SIZE_U64),
                ),
                &mut temp_index,
            )
            .expect("copy main index entries");
            let suffix_len = if complete_payload {
                height_one.len()
            } else {
                height_one.len().saturating_sub(1)
            };
            OpenOptions::new()
                .append(true)
                .open(&data_path)
                .expect("open main data for prepend suffix")
                .write_all(&height_one[..suffix_len])
                .expect("stage prepend payload suffix");
            (
                temp_dir,
                kura,
                data_path,
                index_path,
                temp_index_path,
                original_len,
            )
        };

        // A complete prepend suffix is recovered by publishing its exact index.
        {
            let (_temp_dir, kura, data_path, index_path, temp_index_path, _) = stage_prepend(true);
            assert!(kura.recover_bound_progress_sidecar_artifacts(
                &data_path,
                &index_path,
                "bound recovery test",
            ));
            assert!(!temp_index_path.exists());
            assert_eq!(
                read(&data_path, &index_path, 1),
                Some(DummySidecar { height: 1 })
            );
            assert_eq!(
                read(&data_path, &index_path, 2),
                Some(DummySidecar { height: 2 })
            );
        }

        // A partial prepend suffix was never published. Recovery keeps the
        // main index authoritative and truncates the incomplete payload.
        {
            let (_temp_dir, kura, data_path, index_path, temp_index_path, original_len) =
                stage_prepend(false);
            assert!(kura.recover_bound_progress_sidecar_artifacts(
                &data_path,
                &index_path,
                "bound recovery test",
            ));
            assert!(!temp_index_path.exists());
            assert_eq!(
                fs::metadata(&data_path)
                    .expect("rolled-back data metadata")
                    .len(),
                original_len
            );
            assert_eq!(read(&data_path, &index_path, 1), None);
            assert_eq!(
                read(&data_path, &index_path, 2),
                Some(DummySidecar { height: 2 })
            );
        }

        // Absence is only meaningful in the directory hierarchy that was
        // bound. Replacing that hierarchy after binding must not turn a
        // missing recovery artifact into a successful absence observation.
        {
            let (temp_dir, kura, data_path, index_path) = fixture();
            let main = norito::to_bytes(&DummySidecar { height: 1 }).expect("encode main");
            persist(&kura, &data_path, &index_path, 1, &main);
            let namespace = kura
                .open_bound_progress_namespace(&data_path, &index_path)
                .expect("bind progress namespace before replacement");
            let sidecar_dir = data_path.parent().expect("sidecar parent").to_path_buf();
            let displaced_dir = temp_dir.path().join("displaced-lane-artifacts");
            fs::rename(&sidecar_dir, &displaced_dir).expect("displace bound namespace");
            fs::create_dir(&sidecar_dir).expect("install replacement namespace");

            let missing_temp = data_path.with_extension("norito.tmp");
            assert!(
                kura.open_optional_bound_progress_file(&namespace, &missing_temp)
                    .is_err(),
                "a missing artifact in a replacement namespace must fail closed"
            );
            assert_eq!(
                fs::read(displaced_dir.join(data_path.file_name().expect("data file name")))
                    .expect("displaced main data retained"),
                main
            );
            assert!(
                !data_path.exists() && !index_path.exists(),
                "the replacement namespace must remain untouched"
            );
        }

        // Unlike a build, a malformed published intent is an ambiguous
        // durable mutation authority and must fail closed without touching the
        // otherwise valid main pair.
        {
            let (_temp_dir, kura, data_path, index_path) = fixture();
            let main = norito::to_bytes(&DummySidecar { height: 1 }).expect("encode main");
            persist(&kura, &data_path, &index_path, 1, &main);
            let data_before = fs::read(&data_path).expect("main data bytes");
            let index_before = fs::read(&index_path).expect("main index bytes");
            let intent_path = Kura::bound_progress_append_intent_path(&index_path);
            fs::write(&intent_path, b"malformed published intent").expect("stage malformed intent");
            assert!(!kura.recover_bound_progress_sidecar_artifacts(
                &data_path,
                &index_path,
                "bound recovery malformed intent test",
            ));
            assert_eq!(
                fs::read(&data_path).expect("main data retained"),
                data_before
            );
            assert_eq!(
                fs::read(&index_path).expect("main index retained"),
                index_before
            );
            assert!(
                intent_path.exists(),
                "malformed authority must remain for diagnosis"
            );
            let namespace = kura
                .open_bound_progress_namespace(&data_path, &index_path)
                .expect("bind malformed-intent namespace");
            assert_eq!(
                kura.recover_bound_progress_sidecar_artifacts_in_namespace_classified(
                    &namespace,
                    &data_path,
                    &index_path,
                    "bound recovery malformed intent classification test",
                ),
                Err(BoundProgressRecoveryFailure::InvalidData)
            );
        }

        // The pre-release V1 record had the same positional fields except for
        // the relative namespace identity. Even when that old layout is
        // canonically encoded and sealed under the intent digest domain, it is
        // not first-release authority: rejection precedes both rollback and
        // roll-forward mutation and retains the marker for diagnosis.
        {
            #[derive(Encode)]
            struct PreNamespaceBoundProgressAppendIntentV1 {
                version: u16,
                data_file: String,
                index_file: String,
                height: u64,
                pair_was_present: bool,
                old_data_len: u64,
                new_data_len: u64,
                payload_hash: Hash,
                old_index_len: u64,
                new_index_len: u64,
                index_write_offset: u64,
                old_index_bytes: Vec<u8>,
                new_index_bytes: Vec<u8>,
                integrity_hash: Hash,
            }

            let (_temp_dir, kura, data_path, index_path) = fixture();
            let first = norito::to_bytes(&DummySidecar { height: 1 }).expect("encode first");
            let second = norito::to_bytes(&DummySidecar { height: 2 }).expect("encode second");
            persist(&kura, &data_path, &index_path, 1, &first);
            let old_data_len = fs::metadata(&data_path)
                .expect("pre-namespace data metadata")
                .len();
            let old_index_len = fs::metadata(&index_path)
                .expect("pre-namespace index metadata")
                .len();
            let current = append_intent(
                &kura,
                &data_path,
                &index_path,
                2,
                true,
                old_data_len,
                old_index_len,
                old_index_len,
                Vec::new(),
                SidecarIndexEntry {
                    offset: old_data_len,
                    len: u64::try_from(second.len()).expect("second payload length"),
                }
                .to_bytes()
                .to_vec(),
                &second,
            );
            let mut pre_release = PreNamespaceBoundProgressAppendIntentV1 {
                version: current.version,
                data_file: current.data_file.clone(),
                index_file: current.index_file.clone(),
                height: current.height,
                pair_was_present: current.pair_was_present,
                old_data_len: current.old_data_len,
                new_data_len: current.new_data_len,
                payload_hash: current.payload_hash,
                old_index_len: current.old_index_len,
                new_index_len: current.new_index_len,
                index_write_offset: current.index_write_offset,
                old_index_bytes: current.old_index_bytes.clone(),
                new_index_bytes: current.new_index_bytes.clone(),
                integrity_hash: Hash::prehashed([0; Hash::LENGTH]),
            };
            let encode_with_current_schema = |intent: &PreNamespaceBoundProgressAppendIntentV1| {
                let mut bytes =
                    norito::to_bytes(intent).expect("encode pre-namespace intent layout");
                let schema =
                    <BoundProgressAppendIntentV1 as norito::core::NoritoSerialize>::schema_hash();
                let schema_start = MAGIC.len() + 2;
                let schema_end = schema_start + schema.len();
                assert!(bytes.len() >= Header::SIZE);
                bytes[schema_start..schema_end].copy_from_slice(&schema);
                bytes
            };
            // Force the current same-type schema onto the historical payload
            // before sealing it. The regression therefore reaches positional
            // decoding and integrity validation instead of passing only
            // because this local fixture type has a different schema name.
            let pre_release_preimage = encode_with_current_schema(&pre_release);
            pre_release.integrity_hash = Hash::new_from_chunks(&[
                BOUND_PROGRESS_APPEND_INTENT_DIGEST_DOMAIN,
                &pre_release_preimage,
            ]);
            let pre_release_bytes = encode_with_current_schema(&pre_release);
            let intent_path = Kura::bound_progress_append_intent_path(&index_path);
            fs::write(&intent_path, pre_release_bytes).expect("stage pre-namespace intent");
            OpenOptions::new()
                .append(true)
                .open(&data_path)
                .expect("open data for pre-namespace suffix")
                .write_all(&second)
                .expect("stage exact pre-namespace suffix");
            let data_before = fs::read(&data_path).expect("staged pre-namespace data");
            let index_before = fs::read(&index_path).expect("pre-namespace index bytes");

            assert!(!kura.recover_bound_progress_sidecar_artifacts(
                &data_path,
                &index_path,
                "bound recovery pre-namespace intent test",
            ));
            assert_eq!(
                fs::read(&data_path).expect("pre-namespace data retained"),
                data_before
            );
            assert_eq!(
                fs::read(&index_path).expect("pre-namespace index retained"),
                index_before
            );
            assert!(
                intent_path.exists(),
                "rejected pre-namespace authority must remain for diagnosis"
            );
            let namespace = kura
                .open_bound_progress_namespace(&data_path, &index_path)
                .expect("bind pre-namespace intent namespace");
            assert_eq!(
                kura.recover_bound_progress_sidecar_artifacts_in_namespace_classified(
                    &namespace,
                    &data_path,
                    &index_path,
                    "bound recovery pre-namespace intent classification test",
                ),
                Err(BoundProgressRecoveryFailure::InvalidData)
            );
        }

        // A canonical Norito record is still invalid authority when its index
        // postimage does not encode the journaled payload at the target
        // height. Classification must be terminal and recovery must not first
        // corrupt the valid preimage.
        {
            let (_temp_dir, kura, data_path, index_path) = fixture();
            let main = norito::to_bytes(&DummySidecar { height: 1 }).expect("encode main");
            let replacement =
                norito::to_bytes(&DummySidecar { height: 9 }).expect("encode replacement");
            persist(&kura, &data_path, &index_path, 1, &main);
            let data_before = fs::read(&data_path).expect("main data bytes");
            let index_before = fs::read(&index_path).expect("main index bytes");
            let intent = append_intent(
                &kura,
                &data_path,
                &index_path,
                1,
                true,
                u64::try_from(data_before.len()).expect("main data length"),
                u64::try_from(index_before.len()).expect("main index length"),
                0,
                index_before[..PIPELINE_INDEX_ENTRY_SIZE].to_vec(),
                SidecarIndexEntry {
                    offset: u64::try_from(data_before.len())
                        .expect("main data length")
                        .checked_add(1)
                        .expect("wrong offset fixture"),
                    len: u64::try_from(replacement.len()).expect("replacement length"),
                }
                .to_bytes()
                .to_vec(),
                &replacement,
            );
            stage_intent(&index_path, &intent);
            let intent_path = Kura::bound_progress_append_intent_path(&index_path);
            assert!(!kura.recover_bound_progress_sidecar_artifacts(
                &data_path,
                &index_path,
                "bound recovery semantic intent test",
            ));
            assert_eq!(
                fs::read(&data_path).expect("main data retained"),
                data_before
            );
            assert_eq!(
                fs::read(&index_path).expect("main index retained"),
                index_before
            );
            assert!(intent_path.exists(), "invalid authority must remain");
            let namespace = kura
                .open_bound_progress_namespace(&data_path, &index_path)
                .expect("bind semantic-intent namespace");
            assert_eq!(
                kura.recover_bound_progress_sidecar_artifacts_in_namespace_classified(
                    &namespace,
                    &data_path,
                    &index_path,
                    "bound recovery semantic intent classification test",
                ),
                Err(BoundProgressRecoveryFailure::InvalidData)
            );
        }

        // Canonical encoding alone cannot make a corrupted undo window safe.
        // The record hash is verified before rollback is allowed to rewrite
        // an existing index entry.
        {
            let (_temp_dir, kura, data_path, index_path) = fixture();
            let first = norito::to_bytes(&DummySidecar { height: 1 }).expect("encode first");
            let second = norito::to_bytes(&DummySidecar { height: 2 }).expect("encode second");
            let replacement =
                norito::to_bytes(&DummySidecar { height: 9 }).expect("encode replacement");
            persist(&kura, &data_path, &index_path, 1, &first);
            persist(&kura, &data_path, &index_path, 2, &second);
            let data_before = fs::read(&data_path).expect("main data bytes");
            let index_before = fs::read(&index_path).expect("main index bytes");
            let mut intent = append_intent(
                &kura,
                &data_path,
                &index_path,
                1,
                true,
                u64::try_from(data_before.len()).expect("main data length"),
                u64::try_from(index_before.len()).expect("main index length"),
                0,
                index_before[..PIPELINE_INDEX_ENTRY_SIZE].to_vec(),
                SidecarIndexEntry {
                    offset: u64::try_from(data_before.len()).expect("main data length"),
                    len: u64::try_from(replacement.len()).expect("replacement length"),
                }
                .to_bytes()
                .to_vec(),
                &replacement,
            );
            intent.old_index_bytes.fill(0);
            stage_intent(&index_path, &intent);
            assert!(!kura.recover_bound_progress_sidecar_artifacts(
                &data_path,
                &index_path,
                "bound recovery corrupted undo test",
            ));
            assert_eq!(
                fs::read(&data_path).expect("main data retained"),
                data_before
            );
            assert_eq!(
                fs::read(&index_path).expect("main index retained"),
                index_before
            );
            let namespace = kura
                .open_bound_progress_namespace(&data_path, &index_path)
                .expect("bind corrupted-undo namespace");
            assert_eq!(
                kura.recover_bound_progress_sidecar_artifacts_in_namespace_classified(
                    &namespace,
                    &data_path,
                    &index_path,
                    "bound recovery corrupted undo classification test",
                ),
                Err(BoundProgressRecoveryFailure::InvalidData)
            );
        }

        // An intact, correctly sealed intent cannot be transplanted between
        // sibling lane directories even when the main pairs have identical
        // basenames, lengths, layouts, contents, and an exact roll-forward
        // payload suffix.
        {
            let (_temp_dir, kura, _data_path, _index_path) = fixture();
            let lane_root = kura.store_root().join("blocks").join("lane");
            let source_dir = lane_root.join("lane_001").join(LANE_ARTIFACTS_DIR_NAME);
            let target_dir = lane_root
                .join("lane_001_copy")
                .join(LANE_ARTIFACTS_DIR_NAME);
            fs::create_dir_all(&source_dir).expect("create source lane directory");
            fs::create_dir_all(&target_dir).expect("create target lane directory");
            let source_data = source_dir.join("matching-progress.norito");
            let source_index = source_dir.join("matching-progress.index");
            let target_data = target_dir.join("matching-progress.norito");
            let target_index = target_dir.join("matching-progress.index");
            let first = norito::to_bytes(&DummySidecar { height: 1 }).expect("encode first");
            let second = norito::to_bytes(&DummySidecar { height: 2 }).expect("encode second");
            persist(&kura, &source_data, &source_index, 1, &first);
            persist(&kura, &target_data, &target_index, 1, &first);
            let source_namespace = kura
                .open_bound_progress_namespace(&source_data, &source_index)
                .expect("bind source lane namespace");
            let target_namespace = kura
                .open_bound_progress_namespace(&target_data, &target_index)
                .expect("bind target lane namespace");
            let source_identity = source_namespace
                .stable_relative_components(&source_data, &source_index)
                .expect("source relative identity");
            let target_identity = target_namespace
                .stable_relative_components(&target_data, &target_index)
                .expect("target relative identity");
            assert_ne!(
                source_identity, target_identity,
                "sibling-prefix lane directories must have distinct identities"
            );
            let old_data_len = fs::metadata(&source_data)
                .expect("source data metadata")
                .len();
            let old_index_len = fs::metadata(&source_index)
                .expect("source index metadata")
                .len();
            assert_eq!(
                fs::metadata(&target_data)
                    .expect("target data metadata")
                    .len(),
                old_data_len
            );
            assert_eq!(
                fs::metadata(&target_index)
                    .expect("target index metadata")
                    .len(),
                old_index_len
            );
            let intent = append_intent(
                &kura,
                &source_data,
                &source_index,
                2,
                true,
                old_data_len,
                old_index_len,
                old_index_len,
                Vec::new(),
                SidecarIndexEntry {
                    offset: old_data_len,
                    len: u64::try_from(second.len()).expect("second payload length"),
                }
                .to_bytes()
                .to_vec(),
                &second,
            );
            assert_eq!(
                intent.validate_for(&source_namespace, &source_data, &source_index),
                Ok(()),
                "the source intent must be canonical and valid in its bound namespace"
            );
            assert_eq!(
                intent.validate_for(&target_namespace, &target_data, &target_index),
                Err("bound progress append intent names the wrong relative namespace"),
                "the intact source intent must fail only at the target namespace binding"
            );
            let mut tampered = intent.clone();
            tampered.namespace_components = target_identity.clone();
            assert_eq!(
                tampered.validate_for(&target_namespace, &target_data, &target_index),
                Err("bound progress append intent integrity hash is invalid"),
                "the integrity digest must cover the relative namespace identity"
            );
            for wrong_identity in [
                Vec::new(),
                vec![".".to_owned()],
                vec!["..".to_owned()],
                vec!["/absolute".to_owned()],
                vec!["lane_001/lane-artifacts".to_owned()],
                source_identity.clone(),
            ] {
                let mut forged = intent.clone();
                forged.namespace_components = wrong_identity;
                let forged = forged.seal();
                assert_eq!(
                    forged.validate_for(&target_namespace, &target_data, &target_index),
                    Err("bound progress append intent names the wrong relative namespace"),
                    "a resealed non-matching structured identity must still be rejected"
                );
            }
            stage_intent(&target_index, &intent);
            OpenOptions::new()
                .append(true)
                .open(&target_data)
                .expect("open target data for transplanted suffix")
                .write_all(&second)
                .expect("stage exact transplanted suffix");
            let staged_data = fs::read(&target_data).expect("staged target data");
            let index_before = fs::read(&target_index).expect("target index before recovery");
            assert!(!kura.recover_bound_progress_sidecar_artifacts(
                &target_data,
                &target_index,
                "bound recovery cross-namespace transplant test",
            ));
            assert_eq!(
                fs::read(&target_data).expect("transplant target data retained"),
                staged_data,
                "rejection must precede rollback or roll-forward data mutation"
            );
            assert_eq!(
                fs::read(&target_index).expect("transplant target index retained"),
                index_before,
                "rejection must precede index mutation"
            );
            assert_eq!(read(&target_data, &target_index, 2), None);
            let intent_path = Kura::bound_progress_append_intent_path(&target_index);
            assert!(intent_path.exists(), "transplanted authority must remain");
            assert_eq!(
                kura.recover_bound_progress_sidecar_artifacts_in_namespace_classified(
                    &target_namespace,
                    &target_data,
                    &target_index,
                    "bound recovery cross-namespace transplant classification test",
                ),
                Err(BoundProgressRecoveryFailure::InvalidData)
            );
        }

        // Active and retired geometry namespaces are equally non-
        // interchangeable. This covers a deeper archive path rather than
        // relying only on sibling-name separation.
        {
            let (_temp_dir, kura, _data_path, _index_path) = fixture();
            let active_dir = kura
                .store_root()
                .join("blocks")
                .join("lane")
                .join("lane_0000000001")
                .join(LANE_ARTIFACTS_DIR_NAME);
            let archive_dir = kura
                .store_root()
                .join("retired")
                .join("lane_geometry")
                .join("transition_fixture")
                .join("lane_0000000001")
                .join(LANE_ARTIFACTS_DIR_NAME);
            fs::create_dir_all(&active_dir).expect("create active lane directory");
            fs::create_dir_all(&archive_dir).expect("create retired lane archive directory");
            let active_data = active_dir.join("matching-progress.norito");
            let active_index = active_dir.join("matching-progress.index");
            let archive_data = archive_dir.join("matching-progress.norito");
            let archive_index = archive_dir.join("matching-progress.index");
            let first = norito::to_bytes(&DummySidecar { height: 1 }).expect("encode first");
            let second = norito::to_bytes(&DummySidecar { height: 2 }).expect("encode second");
            persist(&kura, &active_data, &active_index, 1, &first);
            persist(&kura, &archive_data, &archive_index, 1, &first);
            let old_data_len = fs::metadata(&active_data)
                .expect("active data metadata")
                .len();
            let old_index_len = fs::metadata(&active_index)
                .expect("active index metadata")
                .len();
            let intent = append_intent(
                &kura,
                &active_data,
                &active_index,
                2,
                true,
                old_data_len,
                old_index_len,
                old_index_len,
                Vec::new(),
                SidecarIndexEntry {
                    offset: old_data_len,
                    len: u64::try_from(second.len()).expect("second payload length"),
                }
                .to_bytes()
                .to_vec(),
                &second,
            );
            stage_intent(&archive_index, &intent);
            OpenOptions::new()
                .append(true)
                .open(&archive_data)
                .expect("open archive data for transplanted suffix")
                .write_all(&second)
                .expect("stage exact archive suffix");
            let data_before = fs::read(&archive_data).expect("staged archive data");
            let index_before = fs::read(&archive_index).expect("archive index before recovery");
            assert!(!kura.recover_bound_progress_sidecar_artifacts(
                &archive_data,
                &archive_index,
                "bound recovery active/archive transplant test",
            ));
            assert_eq!(
                fs::read(&archive_data).expect("archive data retained"),
                data_before
            );
            assert_eq!(
                fs::read(&archive_index).expect("archive index retained"),
                index_before
            );
            assert_eq!(read(&archive_data, &archive_index, 2), None);
            let archive_namespace = kura
                .open_bound_progress_namespace(&archive_data, &archive_index)
                .expect("bind archive transplant namespace");
            assert_eq!(
                kura.recover_bound_progress_sidecar_artifacts_in_namespace_classified(
                    &archive_namespace,
                    &archive_data,
                    &archive_index,
                    "bound recovery active/archive transplant classification test",
                ),
                Err(BoundProgressRecoveryFailure::InvalidData)
            );
            assert!(
                Kura::bound_progress_append_intent_path(&archive_index).exists(),
                "rejected archive authority must remain for diagnosis"
            );
        }

        // The identity deliberately excludes the absolute Kura root. Stop the
        // owner, rename the actual root directory (including the intent and
        // main pair), and reopen it at the new location: the same directory
        // object must retain its relative authority. This relative identity
        // assumes the Kura directory is trusted; without a persisted store ID,
        // an independently copied root with the same relative tree is
        // intentionally indistinguishable from this relocation.
        {
            let relocation_parent = TempDir::new().expect("create relocation parent");
            let relocation_parent_path =
                fs::canonicalize(relocation_parent.path()).expect("canonicalize relocation parent");
            let source_root_path = relocation_parent_path.join("source-kura");
            let relocated_root_path = relocation_parent_path.join("relocated-kura");
            let source_config = kura_config_for_path(&source_root_path, BLOCKS_IN_MEMORY);
            let lane_config = RuntimeLaneConfig::default();
            let (source_kura, _) = Kura::new(&source_config, &lane_config)
                .expect("create source Kura for root relocation");
            let source_root = source_kura.store_root();
            let relative_sidecar_dir = PathBuf::from("blocks")
                .join("lane")
                .join(LANE_ARTIFACTS_DIR_NAME);
            let source_sidecar_dir = source_root.join(&relative_sidecar_dir);
            fs::create_dir_all(&source_sidecar_dir).expect("create relocated sidecar directory");
            let source_data = source_sidecar_dir.join("relocated-progress.norito");
            let source_index = source_sidecar_dir.join("relocated-progress.index");
            let first = norito::to_bytes(&DummySidecar { height: 1 }).expect("encode first");
            let second = norito::to_bytes(&DummySidecar { height: 2 }).expect("encode second");
            persist(&source_kura, &source_data, &source_index, 1, &first);
            let source_namespace = source_kura
                .open_bound_progress_namespace(&source_data, &source_index)
                .expect("bind source relocated namespace");
            let source_identity = source_namespace
                .stable_relative_components(&source_data, &source_index)
                .expect("source relocated identity");
            let old_data_len = fs::metadata(&source_data)
                .expect("source data metadata")
                .len();
            let old_index_len = fs::metadata(&source_index)
                .expect("source index metadata")
                .len();
            let intent = append_intent(
                &source_kura,
                &source_data,
                &source_index,
                2,
                true,
                old_data_len,
                old_index_len,
                old_index_len,
                Vec::new(),
                SidecarIndexEntry {
                    offset: old_data_len,
                    len: u64::try_from(second.len()).expect("second payload length"),
                }
                .to_bytes()
                .to_vec(),
                &second,
            );
            stage_intent(&source_index, &intent);
            OpenOptions::new()
                .append(true)
                .open(&source_data)
                .expect("open pre-relocation data")
                .write_all(&second)
                .expect("stage pre-relocation suffix");
            let source_root_metadata = fs::metadata(&source_root).expect("source root metadata");
            drop(source_namespace);
            drop(source_kura);

            fs::rename(&source_root, &relocated_root_path).expect("relocate whole Kura root");
            assert!(!source_root.exists(), "the old root path must be absent");
            let relocated_root_metadata =
                fs::metadata(&relocated_root_path).expect("relocated root metadata");
            assert_eq!(
                (source_root_metadata.dev(), source_root_metadata.ino(),),
                (relocated_root_metadata.dev(), relocated_root_metadata.ino(),),
                "root relocation must preserve the directory object"
            );

            let relocated_config = kura_config_for_path(&relocated_root_path, BLOCKS_IN_MEMORY);
            let (relocated_kura, _) =
                Kura::new(&relocated_config, &lane_config).expect("reopen relocated Kura root");
            let relocated_root = relocated_kura.store_root();
            let relocated_sidecar_dir = relocated_root.join(&relative_sidecar_dir);
            let relocated_data = relocated_sidecar_dir.join("relocated-progress.norito");
            let relocated_index = relocated_sidecar_dir.join("relocated-progress.index");
            assert!(
                Kura::bound_progress_append_intent_path(&relocated_index).is_file(),
                "the published intent must move with the Kura root"
            );
            let relocated_namespace = relocated_kura
                .open_bound_progress_namespace(&relocated_data, &relocated_index)
                .expect("bind relocated namespace");
            assert_eq!(
                relocated_namespace
                    .stable_relative_components(&relocated_data, &relocated_index)
                    .expect("relocated identity"),
                source_identity
            );
            assert!(
                relocated_kura.recover_bound_progress_sidecar_artifacts_in_namespace(
                    &relocated_namespace,
                    &relocated_data,
                    &relocated_index,
                    "bound recovery relocated-root intent test",
                )
            );
            assert_eq!(
                read(&relocated_data, &relocated_index, 2),
                Some(DummySidecar { height: 2 })
            );

            let root_data = relocated_root.join("root-progress.norito");
            let root_index = relocated_root.join("root-progress.index");
            let root_namespace = relocated_kura
                .open_bound_progress_namespace(&root_data, &root_index)
                .expect("bind root-level progress namespace");
            assert!(
                root_namespace
                    .stable_relative_components(&root_data, &root_index)
                    .expect("root-level relative identity")
                    .is_empty(),
                "a root-level pair has an unambiguous empty parent identity"
            );
        }

        // Every predictable recovery name rejects a symlink without changing
        // either the main pair or the external target.
        for extension in [
            "norito.tmp",
            "index.tmp",
            "index.prepend.tmp",
            "index.append.build.tmp",
            "index.append.intent.tmp",
        ] {
            let (temp_dir, kura, data_path, index_path) = fixture();
            let main = norito::to_bytes(&DummySidecar { height: 1 }).expect("encode main");
            persist(&kura, &data_path, &index_path, 1, &main);
            let data_before = fs::read(&data_path).expect("main data bytes");
            let index_before = fs::read(&index_path).expect("main index bytes");
            let data_identity = fs::metadata(&data_path).expect("main data identity");
            let index_identity = fs::metadata(&index_path).expect("main index identity");
            let sentinel = temp_dir.path().join(format!("outside-{extension}"));
            fs::write(&sentinel, b"external sentinel").expect("write external sentinel");
            let recovery_path = if extension == "norito.tmp" {
                data_path.with_extension(extension)
            } else {
                index_path.with_extension(extension)
            };
            symlink(&sentinel, &recovery_path).expect("install recovery symlink");
            assert!(!kura.recover_bound_progress_sidecar_artifacts(
                &data_path,
                &index_path,
                "bound recovery symlink test",
            ));
            assert_eq!(
                fs::read(&sentinel).expect("external sentinel retained"),
                b"external sentinel"
            );
            assert!(
                fs::symlink_metadata(&recovery_path)
                    .expect("recovery symlink retained")
                    .file_type()
                    .is_symlink()
            );
            assert_eq!(
                fs::read(&data_path).expect("main data retained"),
                data_before
            );
            assert_eq!(
                fs::read(&index_path).expect("main index retained"),
                index_before
            );
            let data_after = fs::metadata(&data_path).expect("main data after recovery rejection");
            let index_after =
                fs::metadata(&index_path).expect("main index after recovery rejection");
            assert_eq!(
                (data_after.dev(), data_after.ino()),
                (data_identity.dev(), data_identity.ino())
            );
            assert_eq!(
                (index_after.dev(), index_after.ino()),
                (index_identity.dev(), index_identity.ino())
            );
        }

        // The two new predictable names also reject multi-link and
        // non-regular substitutions, not just symlinks.
        {
            let (temp_dir, kura, data_path, index_path) = fixture();
            let main = norito::to_bytes(&DummySidecar { height: 1 }).expect("encode main");
            persist(&kura, &data_path, &index_path, 1, &main);
            let data_before = fs::read(&data_path).expect("main data bytes");
            let index_before = fs::read(&index_path).expect("main index bytes");
            let sentinel = temp_dir.path().join("append-build-hard-link-sentinel");
            fs::write(&sentinel, b"hard-link sentinel").expect("write hard-link sentinel");
            let build_path = Kura::bound_progress_append_build_path(&index_path);
            fs::hard_link(&sentinel, &build_path).expect("install append-build hard link");
            assert!(!kura.recover_bound_progress_sidecar_artifacts(
                &data_path,
                &index_path,
                "bound recovery append-build hard-link test",
            ));
            assert_eq!(
                fs::read(&sentinel).expect("hard-link sentinel retained"),
                b"hard-link sentinel"
            );
            assert_eq!(
                fs::metadata(&sentinel).expect("hard-link metadata").nlink(),
                2
            );
            assert_eq!(
                fs::read(&data_path).expect("main data retained"),
                data_before
            );
            assert_eq!(
                fs::read(&index_path).expect("main index retained"),
                index_before
            );
        }

        {
            let (_temp_dir, kura, data_path, index_path) = fixture();
            let main = norito::to_bytes(&DummySidecar { height: 1 }).expect("encode main");
            persist(&kura, &data_path, &index_path, 1, &main);
            let data_before = fs::read(&data_path).expect("main data bytes");
            let index_before = fs::read(&index_path).expect("main index bytes");
            let intent_path = Kura::bound_progress_append_intent_path(&index_path);
            fs::create_dir(&intent_path).expect("install append-intent directory");
            assert!(!kura.recover_bound_progress_sidecar_artifacts(
                &data_path,
                &index_path,
                "bound recovery append-intent directory test",
            ));
            assert!(intent_path.is_dir());
            assert_eq!(
                fs::read(&data_path).expect("main data retained"),
                data_before
            );
            assert_eq!(
                fs::read(&index_path).expect("main index retained"),
                index_before
            );
        }
    }

    fn direct_receipt_snapshot_preserves_sparse_and_mixed_format_entries() {
        for include_current_receipt in [false, true] {
            let temp_dir = TempDir::new().expect("create temp dir");
            let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
            let lane_config = two_lane_runtime_config();
            let lane_id = LaneId::from(1);
            let lane_entry = lane_config.entry(lane_id).expect("lane entry");
            let mut generator = DummyBlocks::new();
            let first = dummy_block_with_lane_payload_ownership_from_generator(
                &mut generator,
                lane_id,
                lane_entry.dataspace_id,
                1,
            );
            let mut second = dummy_block_with_lane_payload_ownership_from_generator(
                &mut generator,
                lane_id,
                lane_entry.dataspace_id,
                2,
            )
            .as_ref()
            .clone();
            if include_current_receipt {
                attach_ok_results_to_block(&mut second);
            }
            let second = Arc::new(second);
            let third = dummy_block_with_lane_payload_ownership_from_generator(
                &mut generator,
                lane_id,
                lane_entry.dataspace_id,
                3,
            );
            let proposal = |block: &SignedBlock| {
                lane_block_proposal_from_ownership(
                    block
                        .execution_context()
                        .expect("lane execution context")
                        .lane_payload_ownerships
                        .first()
                        .expect("lane ownership"),
                )
            };
            let first_proposal = proposal(&first);
            let second_proposal = proposal(&second);
            let third_proposal = proposal(&third);

            let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
            kura.store_block(Arc::clone(&first))
                .expect("store first lane block");
            kura.store_block(Arc::clone(&second))
                .expect("store second lane block");
            kura.store_block(Arc::clone(&third))
                .expect("store third lane block");

            let persist_direct =
                |proposal: &LaneBlockProposalV1,
                 preflight_state_height: u64,
                 state_hash_marker: &'static [u8]| {
                    let recovered = kura
                        .recover_lane_block_payload(proposal)
                        .expect("recover direct lane payload");
                    kura.persist_lane_block_execution_input(&recovered)
                        .expect("persist direct execution input");
                    let input = kura
                        .read_lane_block_execution_input(
                            proposal.descriptor.lane_id,
                            proposal.descriptor.lane_block_height,
                        )
                        .expect("read direct execution input");
                    let state_hash = Some(HashOf::<BlockHeader>::from_untyped_unchecked(
                        Hash::new(state_hash_marker),
                    ));
                    let result = TransactionResult::new(TransactionResultInner::Ok(
                        DataTriggerSequence::new(),
                    ));
                    kura.persist_lane_block_execution_preflight(
                        &input,
                        preflight_state_height,
                        state_hash,
                        vec![result],
                    )
                    .expect("persist direct execution preflight");
                    let preflight = kura
                        .read_lane_block_execution_preflight(
                            proposal.descriptor.lane_id,
                            proposal.descriptor.lane_block_height,
                        )
                        .expect("read direct execution preflight");
                    kura.persist_direct_lane_block_application_receipt(&input, &preflight)
                        .expect("persist direct application receipt");
                };
            persist_direct(&first_proposal, 11, b"direct-snapshot-first-state");
            if include_current_receipt {
                kura.persist_lane_block_application_receipt(&second_proposal)
                    .expect("persist intervening current-format receipt");
            }
            persist_direct(&third_proposal, 13, b"direct-snapshot-third-state");

            for entry in kura
                .lane_storage_entries
                .lock()
                .values()
                .cloned()
                .collect::<Vec<_>>()
            {
                let (data_path, index_path) =
                    Kura::lane_block_application_receipt_paths_for_entry(&entry, &kura.store_root);
                let mut pair = kura
                    .open_bound_progress_pair(&data_path, &index_path)
                    .unwrap_or_else(|error| {
                        panic!(
                            "lane {} receipt pair must bind: {error:?}",
                            entry.lane_id.as_u32()
                        )
                    });
                match &mut pair {
                    BoundProgressPair::Absent(namespace) => assert!(
                        kura.sync_bound_progress_absence(
                            namespace,
                            "direct snapshot fixture absence"
                        ),
                        "lane {} absent receipt namespace must attest",
                        entry.lane_id.as_u32()
                    ),
                    BoundProgressPair::Present(bound) => {
                        let heights = kura
                            .bound_indexed_sidecar_payload_heights(
                                bound,
                                "direct snapshot fixture",
                                usize::MAX,
                            )
                            .unwrap_or_else(|error| {
                                panic!(
                                    "lane {} receipt heights must enumerate: {error:?}",
                                    entry.lane_id.as_u32()
                                )
                            });
                        for height in heights {
                            assert!(
                                kura.read_lane_block_application_receipt_from_bound_locked(
                                    entry.lane_id,
                                    height,
                                    bound,
                                )
                                .is_some(),
                                "lane {} height {height} receipt must decode",
                                entry.lane_id.as_u32()
                            );
                        }
                        assert!(
                            kura.sync_bound_progress_sidecar(
                                bound,
                                "direct snapshot fixture receipt"
                            ),
                            "lane {} receipt pair must attest",
                            entry.lane_id.as_u32()
                        );
                    }
                }
            }

            let structural = kura
                .active_lane_block_application_receipts_structural_snapshot()
                .expect("mixed/sparse structural snapshot must be readable");
            assert_eq!(
                structural
                    .iter()
                    .map(|receipt| receipt.proposal.descriptor.lane_block_height)
                    .collect::<Vec<_>>(),
                if include_current_receipt {
                    vec![1, 2, 3]
                } else {
                    vec![1, 3]
                },
                "occupied-entry enumeration must ignore sparse holes without dropping receipts"
            );
            assert!(
                structural
                    .iter()
                    .filter(|receipt| {
                        receipt.format == LaneBlockApplicationReceiptArtifactFormat::DirectExecution
                    })
                    .all(|receipt| kura
                        .lane_block_application_receipt_matches_available_evidence(receipt, true)),
                "every structurally captured direct receipt must retain its preflight evidence"
            );
            assert_eq!(
                kura.active_lane_block_application_receipts_structural_snapshot(),
                Some(structural),
                "a second full occupied-entry scan must match exactly"
            );
            let snapshot = kura.direct_lane_block_application_receipts_snapshot();
            assert_eq!(
                snapshot
                    .iter()
                    .map(|receipt| receipt.proposal.descriptor.lane_block_height)
                    .collect::<Vec<_>>(),
                vec![1, 3],
                "{} must not hide either direct receipt",
                if include_current_receipt {
                    "an intervening Current receipt"
                } else {
                    "a sparse zero index entry"
                }
            );
            assert!(snapshot.iter().all(|receipt| {
                receipt.format == LaneBlockApplicationReceiptArtifactFormat::DirectExecution
            }));
        }
    }

    mod progress_witness_durability {
        #[test]
        fn absent_progress_namespace_requires_every_directory_barrier() {
            super::absent_progress_namespace_requires_every_directory_barrier();
        }

        #[test]
        fn certified_lane_block_strict_retry_reissues_every_barrier() {
            super::certified_lane_block_strict_retry_reissues_every_barrier();
        }

        #[test]
        fn direct_receipt_snapshot_preserves_sparse_and_mixed_format_entries() {
            super::direct_receipt_snapshot_preserves_sparse_and_mixed_format_entries();
        }

        #[test]
        fn lane_block_application_receipt_strict_retry_reissues_every_barrier() {
            super::lane_block_application_receipt_strict_retry_reissues_every_barrier();
        }

        #[test]
        fn initial_preindex_data_sync_failure_rolls_back_payload_before_retry() {
            super::initial_preindex_data_sync_failure_rolls_back_payload_before_retry();
        }

        #[cfg(any(target_os = "linux", target_os = "macos"))]
        #[test]
        fn bound_progress_recovery_handles_crash_phases_without_path_escape() {
            super::bound_progress_recovery_handles_crash_phases_without_path_escape();
        }

        #[cfg(unix)]
        #[test]
        fn progress_sidecar_mutation_rejects_symlinks_without_external_writes() {
            super::progress_sidecar_mutation_rejects_symlinks_without_external_writes();
        }

        #[cfg(unix)]
        #[test]
        fn progress_prepend_directory_failure_retries_without_corruption() {
            super::progress_prepend_directory_failure_retries_without_corruption();
        }

        #[test]
        fn predecessor_application_receipt_fails_closed_while_durability_barrier_fails() {
            super::predecessor_application_receipt_fails_closed_while_durability_barrier_fails();
        }

        #[test]
        fn strict_sidecar_retry_reissues_barriers_for_exact_existing_payload() {
            super::strict_sidecar_retry_reissues_barriers_for_exact_existing_payload();
        }

        #[test]
        fn unindexed_crash_suffix_is_repaired_before_retry_or_append() {
            super::unindexed_crash_suffix_is_repaired_before_retry_or_append();
        }
    }

    #[test]
    fn roster_sidecar_read_reissues_durability_barriers() {
        let failure_modes: [(&str, fn()); 2] = [
            ("index", fail_next_indexed_sidecar_index_sync_for_tests),
            ("directory", fail_next_indexed_sidecar_dir_sync_for_tests),
        ];
        for (label, inject_failure) in failure_modes {
            let kura = Kura::blank_kura_for_testing();
            let block_hash = store_dummy_blocks(&kura, 1)[0];
            let sidecar = RosterSidecar::new(1, block_hash, None, None, None);

            inject_failure();
            assert!(
                !kura.write_roster_metadata(&sidecar),
                "injected {label} failure must reject the strict roster write"
            );
            inject_failure();
            assert!(
                kura.read_roster_metadata(1).is_none(),
                "page-cache bytes from a failed write must not be exposed when the fresh {label} barrier also fails"
            );
            let recovered = kura
                .read_roster_metadata(1)
                .expect("retry should expose the roster after all barriers succeed");
            assert_eq!(recovered.height, sidecar.height);
            assert_eq!(recovered.block_hash, sidecar.block_hash);
            assert_eq!(recovered.commit_qc, sidecar.commit_qc);
            assert_eq!(recovered.validator_checkpoint, sidecar.validator_checkpoint);
            assert_eq!(recovered.stake_snapshot, sidecar.stake_snapshot);
        }
    }

    #[test]
    fn sidecar_append_rejects_zero_height() {
        let temp_dir = TempDir::new().unwrap();
        let data_path = temp_dir.path().join(PIPELINE_SIDECARS_DATA_FILE);
        let index_path = temp_dir.path().join(PIPELINE_SIDECARS_INDEX_FILE);
        let payload = norito::to_bytes(&DummySidecar { height: 0 }).expect("encode sidecar");

        assert!(
            !Kura::append_indexed_sidecar(
                &data_path,
                &index_path,
                0,
                &payload,
                "dummy sidecar",
                FsyncMode::Batched,
                None,
                SidecarIndexOrigin::HeightOne,
            ),
            "height 0 should be rejected"
        );
        assert!(!data_path.exists(), "data file should not be created");
        assert!(!index_path.exists(), "index file should not be created");
    }

    #[test]
    fn based_sidecar_index_handles_high_initial_height_and_sparse_recovery() {
        let temp_dir = TempDir::new().unwrap();
        let data_path = temp_dir.path().join(LANE_ARTIFACTS_DATA_FILE);
        let index_path = temp_dir.path().join(LANE_ARTIFACTS_INDEX_FILE);
        let high_height = MAX_INDEXED_SIDECAR_GAP_ENTRIES * 1_000 + 37;
        let payload = norito::to_bytes(&DummySidecar {
            height: high_height,
        })
        .expect("encode high sidecar");

        assert!(Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            high_height,
            &payload,
            "dummy lane sidecar",
            FsyncMode::Batched,
            None,
            SidecarIndexOrigin::FirstWrite,
        ));
        assert_eq!(
            fs::metadata(&index_path).expect("index metadata").len(),
            INDEXED_SIDECAR_BASE_HEADER_SIZE_U64 + PIPELINE_INDEX_ENTRY_SIZE_U64,
            "a high first height must not allocate entries from height one"
        );

        let mut index = std::fs::File::open(&index_path).expect("open based index");
        let index_len = index.metadata().expect("index metadata").len();
        let layout = SidecarIndexLayout::read_from(&mut index, index_len)
            .expect("decode based index layout");
        assert!(layout.is_based());
        assert_eq!(layout.base_height, high_height);
        assert_eq!(layout.height_range(), Some(high_height..=high_height));

        let temp_index_path = index_path.with_extension("index.tmp");
        fs::rename(&index_path, &temp_index_path).expect("stage index as recovery temp");
        let recovered = Kura::read_indexed_sidecar_from_paths(
            high_height,
            &data_path,
            &index_path,
            norito::decode_from_bytes::<DummySidecar>,
            "dummy lane sidecar",
        )
        .expect("recover based sidecar after reopening files");
        assert_eq!(recovered.height, high_height);
        assert!(
            index_path.exists(),
            "recovery should promote the temp index"
        );
        assert!(
            !temp_index_path.exists(),
            "promoted temp index should be gone"
        );

        let preceding_height = high_height - 1;
        let preceding_payload = norito::to_bytes(&DummySidecar {
            height: preceding_height,
        })
        .expect("encode preceding sidecar");
        assert!(Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            preceding_height,
            &preceding_payload,
            "dummy lane sidecar",
            FsyncMode::Batched,
            None,
            SidecarIndexOrigin::FirstWrite,
        ));
        for expected_height in [preceding_height, high_height] {
            let recovered = Kura::read_indexed_sidecar_from_paths(
                expected_height,
                &data_path,
                &index_path,
                norito::decode_from_bytes::<DummySidecar>,
                "dummy lane sidecar",
            )
            .expect("read sidecar after bounded backward prepend");
            assert_eq!(recovered.height, expected_height);
        }

        let sparse_height = high_height + 2;
        let sparse_payload = norito::to_bytes(&DummySidecar {
            height: sparse_height,
        })
        .expect("encode sparse sidecar");
        assert!(Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            sparse_height,
            &sparse_payload,
            "dummy lane sidecar",
            FsyncMode::Batched,
            None,
            SidecarIndexOrigin::FirstWrite,
        ));
        assert!(
            Kura::read_indexed_sidecar_from_paths(
                high_height + 1,
                &data_path,
                &index_path,
                norito::decode_from_bytes::<DummySidecar>,
                "dummy lane sidecar",
            )
            .is_none(),
            "the normal sparse gap should remain empty"
        );
        let recovered_sparse = Kura::read_indexed_sidecar_from_paths(
            sparse_height,
            &data_path,
            &index_path,
            norito::decode_from_bytes::<DummySidecar>,
            "dummy lane sidecar",
        )
        .expect("read sparse based sidecar");
        assert_eq!(recovered_sparse.height, sparse_height);
        assert_eq!(
            Kura::indexed_sidecar_height_range(&index_path, "dummy lane sidecar"),
            Some(preceding_height..=sparse_height)
        );
        assert_eq!(
            fs::metadata(&index_path).expect("index metadata").len(),
            INDEXED_SIDECAR_BASE_HEADER_SIZE_U64 + 4 * PIPELINE_INDEX_ENTRY_SIZE_U64
        );
    }

    #[test]
    fn based_sidecar_index_pruning_preserves_base_height() {
        let temp_dir = TempDir::new().unwrap();
        let data_path = temp_dir.path().join(LANE_ARTIFACTS_DATA_FILE);
        let index_path = temp_dir.path().join(LANE_ARTIFACTS_INDEX_FILE);
        let base_height = MAX_INDEXED_SIDECAR_GAP_ENTRIES * 2 + 11;
        let retention = NonZeroUsize::new(2).expect("non-zero retention");

        for height in base_height..=base_height + 2 {
            let payload = norito::to_bytes(&DummySidecar { height }).expect("encode based sidecar");
            assert!(Kura::append_indexed_sidecar(
                &data_path,
                &index_path,
                height,
                &payload,
                "dummy lane sidecar",
                FsyncMode::Batched,
                Some(retention),
                SidecarIndexOrigin::FirstWrite,
            ));
        }

        assert!(
            Kura::read_indexed_sidecar_from_paths(
                base_height,
                &data_path,
                &index_path,
                norito::decode_from_bytes::<DummySidecar>,
                "dummy lane sidecar",
            )
            .is_none(),
            "pruning should clear the oldest based entry"
        );
        for height in base_height + 1..=base_height + 2 {
            let sidecar = Kura::read_indexed_sidecar_from_paths(
                height,
                &data_path,
                &index_path,
                norito::decode_from_bytes::<DummySidecar>,
                "dummy lane sidecar",
            )
            .expect("retained based sidecar");
            assert_eq!(sidecar.height, height);
        }

        let mut index = std::fs::File::open(&index_path).expect("open pruned based index");
        let index_len = index.metadata().expect("index metadata").len();
        let layout = SidecarIndexLayout::read_from(&mut index, index_len)
            .expect("decode pruned based layout");
        assert!(layout.is_based());
        assert_eq!(layout.base_height, base_height);
        assert_eq!(layout.entry_count, 3);
        assert!(Kura::sidecar_index_sane_with_label(
            &index_path,
            fs::metadata(&data_path).expect("data metadata").len(),
            "dummy lane sidecar",
            "main",
        ));
    }

    #[test]
    fn legacy_sidecar_index_keeps_normal_sparse_gaps_readable() {
        let temp_dir = TempDir::new().unwrap();
        let data_path = temp_dir.path().join(PIPELINE_SIDECARS_DATA_FILE);
        let index_path = temp_dir.path().join(PIPELINE_SIDECARS_INDEX_FILE);
        for height in [1_u64, 4] {
            let payload =
                norito::to_bytes(&DummySidecar { height }).expect("encode legacy sidecar");
            assert!(Kura::append_indexed_sidecar(
                &data_path,
                &index_path,
                height,
                &payload,
                "dummy sidecar",
                FsyncMode::Batched,
                None,
                SidecarIndexOrigin::HeightOne,
            ));
        }

        assert_eq!(
            fs::metadata(&index_path).expect("index metadata").len(),
            4 * PIPELINE_INDEX_ENTRY_SIZE_U64,
            "legacy indexes must retain their dense on-disk layout"
        );
        for missing_height in [2_u64, 3] {
            assert!(
                Kura::read_indexed_sidecar_from_paths(
                    missing_height,
                    &data_path,
                    &index_path,
                    norito::decode_from_bytes::<DummySidecar>,
                    "dummy sidecar",
                )
                .is_none()
            );
        }
        let sidecar = Kura::read_indexed_sidecar_from_paths(
            4,
            &data_path,
            &index_path,
            norito::decode_from_bytes::<DummySidecar>,
            "dummy sidecar",
        )
        .expect("read legacy sparse sidecar");
        assert_eq!(sidecar.height, 4);
    }

    #[test]
    fn sidecar_append_rejects_oversized_gap_without_file_growth() {
        let temp_dir = TempDir::new().unwrap();
        let data_path = temp_dir.path().join(PIPELINE_SIDECARS_DATA_FILE);
        let index_path = temp_dir.path().join(PIPELINE_SIDECARS_INDEX_FILE);
        let first_payload =
            norito::to_bytes(&DummySidecar { height: 1 }).expect("encode first sidecar");
        assert!(Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            1,
            &first_payload,
            "dummy sidecar",
            FsyncMode::Batched,
            None,
            SidecarIndexOrigin::HeightOne,
        ));
        let data_before = fs::read(&data_path).expect("read sidecar data");
        let index_before = fs::read(&index_path).expect("read sidecar index");
        let hostile_height = 2 + MAX_INDEXED_SIDECAR_GAP_ENTRIES + 1;
        let hostile_payload = norito::to_bytes(&DummySidecar {
            height: hostile_height,
        })
        .expect("encode hostile sidecar");

        assert!(!Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            hostile_height,
            &hostile_payload,
            "dummy sidecar",
            FsyncMode::Batched,
            None,
            SidecarIndexOrigin::HeightOne,
        ));
        assert_eq!(
            fs::read(&data_path).expect("read sidecar data"),
            data_before
        );
        assert_eq!(
            fs::read(&index_path).expect("read sidecar index"),
            index_before
        );
    }

    #[test]
    fn based_sidecar_append_rejects_oversized_backward_gap_without_file_growth() {
        let temp_dir = TempDir::new().unwrap();
        let data_path = temp_dir.path().join(LANE_ARTIFACTS_DATA_FILE);
        let index_path = temp_dir.path().join(LANE_ARTIFACTS_INDEX_FILE);
        let first_height = MAX_INDEXED_SIDECAR_GAP_ENTRIES * 3 + 17;
        let first_payload = norito::to_bytes(&DummySidecar {
            height: first_height,
        })
        .expect("encode first based sidecar");
        assert!(Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            first_height,
            &first_payload,
            "dummy lane sidecar",
            FsyncMode::Batched,
            None,
            SidecarIndexOrigin::FirstWrite,
        ));
        let data_before = fs::read(&data_path).expect("read based sidecar data");
        let index_before = fs::read(&index_path).expect("read based sidecar index");
        let hostile_height = first_height - MAX_INDEXED_SIDECAR_GAP_ENTRIES - 1;
        let hostile_payload = norito::to_bytes(&DummySidecar {
            height: hostile_height,
        })
        .expect("encode hostile preceding sidecar");

        assert!(!Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            hostile_height,
            &hostile_payload,
            "dummy lane sidecar",
            FsyncMode::Batched,
            None,
            SidecarIndexOrigin::FirstWrite,
        ));
        assert_eq!(
            fs::read(&data_path).expect("read based sidecar data"),
            data_before
        );
        assert_eq!(
            fs::read(&index_path).expect("read based sidecar index"),
            index_before
        );
    }

    #[test]
    fn sidecar_append_rejects_max_height_before_creating_files() {
        let temp_dir = TempDir::new().unwrap();
        let data_path = temp_dir.path().join(LANE_ARTIFACTS_DATA_FILE);
        let index_path = temp_dir.path().join(LANE_ARTIFACTS_INDEX_FILE);
        let payload =
            norito::to_bytes(&DummySidecar { height: u64::MAX }).expect("encode max sidecar");

        assert!(!Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            u64::MAX,
            &payload,
            "dummy lane sidecar",
            FsyncMode::Batched,
            None,
            SidecarIndexOrigin::FirstWrite,
        ));
        assert!(!data_path.exists());
        assert!(!index_path.exists());
    }

