    #[test]
    fn one_logical_candidate_cannot_resurrect_at_another_bounded_address() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let wal = directory.path().join("logical-resurrection.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2)
                .expect("open two-slot fixture");
        let first = terminal_continuation_at_view(&context, 1, 1, 2, 1, 1);
        let mut continuations = BTreeMap::new();
        store
            .reserve_producer_continuation(&mut continuations, first.clone())
            .expect("reserve original logical candidate");
        let mut resurrected = first;
        resurrected.identity.lifecycle_slot = 2;
        resurrected.identity.admission_ordinal = 2;
        resurrected.identity.causal_lifecycle_key = Hash::new(b"forged second lifecycle");
        assert!(
            store
                .reserve_producer_continuation(&mut continuations, resurrected)
                .is_err(),
            "the same drained logical candidate cannot acquire a second address"
        );
        assert_eq!(continuations.len(), 1);
    }
    #[test]
    fn snapshot_rejects_nonregular_artifacts() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let wal = directory.path().join("directory.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1)
                .expect("derive snapshot path");
        fs::create_dir(store.path_for_test()).expect("place directory at snapshot path");
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1).is_err()
        );
    }
    #[cfg(unix)]
    #[test]
    fn snapshot_load_and_retire_never_follow_substituted_symlinks() {
        use std::os::unix::fs::symlink;
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let wal = directory.path().join("symlink.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1)
                .expect("open snapshot");
        store
            .persist(&BTreeMap::from([(key(&context, 0, 1), 0)]), false)
            .expect("persist target frame");
        let snapshot = store.path_for_test().to_path_buf();
        let hard_link = directory.path().join("hard-linked.snapshot");
        fs::hard_link(&snapshot, &hard_link).expect("create second link to snapshot");
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1).is_err(),
            "load must reject a multiply linked snapshot"
        );
        fs::remove_file(hard_link).expect("restore single-link fixture");
        let target = directory.path().join("target.snapshot");
        fs::rename(&snapshot, &target).expect("move direct frame to symlink target");
        let target_before = fs::read(&target).expect("read target before substitution");
        symlink(&target, &snapshot).expect("substitute snapshot symlink");
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1).is_err(),
            "load must reject a direct-path symlink"
        );
        assert!(
            store.retire().is_err(),
            "retirement must reject rather than follow a substituted symlink"
        );
        assert_eq!(
            fs::read(&target).expect("read target after rejected retirement"),
            target_before,
            "the symlink target remains untouched"
        );
        assert!(snapshot.is_symlink());
    }
    #[test]
    fn finalized_snapshot_retirement_leaves_successor_rollover_empty() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let successor = successor_context(&context);
        let wal = directory.path().join("00000000000000000007.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2)
                .expect("open finalized-height snapshot");
        let terminal =
            continuation_record(&context, 1, 1, 1, ProducerContinuationStatus::Terminal, &[]);
        let producer_continuations = BTreeMap::from([(terminal.identity.address(), terminal)]);
        let producer_candidate = producer_continuations
            .values()
            .next()
            .expect("terminal producer exists")
            .identity()
            .candidate();
        store
            .persist_with_producer_continuations(
                &BTreeMap::from([(producer_candidate, producer_candidate.source_view())]),
                &producer_continuations,
                false,
            )
            .expect("persist finalized-height owner");
        assert!(
            store
                .persist_with_producer_continuations(
                    &BTreeMap::new(),
                    &producer_continuations,
                    true,
                )
                .is_err(),
            "Decision reclamation rejects an orphan producer table"
        );
        store
            .persist_with_producer_continuations(&BTreeMap::new(), &BTreeMap::new(), true)
            .expect("atomically reclaim finalized-height service and producer owners");
        assert!(
            ServicedCandidateStore::open(&wal, successor.id(), successor.height, OWNER_A, 2,)
                .is_err(),
            "a predecessor snapshot cannot be transplanted into the successor context"
        );
        let snapshot_path = store.path_for_test().to_path_buf();
        store.retire().expect("retire finalized-height snapshot");
        assert!(!snapshot_path.exists());
        let successor_wal = directory.path().join("00000000000000000008.wal");
        let (_successor, restored) = ServicedCandidateStore::open(
            &successor_wal,
            successor.id(),
            successor.height,
            OWNER_A,
            2,
        )
        .expect("open independent successor path");
        assert!(restored.records.is_empty());
        assert!(restored.producer_continuations.is_empty());
        assert!(!restored.decision_reclaimed);
    }
