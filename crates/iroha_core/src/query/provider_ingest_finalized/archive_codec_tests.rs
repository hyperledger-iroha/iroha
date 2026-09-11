// Canonical frame identity, archive persistence, and strict decode coverage.
#[cfg(unix)]
#[test]
fn retention_restart_rejects_extra_crash_candidate_without_cleanup() {
    let directory = physical_tempdir().expect("archive tempdir");
    let root = archive_root(&directory);
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(&root, bounds()).expect("open archive");
    let first = projection(7);
    let second = advance_projection(&first, 8);
    let third = advance_projection(&second, 9);
    let fourth = advance_projection(&third, 10);
    archive.insert(first).expect("insert first");
    archive.insert(second.clone()).expect("insert second");
    archive.insert(third.clone()).expect("insert third");
    let (_prior_fence, _prior_prepared, prior_proposal) =
        prepared_compaction_for_test(&archive, second.key.clone());
    let authority = TestRetentionAuthority::new();
    let binding = authority.binding();
    let prior_approval = ProviderIngestFinalizedArchiveRetentionApprovalRecordV1::try_new(
        1,
        binding.qualification(),
        prior_proposal,
        None,
        None,
    )
    .expect("construct predecessor approval");
    let prior_outcome = compact_for_test(&archive, second.key);
    archive.insert(fourth.clone()).expect("insert fourth");
    let (_approved_fence, approved_prepared, approved_proposal) =
        prepared_compaction_for_test(&archive, third.key.clone());
    let (_extra_fence, extra_prepared, _extra_proposal) =
        prepared_compaction_for_test(&archive, fourth.key);
    let approval = ProviderIngestFinalizedArchiveRetentionApprovalRecordV1::try_new(
        2,
        binding.qualification(),
        approved_proposal,
        Some(prior_approval.revision()),
        Some(prior_outcome.checkpoint_digest()),
    )
    .expect("construct successor approval");
    *authority.latest.lock().expect("lock latest approval") = Some(approval);
    publish_prepared_checkpoint_for_test(&archive, &approved_prepared);
    publish_prepared_checkpoint_for_test(&archive, &extra_prepared);
    let records_before = archive_namespace_snapshot(&archive.records);
    let checkpoints_before = archive_namespace_snapshot(&archive.checkpoints);
    assert_eq!(
        checkpoints_before.len(),
        3,
        "predecessor, approved, and extra checkpoints model the interrupted namespace"
    );
    let network_id = third.key.network_id;
    drop(archive);
    let kura = Kura::blank_kura_for_testing();
    assert!(matches!(
        ProviderIngestFinalizedArchiveV1::try_open_with_retention_authority(
            &root,
            bounds(),
            &network_id,
            kura.as_ref(),
            &binding,
            &authority,
        ),
        Err(ProviderIngestFinalizedArchiveErrorV1::UnapprovedRetentionCheckpoint)
    ));
    assert_eq!(
        archive_namespace_snapshot(&root.join(RECORDS_DIRECTORY)),
        records_before,
        "rejected restart must not continue prefix cleanup"
    );
    assert_eq!(
        archive_namespace_snapshot(&root.join(CHECKPOINTS_DIRECTORY)),
        checkpoints_before,
        "rejected restart must preserve every checkpoint for operator recovery"
    );
}
#[cfg(unix)]
#[test]
fn retention_prepare_ambiguous_cas_and_exact_readback_gate_publication() {
    let directory = physical_tempdir().expect("archive tempdir");
    let archive = ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds())
        .expect("open archive");
    let first = projection(7);
    let second = advance_projection(&first, 8);
    let third = advance_projection(&second, 9);
    archive.insert(first).expect("insert first");
    archive.insert(second.clone()).expect("insert second");
    archive.insert(third).expect("insert suffix");
    let (_fence, prepared, proposal) = prepared_compaction_for_test(&archive, second.key.clone());
    assert_eq!(
        fs::read_dir(archive_root(&directory).join(CHECKPOINTS_DIRECTORY))
            .expect("read checkpoint namespace")
            .count(),
        0,
        "preparation must not publish a checkpoint"
    );
    let authority = TestRetentionAuthority::new();
    authority.set_behavior(TestRetentionCasBehavior::ApplyAmbiguous);
    let binding = authority.binding();
    let approval = ProviderIngestFinalizedArchiveRetentionApprovalRecordV1::try_new(
        1,
        binding.qualification(),
        proposal,
        None,
        None,
    )
    .expect("construct approval");
    compare_and_read_back_retention_approval(
        &binding,
        &authority,
        &second.key.network_id,
        None,
        &approval,
    )
    .expect("ambiguous CAS is resolved only by exact authoritative readback");
    assert_eq!(
        fs::read_dir(archive_root(&directory).join(CHECKPOINTS_DIRECTORY))
            .expect("read checkpoint namespace")
            .count(),
        0,
        "authority approval alone must not mutate local archive storage"
    );
    require_exact_retention_readback(&binding, &authority, &second.key.network_id, &approval)
        .expect("approval remains authoritative");
    let mut index = archive.write_index().expect("lock approved compaction");
    let outcome = archive
        .publish_prepared_compaction(&mut index, prepared, || {}, &mut |_| {})
        .expect("publish only after exact approval");
    assert_eq!(outcome.retention_floor(), &second.key);
    drop(index);
    assert_eq!(
        archive
            .retention_floor(&second.key.network_id)
            .expect("retention floor"),
        Some(second.key)
    );
}
#[cfg(unix)]
#[test]
fn unchanged_equivocating_and_rollback_authorities_never_publish() {
    let directory = physical_tempdir().expect("archive tempdir");
    let archive = ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds())
        .expect("open archive");
    let first = projection(7);
    let second = advance_projection(&first, 8);
    archive.insert(first).expect("insert first");
    archive.insert(second.clone()).expect("insert second");
    let (fence, _prepared, proposal) = prepared_compaction_for_test(&archive, second.key.clone());
    let authority = TestRetentionAuthority::new();
    let binding = authority.binding();
    let approval = ProviderIngestFinalizedArchiveRetentionApprovalRecordV1::try_new(
        1,
        binding.qualification(),
        proposal.clone(),
        None,
        None,
    )
    .expect("construct approval");
    authority.set_behavior(TestRetentionCasBehavior::LeaveUnchanged);
    assert!(matches!(
        compare_and_read_back_retention_approval(
            &binding,
            &authority,
            &second.key.network_id,
            None,
            &approval,
        ),
        Err(ProviderIngestFinalizedArchiveErrorV1::RetentionAuthorityCasUnchanged)
    ));
    let competing_proposal = ProviderIngestFinalizedArchiveCompactionProposalV1::try_new(
        fence.clone(),
        [0xE1; 32],
        [0xE2; 32],
    )
    .expect("construct competing proposal");
    let competing = ProviderIngestFinalizedArchiveRetentionApprovalRecordV1::try_new(
        1,
        binding.qualification(),
        competing_proposal,
        None,
        None,
    )
    .expect("construct competing approval");
    authority.set_competing(competing);
    authority.set_behavior(TestRetentionCasBehavior::Equivocate);
    assert!(matches!(
        compare_and_read_back_retention_approval(
            &binding,
            &authority,
            &second.key.network_id,
            None,
            &approval,
        ),
        Err(ProviderIngestFinalizedArchiveErrorV1::RetentionAuthorityEquivocation)
    ));
    assert!(matches!(
        validate_retention_authority_predecessor(Some(&approval), None, &fence),
        Err(ProviderIngestFinalizedArchiveErrorV1::RetentionAuthorityRollback)
    ));
    assert_eq!(
        fs::read_dir(archive_root(&directory).join(CHECKPOINTS_DIRECTORY))
            .expect("read checkpoint namespace")
            .count(),
        0,
        "failed authority decisions must not publish local checkpoint bytes"
    );
}
#[test]
fn retention_approval_canonical_decode_is_strict_and_bounded() {
    let proposal = ProviderIngestFinalizedArchiveCompactionProposalV1::try_new(
        retention_fence(key(7), 1),
        [0xC1; 32],
        [0xC2; 32],
    )
    .expect("construct proposal");
    let authority = TestRetentionAuthority::new();
    let approval = ProviderIngestFinalizedArchiveRetentionApprovalRecordV1::try_new(
        1,
        authority.qualification,
        proposal,
        None,
        None,
    )
    .expect("construct approval");
    let bytes = approval.to_canonical_bytes().expect("encode approval");
    assert_eq!(
        ProviderIngestFinalizedArchiveRetentionApprovalRecordV1::from_canonical_bytes(&bytes)
            .expect("decode approval"),
        approval
    );
    let mut trailing = bytes;
    trailing.push(0);
    assert!(
        ProviderIngestFinalizedArchiveRetentionApprovalRecordV1::from_canonical_bytes(&trailing)
            .is_err()
    );
    assert!(
        ProviderIngestFinalizedArchiveRetentionApprovalRecordV1::from_canonical_bytes(&vec![
            0;
            RETENTION_APPROVAL_MAX_CANONICAL_BYTES_V1
                + 1
        ])
        .is_err()
    );
}
#[test]
fn retention_approval_rejects_same_label_foreign_genesis_network() {
    let authority = TestRetentionAuthority::new();
    let binding = authority.binding();
    let proposal = ProviderIngestFinalizedArchiveCompactionProposalV1::try_new(
        retention_fence(key(7), 1),
        [0xC3; 32],
        [0xC4; 32],
    )
    .expect("construct exact-network proposal");
    let approval = ProviderIngestFinalizedArchiveRetentionApprovalRecordV1::try_new(
        1,
        binding.qualification(),
        proposal,
        None,
        None,
    )
    .expect("construct exact-network approval");
    // Both deployments may carry the same human-facing ChainName. Only the
    // genesis-derived NetworkId enters this durable approval namespace.
    assert!(
        validate_retention_approval_record(&approval, &binding, &test_network_id(0x33),).is_err()
    );
}
#[test]
fn retention_authority_binding_rejects_test_marked_substituted_and_stale_providers() {
    assert!(matches!(
        ProviderIngestFinalizedArchiveRetentionAuthorityBindingV1::try_new(
            "sealed://sorafs/provider-ingest/test".to_owned(),
            1,
            [1; 32],
        ),
        Err(ProviderIngestFinalizedArchiveErrorV1::InvalidRetentionAuthorityBinding)
    ));
    let expected = TestRetentionAuthority::new();
    let binding = expected.binding();
    let mut substituted = TestRetentionAuthority::new();
    substituted.handle = "sealed://sorafs/provider-ingest/archive-retention-secondary".to_owned();
    assert!(matches!(
        assert_retention_authority_identity(&binding, &substituted),
        Err(ProviderIngestFinalizedArchiveErrorV1::RetentionAuthoritySubstitution)
    ));
    let mut stale = TestRetentionAuthority::new();
    stale.qualification = ProviderIngestFinalizedArchiveRetentionAuthorityQualificationV1::new(
        binding.qualification().revision() - 1,
        binding.qualification().policy_digest(),
    );
    assert!(matches!(
        assert_retention_authority_identity(&binding, &stale),
        Err(ProviderIngestFinalizedArchiveErrorV1::RetentionAuthoritySubstitution)
    ));
}
#[test]
fn bounds_reject_zero_and_inconsistent_page_limits() {
    assert!(matches!(
        ProviderIngestFinalizedArchiveBoundsV1::try_new(0, 1, 1, 1, 1, 1, 1),
        Err(ProviderIngestFinalizedArchiveErrorV1::InvalidBounds { .. })
    ));
    assert!(matches!(
        ProviderIngestFinalizedArchiveBoundsV1::try_new(1024, 1, 1024, 1, 1, 1, 2),
        Err(ProviderIngestFinalizedArchiveErrorV1::InvalidBounds { .. })
    ));
}
#[test]
fn exact_replay_pagination_and_provider_index_isolation_are_deterministic() {
    let directory = physical_tempdir().expect("archive tempdir");
    let root = archive_root(&directory);
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(&root, bounds()).expect("open archive");
    let first = projection(7);
    assert_eq!(
        archive.insert(first.clone()).expect("insert first"),
        ProviderIngestFinalizedArchiveInsertOutcomeV1::Inserted
    );
    assert_eq!(
        archive.insert(first.clone()).expect("exact replay"),
        ProviderIngestFinalizedArchiveInsertOutcomeV1::ExactReplay
    );
    assert_eq!(
        archive
            .activation_floor(&first.key.network_id)
            .expect("activation floor"),
        Some(first.key.clone())
    );
    assert_eq!(
        archive
            .resolve_exact_key(
                &first.key.network_id,
                first.key.height,
                first.key.block_hash
            )
            .expect("resolve height/hash cursor"),
        first.key
    );
    let page_one = archive
        .read_provider_page(&first.key, PROVIDER_A, None, 1)
        .expect("first provider A page");
    assert_eq!(page_one.rows.len(), 1);
    assert!(
        page_one
            .rows
            .iter()
            .all(|row| row.provider_id == PROVIDER_A)
    );
    let expected_provider_a = first
        .providers
        .iter()
        .find(|provider| provider.provider_id == PROVIDER_A)
        .expect("provider A projection");
    assert_eq!(
        page_one.rows[0].expected_owner,
        expected_provider_a.expected_owner
    );
    assert_eq!(
        page_one.rows[0].expected_signer_policy,
        expected_provider_a.expected_signer_policy
    );
    assert_eq!(
        page_one.rows[0].expected_assignment_revision,
        page_one.rows[0].replication_order.assignment_revision
    );
    assert_eq!(
        page_one.rows[0].finalized_anchor,
        first.key.finalized_anchor()
    );
    let cursor = page_one.next_cursor.clone().expect("second page cursor");
    let page_two = archive
        .read_provider_page(&first.key, PROVIDER_A, Some(&cursor), 1)
        .expect("second provider A page");
    assert_eq!(page_two.rows.len(), 1);
    assert!(page_two.next_cursor.is_none());
    assert_ne!(
        page_one.rows[0].replication_order.order_id,
        page_two.rows[0].replication_order.order_id
    );
    let provider_b = archive
        .read_provider_page(&first.key, PROVIDER_B, None, 1)
        .expect("provider B page");
    assert_eq!(provider_b.rows.len(), 1);
    assert_eq!(provider_b.rows[0].provider_id, PROVIDER_B);
    assert_eq!(
        provider_b.rows[0].replication_order.order_id,
        page_one.rows[0].replication_order.order_id
    );
    let empty = archive
        .read_provider_page(&first.key, PROVIDER_EMPTY, None, 1)
        .expect("empty provider page");
    assert!(empty.rows.is_empty());
    assert!(empty.next_cursor.is_none());
    let second_directory = physical_tempdir().expect("second archive tempdir");
    let second_root = archive_root(&second_directory);
    let second_archive = ProviderIngestFinalizedArchiveV1::try_open(&second_root, bounds())
        .expect("open second archive");
    second_archive
        .insert(first.clone())
        .expect("insert same projection");
    let bytes_a =
        fs::read(archive.record_path(&first.key).expect("first path")).expect("read first bytes");
    let bytes_b = fs::read(second_archive.record_path(&first.key).expect("second path"))
        .expect("read second bytes");
    assert_eq!(bytes_a, bytes_b);
}
#[test]
fn unchanged_successor_uses_empty_delta_but_serves_its_exact_anchor() {
    let directory = physical_tempdir().expect("archive tempdir");
    let archive = ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds())
        .expect("open archive");
    let first = projection(7);
    let mut second = advance_projection(&first, 8);
    second.key.finalized_at_unix_ms = 42_999;
    archive.insert(first).expect("insert activation floor");
    archive
        .insert(second.clone())
        .expect("insert unchanged exact successor");
    let record = load_record_at(
        &archive.record_path(&second.key).expect("successor path"),
        bounds(),
        Some(&second.key),
    )
    .expect("load successor record");
    assert!(
        record.material.deltas.is_empty(),
        "unchanged provider state must not be copied into every anchor"
    );
    let page = archive
        .read_provider_page(&second.key, PROVIDER_A, None, 1)
        .expect("exact successor page");
    assert_eq!(page.rows[0].finalized_anchor, second.key.finalized_anchor());
    assert_eq!(
        page.rows[0].finalized_at_unix_ms,
        second.key.finalized_at_unix_ms
    );
    assert_eq!(page.rows[0].completion_epoch, Some(42));
    assert_ne!(page.rows[0].completion_epoch, Some(second.key.height));
}

