    #[test]
    fn signing_guard_rejects_truncated_final_record() {
        let temp = tempfile::tempdir().expect("temp dir");
        let context = MergeSigningContextV1 {
            epoch_id: 5,
            view: 3,
            carrier_height: 11,
            parent_hash: HashOf::from_untyped_unchecked(Hash::new(b"parent-10")),
            validator_set_hash: HashOf::new(&vec![peer(b"validator")]),
        };
        let candidate = signing_candidate(&context, b"truncated final record");
        let guard = MergeSigningGuard::open(temp.path()).expect("open guard");
        guard
            .authorize(context.clone(), Hash::new(b"truncated"), &candidate)
            .expect("authorize candidate before truncation");
        let path = guard.record_path(&context);
        let mut bytes = fs::read(&path).expect("read exact final record");
        bytes.truncate(bytes.len() / 2);
        fs::write(path, bytes).expect("install truncated final record");
        drop(guard);
        assert!(matches!(
            MergeSigningGuard::open(temp.path()),
            Err(MergeSidecarError::SigningGuard(_))
        ));
    }

    #[test]
    fn signing_guard_high_water_allows_more_than_record_cap_committed_epochs() {
        let temp = tempfile::tempdir().expect("temp dir");
        let roster_hash = HashOf::new(&vec![peer(b"validator")]);
        let mut guard = MergeSigningGuard::open(temp.path()).expect("open guard");
        for epoch_id in 1..=(MAX_SIGNING_GUARD_RECORDS as u64 + 64) {
            let context = MergeSigningContextV1 {
                epoch_id,
                view: 0,
                carrier_height: epoch_id + 1,
                parent_hash: HashOf::from_untyped_unchecked(Hash::new(epoch_id.to_le_bytes())),
                validator_set_hash: roster_hash,
            };
            let candidate = signing_candidate(&context, &epoch_id.to_le_bytes());
            guard
                .authorize(context, Hash::new(epoch_id.to_le_bytes()), &candidate)
                .expect("authorize next epoch");
            guard
                .advance_committed_epoch(epoch_id)
                .expect("advance committed high-water");
        }
        let restarted = MergeSigningGuard::open_with_committed_epoch(
            temp.path(),
            MAX_SIGNING_GUARD_RECORDS as u64 + 64,
        )
        .expect("restart beyond record cap");
        assert_eq!(
            restarted.committed_epoch,
            MAX_SIGNING_GUARD_RECORDS as u64 + 64
        );
    }
    #[test]
    fn signing_guard_height_high_water_handles_many_ordinary_carrier_misses() {
        let temp = tempfile::tempdir().expect("temp dir");
        let roster_hash = HashOf::new(&vec![peer(b"validator")]);
        let mut guard = MergeSigningGuard::open(temp.path()).expect("open guard");
        let rounds = MAX_SIGNING_GUARD_RECORDS as u64 + 64;
        for carrier_height in 1..=rounds {
            let context = MergeSigningContextV1 {
                epoch_id: 1,
                view: 0,
                carrier_height,
                parent_hash: HashOf::from_untyped_unchecked(Hash::new(
                    carrier_height.saturating_sub(1).to_le_bytes(),
                )),
                validator_set_hash: roster_hash,
            };
            let candidate = signing_candidate(&context, &carrier_height.to_le_bytes());
            guard
                .authorize(context, Hash::new(carrier_height.to_le_bytes()), &candidate)
                .expect("authorize exact uncommitted carrier round");
            guard
                .advance_committed_frontier(0, carrier_height)
                .expect("ordinary global block finalizes carrier height");
        }
        drop(guard);
        let restarted = MergeSigningGuard::open_with_committed_frontier(
            temp.path(),
            0,
            rounds,
            MergeSigningGuardLimits::defaults(),
        )
        .expect("restart after many ordinary blocks");
        assert_eq!(restarted.committed_carrier_height, rounds);

        let later = MergeSigningContextV1 {
            epoch_id: 1,
            view: 0,
            carrier_height: rounds + 1,
            parent_hash: HashOf::from_untyped_unchecked(Hash::new(rounds.to_le_bytes())),
            validator_set_hash: roster_hash,
        };
        let later_candidate = signing_candidate(&later, b"later candidate");
        restarted
            .authorize(later, Hash::new(b"later candidate"), &later_candidate)
            .expect("same epoch/view remains signable at a new exact carrier");
    }

    #[test]
    fn signing_guard_reconciles_partial_temps_without_weakening_final_records() {
        let temp = tempfile::tempdir().expect("temp dir");
        let context = MergeSigningContextV1 {
            epoch_id: 2,
            view: 1,
            carrier_height: 8,
            parent_hash: HashOf::from_untyped_unchecked(Hash::new(b"parent-8")),
            validator_set_hash: HashOf::new(&vec![peer(b"validator")]),
        };
        let first = Hash::new(b"first");
        let second = Hash::new(b"second");
        let candidate = signing_candidate(&context, b"partial-temp");
        let guard = MergeSigningGuard::open(temp.path()).expect("open guard");
        guard
            .authorize(context.clone(), first, &candidate)
            .expect("authorize final record");
        let record_temp = guard.record_path(&context).with_extension("norito.tmp");
        fs::write(&record_temp, [0xA5, 0x5A]).expect("write partial record temp");
        let high_water_temp = MergeSigningGuard::high_water_temp_path(&guard.directory);
        fs::write(&high_water_temp, [0x01]).expect("write partial high-water temp");
        drop(guard);

        let restarted = MergeSigningGuard::open(temp.path()).expect("reconcile partial temps");
        assert!(!record_temp.exists());
        assert!(!high_water_temp.exists());
        assert_eq!(
            restarted.authorize(context, second, &candidate),
            Err(MergeSidecarError::LocalSigningEquivocation)
        );
    }

    #[cfg(unix)]
    #[test]
    fn signing_guard_rejects_symlink_and_unknown_temps() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("temp dir");
        let guard = MergeSigningGuard::open(temp.path()).expect("open guard");
        let target = temp.path().join("target");
        fs::write(&target, b"target").expect("write target");
        let malicious =
            guard
                .directory
                .join(format!("{}.{}", Hash::new(b"temp"), SIGNING_GUARD_TEMP_EXT));
        symlink(&target, &malicious).expect("create symlink temp");
        drop(guard);
        assert!(matches!(
            MergeSigningGuard::open(temp.path()),
            Err(MergeSidecarError::SigningGuard(_))
        ));
        fs::remove_file(&malicious).expect("remove malicious symlink");
        fs::write(
            temp.path().join(SIGNING_GUARD_DIR).join("unknown.tmp"),
            b"unknown",
        )
        .expect("write unknown temp");
        assert!(matches!(
            MergeSigningGuard::open(temp.path()),
            Err(MergeSidecarError::SigningGuard(_))
        ));
    }

    #[test]
    fn signing_guard_prune_boundary_never_reopens_committed_context() {
        let temp = tempfile::tempdir().expect("temp dir");
        let context = MergeSigningContextV1 {
            epoch_id: 1,
            view: 0,
            carrier_height: 2,
            parent_hash: HashOf::from_untyped_unchecked(Hash::new(b"parent-1")),
            validator_set_hash: HashOf::new(&vec![peer(b"validator")]),
        };
        let candidate = signing_candidate(&context, b"prune-boundary");
        let mut guard = MergeSigningGuard::open(temp.path()).expect("open guard");
        guard
            .authorize(context.clone(), Hash::new(b"first"), &candidate)
            .expect("authorize epoch");
        guard
            .advance_committed_epoch(1)
            .expect("commit and prune epoch");
        drop(guard);
        let restarted =
            MergeSigningGuard::open_with_committed_epoch(temp.path(), 1).expect("restart guard");
        assert_eq!(
            restarted.authorize(context, Hash::new(b"conflict"), &candidate),
            Err(MergeSidecarError::LocalSigningEquivocation)
        );
    }

    #[test]
    fn signing_guard_restart_completes_gc_after_high_water_crash_boundary() {
        let temp = tempfile::tempdir().expect("temp dir");
        let context = MergeSigningContextV1 {
            epoch_id: 1,
            view: 3,
            carrier_height: 2,
            parent_hash: HashOf::from_untyped_unchecked(Hash::new(b"parent-1")),
            validator_set_hash: HashOf::new(&vec![peer(b"validator")]),
        };
        let candidate = signing_candidate(&context, b"high-water-recovery");
        let mut guard = MergeSigningGuard::open(temp.path()).expect("open guard");
        guard
            .authorize(context.clone(), Hash::new(b"first"), &candidate)
            .expect("authorize decision");
        let record_path = guard.record_path(&context);
        let record_bytes = fs::read(&record_path).expect("capture durable decision");
        guard
            .advance_committed_frontier(1, 2)
            .expect("persist high-water and collect decision");
        assert!(!record_path.exists());

        // Recreate the exact on-disk state of a crash after the high-water was
        // fsynced but immediately before the now-idempotent record GC.
        fs::write(&record_path, record_bytes).expect("restore stale durable decision");
        drop(guard);
        let restarted = MergeSigningGuard::open_with_committed_frontier(
            temp.path(),
            1,
            2,
            MergeSigningGuardLimits::defaults(),
        )
        .expect("restart completes stale-record GC");
        assert!(!record_path.exists());
        assert_eq!(
            restarted.authorize(context, Hash::new(b"conflict"), &candidate),
            Err(MergeSidecarError::LocalSigningEquivocation)
        );
    }
