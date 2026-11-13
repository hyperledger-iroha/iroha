//! Durable boundary recovery, monotonicity, malformed records and private file custody.

use super::*;
use crate::{
    state::World,
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};

fn run(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .name("runtime-history-boundary".into())
        .stack_size(32 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap_or_else(|failure| std::panic::resume_unwind(failure));
}

fn chain() -> CertifiedTestChain {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    chain.commit(Vec::new());
    chain.commit(Vec::new());
    chain
}

fn boundary(kura: &Kura, height: u64) -> UpgradeCheckpointV1 {
    UpgradeCheckpointV1 {
        height,
        block_hash: kura
            .get_durable_block_hash(NonZeroUsize::new(height as usize).unwrap())
            .unwrap(),
    }
}

#[test]
fn highest_boundary_survives_reload_and_an_older_configured_anchor() {
    run(|| {
        let chain = chain();
        let kura = chain.kura();
        let anchor = boundary(kura, 1);
        let floor = boundary(kura, 2);
        kura.set_upgrade_checkpoint(&anchor).unwrap();
        kura.record_history_boundary(&floor).unwrap();
        kura.record_history_boundary(&anchor).unwrap();
        assert_eq!(kura.upgrade_checkpoint(), Some(&anchor));
        assert_eq!(kura.history_boundary(), Some(floor));
        *kura.runtime_history_boundary.lock() = None;
        kura.load_runtime_history_boundary().unwrap();
        assert_eq!(kura.history_boundary(), Some(floor));

        let bytes = std::fs::read(kura.store_root.join(FILE_NAME)).unwrap();
        let record: RuntimeHistoryBoundaryV1 = norito::decode_canonical(&bytes).unwrap();
        assert_eq!(record.version, 1);
        assert_eq!(record.checkpoint(), floor);
        assert_eq!(norito::encode_canonical(&record).unwrap(), bytes);
        assert_eq!(record.genesis, anchor.block_hash);
    });
}

#[test]
fn certified_publication_persists_a_ratified_anchor_without_node_configuration() {
    run(|| {
        use iroha_data_model::{
            isi::runtime_boundary::RatifyRuntimeAdoptionAnchorV1,
            runtime::{
                checkpoint::RuntimeAdoptionAnchorV1,
                lifecycle::{AdoptionRecordV1, committee_digest},
            },
        };

        let keys = crate::sumeragi::test_chain::fixture_keys();
        let world = World::new();
        crate::runtime::activate_additions_for_testing(
            &world,
            crate::runtime::catalog::additions::RUNTIME_LIFECYCLE.module,
        )
        .unwrap();
        let mut config = TestChainConfig::new(world, 1_000);
        config.validator_keys = Some(keys.clone());
        let submitter = config.genesis_key.clone();
        let mut chain = CertifiedTestChain::start(config).unwrap();
        let checkpoint = boundary(chain.kura(), 1);
        let record = AdoptionRecordV1 {
            network: chain.network_id(),
            height: checkpoint.height,
            block_hash: *<HashOf<BlockHeader> as AsRef<[u8; 32]>>::as_ref(&checkpoint.block_hash),
            // Validator own-store checks have dedicated runtime checkpoint tests;
            // this test exercises all-member admission and actual publication.
            predecessor_world_root: [2; 32],
            imported_state_root: [3; 32],
            committee: committee_digest(
                &keys
                    .iter()
                    .map(|key| key.public_key().clone())
                    .collect::<Vec<_>>(),
            )
            .unwrap(),
            selection: crate::runtime::release_catalog().unwrap().baseline_id,
            authorization: [0; 32],
        };
        let anchor = RuntimeAdoptionAnchorV1 {
            attestations: keys
                .iter()
                .map(|key| {
                    RuntimeAdoptionAnchorV1::statement(&record, key.public_key().clone())
                        .unwrap()
                        .sign(key.private_key())
                        .unwrap()
                })
                .collect(),
            record,
        };
        let transaction = chain.sign(
            &submitter,
            [RatifyRuntimeAdoptionAnchorV1 { anchor }.into()],
            1_001,
        );
        assert_eq!(chain.commit(vec![transaction]), [true]);
        assert_eq!(chain.kura().upgrade_checkpoint(), None);
        assert_eq!(chain.kura().history_boundary(), Some(checkpoint));
        *chain.kura().runtime_history_boundary.lock() = None;
        chain.kura().load_runtime_history_boundary().unwrap();
        assert_eq!(chain.kura().history_boundary(), Some(checkpoint));
    });
}

#[test]
fn preparation_and_failed_persistence_do_not_advance_memory() {
    run(|| {
        let chain = chain();
        let kura = chain.kura();
        let floor = boundary(kura, 2);
        let prepared = kura
            .prepare_runtime_history_boundary(&floor)
            .unwrap()
            .unwrap();
        assert_eq!(kura.history_boundary(), None);
        assert!(!kura.store_root.join(FILE_NAME).exists());
        std::fs::create_dir(kura.store_root.join(FILE_NAME)).unwrap();
        assert!(prepared.persist(kura).is_err());
        assert_eq!(kura.history_boundary(), None);
        std::fs::remove_dir(kura.store_root.join(FILE_NAME)).unwrap();
        kura.record_history_boundary(&floor).unwrap();
        assert_eq!(kura.history_boundary(), Some(floor));
    });
}

#[test]
fn conflicting_or_absent_boundary_blocks_are_refused() {
    run(|| {
        let chain = chain();
        let kura = chain.kura();
        let floor = boundary(kura, 2);
        let mut wrong = floor;
        wrong.block_hash = boundary(kura, 1).block_hash;
        assert!(kura.record_history_boundary(&wrong).is_err());
        assert_eq!(kura.history_boundary(), None);
        kura.record_history_boundary(&floor).unwrap();
        assert!(kura.record_history_boundary(&wrong).is_err());
        wrong = floor;
        wrong.height = 4;
        assert!(kura.record_history_boundary(&wrong).is_err());
        assert_eq!(kura.history_boundary(), Some(floor));
    });
}

#[test]
fn corrupt_foreign_and_unrecognized_durable_records_fail_closed() {
    run(|| {
        let chain = chain();
        let kura = chain.kura();
        kura.record_history_boundary(&boundary(kura, 2)).unwrap();
        let path = kura.store_root.join(FILE_NAME);
        let original = std::fs::read(&path).unwrap();
        let record: RuntimeHistoryBoundaryV1 = norito::decode_canonical(&original).unwrap();
        for candidate in [
            RuntimeHistoryBoundaryV1 {
                version: 2,
                ..record
            },
            RuntimeHistoryBoundaryV1 {
                height: 0,
                ..record
            },
            RuntimeHistoryBoundaryV1 {
                height: 4,
                ..record
            },
            RuntimeHistoryBoundaryV1 {
                genesis: record.block_hash,
                ..record
            },
            RuntimeHistoryBoundaryV1 {
                block_hash: record.genesis,
                ..record
            },
        ] {
            std::fs::write(&path, norito::encode_canonical(&candidate).unwrap()).unwrap();
            *kura.runtime_history_boundary.lock() = None;
            assert!(kura.load_runtime_history_boundary().is_err());
            assert_eq!(kura.history_boundary(), None);
        }
        for malformed in [
            original[..original.len() - 1].to_vec(),
            vec![0; MAX_BYTES + 1],
        ] {
            std::fs::write(&path, malformed).unwrap();
            assert!(kura.load_runtime_history_boundary().is_err());
        }
        std::fs::write(&path, original).unwrap();
        kura.load_runtime_history_boundary().unwrap();
        assert_eq!(kura.history_boundary(), Some(boundary(kura, 2)));
    });
}

#[cfg(unix)]
#[test]
fn durable_boundary_symlinks_are_never_followed() {
    run(|| {
        let chain = chain();
        let kura = chain.kura();
        let floor = boundary(kura, 2);
        kura.record_history_boundary(&floor).unwrap();
        let path = kura.store_root.join(FILE_NAME);
        let original = kura.store_root.join("retained-original-boundary.norito");
        std::fs::rename(&path, &original).unwrap();
        std::os::unix::fs::symlink(&original, &path).unwrap();
        *kura.runtime_history_boundary.lock() = None;
        assert!(kura.load_runtime_history_boundary().is_err());
        assert!(kura.record_history_boundary(&floor).is_err());
        assert_eq!(kura.history_boundary(), None);
    });
}

#[cfg(unix)]
#[test]
fn replacement_store_directories_cannot_receive_or_supply_boundary_authority() {
    run(|| {
        let chain = chain();
        let kura = chain.kura();
        let floor = boundary(kura, 2);
        let prepared = kura
            .prepare_runtime_history_boundary(&floor)
            .unwrap()
            .unwrap();
        let original = kura.store_root.with_extension("retained-original");
        std::fs::rename(&kura.store_root, &original).unwrap();
        std::fs::create_dir(&kura.store_root).unwrap();
        let read = kura.load_runtime_history_boundary();
        let write = prepared.persist(kura);
        let replacement_written = kura.store_root.join(FILE_NAME).exists();
        // Restore the test-owned original before asserting or releasing the chain.
        std::fs::remove_dir_all(&kura.store_root).unwrap();
        std::fs::rename(&original, &kura.store_root).unwrap();
        assert!(
            read.is_err(),
            "absence on a substitute is not authoritative"
        );
        assert!(
            write.is_err(),
            "a substitute cannot retain the original store's floor"
        );
        assert!(!replacement_written);
        assert_eq!(kura.history_boundary(), None);
        kura.record_history_boundary(&floor).unwrap();
        assert_eq!(kura.history_boundary(), Some(floor));
    });
}
