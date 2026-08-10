// Rooted filesystem and two-slot store regression tests.

#[cfg(test)]
mod tests {
    use std::{
        ffi::{OsStr, OsString},
        fs,
        io::{self, Seek as _, SeekFrom, Write as _},
        panic::AssertUnwindSafe,
        sync::{Arc, Barrier, mpsc},
        thread,
        time::Duration,
    };

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    use std::cell::Cell;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt as _;
    #[cfg(target_os = "macos")]
    use std::process::Command;
    #[cfg(target_os = "linux")]
    use std::{
        ffi::{CString, c_char, c_int, c_void},
        os::fd::AsRawFd as _,
    };

    use tempfile::tempdir;

    use super::{
        ExpectedFile, RootedDirectory, TWO_SLOT_LOST_FOUND_ENTRY_HARD_CAP_V1, TWO_SLOT_NAMES_V1,
        TWO_SLOT_ZERO_DIGEST, TwoSlotInitFileLockV1, TwoSlotSnapshotV1, TwoSlotStageV1,
        TwoSlotStoreConfigV1, TwoSlotStoreV1, decode_two_slot_value, encode_two_slot_value,
        initialize_two_slot_stage, open_existing_two_slot_store, read_exact_file_region,
        two_slot_init_lock_name, two_slot_lost_found_name, two_slot_stage_prefix,
        write_exact_file_region, write_two_slot_record_unlocked,
    };

    fn test_root(path: &std::path::Path) -> RootedDirectory {
        #[cfg(windows)]
        {
            RootedDirectory::open_root(path, true).expect("retain rooted Windows test directory")
        }
        #[cfg(not(windows))]
        {
            let handle = Arc::new(fs::File::open(path).expect("open test root"));
            RootedDirectory::from_retained(path.to_path_buf(), handle, true)
                .expect("retain rooted test directory")
        }
    }

    fn read_only_test_root(path: &std::path::Path) -> RootedDirectory {
        #[cfg(windows)]
        {
            RootedDirectory::open_root(path, false)
                .expect("retain read-only rooted Windows test directory")
        }
        #[cfg(not(windows))]
        {
            let handle = Arc::new(fs::File::open(path).expect("open read-only test root"));
            RootedDirectory::from_retained(path.to_path_buf(), handle, false)
                .expect("retain read-only rooted test directory")
        }
    }

    fn two_slot_config(name: &str) -> TwoSlotStoreConfigV1 {
        TwoSlotStoreConfigV1::try_new(name, [0x51; 32], [0xa7; 32], 512)
            .expect("valid bounded two-slot test config")
    }

    fn two_slot_fault(label: &'static str) -> io::Error {
        io::Error::other(format!("injected two-slot fault after {label}"))
    }

    fn raw_test_record(
        store: &TwoSlotStoreV1,
        slot_id: usize,
        generation: u64,
        predecessor_digest: [u8; 32],
        payload: &[u8],
    ) {
        let mut no_fault = |_| Ok(());
        store
            .with_exclusive_lock(|store| {
                write_two_slot_record_unlocked(
                    store,
                    slot_id,
                    generation,
                    predecessor_digest,
                    payload,
                    ["test"; 6],
                    &mut no_fault,
                )
                .map(drop)
            })
            .expect("write exact test record");
    }

    fn initialize_test_stage(
        root: &RootedDirectory,
        config: &TwoSlotStoreConfigV1,
        payload: &[u8],
    ) -> TwoSlotStageV1 {
        let lock = TwoSlotInitFileLockV1::acquire(root, config).expect("lock test initializer");
        let mut no_fault = |_| Ok(());
        let stage = initialize_two_slot_stage(root, config, lock.identity, payload, &mut no_fault)
            .expect("create complete test stage");
        lock.release().expect("unlock test initializer");
        stage
    }

    fn try_load_test_canonical(
        root: &RootedDirectory,
        config: &TwoSlotStoreConfigV1,
    ) -> io::Result<TwoSlotSnapshotV1> {
        let lock = TwoSlotInitFileLockV1::acquire(root, config)?;
        let result = root
            .open_directory(OsStr::new(&config.store_name))
            .and_then(|directory| {
                open_existing_two_slot_store(directory, config.clone(), lock.identity)
            })
            .and_then(|store| store.load());
        let unlock = lock.release();
        match (result, unlock) {
            (Ok(snapshot), Ok(())) => Ok(snapshot),
            (Err(error), _) | (Ok(_), Err(error)) => Err(error),
        }
    }

    fn root_two_slot_stage_names(
        root: &RootedDirectory,
        config: &TwoSlotStoreConfigV1,
    ) -> Vec<OsString> {
        let prefix = two_slot_stage_prefix(config);
        root.child_names()
            .expect("enumerate test root")
            .into_iter()
            .filter(|name| name.as_encoded_bytes().starts_with(prefix.as_bytes()))
            .collect()
    }

    #[test]
    fn two_slot_store_initializes_noops_and_reads_shorter_payload_exactly() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let config = two_slot_config("bounded-store");
        let initial = vec![0x5a; 257];
        let store = root
            .open_or_create_two_slot_store_v1(config, &initial)
            .expect("initialize two-slot store");
        let first = store.load().expect("load initial record");
        assert_eq!(first.generation(), 1);
        assert_eq!(first.payload(), initial);

        let before = store
            .slots
            .iter()
            .map(|slot| {
                read_exact_file_region(
                    &slot.handle,
                    0,
                    usize::try_from(store.layout.slot_file_bytes).expect("test slot fits usize"),
                )
                .expect("read exact slot")
            })
            .collect::<Vec<_>>();
        let no_op = store
            .compare_and_swap(&first, &initial)
            .expect("exact payload is a no-op");
        assert_eq!(no_op, first);
        let after = store
            .slots
            .iter()
            .map(|slot| {
                read_exact_file_region(
                    &slot.handle,
                    0,
                    usize::try_from(store.layout.slot_file_bytes).expect("test slot fits usize"),
                )
                .expect("read exact slot")
            })
            .collect::<Vec<_>>();
        assert_eq!(after, before, "no-op must not write either fixed slot");

        let slot_1_long = vec![0x6b; 301];
        let second = store
            .compare_and_swap(&no_op, &slot_1_long)
            .expect("commit long payload to slot one");
        let third = store
            .compare_and_swap(&second, &vec![0x7c; 299])
            .expect("advance through slot zero");
        let short = b"x";
        let fourth = store
            .compare_and_swap(&third, short)
            .expect("reuse slot one with a shorter payload");
        assert_eq!(fourth.generation(), 4);
        assert_eq!(fourth.payload(), short);
        write_exact_file_region(
            &store.slots[1].handle,
            store.layout.payload_offset + 100,
            &[0xe1],
        )
        .expect("mutate unauthenticated private stale tail");
        assert_eq!(store.load().expect("reload exact short payload"), fourth);

