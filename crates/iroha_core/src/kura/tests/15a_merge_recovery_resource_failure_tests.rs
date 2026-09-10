// Actual failed merge append and subsequent reader/preflight tail recovery.

thread_local! {
    static FAIL_MERGE_TAIL_RECOVERY_FOR_RESOURCES: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub(super) fn fail_merge_tail_recovery_for_resource_tests() -> Result<()> {
    if FAIL_MERGE_TAIL_RECOVERY_FOR_RESOURCES.with(|flag| flag.replace(false)) {
        return Err(Error::IO(
            std::io::Error::other("injected merge tail-recovery failure before truncate"),
            PathBuf::from("merge_tail_recovery_resource_test"),
        ));
    }
    Ok(())
}

#[test]
fn failed_merge_append_reader_repair_invalidates_every_physical_family_and_stale_audit() {
    for reader in ["hash", "preflight", "all", "length", "identities"] {
        let (_directory, kura) = metadata_physical_fixture();
        let first = sample_merge_entry(1);
        let second = sample_merge_entry(2);
        kura.append_merge_entry_for_test(&first).unwrap();
        let baseline = metadata_physical_register(&kura);
        let path = kura.active_merge_path.lock().clone();
        let original = fs::read(&path).unwrap();
        kura.fail_next_merge_append_after_for_test(MergeLedgerAppendFailurePoint::AfterLength);
        FAIL_MERGE_TAIL_RECOVERY_FOR_RESOURCES.with(|flag| flag.set(true));
        assert!(kura.append_merge_entry_for_test(&second).is_err());
        assert!(!FAIL_MERGE_TAIL_RECOVERY_FOR_RESOURCES.with(std::cell::Cell::get));
        assert_eq!(
            fs::metadata(&path).unwrap().len(),
            original.len() as u64 + 4
        );
        {
            let log = kura.merge_log.lock();
            assert_eq!(log.append_recovery_offset, Some(original.len() as u64));
            assert!(!log.resident_inventory_valid);
            assert_eq!(log.total_entries, 1);
        }
        physical_guard_assert_unavailable(&kura);
        // A physical audit can count the failed tail without authenticating the
        // unresolved resident log. Its later repair must invalidate that audit.
        let failed_tail = metadata_physical_register(&kura);
        assert_eq!(
            failed_tail[ResourceFamily::StorageBytes as usize].storage_bytes,
            baseline[ResourceFamily::StorageBytes as usize].storage_bytes + 4
        );
        assert!(kura.resource_inventory_snapshot().is_err());
        let generation = kura.resource_inventory.reconciliation_generation().unwrap();
        match reader {
            "hash" => assert_eq!(
                kura.merge_entry_by_hash(first.canonical_hash()).unwrap(),
                Some(first.clone())
            ),
            "preflight" => assert!(kura.merge_log.lock().preflight_append(&second).unwrap()),
            "all" => assert_eq!(
                kura.merge_ledger_all_entries().unwrap(),
                vec![first.clone()]
            ),
            "length" => kura
                .merge_log
                .lock()
                .validate_indexed_file_length()
                .unwrap(),
            "identities" => assert!(
                kura.merge_log
                    .lock()
                    .execution_entries_for_bounded_identities(&BTreeSet::new())
                    .unwrap()
                    .is_empty()
            ),
            _ => unreachable!(),
        }
        assert_eq!(fs::read(&path).unwrap(), original, "{reader}");
        {
            let log = kura.merge_log.lock();
            assert!(log.append_recovery_offset.is_none());
            assert!(
                !log.resident_inventory_valid,
                "repair cannot authenticate the failed owner"
            );
        }
        for family in PHYSICAL_RESOURCE_FAMILIES {
            assert_eq!(
                kura.resource_inventory.component_usage_for_tests(family),
                Err(resource_inventory::Unavailable::InvalidInventory),
                "{reader}: {family:?}"
            );
        }
        assert!(kura.resource_inventory.reconciliation_generation().unwrap() > generation);
        let stale = PHYSICAL_RESOURCE_FAMILIES
            .iter()
            .map(|family| (*family, failed_tail[*family as usize]))
            .collect::<Vec<_>>();
        assert_eq!(
            kura.resource_inventory.initialize(generation, &stale),
            Err(resource_inventory::Unavailable::GenerationChanged)
        );
        assert_eq!(metadata_physical_register(&kura), baseline);
        assert!(kura.reconcile_resident_resource_inventory().is_err());
        assert!(kura.resource_inventory_snapshot().is_err());
    }
}
