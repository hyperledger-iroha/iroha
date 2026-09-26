// Exact source-assignment revision and resolver projection test.

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the test checks one immutable job across exact archive revisions and source rotation"
)]
fn current_assignment_binding_tracks_new_revision_and_rejects_stale_source() {
    let daemon_root = physical_tempdir().expect("daemon root");
    let bounds = ProviderIngestFinalizedArchiveBoundsV1::try_new(
        2 * 1024 * 1024,
        8,
        16 * 1024 * 1024,
        8,
        8,
        16,
        2,
    )
    .expect("archive bounds");
    let archive = ProviderIngestFinalizedArchiveV1::try_open(
        daemon_root.path().join("current-assignment-archive"),
        bounds,
    )
    .expect("open archive");
    let network_id = test_network_id(0x70);
    let destination = ProviderId::new([0x51; 32]);
    let old_source = ProviderId::new([0x52; 32]);
    let new_source = ProviderId::new([0x53; 32]);
    let mut shared = replay_safe_archived_order(0x61, destination);
    let mut canonical =
        norito::decode_from_bytes::<ReplicationOrderV1>(&shared.replication_order.canonical_order)
            .expect("decode source order");
    canonical.target_replicas = 2;
    canonical.assignments.push(ReplicationAssignmentV1 {
        provider_id: *old_source.as_bytes(),
        slice_gib: 1,
        lane: None,
    });
    canonical.validate().expect("two-provider canonical order");
    shared.replication_order.canonical_order =
        norito::to_bytes(&canonical).expect("encode two-provider order");
    let first_key = ProviderIngestFinalizedArchiveKeyV1::try_new(network_id, 7, [0x71; 32], 7_000)
        .expect("first key");
    let first = ProviderIngestFinalizedProjectionV1 {
        key: first_key,
        providers: vec![
            ProviderIngestFinalizedProviderProjectionV1 {
                provider_id: destination,
                expected_owner: None,
                expected_signer_policy: None,
                orders: vec![shared.clone()],
            },
            ProviderIngestFinalizedProviderProjectionV1 {
                provider_id: old_source,
                expected_owner: None,
                expected_signer_policy: None,
                orders: vec![shared.clone()],
            },
            ProviderIngestFinalizedProviderProjectionV1 {
                provider_id: new_source,
                expected_owner: None,
                expected_signer_policy: None,
                orders: Vec::new(),
            },
        ],
    };
    archive
        .insert(first.clone())
        .expect("insert first assignment");
    let original_lookup = archive
        .read_provider_assignment(&first_key, destination, shared.replication_order.order_id)
        .expect("original lookup");
    let row = original_lookup.assignment.as_ref().expect("original row");
    let authorization = FinalizedProviderIngestAuthorizationV1::from_finalized_state(
        first_key.height,
        first_key.block_hash,
        *destination.as_bytes(),
        *row.replication_order.order_id.as_bytes(),
        *row.pin_manifest.digest.as_bytes(),
        row.pin_manifest.root_cid.as_bytes().to_vec(),
        row.pin_manifest.chunker.to_handle(),
        row.pin_manifest.chunk_digest_sha3_256,
        row.pin_manifest.por_root,
        row.pin_manifest.content_length,
    )
    .expect("original immutable job");
    let original = authenticate_current_assignment(
        network_id,
        destination,
        *old_source.as_bytes(),
        &authorization,
        original_lookup,
    )
    .expect("original current assignment");
    assert_eq!(original.key, first_key);
    assert_eq!(original.assignment.expected_assignment_revision, 1);
    assert_eq!(original.source_provider_ids, vec![*old_source.as_bytes()]);
    let original_request = ProviderIngestSourceRequestV1::new(
        authorization.clone(),
        vec![*old_source.as_bytes()],
        None,
    )
    .expect("original source request");
    assert!(original.matches_source_request(*old_source.as_bytes(), &original_request, 1));
    let original_read = materialize_current_source_assignment(
        network_id,
        *old_source.as_bytes(),
        &original_request,
        1,
        original.clone(),
    )
    .expect("read-only original resolver assignment");
    assert_eq!(original_read.network_id(), network_id);
    assert_eq!(original_read.source_provider_id(), *old_source.as_bytes());
    assert_eq!(original_read.finalized_head().height, first_key.height);
    assert_eq!(
        original_read.finalized_head().block_hash,
        first_key.block_hash
    );
    assert_eq!(
        original_read.finalized_at_unix_ms(),
        first_key.finalized_at_unix_ms
    );
    assert_eq!(original_read.assignment_revision(), 1);
    assert_eq!(
        original_read.source_provider_ids(),
        &[*old_source.as_bytes()]
    );
    assert_eq!(original_read.canonical_request(), &original_request);
    assert_eq!(
        original_read.provider_state_root(),
        original.provider_state_root
    );
    let mut next = first;
    next.key = ProviderIngestFinalizedArchiveKeyV1::try_new(network_id, 8, [0x72; 32], 8_000)
        .expect("next key");
    canonical.assignments[1].provider_id = *new_source.as_bytes();
    canonical.validate().expect("rotated canonical order");
    shared.replication_order.canonical_order =
        norito::to_bytes(&canonical).expect("encode rotated order");
    shared.replication_order.assignment_revision = 2;
    next.providers[0].orders[0] = shared.clone();
    next.providers[1].orders.clear();
    next.providers[2].orders.push(shared.clone());
    archive
        .insert(next.clone())
        .expect("insert rotated assignment");
    let current_lookup = archive
        .read_provider_assignment(&next.key, destination, shared.replication_order.order_id)
        .expect("current lookup");
    let current = authenticate_current_assignment(
        network_id,
        destination,
        *new_source.as_bytes(),
        &authorization,
        current_lookup.clone(),
    )
    .expect("current source and revision");
    assert_eq!(current.key, next.key);
    assert_eq!(current.assignment.expected_assignment_revision, 2);
    assert_eq!(current.source_provider_ids, vec![*new_source.as_bytes()]);
    let current_request = ProviderIngestSourceRequestV1::new(
        authorization.clone(),
        vec![*new_source.as_bytes()],
        None,
    )
    .expect("rotated source request");
    let current_read = materialize_current_source_assignment(
        network_id,
        *new_source.as_bytes(),
        &current_request,
        2,
        current.clone(),
    )
    .expect("read-only revised resolver assignment");
    assert_eq!(current_read.assignment_revision(), 2);
    assert_eq!(
        current_read.source_provider_ids(),
        &[*new_source.as_bytes()]
    );
    assert_ne!(current_read, original_read);
    assert_eq!(
        materialize_current_source_assignment(
            network_id,
            *new_source.as_bytes(),
            &current_request,
            1,
            current.clone(),
        ),
        Err(ProviderIngestFinalizedLedgerErrorV1::Rejected),
        "a stale revision cannot become a resolver assignment"
    );
    assert_eq!(
        materialize_current_source_assignment(
            network_id,
            *old_source.as_bytes(),
            &original_request,
            2,
            current.clone(),
        ),
        Err(ProviderIngestFinalizedLedgerErrorV1::Rejected),
        "a substituted old source cannot reuse a current revision"
    );
    assert_eq!(
        materialize_current_source_assignment(
            test_network_id(0x74),
            *new_source.as_bytes(),
            &current_request,
            2,
            current.clone(),
        ),
        Err(ProviderIngestFinalizedLedgerErrorV1::Rejected),
        "another network cannot reuse a matching order and source"
    );
    assert!(current.matches_source_request(*new_source.as_bytes(), &current_request, 2));
    assert!(!current.matches_source_request(*new_source.as_bytes(), &current_request, 1));
    assert!(!current.matches_source_request(*old_source.as_bytes(), &original_request, 2));
    assert!(!original.matches_source_request(*new_source.as_bytes(), &current_request, 2));
    assert_eq!(
        current.provider_state_root,
        current_lookup.provider_state_root
    );
    assert_eq!(
        authenticate_current_assignment(
            network_id,
            destination,
            *old_source.as_bytes(),
            &authorization,
            current_lookup.clone(),
        ),
        Err(ProviderIngestFinalizedLedgerErrorV1::Rejected)
    );
    let mut changed_pin = current_lookup;
    changed_pin
        .assignment
        .as_mut()
        .expect("current row")
        .pin_manifest
        .por_root[0] ^= 1;
    assert_eq!(
        authenticate_current_assignment(
            network_id,
            destination,
            *new_source.as_bytes(),
            &authorization,
            changed_pin,
        ),
        Err(ProviderIngestFinalizedLedgerErrorV1::Rejected)
    );
    let mut later = next;
    later.key = ProviderIngestFinalizedArchiveKeyV1::try_new(network_id, 9, [0x73; 32], 9_000)
        .expect("later unchanged assignment head");
    archive.insert(later.clone()).expect("insert later head");
    let later_lookup = archive
        .read_provider_assignment(&later.key, destination, shared.replication_order.order_id)
        .expect("later lookup");
    let later_snapshot = authenticate_current_assignment(
        network_id,
        destination,
        *new_source.as_bytes(),
        &authorization,
        later_lookup,
    )
    .expect("same assignment at a later head");
    let later_read = materialize_current_source_assignment(
        network_id,
        *new_source.as_bytes(),
        &current_request,
        2,
        later_snapshot,
    )
    .expect("later read-only assignment");
    assert_eq!(
        later_read.assignment_revision(),
        current_read.assignment_revision()
    );
    assert_eq!(
        later_read.canonical_request(),
        current_read.canonical_request()
    );
    assert_ne!(later_read.finalized_head(), current_read.finalized_head());
    assert_ne!(
        later_read, current_read,
        "a stale head must not pass recheck"
    );
    assert_eq!(
        authenticate_retained_admission_ancestor(&archive, network_id, &later.key, &authorization),
        Ok(()),
        "a retained original admission is an ancestor of the later exact head"
    );
    let authorization_at = |height, block_hash| {
        FinalizedProviderIngestAuthorizationV1::from_finalized_state(
            height,
            block_hash,
            authorization.provider_id(),
            authorization.order_id(),
            authorization.manifest_digest(),
            authorization.manifest_cid().to_vec(),
            authorization.chunker_handle().to_owned(),
            authorization.chunk_digest_sha3_256(),
            authorization.por_root(),
            authorization.content_length(),
        )
        .expect("alternate structural job cursor")
    };
    assert_eq!(
        authenticate_retained_admission_ancestor(
            &archive,
            network_id,
            &later.key,
            &authorization_at(7, [0xF7; 32]),
        ),
        Err(ProviderIngestFinalizedLedgerErrorV1::Rejected),
        "an earlier-height fork must not become an ancestor"
    );
    assert_eq!(
        authenticate_retained_admission_ancestor(
            &archive,
            network_id,
            &later.key,
            &authorization_at(6, [0x70; 32]),
        ),
        Err(ProviderIngestFinalizedLedgerErrorV1::Rejected),
        "history below the archive activation floor must fail closed"
    );
    assert_eq!(
        authenticate_retained_admission_ancestor(
            &archive,
            network_id,
            &later.key,
            &authorization_at(10, [0x74; 32]),
        ),
        Err(ProviderIngestFinalizedLedgerErrorV1::Rejected),
        "a future admission cannot authorize the current head"
    );
    let mut substituted_current = later.key;
    substituted_current.block_hash = [0xF9; 32];
    assert_eq!(
        authenticate_retained_admission_ancestor(
            &archive,
            network_id,
            &substituted_current,
            &authorization,
        ),
        Err(ProviderIngestFinalizedLedgerErrorV1::Rejected),
        "a substituted current head must not supply a historical proof"
    );
    assert_eq!(
        authenticate_retained_admission_ancestor(
            &archive,
            test_network_id(0x75),
            &later.key,
            &authorization,
        ),
        Err(ProviderIngestFinalizedLedgerErrorV1::Rejected),
        "a foreign network cannot reuse retained history"
    );
}

