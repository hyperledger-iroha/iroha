#[test]
fn canonical_association_stage_append_peak_rejects_before_any_stage_or_block_mutation() {
    let temp_dir = TempDir::new().expect("association append capacity temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (mut kura, _) =
        Kura::new(&config, &RuntimeLaneConfig::default()).expect("association append Kura");
    let block = DummyBlocks::new().next();
    let stage_bytes = kura
        .canonical_association_stage_additional_bytes(block.as_ref(), None)
        .expect("encode exact association stage");
    let block_bytes = kura
        .block_required_bytes_for_budget(block.as_ref(), None, u64::MAX)
        .expect("account candidate block");
    let used = kura
        .refresh_disk_usage_bytes()
        .expect("measure append baseline");
    let (persisted_count, unindexed_bytes) = kura
        .persisted_count_and_unindexed_bytes()
        .expect("measure append durable frontier");
    let pending = kura
        .pending_block_bytes(persisted_count, unindexed_bytes)
        .expect("measure pending blocks");
    let terminal = kura
        .autonomous_global_terminal_outcome_reserved_bytes()
        .expect("measure terminal reservations");
    let post_wsv = kura
        .post_wsv_lane_artifact_budget_reserved_bytes()
        .expect("measure post-WSV reservations");
    let certified = kura
        .certified_bundle_capacity_reserved_bytes()
        .expect("measure certified-bundle reservations");
    let exact_peak = used
        .checked_add(pending)
        .and_then(|bytes| bytes.checked_add(terminal))
        .and_then(|bytes| bytes.checked_add(post_wsv))
        .and_then(|bytes| bytes.checked_add(certified))
        .and_then(|bytes| {
            bytes.checked_add(Kura::canonical_prune_intent_maintenance_headroom_bytes())
        })
        .and_then(|bytes| bytes.checked_add(block_bytes))
        .and_then(|bytes| bytes.checked_add(stage_bytes))
        .expect("append association peak fits u64");
    Arc::get_mut(&mut kura)
        .expect("exclusive append Kura")
        .max_disk_usage_bytes = exact_peak - 1;
    let error = kura
        .store_block(Arc::clone(&block))
        .expect_err("one byte below association overlap must reject");
    assert!(matches!(
        error,
        Error::StorageBudgetExceeded {
            limit,
            required,
            ..
        } if limit == exact_peak - 1 && required == exact_peak
    ));
    assert!(
        !kura.canonical_association_stage_path().exists(),
        "capacity rejection must precede association-stage publication",
    );
    assert_eq!(kura.exact_durable_blocks_count().unwrap(), 0);
    assert_eq!(kura.blocks_count(), 0);
    Arc::get_mut(&mut kura)
        .expect("exclusive append Kura after rejection")
        .max_disk_usage_bytes = exact_peak;
    kura.store_block(block)
        .expect("the exact association overlap must admit");
    assert!(!kura.canonical_association_stage_path().exists());
    assert_eq!(kura.exact_durable_blocks_count().unwrap(), 1);
}
#[test]
fn canonical_association_stage_replace_peak_keeps_old_top_untouched_on_rejection() {
    let temp_dir = TempDir::new().expect("association replace capacity temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (mut kura, _) =
        Kura::new(&config, &RuntimeLaneConfig::default()).expect("association replace Kura");
    let original = DummyBlocks::new().next();
    let original_hash = original.hash();
    kura.store_block(original.clone())
        .expect("store original top");
    let replacement: Arc<SignedBlock> = Arc::new(
        ValidBlock::new_dummy_and_modify_header(checked_keypair().private_key(), |header| {
            header.set_height(nonzero!(1_u64));
            header.set_prev_block_hash(None);
            header.set_view_change_index(header.view_change_index().saturating_add(1));
        })
        .into(),
    );
    assert_ne!(replacement.hash(), original_hash);
    let limit_probe = u64::MAX;
    let old_bytes = kura
        .block_required_bytes_for_budget(original.as_ref(), None, limit_probe)
        .expect("account old top");
    let new_bytes = kura
        .block_required_bytes_for_budget(replacement.as_ref(), None, limit_probe)
        .expect("account replacement top");
    let stage_bytes = kura
        .canonical_association_stage_additional_bytes(replacement.as_ref(), None)
        .expect("encode replacement association stage");
    let used = kura
        .refresh_disk_usage_bytes()
        .expect("measure replacement baseline");
    let (persisted_count, unindexed_bytes) = kura
        .persisted_count_and_unindexed_bytes()
        .expect("measure replacement durable frontier");
    let pending_raw = kura
        .pending_block_bytes_raw(persisted_count)
        .expect("measure raw pending replacement bytes");
    let pending_current = pending_raw.saturating_sub(unindexed_bytes);
    let terminal = kura
        .autonomous_global_terminal_outcome_reserved_bytes()
        .expect("measure terminal reservations");
    let post_wsv = kura
        .post_wsv_lane_artifact_budget_reserved_bytes()
        .expect("measure post-WSV reservations");
    let certified = kura
        .certified_bundle_capacity_reserved_bytes()
        .expect("measure certified-bundle reservations");
    let before_stage = used
        .checked_add(pending_current)
        .and_then(|bytes| bytes.checked_add(terminal))
        .and_then(|bytes| bytes.checked_add(post_wsv))
        .and_then(|bytes| bytes.checked_add(certified))
        .and_then(|bytes| {
            bytes.checked_add(Kura::canonical_prune_intent_maintenance_headroom_bytes())
        })
        .expect("replacement pre-stage state fits u64");
    let projected_after = used
        .saturating_sub(old_bytes)
        .checked_add(
            pending_raw
                .saturating_add(new_bytes)
                .saturating_sub(unindexed_bytes),
        )
        .and_then(|bytes| bytes.checked_add(terminal))
        .and_then(|bytes| bytes.checked_add(post_wsv))
        .and_then(|bytes| bytes.checked_add(certified))
        .and_then(|bytes| {
            bytes.checked_add(Kura::canonical_prune_intent_maintenance_headroom_bytes())
        })
        .expect("replacement projected state fits u64");
    let exact_peak = before_stage
        .max(projected_after)
        .checked_add(stage_bytes)
        .expect("replacement association peak fits u64");
    Arc::get_mut(&mut kura)
        .expect("exclusive replacement Kura")
        .max_disk_usage_bytes = exact_peak - 1;
    let error = kura
        .replace_top_block(replacement)
        .expect_err("one byte below replacement association overlap must reject");
    assert!(matches!(
        error,
        Error::StorageBudgetExceeded {
            limit,
            required,
            ..
        } if limit == exact_peak - 1 && required == exact_peak
    ));
    assert!(!kura.canonical_association_stage_path().exists());
    assert_eq!(
        kura.get_durable_block_hash(nonzero!(1_usize)),
        Some(original_hash),
        "rejected replacement must not change the canonical marker",
    );
    assert_eq!(
        kura.get_block(nonzero!(1_usize)).as_deref(),
        Some(original.as_ref())
    );
}
#[test]
fn post_marker_association_failure_poison_gates_and_restart_completes_stage() {
    let lane_id = LaneId::SINGLE;
    let lane_block_height = 1;
    let temp_dir = TempDir::new().expect("create Kura root");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let block_hash = {
        let (kura, _) = test_kura_with_default_lane_markers(&config, &RuntimeLaneConfig::default());
        let block = dummy_block_with_lane_payload_ownership(
            lane_id,
            DataSpaceId::UNIVERSAL,
            lane_block_height,
        );
        let block_hash = block.hash();
        kura.fail_next_canonical_association_recovery
            .store(true, Ordering::Release);
        let error = kura
            .store_block(block)
            .expect_err("post-marker association fault must require restart");
        assert!(matches!(
            error,
            Error::CanonicalBlockCommittedRecoveryRequired { .. }
        ));
        assert!(error.requires_restart_recovery());
        assert!(kura.canonical_storage_poisoned.load(Ordering::Acquire));
        assert_eq!(kura.block_data.lock().len(), 1);
        assert_eq!(
            Kura::read_durable_hash_at_height(&mut kura.block_store.lock(), 1)
                .expect("read committed block while poisoned"),
            Some(block_hash)
        );
        assert!(kura.canonical_association_stage_path().is_file());
        assert!(
            kura.read_lane_block_artifact(lane_id, lane_block_height)
                .is_none(),
            "failed association must not fabricate a completed lane binding"
        );
        assert!(matches!(
            kura.store_block(DummyBlocks::new().next()),
            Err(Error::CanonicalStoragePoisoned)
        ));
        block_hash
    };
    let (reopened, count) = Kura::new(&config, &RuntimeLaneConfig::default())
        .expect("restart must finish the committed association stage");
    assert_eq!(count.0, 1);
    assert_eq!(
        reopened.get_durable_block_hash(nonzero!(1_usize)),
        Some(block_hash)
    );
    assert!(
        reopened
            .read_lane_block_artifact(lane_id, lane_block_height)
            .is_some()
    );
    assert!(!reopened.canonical_association_stage_path().exists());
    assert!(!reopened.canonical_storage_poisoned.load(Ordering::Acquire));
}