macro_rules! assert_provider_query_frame {
    ($value:expr, $type:ident) => {{
        let value: &$type = &$value;
        let bytes = norito::to_bytes(value).expect("encode declared query frame");
        assert_eq!(
            norito::core::Header::read(bytes.as_slice())
                .expect("read frame header")
                .schema,
            norito::core::schema_hash_for_name(concat!(
                "iroha_core::query::provider_ingest_finalized::",
                stringify!($type)
            ))
        );
        let decoded =
            norito::decode_from_bytes::<$type>(&bytes).expect("decode declared query frame");
        assert_eq!(&decoded, value);
        assert_eq!(
            norito::to_bytes(&decoded).expect("re-encode canonical frame"),
            bytes
        );
        assert!(matches!(
            norito::decode_from_bytes::<u64>(&bytes),
            Err(norito::Error::SchemaMismatch)
        ));
        bytes
    }};
}

#[test]
fn persisted_record_and_page_frames_have_distinct_logical_identities() {
    let directory = physical_tempdir().expect("archive tempdir");
    let root = archive_root(&directory);
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(&root, bounds()).expect("open archive");
    let first = projection(7);
    archive
        .insert(first.clone())
        .expect("persist populated projection");
    let path = archive.record_path(&first.key).expect("record path");
    let record = load_record_at(&path, bounds(), Some(&first.key)).expect("load persisted record");
    let record_bytes = assert_provider_query_frame!(record, ProviderIngestFinalizedArchiveRecordV1);
    assert_eq!(
        record_bytes,
        fs::read(&path).expect("read exact persisted bytes")
    );
    assert_provider_query_frame!(
        record.material,
        ProviderIngestFinalizedArchiveRecordMaterialV1
    );
    assert_provider_query_frame!(first.key, ProviderIngestFinalizedArchiveKeyV1);
    assert_provider_query_frame!(
        first.providers[0],
        ProviderIngestFinalizedProviderProjectionV1
    );
    let page = archive
        .read_provider_page(&first.key, PROVIDER_A, None, 1)
        .expect("read populated page");
    assert_eq!(page.rows.len(), 1);
    let cursor = page.next_cursor.as_ref().expect("continuation exists");
    assert_provider_query_frame!(*cursor, ProviderIngestFinalizedArchiveCursorV1);
    let page_bytes = assert_provider_query_frame!(page, ProviderIngestFinalizedArchivePageV1);
    let link = ProviderIngestFinalizedPrefixLinkV1 {
        previous_cumulative_digest: None,
        key: first.key,
        record_digest: record.record_digest,
    };
    assert_provider_query_frame!(link, ProviderIngestFinalizedPrefixLinkV1);
    drop(archive);
    let reopened = ProviderIngestFinalizedArchiveV1::try_open(&root, bounds())
        .expect("reopen canonical archive");
    assert_eq!(
        reopened
            .read_provider_page(&first.key, PROVIDER_A, None, 1)
            .expect("read reopened page"),
        page
    );
    fs::write(&path, page_bytes).expect("substitute a valid page frame for the record");
    assert!(matches!(
        load_record_at(&path, bounds(), Some(&first.key)),
        Err(ProviderIngestFinalizedArchiveErrorV1::Decode {
            source: norito::Error::SchemaMismatch,
            ..
        })
    ));
    drop(reopened);
    assert!(ProviderIngestFinalizedArchiveV1::try_open(&root, bounds()).is_err());
}