        let mut names = store
            .directory
            .child_names_bounded(2)
            .expect("enumerate fixed inventory");
        names.sort();
        let mut expected = TWO_SLOT_NAMES_V1.map(OsString::from).to_vec();
        expected.sort();
        assert_eq!(names, expected);
        for slot in &store.slots {
            assert_eq!(
                slot.handle.metadata().expect("slot metadata").len(),
                store.layout.slot_file_bytes
            );
        }
    }

    #[test]
    fn two_slot_store_loads_through_a_read_only_root_without_initializing() {
        let temp = tempdir().expect("tempdir");
        let config = two_slot_config("read-only-store");
        let writer_root = test_root(temp.path());
        let store = writer_root
            .open_or_create_two_slot_store_v1(config.clone(), b"initial")
            .expect("initialize writer store");
        let initial = store.load().expect("load initial writer record");
        store
            .compare_and_swap(&initial, b"committed")
            .expect("commit writer successor");
        drop(store);
        drop(writer_root);

        let reader_root = read_only_test_root(temp.path());
        let read_only_store =
            super::open_existing_read_only_two_slot_store_v1(&reader_root, config.clone())
                .expect("open exact store through read-only descriptors");
        let read_only_predecessor = read_only_store.load().expect("load read-only predecessor");
        for slot in &read_only_store.slots {
            let error =
                write_exact_file_region(&slot.handle, read_only_store.layout.payload_offset, b"X")
                    .expect_err("retained reader slot descriptor must reject writes");
            assert_ne!(error.kind(), io::ErrorKind::Interrupted);
        }
        assert!(
            read_only_store
                .compare_and_swap(&read_only_predecessor, b"forbidden")
                .is_err(),
            "a store reopened through read-only descriptors must not commit"
        );
        let snapshot = reader_root
            .load_existing_two_slot_store_v1(config)
            .expect("load existing store through a read-only capability");
        assert_eq!(snapshot.generation(), 2);
        assert_eq!(snapshot.payload(), b"committed");

        let absent = two_slot_config("absent-read-only-store");
        let error = reader_root
            .load_existing_two_slot_store_v1(absent.clone())
            .expect_err("read-only loading must not initialize an absent store");
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(
            !temp.path().join(&absent.store_name).exists(),
            "read-only loading must not create the requested store"
        );
        assert!(
            !temp.path().join(two_slot_init_lock_name(&absent)).exists(),
            "read-only loading must not create an initializer lock"
        );
    }

    #[test]
    fn read_only_root_cannot_open_or_initialize_mutable_two_slot_store() {
        let temp = tempdir().expect("tempdir");
        let writable_root = test_root(temp.path());
        let existing_config = two_slot_config("existing-store");
        let existing_store = writable_root
            .open_or_create_two_slot_store_v1(existing_config.clone(), b"existing")
            .expect("initialize existing test store");
        let before = writable_root.child_names().expect("enumerate test root");

        let read_only_root = read_only_test_root(temp.path());
        let existing_error = read_only_root
            .open_or_create_two_slot_store_v1(existing_config, b"replacement")
            .expect_err("read-only root must not return a mutable existing store");
        assert_eq!(existing_error.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(
            writable_root.child_names().expect("re-enumerate test root"),
            before,
            "read-only open must not create an initializer or staging artifact"
        );
        assert_eq!(
            existing_store
                .load()
                .expect("reload existing store")
                .payload(),
            b"existing",
            "read-only open must not mutate the existing store"
        );

        let absent_config = two_slot_config("absent-store");
        let absent_error = read_only_root
            .open_or_create_two_slot_store_v1(absent_config, b"initial")
            .expect_err("read-only root must not initialize an absent store");
        assert_eq!(absent_error.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(
            writable_root
                .child_names()
                .expect("enumerate after rejection"),
            before,
            "read-only initialization must not create an init lock, stage, or canonical store"
        );
    }

    #[test]
    fn two_slot_store_remains_two_fixed_files_after_more_than_1024_updates() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let config = two_slot_config("long-lived-store");
        let store = root
            .open_or_create_two_slot_store_v1(config, b"initial")
            .expect("initialize two-slot store");
        let mut snapshot = store.load().expect("load initial record");
        for generation in 0..1_025_u64 {
            let payload = format!("bounded-update-{generation:04}");
            snapshot = store
                .compare_and_swap(&snapshot, payload.as_bytes())
                .expect("commit bounded update");
        }
        assert_eq!(snapshot.generation(), 1_026);
        assert_eq!(store.load().expect("load final update"), snapshot);
        assert_eq!(
            store
                .directory
                .child_names_bounded(2)
                .expect("bounded inventory")
                .len(),
            2
        );
        let logical_bytes = store
            .slots
            .iter()
            .map(|slot| slot.handle.metadata().expect("slot metadata").len())
            .sum::<u64>();
        assert_eq!(logical_bytes, store.layout.slot_file_bytes * 2);
    }

    #[test]
    fn two_slot_initialization_recovers_after_every_injected_boundary() {
        const LABELS: &[&str] = &[
            "stage-directory-created",
            "stage-parent-synced",
            "slot-0-created",
            "slot-0-sized",
            "slot-0-sized-and-synced",
            "slot-1-created",
            "slot-1-sized",
            "slot-1-sized-and-synced",
            "slot-0-header-written",
            "slot-0-header-synced",
            "slot-1-header-written",
            "slot-1-header-synced",
            "initial-trailer-invalidated",
            "initial-trailer-invalidation-synced",
            "initial-record-written",
            "initial-record-synced",
            "initial-commit-trailer-written",
            "initial-commit-trailer-synced",
            "initial-record-readback-verified",
            "stage-directory-synced",
            "before-directory-rename",
            "directory-renamed",
            "parent-synced",
            "initialization-postcheck",
        ];

        for &fault_label in LABELS {
            let temp = tempdir().expect("tempdir");
            let root = test_root(temp.path());
            let config = two_slot_config("faulted-init");
            let error = root
                .open_or_create_two_slot_store_v1_with_init_hook(
                    config.clone(),
                    b"initial",
                    |step| {
                        if step == fault_label {
                            Err(two_slot_fault(fault_label))
                        } else {
                            Ok(())
                        }
                    },
                )
                .expect_err("fault must stop this initialization attempt");
            assert!(error.to_string().contains(fault_label));
            let recovered = root
                .open_or_create_two_slot_store_v1(config.clone(), b"initial")
                .unwrap_or_else(|error| panic!("recover after {fault_label}: {error}"));
            let snapshot = recovered
                .load()
                .unwrap_or_else(|error| panic!("load after {fault_label}: {error}"));
            assert_eq!(snapshot.generation(), 1, "fault label {fault_label}");
            assert_eq!(snapshot.payload(), b"initial", "fault label {fault_label}");
            assert!(
                root_two_slot_stage_names(&root, &config).is_empty(),
                "recovery must preserve stages in lost+found after {fault_label}"
            );
        }
    }

    #[test]
    fn two_slot_cas_recovers_after_every_injected_boundary() {
        const LABELS: &[&str] = &[
            "inactive-zero-trailer-written",
            "inactive-trailer-invalidated",
            "inactive-record-written",
            "inactive-record-synced",
            "inactive-commit-trailer-written",
            "inactive-commit-trailer-synced",
            "successor-readback-verified",
        ];
        for &fault_label in LABELS {
            let temp = tempdir().expect("tempdir");
            let root = test_root(temp.path());
            let store = root
                .open_or_create_two_slot_store_v1(two_slot_config("faulted-cas"), b"old")
                .expect("initialize CAS store");
            let old = store.load().expect("load old record");
            let error = store
                .compare_and_swap_with_test_hook(&old, b"new", |step| {
                    if step == fault_label {
                        Err(two_slot_fault(fault_label))
                    } else {
                        Ok(())
                    }
                })
                .expect_err("fault must stop CAS call");
            assert!(error.to_string().contains(fault_label));
            let observed = store.load().expect("load after CAS fault");
            let committed = matches!(
                fault_label,
                "inactive-commit-trailer-written"
                    | "inactive-commit-trailer-synced"
                    | "successor-readback-verified"
            );
            if committed {
                assert_eq!(observed.generation(), 2);
                assert_eq!(observed.payload(), b"new");
            } else {
                assert_eq!(observed, old);
                let retried = store
                    .compare_and_swap(&observed, b"new")
                    .expect("reuse torn peer slot");
                assert_eq!(retried.generation(), 2);
                assert_eq!(retried.payload(), b"new");
            }
        }
    }

    #[test]
    fn two_slot_compare_and_swap_serializes_concurrent_writers() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let config = two_slot_config("concurrent-cas");
        let first_store = root
            .open_or_create_two_slot_store_v1(config.clone(), b"old")
            .expect("initialize concurrent store");
        let second_store = root
            .open_or_create_two_slot_store_v1(config, b"old")
            .expect("reopen with independent handles");
        assert!(!Arc::ptr_eq(
            &first_store.slots[0].handle,
            &second_store.slots[0].handle
        ));
        let expected = first_store.load().expect("load predecessor");
        let barrier = Arc::new(Barrier::new(3));
        let mut writers = Vec::new();
        for (store, payload) in [
            (first_store.clone(), b"left".as_slice()),
            (second_store, b"right".as_slice()),
        ] {
            let expected = expected.clone();
            let barrier = Arc::clone(&barrier);
            let payload = payload.to_vec();
            writers.push(thread::spawn(move || {
                barrier.wait();
                store.compare_and_swap(&expected, &payload)
            }));
        }
        barrier.wait();
        let results = writers
            .into_iter()
            .map(|writer| writer.join().expect("writer did not panic"))
            .collect::<Vec<_>>();
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        let failure = results
            .iter()
            .find_map(|result| result.as_ref().err())
            .expect("one stale writer must fail");
        assert_eq!(failure.kind(), io::ErrorKind::WouldBlock);
        assert_eq!(first_store.load().expect("load winner").generation(), 2);
    }

    #[test]
    fn two_slot_open_create_is_concurrent_and_canonical_wins() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let config = two_slot_config("concurrent-init");
        let barrier = Arc::new(Barrier::new(3));
        let mut openers = Vec::new();
        for _ in 0..2 {
            let root = root.clone();
            let config = config.clone();
            let barrier = Arc::clone(&barrier);
            openers.push(thread::spawn(move || {
                barrier.wait();
                root.open_or_create_two_slot_store_v1(config, b"initial")
                    .and_then(|store| store.load())
            }));
        }
        barrier.wait();
        for opener in openers {
            let snapshot = opener
                .join()
                .expect("opener did not panic")
                .expect("concurrent open/create succeeds");
            assert_eq!(snapshot.generation(), 1);
            assert_eq!(snapshot.payload(), b"initial");
        }

        let canonical = root
            .open_or_create_two_slot_store_v1(config.clone(), b"initial")
            .expect("open canonical");
        let current = canonical.load().expect("load canonical");
        let advanced = canonical
            .compare_and_swap(&current, b"canonical-winner")
            .expect("advance canonical");
        let extra = initialize_test_stage(&root, &config, b"initial");
        drop(extra);
        let reopened = root
            .open_or_create_two_slot_store_v1(config.clone(), b"initial")
            .expect("canonical wins over exact race stage");
        assert_eq!(reopened.load().expect("load canonical winner"), advanced);
        assert!(root_two_slot_stage_names(&root, &config).is_empty());
        let lost = root
            .open_directory(&two_slot_lost_found_name(&config))
            .expect("race stage preserved in lost+found");
        assert_eq!(lost.child_names().expect("lost+found entries").len(), 1);
    }

    #[test]
    fn two_slot_init_file_lock_blocks_independent_handle_until_release() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let config = two_slot_config("init-lock-handoff");
        let first = TwoSlotInitFileLockV1::acquire(&root, &config).expect("first init lock");
        let first_identity = first.identity;
        let (started_tx, started_rx) = mpsc::channel();
        let (acquired_tx, acquired_rx) = mpsc::channel();
        let second_root = root.clone();
        let second_config = config.clone();
        let waiter = thread::spawn(move || {
            started_tx.send(()).expect("signal waiter start");
            let second = TwoSlotInitFileLockV1::acquire(&second_root, &second_config)
                .expect("second init lock");
            acquired_tx
                .send(second.identity)
                .expect("signal second acquisition");
            second.release().expect("release second init lock");
        });
        started_rx.recv().expect("waiter started");
        assert_eq!(
            acquired_rx.recv_timeout(Duration::from_millis(150)),
            Err(mpsc::RecvTimeoutError::Timeout),
            "independent init-lock handle must block"
        );
        first.release().expect("release first init lock");
        assert_eq!(
            acquired_rx
                .recv_timeout(Duration::from_secs(5))
                .expect("second lock acquires after handoff"),
            first_identity
        );
        waiter.join().expect("init-lock waiter did not panic");
    }

    #[test]
    fn two_slot_unrelated_store_progresses_while_another_os_lock_is_held() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let blocked = root
            .open_or_create_two_slot_store_v1(two_slot_config("blocked-store"), b"blocked")
            .expect("initialize blocked store");
        let independent = root
            .open_or_create_two_slot_store_v1(two_slot_config("independent-store"), b"independent")
            .expect("initialize independent store");
        let blocker = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(blocked.directory.display_path.join(TWO_SLOT_NAMES_V1[0]))
            .expect("open independent blocker handle");
        fs::File::lock(&blocker).expect("hold blocked store OS lock");

        let (blocked_started_tx, blocked_started_rx) = mpsc::channel();
        let blocked_wait_store = blocked.clone();
        let blocked_waiter = thread::spawn(move || {
            blocked_started_tx.send(()).expect("signal blocked load");
            blocked_wait_store.load()
        });
        blocked_started_rx.recv().expect("blocked load started");
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            match blocked.process_lock.try_lock() {
                Err(std::sync::TryLockError::WouldBlock) => break,
                Err(std::sync::TryLockError::Poisoned(poisoned)) => {
                    drop(poisoned.into_inner());
                }
                Ok(guard) => drop(guard),
            }
            assert!(
                std::time::Instant::now() < deadline,
                "blocked load did not reach its per-store OS-lock wait"
            );
            thread::yield_now();
        }
        let (independent_tx, independent_rx) = mpsc::channel();
        let independent_waiter = thread::spawn(move || {
            independent_tx
                .send(independent.load())
                .expect("send independent result");
        });
        let independent_result = independent_rx.recv_timeout(Duration::from_secs(2));
        fs::File::unlock(&blocker).expect("release blocked store OS lock");
        let blocked_result = blocked_waiter
            .join()
            .expect("blocked waiter did not panic")
            .expect("blocked store resumes");
        independent_waiter
            .join()
            .expect("independent waiter did not panic");
        assert_eq!(blocked_result.payload(), b"blocked");
        assert_eq!(
            independent_result
                .expect("unrelated store progresses before blocker release")
                .expect("independent load succeeds")
                .payload(),
            b"independent"
        );
    }

    #[test]
    fn two_slot_empty_initial_payload_roundtrips() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let store = root
            .open_or_create_two_slot_store_v1(two_slot_config("empty-payload"), b"")
            .expect("initialize empty payload");
        let snapshot = store.load().expect("load empty payload");
        assert_eq!(snapshot.generation(), 1);
        assert!(snapshot.payload().is_empty());
        assert_eq!(
            store
                .compare_and_swap(&snapshot, b"")
                .expect("empty exact no-op"),
            snapshot
        );
    }

    #[test]
    fn two_slot_recovery_promotes_lexically_first_complete_stage() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let config = two_slot_config("deterministic-init");
        let first = initialize_test_stage(&root, &config, b"initial");
        let second = initialize_test_stage(&root, &config, b"initial");
        let mut candidates = [
            (first.name.clone(), first.directory.identity),
            (second.name.clone(), second.directory.identity),
        ];
        candidates.sort_by(|left, right| left.0.cmp(&right.0));
        let expected_identity = candidates[0].1;
        drop((first, second));

        let store = root
            .open_or_create_two_slot_store_v1(config.clone(), b"initial")
            .expect("promote deterministic stage");
        assert_eq!(store.directory.identity, expected_identity);
        assert_eq!(store.load().expect("load promoted stage").generation(), 1);
        let lost = root
            .open_directory(&two_slot_lost_found_name(&config))
            .expect("other complete stage is preserved");
        assert_eq!(lost.child_names().expect("lost+found entries").len(), 1);
    }

    #[test]
    fn two_slot_selection_rejects_ambiguous_generations_and_bad_lineage() {
        for case in ["equal", "gap", "lineage"] {
            let temp = tempdir().expect("tempdir");
            let root = test_root(temp.path());
            let store = root
                .open_or_create_two_slot_store_v1(two_slot_config(case), b"one")
                .expect("initialize record-selection case");
            let first = store.load().expect("load generation one");
            match case {
                "equal" => raw_test_record(&store, 1, 1, TWO_SLOT_ZERO_DIGEST, b"other"),
                "gap" => raw_test_record(&store, 1, 3, first.record_digest(), b"three"),
                "lineage" => raw_test_record(&store, 1, 2, [0x99; 32], b"two"),
                _ => unreachable!(),
            }
            let error = store
                .load()
                .expect_err("ambiguous history must fail closed");
            assert_eq!(error.kind(), io::ErrorKind::InvalidData, "case {case}");
        }
    }

    #[test]
    fn two_slot_compare_and_swap_rejects_foreign_snapshot() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let left = root
            .open_or_create_two_slot_store_v1(two_slot_config("left-store"), b"left")
            .expect("initialize left store");
        let right = root
            .open_or_create_two_slot_store_v1(two_slot_config("right-store"), b"right")
            .expect("initialize right store");
        let foreign = left.load().expect("load foreign snapshot");
        let error = right
            .compare_and_swap(&foreign, b"substitute")
            .expect_err("foreign snapshot must not authorize CAS");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(
            right.load().expect("right store unchanged").payload(),
            b"right"
        );
    }

    #[test]
    fn two_slot_canonical_header_corruption_fails_without_overwrite() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let config = two_slot_config("corrupt-canonical");
        let store = root
            .open_or_create_two_slot_store_v1(config.clone(), b"initial")
            .expect("initialize canonical store");
        let identity = store.directory.identity;
        let offset = u64::try_from(store.layout.header_region_bytes - 1)
            .expect("test header offset fits u64");
        write_exact_file_region(&store.slots[0].handle, offset, &[0x7f])
            .expect("corrupt immutable reserved byte");
        store.slots[0].handle.sync_all().expect("sync corruption");
        assert!(store.load().is_err());
        assert!(
            root.open_or_create_two_slot_store_v1(config.clone(), b"replacement")
                .is_err(),
            "invalid canonical must never be overwritten"
        );
        assert_eq!(
            root.open_directory(OsStr::new(&config.store_name))
                .expect("canonical remains present")
                .identity,
            identity
        );
    }

    #[test]
    fn two_slot_binding_detects_slot_substitution_and_hard_links() {
        let substitution_temp = tempdir().expect("tempdir");
        let substitution_root = test_root(substitution_temp.path());
        let substitution_store = substitution_root
            .open_or_create_two_slot_store_v1(two_slot_config("substitution"), b"initial")
            .expect("initialize substitution store");
        let canonical_path = substitution_store.directory.display_path.clone();
        let slot_path = canonical_path.join(TWO_SLOT_NAMES_V1[1]);
        let preserved_path = canonical_path.join("preserved-slot");
        fs::rename(&slot_path, &preserved_path).expect("preserve original slot");
        let replacement = fs::File::create(&slot_path).expect("create replacement slot");
        replacement
            .set_len(substitution_store.layout.slot_file_bytes)
            .expect("size replacement slot");
        #[cfg(unix)]
        fs::set_permissions(&slot_path, fs::Permissions::from_mode(0o600))
            .expect("make replacement private");
        assert!(substitution_store.load().is_err());
        assert!(preserved_path.exists());
        assert!(slot_path.exists());

        let hard_link_temp = tempdir().expect("tempdir");
        let hard_link_root = test_root(hard_link_temp.path());
        let hard_link_store = hard_link_root
            .open_or_create_two_slot_store_v1(two_slot_config("hard-link"), b"initial")
            .expect("initialize hard-link store");
        let slot_path = hard_link_store
            .directory
            .display_path
            .join(TWO_SLOT_NAMES_V1[0]);
        let alias_path = hard_link_store.directory.display_path.join("slot-alias");
        fs::hard_link(&slot_path, &alias_path).expect("create hard link");
        assert!(hard_link_store.load().is_err());
        assert!(slot_path.exists());
        assert!(alias_path.exists());
    }

    #[test]
    fn two_slot_promotion_rejects_source_substitution_and_new_hard_link() {
        let substitution_temp = tempdir().expect("tempdir");
        let substitution_root = test_root(substitution_temp.path());
        let substitution_config = two_slot_config("stage-substitution");
        let mut substituted = false;
        let mut substituted_stage_name = None;
        let result = substitution_root.open_or_create_two_slot_store_v1_with_init_hook(
            substitution_config.clone(),
            b"initial",
            |step| {
                if step == "before-directory-rename" && !substituted {
                    let stage = root_two_slot_stage_names(&substitution_root, &substitution_config)
                        .into_iter()
                        .next()
                        .expect("stage exists before promotion");
                    let stage_path = substitution_temp.path().join(&stage);
                    let detached = substitution_temp.path().join("detached-stage");
                    fs::rename(&stage_path, &detached).expect("detach exact stage");
                    fs::create_dir(&stage_path).expect("install substituted stage directory");
                    substituted_stage_name = Some(stage);
                    substituted = true;
                }
                Ok(())
            },
        );
        if let Ok(store) = result {
            assert_eq!(
                store
                    .load()
                    .expect("only the exact original may be trusted")
                    .payload(),
                b"initial"
            );
        }
        let stage_name = substituted_stage_name.expect("substitution hook ran");
        let mut candidates = vec![
            substitution_temp.path().join("detached-stage"),
            substitution_temp
                .path()
                .join(&substitution_config.store_name),
            substitution_temp.path().join(stage_name),
        ];
        let lost_path = substitution_temp
            .path()
            .join(two_slot_lost_found_name(&substitution_config));
        if lost_path.is_dir() {
            candidates.extend(
                fs::read_dir(&lost_path)
                    .expect("inspect substitution lost+found")
                    .map(|entry| entry.expect("lost+found entry").path()),
            );
        }
        let inventories = candidates
            .iter()
            .filter(|path| path.is_dir())
            .map(|path| {
                fs::read_dir(path)
                    .expect("inspect preserved object")
                    .count()
            })
            .collect::<Vec<_>>();
        assert!(inventories.iter().any(|entries| *entries == 2));
        assert!(inventories.contains(&0));

        let hard_link_temp = tempdir().expect("tempdir");
        let hard_link_root = test_root(hard_link_temp.path());
        let hard_link_config = two_slot_config("stage-hard-link");
        let mut linked = false;
        let result = hard_link_root.open_or_create_two_slot_store_v1_with_init_hook(
            hard_link_config.clone(),
            b"initial",
            |step| {
                if step == "before-directory-rename" && !linked {
                    let stage = root_two_slot_stage_names(&hard_link_root, &hard_link_config)
                        .into_iter()
                        .next()
                        .expect("stage exists before promotion");
                    let stage = hard_link_temp.path().join(stage);
                    fs::hard_link(stage.join(TWO_SLOT_NAMES_V1[0]), stage.join("slot-alias"))
                        .expect("hard-link stage slot");
                    linked = true;
                }
                Ok(())
            },
        );
        assert!(result.is_err());
        if hard_link_root
            .open_directory(OsStr::new(&hard_link_config.store_name))
            .is_ok()
        {
            assert!(
                try_load_test_canonical(&hard_link_root, &hard_link_config).is_err(),
                "hard-linked promotion target must never be trusted"
            );
        }
        assert_eq!(
            root_two_slot_stage_names(&hard_link_root, &hard_link_config).len(),
            1
        );
    }

    #[test]
    fn two_slot_lost_found_preserves_multiple_stages_and_uses_free_slot() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let config = two_slot_config("lost-found");
        let lost = root
            .open_or_create_directory(&two_slot_lost_found_name(&config))
            .expect("create lost+found");
        lost.create_child_directory_exclusive(OsStr::new("entry-v1-0000"))
            .expect("preoccupy first lost+found slot");
        let prefix = two_slot_stage_prefix(&config);
        for suffix in [
            "0000000000000000-0000000000000000",
            "0000000000000000-0000000000000001",
        ] {
            root.create_child_directory_exclusive(OsStr::new(&format!("{prefix}{suffix}")))
                .expect("create incomplete stage");
        }
        let store = root
            .open_or_create_two_slot_store_v1(config.clone(), b"initial")
            .expect("preserve partial stages and continue");
        assert_eq!(
            store.load().expect("load initialized store").payload(),
            b"initial"
        );
        assert!(root_two_slot_stage_names(&root, &config).is_empty());
        let mut names = lost.child_names().expect("lost+found inventory");
        names.sort();
        assert_eq!(
            names,
            ["entry-v1-0000", "entry-v1-0001", "entry-v1-0002"].map(OsString::from)
        );
    }

    #[test]
    fn two_slot_lost_found_saturation_fails_without_deleting_stage() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let config = two_slot_config("saturated-lost-found");
        let lost = root
            .open_or_create_directory(&two_slot_lost_found_name(&config))
            .expect("create lost+found");
        for index in 0..TWO_SLOT_LOST_FOUND_ENTRY_HARD_CAP_V1 {
            lost.create_child_directory_exclusive(OsStr::new(&format!("entry-v1-{index:04}")))
                .expect("fill bounded lost+found");
        }
        let stage_name = OsString::from(format!(
            "{}0000000000000000-0000000000000000",
            two_slot_stage_prefix(&config)
        ));
        root.create_child_directory_exclusive(&stage_name)
            .expect("create stage requiring preservation");
        let error = root
            .open_or_create_two_slot_store_v1(config.clone(), b"initial")
            .expect_err("saturated lost+found must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
        assert_eq!(
            lost.child_names().expect("lost+found remains full").len(),
            16
        );
        assert_eq!(root_two_slot_stage_names(&root, &config), vec![stage_name]);
        assert!(
            root.open_directory(OsStr::new(&config.store_name)).is_err(),
            "canonical must not be installed after failed preservation"
        );
    }

    #[test]
    fn two_slot_valid_canonical_survives_saturated_lost_found_and_exact_stage() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let config = two_slot_config("available-canonical");
        let canonical = root
            .open_or_create_two_slot_store_v1(config.clone(), b"initial")
            .expect("initialize canonical");
        let initial = canonical.load().expect("load initial canonical");
        let advanced = canonical
            .compare_and_swap(&initial, b"canonical")
            .expect("advance canonical");
        let lost = root
            .open_or_create_directory(&two_slot_lost_found_name(&config))
            .expect("create lost+found");
        for index in 0..TWO_SLOT_LOST_FOUND_ENTRY_HARD_CAP_V1 {
            lost.create_child_directory_exclusive(OsStr::new(&format!("entry-v1-{index:04}")))
                .expect("fill lost+found");
        }
        let exact_stage = initialize_test_stage(&root, &config, b"initial");
        let stage_name = exact_stage.name.clone();
        drop(exact_stage);
        let reopened = root
            .open_or_create_two_slot_store_v1(config.clone(), b"initial")
            .expect("valid canonical remains available");
        assert_eq!(reopened.load().expect("load canonical"), advanced);
        assert_eq!(root_two_slot_stage_names(&root, &config), vec![stage_name]);
        assert_eq!(
            lost.child_names().expect("lost+found stays bounded").len(),
            16
        );
    }

    #[test]
    fn two_slot_uppercase_stage_suffix_is_rejected_and_preserved() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let config = two_slot_config("lowercase-stage");
        let name = OsString::from(format!(
            "{}000000000000000A-0000000000000000",
            two_slot_stage_prefix(&config)
        ));
        root.create_child_directory_exclusive(&name)
            .expect("create uppercase lookalike stage");
        let error = root
            .open_or_create_two_slot_store_v1(config.clone(), b"initial")
            .expect_err("uppercase stage namespace must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(temp.path().join(&name).is_dir());
        assert!(
            root.open_directory(OsStr::new(&config.store_name)).is_err(),
            "canonical remains absent"
        );
    }

    #[test]
    fn two_slot_nonempty_lost_found_does_not_block_clean_initialization() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let config = two_slot_config("nonempty-lost-found");
        let lost = root
            .open_or_create_directory(&two_slot_lost_found_name(&config))
            .expect("create lost+found");
        lost.create_child_directory_exclusive(OsStr::new("entry-v1-0000"))
            .expect("create preserved entry");
        let store = root
            .open_or_create_two_slot_store_v1(config, b"initial")
            .expect("nonempty lost+found is not a global stop");
        assert_eq!(
            store.load().expect("load clean store").payload(),
            b"initial"
        );
    }

    #[test]
    fn two_slot_preoccupied_canonical_is_never_overwritten() {
        let file_temp = tempdir().expect("tempdir");
        let file_root = test_root(file_temp.path());
        let file_config = two_slot_config("preoccupied-file");
        let file_path = file_temp.path().join(&file_config.store_name);
        fs::write(&file_path, b"sentinel").expect("preoccupy canonical file");
        #[cfg(unix)]
        fs::set_permissions(&file_path, fs::Permissions::from_mode(0o600))
            .expect("make sentinel private");
        assert!(
            file_root
                .open_or_create_two_slot_store_v1(file_config, b"initial")
                .is_err()
        );
        assert_eq!(fs::read(&file_path).expect("sentinel remains"), b"sentinel");

        let directory_temp = tempdir().expect("tempdir");
        let directory_root = test_root(directory_temp.path());
        let directory_config = two_slot_config("preoccupied-directory");
        let directory_path = directory_temp.path().join(&directory_config.store_name);
        fs::create_dir(&directory_path).expect("preoccupy canonical directory");
        fs::write(directory_path.join("sentinel"), b"keep").expect("write sentinel child");
        assert!(
            directory_root
                .open_or_create_two_slot_store_v1(directory_config, b"initial")
                .is_err()
        );
        assert_eq!(
            fs::read(directory_path.join("sentinel")).expect("sentinel child remains"),
            b"keep"
        );
    }

    #[test]
    fn two_slot_init_lock_substitution_and_hard_link_fail_closed() {
        let hard_link_temp = tempdir().expect("tempdir");
        let hard_link_root = test_root(hard_link_temp.path());
        let hard_link_config = two_slot_config("linked-init-lock");
        let hard_link_store = hard_link_root
            .open_or_create_two_slot_store_v1(hard_link_config.clone(), b"initial")
            .expect("initialize hard-link lock store");
        let lock_path = hard_link_temp
            .path()
            .join(two_slot_init_lock_name(&hard_link_config));
        let alias_path = hard_link_temp.path().join("init-lock-alias");
        fs::hard_link(&lock_path, &alias_path).expect("hard-link init lock");
        assert!(
            hard_link_root
                .open_or_create_two_slot_store_v1(hard_link_config, b"initial")
                .is_err()
        );
        assert_eq!(
            hard_link_store
                .load()
                .expect("already-open exact store remains readable")
                .payload(),
            b"initial"
        );
        assert!(lock_path.exists());
        assert!(alias_path.exists());

        let substitution_temp = tempdir().expect("tempdir");
        let substitution_root = test_root(substitution_temp.path());
        let substitution_config = two_slot_config("substituted-init-lock");
        let substitution_store = substitution_root
            .open_or_create_two_slot_store_v1(substitution_config.clone(), b"initial")
            .expect("initialize substitution lock store");
        let lock_path = substitution_temp
            .path()
            .join(two_slot_init_lock_name(&substitution_config));
        let preserved_path = substitution_temp.path().join("preserved-init-lock");
        fs::rename(&lock_path, &preserved_path).expect("preserve original init lock");
        fs::File::create(&lock_path).expect("install replacement init lock");
        #[cfg(unix)]
        fs::set_permissions(&lock_path, fs::Permissions::from_mode(0o600))
            .expect("make replacement init lock private");
        assert!(
            substitution_root
                .open_or_create_two_slot_store_v1(substitution_config, b"initial")
                .is_err(),
            "canonical headers bind the original init-lock identity"
        );
        assert_eq!(
            substitution_store
                .load()
                .expect("already-open exact store remains readable")
                .payload(),
            b"initial"
        );
        assert!(lock_path.exists());
        assert!(preserved_path.exists());
    }

    #[test]
    fn two_slot_cas_detects_mid_commit_slot_substitution_and_hard_link() {
        let substitution_temp = tempdir().expect("tempdir");
        let substitution_root = test_root(substitution_temp.path());
        let substitution_store = substitution_root
            .open_or_create_two_slot_store_v1(two_slot_config("cas-substitution"), b"old")
            .expect("initialize substitution CAS store");
        let expected = substitution_store.load().expect("load predecessor");
        let slot_path = substitution_store
            .directory
            .display_path
            .join(TWO_SLOT_NAMES_V1[1]);
        let preserved_path = substitution_store
            .directory
            .display_path
            .join("preserved-inactive");
        let mut substituted = false;
        let result =
            substitution_store.compare_and_swap_with_test_hook(&expected, b"new", |step| {
                if step == "inactive-zero-trailer-written" && !substituted {
                    fs::rename(&slot_path, &preserved_path).expect("preserve inactive slot");
                    let replacement = fs::File::create(&slot_path).expect("replace inactive slot");
                    replacement
                        .set_len(substitution_store.layout.slot_file_bytes)
                        .expect("size replacement slot");
                    #[cfg(unix)]
                    fs::set_permissions(&slot_path, fs::Permissions::from_mode(0o600))
                        .expect("make replacement private");
                    substituted = true;
                }
                Ok(())
            });
        assert!(result.is_err());
        assert!(slot_path.exists());
        assert!(preserved_path.exists());

        let hard_link_temp = tempdir().expect("tempdir");
        let hard_link_root = test_root(hard_link_temp.path());
        let hard_link_store = hard_link_root
            .open_or_create_two_slot_store_v1(two_slot_config("cas-hard-link"), b"old")
            .expect("initialize hard-link CAS store");
        let expected = hard_link_store.load().expect("load predecessor");
        let slot_path = hard_link_store
            .directory
            .display_path
            .join(TWO_SLOT_NAMES_V1[1]);
        let alias_path = hard_link_store
            .directory
            .display_path
            .join("inactive-alias");
        let mut linked = false;
        let result = hard_link_store.compare_and_swap_with_test_hook(&expected, b"new", |step| {
            if step == "inactive-zero-trailer-written" && !linked {
                fs::hard_link(&slot_path, &alias_path).expect("hard-link inactive slot");
                linked = true;
            }
            Ok(())
        });
        assert!(result.is_err());
        assert!(slot_path.exists());
        assert!(alias_path.exists());
    }

    #[test]
    fn two_slot_stable_nonzero_partial_trailer_fails_closed() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let store = root
            .open_or_create_two_slot_store_v1(two_slot_config("partial-record"), b"old")
            .expect("initialize partial-record store");
        let record_offset =
            u64::try_from(store.layout.header_region_bytes).expect("record offset fits u64");
        let active_record = read_exact_file_region(
            &store.slots[0].handle,
            record_offset,
            store.layout.record_header_region_bytes,
        )
        .expect("read active record header");
        write_exact_file_region(
            &store.slots[1].handle,
            record_offset,
            &active_record[..active_record.len() / 2],
        )
        .expect("write partial record header");
        let active_trailer = read_exact_file_region(
            &store.slots[0].handle,
            store.layout.trailer_offset,
            store.layout.commit_trailer_region_bytes,
        )
        .expect("read active trailer");
        write_exact_file_region(
            &store.slots[1].handle,
            store.layout.trailer_offset,
            &active_trailer[..active_trailer.len() / 2],
        )
        .expect("write partial commit trailer");
        store.slots[1].handle.sync_all().expect("sync torn bytes");
        let error = store
            .load()
            .expect_err("stable nonzero torn trailer must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn two_slot_exact_zero_trailer_allows_interrupted_body_reuse() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let store = root
            .open_or_create_two_slot_store_v1(two_slot_config("zero-trailer"), b"old")
            .expect("initialize zero-trailer store");
        let old = store.load().expect("load old record");
        let record_offset =
            u64::try_from(store.layout.header_region_bytes).expect("record offset fits u64");
        let active_record = read_exact_file_region(
            &store.slots[0].handle,
            record_offset,
            store.layout.record_header_region_bytes,
        )
        .expect("read active record header");
        write_exact_file_region(
            &store.slots[1].handle,
            record_offset,
            &active_record[..active_record.len() / 2],
        )
        .expect("write interrupted record body under zero trailer");
        store.slots[1]
            .handle
            .sync_all()
            .expect("sync interrupted body");
        assert_eq!(store.load().expect("ignore exact-zero inactive slot"), old);
        let recovered = store
            .compare_and_swap(&old, b"new")
            .expect("reuse exact-zero inactive slot");
        assert_eq!(recovered.generation(), 2);
        assert_eq!(recovered.payload(), b"new");
    }

    #[test]
    fn two_slot_newest_committed_corruption_never_falls_back() {
        for case in [
            "trailer-decode",
            "trailer-field",
            "record-digest",
            "header-decode",
            "payload",
            "oversized-length",
        ] {
            let temp = tempdir().expect("tempdir");
            let root = test_root(temp.path());
            let store = root
                .open_or_create_two_slot_store_v1(two_slot_config(case), b"old")
                .expect("initialize corruption case");
            let old = store.load().expect("load predecessor");
            let newest = store
                .compare_and_swap(&old, b"newest")
                .expect("commit newest record");
            assert_eq!(newest.generation(), 2);
            let slot = &store.slots[1];
            let record_offset =
                u64::try_from(store.layout.header_region_bytes).expect("record offset fits u64");

            match case {
                "trailer-decode" => {
                    write_exact_file_region(
                        &slot.handle,
                        store.layout.trailer_offset,
                        &vec![0xff; store.layout.commit_trailer_region_bytes],
                    )
                    .expect("corrupt trailer encoding");
                }
                "trailer-field" | "record-digest" => {
                    let bytes = read_exact_file_region(
                        &slot.handle,
                        store.layout.trailer_offset,
                        store.layout.commit_trailer_region_bytes,
                    )
                    .expect("read committed trailer");
                    let mut region: super::TwoSlotCommitTrailerRegionV1 =
                        decode_two_slot_value(&bytes, "test commit trailer")
                            .expect("decode committed trailer");
                    if case == "trailer-field" {
                        region.trailer.commit_marker[0] ^= 1;
                    } else {
                        region.trailer.record_digest[0] ^= 1;
                    }
                    let bytes = encode_two_slot_value(&region, "test commit trailer")
                        .expect("encode corrupted trailer");
                    write_exact_file_region(&slot.handle, store.layout.trailer_offset, &bytes)
                        .expect("write corrupted trailer");
                }
                "header-decode" => {
                    write_exact_file_region(
                        &slot.handle,
                        record_offset,
                        &vec![0xff; store.layout.record_header_region_bytes],
                    )
                    .expect("corrupt record header encoding");
                }
                "payload" => {
                    write_exact_file_region(&slot.handle, store.layout.payload_offset, b"X")
                        .expect("corrupt committed payload");
                }
                "oversized-length" => {
                    let bytes = read_exact_file_region(
                        &slot.handle,
                        record_offset,
                        store.layout.record_header_region_bytes,
                    )
                    .expect("read committed header");
                    let mut region: super::TwoSlotRecordHeaderRegionV1 =
                        decode_two_slot_value(&bytes, "test record header")
                            .expect("decode committed header");
                    region.header.payload_len = u64::try_from(store.config.max_payload_bytes)
                        .expect("bound fits u64")
                        .checked_add(1)
                        .expect("test bound has a successor");
                    let bytes = encode_two_slot_value(&region, "test record header")
                        .expect("encode oversized header");
                    write_exact_file_region(&slot.handle, record_offset, &bytes)
                        .expect("write oversized header");
                }
                _ => unreachable!(),
            }
            slot.handle.sync_all().expect("sync corruption");
            let error = store
                .load()
                .expect_err("newest committed corruption must not fall back");
            assert_eq!(error.kind(), io::ErrorKind::InvalidData, "case {case}");
        }
    }

    #[test]
    fn two_slot_process_mutex_and_init_file_lock_recover_after_panics() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let config = two_slot_config("poison-recovery");
        let store = root
            .open_or_create_two_slot_store_v1(config.clone(), b"initial")
            .expect("initialize poison test store");
        let process_panic = std::panic::catch_unwind(AssertUnwindSafe(|| {
            let _: io::Result<()> = store.with_exclusive_lock(|_| panic!("poison process lock"));
        }));
        assert!(process_panic.is_err());
        assert_eq!(
            store.load().expect("recover process lock").payload(),
            b"initial"
        );
        assert!(store.process_lock.is_poisoned());

        let init_temp = tempdir().expect("tempdir");
        let init_root = test_root(init_temp.path());
        let init_config = two_slot_config("init-poison-recovery");
        let init_panic = std::panic::catch_unwind(AssertUnwindSafe(|| {
            let _ = init_root.open_or_create_two_slot_store_v1_with_init_hook(
                init_config.clone(),
                b"initial",
                |step| {
                    if step == "stage-directory-created" {
                        panic!("poison init lock");
                    }
                    Ok(())
                },
            );
        }));
        assert!(init_panic.is_err());
        let recovered = init_root
            .open_or_create_two_slot_store_v1(init_config, b"initial")
            .expect("recover init lock and partial stage");
        assert_eq!(
            recovered.load().expect("load recovered init").payload(),
            b"initial"
        );
    }

    #[cfg(target_os = "linux")]
    unsafe extern "C" {
        fn fsetxattr(
            fd: c_int,
            name: *const c_char,
            value: *const c_void,
            size: usize,
            flags: c_int,
        ) -> c_int;
        fn fremovexattr(fd: c_int, name: *const c_char) -> c_int;
    }

    #[cfg(target_os = "linux")]
    fn install_linux_default_acl(handle: &fs::File) -> CString {
        fn push_acl_entry(bytes: &mut Vec<u8>, tag: u16, permissions: u16, id: u32) {
            bytes.extend_from_slice(&tag.to_le_bytes());
            bytes.extend_from_slice(&permissions.to_le_bytes());
            bytes.extend_from_slice(&id.to_le_bytes());
        }

        let name = CString::new("system.posix_acl_default").expect("ACL xattr name");
        let mut acl = 2_u32.to_le_bytes().to_vec();
        let undefined_id = u32::MAX;
        push_acl_entry(&mut acl, 0x01, 0o7, undefined_id);
        push_acl_entry(&mut acl, 0x02, 0o7, 65_534);
        push_acl_entry(&mut acl, 0x04, 0o0, undefined_id);
        push_acl_entry(&mut acl, 0x10, 0o7, undefined_id);
        push_acl_entry(&mut acl, 0x20, 0o0, undefined_id);
        // SAFETY: the descriptor and NUL-terminated name are valid and the
        // ACL buffer follows Linux's fixed little-endian POSIX ACL xattr ABI.
        let installed = unsafe {
            fsetxattr(
                handle.as_raw_fd(),
                name.as_ptr(),
                acl.as_ptr().cast(),
                acl.len(),
                0,
            )
        };
        assert_eq!(
            installed,
            0,
            "install descriptor-bound POSIX default ACL: {}",
            io::Error::last_os_error()
        );
        name
    }

    #[cfg(target_os = "linux")]
    fn remove_linux_default_acl(handle: &fs::File, name: &CString) {
        // SAFETY: the retained descriptor and NUL-terminated xattr name remain
        // valid for this cleanup call.
        assert_eq!(
            unsafe { fremovexattr(handle.as_raw_fd(), name.as_ptr()) },
            0
        );
    }

    #[test]
    fn windows_dacl_qualification_source_contract_is_handle_bound() {
        let source = [
            include_str!("../governance_rooted_fs.rs"),
            include_str!("two_slot_store.rs"),
        ]
        .concat();
        assert!(source.contains("#[link_name = \"GetSecurityInfo\"]"));
        assert!(source.contains("#[link_name = \"GetSecurityDescriptorControl\"]"));
        assert!(source.contains("#[link_name = \"LocalFree\"]"));
        assert!(source.contains("handle.as_raw_handle(),"));
        let pathname_api = ["GetNamed", "SecurityInfo"].concat();
        assert!(!source.contains(&pathname_api));
    }

    #[test]
    fn windows_atomic_replacement_source_contract_is_non_destructive() {
        let source = [
            include_str!("../governance_rooted_fs.rs"),
            include_str!("two_slot_store.rs"),
        ]
        .concat();
        assert!(source.contains("(*info).replace_or_flags = 0;"));
        assert!(source.contains("without replacement: {error}"));
        assert!(source.contains("Windows governance existing-target replacement is disabled"));
        assert!(source.contains("metadata.number_of_links() != Some(1)"));
        let destructive_match = ["matches!(&expected, ExpectedFile::", "Identity(_))"].concat();
        assert!(!source.contains(&destructive_match));
    }

    #[test]
    fn linux_acl_stability_contract_rejects_equal_length_churn() {
        let mut snapshots = std::collections::VecDeque::from([
            b"user.a\0".to_vec(),
            b"user.b\0".to_vec(),
            b"user.a\0".to_vec(),
            b"user.b\0".to_vec(),
            b"user.a\0".to_vec(),
            b"user.b\0".to_vec(),
        ]);
        let error = super::stable_linux_acl_attribute_names(
            std::path::Path::new("synthetic-linux-directory"),
            || {
                Ok(Some(
                    snapshots
                        .pop_front()
                        .expect("bounded stability reader call"),
                ))
            },
        )
        .expect_err("equal-length ACL-name substitution must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
        assert!(
            snapshots.is_empty(),
            "both snapshots in every retry are read"
        );
    }

    #[cfg(windows)]
    #[test]
    fn rooted_directory_pins_initial_windows_owner_sid() {
        let temp = tempdir().expect("tempdir");
        let mut root = test_root(temp.path());
        root.owner_sid[0] ^= 1;
        assert_eq!(
            root.verify()
                .expect_err("substituted pinned owner SID must fail closed")
                .kind(),
            io::ErrorKind::PermissionDenied
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn retained_directory_acl_policy_accepts_plain_directory() {
        let temp = tempdir().expect("tempdir");
        let handle = fs::File::open(temp.path()).expect("open plain directory");
        super::validate_retained_directory_acl(&handle, temp.path())
            .expect("plain descriptor has no ACL mutation grant");
    }

    #[cfg(target_os = "macos")]
    fn change_macos_acl(path: &std::path::Path, operation: &str, acl: Option<&str>) {
        let mut command = Command::new("chmod");
        command.arg(operation);
        if let Some(acl) = acl {
            command.arg(acl);
        }
        let status = command
            .arg(path)
            .status()
            .expect("execute macOS chmod ACL operation");
        assert!(status.success(), "macOS chmod ACL operation must succeed");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn retained_directory_acl_policy_rejects_mutation_allow_entry() {
        let temp = tempdir().expect("tempdir");
        change_macos_acl(temp.path(), "+a", Some("everyone allow add_file"));
        let handle = fs::File::open(temp.path()).expect("open ACL directory");
        let result = super::validate_retained_directory_acl(&handle, temp.path());
        change_macos_acl(temp.path(), "-RN", None);
        let error = result.expect_err("ACL add-file grant must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn retained_directory_acl_policy_accepts_deny_only_entry() {
        let temp = tempdir().expect("tempdir");
        change_macos_acl(temp.path(), "+a", Some("everyone deny delete"));
        let handle = fs::File::open(temp.path()).expect("open deny-ACL directory");
        let result = super::validate_retained_directory_acl(&handle, temp.path());
        change_macos_acl(temp.path(), "-RN", None);
        result.expect("deny-only ACL must not grant mutation authority");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn retained_directory_acl_policy_rejects_posix_default_acl() {
        let temp = tempdir().expect("tempdir");
        let handle = fs::File::open(temp.path()).expect("open ACL directory");
        let name = install_linux_default_acl(&handle);
        let result = super::validate_retained_directory_acl(&handle, temp.path());
        remove_linux_default_acl(&handle, &name);
        let error = result.expect_err("POSIX ACL attribute must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn rooted_descendant_rejects_post_capture_acl_mutation() {
        let temp = tempdir().expect("tempdir");
        fs::create_dir(temp.path().join("child")).expect("create child");
        let root = test_root(temp.path());
        let child = root
            .open_directory(OsStr::new("child"))
            .expect("retain child");
        let name = install_linux_default_acl(&child.handle);
        let result = child.verify();
        remove_linux_default_acl(&child.handle, &name);
        assert_eq!(
            result
                .expect_err("post-capture descendant ACL must fail closed")
                .kind(),
            io::ErrorKind::PermissionDenied
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn rooted_descendant_rejects_post_capture_acl_mutation() {
        let temp = tempdir().expect("tempdir");
        let child_path = temp.path().join("child");
        fs::create_dir(&child_path).expect("create child");
        let root = test_root(temp.path());
        let child = root
            .open_directory(OsStr::new("child"))
            .expect("retain child");
        change_macos_acl(&child_path, "+a", Some("everyone allow add_file"));
        let result = child.verify();
        change_macos_acl(&child_path, "-RN", None);
        assert_eq!(
            result
                .expect_err("post-capture descendant ACL must fail closed")
                .kind(),
            io::ErrorKind::PermissionDenied
        );
    }

    #[test]
    fn rooted_atomic_write_rejects_equal_length_identity_substitution() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        fs::write(temp.path().join("state"), b"first").expect("seed target");
        let snapshot = root
            .read_file(OsStr::new("state"), 16)
            .expect("read original target");
        fs::remove_file(temp.path().join("state")).expect("remove original target");
        fs::write(temp.path().join("state"), b"other").expect("replace with equal length");
        let error = root
            .atomic_write(
                OsStr::new("state"),
                OsStr::new(".state.tmp-1-1"),
                b"next",
                ExpectedFile::Identity(snapshot.binding()),
            )
            .expect_err("identity substitution must fail");
        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
        assert_eq!(
            fs::read(temp.path().join("state")).expect("read target"),
            b"other"
        );
    }

    #[test]
    fn rooted_atomic_exact_bytes_are_storage_idempotent() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        fs::write(temp.path().join("state"), b"unchanged").expect("seed exact state");
        let snapshot = root
            .read_file(OsStr::new("state"), 32)
            .expect("bind exact state");

        root.atomic_write(
            OsStr::new("state"),
            OsStr::new(".state.tmp-1-9"),
            b"unchanged",
            ExpectedFile::Identity(snapshot.binding()),
        )
        .expect("exact-byte retry is a verified no-op");
        assert_eq!(
            fs::read(temp.path().join("state")).expect("read exact state"),
            b"unchanged"
        );
        assert!(!temp.path().join(".state.tmp-1-9").exists());
        #[cfg(any(target_os = "linux", target_os = "macos", windows))]
        assert!(!temp.path().join(".state.retained-v1-0000").exists());
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rooted_atomic_write_replaces_the_exact_existing_destination() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        fs::write(temp.path().join("state"), b"predecessor").expect("seed predecessor");
        let predecessor = root
            .read_file(OsStr::new("state"), 32)
            .expect("read predecessor");
        root.atomic_write(
            OsStr::new("state"),
            OsStr::new(".state.tmp-1-10"),
            b"successor",
            ExpectedFile::Identity(predecessor.binding()),
        )
        .expect("replace the exact existing destination");
        assert_eq!(
            fs::read(temp.path().join("state")).expect("read successor"),
            b"successor"
        );
        assert!(!temp.path().join(".state.tmp-1-10").exists());
        assert_eq!(
            fs::read(temp.path().join(".state.retained-v1-0000"))
                .expect("read exact retained predecessor"),
            b"predecessor"
        );
        let successor = root
            .read_file(OsStr::new("state"), 32)
            .expect("retain first successor");
        root.atomic_write(
            OsStr::new("state"),
            OsStr::new(".state.tmp-1-11"),
            b"second-successor",
            ExpectedFile::Identity(successor.binding()),
        )
        .expect("use the next bounded retained-generation slot");
        assert_eq!(
            fs::read(temp.path().join(".state.retained-v1-0001"))
                .expect("read second retained predecessor"),
            b"successor"
        );
    }

    #[cfg(windows)]
    #[test]
    fn rooted_atomic_write_fails_closed_for_changed_windows_target() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        fs::write(temp.path().join("state"), b"predecessor").expect("seed predecessor");
        let predecessor = root
            .read_file(OsStr::new("state"), 32)
            .expect("bind Windows predecessor");

        let error = root
            .atomic_write(
                OsStr::new("state"),
                OsStr::new(".state.tmp-1-10"),
                b"successor",
                ExpectedFile::Identity(predecessor.binding()),
            )
            .expect_err("Windows changed-target replacement must fail before mutation");
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
        assert_eq!(
            fs::read(temp.path().join("state")).expect("read untouched predecessor"),
            b"predecessor"
        );
        assert!(!temp.path().join(".state.tmp-1-10").exists());
        assert!(!temp.path().join(".state.retained-v1-0000").exists());
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rooted_atomic_exchange_preserves_a_substituted_target_and_predecessor() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let target = temp.path().join("state");
        let detached = temp.path().join("detached-predecessor");
        let temporary = temp.path().join(".state.tmp-1-20");
        fs::write(&target, b"expected-predecessor").expect("seed predecessor");
        let predecessor = root
            .read_file(OsStr::new("state"), 64)
            .expect("retain predecessor");

        let error = root
            .atomic_write_with_test_hooks(
                OsStr::new("state"),
                OsStr::new(".state.tmp-1-20"),
                b"prepared-successor",
                ExpectedFile::Identity(predecessor.binding()),
                || {
                    fs::rename(&target, &detached).expect("detach expected predecessor");
                    fs::write(&target, b"racing-replacement").expect("install replacement");
                    Ok(())
                },
                |file| file.sync_all(),
                |directory| directory.sync_all(),
            )
            .expect_err("exchange must detect the substituted predecessor");
        assert!(error.to_string().contains("substituted during exchange"));
        assert_eq!(
            fs::read(&target).expect("read target"),
            b"prepared-successor"
        );
        assert_eq!(
            fs::read(&temporary).expect("read preserved replacement"),
            b"racing-replacement"
        );
        assert_eq!(
            fs::read(&detached).expect("read detached predecessor"),
            b"expected-predecessor"
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rooted_atomic_exchange_preserves_a_substituted_prepared_object() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let target = temp.path().join("state");
        let temporary = temp.path().join(".state.tmp-1-21");
        let detached_prepared = temp.path().join("detached-prepared");
        fs::write(&target, b"predecessor").expect("seed predecessor");
        let predecessor = root
            .read_file(OsStr::new("state"), 64)
            .expect("retain predecessor");

        root.atomic_write_with_test_hooks(
            OsStr::new("state"),
            OsStr::new(".state.tmp-1-21"),
            b"prepared-successor",
            ExpectedFile::Identity(predecessor.binding()),
            || {
                fs::rename(&temporary, &detached_prepared).expect("detach prepared object");
                fs::write(&temporary, b"racing-replacement").expect("replace prepared name");
                Ok(())
            },
            |file| file.sync_all(),
            |directory| directory.sync_all(),
        )
        .expect_err("promoted identity substitution must fail closed");
        assert_eq!(
            fs::read(&target).expect("read target"),
            b"racing-replacement"
        );
        assert_eq!(
            fs::read(&temporary).expect("read preserved predecessor"),
            b"predecessor"
        );
        assert_eq!(
            fs::read(&detached_prepared).expect("read detached prepared bytes"),
            b"prepared-successor"
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rooted_atomic_retention_never_overwrites_a_prepopulated_slot() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let target = temp.path().join("state");
        let retained = temp.path().join(".state.retained-v1-0000");
        let temporary = temp.path().join(".state.tmp-1-22");
        fs::write(&target, b"predecessor").expect("seed predecessor");
        let predecessor = root
            .read_file(OsStr::new("state"), 64)
            .expect("retain predecessor");

        root.atomic_write_with_test_hooks(
            OsStr::new("state"),
            OsStr::new(".state.tmp-1-22"),
            b"successor",
            ExpectedFile::Identity(predecessor.binding()),
            || {
                fs::write(&retained, b"prepopulated-slot").expect("race retention slot");
                Ok(())
            },
            |file| file.sync_all(),
            |directory| directory.sync_all(),
        )
        .expect_err("exclusive retention must reject a populated destination");
        assert_eq!(fs::read(&target).expect("read target"), b"successor");
        assert_eq!(
            fs::read(&temporary).expect("read preserved predecessor"),
            b"predecessor"
        );
        assert_eq!(
            fs::read(&retained).expect("read prepopulated slot"),
            b"prepopulated-slot"
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rooted_atomic_retention_does_not_mutate_a_racing_hardlink() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let target = temp.path().join("state");
        let external = temp.path().join("external-predecessor-link");
        let temporary = temp.path().join(".state.tmp-1-23");
        fs::write(&target, b"predecessor-bytes").expect("seed predecessor");
        let predecessor = root
            .read_file(OsStr::new("state"), 64)
            .expect("retain predecessor");

        root.atomic_write_with_test_hooks(
            OsStr::new("state"),
            OsStr::new(".state.tmp-1-23"),
            b"successor-bytes",
            ExpectedFile::Identity(predecessor.binding()),
            || {
                fs::hard_link(&target, &external).expect("race an external hard link");
                Ok(())
            },
            |file| file.sync_all(),
            |directory| directory.sync_all(),
        )
        .expect_err("post-check hard link must stop retention");
        assert_eq!(fs::read(&target).expect("read target"), b"successor-bytes");
        assert_eq!(
            fs::read(&temporary).expect("read exchanged predecessor"),
            b"predecessor-bytes"
        );
        assert_eq!(
            fs::read(&external).expect("read external predecessor link"),
            b"predecessor-bytes"
        );
    }

    #[test]
    fn rooted_child_binding_rejects_ancestor_replacement() {
        let temp = tempdir().expect("tempdir");
        fs::create_dir(temp.path().join("child")).expect("create child");
        let root = test_root(temp.path());
        let child = root
            .open_directory(OsStr::new("child"))
            .expect("retain child");
        fs::rename(temp.path().join("child"), temp.path().join("original"))
            .expect("rename retained child");
        fs::create_dir(temp.path().join("child")).expect("create replacement child");
        let error = child
            .atomic_replace_current(
                OsStr::new("state"),
                OsStr::new(".state.tmp-1-2"),
                b"must-not-land",
            )
            .expect_err("substituted ancestor must fail");
        assert!(!temp.path().join("child/state").exists());
        assert!(!temp.path().join("original/state").exists());
        assert!(error.to_string().contains("substituted"));
    }

    #[cfg(unix)]
    #[test]
    fn rooted_child_open_rejects_symlink() {
        let temp = tempdir().expect("tempdir");
        fs::create_dir(temp.path().join("outside")).expect("create outside");
        std::os::unix::fs::symlink(temp.path().join("outside"), temp.path().join("child"))
            .expect("create child symlink");
        let root = test_root(temp.path());
        assert!(
            root.open_directory(OsStr::new("child")).is_err(),
            "no-follow traversal must reject symlinks"
        );
    }

    #[test]
    fn rooted_atomic_write_propagates_directory_sync_failure() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let error = root
            .atomic_write_with_test_sync(
                OsStr::new("state"),
                OsStr::new(".state.tmp-1-3"),
                b"durability-uncertain",
                ExpectedFile::Missing,
                |file| file.sync_all(),
                |_directory| Err(io::Error::other("injected directory sync failure")),
            )
            .expect_err("directory sync failure must propagate");
        assert!(
            error
                .to_string()
                .contains("injected directory sync failure")
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rooted_atomic_replacement_preserves_both_generations_when_exchange_sync_fails() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        fs::write(temp.path().join("state"), b"predecessor").expect("seed predecessor");
        let predecessor = root
            .read_file(OsStr::new("state"), 64)
            .expect("retain predecessor");
        let sync_calls = Cell::new(0_usize);

        root.atomic_write_with_test_sync(
            OsStr::new("state"),
            OsStr::new(".state.tmp-1-30"),
            b"successor",
            ExpectedFile::Identity(predecessor.binding()),
            |file| file.sync_all(),
            |_directory| {
                let call = sync_calls.get() + 1;
                sync_calls.set(call);
                if call == 1 {
                    Err(io::Error::other("injected exchange sync failure"))
                } else {
                    Ok(())
                }
            },
        )
        .expect_err("exchange directory sync failure must propagate");
        assert_eq!(sync_calls.get(), 1);
        assert_eq!(
            fs::read(temp.path().join("state")).expect("read successor"),
            b"successor"
        );
        assert_eq!(
            fs::read(temp.path().join(".state.tmp-1-30"))
                .expect("read preserved predecessor temporary"),
            b"predecessor"
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rooted_atomic_replacement_preserves_retained_generation_when_retention_sync_fails() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        fs::write(temp.path().join("state"), b"predecessor").expect("seed predecessor");
        let predecessor = root
            .read_file(OsStr::new("state"), 64)
            .expect("retain predecessor");
        let sync_calls = Cell::new(0_usize);

        root.atomic_write_with_test_sync(
            OsStr::new("state"),
            OsStr::new(".state.tmp-1-31"),
            b"successor",
            ExpectedFile::Identity(predecessor.binding()),
            |file| file.sync_all(),
            |_directory| {
                let call = sync_calls.get() + 1;
                sync_calls.set(call);
                if call == 2 {
                    Err(io::Error::other("injected retention sync failure"))
                } else {
                    Ok(())
                }
            },
        )
        .expect_err("retention directory sync failure must propagate");
        assert_eq!(sync_calls.get(), 2);
        assert_eq!(
            fs::read(temp.path().join("state")).expect("read successor"),
            b"successor"
        );
        assert_eq!(
            fs::read(temp.path().join(".state.retained-v1-0000"))
                .expect("read retained predecessor"),
            b"predecessor"
        );
        assert!(!temp.path().join(".state.tmp-1-31").exists());
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rooted_atomic_replacement_fails_closed_when_retention_slots_are_saturated() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        fs::write(temp.path().join("state"), b"predecessor").expect("seed predecessor");
        for slot in 0..super::ATOMIC_RETAINED_SLOT_COUNT_V1 {
            fs::write(
                temp.path().join(format!(".state.retained-v1-{slot:04}")),
                b"retained",
            )
            .expect("fill retained-generation slot");
        }
        let predecessor = root
            .read_file(OsStr::new("state"), 64)
            .expect("retain predecessor");

        let error = root
            .atomic_write(
                OsStr::new("state"),
                OsStr::new(".state.tmp-1-32"),
                b"must-not-land",
                ExpectedFile::Identity(predecessor.binding()),
            )
            .expect_err("saturated retention must fail before creating a successor");
        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
        assert!(error.to_string().contains("offline"));
        assert_eq!(
            fs::read(temp.path().join("state")).expect("read unchanged predecessor"),
            b"predecessor"
        );
        assert!(!temp.path().join(".state.tmp-1-32").exists());
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rooted_atomic_replacement_enforces_retention_aggregate_byte_bound() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        fs::write(temp.path().join("state"), b"predecessor").expect("seed predecessor");
        let mut retained = fs::File::create(temp.path().join(".other.retained-v1-0000"))
            .expect("seed sparse retained generation");
        retained
            .seek(SeekFrom::Start(
                super::ATOMIC_RETAINED_TOTAL_MAX_BYTES_V1 - 1,
            ))
            .expect("seek sparse retained generation");
        retained
            .write_all(&[0])
            .expect("extend sparse retained generation");
        let predecessor = root
            .read_file(OsStr::new("state"), 64)
            .expect("retain predecessor");

        let error = root
            .atomic_write(
                OsStr::new("state"),
                OsStr::new(".state.tmp-1-33"),
                b"must-not-land",
                ExpectedFile::Identity(predecessor.binding()),
            )
            .expect_err("aggregate retention bound must fail before exchange");
        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
        assert!(error.to_string().contains("aggregate bound"));
        assert_eq!(
            fs::read(temp.path().join("state")).expect("read unchanged predecessor"),
            b"predecessor"
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rooted_atomic_write_preserves_a_pre_rename_temporary_after_failure() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let temporary_name = OsStr::new(".state.tmp-1-4");
        let error = root
            .atomic_write_with_test_sync(
                OsStr::new("state"),
                temporary_name,
                b"preserved-for-recovery",
                ExpectedFile::Missing,
                |_file| Err(io::Error::other("injected file sync failure")),
                |_directory| Ok(()),
            )
            .expect_err("file sync failure must stop before rename");
        assert!(error.to_string().contains("injected file sync failure"));
        assert!(!temp.path().join("state").exists());
        assert_eq!(
            fs::read(temp.path().join(temporary_name))
                .expect("failed transaction temporary remains recoverable"),
            b"preserved-for-recovery"
        );
    }

    #[test]
    fn atomic_temp_candidate_classifier_is_target_exact_and_fail_closed() {
        assert!(super::is_atomic_temp_candidate_for(
            ".state.tmp-42000-1",
            "state"
        ));
        assert!(super::is_atomic_temp_candidate_for(
            ".state.tmp-malformed",
            "state"
        ));
        assert!(!super::is_atomic_temp_candidate_for(
            ".other.tmp-42000-1",
            "state"
        ));
        assert!(!super::is_atomic_temp_candidate_for(
            ".stateful.tmp-42000-1",
            "state"
        ));
        assert!(!super::is_atomic_temp_candidate_for(
            "state.tmp-42000-1",
            "state"
        ));
    }

    #[test]
    fn rooted_recovery_removes_only_matching_atomic_temporaries() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        fs::write(temp.path().join(".state.tmp-42000-1"), b"stale").expect("seed stale temp");
        fs::write(temp.path().join(".other.tmp-42000-1"), b"other").expect("seed unrelated temp");
        assert_eq!(
            root.remove_atomic_temps_for("state")
                .expect("recover matching temp"),
            1
        );
        assert!(!temp.path().join(".state.tmp-42000-1").exists());
        assert!(temp.path().join(".other.tmp-42000-1").exists());
    }

    #[test]
    fn rooted_bounded_atomic_temp_recovery_filters_decoded_targets() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        fs::write(temp.path().join(".state.tmp-42000-1"), b"stale").expect("seed allowed temp");
        fs::write(temp.path().join(".other.tmp-42000-2"), b"other").expect("seed rejected temp");
        fs::write(temp.path().join("retained"), b"retained").expect("seed retained file");

        assert_eq!(
            root.remove_atomic_temps_matching(3, |target| target == "state")
                .expect("recover bounded allowed temp"),
            1
        );
        assert!(!temp.path().join(".state.tmp-42000-1").exists());
        assert!(temp.path().join(".other.tmp-42000-2").exists());
        assert_eq!(
            fs::read(temp.path().join("retained")).expect("read retained file"),
            b"retained"
        );
    }

    #[test]
    fn rooted_child_enumeration_is_deterministically_sorted() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        fs::write(temp.path().join("zeta"), b"z").expect("seed zeta");
        fs::write(temp.path().join("alpha"), b"a").expect("seed alpha");
        fs::write(temp.path().join("middle"), b"m").expect("seed middle");

        assert_eq!(
            root.child_names().expect("enumerate retained directory"),
            ["alpha", "middle", "zeta"].map(OsString::from)
        );
    }

    #[test]
    fn rooted_child_enumeration_rejects_bound_overflow() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        for name in ["one", "two", "three"] {
            fs::write(temp.path().join(name), name.as_bytes()).expect("seed bounded child");
        }

        let error = root
            .child_names_bounded(2)
            .expect_err("enumeration overflow must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn rooted_empty_directory_binding_removes_empty_child() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        fs::create_dir(temp.path().join("orphan")).expect("seed empty orphan directory");
        let retained = root
            .open_directory(OsStr::new("orphan"))
            .expect("retain empty orphan directory");

        root.remove_empty_directory_binding(retained)
            .expect("remove exact empty orphan");
        assert!(!temp.path().join("orphan").exists());
    }

    #[test]
    fn rooted_empty_directory_removal_preserves_a_child_created_at_the_destructive_gap() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let retained = temp.path().join("orphan");
        fs::create_dir(&retained).expect("seed empty orphan directory");
        let binding = root
            .open_directory(OsStr::new("orphan"))
            .expect("retain empty orphan directory");

        let error = root
            .remove_empty_directory_binding_with(binding, || {
                fs::write(retained.join("racing-child"), b"preserve me")
            })
            .expect_err("a child created after the emptiness check must block removal");
        assert_eq!(error.kind(), io::ErrorKind::DirectoryNotEmpty);
        assert_eq!(
            fs::read(retained.join("racing-child")).expect("racing child remains"),
            b"preserve me"
        );
    }

    #[test]
    fn rooted_exact_file_removal_preserves_a_name_substitution() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let target = temp.path().join("orphan");
        let original = temp.path().join("original");
        fs::write(&target, b"planned-orphan").expect("seed planned orphan");
        let binding = root
            .removal_file_binding(OsStr::new("orphan"), 64)
            .expect("retain planned orphan")
            .expect("planned orphan exists");
        fs::rename(&target, &original).expect("detach planned orphan");
        fs::write(&target, b"replacement").expect("install replacement");

        root.remove_file_binding(binding)
            .expect_err("exact removal must reject a substituted name");
        assert_eq!(
            fs::read(&target).expect("replacement remains"),
            b"replacement"
        );
        assert_eq!(
            fs::read(&original).expect("planned orphan remains detached"),
            b"planned-orphan"
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rooted_private_removal_binding_enforces_private_file_policy() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let target = temp.path().join("private-orphan");
        fs::write(&target, b"private recovery state").expect("seed private orphan");
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600))
            .expect("secure private orphan mode");

        let binding = root
            .private_removal_file_binding(OsStr::new("private-orphan"), 64)
            .expect("retain private orphan")
            .expect("private orphan exists");
        root.remove_file_binding(binding)
            .expect("remove exact private orphan");
        assert!(!target.exists());

        fs::write(&target, b"exposed recovery state").expect("seed exposed orphan");
        fs::set_permissions(&target, fs::Permissions::from_mode(0o644))
            .expect("set exposed orphan mode");
        let error = root
            .private_removal_file_binding(OsStr::new("private-orphan"), 64)
            .expect_err("non-private recovery state must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn rooted_exact_directory_removal_preserves_a_name_substitution() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let target = temp.path().join("orphan");
        let original = temp.path().join("original");
        fs::create_dir(&target).expect("seed planned directory");
        let retained = root
            .open_directory(OsStr::new("orphan"))
            .expect("retain planned directory");
        fs::rename(&target, &original).expect("detach planned directory");
        fs::create_dir(&target).expect("install replacement directory");

        root.remove_empty_directory_binding(retained)
            .expect_err("exact removal must reject a substituted directory name");
        assert!(target.is_dir(), "replacement directory must remain");
        assert!(original.is_dir(), "planned directory must remain detached");
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rooted_file_isolation_preserves_a_replacement_installed_at_the_destructive_gap() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let quarantine = root
            .open_or_create_directory(OsStr::new(".quarantine"))
            .expect("create retained quarantine");
        let target = temp.path().join("orphan");
        let detached = temp.path().join("detached");
        fs::write(&target, b"planned-orphan").expect("seed planned orphan");
        let binding = root
            .removal_file_binding(OsStr::new("orphan"), 64)
            .expect("retain planned orphan")
            .expect("planned orphan exists");

        root.isolate_file_binding_with(binding, &quarantine, OsStr::new("file-slot"), || {
            fs::rename(&target, &detached).expect("detach checked inode in race hook");
            fs::write(&target, b"replacement").expect("install racing replacement");
            Ok(())
        })
        .expect_err("post-check name substitution must fail after preserving both files");
        assert_eq!(
            fs::read(&detached).expect("checked inode remains detached"),
            b"planned-orphan"
        );
        assert_eq!(
            fs::read(temp.path().join(".quarantine").join("file-slot"))
                .expect("replacement remains quarantined"),
            b"replacement"
        );
        assert!(
            !target.exists(),
            "the raced name was isolated, not unlinked"
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rooted_file_isolation_never_overwrites_a_prepopulated_destination() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let quarantine = root
            .open_or_create_directory(OsStr::new(".quarantine"))
            .expect("create quarantine");
        fs::write(temp.path().join("orphan"), b"planned-orphan").expect("seed source");
        let binding = root
            .removal_file_binding(OsStr::new("orphan"), 64)
            .expect("retain source")
            .expect("source exists");

        root.isolate_file_binding_with(binding, &quarantine, OsStr::new("file-slot"), || {
            fs::write(
                temp.path().join(".quarantine").join("file-slot"),
                b"prepopulated",
            )
            .expect("prepopulate destination slot");
            Ok(())
        })
        .expect_err("exclusive isolation must reject a populated destination");
        assert_eq!(
            fs::read(temp.path().join("orphan")).expect("read unchanged source"),
            b"planned-orphan"
        );
        assert_eq!(
            fs::read(temp.path().join(".quarantine").join("file-slot")).expect("read destination"),
            b"prepopulated"
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rooted_file_isolation_attempts_both_parent_syncs_when_source_sync_fails() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let quarantine = root
            .open_or_create_directory(OsStr::new(".quarantine"))
            .expect("create quarantine");
        fs::write(temp.path().join("orphan"), b"planned-orphan").expect("seed source");
        let binding = root
            .removal_file_binding(OsStr::new("orphan"), 64)
            .expect("retain source")
            .expect("source exists");
        let source_syncs = Cell::new(0_usize);
        let quarantine_syncs = Cell::new(0_usize);

        let error = root
            .isolate_file_binding_with_sync(
                binding,
                &quarantine,
                OsStr::new("file-slot"),
                || Ok(()),
                |_directory| {
                    source_syncs.set(source_syncs.get() + 1);
                    Err(io::Error::other("injected source-parent sync failure"))
                },
                |_directory| {
                    quarantine_syncs.set(quarantine_syncs.get() + 1);
                    Ok(())
                },
            )
            .expect_err("source-parent sync failure must propagate");
        assert!(error.to_string().contains("source-parent sync failure"));
        assert_eq!(source_syncs.get(), 1);
        assert_eq!(quarantine_syncs.get(), 1);
        assert!(!temp.path().join("orphan").exists());
        assert_eq!(
            fs::read(temp.path().join(".quarantine").join("file-slot"))
                .expect("read preserved quarantined source"),
            b"planned-orphan"
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rooted_file_isolation_propagates_quarantine_parent_sync_failure() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let quarantine = root
            .open_or_create_directory(OsStr::new(".quarantine"))
            .expect("create quarantine");
        fs::write(temp.path().join("orphan"), b"planned-orphan").expect("seed source");
        let binding = root
            .removal_file_binding(OsStr::new("orphan"), 64)
            .expect("retain source")
            .expect("source exists");
        let source_syncs = Cell::new(0_usize);
        let quarantine_syncs = Cell::new(0_usize);

        let error = root
            .isolate_file_binding_with_sync(
                binding,
                &quarantine,
                OsStr::new("file-slot"),
                || Ok(()),
                |_directory| {
                    source_syncs.set(source_syncs.get() + 1);
                    Ok(())
                },
                |_directory| {
                    quarantine_syncs.set(quarantine_syncs.get() + 1);
                    Err(io::Error::other("injected quarantine-parent sync failure"))
                },
            )
            .expect_err("quarantine-parent sync failure must propagate");
        assert!(error.to_string().contains("quarantine-parent sync failure"));
        assert_eq!(source_syncs.get(), 1);
        assert_eq!(quarantine_syncs.get(), 1);
        assert!(!temp.path().join("orphan").exists());
        assert_eq!(
            fs::read(temp.path().join(".quarantine").join("file-slot"))
                .expect("read preserved quarantined source"),
            b"planned-orphan"
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rooted_directory_isolation_preserves_a_replacement_installed_at_the_destructive_gap() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let quarantine = root
            .open_or_create_directory(OsStr::new(".quarantine"))
            .expect("create retained quarantine");
        let target = temp.path().join("orphan");
        let detached = temp.path().join("detached");
        fs::create_dir(&target).expect("seed planned directory");
        let retained = root
            .open_directory(OsStr::new("orphan"))
            .expect("retain planned directory");

        root.isolate_empty_directory_binding_with(
            retained,
            &quarantine,
            OsStr::new("directory-slot"),
            || {
                fs::rename(&target, &detached).expect("detach checked directory in race hook");
                fs::create_dir(&target).expect("install racing replacement directory");
                Ok(())
            },
        )
        .expect_err("post-check directory substitution must preserve both directories");
        assert!(detached.is_dir(), "checked directory remains detached");
        assert!(
            temp.path()
                .join(".quarantine")
                .join("directory-slot")
                .is_dir(),
            "replacement directory remains quarantined"
        );
        assert!(!target.exists(), "the raced directory was never unlinked");
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rooted_directory_isolation_attempts_both_parent_syncs_when_source_sync_fails() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let quarantine = root
            .open_or_create_directory(OsStr::new(".quarantine"))
            .expect("create quarantine");
        fs::create_dir(temp.path().join("orphan")).expect("seed source directory");
        let child = root
            .open_directory(OsStr::new("orphan"))
            .expect("retain source directory");
        let source_syncs = Cell::new(0_usize);
        let quarantine_syncs = Cell::new(0_usize);

        let error = root
            .isolate_empty_directory_binding_with_sync(
                child,
                &quarantine,
                OsStr::new("directory-slot"),
                || Ok(()),
                |_directory| {
                    source_syncs.set(source_syncs.get() + 1);
                    Err(io::Error::other("injected directory-source sync failure"))
                },
                |_directory| {
                    quarantine_syncs.set(quarantine_syncs.get() + 1);
                    Ok(())
                },
            )
            .expect_err("directory source-parent sync failure must propagate");
        assert!(error.to_string().contains("directory-source sync failure"));
        assert_eq!(source_syncs.get(), 1);
        assert_eq!(quarantine_syncs.get(), 1);
        assert!(!temp.path().join("orphan").exists());
        assert!(
            temp.path()
                .join(".quarantine")
                .join("directory-slot")
                .is_dir()
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rooted_directory_isolation_propagates_quarantine_parent_sync_failure() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let quarantine = root
            .open_or_create_directory(OsStr::new(".quarantine"))
            .expect("create quarantine");
        fs::create_dir(temp.path().join("orphan")).expect("seed source directory");
        let child = root
            .open_directory(OsStr::new("orphan"))
            .expect("retain source directory");
        let source_syncs = Cell::new(0_usize);
        let quarantine_syncs = Cell::new(0_usize);

        let error = root
            .isolate_empty_directory_binding_with_sync(
                child,
                &quarantine,
                OsStr::new("directory-slot"),
                || Ok(()),
                |_directory| {
                    source_syncs.set(source_syncs.get() + 1);
                    Ok(())
                },
                |_directory| {
                    quarantine_syncs.set(quarantine_syncs.get() + 1);
                    Err(io::Error::other(
                        "injected directory-quarantine sync failure",
                    ))
                },
            )
            .expect_err("directory quarantine-parent sync failure must propagate");
        assert!(
            error
                .to_string()
                .contains("directory-quarantine sync failure")
        );
        assert_eq!(source_syncs.get(), 1);
        assert_eq!(quarantine_syncs.get(), 1);
        assert!(!temp.path().join("orphan").exists());
        assert!(
            temp.path()
                .join(".quarantine")
                .join("directory-slot")
                .is_dir()
        );
    }

    #[test]
    fn rooted_empty_directory_removal_rejects_nonempty_children() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let retained = temp.path().join("retained");
        fs::create_dir(&retained).expect("seed retained directory");
        fs::write(retained.join("state"), b"retained").expect("seed retained child");
        let retained_binding = root
            .open_directory(OsStr::new("retained"))
            .expect("retain nonempty directory");

        root.remove_empty_directory_binding(retained_binding)
            .expect_err("nonempty retained directory must not be removed");
        assert_eq!(
            fs::read(retained.join("state")).expect("read retained child"),
            b"retained"
        );
    }

    #[test]
    fn rooted_read_enforces_its_byte_bound() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        fs::write(temp.path().join("state"), b"12345").expect("seed bounded state");

        let exact = root
            .read_file(OsStr::new("state"), 5)
            .expect("read at exact byte bound");
        assert_eq!(exact.bytes(), b"12345");
        let error = root
            .read_file(OsStr::new("state"), 4)
            .expect_err("oversized state must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[cfg(windows)]
    #[test]
    fn rooted_read_rejects_windows_hardlinks() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let state = temp.path().join("state");
        fs::write(&state, b"linked-state").expect("seed Windows state");
        fs::hard_link(&state, temp.path().join("state-link"))
            .expect("create Windows governance hardlink");

        let error = root
            .read_file(OsStr::new("state"), 32)
            .expect_err("Windows governance files with multiple links must fail closed");
        assert!(error.to_string().contains("exactly one hard link"));
    }

    #[test]
    fn retained_private_file_rejects_name_substitution() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let retained = root
            .open_or_create_private_file(OsStr::new(".service.lock"), 4096)
            .expect("retain private file");
        fs::rename(
            temp.path().join(".service.lock"),
            temp.path().join("original.lock"),
        )
        .expect("detach retained file");
        fs::write(temp.path().join(".service.lock"), b"replacement")
            .expect("install name replacement");
        #[cfg(unix)]
        fs::set_permissions(
            temp.path().join(".service.lock"),
            fs::Permissions::from_mode(0o600),
        )
        .expect("secure replacement mode");

        let error = retained
            .verify()
            .expect_err("retained file substitution must fail closed");
        assert!(error.to_string().contains("substituted"));
        assert_eq!(
            fs::read(temp.path().join(".service.lock")).expect("read replacement"),
            b"replacement"
        );
    }

    #[test]
    fn rooted_recovery_is_idempotent_across_restart() {
        let temp = tempdir().expect("tempdir");
        fs::write(temp.path().join(".state.tmp-42000-7"), b"crash").expect("seed crash temporary");
        {
            let first = test_root(temp.path());
            assert_eq!(
                first
                    .remove_atomic_temps_for("state")
                    .expect("first restart recovery"),
                1
            );
        }
        let restarted = test_root(temp.path());
        assert_eq!(
            restarted
                .remove_atomic_temps_for("state")
                .expect("second restart recovery"),
            0
        );
        restarted
            .atomic_replace_current(
                OsStr::new("state"),
                OsStr::new(".state.tmp-42000-8"),
                b"restarted",
            )
            .expect("write after restart recovery");
        assert_eq!(
            fs::read(temp.path().join("state")).expect("read restarted state"),
            b"restarted"
        );
    }

    #[cfg(windows)]
    #[test]
    fn rooted_file_open_rejects_reparse_point() {
        let temp = tempdir().expect("tempdir");
        fs::write(temp.path().join("target"), b"target").expect("seed target");
        std::os::windows::fs::symlink_file(temp.path().join("target"), temp.path().join("linked"))
            .expect("create Windows file symlink");
        let root = test_root(temp.path());
        root.read_file(OsStr::new("linked"), 16)
            .expect_err("reparse-backed file must fail closed");
    }

    #[cfg(windows)]
    #[test]
    fn windows_disposition_deletes_the_opened_object_after_name_replacement() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let name = OsStr::new(".state.tmp-42000-2");
        let stale = temp.path().join(name);
        let moved = temp.path().join("opened-stale-object");
        fs::write(&stale, b"opened-object").expect("seed stale object");
        let opened =
            super::platform::open_file(&root.handle, name, true).expect("open exact stale object");
        let identity = super::file_identity(&opened.metadata().expect("inspect opened object"))
            .expect("capture Windows file identity");
        fs::rename(&stale, &moved).expect("move opened stale object");
        fs::write(&stale, b"name-replacement").expect("replace stale pathname");

        super::platform::remove_open_file(&root.handle, &opened, name, Some(identity))
            .expect("mark exact opened object for deletion");
        drop(opened);

        assert!(!moved.exists(), "the opened stale object must be deleted");
        assert_eq!(
            fs::read(&stale).expect("read replacement"),
            b"name-replacement",
            "a later pathname replacement must remain untouched"
        );
    }
}
