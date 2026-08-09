#[test]
fn restart_publishes_complete_carrier_temp_for_durable_block() {
    let dir = TempDir::new().expect("tempdir");
    let config = kura_config_for_dir(&dir, BLOCKS_IN_MEMORY);
    let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("initialize Kura");
    let (carrier, entry) = store_genesis_and_build_merge_carrier(&kura, 1);
    let carrier_hash = carrier.hash();
    let entry_hash = entry.canonical_hash();
    kura.store_block_with_merge_entry(carrier, &entry)
        .expect("store merge carrier");
    let _ = persist_v2_finality_chain_through(&kura, nonzero!(2_usize));
    let path = kura.merge_carrier_path(2);
    let temp_path = path.with_extension("norito.tmp");
    fs::rename(&path, &temp_path).expect("simulate crash before carrier rename");
    drop(kura);

    let (reopened, _) =
        Kura::new(&config, &RuntimeLaneConfig::default()).expect("recover carrier temp");
    assert!(path.exists());
    assert!(!temp_path.exists());
    assert_eq!(
        reopened
            .merge_carrier_for_entry(entry_hash)
            .expect("lookup recovered carrier")
            .map(|record| record.block_hash),
        Some(carrier_hash)
    );
}

#[test]
fn restart_rolls_back_uncommitted_merge_publication_suffixes() {
    for boundary in ["log", "carrier", "carrier_temp"] {
        let dir = TempDir::new().expect("tempdir");
        let config = kura_config_for_dir(&dir, BLOCKS_IN_MEMORY);
        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("initialize Kura");
        let (carrier, entry) = store_genesis_and_build_merge_carrier(&kura, 1);
        let record = MergeLedgerCarrierRecord::new(&entry, &carrier);
        kura.append_merge_entry(&entry)
            .expect("stage merge log frame");
        if boundary == "carrier" {
            let _guard = kura.merge_carrier_lock.lock();
            kura.write_merge_carrier_record_unlocked(record)
                .expect("stage published carrier");
        } else if boundary == "carrier_temp" {
            let directory = kura.merge_carrier_dir();
            fs::create_dir_all(&directory).expect("create carrier directory");
            let temp_path = kura.merge_carrier_path(2).with_extension("norito.tmp");
            fs::write(
                &temp_path,
                norito::to_bytes(&record).expect("encode carrier record"),
            )
            .expect("stage complete carrier temporary");
        }
        drop(kura);

        let (reopened, _) = Kura::new(&config, &RuntimeLaneConfig::default())
            .unwrap_or_else(|err| panic!("recover {boundary} boundary: {err}"));
        assert!(
            reopened
                .merge_ledger_all_entries()
                .expect("read log")
                .is_empty(),
            "{boundary} crash suffix must not become globally committed"
        );
        assert!(
            reopened
                .merge_carrier_records()
                .expect("read carriers")
                .is_empty(),
            "{boundary} crash carrier must be removed"
        );
        assert_eq!(reopened.blocks_count(), 1, "genesis remains durable");
    }
}

#[test]
fn restart_rejects_torn_or_noncanonical_carrier_temporary() {
    for bytes in [vec![0xAA, 0xBB], vec![0; MERGE_CARRIER_MAX_BYTES + 1]] {
        let dir = TempDir::new().expect("tempdir");
        let config = kura_config_for_dir(&dir, BLOCKS_IN_MEMORY);
        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("initialize Kura");
        let (_carrier, entry) = store_genesis_and_build_merge_carrier(&kura, 1);
        kura.append_merge_entry(&entry)
            .expect("stage merge log frame");
        let directory = kura.merge_carrier_dir();
        fs::create_dir_all(&directory).expect("create carrier directory");
        fs::write(
            kura.merge_carrier_path(2).with_extension("norito.tmp"),
            bytes,
        )
        .expect("write malformed carrier temporary");
        drop(kura);

        assert!(
            Kura::new(&config, &RuntimeLaneConfig::default()).is_err(),
            "malformed carrier temporary must fail closed"
        );
    }
}