#[test]
fn prepared_checkpoint_frames_roundtrip_and_reject_material_as_the_root() {
    let directory = physical_tempdir().expect("archive tempdir");
    let archive = ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds())
        .expect("open archive");
    let first = projection(7);
    let second = advance_projection(&first, 8);
    let third = advance_projection(&second, 9);
    archive.insert(first).expect("persist activation floor");
    archive
        .insert(second.clone())
        .expect("persist retention floor");
    archive.insert(third).expect("persist retained successor");
    let (_, prepared, _) = prepared_compaction_for_test(&archive, second.key);
    let bytes = assert_provider_query_frame!(
        prepared.checkpoint,
        ProviderIngestFinalizedArchiveCheckpointV1
    );
    assert_eq!(bytes, prepared.canonical_bytes);
    let material_bytes = assert_provider_query_frame!(
        prepared.checkpoint.material,
        ProviderIngestFinalizedArchiveCheckpointMaterialV1
    );
    let path = archive
        .checkpoints
        .join(checkpoint_file_name(prepared.checkpoint.checkpoint_digest));
    fs::write(&path, &bytes).expect("persist prepared canonical checkpoint");
    assert_eq!(
        load_checkpoint_at(&path, bounds()).expect("load canonical checkpoint"),
        prepared.checkpoint
    );
    fs::write(&path, material_bytes).expect("substitute valid material frame");
    assert!(matches!(
        load_checkpoint_at(&path, bounds()),
        Err(ProviderIngestFinalizedArchiveErrorV1::Decode {
            source: norito::Error::SchemaMismatch,
            ..
        })
    ));
}

