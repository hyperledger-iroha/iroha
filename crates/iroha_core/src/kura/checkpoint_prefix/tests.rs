//! Genuine portable bootstrap, restart, missing-body and staged-publication controls.

use super::*;
use crate::sumeragi::checkpoint_prefix::tests::{Fixture, fixture};

fn run(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap_or_else(|failure| std::panic::resume_unwind(failure));
}

fn config(path: &Path) -> Config {
    use iroha_config::parameters::defaults::kura as defaults;
    Config {
        init_mode: InitMode::Strict,
        store_dir: WithOrigin::inline(path.to_path_buf()),
        max_disk_usage_bytes: defaults::MAX_DISK_USAGE_BYTES,
        blocks_in_memory: defaults::BLOCKS_IN_MEMORY,
        debug_output_new_blocks: false,
        fsync_mode: FsyncMode::Always,
        fsync_interval: defaults::FSYNC_INTERVAL,
        native_context_archive_max_bytes: defaults::NATIVE_CONTEXT_ARCHIVE_MAX_BYTES,
        history_checkpoint_cache_capacity: defaults::HISTORY_CHECKPOINT_CACHE_CAPACITY,
        block_hash_history_bytes: defaults::BLOCK_HASH_HISTORY_BYTES,
        transaction_history_bytes: defaults::TRANSACTION_HISTORY_BYTES,
        membership_storage: defaults::MEMBERSHIP_STORAGE_POLICY,
        fastpq_artifacts: defaults::FASTPQ_ARTIFACT_POLICY,
    }
}

/// Original full chain plus freshly imported sparse store, retained for consumer controls.
pub(crate) fn imported() -> (tempfile::TempDir, Config, Arc<Kura>, Fixture) {
    let fixture = fixture();
    let directory = tempfile::tempdir().unwrap();
    let config = config(directory.path());
    let proof = Kura::prepare_portable_checkpoint_store(
        &config,
        fixture
            .chain
            .kura()
            .configured_lane_catalog_baseline()
            .unwrap()
            .unwrap(),
        fixture.chain.state().chain_id_ref(),
        fixture.chain.network_id(),
        &fixture.genesis,
        &fixture.hashes,
        fixture.chain.committed(3).block(),
        &mut |height| Ok(fixture.frames[(height - 2) as usize].clone()),
    )
    .unwrap();
    assert_eq!(proof.checkpoint().header().height().get(), 2);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &LaneConfig::default()).unwrap();
    (directory, config, kura, fixture)
}

#[test]
fn portable_bootstrap_reopens_exact_holes_and_never_creates_native_frames() {
    run(|| {
        let (_directory, mut config, kura, mut fixture) = imported();
        assert_eq!(
            kura.configured_lane_catalog_baseline().unwrap(),
            fixture
                .chain
                .kura()
                .configured_lane_catalog_baseline()
                .unwrap(),
            "portable bootstrap preserves the original configured catalog"
        );
        assert_eq!(kura.checkpoint_prefix_height(), Some(2));
        assert!(
            kura.native_frame_read(2, fixture.hashes[1])
                .unwrap()
                .is_none()
        );
        assert!(
            kura.canonical_block_wire_bytes_for_testing(NonZeroUsize::new(2).unwrap())
                .is_err()
        );
        assert!(!kura.transaction_entrypoint_index.lock().complete);
        assert!(
            kura.transaction_entrypoint_index
                .lock()
                .incomplete_heights
                .contains(&NonZeroUsize::new(2).unwrap())
        );
        for height in [1, 3] {
            assert_eq!(
                kura.canonical_block_wire_bytes_for_testing(NonZeroUsize::new(height).unwrap())
                    .unwrap(),
                fixture
                    .chain
                    .committed(height as u64)
                    .block()
                    .encode_wire()
                    .unwrap()
            );
        }
        let mut observed = Vec::new();
        let verified = kura
            .walk_checkpoint_prefix(
                fixture.chain.state().chain_id_ref(),
                fixture.chain.network_id(),
                &fixture.hashes,
                &mut |block| {
                    observed.push(block.header().height().get());
                    Ok(())
                },
            )
            .unwrap()
            .unwrap();
        assert_eq!(observed, [1, 2]);
        assert_eq!(verified.checkpoint().header().hash(), fixture.hashes[1]);
        assert_eq!(
            kura.checkpoint_prefix_frame(2).unwrap(),
            Some(fixture.frames[0].clone())
        );
        assert!(kura.checkpoint_prefix_frame(4).unwrap().is_none());
        fixture.chain.commit(Vec::new());
        kura.persist_block_immediate_for_bench(fixture.chain.committed(4).block())
            .unwrap();
        assert_eq!(
            kura.canonical_block_wire_bytes_for_testing(NonZeroUsize::new(4).unwrap())
                .unwrap(),
            fixture.chain.committed(4).block().encode_wire().unwrap()
        );
        drop(kura);
        config.init_mode = InitMode::Fast;
        let (reopened, _) =
            Kura::open_test_kura_with_configured_lane_config(&config, &LaneConfig::default())
                .unwrap();
        assert_eq!(reopened.checkpoint_prefix_height(), Some(2));
        assert!(
            reopened
                .native_frame_read(2, fixture.hashes[1])
                .unwrap()
                .is_none()
        );
    });
}

