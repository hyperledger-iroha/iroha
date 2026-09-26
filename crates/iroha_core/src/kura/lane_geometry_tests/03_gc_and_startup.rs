#[test]
fn geometry_gc_crash_boundaries_replay_safely_after_restart() {
    for stage in [
        GC_FAIL_AFTER_COMPACTION_INTENT,
        GC_FAIL_AFTER_ARCHIVE_QUARANTINE,
        GC_FAIL_AFTER_ARCHIVE_DELETION,
        GC_FAIL_AFTER_COMPLETION,
    ] {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join(format!("kura-stage-{stage}"));
        let (initial, _) = initial_and_extended_configs();
        let (initial_incarnations, _) = initial_geometry();
        let kura = open_kura(&root, &initial);
        // Bind the actual configured primary before retaining geometry or bodies for restart.
        kura.establish_or_verify_configured_primary_geometry_anchor(
            initial.primary(),
            initial_incarnations[&LaneId::SINGLE],
            kura.configured_lane_catalog_baseline()
                .expect("read authenticated configured catalog")
                .expect("configured catalog was admitted at open"),
        )
        .expect("anchor configured primary before restart fixture writes");
        let fixture = prepare_retired_geometry_archive(&kura, &root);
        let transition_roots = kura
            .read_lane_geometry_journal()
            .expect("journal before GC")
            .records
            .iter()
            .map(|record| {
                root.join("retired/lane_geometry")
                    .join(hex::encode(record.transition_id.as_ref()))
            })
            .collect::<Vec<_>>();
        assert_eq!(transition_roots.len(), 2);
        assert!(transition_roots.iter().all(|archive| !archive.exists()));
        assert!(
            fixture.retained_blocks.is_dir(),
            "Apply retains the immutable instance in place"
        );
        // Only the retirement collects an unreferenced instance; creation has no archive.
        let collected_archive = &transition_roots[1];
        let quarantine = collected_archive
            .parent()
            .expect("archive parent")
            .join(format!(
                "{GC_QUARANTINE_PREFIX}{}",
                collected_archive
                    .file_name()
                    .expect("transition id")
                    .to_string_lossy()
            ));
        kura.fail_next_lane_geometry_gc_at_stage_for_test(stage);
        checkpoint_retired_geometry(&kura, &fixture, 20)
            .expect_err("injected GC boundary must interrupt acknowledgement");
        let after_failure = kura
            .read_lane_geometry_journal()
            .expect("journal after crash");
        assert!(after_failure.records.is_empty());
        if stage == GC_FAIL_AFTER_COMPACTION_INTENT {
            assert!(transition_roots.iter().all(|archive| !archive.exists()));
            assert!(fixture.retained_blocks.is_dir());
            assert!(!quarantine.exists());
            assert!(!after_failure.pending_archive_gc.is_empty());
        } else if stage == GC_FAIL_AFTER_ARCHIVE_QUARANTINE {
            assert!(!collected_archive.exists());
            assert!(!fixture.retained_blocks.exists());
            assert!(quarantine.exists());
            assert!(!after_failure.pending_archive_gc.is_empty());
        } else if stage == GC_FAIL_AFTER_ARCHIVE_DELETION {
            assert!(transition_roots.iter().all(|archive| !archive.exists()));
            assert!(!quarantine.exists());
            assert!(!after_failure.pending_archive_gc.is_empty());
        } else {
            assert!(transition_roots.iter().all(|archive| !archive.exists()));
            assert!(!quarantine.exists());
            assert!(after_failure.pending_archive_gc.is_empty());
        }
        if stage != GC_FAIL_AFTER_COMPACTION_INTENT {
            assert!(!fixture.retained_blocks.exists());
        }
        drop(kura);
        let restarted = open_kura(&root, &fixture.initial);
        restarted
            .recover_lane_geometry_journal(
                &fixture.initial,
                &fixture.initial_incarnations,
                &fixture.initial_activations,
            )
            .expect("restart completes or observes completed GC");
        let recovered = restarted
            .read_lane_geometry_journal()
            .expect("recovered journal");
        assert!(recovered.pending_archive_gc.is_empty());
        assert!(!fixture.retained_blocks.exists());
        assert!(transition_roots.iter().all(|archive| !archive.exists()));
        assert!(!quarantine.exists());
    }
}
#[test]
fn storage_budget_purge_only_resumes_snapshot_proven_geometry_gc() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let kura = open_kura(&root, &initial_and_extended_configs().0);
    let fixture = prepare_retired_geometry_archive(&kura, &root);
    kura.fail_next_lane_geometry_gc_at_stage_for_test(GC_FAIL_AFTER_COMPACTION_INTENT);
    checkpoint_retired_geometry(&kura, &fixture, 20).expect_err("leave durable pending-GC intent");
    assert!(fixture.retained_blocks.exists());
    assert!(
        kura.purge_retired_segments()
            .expect("budget purge must validate retired geometry"),
        "budget purge resumes only the already-proven archive deletion"
    );
    assert!(!fixture.retained_blocks.exists());
    assert!(
        kura.read_lane_geometry_journal()
            .expect("journal after budget purge")
            .pending_archive_gc
            .is_empty()
    );
    assert_eq!(
        kura.disk_usage.load(std::sync::atomic::Ordering::Relaxed),
        kura.kura_disk_usage_bytes()
            .expect("exact usage after purge")
    );
}
#[test]
fn archive_gc_through_budget_purge_forces_a_paused_usage_scan_to_retry_exactly() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let kura = open_kura(&root, &initial_and_extended_configs().0);
    let fixture = prepare_retired_geometry_archive(&kura, &root);
    kura.fail_next_lane_geometry_gc_at_stage_for_test(GC_FAIL_AFTER_COMPACTION_INTENT);
    checkpoint_retired_geometry(&kura, &fixture, 20)
        .expect_err("leave a durable, snapshot-proven pending archive deletion");
    assert!(fixture.retained_blocks.exists());
    kura.refresh_disk_usage_bytes()
        .expect("establish exact pre-GC usage baseline");
    kura.pause_next_total_disk_usage_scan_after_scan_for_tests();
    let scan_kura = Arc::clone(&kura);
    let (scan_tx, scan_rx) = mpsc::channel();
    let scan = thread::spawn(move || {
        scan_tx
            .send(scan_kura.refresh_disk_usage_bytes())
            .expect("report usage scan result");
    });
    wait_for_total_usage_scan_pause(&kura);
    let purged = kura
        .purge_retired_segments()
        .expect("budget purge must validate retired geometry");
    let remained_paused = matches!(
        scan_rx.recv_timeout(Duration::from_millis(50)),
        Err(mpsc::RecvTimeoutError::Timeout)
    );
    kura.resume_total_disk_usage_scan_for_tests();
    let refreshed = scan_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("paused usage scan must finish after release")
        .expect("retried usage scan succeeds");
    scan.join().expect("join paused usage scan");
    assert!(purged, "budget purge must resume the proven archive GC");
    assert!(
        remained_paused,
        "the deterministic scan barrier must remain active through archive GC"
    );
    assert!(!fixture.retained_blocks.exists());
    assert!(
        kura.read_lane_geometry_journal()
            .expect("journal after archive GC")
            .pending_archive_gc
            .is_empty()
    );
    let exact_enforced = kura
        .kura_disk_usage_bytes()
        .expect("exact enforced usage after archive GC");
    let exact_total = kura
        .kura_total_disk_usage_bytes()
        .expect("exact total usage after archive GC");
    assert_eq!(refreshed, exact_enforced);
    assert_eq!(
        kura.disk_usage.load(std::sync::atomic::Ordering::Relaxed),
        exact_enforced,
        "archive GC must publish its enforced-usage subtraction exactly"
    );
    assert_eq!(
        kura.disk_usage_total
            .load(std::sync::atomic::Ordering::Relaxed),
        exact_total,
        "archive GC must publish its total-usage subtraction exactly"
    );
}
#[test]
fn storage_budget_purge_never_deletes_uncheckpointed_geometry_by_age_or_pressure() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let kura = open_kura(&root, &initial_and_extended_configs().0);
    let fixture = prepare_retired_geometry_archive(&kura, &root);
    let sentinel = fixture.retained_blocks.join(MARKER_FILE_NAME);
    let expected_marker = fs::read(&sentinel).expect("exact uncheckpointed instance marker");
    let _ = kura.purge_retired_segments();
    assert!(fixture.retained_blocks.exists());
    assert_eq!(
        fs::read(sentinel).expect("uncheckpointed archive retained"),
        expected_marker
    );
    assert_eq!(
        kura.read_lane_geometry_journal()
            .expect("retained recovery journal")
            .records
            .len(),
        2
    );
}
#[cfg(unix)]
#[test]
fn geometry_sidecar_temp_symlink_and_regular_collision_fail_without_clobbering() {
    use std::os::unix::fs::symlink;
    for collision_kind in ["symlink", "regular"] {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join(format!("kura-{collision_kind}"));
        let (initial, extended) = initial_and_extended_configs();
        let (initial_incarnations, initial_activations) = initial_geometry();
        let (extended_incarnations, extended_activations) = extended_geometry();
        let kura = open_kura(&root, &initial);
        let collision = root.join(JOURNAL_TEMP_FILE_NAME);
        let outside = temp.path().join("operator-data");
        fs::write(&outside, b"operator-owned").expect("outside sentinel");
        if collision_kind == "symlink" {
            symlink(&outside, &collision).expect("journal temp symlink");
        } else {
            fs::write(&collision, b"operator-owned").expect("journal temp collision");
        }
        kura.apply_lane_geometry_transition(
            &initial,
            &extended,
            &initial_incarnations,
            &extended_incarnations,
            &initial_activations,
            &extended_activations,
            &BTreeSet::new(),
        )
        .expect_err("unsafe or unrelated temp collision must fail closed");
        assert_eq!(
            fs::read(&outside).expect("outside retained"),
            b"operator-owned"
        );
        if collision_kind == "regular" {
            assert_eq!(
                fs::read(&collision).expect("regular collision retained"),
                b"operator-owned"
            );
        }
    }
}
#[test]
fn geometry_inode_identity_detects_path_replacement() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let kura = open_kura(&root, &initial_and_extended_configs().0);
    let path = root.join("inode-guard.norito");
    fs::write(&path, b"first").expect("first inode");
    let identity = kura
        .geometry_path_identity(&path, false)
        .expect("capture first inode");
    fs::rename(&path, root.join("inode-guard.old")).expect("move first inode");
    fs::write(&path, b"second").expect("replacement inode");
    kura.require_geometry_path_identity(&path, false, identity)
        .expect_err("replacement inode must not pass identity revalidation");
}
#[test]
fn geometry_gc_rejects_preexisting_quarantine_collision() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let kura = open_kura(&root, &initial_and_extended_configs().0);
    let fixture = prepare_retired_geometry_archive(&kura, &root);
    kura.fail_next_lane_geometry_gc_at_stage_for_test(GC_FAIL_AFTER_COMPACTION_INTENT);
    checkpoint_retired_geometry(&kura, &fixture, 20).expect_err("leave pending deletion");
    let pending = kura
        .read_lane_geometry_journal()
        .expect("pending GC ownership");
    let retirement = pending
        .pending_archive_gc
        .last()
        .expect("retired instance owner");
    let archive_parent = kura.store_root().join("retired/lane_geometry");
    fs::create_dir_all(&archive_parent).expect("quarantine parent");
    let quarantine = archive_parent.join(format!(
        "{GC_QUARANTINE_PREFIX}{}",
        hex::encode(retirement.intent.transition_id.as_ref()),
    ));
    fs::create_dir(&quarantine).expect("quarantine collision");
    fs::write(quarantine.join("operator-data"), b"retain").expect("collision sentinel");
    kura.resume_proven_lane_geometry_archive_gc()
        .expect_err("an unowned quarantine must fail before moving the retained instance");
    assert!(fixture.retained_blocks.exists());
    assert_eq!(
        fs::read(quarantine.join("operator-data")).expect("collision retained"),
        b"retain"
    );
}
#[cfg(any(
    target_vendor = "apple",
    target_os = "linux",
    target_os = "android",
    target_os = "redox"
))]
#[test]
fn geometry_gc_quarantine_cannot_escape_a_substituted_parent() {
    let temp = TempDir::new().expect("temporary directory");
    let configured_root = temp.path().join("kura");
    let kura = open_kura(&configured_root, &initial_and_extended_configs().0);
    // Kura resolves a configured root exactly once at startup. On macOS,
    // `TempDir` may spell a `/private/var/...` directory through its
    // `/var/...` alias, so every post-startup path must descend from Kura's
    // authenticated root.
    let root = kura.store_root();
    let fixture = prepare_retired_geometry_archive(&kura, &root);
    kura.fail_next_lane_geometry_gc_at_stage_for_test(GC_FAIL_AFTER_COMPACTION_INTENT);
    checkpoint_retired_geometry(&kura, &fixture, 20).expect_err("leave pending deletion");
    let mut expected_intent = kura
        .read_lane_geometry_journal()
        .expect("pending geometry journal");
    let retirement = expected_intent
        .pending_archive_gc
        .last_mut()
        .expect("retirement owns the last exact reference to the released instance");
    let attacked_transition = retirement.intent.transition_id;
    let collected_binding = retirement
        .intent
        .operations
        .iter()
        .find(|operation| operation.lane_id == LaneId::new(1))
        .and_then(|operation| operation.previous.clone())
        .expect("exact retired instance");
    assert!(retirement.collecting.is_empty());
    retirement.collecting.push(collected_binding);
    let pending_root = geometry_pending_archive_gc_root(&expected_intent.pending_archive_gc);
    let checkpoint = expected_intent
        .checkpoint
        .as_mut()
        .expect("durable snapshot checkpoint");
    checkpoint.pending_archive_gc_root = Some(pending_root);
    checkpoint.commitment = geometry_checkpoint_commitment(checkpoint);
    let archive_parent = root.join("retired/lane_geometry");
    let root_name = hex::encode(attacked_transition.as_ref());
    let quarantine_name = format!("{GC_QUARANTINE_PREFIX}{root_name}");
    assert!(fixture.retained_blocks.is_dir());
    assert!(
        !archive_parent.join(&root_name).exists(),
        "Apply and checkpoint admission must retain the immutable instance in place"
    );
    let journal_path = kura.lane_geometry_journal_path();
    let displaced_parent = root.join("authenticated-retired-parent");
    let outside_parent = temp.path().join("outside-retired-parent");
    fs::create_dir(&outside_parent).expect("create outside replacement parent");
    *GEOMETRY_MOVE_PARENT_SUBSTITUTION
        .lock()
        .expect("geometry parent-substitution hook lock") = Some((
        archive_parent.clone(),
        displaced_parent.clone(),
        outside_parent.clone(),
    ));
    let substitution_error = kura
        .resume_proven_lane_geometry_archive_gc()
        .expect_err("a substituted archive parent must fail GC closed");
    assert!(
        GEOMETRY_MOVE_PARENT_SUBSTITUTION
            .lock()
            .expect("geometry parent-substitution hook lock")
            .is_none(),
        "the parent substitution must occur at the pre-rename barrier"
    );
    assert!(
        fs::symlink_metadata(&archive_parent)
            .expect("substituted archive parent metadata")
            .file_type()
            .is_symlink()
    );
    for name in [&root_name, &quarantine_name] {
        assert!(
            !outside_parent.join(name).exists(),
            "descriptor-relative GC must not publish through the replacement symlink"
        );
    }
    let retained_source = displaced_parent.join(&root_name).is_dir();
    let retained_quarantine = displaced_parent.join(&quarantine_name).is_dir();
    assert_ne!(
        retained_source, retained_quarantine,
        "the authenticated parent must retain exactly one archive image after the failed GC: {substitution_error:?}"
    );
    kura.read_lane_geometry_journal()
        .expect_err("validated journal reads must reject the substituted archive namespace");
    assert_eq!(
        fs::read(&journal_path).expect("durable pending GC intent after failed revalidation"),
        expected_intent.encode(),
        "the exact instance transfer must be durable, but failed parent revalidation must not acknowledge deletion or clear its intent"
    );
    fs::remove_file(&archive_parent).expect("remove substituted archive-parent symlink");
    fs::rename(&displaced_parent, &archive_parent).expect("restore authenticated archive parent");
    assert!(
        !kura
            .read_lane_geometry_journal()
            .expect("pending geometry journal")
            .pending_archive_gc
            .is_empty(),
        "failed parent revalidation must retain the durable GC intent"
    );
}
#[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
#[test]
fn geometry_gc_descriptor_deletion_cannot_follow_a_substituted_parent() {
    use std::os::unix::fs::symlink;
    let temp = TempDir::new().expect("temporary directory");
    let configured_root = temp.path().join("kura");
    let kura = open_kura(&configured_root, &initial_and_extended_configs().0);
    // Use the root identity Kura authenticated, rather than retaining a
    // possibly symlink-aliased spelling supplied at the configuration edge.
    let root = kura.store_root();
    let archive_parent = root.join("descriptor-gc-parent");
    let deletion_root = archive_parent.join(".gc-authenticated");
    let nested = deletion_root.join("nested/leaf");
    fs::create_dir_all(&nested).expect("create authenticated deletion tree");
    fs::write(nested.join("payload.norito"), b"authenticated")
        .expect("seed authenticated deletion tree");
    let deletion_identity = kura
        .geometry_path_identity(&deletion_root, true)
        .expect("authenticated deletion identity");
    let (parent_handle, _) = kura
        .open_geometry_parent(&archive_parent)
        .expect("authenticated deletion parent");
    let displaced_parent = root.join("descriptor-gc-parent.displaced");
    let outside_parent = temp.path().join("outside-descriptor-gc-parent");
    let outside_collision = outside_parent.join(".gc-authenticated");
    fs::create_dir_all(&outside_collision).expect("create outside collision");
    fs::write(outside_collision.join("operator-data"), b"retain")
        .expect("outside collision sentinel");
    fs::rename(&archive_parent, &displaced_parent).expect("displace authenticated parent");
    symlink(&outside_parent, &archive_parent).expect("substitute archive parent");
    Kura::remove_authenticated_geometry_tree_at(
        &parent_handle,
        std::ffi::OsStr::new(".gc-authenticated"),
        deletion_identity,
        &deletion_root,
    )
    .expect("descriptor-relative deletion stays under the authenticated parent handle");
    assert!(
        !displaced_parent.join(".gc-authenticated").exists(),
        "the authenticated quarantine must be removed"
    );
    assert_eq!(
        fs::read(outside_collision.join("operator-data")).expect("outside sentinel retained"),
        b"retain",
        "the substituted path namespace must remain untouched"
    );
}
#[test]
fn geometry_gc_rejects_unauthenticated_archive_collision_without_deleting_it() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let kura = open_kura(&root, &initial_and_extended_configs().0);
    let fixture = prepare_retired_geometry_archive(&kura, &root);
    let collision = fixture.retained_blocks.join("operator-data.txt");
    fs::write(&collision, b"must not delete").expect("seed unauthenticated collision");
    checkpoint_retired_geometry(&kura, &fixture, 20)
        .expect_err("unexpected archive content must fail closed");
    assert_eq!(
        fs::read(&collision).expect("collision retained"),
        b"must not delete"
    );
    let journal = kura.read_lane_geometry_journal().expect("pending journal");
    assert!(journal.records.is_empty());
    assert!(!journal.pending_archive_gc.is_empty());
    fs::remove_file(&collision).expect("operator resolves collision");
    let resumed = kura
        .resume_proven_lane_geometry_archive_gc()
        .expect("resume proven GC after repair");
    assert_eq!(resumed.removed_archive_roots, 1);
}
#[test]
fn native_amx_archive_is_admissible_accounted_and_purged_without_touching_sibling() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (kura, fixture) = prepare_native_amx_archive(&root);
    let sibling = root.join("operator-sibling.keep");
    fs::write(&sibling, b"retain across Native archive GC").expect("seed non-archive sibling");
    let native_paths = [&fixture.manifest, &fixture.receipt, &fixture.latest_index];
    for path in native_paths {
        assert!(
            path.is_file(),
            "recognized Native archive evidence is retained: {}",
            path.display()
        );
    }
    kura.ensure_archived_lane_work_released_for_test(
        &fixture.archived_blocks,
        &fixture.binding,
        &[],
    )
    .expect("exact Native manifest, receipt, and latest index are terminal");
    let recognized_bytes = native_paths
        .into_iter()
        .map(|path| {
            fs::metadata(path)
                .expect("Native archive artifact metadata")
                .len()
        })
        .sum::<u64>();
    let archived_bytes = Kura::regular_geometry_archive_tree_bytes(&fixture.archived_blocks)
        .expect("account authenticated Native archive bytes");
    assert!(
        archived_bytes >= recognized_bytes && recognized_bytes > 0,
        "archive accounting must include Native manifest, receipt, and latest-index bytes"
    );
    durable_geometry_snapshot_identity(&kura, 20);
    kura.refresh_disk_usage_bytes()
        .expect("refresh usage with Native evidence archive");
    let usage_before = kura
        .kura_disk_usage_bytes()
        .expect("exact usage before Native archive GC");
    kura.fail_next_lane_geometry_gc_at_stage_for_test(GC_FAIL_AFTER_COMPACTION_INTENT);
    checkpoint_retired_geometry(&kura, &fixture.geometry, 20)
        .expect_err("leave the Native archive under a durable snapshot-proven GC intent");
    assert!(
        fixture.geometry.retained_blocks.exists(),
        "the injected boundary must preserve Native evidence for purge replay"
    );
    assert!(
        kura.purge_retired_segments()
            .expect("budget purge revalidates exact Native archive evidence"),
        "budget purge must resume the snapshot-proven Native archive deletion"
    );
    assert!(!fixture.geometry.retained_blocks.exists());
    let usage_after = kura
        .kura_disk_usage_bytes()
        .expect("exact usage after Native archive GC");
    assert!(
        usage_after < usage_before,
        "removing the Native evidence archive must reduce accounted disk usage"
    );
    assert_eq!(
        kura.disk_usage.load(std::sync::atomic::Ordering::Relaxed),
        usage_after,
        "Native archive GC must publish exact post-removal disk accounting"
    );
    assert_eq!(
        fs::read(&sibling).expect("non-archive sibling retained"),
        b"retain across Native archive GC",
        "authenticated Native archive purge must remain path-scoped"
    );
}
#[test]
fn native_amx_archive_gc_rejects_malformed_truncated_and_oversized_evidence() {
    for corruption in [
        "malformed",
        "truncated",
        "missing-receipt",
        "oversized",
        "oversized-artifact",
        "aggregate",
    ] {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join(corruption);
        let (kura, fixture) = prepare_native_amx_archive(&root);
        let sibling = root.join("operator-sibling.keep");
        fs::write(&sibling, b"retain while Native evidence is repaired")
            .expect("seed non-archive sibling");
        match corruption {
            "malformed" => {
                fs::write(&fixture.latest_index, b"malformed Native latest index")
                    .expect("corrupt Native latest index");
            }
            "truncated" => {
                let mut bytes = fs::read(&fixture.receipt).expect("read Native receipt");
                assert!(bytes.pop().is_some(), "receipt fixture is non-empty");
                fs::write(&fixture.receipt, bytes).expect("truncate Native receipt");
            }
            "missing-receipt" => {
                fs::remove_file(&fixture.receipt).expect("remove Native archive receipt half-pair");
            }
            "oversized" => {
                let file = OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .open(&fixture.latest_index)
                    .expect("open Native latest index for oversize corruption");
                file.set_len(
                    u64::try_from(
                        crate::kura::NATIVE_AMX_PARTICIPANT_RECEIPTS_LATEST_INDEX_MAX_BYTES
                            .saturating_add(1),
                    )
                    .expect("Native latest-index limit fits u64"),
                )
                .expect("extend Native latest index beyond its hard limit");
            }
            "oversized-artifact" => {
                let file = OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .open(&fixture.manifest)
                    .expect("open Native manifest for oversize corruption");
                file.set_len(
                    DEFAULT_NATIVE_AMX_PARTICIPANT_EVIDENCE_FILE_BYTES
                        .checked_add(1)
                        .expect("Native evidence limit can grow by one"),
                )
                .expect("extend Native manifest beyond its hard limit");
            }
            "aggregate" => {
                let second_manifest = fixture
                        .manifest
                        .parent()
                        .expect("Native manifest has an archive directory")
                        .join(format!(
                            "{NATIVE_AMX_APPLICATION_MANIFEST_FILE_PREFIX}{:0width$}{NATIVE_AMX_EVIDENCE_FILE_SUFFIX}",
                            2,
                            width = NATIVE_AMX_EVIDENCE_HEIGHT_DIGITS,
                        ));
                let file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&second_manifest)
                    .expect("create a second Native manifest");
                file.set_len(DEFAULT_NATIVE_AMX_PARTICIPANT_EVIDENCE_FILE_BYTES)
                    .expect("stage aggregate-bound Native manifest");
            }
            _ => unreachable!("enumerated corruption"),
        }
        let result = kura.ensure_archived_lane_work_released_for_test(
            &fixture.archived_blocks,
            &fixture.binding,
            &[],
        );
        assert!(
            result.is_err(),
            "{corruption} Native evidence must fail archive validation"
        );
        assert!(
            fixture.geometry.retained_blocks.exists(),
            "{corruption} Native evidence must pin the archive for repair"
        );
        assert_eq!(
            fs::read(&sibling).expect("non-archive sibling retained"),
            b"retain while Native evidence is repaired",
            "failed Native validation must not mutate a sibling"
        );
    }
}
#[cfg(unix)]
#[test]
fn native_amx_archive_gc_rejects_symlinked_evidence_without_following_it() {
    use std::os::unix::fs::symlink;
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let outside = temp.path().join("operator-owned-native-manifest");
    fs::write(&outside, b"operator-owned").expect("outside Native sentinel");
    let (kura, fixture) = prepare_native_amx_archive(&root);
    fs::remove_file(&fixture.manifest).expect("remove canonical Native manifest");
    symlink(&outside, &fixture.manifest).expect("install Native manifest symlink");
    kura.ensure_archived_lane_work_released_for_test(
        &fixture.archived_blocks,
        &fixture.binding,
        &[],
    )
    .expect_err("a symlinked Native manifest must fail closed");
    assert_eq!(
        fs::read(&outside).expect("outside Native sentinel retained"),
        b"operator-owned"
    );
    assert!(
        fs::symlink_metadata(&fixture.manifest)
            .expect("Native manifest symlink remains for operator repair")
            .file_type()
            .is_symlink()
    );
    assert!(fixture.geometry.retained_blocks.exists());
    let hardlink_root = temp.path().join("hardlink-kura");
    let hardlink_outside = temp.path().join("operator-owned-native-hardlink");
    let (hardlink_kura, hardlink_fixture) = prepare_native_amx_archive(&hardlink_root);
    fs::hard_link(&hardlink_fixture.manifest, &hardlink_outside)
        .expect("create external hardlink to the Native manifest");
    hardlink_kura
        .ensure_archived_lane_work_released_for_test(
            &hardlink_fixture.archived_blocks,
            &hardlink_fixture.binding,
            &[],
        )
        .expect_err("a hardlinked Native manifest must fail closed");
    assert_eq!(
        fs::read(&hardlink_outside).expect("outside hardlink remains readable"),
        fs::read(&hardlink_fixture.manifest).expect("archived hardlink remains readable"),
    );
    assert!(hardlink_fixture.geometry.retained_blocks.exists());
}
#[test]
fn tombstoned_autonomous_artifacts_are_retirement_archive_gc_admissible_and_accounted() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (kura, fixture) = prepare_tombstoned_autonomous_archive(&root);
    for path in [
        &fixture.autonomous_attempt,
        &fixture.view_state,
        &fixture.height_pointer,
        &fixture.route_pointer,
    ] {
        assert!(
            path.is_file(),
            "recognized autonomous archive artifact is retained: {}",
            path.display()
        );
    }
    kura.ensure_archived_lane_work_released_for_test(
        &fixture.archived_blocks,
        &fixture.binding,
        &[],
    )
    .expect("an exact tombstone makes archived autonomous work terminal");
    let recognized_bytes = [
        &fixture.autonomous_attempt,
        &fixture.view_state,
        &fixture.height_pointer,
        &fixture.route_pointer,
    ]
    .into_iter()
    .map(|path| {
        fs::metadata(path)
            .expect("recognized artifact metadata")
            .len()
    })
    .sum::<u64>();
    let archived_bytes = Kura::regular_geometry_archive_tree_bytes(&fixture.archived_blocks)
        .expect("account authenticated autonomous archive bytes");
    assert!(
        archived_bytes >= recognized_bytes && recognized_bytes > 0,
        "archive accounting must include autonomous attempt, view, and latest-pointer bytes"
    );
    durable_geometry_snapshot_identity(&kura, 20);
    kura.refresh_disk_usage_bytes()
        .expect("refresh usage with autonomous archive");
    let usage_before = kura
        .kura_disk_usage_bytes()
        .expect("exact usage before autonomous archive GC");
    let summary = checkpoint_retired_geometry(&kura, &fixture.geometry, 20)
        .expect("snapshot-proven GC accepts tombstoned autonomous evidence");
    assert_eq!(summary.removed_archive_roots, 1);
    assert!(!fixture.geometry.retained_blocks.exists());
    let usage_after = kura
        .kura_disk_usage_bytes()
        .expect("exact usage after autonomous archive GC");
    assert!(
        usage_after < usage_before,
        "removing the authenticated autonomous archive must reduce accounted disk usage"
    );
    assert_eq!(
        kura.disk_usage.load(std::sync::atomic::Ordering::Relaxed),
        usage_after,
        "archive GC must publish exact post-removal disk accounting"
    );
}
#[test]
fn autonomous_archive_gc_rejects_malformed_oversized_temporary_and_unexpected_artifacts() {
    for corruption in ["malformed", "oversized", "temporary", "unexpected"] {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join(corruption);
        let (kura, fixture) = prepare_tombstoned_autonomous_archive(&root);
        match corruption {
            "malformed" => {
                fs::write(&fixture.view_state, b"malformed autonomous view state")
                    .expect("corrupt autonomous view state");
            }
            "oversized" => {
                let file = OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .open(&fixture.view_state)
                    .expect("open autonomous view state for oversize corruption");
                file.set_len(
                    u64::try_from(
                        crate::kura::AUTONOMOUS_LANE_BLOCK_VIEW_STATE_MAX_BYTES.saturating_add(1),
                    )
                    .expect("view-state limit fits u64"),
                )
                .expect("extend autonomous view state past its hard limit");
            }
            "temporary" => {
                fs::write(
                    fixture.view_state.with_extension("norito.tmp"),
                    b"ambiguous autonomous view rewrite",
                )
                .expect("stage autonomous view temp");
            }
            "unexpected" => {
                fs::write(
                    fixture
                        .view_state
                        .parent()
                        .expect("lane artifact directory")
                        .join("operator-junk.bin"),
                    b"unexpected",
                )
                .expect("seed unexpected autonomous archive artifact");
            }
            _ => unreachable!("enumerated corruption"),
        }
        let result = kura.ensure_archived_lane_work_released_for_test(
            &fixture.archived_blocks,
            &fixture.binding,
            &[],
        );
        assert!(
            result.is_err(),
            "{corruption} autonomous archive artifact must fail closed"
        );
        assert!(
            fixture.geometry.retained_blocks.exists(),
            "{corruption} autonomous evidence must pin the archive for repair"
        );
    }
}
#[cfg(unix)]
#[test]
fn autonomous_archive_gc_rejects_symlinked_view_artifact_without_following_it() {
    use std::os::unix::fs::symlink;
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let outside = temp.path().join("operator-owned-view-state");
    fs::write(&outside, b"operator-owned").expect("outside sentinel");
    let (kura, fixture) = prepare_tombstoned_autonomous_archive(&root);
    fs::remove_file(&fixture.view_state).expect("remove canonical archived view state");
    symlink(&outside, &fixture.view_state).expect("install autonomous view-state symlink");
    kura.ensure_archived_lane_work_released_for_test(
        &fixture.archived_blocks,
        &fixture.binding,
        &[],
    )
    .expect_err("a symlinked autonomous view state must fail closed");
    assert_eq!(
        fs::read(&outside).expect("outside sentinel retained"),
        b"operator-owned"
    );
    assert!(
        fs::symlink_metadata(&fixture.view_state)
            .expect("symlink remains for operator repair")
            .file_type()
            .is_symlink()
    );
    assert!(fixture.geometry.retained_blocks.exists());
}
#[test]
fn geometry_gc_pins_unmerged_autonomous_work_and_preserves_global_claim_evidence() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (kura, fixture) = prepare_tombstoned_autonomous_archive(&root);
    let terminal_view =
        fs::read(&fixture.view_state).expect("read exact archived autonomous tombstone");
    fs::remove_file(&fixture.view_state)
        .expect("remove tombstone to model a valid pending autonomous attempt");
    let claim = root
        .join("blocks/autonomous_entrypoint_claims_ff")
        .join("claim.norito");
    fs::create_dir_all(claim.parent().expect("claim parent")).expect("claim directory");
    fs::write(
        &claim,
        b"reservation/entrypoint claim outside retired geometry",
    )
    .expect("global claim sentinel");
    checkpoint_retired_geometry(&kura, &fixture.geometry, 20)
        .expect_err("unmerged autonomous sidecar must pin retired geometry");
    assert!(fixture.geometry.retained_blocks.exists());
    assert!(
        !kura
            .read_lane_geometry_journal()
            .expect("pinned pending journal")
            .pending_archive_gc
            .is_empty()
    );
    // Restore the exact durable tombstone. Once the attempt is terminal, the
    // already-proven snapshot may release storage.
    fs::write(&fixture.view_state, terminal_view).expect("restore exact autonomous tombstone");
    let resumed = kura
        .resume_proven_lane_geometry_archive_gc()
        .expect("empty retired work set releases after repair");
    assert_eq!(resumed.removed_archive_roots, 1);
    assert_eq!(
        fs::read(&claim).expect("global claim evidence retained"),
        b"reservation/entrypoint claim outside retired geometry"
    );
}
#[test]
fn geometry_gc_pins_certified_work_without_a_durable_merge_receipt() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let kura = open_kura(&root, &initial_and_extended_configs().0);
    let fixture = prepare_retired_geometry_archive(&kura, &root);
    let lane_id = LaneId::new(1);
    let incarnation = fixture.extended_incarnations[&lane_id];
    let dataspace_id = fixture
        .extended
        .entry(lane_id)
        .expect("retired lane")
        .dataspace_id;
    let certified = certified_geometry_lane_block(lane_id, dataspace_id, incarnation, 1);
    let descriptor = &certified.proposal.descriptor;
    let archived_blocks = fixture.retained_blocks.clone();
    let lane_artifacts = archived_blocks.join(LANE_ARTIFACTS_DIR_NAME);
    fs::create_dir_all(&lane_artifacts).expect("archived lane artifacts");
    let payload = certified
        .encode_framed()
        .expect("encode certified lane block");
    fs::write(
        lane_artifacts.join(CERTIFIED_LANE_BLOCKS_DATA_FILE),
        &payload,
    )
    .expect("certified data sidecar");
    let mut index = SidecarIndexLayout::base_header(descriptor.lane_block_height).to_vec();
    index.extend_from_slice(
        &SidecarIndexEntry {
            offset: 0,
            len: u64::try_from(payload.len()).expect("payload length"),
        }
        .to_bytes(),
    );
    fs::write(lane_artifacts.join(CERTIFIED_LANE_BLOCKS_INDEX_FILE), index)
        .expect("certified index sidecar");
    let journal = kura.read_lane_geometry_journal().expect("geometry journal");
    let binding = journal
        .records
        .last()
        .expect("retirement transition")
        .operations
        .iter()
        .find_map(|operation| {
            (operation.lane_id == lane_id)
                .then_some(operation.previous.as_ref())
                .flatten()
        })
        .expect("retired lane binding")
        .clone();
    let carrier_hash = HashOf::from_untyped_unchecked(Hash::new(b"carrier"));
    let release = LaneGeometryMergeRelease {
        lane_id,
        dataspace_id,
        lane_incarnation: incarnation,
        lane_block_height: descriptor.lane_block_height,
        application_block_height: 20,
        application_block_hash: carrier_hash,
        merge_entry_hash: HashOf::from_untyped_unchecked(Hash::new(b"merge-entry")),
        merge_epoch_id: 7,
        source_bundle_hash: Hash::new(b"source-bundle"),
        batch_identity_hash: Hash::new(b"batch-identity"),
        batch_hash: Hash::new(b"batch"),
        lane_execution_hash: Hash::new(b"lane-execution"),
        marker_set_root: Hash::new(b"markers"),
        receipt_hash: Hash::new(b"receipt"),
    };
    let error = kura
        .ensure_archived_lane_work_released_for_test(&archived_blocks, &binding, &[release])
        .expect_err("a merge release without its durable receipt must pin the archive");
    assert_geometry_io_error(
        &error,
        ErrorKind::WouldBlock,
        "retired lane merge application receipt is missing or malformed",
    );
    let Error::IO(_, path) = &error else {
        unreachable!("assert_geometry_io_error established the error variant")
    };
    assert_eq!(path, &kura.lane_geometry_journal_path());
    assert!(fixture.retained_blocks.exists());
}
#[test]
fn geometry_gc_requires_bound_merge_receipt_durability_before_deletion() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (initial, extended) = retirement_test_configs();
    let (extended_incarnations, extended_activations) = retirement_test_geometry();
    let initial_incarnations =
        BTreeMap::from([(LaneId::SINGLE, extended_incarnations[&LaneId::SINGLE])]);
    let initial_activations =
        BTreeMap::from([(LaneId::SINGLE, extended_activations[&LaneId::SINGLE])]);
    let (kura, _, _) = open_published_retirement_kura(
        &root,
        &initial,
        &extended,
        &initial_incarnations,
        &extended_incarnations,
        &initial_activations,
        &extended_activations,
    );
    let retiring_lane = LaneId::new(1);
    let retiring_incarnation = extended_incarnations[&retiring_lane];
    let work = install_merge_applied_retirement_work(&kura, retiring_incarnation);
    kura.apply_lane_geometry_transition(
        &extended,
        &initial,
        &extended_incarnations,
        &initial_incarnations,
        &extended_activations,
        &initial_activations,
        &BTreeSet::new(),
    )
    .expect("archive merge-applied retiring work");
    kura.mark_lane_geometry_catalog_published(
        &initial,
        &initial_incarnations,
        &initial_activations,
        None,
    )
    .expect("publish merge-applied retirement");
    let journal = kura
        .read_lane_geometry_journal()
        .expect("retirement journal");
    let retirement = journal.records.last().expect("retirement transition");
    let archive_root = root
        .join("retired/lane_geometry")
        .join(hex::encode(retirement.transition_id.as_ref()));
    let retired_binding = retirement
        .operations
        .iter()
        .find(|operation| operation.lane_id == retiring_lane)
        .and_then(|operation| operation.previous.as_ref())
        .expect("exact retired instance binding");
    let retained_blocks = kura.binding_blocks_path(retired_binding);
    assert!(retained_blocks.is_dir());
    assert!(
        !archive_root.exists(),
        "Apply only changes authenticated references"
    );
    let lane_artifacts = retained_blocks.join(LANE_ARTIFACTS_DIR_NAME);
    let receipt_data = lane_artifacts.join(LANE_BLOCK_APPLICATION_RECEIPTS_DATA_FILE);
    let receipt_index = lane_artifacts.join(LANE_BLOCK_APPLICATION_RECEIPTS_INDEX_FILE);
    {
        let _sidecar_guard = kura.sidecar_lock.lock();
        assert!(
            kura.read_lane_block_application_receipt_from_paths_locked(
                retiring_lane,
                work.certified.proposal.descriptor.lane_block_height,
                &receipt_data,
                &receipt_index,
                false,
            )
            .is_some(),
            "the merge receipt must remain page-cache readable before its barrier fails"
        );
    }
    let (snapshot_block_hash, snapshot_state_hash) = durable_geometry_snapshot_identity(&kura, 20);
    let bindings = kura
        .geometry_bindings(&initial, &initial_incarnations, &initial_activations)
        .expect("snapshot geometry bindings");
    let lineage_root = unscoped_lineage_root(&bindings);
    let (recovery_bindings, recovery_root) =
        geometry_fixture_recovery(&kura, 20, &bindings, lineage_root);
    fail_next_archived_receipt_durability_attestation_for_test(
        ProgressSidecarDurabilityFault::Ancestor(0),
    );
    let error = kura
        .checkpoint_lane_geometry_with_proven_snapshot(
            bindings,
            lineage_root,
            recovery_bindings,
            recovery_root,
            20,
            Some(snapshot_block_hash),
            snapshot_state_hash,
            vec![work.release],
        )
        .expect_err("an unsynchronized merge receipt must not authorize archive deletion");
    assert_geometry_io_error(
        &error,
        ErrorKind::WouldBlock,
        "retired lane merge application receipt durability attestation failed",
    );
    assert!(
        retained_blocks.is_dir() && !archive_root.exists(),
        "failed receipt durability must retain the immutable instance before any GC move"
    );
    assert!(
        !kura
            .read_lane_geometry_journal()
            .expect("pending GC journal")
            .pending_archive_gc
            .is_empty(),
        "failed receipt durability must retain the replayable GC intent"
    );
    assert!(
        receipt_data.is_file() && receipt_index.is_file(),
        "failed receipt durability must retain both readable sidecar files"
    );
    let resumed = kura
        .resume_proven_lane_geometry_archive_gc()
        .expect("receipt durability recovery resumes exact archived GC");
    assert_eq!(resumed.removed_archive_roots, 1);
    assert!(
        !retained_blocks.exists() && !archive_root.exists(),
        "the same authenticated instance is collected only after barrier recovery"
    );
}
#[test]
fn partial_multi_archive_gc_retains_intent_and_repairs_disk_accounting_on_resume() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let kura = open_kura(&root, &initial_and_extended_configs().0);
    let fixture = prepare_retired_geometry_archive(&kura, &root);
    let mut recreated_incarnations = fixture.extended_incarnations.clone();
    recreated_incarnations.insert(LaneId::new(1), Hash::prehashed([0x44; Hash::LENGTH]));
    let mut recreated_activations = fixture.extended_activations.clone();
    recreated_activations.insert(LaneId::new(1), 10);
    kura.apply_lane_geometry_transition(
        &fixture.initial,
        &fixture.extended,
        &fixture.initial_incarnations,
        &recreated_incarnations,
        &fixture.initial_activations,
        &recreated_activations,
        &BTreeSet::new(),
    )
    .expect("recreate retired lane");
    kura.mark_lane_geometry_catalog_published(
        &fixture.extended,
        &recreated_incarnations,
        &recreated_activations,
        None,
    )
    .expect("publish recreated lane");
    let recreated_binding = kura
        .geometry_binding(
            fixture
                .extended
                .entry(LaneId::new(1))
                .expect("recreated lane"),
            &recreated_incarnations,
            &recreated_activations,
        )
        .expect("exact recreated identity");
    let recreated_blocks = kura.binding_blocks_path(&recreated_binding);
    assert_ne!(recreated_blocks, fixture.retained_blocks);
    assert!(recreated_blocks.join(MARKER_FILE_NAME).is_file());
    kura.apply_lane_geometry_transition(
        &fixture.extended,
        &fixture.initial,
        &recreated_incarnations,
        &fixture.initial_incarnations,
        &recreated_activations,
        &fixture.initial_activations,
        &BTreeSet::new(),
    )
    .expect("retire recreated lane");
    kura.mark_lane_geometry_catalog_published(
        &fixture.initial,
        &fixture.initial_incarnations,
        &fixture.initial_activations,
        None,
    )
    .expect("publish second retirement");
    let journal = kura.read_lane_geometry_journal().expect("four transitions");
    assert_eq!(journal.records.len(), 4);
    let second_archive = root
        .join("retired/lane_geometry")
        .join(hex::encode(journal.records[3].transition_id.as_ref()));
    assert!(
        !second_archive.exists(),
        "Apply must not archive the second instance"
    );
    let collision = recreated_blocks.join("operator-data.txt");
    fs::write(&collision, b"retain until operator repair").expect("collision");
    durable_geometry_snapshot_identity(&kura, 20);
    kura.refresh_disk_usage_bytes()
        .expect("usage before partial GC");
    checkpoint_retired_geometry(&kura, &fixture, 20)
        .expect_err("second archive collision interrupts a multi-root GC pass");
    assert!(
        !fixture.retained_blocks.exists(),
        "first proven root was deleted"
    );
    assert!(
        recreated_blocks.exists(),
        "the failing immutable instance remains intact"
    );
    assert_eq!(
        fs::read(&collision).expect("collision retained"),
        b"retain until operator repair"
    );
    assert!(
        !kura
            .read_lane_geometry_journal()
            .expect("pending partial GC")
            .pending_archive_gc
            .is_empty()
    );
    let exact_after_partial = kura.kura_disk_usage_bytes().expect("exact partial usage");
    assert_eq!(
        kura.disk_usage.load(std::sync::atomic::Ordering::Relaxed),
        exact_after_partial,
        "a failed partial pass must repair the live disk-usage cache to the exact retained tree"
    );
    fs::remove_file(&collision).expect("repair archive collision");
    let resumed = kura
        .resume_proven_lane_geometry_archive_gc()
        .expect("resume all exact pending roots");
    assert_eq!(resumed.removed_archive_roots, 1);
    assert!(!second_archive.exists());
    assert!(!recreated_blocks.exists());
    assert_eq!(
        kura.disk_usage.load(std::sync::atomic::Ordering::Relaxed),
        kura.kura_disk_usage_bytes()
            .expect("exact usage after completed resume")
    );
}
#[cfg(unix)]
#[test]
fn geometry_gc_rejects_symlink_inside_archive_tree() {
    use std::os::unix::fs::symlink;
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let outside = temp.path().join("outside.txt");
    fs::write(&outside, b"outside").expect("outside sentinel");
    let kura = open_kura(&root, &initial_and_extended_configs().0);
    let fixture = prepare_retired_geometry_archive(&kura, &root);
    let archived_blocks = fixture.retained_blocks.clone();
    let link = archived_blocks.join("escape");
    symlink(&outside, &link).expect("seed archive symlink");
    checkpoint_retired_geometry(&kura, &fixture, 20).expect_err("archive symlink must fail closed");
    assert_eq!(fs::read(&outside).expect("outside retained"), b"outside");
    assert!(link.exists());
}
#[test]
fn recovery_rejects_pre_release_journal_layout() {
    #[derive(norito::NoritoSchema)]
    #[norito_schema(
        name = "iroha_core::kura::lane_geometry::tests::recovery_rejects_pre_release_journal_layout::PreReleaseLaneGeometryJournal"
    )]
    #[derive(Encode)]
    struct PreReleaseLaneGeometryJournal {
        version: u8,
        records: Vec<LaneGeometryIntent>,
    }
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (initial, _) = initial_and_extended_configs();
    let kura = open_kura(&root, &initial);
    let pre_release = PreReleaseLaneGeometryJournal {
        version: 1,
        records: Vec::new(),
    };
    fs::write(kura.lane_geometry_journal_path(), pre_release.encode())
        .expect("write pre-release journal");
    kura.read_lane_geometry_journal()
        .expect_err("pre-release journal layout must fail closed");
}
#[test]
fn recovery_rejects_corrupt_and_forged_journals() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (initial, extended) = initial_and_extended_configs();
    let (initial_incarnations, initial_activations) = initial_geometry();
    let (extended_incarnations, extended_activations) = extended_geometry();
    let kura = open_kura(&root, &initial);
    let pristine = fs::read(kura.lane_geometry_journal_path())
        .expect("retain the authenticated fixture baseline");
    fs::write(kura.lane_geometry_journal_path(), b"not norito").expect("write corrupt journal");
    kura.recover_lane_geometry_journal(&initial, &initial_incarnations, &initial_activations)
        .expect_err("corrupt journal must fail closed");
    fs::write(kura.lane_geometry_journal_path(), pristine)
        .expect("restore the exact authenticated baseline after the corruption control");
    kura.apply_lane_geometry_transition(
        &initial,
        &extended,
        &initial_incarnations,
        &extended_incarnations,
        &initial_activations,
        &extended_activations,
        &BTreeSet::new(),
    )
    .expect("prepare valid journal");
    let valid = kura.read_lane_geometry_journal().expect("valid journal");
    let mut forged_root = valid.clone();
    forged_root.records[0].updated_lineage_root = Hash::new(b"forged-lineage-root");
    fs::write(kura.lane_geometry_journal_path(), forged_root.encode())
        .expect("write forged lineage root");
    kura.recover_lane_geometry_journal(&extended, &extended_incarnations, &extended_activations)
        .expect_err("lineage-root tampering must invalidate the transition id");
    let mut forged_sequence = valid.clone();
    forged_sequence.records[0].transition_sequence = forged_sequence.records[0]
        .transition_sequence
        .checked_add(1)
        .expect("test transition sequence");
    fs::write(kura.lane_geometry_journal_path(), forged_sequence.encode())
        .expect("write forged transition sequence");
    kura.recover_lane_geometry_journal(&extended, &extended_incarnations, &extended_activations)
        .expect_err("transition-sequence tampering must invalidate the transition id");
    let mut forged_height = valid.clone();
    forged_height.records[0].transition_height = forged_height.records[0]
        .transition_height
        .checked_add(1)
        .expect("test transition height");
    fs::write(kura.lane_geometry_journal_path(), forged_height.encode())
        .expect("write forged transition height");
    kura.recover_lane_geometry_journal(&extended, &extended_incarnations, &extended_activations)
        .expect_err("transition-height tampering must invalidate the transition id");
    fs::write(kura.lane_geometry_journal_path(), valid.encode()).expect("restore valid journal");
    let mut forged = valid;
    forged.records[0].operations[0].archived_blocks_path = "../escape".to_owned();
    fs::write(kura.lane_geometry_journal_path(), forged.encode())
        .expect("write forged journal bytes");
    kura.recover_lane_geometry_journal(&extended, &extended_incarnations, &extended_activations)
        .expect_err("forged archive path must fail closed");
}
#[test]
fn recovery_rejects_noncontiguous_phase_frontiers() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let kura = open_kura(&root, &initial_and_extended_configs().0);
    let _fixture = prepare_retired_geometry_archive(&kura, &root);
    let valid = kura
        .read_lane_geometry_journal()
        .expect("two-transition published journal");
    assert_eq!(valid.records.len(), 2);
    for (_label, phases, expected_message) in [
        (
            "published-after-rollback",
            [
                LaneGeometryPhase::RolledBack,
                LaneGeometryPhase::CatalogPublished,
            ],
            "lane geometry journal phases do not form a durable applied frontier",
        ),
        (
            "multiple-uncertain-boundaries",
            [LaneGeometryPhase::Intent, LaneGeometryPhase::FilesApplied],
            "lane geometry journal has more than one uncertain transition boundary",
        ),
    ] {
        let mut forged = valid.clone();
        for (record, phase) in forged.records.iter_mut().zip(phases) {
            record.phase = phase;
        }
        fs::write(kura.lane_geometry_journal_path(), forged.encode())
            .expect("write phase-frontier forgery");
        let error = kura
            .read_lane_geometry_journal()
            .expect_err("impossible phase topology must fail closed");
        assert_geometry_io_error(&error, ErrorKind::InvalidData, expected_message);
    }
}
#[test]
fn recovery_rejects_both_branch_v5_journal_layouts_without_migration() {
    #[derive(norito::NoritoSchema)]
    #[norito_schema(
        name = "iroha_core::kura::lane_geometry::tests::recovery_rejects_both_branch_v5_journal_layouts_without_migration::HeightCursorJournalV5"
    )]
    #[derive(Encode)]
    struct HeightCursorJournalV5 {
        version: u8,
        configured_catalog_hash: Option<Hash>,
        configured_primary_binding: Option<LaneGeometryBinding>,
        checkpoint: Option<LaneGeometrySnapshotCheckpoint>,
        pending_archive_gc: Vec<LaneGeometryPendingArchiveGc>,
        records: Vec<LaneGeometryIntent>,
    }
    #[derive(norito::NoritoSchema)]
    #[norito_schema(
        name = "iroha_core::kura::lane_geometry::tests::recovery_rejects_both_branch_v5_journal_layouts_without_migration::LineageJournalV5"
    )]
    #[derive(Encode)]
    struct LineageJournalV5 {
        version: u8,
        configured_catalog_hash: Option<Hash>,
        // These containers are empty below, so their bytes exactly match the lineage
        // branch's checkpoint and transition container encodings.
        checkpoint: Option<LaneGeometrySnapshotCheckpoint>,
        pending_archive_gc: Vec<LaneGeometryPendingArchiveGc>,
        records: Vec<LaneGeometryIntent>,
    }
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (initial, _) = initial_and_extended_configs();
    let (initial_incarnations, initial_activations) = initial_geometry();
    let kura = open_kura(&root, &initial);
    let obsolete_layouts = [
        (
            "height-cursor v5",
            HeightCursorJournalV5 {
                version: 5,
                configured_catalog_hash: None,
                configured_primary_binding: None,
                checkpoint: None,
                pending_archive_gc: Vec::new(),
                records: Vec::new(),
            }
            .encode(),
        ),
        (
            "lineage v5",
            LineageJournalV5 {
                version: 5,
                configured_catalog_hash: Some(Hash::new(b"lineage-v5")),
                checkpoint: None,
                pending_archive_gc: Vec::new(),
                records: Vec::new(),
            }
            .encode(),
        ),
    ];
    for (name, bytes) in obsolete_layouts {
        let journal_path = kura.lane_geometry_journal_path();
        fs::write(&journal_path, &bytes).expect("write obsolete v5 journal");
        let error = match kura.recover_lane_geometry_journal(
            &initial,
            &initial_incarnations,
            &initial_activations,
        ) {
            Ok(()) => panic!("{name} must not be migrated to the current journal"),
            Err(error) => error,
        };
        assert_eq!(
            fs::read(&journal_path).expect("read rejected v5 journal"),
            bytes,
            "recovery must leave the rejected {name} bytes untouched"
        );
        if name == "height-cursor v5" {
            assert_kura_io_error(
                &error,
                std::io::ErrorKind::InvalidData,
                &format!("unsupported lane geometry journal version 5; expected {JOURNAL_VERSION}"),
            );
        }
    }
}
#[test]
fn recovery_rejects_prior_lane_geometry_checkpoint_version() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let kura = open_kura(&root, &initial_and_extended_configs().0);
    let fixture = prepare_retired_geometry_archive(&kura, &root);
    checkpoint_retired_geometry(&kura, &fixture, 20).expect("create current rooted checkpoint");
    let mut prior = kura
        .read_lane_geometry_journal()
        .expect("read rooted checkpoint journal");
    let checkpoint = prior.checkpoint.as_mut().expect("checkpoint exists");
    checkpoint.version = CHECKPOINT_VERSION - 1;
    checkpoint.commitment = geometry_checkpoint_commitment(checkpoint);
    fs::write(kura.lane_geometry_journal_path(), prior.encode())
        .expect("write prior-version checkpoint");
    let error = kura
        .recover_lane_geometry_journal(
            &fixture.initial,
            &fixture.initial_incarnations,
            &fixture.initial_activations,
        )
        .expect_err("a prior checkpoint version must not be interpreted as the current layout");
    assert_kura_io_error(
        &error,
        std::io::ErrorKind::InvalidData,
        "lane geometry checkpoint commitment, catalog, height, block hash, or activation is invalid",
    );
}
#[test]
fn configured_catalog_preflight_persists_baseline_before_any_lane_path() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let configured_a = configured_primary_catalog("crash-a");
    let configured_b = configured_primary_catalog("crash-b");
    let lane_config_a = RuntimeLaneConfig::from_catalog(&configured_a);
    let lane_config_b = RuntimeLaneConfig::from_catalog(&configured_b);
    let config = kura_config(&root);
    Kura::fail_after_configured_catalog_preflight_for_test(&root);
    let error = Kura::new_with_configured_lane_catalog(&config, &lane_config_a, &configured_a)
        .expect_err("injected crash must stop immediately after baseline establishment");
    assert!(matches!(
        error,
        Error::IO(ref source, _) if source.kind() == ErrorKind::Interrupted
    ));
    assert_lane_paths_absent(&root, &lane_config_a);
    let journal = decode_exact::<LaneGeometryJournal>(
        &fs::read(root.join(JOURNAL_FILE_NAME)).expect("durable baseline journal"),
    )
    .expect("decode durable baseline journal");
    assert_eq!(
        journal.configured_catalog_hash,
        Some(LaneLifecycleParameterV1::catalog_hash(&configured_a))
    );
    Kura::new_with_configured_lane_catalog(&config, &lane_config_b, &configured_b)
        .expect_err("a reconstructed process must reject a different configured catalog");
    assert_lane_paths_absent(&root, &lane_config_b);
    Kura::new_with_configured_lane_catalog(&config, &lane_config_a, &configured_a)
        .expect("the exact configured catalog must resume after the crash boundary");
}
#[cfg(unix)]
#[test]
fn authenticated_primary_admission_rejects_block_path_symlink_before_external_write() {
    use std::os::unix::fs::symlink;
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let outside = temp.path().join("outside-blocks");
    fs::create_dir_all(&outside).expect("outside directory");
    let configured = configured_primary_catalog("primary-block-symlink");
    let lane_config = RuntimeLaneConfig::from_catalog(&configured);
    Kura::establish_or_verify_configured_lane_catalog_baseline(
        &root,
        LaneLifecycleParameterV1::catalog_hash(&configured),
    )
    .expect("establish configured-catalog baseline");
    let (kura, _) =
        Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
            .expect("open canonical storage before authenticated lane admission");
    kura.bind_lane_storage_network(geometry_fixture_network_id())
        .expect("bind explicit fixture network");
    let incarnation = Hash::prehashed([0xA6; Hash::LENGTH]);
    let incarnations = BTreeMap::from([(LaneId::SINGLE, incarnation)]);
    let activations = BTreeMap::from([(LaneId::SINGLE, 0)]);
    let blocks = geometry_fixture_blocks(&kura, lane_config.primary(), &incarnations, &activations);
    let merge = geometry_fixture_merge(&kura, lane_config.primary(), &incarnations, &activations);
    fs::create_dir_all(blocks.parent().expect("block parent")).expect("block parent");
    symlink(&outside, &blocks).expect("configured primary block symlink");
    kura.establish_or_verify_configured_primary_geometry_anchor(
        lane_config.primary(),
        incarnation,
        LaneLifecycleParameterV1::catalog_hash(&configured),
    )
    .expect_err("exact primary admission must reject the substituted path before opening it");
    assert!(blocks.is_symlink());
    assert_eq!(
        fs::read_dir(&outside).expect("outside directory").count(),
        0,
        "preflight rejection must not create block-store files outside the Kura root"
    );
    assert!(!merge.exists());
}
#[cfg(unix)]
#[test]
fn authenticated_primary_admission_rejects_merge_path_symlink_before_external_write() {
    use std::os::unix::fs::symlink;
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let outside = temp.path().join("outside-merge.log");
    fs::write(&outside, b"operator-owned").expect("outside merge sentinel");
    let configured = configured_primary_catalog("primary-merge-symlink");
    let lane_config = RuntimeLaneConfig::from_catalog(&configured);
    Kura::establish_or_verify_configured_lane_catalog_baseline(
        &root,
        LaneLifecycleParameterV1::catalog_hash(&configured),
    )
    .expect("establish configured-catalog baseline");
    let (kura, _) =
        Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
            .expect("open canonical storage before authenticated lane admission");
    kura.bind_lane_storage_network(geometry_fixture_network_id())
        .expect("bind explicit fixture network");
    let incarnation = Hash::prehashed([0xA6; Hash::LENGTH]);
    let incarnations = BTreeMap::from([(LaneId::SINGLE, incarnation)]);
    let activations = BTreeMap::from([(LaneId::SINGLE, 0)]);
    let blocks = geometry_fixture_blocks(&kura, lane_config.primary(), &incarnations, &activations);
    let merge = geometry_fixture_merge(&kura, lane_config.primary(), &incarnations, &activations);
    fs::create_dir_all(merge.parent().expect("merge parent")).expect("merge parent");
    symlink(&outside, &merge).expect("configured primary merge symlink");
    kura.establish_or_verify_configured_primary_geometry_anchor(
        lane_config.primary(),
        incarnation,
        LaneLifecycleParameterV1::catalog_hash(&configured),
    )
    .expect_err("exact primary admission must reject the substituted path before opening it");
    assert!(merge.is_symlink());
    assert_eq!(
        fs::read(&outside).expect("outside sentinel"),
        b"operator-owned"
    );
    assert!(!blocks.exists());
}
#[cfg(unix)]
#[test]
fn configured_primary_preflight_rejects_core_block_file_symlinks_before_external_write() {
    use std::os::unix::fs::symlink;
    for file_name in [
        INDEX_FILE_NAME,
        DATA_FILE_NAME,
        HASHES_FILE_NAME,
        COUNT_FILE_NAME,
    ] {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let outside = temp.path().join(format!("outside-{file_name}"));
        fs::write(&outside, b"operator-owned-block-file").expect("outside sentinel");
        let configured = configured_primary_catalog("child-link");
        let lane_config = RuntimeLaneConfig::from_catalog(&configured);
        let baseline = LaneLifecycleParameterV1::catalog_hash(&configured);
        let incarnation = Hash::prehashed([0xA7; Hash::LENGTH]);
        let (kura, _) =
            Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
                .expect("open authenticated configured Kura");
        kura.bind_lane_storage_network(geometry_fixture_network_id())
            .expect("bind explicit fixture network before geometry authority");
        kura.establish_or_verify_configured_primary_geometry_anchor(
            lane_config.primary(),
            incarnation,
            baseline,
        )
        .expect("bind configured primary");
        let child = geometry_fixture_blocks(
            &kura,
            lane_config.primary(),
            &BTreeMap::from([(LaneId::SINGLE, incarnation)]),
            &BTreeMap::from([(LaneId::SINGLE, 0)]),
        )
        .join(file_name);
        drop(kura);
        fs::remove_file(&child).expect("remove core block file before symlink injection");
        symlink(&outside, &child).expect("inject core block-file symlink");
        Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
            .expect_err("configured primary descendants must be rejected before BlockStore opens");
        assert!(child.is_symlink());
        assert_eq!(
            fs::read(&outside).expect("outside sentinel retained"),
            b"operator-owned-block-file",
            "outside target changed for {file_name}"
        );
    }
}
#[cfg(unix)]
#[test]
fn configured_primary_preflight_rejects_retired_root_artifact_symlink() {
    use std::os::unix::fs::symlink;
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let outside = temp.path().join("outside-retired-artifact");
    fs::write(&outside, b"operator-owned-retired-artifact").expect("outside sentinel");
    let configured = configured_primary_catalog("root-sidecar-link");
    let lane_config = RuntimeLaneConfig::from_catalog(&configured);
    let baseline = LaneLifecycleParameterV1::catalog_hash(&configured);
    let (kura, _) =
        Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
            .expect("open authenticated configured Kura");
    kura.bind_lane_storage_network(geometry_fixture_network_id())
        .expect("bind explicit fixture network before geometry authority");
    kura.establish_or_verify_configured_primary_geometry_anchor(
        lane_config.primary(),
        Hash::prehashed([0xA8; Hash::LENGTH]),
        baseline,
    )
    .expect("bind configured primary");
    drop(kura);
    let sidecar_temp = root.join("commit-rosters.norito.tmp");
    symlink(&outside, &sidecar_temp).expect("inject retired-artifact symlink");
    Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
        .expect_err("retired root artifact must fail before configured-lane reconciliation");
    assert!(sidecar_temp.is_symlink());
    assert_eq!(
        fs::read(&outside).expect("outside sentinel retained"),
        b"operator-owned-retired-artifact"
    );
}
#[test]
fn configured_primary_preflight_rejects_foreign_marker_before_kura_reconciliation() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let configured = configured_primary_catalog("primary-marker");
    let lane_config = RuntimeLaneConfig::from_catalog(&configured);
    let (kura, _) =
        Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
            .expect("open configured Kura");
    kura.bind_lane_storage_network(geometry_fixture_network_id())
        .expect("bind explicit fixture network before geometry authority");
    let incarnation = Hash::prehashed([0xA1; Hash::LENGTH]);
    kura.establish_or_verify_configured_primary_geometry_anchor(
        lane_config.primary(),
        incarnation,
        LaneLifecycleParameterV1::catalog_hash(&configured),
    )
    .expect("bind configured primary");
    let marker_path = geometry_fixture_blocks(
        &kura,
        lane_config.primary(),
        &BTreeMap::from([(LaneId::SINGLE, incarnation)]),
        &BTreeMap::from([(LaneId::SINGLE, 0)]),
    )
    .join(MARKER_FILE_NAME);
    fs::write(
        &marker_path,
        LaneIncarnationMarker {
            version: MARKER_VERSION,
            network_id: geometry_fixture_network_id(),
            dataspace_id: lane_config.primary().dataspace_id,
            lane_id: LaneId::SINGLE,
            incarnation: Hash::prehashed([0xA2; Hash::LENGTH]),
            activation_height: 0,
            move_target_blocks: None,
            move_target_merge: None,
            block_store_digest: Hash::prehashed([0xA4; Hash::LENGTH]),
            merge_log_digest: Hash::prehashed([0xA3; Hash::LENGTH]),
        }
        .encode(),
    )
    .expect("write foreign marker");
    drop(kura);
    Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
        .expect_err("foreign configured-primary marker must fail before Kura reconciliation");
    let marker = decode_exact::<LaneIncarnationMarker>(
        &fs::read(&marker_path).expect("foreign marker retained"),
    )
    .expect("decode retained marker");
    assert_eq!(marker.incarnation, Hash::prehashed([0xA2; Hash::LENGTH]));
}
#[test]
fn configured_catalog_preflight_rejects_nonzero_physical_primary_without_mutation() {
    let temp = TempDir::new().expect("temporary directory");
    let nonzero_root = temp.path().join("nonzero-primary");
    let nonzero_catalog = LaneCatalog::new(
        nonzero!(2_u32),
        vec![ModelLaneConfig {
            id: LaneId::new(1),
            alias: "not-physical-primary".to_owned(),
            ..ModelLaneConfig::default()
        }],
    )
    .expect("sparse nonzero-only catalog");
    let nonzero_config = RuntimeLaneConfig::from_catalog(&nonzero_catalog);
    let error = Kura::new_with_configured_lane_catalog(
        &kura_config(&nonzero_root),
        &nonzero_config,
        &nonzero_catalog,
    )
    .expect_err("authenticated Kura must require physical lane zero");
    assert_geometry_io_error(
        &error,
        ErrorKind::InvalidInput,
        "authenticated configured catalog must contain physical primary lane zero",
    );
    assert!(!nonzero_root.exists());
}
#[test]
fn configured_catalog_preflight_refuses_to_bind_a_nonpristine_root() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    fs::create_dir_all(&root).expect("seed Kura root");
    let sentinel = root.join("operator-ledger-data");
    fs::write(&sentinel, b"must-not-adopt-or-delete").expect("seed foreign ledger data");
    let configured = configured_primary_catalog("pristine-root-required");
    let lane_config = RuntimeLaneConfig::from_catalog(&configured);
    let error =
        Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
            .expect_err("a missing baseline must never bind an existing ledger root");
    assert_geometry_io_error(
        &error,
        ErrorKind::InvalidData,
        "cannot establish a configured-catalog baseline on a non-pristine Kura root",
    );
    assert_eq!(
        fs::read(&sentinel).expect("foreign data retained"),
        b"must-not-adopt-or-delete"
    );
    assert!(!root.join(JOURNAL_FILE_NAME).exists());
    assert!(!root.join(JOURNAL_TEMP_FILE_NAME).exists());
    assert_lane_paths_absent(&root, &lane_config);
}
#[cfg(unix)]
#[test]
fn configured_catalog_admits_only_the_bound_public_reset_storage_marker() {
    use std::os::unix::fs::PermissionsExt as _;

    let temp = TempDir::new().expect("temporary directory");
    let state = temp.path().join("taira-validator-1");
    let root = state.join("storage");
    fs::create_dir_all(&root).expect("create fresh validator storage");
    for path in [&state, &root] {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .expect("retain reset directory custody");
    }
    let marker = PublicResetGeneratedMarkerV1 {
        schema: "iroha.taira.public-reset.generated-path.v1".into(),
        kind: "fresh_state".into(),
        host_slug: "taira-validator-1".into(),
        inventory_sha256: "a".repeat(64),
        authorization_nonce: "b".repeat(32),
        revision: "c".repeat(40),
        created_at_unix_ms: 1,
    };
    let marker_path = |directory: &Path| directory.join(".public-reset-generated-v1.json");
    let write_marker = |directory: &Path, marker: &PublicResetGeneratedMarkerV1| {
        let path = marker_path(directory);
        fs::write(
            &path,
            norito::json::to_json(marker).expect("canonical reset marker"),
        )
        .expect("write reset marker");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .expect("retain reset marker custody");
    };
    write_marker(&state, &marker);
    let mut child = marker.clone();
    child.kind = "fresh_state_entry".into();
    write_marker(&root, &child);
    assert!(exact_public_reset_storage_marker(&root));

    let configured = configured_primary_catalog("public-reset-storage-marker");
    let lane_config = RuntimeLaneConfig::from_catalog(&configured);
    let (kura, _) =
        Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
            .expect("fresh reset storage marker is part of the exact Kura root closure");
    drop(kura);
    assert!(root.join(JOURNAL_FILE_NAME).is_file());
    assert!(marker_path(&root).is_file());

    child.authorization_nonce = "d".repeat(32);
    write_marker(&root, &child);
    assert!(!exact_public_reset_storage_marker(&root));
    child.authorization_nonce = marker.authorization_nonce.clone();
    write_marker(&root, &child);
    fs::set_permissions(marker_path(&root), fs::Permissions::from_mode(0o644))
        .expect("weaken marker mode");
    assert!(!exact_public_reset_storage_marker(&root));
}
#[test]
fn authenticated_primary_restore_heals_missing_lane_artifact_namespace() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let configured_catalog = configured_primary_catalog("authenticated-primary");
    let configured = RuntimeLaneConfig::from_catalog(&configured_catalog);
    let (incarnations, activation_heights) = initial_geometry();
    let configured_catalog_hash = LaneLifecycleParameterV1::catalog_hash(&configured_catalog);
    let (kura, _) = Kura::new_with_configured_lane_catalog(
        &kura_config(&root),
        &configured,
        &configured_catalog,
    )
    .expect("open authenticated configured Kura");
    kura.bind_lane_storage_network(geometry_fixture_network_id())
        .expect("bind explicit fixture network before geometry authority");
    kura.establish_or_verify_configured_primary_geometry_anchor(
        configured.primary(),
        incarnations[&LaneId::SINGLE],
        configured_catalog_hash,
    )
    .expect("authenticate configured primary geometry");
    let bindings = kura
        .geometry_bindings(&configured, &incarnations, &activation_heights)
        .expect("derive authenticated primary binding");
    let lineage_root = unscoped_lineage_root(&bindings);
    let primary_blocks = kura.binding_blocks_path(&bindings[0]);
    let lane_artifacts = Kura::lane_artifact_dir(&primary_blocks);
    if lane_artifacts.exists() {
        fs::remove_dir(&lane_artifacts).expect("remove empty primary artifact namespace");
    }
    assert!(
        !lane_artifacts.exists(),
        "fixture must restore an authenticated primary without its empty artifact namespace"
    );
    kura.restore_lane_segments_with_geometry_at_height_and_lineage_root(
        &configured,
        &incarnations,
        &activation_heights,
        0,
        lineage_root,
    )
    .expect("restore must durably heal the authenticated primary namespace");
    let namespace = Kura::open_bound_progress_directory(&root, &lane_artifacts)
        .expect("healed primary artifact namespace is descriptor-bound");
    assert!(
        kura.geometry_bound_progress_directory_unchanged(&namespace),
        "healed primary artifact namespace must retain its durable identity"
    );
    drop(namespace);
    drop(kura);
    let (reopened, _) = Kura::new_with_configured_lane_catalog(
        &kura_config(&root),
        &configured,
        &configured_catalog,
    )
    .expect("reopen authenticated configured Kura");
    reopened
        .bind_lane_storage_network(geometry_fixture_network_id())
        .expect("bind explicit fixture network before geometry authority");
    reopened
        .restore_lane_segments_with_geometry_at_height_and_lineage_root(
            &configured,
            &incarnations,
            &activation_heights,
            0,
            lineage_root,
        )
        .expect("authenticated namespace healing must be restart-idempotent");
    let namespace = Kura::open_bound_progress_directory(&root, &lane_artifacts)
        .expect("reopened primary artifact namespace is descriptor-bound");
    assert!(
        reopened.geometry_bound_progress_directory_unchanged(&namespace),
        "reopened primary artifact namespace must retain its durable identity"
    );
}
#[test]
fn configured_multilane_startup_defers_secondary_provisioning_to_geometry_journal() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let lane_count = NonZeroU32::new(2).expect("non-zero lane count");
    let primary = ModelLaneConfig::default();
    let secondary = ModelLaneConfig {
        id: LaneId::new(1),
        alias: "configured-secondary".to_owned(),
        ..ModelLaneConfig::default()
    };
    let initial_catalog = LaneCatalog::new(lane_count, vec![primary.clone()])
        .expect("configured startup base catalog");
    let configured_catalog = LaneCatalog::new(lane_count, vec![primary, secondary])
        .expect("configured startup two-lane catalog");
    let initial = RuntimeLaneConfig::from_catalog(&initial_catalog);
    let configured = RuntimeLaneConfig::from_catalog(&configured_catalog);
    let initial_incarnations =
        BTreeMap::from([(LaneId::SINGLE, Hash::prehashed([0x81; Hash::LENGTH]))]);
    let configured_incarnations = BTreeMap::from([
        (LaneId::SINGLE, initial_incarnations[&LaneId::SINGLE]),
        (LaneId::new(1), Hash::prehashed([0x82; Hash::LENGTH])),
    ]);
    let initial_activations = BTreeMap::from([(LaneId::SINGLE, 0)]);
    let configured_activations = BTreeMap::from([(LaneId::SINGLE, 0), (LaneId::new(1), 0)]);
    let secondary_entry = configured.entry(LaneId::new(1)).expect("secondary lane");
    let (kura, _) = Kura::new_with_configured_lane_catalog(
        &kura_config(&root),
        &configured,
        &configured_catalog,
    )
    .expect("open authenticated configured Kura");
    kura.bind_lane_storage_network(geometry_fixture_network_id())
        .expect("bind explicit fixture network before geometry authority");
    let secondary_blocks = geometry_fixture_blocks(
        &kura,
        secondary_entry,
        &configured_incarnations,
        &configured_activations,
    );
    let secondary_merge = geometry_fixture_merge(
        &kura,
        secondary_entry,
        &configured_incarnations,
        &configured_activations,
    );
    kura.establish_or_verify_configured_primary_geometry_anchor(
        initial.primary(),
        initial_incarnations[&LaneId::SINGLE],
        LaneLifecycleParameterV1::catalog_hash(&configured_catalog),
    )
    .expect("bind configured primary before publishing the full catalog");
    assert!(
        !secondary_blocks.exists() && !secondary_merge.exists(),
        "authenticated Kura open must not precreate secondary storage without incarnation evidence"
    );
    assert!(
        kura.lane_storage_entry(LaneId::new(1)).is_err(),
        "authenticated Kura must not advertise an unowned secondary segment"
    );
    kura.apply_lane_geometry_transition(
        &initial,
        &configured,
        &initial_incarnations,
        &configured_incarnations,
        &initial_activations,
        &configured_activations,
        &BTreeSet::new(),
    )
    .expect("journal configured secondary-lane creation");
    kura.mark_lane_geometry_catalog_published(
        &configured,
        &configured_incarnations,
        &configured_activations,
        Some(LaneLifecycleParameterV1::catalog_hash(&configured_catalog)),
    )
    .expect("publish configured secondary-lane geometry");
    let secondary_binding = kura
        .geometry_bindings(
            &configured,
            &configured_incarnations,
            &configured_activations,
        )
        .expect("configured geometry bindings")
        .into_iter()
        .find(|binding| binding.lane_id == LaneId::new(1))
        .expect("secondary geometry binding");
    kura.require_lane_marker(&secondary_binding)
        .expect("secondary storage has the exact authoritative marker");
    assert!(secondary_merge.is_file());
    assert!(kura.lane_storage_entry(LaneId::new(1)).is_ok());
    drop(kura);
    let (reopened, _) = Kura::new_with_configured_lane_catalog(
        &kura_config(&root),
        &configured,
        &configured_catalog,
    )
    .expect("reopen exact configured Kura");
    reopened
        .bind_lane_storage_network(geometry_fixture_network_id())
        .expect("bind explicit fixture network before geometry authority");
    reopened
        .recover_lane_geometry_journal(
            &configured,
            &configured_incarnations,
            &configured_activations,
        )
        .expect("reopen authenticates published configured geometry");
    reopened
        .require_lane_marker(&secondary_binding)
        .expect("reopened secondary marker remains exact");
    fs::remove_dir_all(&secondary_blocks).expect("simulate loss of published secondary blocks");
    fs::remove_file(&secondary_merge).expect("simulate loss of published secondary merge log");
    let error = reopened
        .recover_lane_geometry_journal(
            &configured,
            &configured_incarnations,
            &configured_activations,
        )
        .expect_err("published configured secondary must never be silently recreated empty");
    assert_geometry_io_error(
        &error,
        ErrorKind::NotFound,
        "durable lane instance evidence is missing; refusing empty provisioning",
    );
    assert!(!secondary_blocks.exists());
    assert!(!secondary_merge.exists());
}
#[test]
fn configured_multilane_startup_rejects_unjournaled_secondary_storage() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let lane_count = NonZeroU32::new(2).expect("non-zero lane count");
    let primary = ModelLaneConfig::default();
    let secondary = ModelLaneConfig {
        id: LaneId::new(1),
        alias: "unjournaled-secondary".to_owned(),
        ..ModelLaneConfig::default()
    };
    let initial_catalog = LaneCatalog::new(lane_count, vec![primary.clone()])
        .expect("configured startup base catalog");
    let configured_catalog = LaneCatalog::new(lane_count, vec![primary, secondary])
        .expect("configured startup two-lane catalog");
    let initial = RuntimeLaneConfig::from_catalog(&initial_catalog);
    let configured = RuntimeLaneConfig::from_catalog(&configured_catalog);
    let initial_incarnations =
        BTreeMap::from([(LaneId::SINGLE, Hash::prehashed([0x91; Hash::LENGTH]))]);
    let configured_incarnations = BTreeMap::from([
        (LaneId::SINGLE, initial_incarnations[&LaneId::SINGLE]),
        (LaneId::new(1), Hash::prehashed([0x92; Hash::LENGTH])),
    ]);
    let initial_activations = BTreeMap::from([(LaneId::SINGLE, 0)]);
    let configured_activations = BTreeMap::from([(LaneId::SINGLE, 0), (LaneId::new(1), 0)]);
    let (kura, _) = Kura::new_with_configured_lane_catalog(
        &kura_config(&root),
        &configured,
        &configured_catalog,
    )
    .expect("canonical open must not provision an unowned secondary instance");
    kura.bind_lane_storage_network(geometry_fixture_network_id())
        .expect("bind explicit fixture network");
    kura.establish_or_verify_configured_primary_geometry_anchor(
        initial.primary(),
        initial_incarnations[&LaneId::SINGLE],
        LaneLifecycleParameterV1::catalog_hash(&configured_catalog),
    )
    .expect("authenticate the existing primary before secondary admission");
    let secondary_blocks = geometry_fixture_blocks(
        &kura,
        configured.entry(LaneId::new(1)).expect("secondary lane"),
        &configured_incarnations,
        &configured_activations,
    );
    assert!(!secondary_blocks.exists());
    fs::create_dir_all(&secondary_blocks).expect("seed unjournaled exact secondary namespace");
    let sentinel = secondary_blocks.join("operator-sentinel");
    fs::write(&sentinel, b"must-not-adopt-or-delete").expect("seed unjournaled sentinel");
    let error = kura
        .apply_lane_geometry_transition(
            &initial,
            &configured,
            &initial_incarnations,
            &configured_incarnations,
            &initial_activations,
            &configured_activations,
            &BTreeSet::new(),
        )
        .expect_err("unjournaled secondary storage must not be adopted");
    assert_geometry_io_error(
        &error,
        ErrorKind::AlreadyExists,
        "new lane instance target already contains storage",
    );
    assert_eq!(
        fs::read(&sentinel).expect("unjournaled sentinel retained"),
        b"must-not-adopt-or-delete"
    );
    assert!(
        kura.read_lane_geometry_journal()
            .expect("configured baseline journal")
            .records
            .is_empty(),
        "rejection must precede geometry intent publication"
    );
}
#[test]
fn configured_catalog_preflight_recovers_exact_first_start_temp() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    fs::create_dir_all(&root).expect("Kura root");
    let configured = configured_primary_catalog("temp-recovery");
    let lane_config = RuntimeLaneConfig::from_catalog(&configured);
    let expected = LaneGeometryJournal {
        configured_catalog_hash: Some(LaneLifecycleParameterV1::catalog_hash(&configured)),
        ..LaneGeometryJournal::default()
    };
    fs::write(root.join(JOURNAL_TEMP_FILE_NAME), expected.encode())
        .expect("simulate synced first-start temp before hard-link promotion");
    Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
        .expect("reconstructed process must promote the exact baseline temp");
    assert!(!root.join(JOURNAL_TEMP_FILE_NAME).exists());
    let recovered = decode_exact::<LaneGeometryJournal>(
        &fs::read(root.join(JOURNAL_FILE_NAME)).expect("promoted baseline journal"),
    )
    .expect("decode promoted baseline journal");
    assert_eq!(recovered, expected);
}
#[test]
fn configured_catalog_preflight_cleans_exact_startup_owned_hard_link_temp() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let configured = configured_primary_catalog("link-recovery");
    let lane_config = RuntimeLaneConfig::from_catalog(&configured);
    let baseline = LaneLifecycleParameterV1::catalog_hash(&configured);
    Kura::establish_or_verify_configured_lane_catalog_baseline(&root, baseline)
        .expect("establish baseline before simulated crash");
    let journal_path = root.join(JOURNAL_FILE_NAME);
    fs::hard_link(&journal_path, root.join(JOURNAL_TEMP_FILE_NAME))
        .expect("simulate crash after durable hard-link promotion");
    Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
        .expect("exact startup-owned hard-link temp must be cleaned before lane storage opens");
    assert!(!root.join(JOURNAL_TEMP_FILE_NAME).exists());
    assert!(journal_path.is_file());
}
#[test]
fn configured_catalog_preflight_rejects_unproven_restore_temp() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let configured = configured_primary_catalog("restore-temp");
    let attempted = configured_primary_catalog("restore-must-not-open");
    let attempted_lane_config = RuntimeLaneConfig::from_catalog(&attempted);
    let baseline = LaneLifecycleParameterV1::catalog_hash(&configured);
    Kura::establish_or_verify_configured_lane_catalog_baseline(&root, baseline)
        .expect("establish baseline");
    let journal_path = root.join(JOURNAL_FILE_NAME);
    fs::copy(&journal_path, root.join(JOURNAL_RESTORE_TEMP_FILE_NAME))
        .expect("seed byte-identical but unowned restore temp");
    Kura::new_with_configured_lane_catalog(
        &kura_config(&root),
        &attempted_lane_config,
        &configured,
    )
    .expect_err("byte equality does not prove restore-temp ownership");
    assert_lane_paths_absent(&root, &attempted_lane_config);
    assert!(root.join(JOURNAL_RESTORE_TEMP_FILE_NAME).is_file());
}
#[test]
fn configured_catalog_preflight_discards_uncommitted_restore_temp() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let configured = configured_primary_catalog("restore-temp");
    let lane_config = RuntimeLaneConfig::from_catalog(&configured);
    let baseline = LaneLifecycleParameterV1::catalog_hash(&configured);
    Kura::establish_or_verify_configured_lane_catalog_baseline(&root, baseline)
        .expect("establish baseline");
    let journal_path = root.join(JOURNAL_FILE_NAME);
    let authoritative = fs::read(&journal_path).expect("authoritative journal bytes");
    let root_identity = configured_catalog_store_root_identity(&root).expect("root identity");
    write_initial_configured_catalog_temp(
        &root,
        root_identity,
        &root.join(JOURNAL_RESTORE_TEMP_FILE_NAME),
        b"synced-but-uncommitted-restore-bytes",
    )
    .expect("simulate crash before restore-temp rename");
    Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
        .expect("the final journal is the sole restore commit point");
    assert!(!root.join(JOURNAL_RESTORE_TEMP_FILE_NAME).exists());
    assert_eq!(
        fs::read(&journal_path).expect("journal retained"),
        authoritative
    );
}
#[test]
fn configured_catalog_preflight_discards_different_uncommitted_publication_temp() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let configured = configured_primary_catalog("publication-temp");
    let lane_config = RuntimeLaneConfig::from_catalog(&configured);
    let baseline = LaneLifecycleParameterV1::catalog_hash(&configured);
    Kura::establish_or_verify_configured_lane_catalog_baseline(&root, baseline)
        .expect("establish baseline");
    let journal_path = root.join(JOURNAL_FILE_NAME);
    let authoritative = fs::read(&journal_path).expect("authoritative journal bytes");
    let different = LaneGeometryJournal {
        configured_catalog_hash: Some(Hash::new(b"different-uncommitted-catalog")),
        ..LaneGeometryJournal::default()
    }
    .encode();
    let root_identity = configured_catalog_store_root_identity(&root).expect("root identity");
    write_initial_configured_catalog_temp(
        &root,
        root_identity,
        &root.join(JOURNAL_TEMP_FILE_NAME),
        &different,
    )
    .expect("simulate crash before publication-temp rename");
    Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
        .expect("the final journal is the sole publication commit point");
    assert!(!root.join(JOURNAL_TEMP_FILE_NAME).exists());
    assert_eq!(
        fs::read(&journal_path).expect("journal retained"),
        authoritative
    );
}
#[cfg(unix)]
#[test]
fn configured_catalog_preflight_rejects_reserved_temp_symlink_without_touching_target() {
    use std::os::unix::fs::symlink;
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let outside = temp.path().join("outside-temp-target");
    fs::write(&outside, b"operator-owned").expect("outside sentinel");
    let configured = configured_primary_catalog("reserved-temp-symlink");
    let lane_config = RuntimeLaneConfig::from_catalog(&configured);
    let baseline = LaneLifecycleParameterV1::catalog_hash(&configured);
    Kura::establish_or_verify_configured_lane_catalog_baseline(&root, baseline)
        .expect("establish authoritative baseline");
    let reserved = root.join(JOURNAL_TEMP_FILE_NAME);
    symlink(&outside, &reserved).expect("reserved temp symlink");
    Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
        .expect_err("reserved temp symlinks must never be deleted or followed");
    assert!(reserved.is_symlink());
    assert_eq!(
        fs::read(&outside).expect("outside sentinel"),
        b"operator-owned"
    );
}
#[test]
fn configured_catalog_preflight_rejects_tampered_v6_structure_before_lane_mutation() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let configured = configured_primary_catalog("structural-baseline");
    let lane_config = RuntimeLaneConfig::from_catalog(&configured);
    let baseline = LaneLifecycleParameterV1::catalog_hash(&configured);
    Kura::establish_or_verify_configured_lane_catalog_baseline(&root, baseline)
        .expect("establish current baseline");
    let journal_path = root.join(JOURNAL_FILE_NAME);
    let mut journal = decode_exact::<LaneGeometryJournal>(
        &fs::read(&journal_path).expect("read valid baseline journal"),
    )
    .expect("decode valid baseline journal");
    let previous_catalog = Hash::new(b"forged previous catalog");
    let previous_lineage_root = Hash::new(b"forged previous lineage");
    let updated_catalog = Hash::new(b"forged updated catalog");
    let updated_lineage_root = Hash::new(b"forged updated lineage");
    journal.records.push(LaneGeometryIntent {
        transition_id: geometry_transition_id(
            0,
            0,
            previous_catalog,
            previous_lineage_root,
            updated_catalog,
            updated_lineage_root,
        ),
        transition_sequence: 0,
        transition_height: 0,
        previous_catalog,
        previous_lineage_root,
        updated_catalog,
        updated_lineage_root,
        previous_bindings: Vec::new(),
        updated_bindings: Vec::new(),
        phase: LaneGeometryPhase::Intent,
        operations: Vec::new(),
    });
    fs::write(&journal_path, journal.encode()).expect("write decodable structural forgery");
    Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
        .expect_err("correct baseline must not mask a structurally malformed journal");
    assert_lane_paths_absent(&root, &lane_config);
}
#[test]
fn configured_catalog_preflight_rejects_version_mismatch_before_lane_mutation() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let configured = configured_primary_catalog("version-baseline");
    let lane_config = RuntimeLaneConfig::from_catalog(&configured);
    let baseline = LaneLifecycleParameterV1::catalog_hash(&configured);
    Kura::establish_or_verify_configured_lane_catalog_baseline(&root, baseline)
        .expect("establish current baseline");
    let journal_path = root.join(JOURNAL_FILE_NAME);
    let mut journal = decode_exact::<LaneGeometryJournal>(
        &fs::read(&journal_path).expect("read valid baseline journal"),
    )
    .expect("decode valid baseline journal");
    journal.version = JOURNAL_VERSION.saturating_add(1);
    fs::write(&journal_path, journal.encode()).expect("write unsupported journal version");
    Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
        .expect_err("unsupported journal version must fail at the startup boundary");
    assert_lane_paths_absent(&root, &lane_config);
}
#[cfg(unix)]
#[test]
fn configured_catalog_preflight_rejects_journal_derived_symlink_before_lane_mutation() {
    use std::os::unix::fs::symlink;
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let configured = LaneCatalog::default();
    let (initial, extended) = initial_and_extended_configs();
    let (initial_incarnations, initial_activations) = initial_geometry();
    let (extended_incarnations, extended_activations) = extended_geometry();
    let (kura, _) =
        Kura::new_with_configured_lane_catalog(&kura_config(&root), &initial, &configured)
            .expect("establish valid configured startup");
    kura.bind_lane_storage_network(geometry_fixture_network_id())
        .expect("bind explicit fixture network before geometry authority");
    kura.establish_or_verify_configured_primary_geometry_anchor(
        initial.primary(),
        initial_incarnations[&LaneId::SINGLE],
        LaneLifecycleParameterV1::catalog_hash(&configured),
    )
    .expect("authenticate primary before a transition can reference it");
    kura.apply_lane_geometry_transition(
        &initial,
        &extended,
        &initial_incarnations,
        &extended_incarnations,
        &initial_activations,
        &extended_activations,
        &BTreeSet::new(),
    )
    .expect("persist a valid journal-derived archive path");
    let journal = kura
        .read_lane_geometry_journal()
        .expect("transition journal");
    let binding = journal.records[0].operations[0]
        .updated
        .as_ref()
        .expect("created immutable instance binding");
    let link = kura.binding_blocks_path(binding);
    let displaced = link.with_extension("displaced");
    fs::rename(&link, &displaced).expect("retain the original journal-owned instance");
    let outside = temp.path().join("outside");
    fs::create_dir(&outside).expect("outside directory");
    fs::write(outside.join("operator-data"), b"retain").expect("outside sentinel");
    symlink(&outside, &link).expect("inject journal-derived immutable-path symlink");
    drop(kura);
    Kura::new_with_configured_lane_catalog(&kura_config(&root), &initial, &configured)
        .expect_err("journal-derived symlink must fail before opening attempted lane storage");
    assert!(link.is_symlink());
    assert!(displaced.is_dir());
    assert_eq!(fs::read(outside.join("operator-data")).unwrap(), b"retain");
}
#[cfg(unix)]
#[test]
fn configured_catalog_preflight_rejects_journal_symlink_before_lane_mutation() {
    use std::os::unix::fs::symlink;
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let configured = configured_primary_catalog("journal-symlink-baseline");
    let baseline = LaneLifecycleParameterV1::catalog_hash(&configured);
    Kura::establish_or_verify_configured_lane_catalog_baseline(&root, baseline)
        .expect("establish valid configured baseline");
    let journal_path = root.join(JOURNAL_FILE_NAME);
    let outside_journal = temp.path().join("outside-journal.norito");
    fs::rename(&journal_path, &outside_journal).expect("move journal outside Kura root");
    symlink(&outside_journal, &journal_path).expect("replace journal with a symlink");
    let lane_config = RuntimeLaneConfig::from_catalog(&configured);
    Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
        .expect_err("configured-catalog journal symlink must fail closed");
    assert_lane_paths_absent(&root, &lane_config);
    assert!(journal_path.is_symlink());
    assert!(outside_journal.is_file());
}
#[cfg(unix)]
#[test]
fn configured_catalog_preflight_rejects_journal_identity_swap_before_lane_mutation() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let configured = configured_primary_catalog("identity-baseline");
    let baseline = LaneLifecycleParameterV1::catalog_hash(&configured);
    Kura::establish_or_verify_configured_lane_catalog_baseline(&root, baseline)
        .expect("establish valid configured baseline");
    let journal_path = root.join(JOURNAL_FILE_NAME);
    fs::copy(&journal_path, root.join(JOURNAL_IDENTITY_SWAP_FILE_NAME))
        .expect("prepare same-content replacement inode");
    Kura::replace_configured_catalog_journal_after_open_for_test(&root);
    let lane_config = RuntimeLaneConfig::from_catalog(&configured);
    Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
        .expect_err("journal identity replacement during read must fail closed");
    assert_lane_paths_absent(&root, &lane_config);
    assert!(root.join(JOURNAL_IDENTITY_DISPLACED_FILE_NAME).is_file());
}