#[test]
fn current_assignment_rejects_substituted_musubi_binding() {
    let mut page = archive_page_with_raw_musubi_binding();
    let row = page.rows.first_mut().expect("bound assignment");
    row.pin_manifest.status = PinStatus::Approved(1);
    let archive_id = row
        .musubi_archive
        .as_ref()
        .expect("Musubi binding")
        .archive_id;
    let authorization = FinalizedProviderIngestAuthorizationV1::from_finalized_musubi_state(
        page.key.height,
        page.key.block_hash,
        *page.provider_id.as_bytes(),
        *row.replication_order.order_id.as_bytes(),
        *row.pin_manifest.digest.as_bytes(),
        row.pin_manifest.root_cid.as_bytes().to_vec(),
        row.pin_manifest.chunker.to_handle(),
        row.pin_manifest.chunk_digest_sha3_256,
        row.pin_manifest.por_root,
        row.pin_manifest.content_length,
        FinalizedProviderIngestMusubiContextV1::new(page.key.network_id, archive_id)
            .expect("Musubi context"),
    )
    .expect("immutable Musubi authorization");
    let source = [0x52; 32];
    let lookup = ProviderIngestFinalizedArchiveAssignmentLookupV1 {
        key: page.key,
        provider_id: page.provider_id,
        provider_state_root: page.provider_state_root,
        assignment: Some(row.clone()),
        source_provider_ids: vec![source],
    };
    authenticate_current_assignment(
        page.key.network_id,
        page.provider_id,
        source,
        &authorization,
        lookup.clone(),
    )
    .expect("complete Musubi binding");
    let mut substituted_order = lookup.clone();
    substituted_order
        .assignment
        .as_mut()
        .expect("bound assignment")
        .musubi_archive
        .as_mut()
        .expect("Musubi binding")
        .replication_order = ReplicationOrderId::new([0x62; 32]);
    assert_eq!(
        authenticate_current_assignment(
            page.key.network_id,
            page.provider_id,
            source,
            &authorization,
            substituted_order,
        ),
        Err(ProviderIngestFinalizedLedgerErrorV1::Rejected)
    );
    let mut substituted_commitment = lookup;
    substituted_commitment
        .assignment
        .as_mut()
        .expect("bound assignment")
        .musubi_archive
        .as_mut()
        .expect("Musubi binding")
        .commitment
        .content_length += 1;
    assert_eq!(
        authenticate_current_assignment(
            page.key.network_id,
            page.provider_id,
            source,
            &authorization,
            substituted_commitment,
        ),
        Err(ProviderIngestFinalizedLedgerErrorV1::Rejected)
    );
}
#[test]
fn current_assignment_lookup_fails_closed_without_head_and_preserves_worker_cursor() {
    let daemon_root = physical_tempdir().expect("daemon root");
    let bounds = ProviderIngestFinalizedArchiveBoundsV1::try_new(
        2 * 1024 * 1024,
        8,
        16 * 1024 * 1024,
        8,
        8,
        16,
        2,
    )
    .expect("archive bounds");
    let archive = Arc::new(
        ProviderIngestFinalizedArchiveV1::try_open(
            daemon_root.path().join("head-unavailable-archive"),
            bounds,
        )
        .expect("open archive"),
    );
    let kura = Kura::blank_kura_for_testing();
    let network_id = test_network_id(0x73);
    let state = Arc::new(State::new_with_chain_and_network_id_for_testing(
        World::default(),
        Arc::clone(&kura),
        LiveQueryStore::start_test(),
        ChainId::from("provider-ingest-current-head-unavailable"),
        network_id,
    ));
    let destination = ProviderId::new([0x51; 32]);
    let source = [0x52; 32];
    let query =
        ArchivedProviderIngestFinalizedLedgerV1::new(ArchivedProviderIngestFinalizedLedgerArgsV1 {
            network_id,
            provider_id: destination,
            archive,
            kura,
            state,
            max_page_rows: 2,
            max_kura_tip_lag_blocks: 0,
            activation_gate: ArchiveActivationGateV1::StrictLive,
        });
    let key = ProviderIngestFinalizedArchiveKeyV1::try_new(network_id, 1, [0x74; 32], 1_000)
        .expect("cursor key");
    let scan = ActiveArchiveScanV1 {
        key,
        cursor: ProviderIngestFinalizedArchiveCursorV1 {
            key,
            provider_id: destination,
            provider_state_root: [0x75; 32],
            after_order_id: ReplicationOrderId::new([0x76; 32]),
        },
    };
    *query.active.lock().expect("active scan") = Some(scan.clone());
    let order = replay_safe_archived_order(0x61, destination);
    let authorization = FinalizedProviderIngestAuthorizationV1::from_finalized_state(
        1,
        key.block_hash,
        *destination.as_bytes(),
        *order.replication_order.order_id.as_bytes(),
        *order.pin_manifest.digest.as_bytes(),
        order.pin_manifest.root_cid.as_bytes().to_vec(),
        order.pin_manifest.chunker.to_handle(),
        order.pin_manifest.chunk_digest_sha3_256,
        order.pin_manifest.por_root,
        order.pin_manifest.content_length,
    )
    .expect("immutable job");
    assert_eq!(
        query.lookup_current_assignment(network_id, source, &authorization),
        Err(ProviderIngestFinalizedLedgerErrorV1::Unavailable),
        "height-zero State/Kura cannot be treated as a current finalized assignment"
    );
    let request = ProviderIngestSourceRequestV1::new(authorization, vec![source], None)
        .expect("payload-free request");
    let token_signer =
        KeyPair::try_from_seed(vec![0x31; 32], Algorithm::Ed25519).expect("source token key");
    let token_binding = sorafs_manifest::signer::custody::SignerCustodyBindingV1 {
        chain_id: "provider-ingest-current-head-unavailable".to_owned(),
        network_id: *network_id.as_bytes(),
        runtime_handle: "software://sorafs/stream-token/source".to_owned(),
        key_handle: "software://sorafs/stream-token/key-1".to_owned(),
        service_id: "source-token-signer".to_owned(),
        administrator_id: "source-token-administrator".to_owned(),
        role: sorafs_manifest::signer::protocol::SignerRoleV1::StreamToken,
        purpose: sorafs_manifest::signer::protocol::SignerPurposeBindingV1::StreamToken {
            provider_id: source,
        },
        algorithm: sorafs_manifest::signer::protocol::SignerKeyAlgorithmV1::Ed25519,
        public_key: token_signer.public_key().clone(),
        key_revision: 1,
        policy_revision: 1,
        policy_digest: [0x41; 32],
    };
    assert_eq!(
        query.read_current_source_assignment_for_resolver(network_id, source, &request, 1),
        Err(ProviderIngestFinalizedLedgerErrorV1::Unavailable),
        "the resolver-facing service requires a committed State/Kura head"
    );
    assert_eq!(
        query.read_current_source_stream_token_custody_for_resolver(
            network_id,
            source,
            &request,
            1,
            &token_binding,
        ),
        Err(ProviderIngestFinalizedLedgerErrorV1::Unavailable),
        "a configured token key cannot create missing finalized source authority"
    );
    let mut substituted_token_binding = token_binding;
    substituted_token_binding.purpose =
        sorafs_manifest::signer::protocol::SignerPurposeBindingV1::StreamToken {
            provider_id: [0x53; 32],
        };
    assert_eq!(
        query.read_current_source_stream_token_custody_for_resolver(
            network_id,
            source,
            &request,
            1,
            &substituted_token_binding,
        ),
        Err(ProviderIngestFinalizedLedgerErrorV1::Rejected),
        "a substituted source role must fail before finality selection"
    );
    assert_eq!(
        query.read_current_source_assignment_for_resolver(network_id, source, &request, 0),
        Err(ProviderIngestFinalizedLedgerErrorV1::Rejected),
        "zero assignment revision is never a current source request"
    );
    let inspected = std::cell::Cell::new(false);
    assert_eq!(
        query.validate_with_current_source_request(network_id, source, &request, 1, || {
            inspected.set(true);
            Ok(())
        }),
        Err(ProviderIngestSourceFetchErrorV1::Unavailable),
        "missing live finality must stop before inspecting other authority inputs"
    );
    assert!(!inspected.get());
    let after = query.active.lock().expect("retained active scan");
    assert_eq!(after.as_ref().map(|scan| scan.key), Some(scan.key));
    assert_eq!(after.as_ref().map(|scan| scan.cursor), Some(scan.cursor));
}