#[test]
fn portable_bootstrap_manifest_is_the_only_owner_of_unbound_canonical_history() {
    run(|| {
        let (directory, config, kura, _) = imported();
        drop(kura);
        assert!(is_published(directory.path()).unwrap());
        let open =
            || Kura::open_test_kura_with_configured_lane_config(&config, &LaneConfig::default());
        let aside = tempfile::tempdir().unwrap();
        // Without its manifest the same store is an unbound catalog over foreign bodies.
        let manifest = directory.path().join(DIRECTORY).join(MANIFEST);
        std::fs::rename(&manifest, aside.path().join(MANIFEST)).unwrap();
        assert!(!is_published(directory.path()).unwrap());
        let Err(Error::IO(error, _)) = open() else {
            panic!("unbound canonical bodies opened without their portable manifest")
        };
        assert_eq!(error.kind(), ErrorKind::InvalidData);
        assert!(
            error
                .to_string()
                .contains("unbound configured primary block store is not empty"),
            "{error}"
        );
        std::fs::rename(aside.path().join(MANIFEST), &manifest).unwrap();
        // A pending fresh-store intent is not a published prefix.
        let intent = directory.path().join(DIRECTORY).join(INTENT);
        std::fs::write(&intent, [0_u8; Hash::LENGTH]).unwrap();
        assert!(!is_published(directory.path()).unwrap());
        std::fs::remove_file(intent).unwrap();
        let (reopened, _) = open().unwrap();
        assert_eq!(reopened.checkpoint_prefix_height(), Some(2));
        drop(reopened);
        // The published manifest cannot silently recreate a missing canonical chain.
        let canonical = Kura::canonical_storage_path(directory.path());
        std::fs::rename(&canonical, aside.path().join("canonical")).unwrap();
        let Err(Error::IO(error, _)) = open() else {
            panic!("a published portable prefix recreated its missing canonical chain")
        };
        assert_eq!(error.kind(), ErrorKind::NotFound);
        assert!(
            error
                .to_string()
                .contains("portable checkpoint canonical storage is missing"),
            "{error}"
        );
        assert!(!canonical.exists());
    });
}

#[test]
fn portable_bootstrap_rejects_a_different_configured_catalog_without_rewriting_the_store() {
    run(|| {
        let (directory, config, kura, _) = imported();
        drop(kura);
        let journal_path = directory.path().join("lane_geometry_journal.norito");
        let original = std::fs::read(&journal_path).unwrap();
        let mut lanes = LaneCatalog::default().lanes().to_vec();
        lanes[0].description = Some("different configured catalog".into());
        let catalog = LaneCatalog::new(std::num::NonZeroU32::new(1).unwrap(), lanes).unwrap();
        let lane_config = LaneConfig::from_catalog(&catalog);
        assert!(Kura::new_with_configured_lane_catalog(&config, &lane_config, &catalog).is_err());
        assert_eq!(std::fs::read(journal_path).unwrap(), original);
    });
}

#[test]
fn portable_bootstrap_corrupt_proof_rejects_strict_and_fast_without_journal_mutation() {
    run(|| {
        let (directory, mut config, kura, _) = imported();
        drop(kura);
        let canonical = Kura::canonical_storage_path(directory.path());
        let before: Vec<_> = [
            DATA_FILE_NAME,
            INDEX_FILE_NAME,
            HASHES_FILE_NAME,
            COUNT_FILE_NAME,
        ]
        .iter()
        .map(|name| std::fs::read(canonical.join(name)).unwrap())
        .collect();
        let path = directory.path().join(DIRECTORY).join(frame_name(2));
        let mut bytes = std::fs::read(&path).unwrap();
        *bytes.last_mut().unwrap() ^= 1;
        std::fs::write(path, bytes).unwrap();
        for mode in [InitMode::Strict, InitMode::Fast] {
            config.init_mode = mode;
            assert!(
                Kura::open_test_kura_with_configured_lane_config(&config, &LaneConfig::default())
                    .is_err()
            );
            for (name, original) in [
                DATA_FILE_NAME,
                INDEX_FILE_NAME,
                HASHES_FILE_NAME,
                COUNT_FILE_NAME,
            ]
            .iter()
            .zip(&before)
            {
                assert_eq!(&std::fs::read(canonical.join(name)).unwrap(), original);
            }
        }
    });
}