#[test]
fn retention_material_frames_cannot_substitute_for_the_approval_root() {
    let proposal = ProviderIngestFinalizedArchiveCompactionProposalV1::try_new(
        retention_fence(key(7), 1),
        [0xC1; 32],
        [0xC2; 32],
    )
    .expect("construct valid proposal");
    assert_provider_query_frame!(
        proposal.material,
        ProviderIngestFinalizedArchiveCompactionProposalMaterialV1
    );
    let authority = TestRetentionAuthority::new();
    let approval = ProviderIngestFinalizedArchiveRetentionApprovalRecordV1::try_new(
        1,
        authority.qualification,
        proposal,
        None,
        None,
    )
    .expect("construct valid approval");
    let canonical = assert_provider_query_frame!(
        approval,
        ProviderIngestFinalizedArchiveRetentionApprovalRecordV1
    );
    assert_eq!(
        approval
            .to_canonical_bytes()
            .expect("production approval encoder"),
        canonical
    );
    assert_eq!(
        ProviderIngestFinalizedArchiveRetentionApprovalRecordV1::from_canonical_bytes(&canonical)
            .expect("production approval decoder"),
        approval
    );
    let foreign = assert_provider_query_frame!(
        approval.material,
        ProviderIngestFinalizedArchiveRetentionApprovalMaterialV1
    );
    assert!(matches!(
        norito::decode_from_bytes::<ProviderIngestFinalizedArchiveRetentionApprovalRecordV1>(
            &foreign
        ),
        Err(norito::Error::SchemaMismatch)
    ));
    assert!(
        ProviderIngestFinalizedArchiveRetentionApprovalRecordV1::from_canonical_bytes(&foreign)
            .is_err()
    );
}

fn archived_pin_policy() -> PinPolicy {
    PinPolicy {
        retention_epoch: 1_000,
        ..PinPolicy::default()
    }
}

#[test]
fn archived_pin_fixture_respects_the_approval_retention_boundary() {
    let mut pin = archived_order(0x23, &[PROVIDER_A]).pin_manifest;
    validate_pin_manifest_lifecycle(&pin).expect("shared archive fixture is canonical");
    let retention_epoch = pin.policy.retention_epoch;
    pin.approve(retention_epoch - 1, None);
    validate_pin_manifest_lifecycle(&pin).expect("approval before retention is canonical");
    pin.approve(retention_epoch, None);
    assert!(matches!(
        validate_pin_manifest_lifecycle(&pin),
        Err(ProviderIngestFinalizedArchiveErrorV1::InvalidProjection {
            reason: "pin-manifest lifecycle state is noncanonical",
        })
    ));
    pin.policy = PinPolicy::default();
    pin.approve(1, None);
    assert!(matches!(
        validate_pin_manifest_lifecycle(&pin),
        Err(ProviderIngestFinalizedArchiveErrorV1::InvalidProjection {
            reason: "pin-manifest lifecycle state is noncanonical",
        })
    ));
}