#[test]
fn store_block_with_merge_entry_rejects_carrier_round_drift_without_mutation() {
    #[derive(Clone, Copy)]
    enum Drift {
        Height,
        Parent,
        View,
    }

    for (drift, label) in [
        (Drift::Height, "height"),
        (Drift::Parent, "parent"),
        (Drift::View, "view"),
    ] {
        let kura = Kura::blank_kura_for_testing();
        let mut blocks = DummyBlocks::new();
        let parent = blocks.next();
        let mut entry = sample_merge_entry(1);
        let carrier = next_merge_carrier(&mut blocks, &mut entry);
        kura.store_block(parent).expect("store carrier parent");

        match drift {
            Drift::Height => {
                entry.merge_qc.carrier_height = entry.merge_qc.carrier_height.saturating_add(1);
            }
            Drift::Parent => {
                entry.merge_qc.carrier_parent_hash =
                    HashOf::from_untyped_unchecked(Hash::new(b"wrong merge carrier parent"));
            }
            Drift::View => {
                entry.merge_qc.view = entry.merge_qc.view.saturating_add(1);
            }
        }

        let mut carrier = carrier.as_ref().clone();
        let execution_context = carrier
            .execution_context()
            .cloned()
            .unwrap_or_else(|| BlockExecutionContextBundle::new(Vec::new()))
            .with_merge_entry(CertifiedMergeLedgerReference::new(&entry));
        carrier.set_execution_context(Some(execution_context));
        let entry_hash = entry.canonical_hash();
        let error = kura
            .store_block_with_merge_entry(Arc::new(carrier), &entry)
            .expect_err("carrier-round drift must fail closed");

        assert!(
            matches!(
                &error,
                Error::MergeReferenceMismatch(message)
                    if message.contains("height, parent, or view")
            ),
            "{label} drift must report the exact carrier-round mismatch: {error}"
        );
        assert_eq!(
            kura.blocks_count(),
            1,
            "{label} drift must not append the carrier block"
        );
        assert!(
            kura.merge_ledger_snapshot().is_empty(),
            "{label} drift must not append the merge log"
        );
        assert_eq!(
            kura.merge_carrier_for_entry(entry_hash)
                .expect("carrier index remains readable"),
            None,
            "{label} drift must not publish a sparse carrier record"
        );
    }
}

#[test]
fn store_block_with_merge_entry_preflights_sparse_carrier_conflicts_before_block_commit() {
    let kura = Kura::blank_kura_for_testing();
    let mut blocks = DummyBlocks::new();
    let parent = blocks.next();
    let mut entry = sample_merge_entry(1);
    let block = next_merge_carrier(&mut blocks, &mut entry);
    let conflicting = MergeLedgerCarrierRecord {
        version: 1,
        entry_hash: sample_merge_entry(2).canonical_hash(),
        epoch_id: 2,
        block_height: block.header().height().get(),
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"conflicting carrier block")),
    };

    kura.store_block(parent).expect("store carrier parent");
    {
        let _guard = kura.merge_carrier_lock.lock();
        kura.write_merge_carrier_record_unlocked(conflicting)
            .expect("seed conflicting sparse carrier record");
    }

    let error = kura
        .store_block_with_merge_entry(block, &entry)
        .expect_err("sparse carrier conflict must fail before the block commit point");
    assert!(matches!(&error, Error::MergeCarrierConflict(_)));
    assert_eq!(kura.blocks_count(), 1);
    assert_eq!(kura.exact_durable_blocks_count().unwrap(), 1);
    assert!(kura.merge_ledger_snapshot().is_empty());
    assert!(
        kura.pending_certified_merge_entries()
            .expect("read pending sidecar store")
            .is_empty(),
        "preflight failure must not stage a new recovery sidecar"
    );
}