#[test]
fn portable_bootstrap_requires_exact_missing_marker_and_original_journal() {
    run(|| {
        let (directory, config, kura, fixture) = imported();
        drop(kura);
        let canonical = Kura::canonical_storage_path(directory.path());
        let original = std::fs::read(canonical.join(INDEX_FILE_NAME)).unwrap();
        for marker in [
            BlockIndex {
                start: EVICTED_BLOCK_START,
                length: 0,
            },
            BlockIndex {
                start: CHECKPOINT_OMITTED_BLOCK_START,
                length: 1,
            },
            BlockIndex {
                start: 0,
                length: 0,
            },
        ] {
            let mut changed = original.clone();
            changed[BlockIndex::SIZE_USIZE..2 * BlockIndex::SIZE_USIZE]
                .copy_from_slice(&marker.encode());
            std::fs::write(canonical.join(INDEX_FILE_NAME), changed).unwrap();
            assert!(
                Kura::open_test_kura_with_configured_lane_config(&config, &LaneConfig::default())
                    .is_err()
            );
        }
        std::fs::write(canonical.join(INDEX_FILE_NAME), original).unwrap();
        let mut hashes = std::fs::read(canonical.join(HASHES_FILE_NAME)).unwrap();
        hashes[Hash::LENGTH..2 * Hash::LENGTH].copy_from_slice(fixture.hashes[0].as_ref());
        std::fs::write(canonical.join(HASHES_FILE_NAME), hashes).unwrap();
        assert!(
            Kura::open_test_kura_with_configured_lane_config(&config, &LaneConfig::default())
                .is_err()
        );
    });
}

#[test]
fn portable_bootstrap_recovers_exact_partial_promotion_with_marker_last() {
    run(|| {
        let (directory, config, kura, fixture) = imported();
        drop(kura);
        let canonical = Kura::canonical_storage_path(directory.path());
        let proof_directory =
            iroha_fs::OwnerDirectory::open(directory.path().join(DIRECTORY)).unwrap();
        let manifest = proof_directory
            .read_regular(MANIFEST, MAX_MANIFEST)
            .unwrap();
        proof_directory
            .write_private_atomic(
                INTENT,
                Hash::new(manifest.as_slice()).as_ref(),
                iroha_fs::PublishMode::CreateNew,
            )
            .unwrap();
        std::fs::write(canonical.join(INDEX_FILE_NAME), []).unwrap();
        std::fs::write(canonical.join(HASHES_FILE_NAME), []).unwrap();
        std::fs::write(
            canonical.join(COUNT_FILE_NAME),
            norito::encode_canonical(&BlockStoreCommitMarker::new(0, None)).unwrap(),
        )
        .unwrap();
        let (reopened, _) =
            Kura::open_test_kura_with_configured_lane_config(&config, &LaneConfig::default())
                .unwrap();
        assert_eq!(reopened.checkpoint_prefix_height(), Some(2));
        assert_eq!(
            reopened.get_durable_block_hash(NonZeroUsize::new(3).unwrap()),
            Some(fixture.chain.committed(3).block_hash())
        );
        assert!(!directory.path().join(DIRECTORY).join(INTENT).exists());
        assert!(
            reopened
                .native_frame_read(2, fixture.hashes[1])
                .unwrap()
                .is_none()
        );
    });
}

#[test]
fn portable_bootstrap_original_refusal_and_quota_do_not_publish_an_intent() {
    run(|| {
        let fixture = fixture();
        let directory = tempfile::tempdir().unwrap();
        let mut config = config(directory.path());
        config.max_disk_usage_bytes = iroha_config::base::util::Bytes(1);
        assert!(
            Kura::prepare_portable_checkpoint_store(
                &config,
                fixture
                    .chain
                    .kura()
                    .configured_lane_catalog_baseline()
                    .unwrap()
                    .unwrap(),
                fixture.chain.state().chain_id_ref(),
                fixture.chain.network_id(),
                &fixture.genesis,
                &fixture.hashes,
                fixture.chain.committed(3).block(),
                &mut |height| Ok(fixture.frames[(height - 2) as usize].clone())
            )
            .is_err()
        );
        assert!(!directory.path().join(DIRECTORY).join(INTENT).exists());
        assert!(
            !Kura::canonical_storage_path(directory.path())
                .join(COUNT_FILE_NAME)
                .exists()
        );
        config.max_disk_usage_bytes =
            iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES;
        // The fixture's own journal and body reads are not the refusal under test: take the
        // configured catalog and the successor before the zero-allocation scope so only the
        // bootstrap runs inside it.
        let configured_catalog = fixture
            .chain
            .kura()
            .configured_lane_catalog_baseline()
            .unwrap()
            .unwrap();
        let successor = fixture.chain.committed(3);
        let refusal = norito::core::with_decode_limits_scope(
            norito::core::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX),
            || {
                Kura::prepare_portable_checkpoint_store(
                    &config,
                    configured_catalog,
                    fixture.chain.state().chain_id_ref(),
                    fixture.chain.network_id(),
                    &fixture.genesis,
                    &fixture.hashes,
                    successor.block(),
                    &mut |height| Ok(fixture.frames[(height - 2) as usize].clone()),
                )
            },
        )
        .unwrap_err();
        assert!(matches!(refusal, ExecutionAttemptError::Deferred(_)));
        assert!(!directory.path().join(DIRECTORY).join(INTENT).exists());
    });
}
