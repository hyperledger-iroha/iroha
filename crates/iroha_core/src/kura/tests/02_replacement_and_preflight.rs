#[test]
fn partial_stage_discard_recovers_committed_replacement_before_returning() {
    let (_temp_dir, config) = kura_storage_fixture("create Kura root", BLOCKS_IN_MEMORY);
    let replacement_hash = {
        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("open Kura");
        let original = DummyBlocks::new().next();
        kura.store_block(Arc::clone(&original))
            .expect("store original block");
        let blocks_dir = kura.active_blocks_dir.lock().clone();
        kura.persist_retained_block_record(&blocks_dir, original.hash(), original.as_ref())
            .expect("persist original retained record");
        let replacement: Arc<SignedBlock> = Arc::new(
            ValidBlock::new_dummy_and_modify_header(checked_keypair().private_key(), |header| {
                header.set_height(nonzero!(1_u64));
                header.set_prev_block_hash(None);
                header.set_view_change_index(header.view_change_index().saturating_add(1));
            })
            .into(),
        );
        let replacement_hash = replacement.hash();
        kura.fail_retained_rewrite_discard_after_for_tests(0);
        kura.replace_top_block(Arc::clone(&replacement))
            .expect("published replacement must report committed success after cleanup recovery");
        assert_eq!(
            kura.get_durable_block_hash(nonzero!(1_usize)),
            Some(replacement_hash),
            "replacement must be durable when the public call returns"
        );
        assert_eq!(
            kura.get_block_hash(nonzero!(1_usize)),
            Some(replacement_hash)
        );
        assert_eq!(
            kura.get_block(nonzero!(1_usize)).as_deref(),
            Some(replacement.as_ref()),
            "in-memory body must match the published durable replacement"
        );
        let durable_replacement = {
            let mut store = kura.block_store.lock();
            read_block(&mut store, 0).expect("decode durable replacement body")
        };
        assert_eq!(durable_replacement, *replacement);
        let staging_directory = Kura::retained_block_rewrite_staging_dir_for(&blocks_dir);
        assert!(
            !staging_directory.exists(),
            "one-shot cleanup failure must be recovered before returning"
        );
        kura.replace_top_block(Arc::clone(&replacement))
            .expect("idempotent retry after recovered cleanup");
        let successor: Arc<SignedBlock> = Arc::new(
            ValidBlock::new_dummy_and_modify_header(checked_keypair().private_key(), |header| {
                header.set_height(nonzero!(2_u64));
                header.set_prev_block_hash(Some(replacement_hash));
            })
            .into(),
        );
        kura.store_block(Arc::clone(&successor))
            .expect("append after recovered replacement cleanup");
        assert_eq!(
            kura.get_block_hash(nonzero!(2_usize)),
            Some(successor.hash())
        );
        assert_eq!(
            kura.get_durable_block_hash(nonzero!(2_usize)),
            Some(successor.hash())
        );
        assert_eq!(
            kura.disk_usage_bytes()
                .expect("invalidated accounting rescans after partial discard"),
            kura.kura_total_disk_usage_bytes()
                .expect("exact usage after partial discard")
        );
        replacement_hash
    };
    let (reopened, _) = Kura::new(&config, &RuntimeLaneConfig::default())
        .expect("restart resolves partially discarded rewrite stage");
    assert_eq!(
        reopened.get_durable_block_hash(nonzero!(1_usize)),
        Some(replacement_hash)
    );
    assert_eq!(reopened.blocks_count(), 2);
    assert!(!reopened.retained_block_record_path(1).exists());
    assert!(
        !Kura::retained_block_rewrite_staging_dir_for(&reopened.active_blocks_dir.lock().clone())
            .exists()
    );
}
#[test]
fn persistent_retained_cleanup_failure_poison_gates_committed_rewrite() {
    let (_temp_dir, config) = kura_storage_fixture("create Kura root", BLOCKS_IN_MEMORY);
    let replacement = {
        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("open Kura");
        let original = DummyBlocks::new().next();
        let original_hash = original.hash();
        kura.store_block(Arc::clone(&original))
            .expect("store original block");
        let blocks_dir = kura.active_blocks_dir.lock().clone();
        kura.persist_retained_block_record(&blocks_dir, original_hash, original.as_ref())
            .expect("persist retained original record");
        let replacement: Arc<SignedBlock> = Arc::new(
            ValidBlock::new_dummy_and_modify_header(checked_keypair().private_key(), |header| {
                header.set_height(nonzero!(1_u64));
                header.set_prev_block_hash(None);
                header.set_view_change_index(header.view_change_index().saturating_add(1));
            })
            .into(),
        );
        let replacement_hash = replacement.hash();
        kura.fail_retained_rewrite_discard_after_for_tests(0);
        kura.fail_next_retained_rewrite_recovery_for_tests();
        let error = kura
            .replace_top_block(Arc::clone(&replacement))
            .expect_err("unresolved committed cleanup must never report success");
        assert!(matches!(
            error,
            Error::CanonicalBlockCommittedRecoveryRequired { .. }
        ));
        assert!(error.requires_restart_recovery());
        assert!(kura.canonical_storage_poisoned.load(Ordering::Acquire));
        assert_eq!(
            kura.block_data.lock().first().map(|(hash, _)| *hash),
            Some(original_hash),
            "the live in-memory image must remain at its pre-publication state"
        );
        assert_eq!(
            Kura::read_durable_hash_at_height(&mut kura.block_store.lock(), 1)
                .expect("read durable replacement while poisoned"),
            Some(replacement_hash)
        );
        assert!(
            Kura::retained_block_rewrite_staging_dir_for(&blocks_dir).exists(),
            "the durable cleanup stage must remain for startup recovery"
        );
        assert!(matches!(
            kura.replace_top_block(Arc::clone(&replacement)),
            Err(Error::CanonicalStoragePoisoned)
        ));
        replacement
    };
    let (reopened, count) = Kura::new(&config, &RuntimeLaneConfig::default())
        .expect("restart must finish the marker-selected retained cleanup");
    assert_eq!(count.0, 1);
    assert_eq!(
        reopened.get_block(nonzero!(1_usize)).as_deref(),
        Some(replacement.as_ref())
    );
    assert!(!reopened.retained_block_record_path(1).exists());
    assert!(
        !Kura::retained_block_rewrite_staging_dir_for(&reopened.active_blocks_dir.lock().clone())
            .exists()
    );
    assert!(!reopened.canonical_storage_poisoned.load(Ordering::Acquire));
}
#[test]
fn partial_multi_height_stage_discard_keeps_public_prune_coherent_and_appendable() {
    let (_temp_dir, _config, kura) = kura_root_fixture(BLOCKS_IN_MEMORY);
    let blocks = store_dummy_block_arcs(&kura, 4);
    let blocks_dir = kura.active_blocks_dir.lock().clone();
    for block in blocks.iter().skip(1) {
        kura.persist_retained_block_record(&blocks_dir, block.hash(), block.as_ref())
            .expect("persist retained rewrite fixture");
    }
    assert!(
        (2..=4).all(|height| kura.retained_block_record_path(height).is_file()),
        "fixture must stage multiple retained heights"
    );
    kura.fail_retained_rewrite_discard_after_for_tests(0);
    kura.prune_to_height(1)
        .expect("published prune must recover partial retained cleanup and report success");
    assert_eq!(kura.blocks_count(), 1);
    assert_eq!(kura.exact_durable_blocks_count().unwrap(), 1);
    assert_eq!(
        kura.get_block_hash(nonzero!(1_usize)),
        Some(blocks[0].hash())
    );
    assert_eq!(
        kura.get_durable_block_hash(nonzero!(1_usize)),
        Some(blocks[0].hash())
    );
    assert_eq!(
        kura.get_block(nonzero!(1_usize)).as_deref(),
        Some(blocks[0].as_ref())
    );
    assert!(kura.get_block(nonzero!(2_usize)).is_none());
    assert!(kura.get_durable_block_hash(nonzero!(2_usize)).is_none());
    assert!((2..=4).all(|height| !kura.retained_block_record_path(height).exists()));
    assert!(
        !Kura::retained_block_rewrite_staging_dir_for(&blocks_dir).exists(),
        "in-process recovery must fully resolve a partially discarded multi-height stage"
    );
    let durable_genesis = {
        let mut store = kura.block_store.lock();
        read_block(&mut store, 0).expect("decode durable retained prefix")
    };
    assert_eq!(durable_genesis, *blocks[0]);
    kura.store_block(Arc::clone(&blocks[1]))
        .expect("append canonical successor after recovered public prune");
    assert_eq!(kura.blocks_count(), 2);
    assert_eq!(kura.exact_durable_blocks_count().unwrap(), 2);
    assert_eq!(
        kura.get_block_hash(nonzero!(2_usize)),
        Some(blocks[1].hash())
    );
    assert_eq!(
        kura.get_durable_block_hash(nonzero!(2_usize)),
        Some(blocks[1].hash())
    );
    assert_eq!(
        kura.get_block(nonzero!(2_usize)).as_deref(),
        Some(blocks[1].as_ref())
    );
    let durable_successor = {
        let mut store = kura.block_store.lock();
        read_block(&mut store, 1).expect("decode durable successor")
    };
    assert_eq!(durable_successor, *blocks[1]);
    assert_eq!(
        kura.disk_usage_bytes()
            .expect("refresh total usage after recovered prune and append"),
        kura.kura_total_disk_usage_bytes()
            .expect("scan exact total usage after recovered prune and append")
    );
}
#[test]
fn v2_finality_durably_archives_sccp_before_body_eviction_and_restart() {
    let (_temp_dir, config) = kura_storage_fixture("create Kura root", nonzero!(1_usize));
    let (expected, artifact, expected_header) = {
        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("open Kura");
        let (blocks, payloads) = store_retained_archive_chain(&kura);
        let artifact = v2_finality_artifacts_for_chain(&blocks[..2])[1].clone();
        let expected_header = blocks[1].header();
        assert!(
            !kura.retained_block_record_path(2).exists(),
            "inline non-finalized bodies need no eager archive"
        );
        let _receipt = kura
            .store_v2_finality_artifact(&artifact)
            .expect("persist finality and its SCCP archive");
        assert!(
            kura.retained_block_record_path(2).is_file(),
            "archive must be durable before finality publication returns"
        );
        let (header, archived) = kura
            .retained_sccp_archive(2)
            .expect("read retained SCCP archive")
            .expect("retained SCCP archive exists");
        assert_eq!(header, expected_header);
        assert_eq!(archived.len(), 2);
        for (index, (projection, payload)) in archived.iter().zip(&payloads).enumerate() {
            assert_eq!(projection.commitment_index, index as u32);
            assert_eq!(&projection.payload, payload);
        }
        assert!(
            archived[0].commitment.message_id > archived[1].commitment.message_id,
            "fixture commitment order deliberately differs from replay-key ordering"
        );
        let (_, payload_len) = advertise_required_replicas(&kura, nonzero!(2_usize));
        kura.evict_block_bodies(payload_len)
            .expect("evict the already archived SCCP body");
        {
            let store = kura.block_store.lock();
            store
                .remove_da_block_file(2)
                .expect("make archived SCCP block remote-only");
        }
        assert!(kura.get_block(nonzero!(2_usize)).is_none());
        (archived, artifact, expected_header)
    };
    let (reopened, _) =
        Kura::new(&config, &RuntimeLaneConfig::default()).expect("restart bodyless Kura");
    assert!(reopened.get_block(nonzero!(2_usize)).is_none());
    let (header, recovered_artifact, archived) = reopened
        .v2_finality_artifact_with_archive(2)
        .expect("read bodyless finality and retained SCCP archive")
        .expect("bodyless finality and archive exist");
    assert_eq!(header, expected_header);
    assert_eq!(archived, expected);
    assert_eq!(recovered_artifact, artifact);
}
#[test]
fn v2_finality_store_rejects_header_matching_canonical_wire_substitution() {
    let kura = Kura::blank_kura_for_testing();
    let canonical = store_dummy_block_arcs(&kura, 1)
        .pop()
        .expect("canonical block");
    let mut substituted = canonical.as_ref().clone();
    let substitute_key = KeyPair::try_random().expect("substitute block key");
    let substitute_signature = BlockSignature::new(
        0,
        SignatureOf::try_from_hash(substitute_key.private_key(), substituted.hash())
            .expect("sign substituted canonical header"),
    );
    substituted
        .replace_signatures([substitute_signature].into_iter().collect())
        .expect("replace substituted block signature");
    assert_eq!(substituted.header(), canonical.header());
    assert_ne!(
        Kura::canonical_block_wire_hash(&substituted).expect("substituted wire hash"),
        Kura::canonical_block_wire_hash(&canonical).expect("canonical wire hash")
    );
    let substituted_artifact = v2_finality_artifact_for_block(&substituted);
    assert!(matches!(
        kura.store_v2_finality_artifact(&substituted_artifact),
        Err(Error::V2FinalityPayloadHashMismatch { height: 1 })
    ));
    assert!(
        !kura.retained_block_record_path(1).exists(),
        "a body-substituted artifact must reject before retained-record publication"
    );
    assert!(!kura.v2_finality_artifact_path(1).exists());
}
#[test]
fn retained_wire_hash_tamper_rejects_live_body_bodyless_read_and_restart() {
    let (_temp_dir, config) = kura_storage_fixture("create Kura root", nonzero!(1_usize));
    {
        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("open Kura");
        let blocks = store_dummy_block_arcs(&kura, 4);
        let artifact = v2_finality_artifacts_for_chain(&blocks[..2])[1].clone();
        let _ = kura
            .store_v2_finality_artifact(&artifact)
            .expect("store exact finality and retained block record");
        let retained_path = kura.retained_block_record_path(2);
        let canonical_bytes = std::fs::read(&retained_path).expect("read retained record");
        let mut input = canonical_bytes.as_slice();
        let mut tampered = KuraRetainedBlockRecord::decode_all(&mut input)
            .expect("decode retained record for wire-hash tamper");
        tampered.executed_block_wire_hash =
            Hash::new(b"attacker substituted executed canonical wire");
        let tampered_bytes = tampered.encode();
        std::fs::write(&retained_path, &tampered_bytes)
            .expect("tamper retained canonical-wire hash");
        assert!(matches!(
            kura.v2_finality_artifact_with_archive(2),
            Err(Error::ConflictingRetainedBlockRecord { height: 2 })
        ));
        std::fs::write(&retained_path, &canonical_bytes)
            .expect("restore exact retained record before eviction");
        let height = nonzero!(2_usize);
        let (_, payload_len) = advertise_required_replicas(&kura, height);
        assert!(
            kura.evict_block_bodies(payload_len)
                .expect("evict exact canonical body")
                >= payload_len
        );
        kura.remove_evicted_block_sidecar_for_testing(height)
            .expect("make exact historical body remote-only");
        assert!(kura.get_block(height).is_none());
        std::fs::write(&retained_path, tampered_bytes)
            .expect("tamper bodyless retained canonical-wire hash");
        assert!(matches!(
            kura.v2_finality_artifact_with_archive(2),
            Err(Error::V2FinalityExecutedBlockWireHashMismatch { height: 2 })
        ));
    }
    assert!(matches!(
        Kura::new(&config, &RuntimeLaneConfig::default()),
        Err(Error::V2FinalityExecutedBlockWireHashMismatch { height: 2 })
    ));
}
#[test]
fn coordinated_retained_and_finality_payload_hash_tamper_fails_crypto_and_restart() {
    let (_temp_dir, config) = kura_storage_fixture("create Kura root", nonzero!(1_usize));
    {
        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("open Kura");
        let blocks = store_dummy_block_arcs(&kura, 4);
        let artifact = v2_finality_artifacts_for_chain(&blocks[..2])[1].clone();
        let _ = kura
            .store_v2_finality_artifact(&artifact)
            .expect("store exact finality and retained record");
        let height = nonzero!(2_usize);
        let (_, payload_len) = advertise_required_replicas(&kura, height);
        assert!(
            kura.evict_block_bodies(payload_len)
                .expect("evict exact canonical body")
                >= payload_len
        );
        kura.remove_evicted_block_sidecar_for_testing(height)
            .expect("make exact historical body remote-only");
        let forged_wire_hash = Hash::new(b"coordinated attacker wire hash");
        let retained_path = kura.retained_block_record_path(2);
        let retained_bytes = std::fs::read(&retained_path).expect("read retained record");
        let mut retained_input = retained_bytes.as_slice();
        let mut retained = KuraRetainedBlockRecord::decode_all(&mut retained_input)
            .expect("decode retained record");
        retained.proposal_wire_hash = forged_wire_hash;
        std::fs::write(&retained_path, retained.encode())
            .expect("coordinate retained payload hash tamper");
        let finality_path = kura.v2_finality_artifact_path(2);
        let finality_bytes = std::fs::read(&finality_path).expect("read finality record");
        let mut finality_input = finality_bytes.as_slice();
        let mut finality =
            KuraV2FinalityRecord::decode_all(&mut finality_input).expect("decode finality record");
        finality.artifact.subject.payload_hash = forged_wire_hash;
        finality.artifact.commit_qc.subject.payload_hash = forged_wire_hash;
        std::fs::write(&finality_path, finality.encode())
            .expect("coordinate finality payload hash tamper");
        assert!(matches!(
            kura.v2_finality_artifact_with_archive(2),
            Err(Error::V2FinalityCryptography(_))
        ));
    }
    assert!(matches!(
        Kura::new(&config, &RuntimeLaneConfig::default()),
        Err(Error::V2FinalityCryptography(_))
    ));
}
#[test]
fn retained_sccp_inventory_is_bounded_nonempty_and_fails_closed_on_selected_tamper() {
    let kura = Kura::blank_kura_for_testing();
    let (blocks, _) = store_retained_archive_chain(&kura);
    let blocks_dir = kura.active_blocks_dir.lock().clone();
    for block in &blocks {
        kura.persist_retained_block_record(&blocks_dir, block.hash(), block.as_ref())
            .expect("persist canonical retained inventory fixture");
    }
    assert!(
        kura.retained_nonempty_sccp_archive_inventory_at_or_below(0)
            .expect("zero boundary inventory")
            .is_empty()
    );
    assert!(
        kura.retained_nonempty_sccp_archive_inventory_at_or_below(1)
            .expect("rootless retained record is accepted")
            .is_empty(),
        "valid rootless/empty records must not manufacture inventory entries"
    );
    let through_two = kura
        .retained_nonempty_sccp_archive_inventory_at_or_below(2)
        .expect("inventory through SCCP height");
    assert_eq!(
        through_two,
        vec![RetainedSccpArchiveSummary {
            height: 2,
            block_hash: blocks[1].hash(),
            message_count: 2,
        }]
    );
    assert_eq!(
        kura.retained_nonempty_sccp_archive_inventory_at_or_below(3)
            .expect("rootless suffix remains omitted"),
        through_two
    );
    let suffix_path = kura.retained_block_record_path(4);
    std::fs::write(&suffix_path, b"tampered retained suffix")
        .expect("tamper retained record above WSV boundary");
    assert_eq!(
        kura.retained_nonempty_sccp_archive_inventory_at_or_below(2)
            .expect("Kura suffix above committed boundary must not be decoded"),
        through_two
    );
    assert!(
        kura.retained_nonempty_sccp_archive_inventory_at_or_below(4)
            .is_err(),
        "a tampered retained record inside the selected boundary must fail closed"
    );
}
#[test]
fn failed_finality_publication_keeps_valid_archive_for_exact_retry() {
    let (_temp_dir, _config, kura) = kura_root_fixture(nonzero!(1_usize));
    let (blocks, _) = store_retained_archive_chain(&kura);
    let artifact = v2_finality_artifacts_for_chain(&blocks[..2])[1].clone();
    let enforced_before = kura
        .refresh_disk_usage_bytes()
        .expect("refresh enforced usage before retained evidence");
    let total_before = kura
        .refresh_total_disk_usage_bytes()
        .expect("refresh total usage before retained evidence");
    kura.fail_next_v2_finality_write_for_tests();
    assert!(matches!(
        kura.store_v2_finality_artifact(&artifact),
        Err(Error::IO(error, _)) if error.to_string().contains("injected failure")
    ));
    assert!(!kura.v2_finality_artifact_path(2).exists());
    let retained_path = kura.retained_block_record_path(2);
    let retained_before =
        std::fs::read(&retained_path).expect("archive is durable before injected finality failure");
    let retained_len = u64::try_from(retained_before.len()).expect("record length fits u64");
    assert_eq!(
        kura.kura_disk_usage_bytes()
            .expect("measure enforced usage after archive"),
        enforced_before,
        "immutable safety evidence is visible in total usage but cannot deadlock the evictable budget"
    );
    assert_eq!(
        kura.disk_usage_bytes()
            .expect("cached total usage after archive"),
        total_before.saturating_add(retained_len)
    );
    let (_, archive) = kura
        .retained_sccp_archive(2)
        .expect("validate retained archive after finality failure")
        .expect("retained archive exists");
    assert_eq!(archive.len(), 2);
    let _receipt = kura
        .store_v2_finality_artifact(&artifact)
        .expect("retry finality with exact retained archive");
    let finality_len = std::fs::metadata(kura.v2_finality_artifact_path(2))
        .expect("finality metadata")
        .len();
    assert_eq!(
        std::fs::read(retained_path).expect("read retained archive after retry"),
        retained_before,
        "retry must reuse the immutable archive byte-for-byte"
    );
    let total_after_retry = total_before
        .saturating_add(retained_len)
        .saturating_add(finality_len);
    assert_eq!(
        kura.disk_usage_bytes()
            .expect("cached total usage after finality retry"),
        total_after_retry
    );
    let _receipt = kura
        .store_v2_finality_artifact(&artifact)
        .expect("exact finality repeat remains idempotent");
    assert_eq!(
        kura.disk_usage_bytes()
            .expect("cached total usage after idempotent repeat"),
        total_after_retry,
        "idempotent finality must not double-count either immutable sidecar"
    );
    assert_eq!(
        kura.v2_finality_artifact(2).expect("read retried finality"),
        Some(artifact)
    );
}
#[test]
fn retained_sccp_archive_rejects_gap_omission_swap_overflow_and_rootless_extra() {
    let kura = Kura::blank_kura_for_testing();
    let (blocks, _) = store_retained_archive_chain(&kura);
    let canonical_hash = blocks[1].hash();
    let archive = Kura::retained_sccp_archive_from_block(&blocks[1])
        .expect("construct canonical retained archive");
    assert_eq!(archive.len(), 2);
    let path = kura.retained_block_record_path(2);
    let canonical = KuraRetainedBlockRecord::new(
        blocks[1].header(),
        Kura::canonical_proposal_wire_hash(&blocks[1]).expect("canonical proposal wire hash"),
        Kura::canonical_block_wire_identity(&blocks[1])
            .expect("canonical block wire identity")
            .0,
        Kura::canonical_block_wire_hash(&blocks[1]).expect("canonical block wire hash"),
        None,
        archive.clone(),
    );
    Kura::validate_retained_block_record_at(&path, 2, canonical_hash, &canonical)
        .expect("canonical retained archive validates");
    let mut gap = canonical.clone();
    gap.sccp_archive[1].commitment_index = 2;
    assert!(matches!(
        Kura::validate_retained_block_record_at(&path, 2, canonical_hash, &gap),
        Err(Error::InvalidRetainedSccpArchive { reason, .. }) if reason.contains("not dense")
    ));
    let mut context_tamper = canonical.clone();
    context_tamper.sccp_archive[0]
        .context
        .destination_binding_hash = [0; 32];
    assert!(matches!(
        Kura::validate_retained_block_record_at(
            &path,
            2,
            canonical_hash,
            &context_tamper,
        ),
        Err(Error::InvalidRetainedSccpArchive { reason, .. }) if reason.contains("message 0 is invalid")
    ));
    let mut noncanonical = canonical.clone();
    noncanonical.sccp_archive[0].payload_bytes.push(0);
    assert!(matches!(
        Kura::validate_retained_block_record_at(
            &path,
            2,
            canonical_hash,
            &noncanonical,
        ),
        Err(Error::InvalidRetainedSccpArchive { reason, .. }) if reason.contains("message 0 is invalid")
    ));
    let mut duplicate = canonical.clone();
    let duplicated_context = duplicate.sccp_archive[0].context;
    let duplicated_payload = duplicate.sccp_archive[0].payload_bytes.clone();
    duplicate.sccp_archive[1].context = duplicated_context;
    duplicate.sccp_archive[1].payload_bytes = duplicated_payload;
    assert!(matches!(
        Kura::validate_retained_block_record_at(&path, 2, canonical_hash, &duplicate),
        Err(Error::InvalidRetainedSccpArchive { reason, .. }) if reason.contains("repeats an outbound replay key")
    ));
    let mut omitted = canonical.clone();
    omitted.sccp_archive.pop();
    assert!(matches!(
        Kura::validate_retained_block_record_at(&path, 2, canonical_hash, &omitted),
        Err(Error::InvalidRetainedSccpArchive { reason, .. }) if reason.contains("commitment root")
    ));
    let mut swapped = canonical.clone();
    let first_payload = swapped.sccp_archive[0].payload_bytes.clone();
    let second_payload = swapped.sccp_archive[1].payload_bytes.clone();
    swapped.sccp_archive[0].payload_bytes = second_payload;
    swapped.sccp_archive[1].payload_bytes = first_payload;
    assert!(matches!(
        Kura::validate_retained_block_record_at(&path, 2, canonical_hash, &swapped),
        Err(Error::InvalidRetainedSccpArchive { reason, .. }) if reason.contains("commitment root")
    ));
    let mut overflow = canonical.clone();
    overflow.sccp_archive =
        vec![
            canonical.sccp_archive[0].clone();
            usize::try_from(iroha_data_model::bridge::SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1 + 1,)
                .expect("SCCP bound fits usize")
        ];
    assert!(matches!(
        Kura::validate_retained_block_record_at(&path, 2, canonical_hash, &overflow),
        Err(Error::InvalidRetainedSccpArchive { reason, .. }) if reason.contains("maximum")
    ));
    let rootless_header = blocks[0].header();
    let rootless_extra = KuraRetainedBlockRecord::new(
        rootless_header,
        Kura::canonical_proposal_wire_hash(&blocks[0]).expect("rootless proposal wire hash"),
        Kura::canonical_block_wire_identity(&blocks[0])
            .expect("rootless block wire identity")
            .0,
        Kura::canonical_block_wire_hash(&blocks[0]).expect("rootless block wire hash"),
        None,
        archive,
    );
    assert!(matches!(
        Kura::validate_retained_block_record_at(
            &kura.retained_block_record_path(1),
            1,
            blocks[0].hash(),
            &rootless_extra,
        ),
        Err(Error::InvalidRetainedSccpArchive { reason, .. }) if reason.contains("commitment root")
    ));
}
#[test]
fn retained_sccp_archive_tamper_fails_reader_and_restart_closed() {
    let (_temp_dir, config) = kura_storage_fixture("create Kura root", nonzero!(1_usize));
    {
        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("open Kura");
        let (blocks, _) = store_retained_archive_chain(&kura);
        let artifact = v2_finality_artifacts_for_chain(&blocks[..2])[1].clone();
        let _receipt = kura
            .store_v2_finality_artifact(&artifact)
            .expect("persist archive-backed finality");
        let path = kura.retained_block_record_path(2);
        let bytes = std::fs::read(&path).expect("read retained SCCP record");
        let mut input = bytes.as_slice();
        let mut record =
            KuraRetainedBlockRecord::decode_all(&mut input).expect("decode retained SCCP record");
        record.sccp_archive.clear();
        std::fs::write(&path, record.encode()).expect("omit rooted SCCP archive");
        assert!(matches!(
            kura.retained_sccp_archive(2),
            Err(Error::InvalidRetainedSccpArchive { reason, .. }) if reason.contains("commitment root")
        ));
    }
    assert!(matches!(
        Kura::new(&config, &RuntimeLaneConfig::default()),
        Err(Error::InvalidRetainedSccpArchive { height: 2, reason })
            if reason.contains("commitment root")
    ));
}
#[test]
fn rooted_finality_reader_rejects_deleted_archive_even_while_body_is_inline() {
    let (_temp_dir, config) = kura_storage_fixture("create Kura root", BLOCKS_IN_MEMORY);
    {
        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("open Kura");
        let (blocks, _) = store_retained_archive_chain(&kura);
        let artifact = v2_finality_artifacts_for_chain(&blocks[..2])[1].clone();
        let _receipt = kura
            .store_v2_finality_artifact(&artifact)
            .expect("persist archive-backed finality");
        std::fs::remove_file(kura.retained_block_record_path(2)).expect("delete rooted archive");
        assert!(matches!(
            kura.retained_sccp_archive(2),
            Err(Error::MissingRetainedBlockRecord { height: 2 })
        ));
    }
    let error = match Kura::new(&config, &RuntimeLaneConfig::default()) {
        Ok(_) => panic!("startup must reject the deleted retained header"),
        Err(error) => error,
    };
    assert!(
        matches!(error, Error::MissingRetainedBlockRecord { height: 2 }),
        "unexpected startup error: {error:?}"
    );
}
#[test]
fn unfinalized_block_cannot_become_bodyless_before_finality() {
    let (_temp_dir, config) = kura_storage_fixture("create Kura root", nonzero!(1_usize));
    let artifact = {
        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("open Kura");
        let blocks = store_dummy_block_arcs(&kura, 4);
        let height = nonzero!(2_usize);
        let artifact = v2_finality_artifacts_for_chain(&blocks[..2])[1].clone();
        let (_, payload_len) = advertise_unfinalized_required_replicas(&kura, height);
        assert_eq!(
            kura.evict_block_bodies(payload_len)
                .expect("deny eviction before finality exists"),
            0
        );
        assert!(kura.get_block(height).is_some());
        artifact
    };
    let (reopened, _) =
        Kura::new(&config, &RuntimeLaneConfig::default()).expect("restart inline Kura");
    assert!(reopened.get_block(nonzero!(2_usize)).is_some());
    let _ = reopened
        .store_v2_finality_artifact(&artifact)
        .expect("finalize the still-inline canonical body");
    assert_eq!(
        reopened
            .v2_finality_artifact(2)
            .expect("read finality after inline association"),
        Some(artifact)
    );
}
#[test]
fn retained_header_tamper_fails_finality_read_and_restart_closed() {
    let (_temp_dir, config, kura) = kura_root_fixture(nonzero!(1_usize));
    let blocks = store_dummy_block_arcs(&kura, 4);
    let artifact = v2_finality_artifacts_for_chain(&blocks[..2])[1].clone();
    let _ = kura
        .store_v2_finality_artifact(&artifact)
        .expect("persist signed finality before eviction");
    let height = nonzero!(2_usize);
    let (_, payload_len) = advertise_required_replicas(&kura, height);
    kura.evict_block_bodies(payload_len)
        .expect("evict canonical body");
    {
        let store = kura.block_store.lock();
        store
            .remove_da_block_file(2)
            .expect("make the body remote-only");
    }
    let path = kura.retained_block_record_path(2);
    let substitute: SignedBlock =
        ValidBlock::new_dummy_and_modify_header(checked_keypair().private_key(), |header| {
            header.set_height(nonzero!(2_u64));
            header.set_prev_block_hash(Some(blocks[0].hash()));
            header.set_view_change_index(header.view_change_index().saturating_add(1));
        })
        .into();
    let forged = KuraRetainedBlockRecord::new(
        substitute.header(),
        Kura::canonical_proposal_wire_hash(&substitute).expect("substitute proposal wire hash"),
        Kura::canonical_block_wire_identity(&substitute)
            .expect("substitute block wire identity")
            .0,
        Kura::canonical_block_wire_hash(&substitute).expect("substitute block wire hash"),
        None,
        Vec::new(),
    );
    std::fs::write(&path, forged.encode()).expect("replace retained header with a conflict");
    assert!(matches!(
        kura.store_v2_finality_artifact(&artifact),
        Err(Error::BlockHeightConflict {
            height: 2,
            expected,
            actual,
        }) if expected == blocks[1].hash() && actual == substitute.hash()
    ));
    drop(kura);
    assert!(matches!(
        Kura::new(&config, &RuntimeLaneConfig::default()),
        Err(Error::BlockHeightConflict {
            height: 2,
            expected,
            actual,
        }) if expected == blocks[1].hash() && actual == substitute.hash()
    ));
}
#[test]
fn conflicting_preplanted_retained_header_aborts_eviction_before_index_mutation() {
    let (_temp_dir, _config, kura) = kura_root_fixture(nonzero!(1_usize));
    let blocks = store_dummy_block_arcs(&kura, 4);
    let height = nonzero!(2_usize);
    let artifact = v2_finality_artifacts_for_chain(&blocks[..2])[1].clone();
    let _ = kura
        .store_v2_finality_artifact(&artifact)
        .expect("persist signed finality before retained-record tamper");
    let (_, payload_len) = advertise_required_replicas(&kura, height);
    let substitute: SignedBlock =
        ValidBlock::new_dummy_and_modify_header(checked_keypair().private_key(), |header| {
            header.set_height(nonzero!(2_u64));
            header.set_prev_block_hash(Some(blocks[0].hash()));
            header.set_view_change_index(header.view_change_index().saturating_add(1));
        })
        .into();
    let directory = kura.retained_block_record_dir();
    std::fs::create_dir_all(&directory).expect("create retained-header directory");
    std::fs::write(
        kura.retained_block_record_path(2),
        KuraRetainedBlockRecord::new(
            substitute.header(),
            Kura::canonical_proposal_wire_hash(&substitute).expect("substitute proposal wire hash"),
            Kura::canonical_block_wire_identity(&substitute)
                .expect("substitute block wire identity")
                .0,
            Kura::canonical_block_wire_hash(&substitute).expect("substitute block wire hash"),
            None,
            Vec::new(),
        )
        .encode(),
    )
    .expect("preplant conflicting retained header");
    assert!(matches!(
        kura.evict_block_bodies(payload_len),
        Err(Error::BlockHeightConflict {
            height: 2,
            expected,
            actual,
        }) if expected == blocks[1].hash() && actual == substitute.hash()
    ));
    let index = kura
        .block_store
        .lock()
        .read_block_index(1)
        .expect("read block index after rejected eviction");
    assert!(
        !index.is_evicted(),
        "a conflicting retention path must abort before the body is marked evicted"
    );
}
#[test]
fn startup_rejects_evicted_body_with_deleted_retained_header() {
    let (_temp_dir, config) = kura_storage_fixture("create Kura root", nonzero!(1_usize));
    {
        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("open Kura");
        let blocks = store_dummy_block_arcs(&kura, 4);
        let artifact = v2_finality_artifacts_for_chain(&blocks[..2])[1].clone();
        let _ = kura
            .store_v2_finality_artifact(&artifact)
            .expect("persist signed finality before eviction");
        let (_, payload_len) = advertise_required_replicas(&kura, nonzero!(2_usize));
        kura.evict_block_bodies(payload_len)
            .expect("evict canonical body");
        std::fs::remove_file(kura.retained_block_record_path(2))
            .expect("delete required retained header");
    }
    assert!(matches!(
        Kura::new(&config, &RuntimeLaneConfig::default()),
        Err(Error::MissingRetainedBlockRecord { height: 2 })
    ));
}
#[test]
fn startup_rejects_evicted_body_with_deleted_complete_wire_finality() {
    let (_temp_dir, config) = kura_storage_fixture("create Kura root", nonzero!(1_usize));
    {
        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("open Kura");
        store_dummy_block_arcs(&kura, 4);
        let (_, payload_len) = advertise_required_replicas(&kura, nonzero!(2_usize));
        assert!(
            kura.evict_block_bodies(payload_len)
                .expect("evict finalized canonical body")
                >= payload_len
        );
        std::fs::remove_file(kura.v2_finality_artifact_path(2))
            .expect("delete required complete-wire finality");
    }
    let error = match Kura::new(&config, &RuntimeLaneConfig::default()) {
        Ok(_) => panic!("startup must reject deleted complete-wire finality"),
        Err(error) => error,
    };
    assert!(
        matches!(error, Error::MissingV2FinalityArtifact { height: 2 }),
        "unexpected startup error: {error:?}"
    );
}
#[test]
fn startup_rejects_noncanonical_retained_header_inventory_name() {
    let (_temp_dir, config) = kura_storage_fixture("create Kura root", BLOCKS_IN_MEMORY);
    {
        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("open Kura");
        let block = store_dummy_block_arcs(&kura, 1)
            .pop()
            .expect("stored canonical block");
        let directory = kura.retained_block_record_dir();
        std::fs::create_dir_all(&directory).expect("create retained-header directory");
        std::fs::write(
            directory.join("1.norito"),
            KuraRetainedBlockRecord::new(
                block.header(),
                Kura::canonical_proposal_wire_hash(&block).expect("canonical proposal wire hash"),
                Kura::canonical_block_wire_identity(&block)
                    .expect("canonical block wire identity")
                    .0,
                Kura::canonical_block_wire_hash(&block).expect("canonical block wire hash"),
                None,
                Vec::new(),
            )
            .encode(),
        )
        .expect("write noncanonical retained-header name");
    }
    assert!(matches!(
        Kura::new(&config, &RuntimeLaneConfig::default()),
        Err(Error::IO(error, _)) if error.kind() == ErrorKind::InvalidData
    ));
}
#[test]
fn retained_sidecar_inventory_is_bounded_by_chain_height_plus_transient_slack() {
    let temp_dir = TempDir::new().expect("create Kura root");
    let root = temp_dir.path();
    let directory = root.join(RETAINED_BLOCKS_DIR_NAME);
    std::fs::create_dir_all(&directory).expect("create retained inventory directory");
    std::fs::write(directory.join("00000000000000000001.norito"), b"canonical")
        .expect("write one durable-height entry");
    for index in 0..33 {
        std::fs::write(
            directory.join(format!(".kura-sidecar-{index:02}")),
            b"transient",
        )
        .expect("write bounded transient inventory entry");
    }
    assert!(matches!(
        Kura::canonical_height_sidecar_heights_for(
            root,
            &directory,
            "retained block sidecar",
            1,
        ),
        Err(Error::IO(error, path))
            if error.kind() == ErrorKind::InvalidData && path == directory
    ));
}
fn assert_restart_reconciles_retained_suffix_after_published_truncate(
    remove_one_before_restart: bool,
) {
    let (_temp_dir, config) = kura_storage_fixture("create Kura root", BLOCKS_IN_MEMORY);
    {
        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("open Kura");
        let blocks = store_dummy_block_arcs(&kura, 4);
        let blocks_dir = kura.active_blocks_dir.lock().clone();
        for block in blocks.iter().skip(1) {
            kura.persist_retained_block_record(&blocks_dir, block.hash(), block.as_ref())
                .expect("persist retained suffix fixture");
        }
        kura.block_store
            .lock()
            .prune(1)
            .expect("publish canonical truncate before retained cleanup");
        if remove_one_before_restart {
            std::fs::remove_file(kura.retained_block_record_path(2))
                .expect("simulate crash after one retained suffix removal");
        }
    }
    let (reopened, count) = Kura::new(&config, &RuntimeLaneConfig::default())
        .expect("first restart reconciles stale retained suffix");
    assert_eq!(count.0, 1);
    assert!(
        (2..=4).all(|height| !reopened.retained_block_record_path(height).exists()),
        "startup reconciliation must remove every retained record above the durable tip"
    );
    drop(reopened);
    let (reopened_again, count) = Kura::new(&config, &RuntimeLaneConfig::default())
        .expect("second restart remains converged after retained suffix reconciliation");
    assert_eq!(count.0, 1);
    assert!((2..=4).all(|height| !reopened_again.retained_block_record_path(height).exists()));
}
#[test]
fn restart_reconciles_retained_suffix_when_crash_precedes_cleanup() {
    assert_restart_reconciles_retained_suffix_after_published_truncate(false);
}
#[test]
fn restart_reconciles_retained_suffix_when_crash_interrupts_cleanup() {
    assert_restart_reconciles_retained_suffix_after_published_truncate(true);
}
#[cfg(unix)]
#[test]
fn retained_header_symlink_substitution_aborts_before_eviction() {
    use std::os::unix::fs::symlink;
    let (temp_dir, _config, kura) = kura_root_fixture(nonzero!(1_usize));
    let blocks = store_dummy_block_arcs(&kura, 4);
    let artifact = v2_finality_artifacts_for_chain(&blocks[..2])[1].clone();
    let _ = kura
        .store_v2_finality_artifact(&artifact)
        .expect("persist signed finality before retained-record substitution");
    let (_, payload_len) = advertise_required_replicas(&kura, nonzero!(2_usize));
    let directory = kura.retained_block_record_dir();
    std::fs::create_dir_all(&directory).expect("create retained-header directory");
    let external = temp_dir.path().join("attacker-retained-header.norito");
    std::fs::write(
        &external,
        KuraRetainedBlockRecord::new(
            blocks[1].header(),
            Kura::canonical_proposal_wire_hash(&blocks[1]).expect("canonical proposal wire hash"),
            Kura::canonical_block_wire_identity(&blocks[1])
                .expect("canonical block wire identity")
                .0,
            Kura::canonical_block_wire_hash(&blocks[1]).expect("canonical block wire hash"),
            None,
            Vec::new(),
        )
        .encode(),
    )
    .expect("write external retained-header bytes");
    std::fs::remove_file(kura.retained_block_record_path(2))
        .expect("remove exact retained record before symlink substitution");
    symlink(&external, kura.retained_block_record_path(2))
        .expect("substitute retained-header symlink");
    assert!(matches!(
        kura.evict_block_bodies(payload_len),
        Err(Error::IO(error, _)) if error.kind() == ErrorKind::InvalidData
    ));
    assert!(
        !kura
            .block_store
            .lock()
            .read_block_index(1)
            .expect("read block index")
            .is_evicted(),
        "symlink substitution must fail before index publication"
    );
}
#[test]
fn retained_header_oversize_fails_before_body_eviction() {
    let (_temp_dir, _config, kura) = kura_root_fixture(nonzero!(1_usize));
    let blocks = store_dummy_block_arcs(&kura, 4);
    let artifact = v2_finality_artifacts_for_chain(&blocks[..2])[1].clone();
    let _ = kura
        .store_v2_finality_artifact(&artifact)
        .expect("persist signed finality before retained-record oversize tamper");
    let (_, payload_len) = advertise_required_replicas(&kura, nonzero!(2_usize));
    let directory = kura.retained_block_record_dir();
    std::fs::create_dir_all(&directory).expect("create retained-header directory");
    std::fs::write(
        kura.retained_block_record_path(2),
        vec![0_u8; MAX_RETAINED_BLOCK_RECORD_BYTES.saturating_add(1)],
    )
    .expect("write oversized retained-header record");
    assert!(matches!(
        kura.evict_block_bodies(payload_len),
        Err(Error::IO(error, _)) if error.kind() == ErrorKind::InvalidData
    ));
    assert!(
        !kura
            .block_store
            .lock()
            .read_block_index(1)
            .expect("read block index")
            .is_evicted()
    );
}
#[test]
fn retained_block_decode_rejects_absurd_lengths_trailing_truncation_and_version() {
    let kura = Kura::blank_kura_for_testing();
    let block = retained_archive_empty_block(None);
    kura.store_block(Arc::clone(&block))
        .expect("store canonical rootless block");
    let directory = kura.retained_block_record_dir();
    let path = kura.retained_block_record_path(1);
    std::fs::create_dir_all(&directory).expect("create retained-block directory");
    let canonical = KuraRetainedBlockRecord::new(
        block.header(),
        Kura::canonical_proposal_wire_hash(&block).expect("canonical proposal wire hash"),
        Kura::canonical_block_wire_identity(&block)
            .expect("canonical block wire identity")
            .0,
        Kura::canonical_block_wire_hash(&block).expect("canonical block wire hash"),
        None,
        Vec::new(),
    );
    let canonical_bytes = canonical.encode();
    let mut trailing = canonical_bytes.clone();
    trailing.push(0);
    let mut truncated = canonical_bytes.clone();
    truncated.pop().expect("canonical record is nonempty");
    let mut absurd_archive_len = canonical_bytes.clone();
    assert_eq!(
        absurd_archive_len.pop(),
        Some(0),
        "empty trailing archive uses canonical zero varint"
    );
    absurd_archive_len.extend([0xff; 9]);
    absurd_archive_len.push(1);
    let mut wrong_layout_version = canonical.clone();
    wrong_layout_version.format_version = 2;
    let mut zero_wire_len = canonical.clone();
    zero_wire_len.executed_block_wire_len = 0;
    let mut oversized_wire_len = canonical.clone();
    oversized_wire_len.executed_block_wire_len = STRICT_INIT_MAX_BLOCK_BYTES.saturating_add(1);
    let mut bad_version = canonical;
    bad_version.format_version = RETAINED_BLOCK_RECORD_VERSION.saturating_add(1);
    for hostile in [
        trailing,
        truncated,
        absurd_archive_len,
        wrong_layout_version.encode(),
        zero_wire_len.encode(),
        oversized_wire_len.encode(),
        bad_version.encode(),
    ] {
        assert!(hostile.len() <= MAX_RETAINED_BLOCK_RECORD_BYTES);
        std::fs::write(&path, hostile).expect("write hostile retained record");
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            kura.retained_block_record_at(&kura.active_blocks_dir.lock().clone(), 1, block.hash())
        }));
        assert!(
            result.is_ok(),
            "hostile length prefix must not panic or abort"
        );
        assert!(
            result.expect("checked unwind result").is_err(),
            "hostile retained record unexpectedly validated"
        );
    }
}
#[derive(Encode)]
struct RetiredKuraRetainedBlockRecordV2Fixture {
    format_version: u16,
    height: u64,
    block_hash: HashOf<BlockHeader>,
    block_header: BlockHeader,
    proposal_wire_hash: Hash,
    executed_block_wire_hash: Hash,
    sccp_archive: Vec<KuraRetainedSccpMessage>,
}
fn retired_retained_block_v2_bytes(record: &KuraRetainedBlockRecord) -> Vec<u8> {
    RetiredKuraRetainedBlockRecordV2Fixture {
        format_version: 2,
        height: record.height,
        block_hash: record.block_hash,
        block_header: record.block_header,
        proposal_wire_hash: record.proposal_wire_hash,
        executed_block_wire_hash: record.executed_block_wire_hash,
        sccp_archive: record.sccp_archive.clone(),
    }
    .encode()
}
#[test]
fn retained_block_v2_layout_is_rejected_by_direct_read_and_startup() {
    let (_temp_dir, config) = kura_storage_fixture("create Kura root", BLOCKS_IN_MEMORY);
    {
        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("open Kura");
        let block = store_dummy_block_arcs(&kura, 1)
            .pop()
            .expect("stored canonical block");
        let blocks_dir = kura.active_blocks_dir.lock().clone();
        let record = Kura::prepare_retained_block_record(&blocks_dir, block.hash(), block.as_ref())
            .expect("prepare current retained record");
        let directory = kura.retained_block_record_dir();
        let path = kura.retained_block_record_path(1);
        std::fs::create_dir_all(&directory).expect("create retained-record directory");
        std::fs::write(&path, retired_retained_block_v2_bytes(&record))
            .expect("install retired version-two retained bytes");
        assert!(matches!(
            kura.decode_retained_block_record_at(&path, &directory),
            Err(Error::IO(error, error_path))
                if error.kind() == ErrorKind::InvalidData && error_path == path
        ));
    }
    let startup_error = match Kura::new(&config, &RuntimeLaneConfig::default()) {
        Ok(_) => panic!("startup accepted a retired version-two retained record"),
        Err(error) => error,
    };
    assert!(
        matches!(startup_error, Error::IO(ref error, _) if error.kind() == ErrorKind::InvalidData),
        "unexpected retired-layout startup error: {startup_error:?}"
    );
}
#[test]
fn retained_record_bound_covers_joint_base_and_merge_reference_maxima() {
    assert_eq!(
        MAX_RETAINED_BLOCK_RECORD_BYTES,
        MAX_RETAINED_BLOCK_BASE_ENVELOPE_BYTES
            + MAX_RETAINED_MERGE_REFERENCE_BYTES
            + MAX_RETAINED_BLOCK_RECORD_FRAMING_BYTES
    );
    assert!(
        MAX_RETAINED_BLOCK_RECORD_BYTES > 8 * 1024 * 1024,
        "the current envelope must cover the complete base archive plus a 4 MiB QC"
    );
}
#[test]
fn retained_record_joint_envelope_fits_max_sccp_count_and_qc_geometry() {
    let genesis = retained_archive_empty_block(None);
    let payloads =
        (1..=u64::from(iroha_data_model::bridge::SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1))
            .map(retained_archive_sccp_payload)
            .collect::<Vec<_>>();
    let sccp_block = retained_archive_sccp_block(&genesis, &payloads);
    let mut entry = sample_merge_entry_for_block(1, &sccp_block);
    // This is a storage-envelope geometry fixture, not a cryptographically
    // valid QC: production BLS aggregate signatures are fixed-size. Grow
    // the variable test field until the complete QC encoding is within 64
    // bytes of the independent 4 MiB consensus-side byte ceiling.
    let qc_target: usize = 4 * 1024 * 1024;
    for _ in 0..8 {
        let encoded = entry.merge_qc.encoded_len();
        if encoded >= qc_target.saturating_sub(64) && encoded <= qc_target {
            break;
        }
        if encoded < qc_target {
            entry.merge_qc.aggregate_signature.resize(
                entry
                    .merge_qc
                    .aggregate_signature
                    .len()
                    .saturating_add(qc_target - encoded),
                0xA5,
            );
        } else {
            entry.merge_qc.aggregate_signature.truncate(
                entry
                    .merge_qc
                    .aggregate_signature
                    .len()
                    .saturating_sub(encoded - qc_target),
            );
        }
    }
    let qc_len = entry.merge_qc.encoded_len();
    assert!(
        (qc_target.saturating_sub(64)..=qc_target).contains(&qc_len),
        "geometry QC encoding is {qc_len} bytes"
    );
    let carrier = attach_merge_reference(&sccp_block, &entry);
    let record =
        Kura::prepare_retained_block_record(Path::new("joint-envelope"), carrier.hash(), &carrier)
            .expect("prepare semantically valid max-count SCCP archive with bounded reference");
    assert_eq!(
        record.sccp_archive.len(),
        usize::try_from(iroha_data_model::bridge::SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1)
            .expect("SCCP count fits usize")
    );
    let reference_len = record
        .merge_reference
        .as_ref()
        .expect("joint envelope carries a merge reference")
        .encoded_len();
    assert!(reference_len <= MAX_RETAINED_MERGE_REFERENCE_BYTES);
    let record_len = record.canonical_storage_encoded_len();
    let mut base_projection = record.clone();
    base_projection.merge_reference = None;
    let base_len = base_projection.canonical_storage_encoded_len();
    assert!(base_len <= MAX_RETAINED_BLOCK_BASE_ENVELOPE_BYTES);
    let measured_framing = record_len.saturating_sub(base_len.saturating_add(reference_len));
    assert!(
        measured_framing <= MAX_RETAINED_BLOCK_RECORD_FRAMING_BYTES,
        "measured current framing is {measured_framing} bytes"
    );
    assert!(
        record_len > MAX_RETAINED_BLOCK_BASE_ENVELOPE_BYTES,
        "joint current fixture must exceed the complete base-envelope cap"
    );
    assert!(
        record_len <= MAX_RETAINED_BLOCK_RECORD_BYTES,
        "joint current fixture is {record_len} bytes; cap is {MAX_RETAINED_BLOCK_RECORD_BYTES}"
    );
}
#[test]
fn concurrent_eviction_and_finality_serialize_without_losing_header() {
    let (_temp_dir, _config, kura) = kura_root_fixture(nonzero!(1_usize));
    let blocks = store_dummy_block_arcs(&kura, 4);
    let artifact = v2_finality_artifacts_for_chain(&blocks[..2])[1].clone();
    let _ = kura
        .store_v2_finality_artifact(&artifact)
        .expect("persist signed finality before eviction");
    let (_, payload_len) = advertise_required_replicas(&kura, nonzero!(2_usize));
    kura.pause_next_eviction_after_snapshot_for_tests();
    let evict_kura = Arc::clone(&kura);
    let evict = thread::spawn(move || evict_kura.evict_block_bodies(payload_len));
    let deadline = Instant::now() + Duration::from_secs(5);
    while !kura.eviction_paused_after_snapshot_for_tests() {
        assert!(
            Instant::now() < deadline,
            "eviction did not reach race barrier"
        );
        thread::yield_now();
    }
    let finality_kura = Arc::clone(&kura);
    let finality_artifact = artifact.clone();
    let (finality_tx, finality_rx) = mpsc::channel();
    let finality = thread::spawn(move || {
        finality_tx
            .send(finality_kura.store_v2_finality_artifact(&finality_artifact))
            .expect("report finality result");
    });
    assert!(
        matches!(
            finality_rx.recv_timeout(Duration::from_millis(50)),
            Err(RecvTimeoutError::Timeout)
        ),
        "idempotent finality persistence must wait behind the canonical eviction snapshot"
    );
    kura.resume_eviction_after_snapshot_for_tests();
    assert_eq!(
        evict.join().expect("join eviction").expect("evict body"),
        payload_len
    );
    let _receipt = finality_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("finality worker timed out")
        .expect("repeat concurrent finality persistence");
    finality.join().expect("join finality worker");
    assert_eq!(
        kura.v2_finality_artifact(2)
            .expect("read concurrent finality"),
        Some(artifact)
    );
    let (retained, archive) = kura
        .retained_sccp_archive(2)
        .expect("read retained archive")
        .expect("retained archive exists");
    assert_eq!(retained, blocks[1].header());
    assert!(archive.is_empty());
}
#[test]
fn finalized_top_block_rejects_replacement_without_mutation() {
    let (kura, block) = blank_kura_with_next_block();
    let original_hash = block.hash();
    kura.store_block(Arc::clone(&block))
        .expect("store canonical block");
    let artifact = v2_finality_artifact_for_block(&block);
    let _receipt = kura
        .store_v2_finality_artifact(&artifact)
        .expect("finalize canonical block");
    kura.replace_top_block(Arc::clone(&block))
        .expect("an exact idempotent replacement remains harmless");
    let replacement: SignedBlock =
        ValidBlock::new_dummy_and_modify_header(checked_keypair().private_key(), |header| {
            header.set_height(nonzero!(1_u64));
            header.set_prev_block_hash(None);
            header.set_view_change_index(header.view_change_index().saturating_add(1));
        })
        .into();
    let replacement_hash = replacement.hash();
    assert_ne!(replacement_hash, original_hash);
    assert!(matches!(
        kura.replace_top_block(replacement),
        Err(Error::FinalizedV2BlockMutation {
            rewrite_from_height: 1,
            finalized_height: 1,
        })
    ));
    assert_eq!(kura.blocks_count(), 1);
    assert_eq!(
        kura.get_durable_block_hash(nonzero!(1_usize)),
        Some(original_hash)
    );
    assert_eq!(
        kura.v2_finality_artifact(1)
            .expect("read finality after rejected replacement"),
        Some(artifact)
    );
}
#[test]
fn pruning_across_durable_v2_finality_is_atomic_and_rejected() {
    let (kura, block) = blank_kura_with_next_block();
    let original_hash = block.hash();
    kura.store_block(Arc::clone(&block))
        .expect("store canonical block");
    let artifact = v2_finality_artifact_for_block(&block);
    let _receipt = kura
        .store_v2_finality_artifact(&artifact)
        .expect("finalize canonical block");
    assert!(matches!(
        kura.prune_to_height(0),
        Err(Error::FinalizedV2BlockMutation {
            rewrite_from_height: 1,
            finalized_height: 1,
        })
    ));
    assert_eq!(kura.blocks_count(), 1);
    assert_eq!(
        kura.get_durable_block_hash(nonzero!(1_usize)),
        Some(original_hash)
    );
    assert_eq!(
        kura.v2_finality_artifact(1)
            .expect("read finality after rejected prune"),
        Some(artifact)
    );
}
#[test]
fn startup_rejects_prune_intent_crossing_durable_v2_finality_before_mutation() {
    let (_temp_dir, config) = kura_storage_fixture("create temp dir", BLOCKS_IN_MEMORY);
    let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("init Kura");
    let block = DummyBlocks::new().next();
    let block_hash = block.hash();
    kura.store_block(Arc::clone(&block))
        .expect("store canonical block");
    let artifact = v2_finality_artifact_for_block(&block);
    let _commit_receipt = kura
        .store_v2_finality_artifact(&artifact)
        .expect("finalize canonical block");
    let blocks_dir = kura.active_blocks_dir.lock().clone();
    kura.persist_prune_intent(&admit_prune_intent_fixture(
        &kura,
        KuraPruneIntentV3 {
            version: 3,
            source_height: 1,
            source_tip_hash: Some(block_hash),
            target_height: 0,
            target_tip_hash: None,
            retained_merge_entries: 0,
            retained_merge_tip_hash: None,
            sidecar_rewrite: KuraPruneSidecarRewriteProjectionV3::none(),
            capacity: unsealed_prune_capacity_fixture(),
        },
    ))
    .expect("plant otherwise valid prune intent");
    drop(kura);
    assert!(matches!(
        Kura::new(&config, &RuntimeLaneConfig::default()),
        Err(Error::FinalizedV2BlockMutation {
            rewrite_from_height: 1,
            finalized_height: 1,
        })
    ));
    let mut store = BlockStore::new(&blocks_dir);
    assert_eq!(
        store
            .read_commit_marker()
            .expect("read marker after rejected recovery")
            .map(|marker| marker.count),
        Some(1),
        "startup must reject before applying the prune intent"
    );
}
#[test]
fn startup_rejects_finality_inventory_ahead_of_the_durable_chain() {
    let (_temp_dir, config) = kura_storage_fixture("create temp dir", BLOCKS_IN_MEMORY);
    let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("init Kura");
    let block = DummyBlocks::new().next();
    kura.store_block(Arc::clone(&block))
        .expect("store canonical block");
    let artifact = v2_finality_artifact_for_block(&block);
    let _receipt = kura
        .store_v2_finality_artifact(&artifact)
        .expect("persist canonical finality");
    let blocks_dir = kura.active_blocks_dir.lock().clone();
    let canonical_path = Kura::v2_finality_artifact_path_for(&blocks_dir, 1);
    let impossible_path = Kura::v2_finality_artifact_path_for(&blocks_dir, 2);
    std::fs::copy(&canonical_path, &impossible_path)
        .expect("plant finality beyond the durable marker");
    drop(kura);
    assert!(matches!(
        Kura::new(&config, &RuntimeLaneConfig::default()),
        Err(Error::V2FinalityBeyondDurableChain {
            finalized_height: 2,
            durable_height: 1,
        })
    ));
}
#[test]
fn startup_verifies_every_v2_finality_artifact_below_the_highest() {
    let (_temp_dir, config) = kura_storage_fixture("create temp dir", BLOCKS_IN_MEMORY);
    let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("init Kura");
    let mut generator = DummyBlocks::new();
    let blocks = vec![generator.next(), generator.next()];
    for block in &blocks {
        kura.store_block(Arc::clone(block))
            .expect("store canonical block");
    }
    let artifacts = v2_finality_artifacts_for_chain(&blocks);
    for artifact in &artifacts {
        let _receipt = kura
            .store_v2_finality_artifact(artifact)
            .expect("persist canonical finality");
    }
    let lower_path = kura.v2_finality_artifact_path(1);
    let mut forged_lower = artifacts[0].clone();
    forged_lower.commit_qc.aggregate_signature[0] ^= 0x80;
    forged_lower
        .validate()
        .expect("signature substitution remains structurally valid");
    drop(kura);
    replace_v2_finality_record_artifact(&lower_path, forged_lower);
    assert!(matches!(
        Kura::new(&config, &RuntimeLaneConfig::default()),
        Err(Error::V2FinalityCryptography(_))
    ));
}
#[test]
fn startup_corruption_recovery_cannot_prune_finalized_block_bytes() {
    let (_temp_dir, config) = kura_storage_fixture("create temp dir", BLOCKS_IN_MEMORY);
    let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("init Kura");
    let mut generator = DummyBlocks::new();
    let blocks = vec![generator.next(), generator.next()];
    for block in &blocks {
        kura.store_block(Arc::clone(block))
            .expect("store canonical block");
    }
    let artifacts = v2_finality_artifacts_for_chain(&blocks);
    let _receipt = kura
        .store_v2_finality_artifact(&artifacts[1])
        .expect("finalize the canonical suffix block");
    let blocks_dir = kura.active_blocks_dir.lock().clone();
    drop(kura);
    let mut store = BlockStore::new(&blocks_dir);
    let final_index = store
        .read_block_index(1)
        .expect("read finalized suffix index");
    drop(store);
    let data_path = blocks_dir.join(DATA_FILE_NAME);
    let truncated_len = final_index
        .start
        .checked_add(final_index.length)
        .and_then(|end| end.checked_sub(1))
        .expect("final block contains at least one byte");
    std::fs::OpenOptions::new()
        .write(true)
        .open(&data_path)
        .expect("open canonical data file")
        .set_len(truncated_len)
        .expect("truncate the finalized suffix block");
    let canonical_paths = [
        data_path,
        blocks_dir.join(INDEX_FILE_NAME),
        blocks_dir.join(HASHES_FILE_NAME),
        blocks_dir.join(COUNT_FILE_NAME),
    ];
    let before = canonical_paths
        .iter()
        .map(|path| std::fs::read(path).expect("snapshot corrupted canonical file"))
        .collect::<Vec<_>>();
    assert!(matches!(
        Kura::new(&config, &RuntimeLaneConfig::default()),
        Err(Error::FinalizedV2BlockMutation {
            rewrite_from_height: 2,
            finalized_height: 2,
        })
    ));
    for (path, expected) in canonical_paths.iter().zip(before) {
        assert_eq!(
            std::fs::read(path).expect("read canonical file after rejected recovery"),
            expected,
            "startup must not mutate {} before rejecting finalized corruption",
            path.display()
        );
    }
}
#[test]
fn incomplete_v2_finality_temp_file_does_not_freeze_top_replacement() {
    let (kura, block) = blank_kura_with_next_block();
    let original_hash = block.hash();
    kura.store_block(block).expect("store canonical block");
    let temporary_path = kura
        .v2_finality_artifact_path(1)
        .with_extension("norito.tmp");
    std::fs::create_dir_all(temporary_path.parent().expect("temporary path parent"))
        .expect("create finality directory");
    std::fs::write(&temporary_path, b"interrupted write")
        .expect("write incomplete temporary artifact");
    let replacement: SignedBlock =
        ValidBlock::new_dummy_and_modify_header(checked_keypair().private_key(), |header| {
            header.set_height(nonzero!(1_u64));
            header.set_prev_block_hash(None);
            header.set_view_change_index(header.view_change_index().saturating_add(1));
        })
        .into();
    let replacement_hash = replacement.hash();
    assert_ne!(replacement_hash, original_hash);
    kura.replace_top_block(replacement)
        .expect("noncanonical temporary bytes do not establish finality");
    assert_eq!(
        kura.get_durable_block_hash(nonzero!(1_usize)),
        Some(replacement_hash)
    );
    assert!(
        !temporary_path.exists(),
        "canonical replacement should discard an incomplete finality temporary"
    );
}
#[test]
fn v2_finality_store_and_read_reject_invalid_aggregate_cryptography() {
    let (kura, block) = blank_kura_with_next_block();
    kura.store_block(Arc::clone(&block))
        .expect("store canonical block");
    let artifact = v2_finality_artifact_for_block(&block);
    let mut forged = artifact.clone();
    forged.commit_qc.aggregate_signature[0] ^= 0x80;
    forged
        .validate()
        .expect("aggregate substitution remains structurally well formed");
    let enforced_before = kura
        .refresh_disk_usage_bytes()
        .expect("measure enforced usage before forged finality");
    let total_before = kura
        .refresh_total_disk_usage_bytes()
        .expect("measure total usage before forged finality");
    let retained_path = kura.retained_block_record_path(artifact.height);
    assert!(!retained_path.exists());
    assert!(matches!(
        kura.store_v2_finality_artifact(&forged),
        Err(Error::V2FinalityCryptography(_))
    ));
    let path = kura.v2_finality_artifact_path(artifact.height);
    assert!(
        !path.exists(),
        "cryptographically invalid bytes must not reach the durable path"
    );
    assert!(
        !retained_path.exists(),
        "cryptographically invalid finality must not publish retained evidence"
    );
    assert_eq!(
        kura.kura_disk_usage_bytes()
            .expect("rescan enforced usage after forged finality"),
        enforced_before
    );
    assert_eq!(
        kura.kura_total_disk_usage_bytes()
            .expect("rescan total usage after forged finality"),
        total_before
    );
    assert_eq!(
        kura.disk_usage_bytes()
            .expect("cached total usage after forged finality"),
        total_before,
        "forged finality must leave no cached disk-accounting delta"
    );
    let receipt = kura
        .store_v2_finality_artifact(&artifact)
        .expect("persist valid finality artifact");
    assert_v2_commit_receipt_matches_artifact(&receipt, &artifact);
    assert!(
        retained_path.is_file(),
        "an exact valid retry must publish the canonical retained record"
    );
    replace_v2_finality_record_artifact(&path, forged);
    assert!(matches!(
        kura.v2_finality_artifact(artifact.height),
        Err(Error::V2FinalityCryptography(_))
    ));
}
#[test]
fn v2_finality_artifact_rejects_canonical_block_mismatch() {
    let (kura, block) = blank_kura_with_next_block();
    kura.store_block(Arc::clone(&block))
        .expect("store canonical block");
    let mut artifact = v2_finality_artifact_for_block(&block);
    let wrong_hash = HashOf::from_untyped_unchecked(Hash::new(b"another block"));
    artifact.block_hash = wrong_hash;
    artifact.subject.block_hash = wrong_hash;
    artifact.commit_qc.subject = artifact.subject;
    artifact
        .validate()
        .expect("mismatch is internally coherent");
    let error = kura
        .store_v2_finality_artifact(&artifact)
        .expect_err("Kura must reject a foreign block hash");
    assert!(matches!(
        error,
        Error::BlockHeightConflict {
            height: 1,
            expected,
            actual,
        } if expected == block.hash() && actual == wrong_hash
    ));
    assert_eq!(
        kura.v2_finality_artifact(1)
            .expect("missing artifact is not an error"),
        None
    );
}
#[test]
fn v2_finality_read_ignores_partial_temporary_file_but_fails_on_partial_final_file() {
    let (kura, block) = blank_kura_with_next_block();
    kura.store_block(Arc::clone(&block))
        .expect("store canonical block");
    let artifact = v2_finality_artifact_for_block(&block);
    let receipt = kura
        .store_v2_finality_artifact(&artifact)
        .expect("persist finality artifact");
    assert_v2_commit_receipt_matches_artifact(&receipt, &artifact);
    let path = kura.v2_finality_artifact_path(artifact.height);
    let encoded = std::fs::read(&path).expect("read canonical finality record");
    let partial = &encoded[..encoded.len() / 2];
    std::fs::write(path.with_extension("norito.tmp"), partial)
        .expect("write interrupted temporary artifact");
    assert_eq!(
        kura.v2_finality_artifact(artifact.height)
            .expect("temporary file must not shadow durable artifact"),
        Some(artifact)
    );
    std::fs::write(&path, partial).expect("replace final artifact with truncated bytes");
    assert!(matches!(
        kura.v2_finality_artifact(1),
        Err(Error::NoritoFrame(_))
    ));
}
#[test]
fn v2_finality_read_rejects_oversized_final_file_before_decode() {
    let (kura, block) = blank_kura_with_next_block();
    kura.store_block(Arc::clone(&block))
        .expect("store canonical block");
    let artifact = v2_finality_artifact_for_block(&block);
    let receipt = kura
        .store_v2_finality_artifact(&artifact)
        .expect("persist finality artifact");
    assert_v2_commit_receipt_matches_artifact(&receipt, &artifact);
    let path = kura.v2_finality_artifact_path(artifact.height);
    std::fs::write(&path, vec![0xA5; MAX_KURA_V2_FINALITY_RECORD_BYTES + 1])
        .expect("replace artifact with oversized hostile bytes");
    assert!(matches!(
        kura.v2_finality_artifact(artifact.height),
        Err(Error::IO(error, observed_path))
            if error.kind() == ErrorKind::InvalidData && observed_path == path
    ));
}
#[cfg(unix)]
#[test]
fn v2_finality_read_and_rewrite_reject_symlink_substitution() {
    use std::os::unix::fs::symlink;
    let (kura, block) = blank_kura_with_next_block();
    kura.store_block(Arc::clone(&block))
        .expect("store canonical block");
    let artifact = v2_finality_artifact_for_block(&block);
    let receipt = kura
        .store_v2_finality_artifact(&artifact)
        .expect("persist finality artifact");
    assert_v2_commit_receipt_matches_artifact(&receipt, &artifact);
    let path = kura.v2_finality_artifact_path(artifact.height);
    let target = path.with_extension("attacker.norito");
    std::fs::rename(&path, &target).expect("move valid bytes behind attacker path");
    symlink(&target, &path).expect("substitute finality path with symlink");
    assert!(matches!(
        kura.v2_finality_artifact(artifact.height),
        Err(Error::IO(error, observed_path))
            if error.kind() == ErrorKind::InvalidData && observed_path == path
    ));
    assert!(matches!(
        kura.store_v2_finality_artifact(&artifact),
        Err(Error::IO(error, observed_path))
            if error.kind() == ErrorKind::InvalidData && observed_path == path
    ));
}
#[cfg(unix)]
#[test]
fn v2_finality_read_and_rewrite_reject_hardlink_aliases() {
    let (kura, block) = blank_kura_with_next_block();
    kura.store_block(Arc::clone(&block))
        .expect("store canonical block");
    let artifact = v2_finality_artifact_for_block(&block);
    let receipt = kura
        .store_v2_finality_artifact(&artifact)
        .expect("persist finality artifact");
    assert_v2_commit_receipt_matches_artifact(&receipt, &artifact);
    let path = kura.v2_finality_artifact_path(artifact.height);
    let alias = path.with_extension("hardlink.norito");
    std::fs::hard_link(&path, &alias).expect("create attacker-controlled hardlink alias");
    assert!(matches!(
        kura.v2_finality_artifact(artifact.height),
        Err(Error::IO(error, observed_path))
            if error.kind() == ErrorKind::InvalidData && observed_path == path
    ));
    assert!(matches!(
        kura.store_v2_finality_artifact(&artifact),
        Err(Error::IO(error, observed_path))
            if error.kind() == ErrorKind::InvalidData && observed_path == path
    ));
}
#[cfg(unix)]
#[test]
fn v2_finality_write_rejects_symlinked_parent_directory_even_when_file_is_missing() {
    use std::os::unix::fs::symlink;
    let (kura, block) = blank_kura_with_next_block();
    kura.store_block(Arc::clone(&block))
        .expect("store canonical block");
    let artifact = v2_finality_artifact_for_block(&block);
    let attacker = TempDir::new().expect("create attacker-controlled directory");
    let finality_dir = kura
        .v2_finality_artifact_path(artifact.height)
        .parent()
        .expect("finality artifact has a parent directory")
        .to_path_buf();
    symlink(attacker.path(), &finality_dir).expect("substitute finality directory symlink");
    assert!(matches!(
        kura.store_v2_finality_artifact(&artifact),
        Err(Error::IO(error, observed_path))
            if error.kind() == ErrorKind::InvalidData && observed_path == finality_dir
    ));
    assert!(
        std::fs::read_dir(attacker.path())
            .expect("read attacker directory")
            .next()
            .is_none(),
        "rejected parent substitution must not write outside Kura"
    );
}
#[cfg(unix)]
#[test]
fn v2_finality_write_ignores_preplanted_predictable_temp_symlink() {
    use std::os::unix::fs::symlink;
    let (kura, block) = blank_kura_with_next_block();
    kura.store_block(Arc::clone(&block))
        .expect("store canonical block");
    let artifact = v2_finality_artifact_for_block(&block);
    let path = kura.v2_finality_artifact_path(artifact.height);
    std::fs::create_dir_all(path.parent().expect("finality path parent"))
        .expect("create finality directory");
    let victim = kura.store_root().join("attacker-victim");
    let victim_bytes = b"must remain untouched";
    std::fs::write(&victim, victim_bytes).expect("create attacker victim");
    let predictable = path.with_extension("norito.tmp");
    symlink(&victim, &predictable).expect("preplant retired predictable temp symlink");
    let receipt = kura
        .store_v2_finality_artifact(&artifact)
        .expect("random create-new temp path avoids the preplanted symlink");
    assert_v2_commit_receipt_matches_artifact(&receipt, &artifact);
    assert_eq!(
        std::fs::read(&victim).expect("read attacker victim"),
        victim_bytes
    );
    assert!(predictable.is_symlink());
    assert_eq!(
        kura.v2_finality_artifact(artifact.height)
            .expect("read finality artifact"),
        Some(artifact)
    );
}
#[test]
fn lane_segment_reconciliation_provisions_and_retires_storage() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let store_root = temp_dir.path().join("kura");
    let lane_count = NonZeroU32::new(4).expect("non-zero lane count");
    let lane0 = ModelLaneConfig::default();
    let lane1 = ModelLaneConfig {
        id: LaneId::from(1),
        alias: "beta".to_string(),
        ..ModelLaneConfig::default()
    };
    let initial_catalog =
        LaneCatalog::new(lane_count, vec![lane0.clone(), lane1.clone()]).expect("catalog");
    let initial_lane_config = RuntimeLaneConfig::from_catalog(&initial_catalog);
    let kura_cfg = kura_config_for_path(&store_root, BLOCKS_IN_MEMORY);
    let (kura, _) = Kura::new(&kura_cfg, &initial_lane_config).expect("init kura");
    let lane1_entry = initial_lane_config
        .entry(LaneId::from(1))
        .expect("lane 1 entry");
    let lane1_blocks = lane1_entry.blocks_dir(&store_root);
    assert!(
        lane1_blocks.exists(),
        "expected lane 1 blocks directory to be provisioned"
    );
    let lane2 = ModelLaneConfig {
        id: LaneId::from(2),
        alias: "gamma".to_string(),
        ..ModelLaneConfig::default()
    };
    let extended_catalog = LaneCatalog::new(
        lane_count,
        vec![lane0.clone(), lane1.clone(), lane2.clone()],
    )
    .expect("catalog");
    let extended_lane_config = RuntimeLaneConfig::from_catalog(&extended_catalog);
    let lane2_entry = extended_lane_config
        .entry(LaneId::from(2))
        .expect("lane 2 entry");
    kura.reconcile_lane_segments_for_testing(&[lane2_entry], &[], &[])
        .expect("provision lane 2");
    let lane2_blocks = lane2_entry.blocks_dir(&store_root);
    assert!(
        lane2_blocks.join(INDEX_FILE_NAME).exists(),
        "lane 2 index file missing"
    );
    assert!(
        lane2_blocks.join(DATA_FILE_NAME).exists(),
        "lane 2 data file missing"
    );
    assert!(
        lane2_blocks.join(HASHES_FILE_NAME).exists(),
        "lane 2 hashes file missing"
    );
    assert!(
        lane2_entry.merge_log_path(&store_root).exists(),
        "lane 2 merge ledger missing"
    );
    kura.reconcile_lane_segments_for_testing(&[], &[lane1_entry], &[])
        .expect("retire lane 1");
    assert!(
        !lane1_blocks.exists(),
        "lane 1 blocks directory should be retired"
    );
    let retired_blocks_root = store_root.join("retired").join("blocks");
    let retired_entries: Vec<_> = std::fs::read_dir(&retired_blocks_root)
        .expect("retired blocks dir")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect retired entries");
    assert!(
        !retired_entries.is_empty(),
        "expected retired lane directory to be archived"
    );
}
#[test]
fn blank_kura_lane_segment_reconciliation_is_noop() {
    static CWD_LOCK: std::sync::LazyLock<std::sync::Mutex<()>> =
        std::sync::LazyLock::new(|| std::sync::Mutex::new(()));
    struct WorkingDirGuard(std::path::PathBuf);
    impl Drop for WorkingDirGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.0);
        }
    }
    let _guard = CWD_LOCK.lock().expect("lock cwd");
    let temp_dir = TempDir::new().expect("create temp dir");
    let original_dir = std::env::current_dir().expect("current dir");
    std::env::set_current_dir(temp_dir.path()).expect("set current dir");
    let _restore_dir = WorkingDirGuard(original_dir);
    let lane_count = NonZeroU32::new(2).expect("non-zero lane count");
    let lane0 = ModelLaneConfig::default();
    let lane1 = ModelLaneConfig {
        id: LaneId::from(1),
        alias: "beta".to_string(),
        ..ModelLaneConfig::default()
    };
    let catalog = LaneCatalog::new(lane_count, vec![lane0, lane1]).expect("catalog");
    let lane_config = RuntimeLaneConfig::from_catalog(&catalog);
    let entry = lane_config.entry(LaneId::from(1)).expect("lane entry");
    let kura = Kura::blank_kura_for_testing();
    kura.reconcile_lane_segments_for_testing(&[entry], &[], &[])
        .expect("no-op reconcile");
    assert!(
        !temp_dir.path().join("blocks").exists(),
        "blank kura must not create lane block directories"
    );
    assert!(
        !temp_dir.path().join("merge_ledger").exists(),
        "blank kura must not create merge-ledger log directories"
    );
}
#[test]
fn snapshot_lane_restore_uses_exact_height_and_authenticated_lineage() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let store_root = temp_dir.path().join("kura");
    let lane0 = ModelLaneConfig::default();
    let stale_config_lane = ModelLaneConfig {
        id: LaneId::new(2),
        alias: "stale-config-lane".to_owned(),
        ..ModelLaneConfig::default()
    };
    let configured_catalog = LaneCatalog::new(
        nonzero!(3_u32),
        vec![lane0.clone(), stale_config_lane.clone()],
    )
    .expect("configured catalog");
    let configured = RuntimeLaneConfig::from_catalog(&configured_catalog);
    let kura_cfg = kura_config_for_path(&store_root, BLOCKS_IN_MEMORY);
    let (kura, _) = Kura::new(&kura_cfg, &configured).expect("init Kura");
    let stale_dir = configured
        .entry(stale_config_lane.id)
        .expect("stale configured lane")
        .blocks_dir(&store_root);
    let restored_lane = ModelLaneConfig {
        id: LaneId::new(1),
        alias: "restored-elastic-lane".to_owned(),
        ..ModelLaneConfig::default()
    };
    let restored_catalog = LaneCatalog::new(nonzero!(2_u32), vec![lane0, restored_lane.clone()])
        .expect("restored catalog");
    let restored = RuntimeLaneConfig::from_catalog(&restored_catalog);
    let primary_incarnation = Hash::new(b"snapshot restore primary incarnation");
    let stale_incarnation = Hash::new(b"snapshot restore stale incarnation");
    let restored_incarnation = Hash::new(b"snapshot restore active incarnation");
    let configured_incarnations = BTreeMap::from([
        (LaneId::SINGLE, primary_incarnation),
        (stale_config_lane.id, stale_incarnation),
    ]);
    let configured_activations = BTreeMap::from([(LaneId::SINGLE, 0), (stale_config_lane.id, 0)]);
    let configured_lineage_root = Hash::new(b"snapshot restore configured lineage");
    let restored_incarnations = BTreeMap::from([
        (LaneId::SINGLE, primary_incarnation),
        (restored_lane.id, restored_incarnation),
    ]);
    let restored_activations = BTreeMap::from([(LaneId::SINGLE, 0), (restored_lane.id, 1)]);
    let restored_lineage_root = Hash::new(b"snapshot restore active lineage");
    kura.apply_lane_geometry_transition_at_height_with_lineage_roots(
        &configured,
        &restored,
        &configured_incarnations,
        &restored_incarnations,
        &configured_activations,
        &restored_activations,
        configured_lineage_root,
        restored_lineage_root,
        &BTreeSet::new(),
        1,
    )
    .expect("apply authenticated post-snapshot geometry transition");
    kura.mark_lane_geometry_catalog_published_with_lineage_root(
        &restored,
        &restored_incarnations,
        &restored_activations,
        restored_lineage_root,
        None,
    )
    .expect("publish authenticated post-snapshot geometry transition");
    kura.restore_lane_segments_with_geometry_at_height_and_lineage_root(
        &configured,
        &configured_incarnations,
        &configured_activations,
        0,
        configured_lineage_root,
    )
    .expect("restore exact pre-transition snapshot geometry");
    assert!(
        kura.lane_storage_entry(restored_lane.id).is_err(),
        "a lane introduced after the snapshot must not remain active"
    );
    assert!(
        stale_dir.exists(),
        "snapshot-authoritative lane must be restored"
    );
    kura.restore_lane_segments_with_geometry_at_height_and_lineage_root(
        &restored,
        &restored_incarnations,
        &restored_activations,
        1,
        restored_lineage_root,
    )
    .expect("restore exact post-transition snapshot geometry");
    let restored_entry = kura
        .lane_storage_entry(restored_lane.id)
        .expect("restored lane must be addressable");
    assert_eq!(restored_entry.alias, restored_lane.alias);
    assert!(restored_entry.blocks_dir(&store_root).exists());
    assert!(
        kura.lane_storage_entry(stale_config_lane.id).is_err(),
        "static-only lane must not remain active after snapshot restore"
    );
    assert!(
        !stale_dir.exists(),
        "replaying the authenticated post-transition cursor must retire the stale lane again"
    );
}
#[test]
fn authenticated_snapshot_lane_restore_rejects_primary_path_drift_atomically() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let store_root = temp_dir.path().join("kura");
    let configured_catalog = LaneCatalog::new(nonzero!(1_u32), vec![ModelLaneConfig::default()])
        .expect("configured catalog");
    let configured = RuntimeLaneConfig::from_catalog(&configured_catalog);
    let kura_cfg = kura_config_for_path(&store_root, BLOCKS_IN_MEMORY);
    let (kura, _) = Kura::new(&kura_cfg, &configured).expect("init Kura");
    let configured_incarnation = Hash::new(b"configured primary restore incarnation");
    let configured_incarnations = BTreeMap::from([(LaneId::SINGLE, configured_incarnation)]);
    let configured_activations = BTreeMap::from([(LaneId::SINGLE, 0)]);
    let configured_lineage_root = Hash::new(b"configured primary restore lineage");
    kura.restore_lane_segments_with_geometry_at_height_and_lineage_root(
        &configured,
        &configured_incarnations,
        &configured_activations,
        0,
        configured_lineage_root,
    )
    .expect("authenticate configured primary geometry");
    let drifted_catalog = LaneCatalog::new(
        nonzero!(1_u32),
        vec![ModelLaneConfig {
            alias: "drifted-primary".to_owned(),
            ..ModelLaneConfig::default()
        }],
    )
    .expect("drifted catalog");
    let drifted = RuntimeLaneConfig::from_catalog(&drifted_catalog);
    let drifted_incarnations =
        BTreeMap::from([(LaneId::SINGLE, Hash::new(b"drifted primary incarnation"))]);
    let drifted_activations = BTreeMap::from([(LaneId::SINGLE, 0)]);
    kura.restore_lane_segments_with_geometry_at_height_and_lineage_root(
        &drifted,
        &drifted_incarnations,
        &drifted_activations,
        0,
        Hash::new(b"drifted primary lineage"),
    )
    .expect_err("primary storage path drift must fail closed");
    assert_eq!(
        kura.lane_storage_entry(LaneId::SINGLE)
            .expect("configured primary remains installed")
            .alias,
        configured.primary().alias
    );
}
#[test]
fn lane_segment_reconciliation_propagates_failure() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let store_root = temp_dir.path().join("kura");
    let initial_catalog =
        LaneCatalog::new(nonzero!(1_u32), vec![ModelLaneConfig::default()]).expect("catalog");
    let initial_lane_config = RuntimeLaneConfig::from_catalog(&initial_catalog);
    let kura_cfg = kura_config_for_path(&store_root, BLOCKS_IN_MEMORY);
    let (kura, _) = Kura::new(&kura_cfg, &initial_lane_config).expect("init kura");
    let extended_catalog = LaneCatalog::new(
        nonzero!(2_u32),
        vec![
            ModelLaneConfig::default(),
            ModelLaneConfig {
                id: LaneId::from(1),
                alias: "conflict".to_string(),
                ..ModelLaneConfig::default()
            },
        ],
    )
    .expect("catalog");
    let extended_lane_config = RuntimeLaneConfig::from_catalog(&extended_catalog);
    let conflicting_entry = extended_lane_config
        .entry(LaneId::from(1))
        .expect("lane entry");
    let conflict_dir = conflicting_entry.blocks_dir(&store_root);
    if let Some(parent) = conflict_dir.parent() {
        std::fs::create_dir_all(parent).expect("create parent dir");
    }
    std::fs::File::create(&conflict_dir).expect("seed conflicting file");
    let canonical_conflict_dir =
        std::fs::canonicalize(&conflict_dir).expect("canonicalize conflicting file");
    let err = kura
        .reconcile_lane_segments_for_testing(&[conflicting_entry], &[], &[])
        .expect_err("expected lane provisioning to surface error");
    match err {
        Error::MkDir(_, path) => assert_eq!(path, canonical_conflict_dir),
        other => panic!("unexpected error: {other:?}"),
    }
}
#[test]
fn lane_segment_relabel_updates_primary_directory() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let store_root = temp_dir.path().join("kura");
    let initial_catalog = LaneCatalog::new(
        nonzero!(1_u32),
        vec![ModelLaneConfig {
            alias: "Alpha Lane".to_string(),
            ..ModelLaneConfig::default()
        }],
    )
    .expect("initial catalog");
    let initial_lane_config = RuntimeLaneConfig::from_catalog(&initial_catalog);
    let initial_entry = initial_lane_config
        .entry(LaneId::SINGLE)
        .expect("lane entry");
    let kura_cfg = kura_config_for_path(&store_root, BLOCKS_IN_MEMORY);
    let (kura, _) = Kura::new(&kura_cfg, &initial_lane_config).expect("init kura");
    let old_dir = initial_entry.blocks_dir(&kura.store_root);
    let old_merge = initial_entry.merge_log_path(&kura.store_root);
    assert!(old_dir.exists(), "expected initial lane directory to exist");
    assert!(old_merge.exists(), "expected initial merge log to exist");
    let updated_catalog = LaneCatalog::new(
        nonzero!(1_u32),
        vec![ModelLaneConfig {
            alias: "Payments Lane".to_string(),
            ..ModelLaneConfig::default()
        }],
    )
    .expect("updated catalog");
    let updated_lane_config = RuntimeLaneConfig::from_catalog(&updated_catalog);
    let updated_entry = updated_lane_config
        .entry(LaneId::SINGLE)
        .expect("lane entry");
    let incarnation = Hash::new(b"authenticated primary relabel incarnation");
    let incarnations = BTreeMap::from([(LaneId::SINGLE, incarnation)]);
    let activation_heights = BTreeMap::from([(LaneId::SINGLE, 0)]);
    let lineage_root = Hash::new(b"authenticated primary relabel lineage");
    kura.apply_lane_geometry_transition_at_height_with_lineage_roots(
        &initial_lane_config,
        &updated_lane_config,
        &incarnations,
        &incarnations,
        &activation_heights,
        &activation_heights,
        lineage_root,
        lineage_root,
        &BTreeSet::new(),
        1,
    )
    .expect("apply authenticated lane-storage relabel");
    kura.mark_lane_geometry_catalog_published_with_lineage_root(
        &updated_lane_config,
        &incarnations,
        &activation_heights,
        lineage_root,
        None,
    )
    .expect("publish authenticated lane-storage relabel");
    let new_dir = updated_entry.blocks_dir(&kura.store_root);
    let new_merge = updated_entry.merge_log_path(&kura.store_root);
    assert!(
        new_dir.exists(),
        "expected relabelled lane directory to exist"
    );
    assert!(!old_dir.exists(), "expected old lane directory to be moved");
    assert_eq!(
        *kura.active_blocks_dir.lock(),
        new_dir,
        "active lane path should be updated"
    );
    assert_eq!(
        kura.block_store.lock().path_to_blockchain,
        new_dir,
        "block store should retarget to new directory"
    );
    assert!(new_merge.exists(), "expected relabelled merge log to exist");
    assert!(!old_merge.exists(), "expected old merge log to be moved");
    assert_eq!(
        *kura.active_merge_path.lock(),
        new_merge,
        "active merge log path should be updated"
    );
    let rejected_catalog = LaneCatalog::new(
        nonzero!(1_u32),
        vec![ModelLaneConfig {
            alias: "Treasury Lane".to_string(),
            ..ModelLaneConfig::default()
        }],
    )
    .expect("rejected catalog");
    let rejected_lane_config = RuntimeLaneConfig::from_catalog(&rejected_catalog);
    let rejected_entry = rejected_lane_config
        .entry(LaneId::SINGLE)
        .expect("rejected lane entry");
    kura.fail_next_relabel_after_block_move
        .store(true, Ordering::Release);
    assert!(matches!(
        kura.relabel_lane_segments(&[(updated_entry, rejected_entry)]),
        Err(Error::IO(_, _))
    ));
    let rejected_dir = rejected_entry.blocks_dir(&kura.store_root);
    let rejected_merge = rejected_entry.merge_log_path(&kura.store_root);
    assert!(
        new_dir.exists(),
        "failed relabel must restore the prior block path"
    );
    assert!(
        new_merge.exists(),
        "failed relabel must retain the prior merge path"
    );
    assert!(!rejected_dir.exists());
    assert!(!rejected_merge.exists());
    assert_eq!(*kura.active_blocks_dir.lock(), new_dir);
    assert_eq!(kura.block_store.lock().path_to_blockchain, new_dir);
    assert_eq!(*kura.active_merge_path.lock(), new_merge);
    assert!(!kura.canonical_storage_poisoned.load(Ordering::Acquire));
}
#[test]
fn block_bytes_returns_memory_mapped_slice() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let mut store = new_block_store(&temp_dir);
    store
        .create_files_if_they_do_not_exist()
        .expect("initialise store files");
    let payload = b"test block payload";
    store
        .write_block_data(0, payload.as_ref())
        .expect("write payload");
    let (slice_ptr, slice_len) = {
        let slice = store
            .block_bytes(0, payload.len() as u64)
            .expect("read payload");
        assert_eq!(slice, payload);
        (slice.as_ptr(), slice.len())
    };
    let mirror = store
        .data_mmap
        .as_ref()
        .expect("mirror should be initialised after block_bytes()");
    assert_eq!(mirror.kind(), MemoryMirrorKind::MemoryMapped);
    assert_eq!(mirror.len(), payload.len());
    let mirror_slice = mirror.slice(0, mirror.len());
    assert_eq!(mirror_slice, payload);
    assert_eq!(slice_len, payload.len());
    assert_eq!(slice_ptr, mirror_slice.as_ptr());
    assert_eq!(store.data_mmap_len, payload.len() as u64);
}
#[test]
fn memory_mirror_updates_after_appending_data() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let mut store = new_block_store(&temp_dir);
    store
        .create_files_if_they_do_not_exist()
        .expect("initialise store files");
    let initial = b"initial payload";
    store
        .write_block_data(0, initial.as_ref())
        .expect("write initial payload");
    {
        let slice = store
            .block_bytes(0, initial.len() as u64)
            .expect("prime mirror with initial payload");
        assert_eq!(slice, initial);
    }
    let expected_initial_len = initial.len();
    let mirror = store
        .data_mmap
        .as_ref()
        .expect("mirror initialised after first read");
    assert_eq!(mirror.len(), expected_initial_len);
    assert_eq!(mirror.kind(), MemoryMirrorKind::MemoryMapped);
    let appended = b" appended payload";
    store
        .write_block_data(initial.len() as u64, appended.as_ref())
        .expect("append payload");
    let total_len = (initial.len() + appended.len()) as u64;
    let combined = {
        let slice = store
            .block_bytes(0, total_len)
            .expect("read combined payload");
        assert_eq!(slice.len(), initial.len() + appended.len());
        slice.to_vec()
    };
    let mirror = store
        .data_mmap
        .as_ref()
        .expect("mirror should be remapped after append");
    assert_eq!(mirror.kind(), MemoryMirrorKind::MemoryMapped);
    assert_eq!(mirror.len(), initial.len() + appended.len());
    let mut expected = Vec::with_capacity(initial.len() + appended.len());
    expected.extend_from_slice(initial);
    expected.extend_from_slice(appended);
    assert_eq!(mirror.slice(0, mirror.len()), expected.as_slice());
    assert_eq!(combined, expected);
    assert_eq!(store.data_mmap_len, total_len);
}
fn indices<const N: usize>(value: [(u64, u64); N]) -> [BlockIndex; N] {
    let mut ret = [BlockIndex {
        start: 0,
        length: 0,
    }; N];
    for idx in 0..value.len() {
        ret[idx] = value[idx].into();
    }
    ret
}
fn wait_for_block_hash(kura: &Arc<Kura>, height: usize, expected: HashOf<BlockHeader>) {
    let deadline = Instant::now() + Duration::from_secs(5);
    let target_index = height
        .checked_sub(1)
        .expect("block height should be non-zero");
    loop {
        {
            let mut store = kura.block_store.lock();
            if let Ok(count) = store.read_index_count() {
                if count > target_index as u64 {
                    if let Ok(hashes) = store.read_block_hashes(target_index as u64, 1) {
                        if hashes.first().copied() == Some(expected) {
                            return;
                        }
                    }
                }
            }
        }
        let now = Instant::now();
        assert!(
            now < deadline,
            "Timed out waiting for block {height} to persist"
        );
        thread::sleep(Duration::from_millis(10));
    }
}
struct BackgroundBudgetEvictionCase {
    _temp_dir: TempDir,
    kura: Arc<Kura>,
    retry_block: Arc<SignedBlock>,
    evictable_body_len: u64,
}
fn background_budget_eviction_case() -> BackgroundBudgetEvictionCase {
    let temp_dir = TempDir::new().expect("create temp dir");
    let kura_cfg = kura_config_for_dir(
        &temp_dir,
        NonZeroUsize::new(1).expect("non-zero"),
    );
    let (mut kura, _) =
        Kura::new(&kura_cfg, &RuntimeLaneConfig::default()).expect("initialize kura");
    let mut blocks = DummyBlocks::new();
    let block1 = blocks.next();
    let block2 = blocks.next();
    let block3 = blocks.next();
    let retry_block = blocks.next();
    kura.store_block(Arc::clone(&block1)).expect("store block1");
    kura.store_block(Arc::clone(&block2)).expect("store block2");
    kura.store_block(Arc::clone(&block3)).expect("store block3");
    let evictable_body_len = {
        let mut store = kura.block_store.lock();
        store.read_block_index(1).expect("block2 index").length
    };
    advertise_required_replicas(&kura, nonzero!(2_usize));
    let configured_base = canonical_storage_budget_base_for_test(&kura);
    let retry_required = Kura::block_required_bytes(&retry_block).expect("retry block bytes");
    let retry_association_stage_required = kura
        .canonical_association_stage_additional_bytes(retry_block.as_ref(), None)
        .expect("account retry association stage");
    Arc::get_mut(&mut kura)
        .expect("exclusive kura handle")
        .max_disk_usage_bytes = configured_base
        .saturating_sub(evictable_body_len)
        .saturating_add(retry_required)
        .saturating_add(retry_association_stage_required);
    BackgroundBudgetEvictionCase {
        _temp_dir: temp_dir,
        kura,
        retry_block,
        evictable_body_len,
    }
}
impl PartialEq for BlockIndex {
    fn eq(&self, other: &Self) -> bool {
        self.start == other.start && self.length == other.length
    }
}
impl PartialEq<(u64, u64)> for BlockIndex {
    fn eq(&self, other: &(u64, u64)) -> bool {
        self.start == other.0 && self.length == other.1
    }
}
impl From<(u64, u64)> for BlockIndex {
    fn from(value: (u64, u64)) -> Self {
        Self {
            start: value.0,
            length: value.1,
        }
    }
}
fn primary_blocks_dir(dir: &TempDir) -> PathBuf {
    let lane_cfg = RuntimeLaneConfig::default();
    let blocks_dir = lane_cfg.primary().blocks_dir(dir.path());
    std::fs::create_dir_all(&blocks_dir).unwrap();
    blocks_dir
}
fn new_block_store(dir: &TempDir) -> BlockStore {
    let blocks_dir = primary_blocks_dir(dir);
    BlockStore::new(&blocks_dir)
}
fn kura_config_for_path(path: &Path, blocks_in_memory: NonZeroUsize) -> KuraConfig {
    KuraConfig {
        init_mode: InitMode::Strict,
        store_dir: WithOrigin::inline(path.to_path_buf()),
        max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
        blocks_in_memory,
        debug_output_new_blocks: false,
        merge_ledger_cache_capacity: MERGE_LEDGER_CACHE_CAPACITY,
        fsync_mode: FsyncMode::Batched,
        fsync_interval: FSYNC_INTERVAL,
        lane_history_retention: LANE_HISTORY_RETENTION,
        replica_advert: iroha_config::parameters::defaults::kura::REPLICA_ADVERT_POLICY,
    }
}
fn kura_config_for_dir(dir: &TempDir, blocks_in_memory: NonZeroUsize) -> KuraConfig {
    kura_config_for_path(dir.path(), blocks_in_memory)
}
fn open_configured_kura_with_pending_limits(
    config: &KuraConfig,
    limits: &SumeragiV2RuntimeLimits,
) -> Result<(Arc<Kura>, BlockCount)> {
    let configured = LaneCatalog::default();
    let lane_config = RuntimeLaneConfig::from_catalog(&configured);
    Kura::new_with_configured_lane_catalog_and_snapshot_bootstrap_and_sumeragi_limits(
        config,
        &lane_config,
        &configured,
        &SnapshotBootstrapPolicy::default(),
        limits,
    )
}
#[test]
fn configured_pending_control_limits_fail_before_store_creation() {
    let temp = TempDir::new().expect("temporary parent");
    let store_root = temp.path().join("kura");
    let config = kura_config_for_path(&store_root, BLOCKS_IN_MEMORY);
    let mut limits = SumeragiV2RuntimeLimits::default();
    limits.pending_certified_merge_entry_capacity =
        NonZeroUsize::new(V2_PENDING_CERTIFIED_MERGE_ENTRY_CAPACITY_MAX.saturating_add(1))
            .expect("non-zero invalid limit");
    let error = open_configured_kura_with_pending_limits(&config, &limits)
        .expect_err("out-of-range pending control limit must fail closed");
    assert!(matches!(
        error,
        Error::IO(ref source, ref path)
            if source.kind() == ErrorKind::InvalidInput
                && source
                    .to_string()
                    .contains("pending_certified_merge_entry_capacity")
                && path == &store_root
    ));
    assert!(
        !store_root.exists(),
        "invalid Sumeragi limits must be rejected before Kura creates its root"
    );
}
#[test]
fn configured_pending_control_count_limits_gate_live_admission() {
    let temp = TempDir::new().expect("temporary Kura root");
    let config = kura_config_for_dir(&temp, BLOCKS_IN_MEMORY);
    let mut limits = SumeragiV2RuntimeLimits::default();
    limits.pending_certified_merge_entry_capacity =
        NonZeroUsize::new(1).expect("non-zero merge capacity");
    limits.pending_queue_plan_admission_capacity =
        NonZeroUsize::new(1).expect("non-zero QueuePlan capacity");
    let (kura, _) =
        open_configured_kura_with_pending_limits(&config, &limits).expect("open configured Kura");
    assert_eq!(
        kura.pending_control_sidecar_limits,
        PendingControlSidecarLimits {
            certified_merge_entries: 1,
            queue_plan_admissions: 1,
            aggregate_bytes: V2_PENDING_CONTROL_SIDECAR_BYTES.get(),
        }
    );
    kura.persist_pending_certified_merge_entry(&sample_merge_entry(1))
        .expect("persist first configured merge sidecar");
    assert!(
        kura.persist_pending_certified_merge_entry(&sample_merge_entry(2))
            .is_err(),
        "configured merge capacity must reject the second identity"
    );
    kura.persist_pending_queue_plan_admission_certificate(b"configured-queue-plan-one")
        .expect("persist first configured QueuePlan sidecar");
    assert!(
        kura.persist_pending_queue_plan_admission_certificate(b"configured-queue-plan-two")
            .is_err(),
        "configured QueuePlan capacity must reject the second identity"
    );
}
#[test]
fn configured_historical_recovery_bytes_follow_runtime_limits() {
    for (label, byte_limit) in [
        ("lower", V2_PENDING_CONTROL_SIDECAR_BYTES_MIN),
        ("higher", V2_PENDING_CONTROL_SIDECAR_BYTES_MAX),
    ] {
        let temp = TempDir::new().expect("temporary configured Kura root");
        let config = kura_config_for_dir(&temp, BLOCKS_IN_MEMORY);
        let mut limits = SumeragiV2RuntimeLimits::default();
        limits.pending_control_sidecar_bytes =
            NonZeroUsize::new(byte_limit).expect("configured byte limit is non-zero");
        let (kura, _) = open_configured_kura_with_pending_limits(&config, &limits)
            .unwrap_or_else(|error| panic!("open {label}-bound configured Kura: {error}"));
        assert_eq!(
            kura.historical_autonomous_recovery_aggregate_byte_limit(),
            u64::try_from(byte_limit).expect("configured byte limit fits u64"),
            "historical recovery must use the {label} configured sidecar byte bound",
        );
    }
}
#[test]
fn configured_pending_control_count_limit_rejects_oversized_startup_inventory() {
    let temp = TempDir::new().expect("temporary Kura root");
    let config = kura_config_for_dir(&temp, BLOCKS_IN_MEMORY);
    let mut initial_limits = SumeragiV2RuntimeLimits::default();
    initial_limits.pending_certified_merge_entry_capacity =
        NonZeroUsize::new(2).expect("non-zero initial merge capacity");
    let (kura, _) = open_configured_kura_with_pending_limits(&config, &initial_limits)
        .expect("open initial configured Kura");
    kura.persist_pending_certified_merge_entry(&sample_merge_entry(1))
        .expect("persist first pending merge identity");
    kura.persist_pending_certified_merge_entry(&sample_merge_entry(2))
        .expect("persist second pending merge identity");
    drop(kura);
    let mut tightened_limits = initial_limits;
    tightened_limits.pending_certified_merge_entry_capacity =
        NonZeroUsize::new(1).expect("non-zero tightened merge capacity");
    assert!(
        open_configured_kura_with_pending_limits(&config, &tightened_limits).is_err(),
        "startup must reject durable pending inventory above the configured capacity"
    );
}
#[test]
fn configured_pending_control_shared_bytes_reject_oversized_startup_inventory() {
    let temp = TempDir::new().expect("temporary Kura root");
    let config = kura_config_for_dir(&temp, BLOCKS_IN_MEMORY);
    let mut limits = SumeragiV2RuntimeLimits::default();
    limits.pending_queue_plan_admission_capacity =
        NonZeroUsize::new(32).expect("non-zero QueuePlan capacity");
    limits.pending_control_sidecar_bytes = NonZeroUsize::new(V2_PENDING_CONTROL_SIDECAR_BYTES_MIN)
        .expect("non-zero shared byte minimum");
    let (kura, _) =
        open_configured_kura_with_pending_limits(&config, &limits).expect("open configured Kura");
    let directory = kura.pending_queue_plan_admission_dir();
    drop(kura);
    let file_bytes = u64::try_from(MAX_PENDING_QUEUE_PLAN_ADMISSION_CERTIFICATE_BYTES)
        .expect("QueuePlan per-file cap fits u64");
    let file_count = V2_PENDING_CONTROL_SIDECAR_BYTES_MIN
        .checked_div(MAX_PENDING_QUEUE_PLAN_ADMISSION_CERTIFICATE_BYTES)
        .expect("non-zero QueuePlan per-file cap")
        .saturating_add(1);
    assert!(file_count < limits.pending_queue_plan_admission_capacity.get());
    for index in 0..file_count {
        let hash = Hash::new(format!("configured-shared-byte-limit-{index}"));
        fs::File::create(directory.join(format!("{}.norito", hex::encode(hash.as_ref()))))
            .expect("create sparse pending QueuePlan sidecar")
            .set_len(file_bytes)
            .expect("size sparse pending QueuePlan sidecar");
    }
    assert!(
        open_configured_kura_with_pending_limits(&config, &limits).is_err(),
        "startup must reject combined pending-control bytes above the configured shared limit"
    );
}
