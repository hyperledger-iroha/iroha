// Deterministic incumbent installation at the real retained no-clobber boundary.

std::thread_local! {
    static RETAINED_RECORD_PRE_NOCLOBBER_HOOK: std::cell::RefCell<Option<Box<dyn FnOnce(&Path)>>> =
        const { std::cell::RefCell::new(None) };
}

struct RetainedRecordNoClobberHookGuard;

impl Drop for RetainedRecordNoClobberHookGuard {
    fn drop(&mut self) {
        RETAINED_RECORD_PRE_NOCLOBBER_HOOK.with(|slot| slot.borrow_mut().take());
    }
}

fn install_retained_record_pre_noclobber_hook_for_test(
    hook: impl FnOnce(&Path) + 'static,
) -> RetainedRecordNoClobberHookGuard {
    RETAINED_RECORD_PRE_NOCLOBBER_HOOK.with(|slot| {
        assert!(slot.borrow_mut().replace(Box::new(hook)).is_none());
    });
    RetainedRecordNoClobberHookGuard
}

/// Run this thread's one-shot incumbent installer at the actual publication boundary.
pub(super) fn run_retained_record_pre_noclobber_hook_for_test(path: &Path) {
    let hook = RETAINED_RECORD_PRE_NOCLOBBER_HOOK.with(|slot| slot.borrow_mut().take());
    if let Some(hook) = hook {
        hook(path);
    }
}

#[test]
fn retained_resource_identical_noclobber_race_finishes_and_conflict_stays_unavailable() {
    for identical in [true, false] {
        let directory = TempDir::new().unwrap();
        let config = kura_config_for_dir(&directory, BLOCKS_IN_MEMORY);
        let (kura, _) = Kura::open_test_kura_with_configured_lane_config(
            &config,
            &RuntimeLaneConfig::default(),
        )
        .unwrap();
        let block = store_dummy_block_arcs(&kura, 1).remove(0);
        let blocks_dir = kura.active_blocks_dir.lock().clone();
        let requested =
            Kura::prepare_retained_block_record(&blocks_dir, block.hash(), block.as_ref()).unwrap();
        let path = kura.retained_block_record_path(1);
        assert!(!path.exists());
        let mut incumbent = requested.clone();
        if !identical {
            incumbent.proposal_wire_hash = Hash::new(b"conflicting retained race proposal wire");
            assert_ne!(incumbent, requested);
        }
        // The negative reaches incumbent equality after the original structural
        // and canonical-header checks, rather than failing the record decoder.
        assert!(
            Kura::validate_retained_block_record_at(&path, 1, block.hash(), &incumbent).is_ok()
        );
        let incumbent_bytes = incumbent.canonical_storage_bytes();
        let expected_bytes = incumbent_bytes.clone();
        let before = initialize_retained_physical_fixture(&kura);
        kura.refresh_total_disk_usage_bytes().unwrap();
        assert!(kura.disk_usage_total_initialized.load(Ordering::Acquire));
        let called = std::rc::Rc::new(std::cell::Cell::new(false));
        let callback_called = std::rc::Rc::clone(&called);
        let observed_kura = Arc::clone(&kura);
        let _hook = install_retained_record_pre_noclobber_hook_for_test(move |path| {
            assert!(!path.exists());
            assert_eq!(
                observed_kura
                    .resource_inventory
                    .component_usage_for_tests(ResourceFamily::StorageBytes)
                    .unwrap_err(),
                resource_inventory::Unavailable::Busy,
            );
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .unwrap();
            file.write_all(&incumbent_bytes).unwrap();
            file.sync_all().unwrap();
            sync_dir(path.parent().unwrap()).unwrap();
            callback_called.set(true);
        });
        let _prune_guard = kura.prune_lock.lock();
        let _canonical_guard = kura.canonical_chain_lock.lock();
        let _sidecar_guard = kura.sidecar_lock.lock();
        let result =
            kura.persist_prepared_retained_block_record(&blocks_dir, block.hash(), &requested);
        assert!(
            called.get(),
            "the real post-preflight no-clobber race boundary was reached"
        );
        assert_eq!(fs::read(&path).unwrap(), expected_bytes);
        assert!(!kura.disk_usage_total_initialized.load(Ordering::Acquire));
        if identical {
            result.unwrap();
            let after = assert_retained_physical_fixture(&kura);
            assert_eq!(
                after[ResourceFamily::EvidenceKeyRecords as usize].persisted_entries,
                before[ResourceFamily::EvidenceKeyRecords as usize].persisted_entries + 1,
            );
            assert_eq!(
                after[ResourceFamily::StorageBytes as usize].storage_bytes,
                before[ResourceFamily::StorageBytes as usize].storage_bytes
                    + u64::try_from(expected_bytes.len()).unwrap(),
            );
            kura.persist_prepared_retained_block_record(&blocks_dir, block.hash(), &requested)
                .unwrap();
            assert_eq!(assert_retained_physical_fixture(&kura), after);
        } else {
            assert!(matches!(
                result,
                Err(Error::ConflictingRetainedBlockRecord { height: 1 })
            ));
            assert_retained_physical_unavailable(&kura);
        }
    }
}
